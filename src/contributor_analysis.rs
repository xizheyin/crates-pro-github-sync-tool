use chrono::{DateTime, Duration, FixedOffset};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use tokio::process::Command as TokioCommand;
use tracing::{debug, error, info, warn};

// 贡献者分析结果
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContributorAnalysis {
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub commits_count: usize,
    pub timezone_stats: HashMap<String, usize>,
    pub china_probability: f64,
    pub common_timezone: String,
    pub commit_hours: HashMap<u32, usize>,
}

// 中国相关时区
const CHINA_TIMEZONES: [&str; 4] = ["+0800", "+08:00", "CST", "Asia/Shanghai"];

// 工作时间
const WORKING_HOURS_START: u32 = 9; // 上午9点
const WORKING_HOURS_END: u32 = 18; // 下午6点

/// 判断时区是否可能是中国时区
fn is_china_timezone(timezone: &str) -> bool {
    CHINA_TIMEZONES.iter().any(|&tz| timezone.contains(tz))
}

/// 解析时区偏移量
fn parse_timezone_offset(timezone: &str) -> Option<FixedOffset> {
    // 处理格式如 +0800, +08:00
    if timezone.starts_with('+') || timezone.starts_with('-') {
        let sign = if timezone.starts_with('+') { 1 } else { -1 };
        let tz_str = timezone.trim_start_matches(|c| c == '+' || c == '-');

        // 尝试解析格式如 0800
        if tz_str.len() == 4 {
            if let (Ok(hours), Ok(minutes)) =
                (tz_str[0..2].parse::<i32>(), tz_str[2..4].parse::<i32>())
            {
                return FixedOffset::east_opt(sign * (hours * 3600 + minutes * 60));
            }
        }

        // 尝试解析格式如 08:00
        if tz_str.len() == 5 && tz_str.contains(':') {
            let parts: Vec<&str> = tz_str.split(':').collect();
            if parts.len() == 2 {
                if let (Ok(hours), Ok(minutes)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>())
                {
                    return FixedOffset::east_opt(sign * (hours * 3600 + minutes * 60));
                }
            }
        }
    }

    // 处理特定时区名称
    match timezone {
        "CST" => FixedOffset::east_opt(8 * 3600), // 假设CST是中国标准时间
        "Asia/Shanghai" => FixedOffset::east_opt(8 * 3600),
        "Asia/Beijing" => FixedOffset::east_opt(8 * 3600),
        _ => None,
    }
}

/// 分析贡献者的时区统计
pub async fn analyze_contributor_timezone(
    repo_path: &str,
    author_email: &str,
) -> Option<ContributorAnalysis> {
    if !Path::new(repo_path).exists() {
        error!("仓库路径不存在: {}", repo_path);
        return None;
    }

    debug!("分析作者 {} 的时区统计", author_email);

    // 获取作者信息
    let author_info = match get_author_info(repo_path, author_email).await {
        Some(info) => info,
        None => {
            warn!("无法获取作者信息: {}", author_email);
            return None;
        }
    };

    // 获取提交时区分布
    let commits = match get_author_commits(repo_path, author_email).await {
        Some(commits) => commits,
        None => {
            warn!("无法获取作者提交: {}", author_email);
            return None;
        }
    };

    if commits.is_empty() {
        warn!("作者没有提交记录: {}", author_email);
        return None;
    }

    let mut timezone_stats: HashMap<String, usize> = HashMap::new();
    let mut commit_hours: HashMap<u32, usize> = HashMap::new();
    let mut china_tz_count = 0;

    // 分析每个提交的时区
    for commit in &commits {
        let timezone = &commit.timezone;

        // 更新时区统计
        *timezone_stats.entry(timezone.clone()).or_insert(0) += 1;

        // 检查是否为中国时区
        if is_china_timezone(timezone) {
            china_tz_count += 1;
        }

        // 提取提交小时并更新统计
        if let Ok(hour) = commit.datetime.format("%H").to_string().parse::<u32>() {
            *commit_hours.entry(hour).or_insert(0) += 1;
        }
    }

    // 计算中国时区的概率
    let china_probability = china_tz_count as f64 / commits.len() as f64;

    // 找出最常用的时区
    let common_timezone = timezone_stats
        .iter()
        .max_by_key(|(_, &count)| count)
        .map(|(tz, _)| tz.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    let analysis = ContributorAnalysis {
        login: author_info.login,
        name: author_info.name,
        email: Some(author_email.to_string()),
        commits_count: commits.len(),
        timezone_stats,
        china_probability,
        common_timezone,
        commit_hours,
    };

    Some(analysis)
}

/// 获取作者信息
async fn get_author_info(repo_path: &str, author_email: &str) -> Option<AuthorInfo> {
    let output = TokioCommand::new("git")
        .current_dir(repo_path)
        .args(&[
            "log",
            "--format=%an|%ae",
            "--author",
            author_email,
            "-n",
            "1",
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = stdout.trim().split('|').collect();

    if parts.len() >= 2 {
        return Some(AuthorInfo {
            login: author_email
                .split('@')
                .next()
                .unwrap_or(author_email)
                .to_string(),
            name: Some(parts[0].to_string()),
            email: Some(parts[1].to_string()),
        });
    }

    Some(AuthorInfo {
        login: author_email
            .split('@')
            .next()
            .unwrap_or(author_email)
            .to_string(),
        name: None,
        email: Some(author_email.to_string()),
    })
}

#[derive(Debug, Clone)]
struct AuthorInfo {
    login: String,
    name: Option<String>,
    email: Option<String>,
}

#[derive(Debug)]
struct CommitInfo {
    datetime: DateTime<FixedOffset>,
    timezone: String,
}

/// 获取作者的所有提交
async fn get_author_commits(repo_path: &str, author_email: &str) -> Option<Vec<CommitInfo>> {
    let output = TokioCommand::new("git")
        .current_dir(repo_path)
        .args(&[
            "log",
            "--format=%aI", // ISO 8601 格式的作者日期
            "--author",
            author_email,
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout
        .trim()
        .split('\n')
        .filter(|l| !l.is_empty())
        .collect();

    let mut commits = Vec::new();

    for line in lines {
        if let Ok(dt) = line.parse::<DateTime<FixedOffset>>() {
            // 提取时区部分
            let timezone = if let Some(pos) = line.rfind(|c| c == '+' || c == '-') {
                line[pos..].to_string()
            } else if line.contains("Z") {
                "Z".to_string() // UTC
            } else {
                "Unknown".to_string()
            };

            commits.push(CommitInfo {
                datetime: dt,
                timezone,
            });
        }
    }

    Some(commits)
}

/// 判断贡献者是否可能来自中国
pub fn is_likely_from_china(analysis: &ContributorAnalysis) -> bool {
    // 贡献者使用中国时区的概率大于70%
    if analysis.china_probability > 0.7 {
        return true;
    }

    // 如果最常用的时区是中国时区
    if is_china_timezone(&analysis.common_timezone) {
        return true;
    }

    // 分析工作时间模式
    let total_commits: usize = analysis.commit_hours.values().sum();
    if total_commits > 0 {
        let china_working_hours_commits: usize = analysis
            .commit_hours
            .iter()
            .filter(|(&hour, _)| hour >= WORKING_HOURS_START && hour <= WORKING_HOURS_END)
            .map(|(_, &count)| count)
            .sum();

        let working_hours_ratio = china_working_hours_commits as f64 / total_commits as f64;
        if working_hours_ratio > 0.6 {
            return true;
        }
    }

    false
}

/// 分析仓库的所有贡献者
pub async fn analyze_repository_contributors(repo_path: &str) -> Vec<ContributorAnalysis> {
    let mut results = Vec::new();

    // 获取所有贡献者的邮箱
    let emails = match get_all_contributor_emails(repo_path).await {
        Some(emails) => emails,
        None => {
            error!("无法获取仓库贡献者邮箱: {}", repo_path);
            return results;
        }
    };

    info!("发现 {} 个贡献者邮箱", emails.len());

    // 分析每个贡献者
    for email in emails {
        if let Some(analysis) = analyze_contributor_timezone(repo_path, &email).await {
            debug!(
                "分析完成: {} (可能来自中国: {})",
                email,
                if is_likely_from_china(&analysis) {
                    "是"
                } else {
                    "否"
                }
            );
            results.push(analysis);
        }
    }

    // 按提交数量排序
    results.sort_by(|a, b| b.commits_count.cmp(&a.commits_count));

    results
}

/// 获取所有贡献者的邮箱
async fn get_all_contributor_emails(repo_path: &str) -> Option<Vec<String>> {
    let output = TokioCommand::new("git")
        .current_dir(repo_path)
        .args(&["shortlog", "-sen", "HEAD"])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout
        .trim()
        .split('\n')
        .filter(|l| !l.is_empty())
        .collect();

    let mut emails = Vec::new();

    for line in lines {
        // 格式通常是: 123  Name <email@example.com>
        if let Some(email_start) = line.find('<') {
            if let Some(email_end) = line.find('>') {
                let email = line[email_start + 1..email_end].trim().to_string();
                emails.push(email);
            }
        }
    }

    Some(emails)
}

/// 生成仓库贡献者分析报告
pub async fn generate_contributors_report(repo_path: &str) -> ContributorsReport {
    info!("正在为仓库 {} 生成贡献者分析报告", repo_path);
    let all_analyses = analyze_repository_contributors(repo_path).await;

    let china_contributors: Vec<&ContributorAnalysis> = all_analyses
        .iter()
        .filter(|analysis| is_likely_from_china(analysis))
        .collect();

    let non_china_contributors: Vec<&ContributorAnalysis> = all_analyses
        .iter()
        .filter(|analysis| !is_likely_from_china(analysis))
        .collect();

    let china_percentage = if !all_analyses.is_empty() {
        china_contributors.len() as f64 / all_analyses.len() as f64 * 100.0
    } else {
        0.0
    };

    // 获取中国贡献者和非中国贡献者的提交总数
    let china_commits: usize = china_contributors.iter().map(|c| c.commits_count).sum();
    let non_china_commits: usize = non_china_contributors.iter().map(|c| c.commits_count).sum();
    let total_commits = china_commits + non_china_commits;

    let china_commits_percentage = if total_commits > 0 {
        china_commits as f64 / total_commits as f64 * 100.0
    } else {
        0.0
    };

    ContributorsReport {
        total_contributors: all_analyses.len(),
        china_contributors_count: china_contributors.len(),
        non_china_contributors_count: non_china_contributors.len(),
        china_percentage,
        total_commits,
        china_commits,
        non_china_commits,
        china_commits_percentage,
        top_china_contributors: china_contributors
            .iter()
            .take(10)
            .map(|&c| c.clone())
            .collect(),
        top_non_china_contributors: non_china_contributors
            .iter()
            .take(10)
            .map(|&c| c.clone())
            .collect(),
    }
}

/// Error type for contributor analysis
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct ContributorsReport {
    pub total_contributors: usize,
    pub china_contributors_count: usize,
    pub non_china_contributors_count: usize,
    pub china_percentage: f64,
    pub total_commits: usize,
    pub china_commits: usize,
    pub non_china_commits: usize,
    pub china_commits_percentage: f64,
    pub top_china_contributors: Vec<ContributorAnalysis>,
    pub top_non_china_contributors: Vec<ContributorAnalysis>,
}

impl ContributorsReport {
    pub fn print_summary(&self) {
        info!("贡献者分析报告摘要:");
        info!("--------------------------------------------------");
        info!("总贡献者: {} 人", self.total_contributors);
        info!(
            "中国贡献者: {} 人 ({:.1}%)",
            self.china_contributors_count, self.china_percentage
        );
        info!(
            "非中国贡献者: {} 人 ({:.1}%)",
            self.non_china_contributors_count,
            100.0 - self.china_percentage
        );
        info!("--------------------------------------------------");
        info!("总提交数: {} 次", self.total_commits);
        info!(
            "中国贡献者提交: {} 次 ({:.1}%)",
            self.china_commits, self.china_commits_percentage
        );
        info!(
            "非中国贡献者提交: {} 次 ({:.1}%)",
            self.non_china_commits,
            100.0 - self.china_commits_percentage
        );
        info!("--------------------------------------------------");

        if !self.top_china_contributors.is_empty() {
            info!("中国TOP贡献者:");
            for (i, contributor) in self.top_china_contributors.iter().enumerate() {
                let name_display = contributor
                    .name
                    .clone()
                    .unwrap_or_else(|| contributor.login.clone());
                info!(
                    "  {}. {} - {} 次提交",
                    i + 1,
                    name_display,
                    contributor.commits_count
                );
            }
        }

        if !self.top_non_china_contributors.is_empty() {
            info!("--------------------------------------------------");
            info!("非中国TOP贡献者:");
            for (i, contributor) in self.top_non_china_contributors.iter().enumerate() {
                let name_display = contributor
                    .name
                    .clone()
                    .unwrap_or_else(|| contributor.login.clone());
                info!(
                    "  {}. {} - {} 次提交",
                    i + 1,
                    name_display,
                    contributor.commits_count
                );
            }
        }
        info!("--------------------------------------------------");
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}
