use tokio_postgres::{NoTls, Error as PgError};
use std::process::Command;
use std::fs;
use std::path::Path;
use url::Url;
use std::sync::Arc;
use tracing::{info, warn, error, debug, instrument, Level};
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use std::time::Duration;
use std::env;
use std::fs::File;
use std::io::Write;
use chrono::Local;
use tokio::time::sleep;
use once_cell::sync::Lazy;

// 导入配置模块
mod config;
mod github_api;
mod contributor_analysis;

use github_api::GitHubClient;
use contributor_analysis::{generate_contributors_report, ContributorsReport};
use config::{load_config, get_next_github_token, get_database_url, save_sample_config};

// 并发处理的最大数量
const MAX_CONCURRENT_TASKS: usize = 1;  // 减少并发数，避免GitHub限制

// 从配置或环境变量获取GitHub令牌，支持令牌轮换
pub fn get_github_token() -> String {
    get_next_github_token()
}


#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 设置 tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("github_handler=info".parse().unwrap()))
        .with_target(false)
        .init();

    info!("启动GitHub和Gitee仓库处理程序");
    
    // 加载配置
    load_config();
    
    // 解析命令行参数
    let args: Vec<String> = std::env::args().collect();
    
    // 处理特殊命令
    if args.len() >= 2 {
        match args[1].as_str() {
            // 生成样例配置
            "--sample-config" => {
                let path = if args.len() >= 3 { &args[2] } else { "config.sample.json" };
                
                match save_sample_config(path) {
                    Ok(_) => {
                        info!("已生成样例配置文件: {}", path);
                        return Ok(());
                    },
                    Err(e) => {
                        error!("生成样例配置失败: {}", e);
                        return Ok(());
                    }
                }
            },
            // 分析仓库贡献者
            "--analyze-contributors" => {
                if args.len() >= 3 {
                    let repo_path = &args[2];
                    info!("开始分析仓库贡献者: {}", repo_path);
                    
                    let report = generate_contributors_report(repo_path).await;
                    report.print_summary();
                    
                    // 保存报告到JSON文件
                    if args.len() >= 4 {
                        let output_file = &args[3];
                        match report.to_json() {
                            Ok(json) => {
                                if let Err(e) = fs::write(output_file, json) {
                                    error!("写入报告文件失败: {}", e);
                                } else {
                                    info!("贡献者分析报告已保存到: {}", output_file);
                                }
                            },
                            Err(e) => error!("序列化报告失败: {}", e)
                        }
                    }
                    
                    return Ok(());
                } else {
                    error!("缺少仓库路径参数");
                    print_help();
                    return Ok(());
                }
            },
            "--help" | "-h" => {
                print_help();
                return Ok(());
            },
            _ => {}  // 继续处理其他命令
        }
    }
    
    // 连接到PostgreSQL数据库
    let connection_string = get_database_url();
    
    info!("正在连接到PostgreSQL数据库...");
    let (client, connection) = match tokio_postgres::connect(&connection_string, NoTls).await {
        Ok(res) => res,
        Err(e) => {
            error!("无法连接到数据库: {}", e);
            return Ok(());
        }
    };
    
    // 在后台运行连接
    let client = Arc::new(client);
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            error!("数据库连接错误: {}", e);
        }
    });
    
    info!("数据库连接成功");
    
    // 初始化GitHub客户端
    let github_client = Arc::new(GitHubClient::new(client.clone()));
    
    // 初始化数据库表
    if let Err(e) = github_client.init_database_tables().await {
        error!("初始化数据库表失败: {}", e);
        return Ok(());
    }
    
    // 检查数据库约束
    if let Err(e) = github_client.check_db_constraints().await {
        error!("检查数据库约束失败: {}", e);
        return Ok(());
    }
    
    // 处理单个仓库贡献者
    if args.len() >= 3 && args[1] != "--sample-config" && args[1] != "--analyze-contributors" {
        let owner = &args[1];
        let repo = &args[2];
        
        info!("处理指定的单个仓库: {}/{}", owner, repo);
        
        match github_client.process_all_repository_contributors(owner, repo).await {
            Ok(_) => info!("成功处理仓库贡献者!"),
            Err(e) => error!("处理仓库贡献者失败: {}", e)
        }
        
        return Ok(());
    }
    
    // 查询programs表的数据
    info!("查询 'programs' 表中的数据");
    
    // 首先检查programs表是否存在
    let table_exists = client.query(
        "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema = 'public' AND table_name = 'programs')",
        &[],
    ).await?;
    
    let exists: bool = table_exists[0].get(0);
    
    if exists {
        // 1. 统计表中总条目数
        let count_query = "SELECT COUNT(*) FROM programs";
        let count_result = client.query(count_query, &[]).await?;
        let total_count: i64 = count_result[0].get(0);
        info!("'programs' 表中共有 {} 条记录", total_count);
        
        // 2. 查询github_url非空的条目
        let github_query = "SELECT * FROM programs WHERE github_url IS NOT NULL AND github_url != ''";
        let github_rows = client.query(github_query, &[]).await?;
        
        info!("共有 {} 条记录的 github_url 非空", github_rows.len());
        
        // 获取列索引，找到github_url的位置
        let columns_query = 
            "SELECT column_name FROM information_schema.columns 
             WHERE table_schema = 'public' AND table_name = 'programs'";
        let columns = client.query(columns_query, &[]).await?;
        
        // 查找github_url的列索引
        let mut github_url_index = 0;
        let mut name_index = 0;
        
        for (i, col) in columns.iter().enumerate() {
            let column_name: &str = col.get(0);
            if column_name == "github_url" {
                github_url_index = i;
            }
            if column_name == "name" {
                name_index = i;
            }
        }
        
        // 为每个有github_url的条目准备仓库处理任务
        info!("准备处理Git仓库 (并发数量: {})", MAX_CONCURRENT_TASKS);
        
        // 创建任务集合
        let tasks = github_rows.iter().enumerate().filter_map(|(row_index, row)| {
            // 明确指定从数据库中获取的类型
            let git_url: String = match row.try_get::<_, String>(github_url_index) {
                Ok(url) => url,
                Err(_) => return None, // 跳过无法解析的URL
            };
            
            let program_name: String = match row.try_get::<_, String>(name_index) {
                Ok(name) => name,
                Err(_) => "unknown".to_string(),
            };
            
            Some((row_index, program_name, git_url))
        })
        .collect::<Vec<_>>();
        
        // 使用有界信号量控制并发数量
        let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_TASKS));
        let mut join_handles = Vec::new();
        
        for (row_index, program_name, git_url) in tasks {
            // 获取信号量许可
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            
            // 为每个任务传递GitHub客户端的引用
            let github_client = github_client.clone();
            
            // 创建一个新的异步任务
            let handle = tokio::spawn(async move {
                let result = process_repository(row_index, &program_name, &git_url, github_client).await;
                // 许可在这里被释放
                drop(permit);
                result
            });
            
            join_handles.push(handle);
        }
        
        // 等待所有任务完成
        for handle in join_handles {
            let _ = handle.await;
        }
        
        info!("所有Git仓库处理完成");
    } else {
        warn!("'programs' 表不存在于数据库中！");
    }
    
    Ok(())
}

/// 打印帮助信息
fn print_help() {
    println!("GitHub Handler 用法:");
    println!("  cargo run -- [参数]");
    println!("参数:");
    println!("  --sample-config [路径]            生成样例配置文件");
    println!("  --analyze-contributors <仓库路径> [输出文件]  分析仓库贡献者国别");
    println!("  <owner> <repo>                    处理指定的GitHub仓库");
    println!("  (无参数)                           处理数据库中所有仓库");
}

/// 处理单个仓库的克隆或更新
#[instrument(skip_all, fields(program = %program_name, url = %git_url))]
async fn process_repository(row_index: usize, program_name: &str, git_url: &str, github_client: Arc<GitHubClient>) {
    info!("#{}: 开始处理仓库", row_index + 1);
    
    // 解析URL获取平台、owner和repo名称
    match parse_git_url(git_url) {
        Some((platform, owner, repo)) => {
            info!("解析结果: 平台={}, 所有者={}, 仓库={}", platform, owner, repo);
            
            // 创建目标目录 - 使用/mnt/crates/source作为基础目录
            let target_dir = format!("/mnt/crates/source/{}/{}", owner, repo);
            debug!("目标目录: {}", target_dir);
            
            // 检查目录是否已存在
            let repo_exists = Path::new(&target_dir).exists() && 
                           Path::new(&format!("{}/.git", target_dir)).exists();
            
            if repo_exists {
                info!("仓库已存在，执行git pull更新");
                update_repository(&target_dir).await;
            } else {
                info!("仓库不存在，执行git clone");
                clone_repository(&platform, &owner, &repo, &target_dir, git_url).await;
            }
            
            // 如果是GitHub仓库，处理贡献者
            if platform == "github" {
                info!("处理GitHub仓库贡献者");
                
                // 获取仓库ID
                let repository_id = match github_client.get_repository_id(&owner, &repo).await {
                    Ok(Some(id)) => id,
                    Ok(None) => {
                        warn!("未能找到仓库ID: {}/{}", owner, repo);
                        return;
                    },
                    Err(e) => {
                        error!("获取仓库ID时出错: {}", e);
                        return;
                    }
                };
                
                // 处理GitHub贡献者
                match github_client.process_all_repository_contributors(&owner, &repo).await {
                    Ok(_) => {
                        info!("成功处理仓库贡献者");
                        
                        // 分析贡献者地理位置
                        if Path::new(&target_dir).exists() {
                            match generate_contributors_report(&target_dir).await {
                                report => {
                                    report.print_summary();
                                    
                                    // 保存分析结果到数据库
                                    store_contributor_analysis(&github_client, &repository_id, &report, &target_dir).await;
                                    
                                    // 获取并显示中国贡献者统计
                                    if let Ok((total, china, percentage)) = github_client.get_repository_china_contributor_stats(&repository_id).await {
                                        info!("数据库中的中国贡献者统计: 共{}人，其中{}人来自中国 ({:.1}%)", 
                                              total, china, percentage);
                                    }
                                }
                            }
                        }
                    },
                    Err(e) => warn!("处理仓库贡献者失败: {}", e),
                }
                
                // 添加延迟避免GitHub API限制
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        },
        None => {
            warn!("无法解析Git URL");
        }
    }
}

/// 更新已存在的仓库
async fn update_repository(target_dir: &str) {
    // 异步执行git pull命令
    let pull_output = tokio::process::Command::new("git")
        .current_dir(target_dir)
        .args(&["pull"])
        // 完全禁用Git凭证请求
        .env("GIT_ASKPASS", "echo")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .output()
        .await;
        
    match pull_output {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("Already up to date") {
                    info!("仓库已是最新");
                } else {
                    info!("仓库更新成功: {}", stdout.trim());
                }
            } else {
                warn!("仓库更新失败: {}", String::from_utf8_lossy(&output.stderr));
            }
        },
        Err(e) => {
            error!("执行git pull命令失败: {}", e);
        }
    }
}

/// 克隆新仓库
async fn clone_repository(platform: &str, owner: &str, repo: &str, target_dir: &str, git_url: &str) {
    // 创建父目录结构(如果不存在)
    let parent_dir = format!("/mnt/crates/source/{}", owner);
    match fs::create_dir_all(&parent_dir) {
        Ok(_) => {
            debug!("创建父目录成功");
            
            // 构建克隆URL - 对GitHub使用令牌
            let github_token = get_github_token();
            let clone_url = if platform == "github" && !github_token.is_empty() {
                // 对于GitHub仓库，使用令牌
                format!("https://{}@github.com/{}/{}.git", github_token, owner, repo)
            } else if platform == "github" {
                // 没有令牌的情况下使用公共URL
                format!("https://github.com/{}/{}.git", owner, repo)
            } else if platform == "gitee" {
                // 对于Gitee仓库
                format!("https://gitee.com/{}/{}.git", owner, repo)
            } else {
                // 其他平台
                git_url.to_string()
            };
            
            info!("开始克隆仓库 ({})", platform);
            
            // 创建临时脚本来执行克隆，防止泄露令牌
            let script_path = format!("{}/clone_script.sh", parent_dir);
            let script_content = format!(
                "#!/bin/bash\n\
                git clone '{}' '{}' 2>&1", 
                clone_url, target_dir
            );
            
            if let Err(e) = fs::write(&script_path, script_content) {
                error!("无法创建克隆脚本: {}", e);
                return;
            }
            
            // 设置脚本权限
            let _ = Command::new("chmod")
                .args(&["+x", &script_path])
                .output();
            
            // 执行克隆脚本
            let output = tokio::process::Command::new(&script_path)
                // 完全禁用Git凭证请求
                .env("GIT_ASKPASS", "echo")
                .env("GIT_TERMINAL_PROMPT", "0")
                .env("GCM_INTERACTIVE", "never")
                .output()
                .await;
            
            // 删除临时脚本
            let _ = fs::remove_file(&script_path);
                
            match output {
                Ok(output) => {
                    if output.status.success() {
                        info!("仓库克隆成功");
                    } else {
                        let err_msg = String::from_utf8_lossy(&output.stdout);
                        warn!("克隆失败: {}", err_msg);
                        
                        if err_msg.contains("Authentication failed") {
                            warn!("认证失败 - GitHub令牌可能无效或未设置");
                            
                            // 对于GitHub, 尝试无令牌克隆公共仓库
                            if platform == "github" && !github_token.is_empty() {
                                info!("尝试以公共仓库方式克隆 (无令牌)...");
                                let public_url = format!("https://github.com/{}/{}.git", owner, repo);
                                
                                let public_output = tokio::process::Command::new("git")
                                    .args(&["clone", &public_url, target_dir])
                                    .env("GIT_ASKPASS", "echo")
                                    .env("GIT_TERMINAL_PROMPT", "0") 
                                    .env("GCM_INTERACTIVE", "never")
                                    .output()
                                    .await;
                                
                                match public_output {
                                    Ok(output) => {
                                        if output.status.success() {
                                            info!("无令牌克隆成功 (公共仓库)");
                                        } else {
                                            error!("无令牌克隆也失败: {}", String::from_utf8_lossy(&output.stderr));
                                        }
                                    },
                                    Err(e) => {
                                        error!("执行无令牌克隆命令失败: {}", e);
                                    }
                                }
                            }
                        }
                    }
                },
                Err(e) => {
                    error!("执行克隆脚本失败: {}", e);
                }
            }
        },
        Err(e) => {
            error!("创建父目录失败: {}", e);
        }
    }
}

// 从Git URL中解析出平台、owner和repo名称
fn parse_git_url(git_url: &str) -> Option<(String, String, String)> {
    if let Ok(url) = Url::parse(git_url) {
        let host = url.host_str()?;
        let path = url.path().trim_start_matches('/');
        let parts: Vec<&str> = path.split('/').collect();
        
        if parts.len() >= 2 {
            let platform = if host.contains("github.com") {
                "github"
            } else if host.contains("gitee.com") {
                "gitee"
            } else {
                "other"
            };
            
            return Some((platform.to_string(), parts[0].to_string(), parts[1].to_string()));
        }
    } else {
        // 尝试匹配格式 "github.com/owner/repo" 或 "gitee.com/owner/repo"
        if git_url.contains("github.com/") {
            let parts: Vec<&str> = git_url.split("github.com/").collect();
            if parts.len() == 2 {
                let repo_parts: Vec<&str> = parts[1].split('/').collect();
                if repo_parts.len() >= 2 {
                    return Some(("github".to_string(), repo_parts[0].to_string(), repo_parts[1].to_string()));
                }
            }
        } else if git_url.contains("gitee.com/") {
            let parts: Vec<&str> = git_url.split("gitee.com/").collect();
            if parts.len() == 2 {
                let repo_parts: Vec<&str> = parts[1].split('/').collect();
                if repo_parts.len() >= 2 {
                    return Some(("gitee".to_string(), repo_parts[0].to_string(), repo_parts[1].to_string()));
                }
            }
        }
    }
    
    None
}

/// 将贡献者国别分析结果存储到数据库
async fn store_contributor_analysis(
    github_client: &GitHubClient, 
    repository_id: &str, 
    report: &ContributorsReport,
    repo_path: &str
) {
    info!("正在将贡献者国别分析结果存入数据库...");
    
    // 尝试解析repository_id为整数
    let numeric_id: i32 = match repository_id.parse::<i32>() {
        Ok(id) => id,
        Err(_) => {
            match github_client.get_db_client().query_opt(
                "SELECT id FROM programs WHERE id = $1",
                &[&repository_id]
            ).await {
                Ok(Some(row)) => row.get(0),
                _ => {
                    warn!("未找到ID为{}的仓库，无法存储贡献者分析", repository_id);
                    return;
                }
            }
        }
    };
    
    // 查询所有贡献者
    let query = format!(
        "SELECT rc.user_id, gu.login FROM repository_contributors rc 
         JOIN github_users gu ON rc.user_id = gu.id 
         WHERE rc.repository_id = {}", 
        numeric_id
    );
    
    match github_client.get_db_client().query(&query, &[]).await {
        Ok(rows) => {
            if rows.is_empty() {
                warn!("仓库 {} 在数据库中没有贡献者记录", repository_id);
                return;
            }
            
            info!("找到 {} 个数据库贡献者记录", rows.len());
            
            let mut china_contributors = 0;
            let mut processed = 0;
            
            // 处理每个贡献者
            for row in &rows {
                let user_id: i32 = row.get(0);
                let login: String = row.get(1);
                
                // 在分析结果中查找对应的贡献者
                let analysis = report.top_china_contributors.iter()
                    .find(|c| c.login == login)
                    .or_else(|| report.top_non_china_contributors.iter().find(|c| c.login == login));
                
                if let Some(analysis) = analysis {
                    // 存储分析结果
                    if let Err(e) = github_client.store_contributor_location(repository_id, user_id, analysis).await {
                        warn!("存储贡献者{}的国别分析失败: {}", login, e);
                    } else {
                        processed += 1;
                        if crate::contributor_analysis::is_likely_from_china(analysis) {
                            china_contributors += 1;
                        }
                    }
                } else {
                    // 未找到对应分析结果，尝试单独分析
                    debug!("在报告中未找到贡献者 {} 的分析，尝试单独分析", login);
                    
                    // 使用git log查找贡献者的email
                    let email_output = tokio::process::Command::new("git")
                        .current_dir(repo_path)
                        .args(&["log", "--format=%ae", "--author", &login, "-n", "1"])
                        .output()
                        .await;
                    
                    if let Ok(output) = email_output {
                        if output.status.success() {
                            let email = String::from_utf8_lossy(&output.stdout).trim().to_string();
                            
                            if !email.is_empty() {
                                if let Some(analysis) = crate::contributor_analysis::analyze_contributor_timezone(repo_path, &email).await {
                                    if let Err(e) = github_client.store_contributor_location(repository_id, user_id, &analysis).await {
                                        warn!("存储单独分析的贡献者{}的国别分析失败: {}", login, e);
                                    } else {
                                        processed += 1;
                                        if crate::contributor_analysis::is_likely_from_china(&analysis) {
                                            china_contributors += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            info!("成功存储 {}/{} 位贡献者的国别分析，其中 {} 位来自中国", 
                processed, rows.len(), china_contributors);
        },
        Err(e) => error!("查询仓库贡献者失败: {}", e)
    }
}
