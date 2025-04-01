use clap::{Parser, Subcommand};
use dotenv::dotenv;
use sea_orm::Database;
use std::process::Command;
use std::time::Duration;
use tracing::{error, info, warn};
use url::Url;

// 导入模块
mod config;
mod contributor_analysis;
mod entities;
mod migrations;
mod services;

use crate::config::{get_database_url, save_sample_config};
use crate::contributor_analysis::generate_contributors_report;
use crate::migrations::setup_database;
use crate::services::database::DbService;
use crate::services::github_api::GitHubApiClient;

// CLI 参数结构
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// 仓库所有者（可选）
    owner: Option<String>,

    /// 仓库名称（可选）
    repo: Option<String>,

    /// 生成示例配置文件
    #[arg(long)]
    sample_config: Option<String>,

    /// 分析贡献者地理位置
    #[arg(long)]
    analyze_contributors: Option<String>,

    /// 子命令
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 注册新的GitHub仓库
    Register {
        /// 仓库URL
        url: String,

        /// 仓库名称（可选）
        #[arg(short, long)]
        name: Option<String>,
    },

    /// 分析仓库贡献者
    Analyze {
        /// 仓库所有者
        owner: String,

        /// 仓库名称
        repo: String,
    },

    /// 查询仓库贡献者统计
    Query {
        /// 仓库所有者
        owner: String,

        /// 仓库名称
        repo: String,
    },
}

// 定义错误类型
type BoxError = Box<dyn std::error::Error + Send + Sync>;

// 初始化日志
fn init_logger() {
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::CLOSE)
        .init();
}

// 从URL中解析仓库信息
fn parse_github_repo_url(repo_url: &str) -> Option<(String, String)> {
    if let Ok(url) = Url::parse(repo_url) {
        let path_segments: Vec<&str> = url
            .path_segments()
            .map(|segments| segments.collect())
            .unwrap_or_default();

        if path_segments.len() >= 2 {
            let owner = path_segments[0].to_string();
            // 移除.git后缀如果存在
            let repo = path_segments[1].trim_end_matches(".git").to_string();
            return Some((owner, repo));
        }
    } else {
        // 尝试匹配格式: owner/repo 或 owner/repo.git
        let parts: Vec<&str> = repo_url.split('/').collect();
        if parts.len() >= 2 {
            let owner = parts[parts.len() - 2].to_string();
            let repo = parts[parts.len() - 1].trim_end_matches(".git").to_string();
            return Some((owner, repo));
        }
    }

    None
}

// 分析Git贡献者
async fn analyze_git_contributors(
    db_service: &DbService,
    owner: &str,
    repo: &str,
) -> Result<(), BoxError> {
    info!("分析仓库贡献者: {}/{}", owner, repo);

    // 获取仓库ID
    let repository_id = match db_service.get_repository_id(owner, repo).await? {
        Some(id) => id,
        None => {
            warn!("仓库 {}/{} 未在数据库中注册", owner, repo);
            return Ok(());
        }
    };

    // 创建GitHub API客户端
    let github_client = GitHubApiClient::new();

    // 获取仓库贡献者
    let contributors = github_client
        .get_all_repository_contributors(owner, repo)
        .await?;

    info!("获取到 {} 个贡献者，开始存储到数据库", contributors.len());

    // 存储贡献者信息
    for contributor in contributors {
        // 获取并存储用户详细信息
        let user = match github_client.get_user_details(&contributor.login).await {
            Ok(user) => user,
            Err(e) => {
                warn!("获取用户 {} 详情失败: {}", contributor.login, e);
                continue;
            }
        };

        // 存储用户到数据库
        let user_id = match db_service.store_user(&user).await {
            Ok(id) => id,
            Err(e) => {
                error!("存储用户 {} 失败: {}", user.login, e);
                continue;
            }
        };

        // 存储贡献者关系
        if let Err(e) = db_service
            .store_contributor(repository_id, user_id, contributor.contributions)
            .await
        {
            error!(
                "存储贡献者关系失败: {}/{} -> {}: {}",
                owner, repo, user.login, e
            );
        }

        // 等待一小段时间，避免触发GitHub API限制
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // 查询并显示贡献者统计
    match db_service.query_top_contributors(repository_id).await {
        Ok(top_contributors) => {
            info!("仓库 {}/{} 的贡献者统计:", owner, repo);
            for (i, contributor) in top_contributors.iter().enumerate().take(10) {
                info!(
                    "  {}. {} - {} 次提交",
                    i + 1,
                    contributor.login,
                    contributor.contributions
                );
            }
        }
        Err(e) => {
            error!("查询贡献者统计失败: {}", e);
        }
    }

    // 分析贡献者国别
    analyze_contributor_locations(db_service, owner, repo, repository_id).await?;

    Ok(())
}

// 分析贡献者国别位置
async fn analyze_contributor_locations(
    db_service: &DbService,
    owner: &str,
    repo: &str,
    repository_id: i32,
) -> Result<(), BoxError> {
    info!("分析仓库 {}/{} 的贡献者地理位置", owner, repo);

    // 尝试克隆仓库到临时目录
    let temp_dir = std::env::temp_dir().join(format!("{}-{}", owner, repo));
    let temp_path = temp_dir.to_string_lossy();

    // 检查目录是否已存在
    if !temp_dir.exists() {
        info!("克隆仓库到临时目录: {}", temp_path);
        let status = Command::new("git")
            .args(&[
                "clone",
                &format!("https://github.com/{}/{}.git", owner, repo),
                &temp_path,
            ])
            .status();

        match status {
            Ok(status) if !status.success() => {
                warn!("克隆仓库失败: {}", status);
                return Ok(());
            }
            Err(e) => {
                warn!("执行git命令失败: {}", e);
                return Ok(());
            }
            _ => {}
        }
    } else {
        info!("更新已存在的仓库: {}", temp_path);
        let status = Command::new("git")
            .current_dir(&temp_dir)
            .args(&["pull"])
            .status();

        if let Err(e) = status {
            warn!("更新仓库失败: {}", e);
        }
    }

    // 生成贡献者报告
    let report = generate_contributors_report(&temp_path).await;
    report.print_summary();

    // 在数据库中存储分析结果
    for analysis in &report.top_china_contributors {
        let is_from_china = true;

        // 查找用户ID
        let user_id = match db_service.get_user_id_by_name(&analysis.login).await {
            Ok(Some(id)) => id,
            _ => {
                warn!("未找到用户 {} 的ID", analysis.login);
                continue;
            }
        };

        // 将时区统计和提交时间统计转换为JSON字符串
        let timezone_stats =
            serde_json::to_string(&analysis.timezone_stats).unwrap_or_else(|_| "{}".to_string());

        let commit_hours: Vec<i32> = analysis
            .commit_hours
            .iter()
            .map(|(_, &count)| count as i32)
            .collect();

        // 存储贡献者位置分析
        if let Err(e) = db_service
            .store_contributor_location(
                repository_id,
                user_id,
                is_from_china,
                analysis.china_probability,
                &analysis.common_timezone,
                &timezone_stats,
                &commit_hours,
            )
            .await
        {
            error!("存储贡献者位置分析失败: {}", e);
        }
    }

    for analysis in &report.top_non_china_contributors {
        let is_from_china = false;

        // 查找用户ID
        let user_id = match db_service.get_user_id_by_name(&analysis.login).await {
            Ok(Some(id)) => id,
            _ => {
                warn!("未找到用户 {} 的ID", analysis.login);
                continue;
            }
        };

        // 将时区统计和提交时间统计转换为JSON字符串
        let timezone_stats =
            serde_json::to_string(&analysis.timezone_stats).unwrap_or_else(|_| "{}".to_string());

        let commit_hours: Vec<i32> = analysis
            .commit_hours
            .iter()
            .map(|(_, &count)| count as i32)
            .collect();

        // 存储贡献者位置分析
        if let Err(e) = db_service
            .store_contributor_location(
                repository_id,
                user_id,
                is_from_china,
                analysis.china_probability,
                &analysis.common_timezone,
                &timezone_stats,
                &commit_hours,
            )
            .await
        {
            error!("存储贡献者位置分析失败: {}", e);
        }
    }

    // 查询中国贡献者统计
    match db_service
        .get_repository_china_contributor_stats(repository_id)
        .await
    {
        Ok(stats) => {
            info!(
                "仓库 {}/{} 的中国贡献者统计: {}人中有{}人来自中国 ({:.1}%)",
                owner,
                repo,
                stats.total_contributors,
                stats.china_contributors,
                stats.china_percentage
            );

            if !stats.china_contributors_details.is_empty() {
                info!("中国贡献者TOP列表:");
                for (i, contributor) in stats.china_contributors_details.iter().enumerate().take(5)
                {
                    let name_display = contributor
                        .name
                        .clone()
                        .unwrap_or_else(|| contributor.login.clone());
                    info!(
                        "  {}. {} - {} 次提交",
                        i + 1,
                        name_display,
                        contributor.contributions
                    );
                }
            }
        }
        Err(e) => {
            error!("获取中国贡献者统计失败: {}", e);
        }
    }

    Ok(())
}

// 查询仓库的顶级贡献者
async fn query_top_contributors(
    db_service: &DbService,
    owner: &str,
    repo: &str,
) -> Result<(), BoxError> {
    info!("查询仓库 {}/{} 的顶级贡献者", owner, repo);

    // 获取仓库ID
    let repository_id = match db_service.get_repository_id(owner, repo).await? {
        Some(id) => id,
        None => {
            warn!("仓库 {}/{} 未在数据库中注册", owner, repo);
            return Ok(());
        }
    };

    // 查询贡献者统计
    match db_service.query_top_contributors(repository_id).await {
        Ok(top_contributors) => {
            info!("仓库 {}/{} 的贡献者统计:", owner, repo);
            for (i, contributor) in top_contributors.iter().enumerate().take(10) {
                let location_str = contributor
                    .location
                    .as_ref()
                    .map(|loc| format!(" ({})", loc))
                    .unwrap_or_default();

                let name_display = contributor.name.as_ref().unwrap_or(&contributor.login);

                info!(
                    "  {}. {}{} - {} 次提交",
                    i + 1,
                    name_display,
                    location_str,
                    contributor.contributions
                );
            }
        }
        Err(e) => {
            error!("查询贡献者统计失败: {}", e);
        }
    }

    // 查询中国贡献者统计
    match db_service
        .get_repository_china_contributor_stats(repository_id)
        .await
    {
        Ok(stats) => {
            info!(
                "仓库 {}/{} 的中国贡献者统计: {}人中有{}人来自中国 ({:.1}%)",
                owner,
                repo,
                stats.total_contributors,
                stats.china_contributors,
                stats.china_percentage
            );
        }
        Err(e) => {
            error!("获取中国贡献者统计失败: {}", e);
        }
    }

    Ok(())
}

// 存储分析结果
async fn store_analysis_results(
    db_service: &DbService,
    owner: &str,
    repo: &str,
    analysis_results: &[(String, bool, f64)],
) -> Result<(), BoxError> {
    // 获取仓库ID
    let repository_id = match db_service.get_repository_id(owner, repo).await? {
        Some(id) => id,
        None => {
            warn!("仓库 {}/{} 未在数据库中注册", owner, repo);
            return Ok(());
        }
    };

    for (login, is_from_china, probability) in analysis_results {
        // 查找用户ID
        let user_id = match db_service.get_user_id_by_name(login).await {
            Ok(Some(id)) => id,
            _ => {
                warn!("未找到用户 {} 的ID", login);
                continue;
            }
        };

        // 存储贡献者位置分析（简化版）
        if let Err(e) = db_service
            .store_contributor_location(
                repository_id,
                user_id,
                *is_from_china,
                *probability,
                if *is_from_china { "+0800" } else { "Unknown" },
                "{}",
                &[],
            )
            .await
        {
            error!("存储贡献者位置分析失败: {}", e);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    // 加载.env文件
    dotenv().ok();

    // 初始化日志
    init_logger();

    // 解析命令行参数
    let cli = Cli::parse();

    // 处理生成示例配置文件请求
    if let Some(path) = cli.sample_config {
        let config_path = if path.is_empty() {
            "config.json".to_string()
        } else {
            path
        };
        save_sample_config(&config_path).unwrap();
        return Ok(());
    }

    // 处理贡献者分析请求
    if let Some(repo_path) = cli.analyze_contributors {
        let report = generate_contributors_report(&repo_path).await;
        report.print_summary();

        // 如果提供了第二个位置参数，将结果保存为JSON
        if let Some(output_path) = cli.repo {
            let json = report.to_json()?;
            std::fs::write(&output_path, json)?;
            info!("分析结果已保存到: {}", output_path);
        }

        return Ok(());
    }

    // 连接数据库
    info!("连接数据库...");
    let db_url = get_database_url();
    let conn = Database::connect(&db_url).await?;

    // 设置数据库表结构
    setup_database(&conn).await.unwrap();

    // 创建数据库服务
    let db_service = DbService::new(conn);

    // 处理子命令
    match cli.command {
        Some(Commands::Register { url, name }) => {
            if let Some((owner, repo)) = parse_github_repo_url(&url) {
                info!("注册仓库: {}/{}", owner, repo);
                // 这里需要实现仓库注册逻辑
                // ...
            } else {
                error!("无效的仓库URL: {}", url);
            }
        }

        Some(Commands::Analyze { owner, repo }) => {
            analyze_git_contributors(&db_service, &owner, &repo).await?;
        }

        Some(Commands::Query { owner, repo }) => {
            query_top_contributors(&db_service, &owner, &repo).await?;
        }

        None => {
            // 如果没有提供子命令，但提供了owner和repo参数
            if let (Some(owner), Some(repo)) = (cli.owner, cli.repo) {
                analyze_git_contributors(&db_service, &owner, &repo).await?;
            } else {
                // 没有足够的参数，显示帮助信息
                println!("请提供仓库所有者和名称，或使用子命令。运行 --help 获取更多信息。");
            }
        }
    }

    Ok(())
}
