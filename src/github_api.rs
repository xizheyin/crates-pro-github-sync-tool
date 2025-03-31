use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};
use std::time::Duration;
use std::sync::Arc;
use tokio_postgres::Client as PgClient;
use serde_json;

// GitHub API URL
const GITHUB_API_URL: &str = "https://api.github.com";

// 使用main中定义的函数获取GitHub令牌
use crate::get_github_token;

// GitHub用户信息结构
#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubUser {
    pub id: i64,
    pub login: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub company: Option<String>,
    pub location: Option<String>,
    pub bio: Option<String>,
    pub public_repos: Option<i32>,
    pub followers: Option<i32>,
    pub following: Option<i32>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

// 贡献者信息结构
#[derive(Debug, Serialize, Deserialize)]
pub struct Contributor {
    pub id: i64,
    pub login: String,
    pub avatar_url: String,
    pub contributions: i32,
}

// GitHub API客户端
pub struct GitHubClient {
    client: Client,
    db_client: Arc<PgClient>,
}

impl GitHubClient {
    // 创建新的GitHub API客户端
    pub fn new(db_client: Arc<PgClient>) -> Self {
        // 初始化为不带认证的Client
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("github-handler")
            .build()
            .unwrap_or_else(|_| Client::new());
            
        GitHubClient {
            client,
            db_client,
        }
    }
    
    // 创建带有认证头的请求构建器
    fn authorized_request(&self, url: &str) -> reqwest::RequestBuilder {
        let token = get_github_token();
        let mut builder = self.client.get(url);
        
        if !token.is_empty() {
            builder = builder.header(header::AUTHORIZATION, format!("token {}", token));
        }
        
        builder.header(header::USER_AGENT, "github-handler")
    }
    
    // 检查数据库约束和外键
    pub async fn check_db_constraints(&self) -> Result<(), tokio_postgres::Error> {
        info!("检查数据库约束和外键...");
        
        // 检查外键约束
        let constraint_query = r#"
        SELECT 
            tc.constraint_name, 
            tc.table_name, 
            kcu.column_name, 
            ccu.table_name AS foreign_table_name,
            ccu.column_name AS foreign_column_name 
        FROM 
            information_schema.table_constraints AS tc 
            JOIN information_schema.key_column_usage AS kcu
              ON tc.constraint_name = kcu.constraint_name
              AND tc.table_schema = kcu.table_schema
            JOIN information_schema.constraint_column_usage AS ccu
              ON ccu.constraint_name = tc.constraint_name
              AND ccu.table_schema = tc.table_schema
        WHERE tc.constraint_type = 'FOREIGN KEY' AND tc.table_name = 'repository_contributors';
        "#;
        
        let rows = self.db_client.query(constraint_query, &[]).await?;
        
        if rows.is_empty() {
            warn!("未找到repository_contributors表的外键约束");
            
            // 尝试检查表结构
            let tables_query = "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public';";
            let tables = self.db_client.query(tables_query, &[]).await?;
            
            info!("数据库中的表:");
            for row in &tables {
                let table_name: String = row.get(0);
                info!("表: {}", table_name);
            }
            
            // 检查programs表是否存在
            let program_exists = tables.iter().any(|row| {
                let table_name: String = row.get(0);
                table_name == "programs"
            });
            
            if !program_exists {
                error!("programs表不存在，无法创建外键约束");
            } else {
                // 尝试添加外键约束
                info!("尝试手动添加外键约束...");
                
                // 检查是否有fk_repository_contributors_repository_id约束
                let check_constraint = r#"
                SELECT 1 FROM information_schema.table_constraints
                WHERE constraint_name = 'fk_repository_contributors_repository_id'
                  AND table_name = 'repository_contributors'
                "#;
                
                let has_constraint = !self.db_client.query(check_constraint, &[]).await?.is_empty();
                
                if !has_constraint {
                    let add_fk = r#"
                    ALTER TABLE repository_contributors
                    ADD CONSTRAINT fk_repository_contributors_repository_id
                    FOREIGN KEY (repository_id) REFERENCES programs(id);
                    "#;
                    
                    match self.db_client.execute(add_fk, &[]).await {
                        Ok(_) => info!("成功添加外键约束到programs表"),
                        Err(e) => {
                            error!("添加外键约束失败: {}", e);
                            
                            // 检查programs表的id字段类型
                            let programs_id_type = r#"
                            SELECT column_name, data_type, character_maximum_length
                            FROM information_schema.columns
                            WHERE table_name = 'programs' AND column_name = 'id';
                            "#;
                            
                            match self.db_client.query(programs_id_type, &[]).await {
                                Ok(id_rows) => {
                                    if let Some(row) = id_rows.first() {
                                        let data_type: String = row.get(1);
                                        info!("programs表的id字段类型: {}", data_type);
                                    }
                                },
                                Err(e) => error!("查询programs表id字段类型失败: {}", e)
                            }
                            
                            // 检查repository_contributors表的repository_id字段类型
                            let repo_id_type = r#"
                            SELECT column_name, data_type, character_maximum_length
                            FROM information_schema.columns
                            WHERE table_name = 'repository_contributors' AND column_name = 'repository_id';
                            "#;
                            
                            match self.db_client.query(repo_id_type, &[]).await {
                                Ok(id_rows) => {
                                    if let Some(row) = id_rows.first() {
                                        let data_type: String = row.get(1);
                                        info!("repository_contributors表的repository_id字段类型: {}", data_type);
                                    }
                                },
                                Err(e) => error!("查询repository_id字段类型失败: {}", e)
                            }
                        }
                    }
                }
            }
        } else {
            info!("发现外键约束:");
            for row in rows {
                let constraint_name: String = row.get(0);
                let table_name: String = row.get(1);
                let column_name: String = row.get(2);
                let foreign_table: String = row.get(3);
                let foreign_column: String = row.get(4);
                
                info!("约束: {}, 表: {}, 列: {}, 引用表: {}, 引用列: {}", 
                      constraint_name, table_name, column_name, foreign_table, foreign_column);
            }
        }
        
        Ok(())
    }
    
    // 初始化数据库表
    pub async fn init_database_tables(&self) -> Result<(), tokio_postgres::Error> {
        info!("初始化GitHub用户和贡献者表");
        
        // 创建github_users表
        let create_users_table = r#"
        CREATE TABLE IF NOT EXISTS github_users (
            id SERIAL PRIMARY KEY,
            github_id BIGINT UNIQUE NOT NULL,
            login VARCHAR(255) NOT NULL,
            name VARCHAR(255),
            email VARCHAR(255),
            avatar_url TEXT,
            company VARCHAR(255),
            location VARCHAR(255),
            bio TEXT,
            public_repos INTEGER,
            followers INTEGER,
            following INTEGER,
            created_at TIMESTAMP,
            updated_at TIMESTAMP,
            inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at_local TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )"#;
        
        // 创建repository_contributors表
        let create_contributors_table = r#"
        CREATE TABLE IF NOT EXISTS repository_contributors (
            id SERIAL PRIMARY KEY,
            repository_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            contributions INTEGER NOT NULL DEFAULT 0,
            inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(repository_id, user_id)
        )"#;
        
        // 创建贡献者国别分析表
        let create_contributor_location_table = r#"
        CREATE TABLE IF NOT EXISTS contributor_locations (
            id SERIAL PRIMARY KEY,
            repository_id INTEGER NOT NULL,
            user_id INTEGER NOT NULL,
            is_from_china BOOLEAN NOT NULL,
            china_probability FLOAT NOT NULL,
            common_timezone VARCHAR(50),
            timezone_stats JSONB,
            commit_hours JSONB,
            analyzed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(repository_id, user_id)
        )"#;
        
        // 添加外键约束
        let add_foreign_keys = r#"
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1 
                FROM information_schema.table_constraints 
                WHERE constraint_name = 'fk_repository_contributors_user_id' 
                  AND table_name = 'repository_contributors'
            ) THEN
                ALTER TABLE repository_contributors 
                ADD CONSTRAINT fk_repository_contributors_user_id 
                FOREIGN KEY (user_id) REFERENCES github_users(id);
            END IF;
            
            IF NOT EXISTS (
                SELECT 1 
                FROM information_schema.table_constraints 
                WHERE constraint_name = 'fk_repository_contributors_repository_id' 
                  AND table_name = 'repository_contributors'
            ) THEN
                ALTER TABLE repository_contributors 
                ADD CONSTRAINT fk_repository_contributors_repository_id 
                FOREIGN KEY (repository_id) REFERENCES programs(id);
            END IF;
            
            IF NOT EXISTS (
                SELECT 1 
                FROM information_schema.table_constraints 
                WHERE constraint_name = 'fk_contributor_locations_user_id' 
                  AND table_name = 'contributor_locations'
            ) THEN
                ALTER TABLE contributor_locations 
                ADD CONSTRAINT fk_contributor_locations_user_id 
                FOREIGN KEY (user_id) REFERENCES github_users(id);
            END IF;
            
            IF NOT EXISTS (
                SELECT 1 
                FROM information_schema.table_constraints 
                WHERE constraint_name = 'fk_contributor_locations_repository_id' 
                  AND table_name = 'contributor_locations'
            ) THEN
                ALTER TABLE contributor_locations 
                ADD CONSTRAINT fk_contributor_locations_repository_id 
                FOREIGN KEY (repository_id) REFERENCES programs(id);
            END IF;
        END
        $$;
        "#;
        
        // 添加索引
        let create_indexes = r#"
        CREATE INDEX IF NOT EXISTS idx_github_users_github_id ON github_users(github_id);
        CREATE INDEX IF NOT EXISTS idx_github_users_login ON github_users(login);
        CREATE INDEX IF NOT EXISTS idx_repository_contributors_repo_user ON repository_contributors(repository_id, user_id);
        CREATE INDEX IF NOT EXISTS idx_contributor_locations_repo_id ON contributor_locations(repository_id);
        CREATE INDEX IF NOT EXISTS idx_contributor_locations_is_from_china ON contributor_locations(is_from_china);
        "#;
        
        // 执行SQL语句
        self.db_client.batch_execute(create_users_table).await?;
        self.db_client.batch_execute(create_contributors_table).await?;
        self.db_client.batch_execute(create_contributor_location_table).await?;
        
        // 尝试添加外键约束（可能会失败，如果programs表不存在）
        match self.db_client.batch_execute(add_foreign_keys).await {
            Ok(_) => info!("成功添加外键约束"),
            Err(e) => warn!("添加外键约束失败 (可能是programs表不存在): {}", e)
        }
        
        self.db_client.batch_execute(create_indexes).await?;
        
        info!("数据库表初始化完成");
        Ok(())
    }
    
    // 获取GitHub用户详细信息
    pub async fn get_user_details(&self, username: &str) -> Result<GitHubUser, reqwest::Error> {
        let url = format!("{}/users/{}", GITHUB_API_URL, username);
        debug!("请求用户信息: {}", url);
        
        let response = self.authorized_request(&url)
            .send()
            .await?
            .error_for_status()?;
            
        let user: GitHubUser = response.json().await?;
        
        Ok(user)
    }
    
    // 存储或更新GitHub用户
    pub async fn store_user(&self, user: &GitHubUser) -> Result<i32, tokio_postgres::Error> {
        let query = r#"
        INSERT INTO github_users 
        (github_id, login, name, email, avatar_url, company, location, bio, 
         public_repos, followers, following, created_at, updated_at, updated_at_local)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, CURRENT_TIMESTAMP)
        ON CONFLICT (github_id) 
        DO UPDATE SET 
            login = $2,
            name = $3,
            email = $4,
            avatar_url = $5,
            company = $6,
            location = $7,
            bio = $8,
            public_repos = $9,
            followers = $10,
            following = $11,
            updated_at = $13,
            updated_at_local = CURRENT_TIMESTAMP
        RETURNING id"#;
        
        let row = self.db_client.query_one(
            query,
            &[
                &user.id,
                &user.login,
                &user.name,
                &user.email,
                &user.avatar_url,
                &user.company,
                &user.location,
                &user.bio,
                &user.public_repos,
                &user.followers,
                &user.following,
                &user.created_at,
                &user.updated_at,
            ]
        ).await?;
        
        let user_id: i32 = row.get(0);
        Ok(user_id)
    }
    
    // 获取项目ID
    pub async fn get_repository_id(&self, owner: &str, repo: &str) -> Result<Option<String>, tokio_postgres::Error> {
        info!("尝试获取仓库ID: owner={}, repo={}", owner, repo);
        
        // 首先尝试直接通过仓库名称匹配
        let name_query = "SELECT id FROM programs WHERE name = $1 LIMIT 1";
        let name_rows = self.db_client.query(name_query, &[&repo]).await?;
        
        if let Some(row) = name_rows.first() {
            let repo_id: String = row.get(0);
            info!("通过仓库名称 {} 找到ID: {}", repo, repo_id);
            return Ok(Some(repo_id));
        }
        
        // 如果名称匹配失败，尝试URL模式匹配
        let query = r#"
        SELECT id, github_url FROM programs 
        WHERE github_url LIKE $1 OR github_url LIKE $2 OR github_url LIKE $3
        OR github_url LIKE $4 OR github_url LIKE $5
        LIMIT 1"#;
        
        let patterns = [
            format!("%github.com/{}/{}%", owner, repo),
            format!("%github.com/{}/{}/%", owner, repo),
            format!("%github.com/{}/{}.git%", owner, repo),
            format!("%/{}/{}%", owner, repo),       // 添加更宽松的匹配
            format!("%/{}/{}.git%", owner, repo),   // 添加更宽松的匹配
        ];
        
        info!("尝试URL模式匹配: {:?}", patterns);
        
        let rows = self.db_client.query(
            query,
            &[&patterns[0], &patterns[1], &patterns[2], &patterns[3], &patterns[4]]
        ).await?;
        
        if let Some(row) = rows.first() {
            let repo_id: String = row.get(0);
            let url: String = row.get(1);
            info!("通过URL模式找到ID: {}, URL: {}", repo_id, url);
            return Ok(Some(repo_id));
        }
        
        // 如果所有匹配都失败，尝试查询所有github_url并打印出来进行诊断
        let all_query = "SELECT id, name, github_url FROM programs WHERE github_url IS NOT NULL AND github_url != '' LIMIT 10";
        let all_rows = self.db_client.query(all_query, &[]).await?;
        
        if !all_rows.is_empty() {
            info!("数据库中的仓库URL示例:");
            for row in all_rows {
                let id: String = row.get(0);
                let name: String = row.get(1);
                let url: String = row.get(2);
                info!("ID: {}, 名称: {}, URL: {}", id, name, url);
            }
        } else {
            warn!("数据库中没有找到任何带有github_url的仓库");
        }
        
        warn!("无法找到匹配的仓库ID: owner={}, repo={}", owner, repo);
        Ok(None)
    }
    
    // 存储仓库贡献者关系
    pub async fn store_contributor(&self, repository_id: String, user_id: i32, contributions: i32) -> Result<(), tokio_postgres::Error> {
        let query = r#"
        INSERT INTO repository_contributors (repository_id, user_id, contributions, updated_at)
        VALUES ($1, $2, $3, CURRENT_TIMESTAMP)
        ON CONFLICT (repository_id, user_id)
        DO UPDATE SET 
            contributions = $3,
            updated_at = CURRENT_TIMESTAMP"#;
            
        self.db_client.execute(
            query,
            &[&repository_id, &user_id, &contributions]
        ).await?;
        
        Ok(())
    }
    
    // 获取所有仓库贡献者（仅通过Commits API）
    pub async fn get_all_repository_contributors(&self, owner: &str, repo: &str) -> Result<Vec<Contributor>, Box<dyn std::error::Error + Send + Sync>> {
        info!("通过Commits API获取所有仓库贡献者: {}/{}", owner, repo);
        
        // 使用HashMap统计每个贡献者的提交次数
        let mut contributors_map = std::collections::HashMap::new();
        let mut page = 1;
        let per_page = 100; // GitHub允许的最大值
        
        // 获取最近10,000个提交（100页，每页100个）
        let max_pages = 100;
        
        while page <= max_pages {
            let url = format!(
                "{}/repos/{}/{}/commits?page={}&per_page={}", 
                GITHUB_API_URL, owner, repo, page, per_page
            );
            
            debug!("请求Commits API: {} (第{}页)", url, page);
            
            let response = match self.authorized_request(&url).send().await {
                Ok(resp) => resp,
                Err(e) => {
                    warn!("获取提交页面 {} 失败: {}", page, e);
                    break;
                }
            };
            
            // 检查状态码
            if !response.status().is_success() {
                warn!("获取提交页面 {} 失败: HTTP {}", page, response.status());
                // 如果是速率限制，打印详细信息
                if response.status() == reqwest::StatusCode::FORBIDDEN {
                    if let Some(remain) = response.headers().get("x-ratelimit-remaining") {
                        warn!("GitHub API速率限制剩余: {}", remain.to_str().unwrap_or("未知"));
                    }
                    if let Some(reset) = response.headers().get("x-ratelimit-reset") {
                        let reset_time = match reset.to_str().unwrap_or("0").parse::<i64>() {
                            Ok(t) => t,
                            Err(_) => 0
                        };
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;
                        let wait_time = reset_time - now;
                        warn!("GitHub API速率限制重置时间: {} (还需等待约{}秒)", 
                             reset_time, if wait_time > 0 { wait_time } else { 0 });
                    }
                }
                break;
            }
            
            // 提取分页信息
            let has_next_page = response.headers()
                .get("link")
                .and_then(|h| h.to_str().ok())
                .map(|link| link.contains("rel=\"next\""))
                .unwrap_or(false);
            
            // 解析提交数据
            #[derive(Debug, Deserialize)]
            struct CommitAuthor {
                login: String,
                id: i64,
                avatar_url: String,
            }
            
            #[derive(Debug, Deserialize)]
            struct CommitData {
                author: Option<CommitAuthor>,
            }
            
            let commits: Vec<CommitData> = match response.json().await {
                Ok(c) => c,
                Err(e) => {
                    warn!("解析提交数据失败: {}", e);
                    break;
                }
            };
            
            if commits.is_empty() {
                info!("没有更多提交数据");
                break;
            }
            
            // 统计贡献者信息
            for commit in commits {
                if let Some(author) = commit.author {
                    contributors_map
                        .entry(author.id)
                        .and_modify(|e: &mut (String, String, i32)| e.2 += 1)
                        .or_insert((author.login, author.avatar_url, 1));
                }
            }
            
            info!("已处理 {} 页提交，当前贡献者数量: {}", page, contributors_map.len());
            
            // 如果没有下一页，退出循环
            if !has_next_page {
                break;
            }
            
            // 添加延迟避免触发GitHub API限制
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            page += 1;
        }
        
        info!("通过Commits API找到 {} 名贡献者", contributors_map.len());
        
        // 转换为Contributor结构
        let mut commit_contributors = contributors_map
            .into_iter()
            .map(|(id, (login, avatar_url, contributions))| {
                Contributor {
                    id,
                    login,
                    avatar_url,
                    contributions,
                }
            })
            .collect::<Vec<_>>();
        
        // 按贡献数量排序
        commit_contributors.sort_by(|a, b| b.contributions.cmp(&a.contributions));
        
        Ok(commit_contributors)
    }

    // 处理仓库的所有贡献者
    pub async fn process_all_repository_contributors(&self, owner: &str, repo: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("处理仓库贡献者: {}/{}", owner, repo);
        
        // 获取仓库在数据库中的ID
        let repository_id = match self.get_repository_id(owner, repo).await? {
            Some(id) => id,
            None => {
                warn!("在数据库中未找到仓库: {}/{}", owner, repo);
                return Ok(());
            }
        };
        
        info!("仓库ID: {}", repository_id);
        
        // 检查数据库中是否已有贡献者记录
        let count_query = "SELECT COUNT(*) FROM repository_contributors WHERE repository_id = $1";
        let count_result = match self.db_client.query_one(count_query, &[&repository_id]).await {
            Ok(row) => {
                let count: i64 = row.get(0);
                info!("数据库中已有 {} 名贡献者记录", count);
                count
            },
            Err(e) => {
                warn!("查询贡献者数量失败: {}", e);
                0
            }
        };
        
        // 如果已经有足够的贡献者记录，跳过API获取
        if count_result > 100 {
            info!("数据库中贡献者数量({})已足够，跳过API获取", count_result);
            
            // 直接查询TOP 10贡献者
            self.query_top_contributors(&repository_id).await?;
            
            return Ok(());
        }
        
        // 获取贡献者列表
        let contributors = match self.get_all_repository_contributors(owner, repo).await {
            Ok(c) => c,
            Err(e) => {
                error!("获取贡献者失败: {}", e);
                return Err(e);
            }
        };
        
        info!("获取到 {} 名贡献者", contributors.len());
        
        // 对每个贡献者，获取详细信息并存储
        for contributor in contributors {
            debug!("处理贡献者: {} (贡献: {})", contributor.login, contributor.contributions);
            
            // 获取用户详细信息
            let user_details = match self.get_user_details(&contributor.login).await {
                Ok(u) => u,
                Err(e) => {
                    warn!("获取用户详情失败 {}: {}", contributor.login, e);
                    // 创建一个最小化的用户对象
                    GitHubUser {
                        id: contributor.id,
                        login: contributor.login.clone(),
                        avatar_url: Some(contributor.avatar_url.clone()),
                        name: None,
                        email: None,
                        company: None,
                        location: None,
                        bio: None,
                        public_repos: None,
                        followers: None,
                        following: None,
                        created_at: None,
                        updated_at: None,
                    }
                }
            };
            
            // 存储用户信息
            match self.store_user(&user_details).await {
                Ok(user_id) => {
                    // 存储贡献者关系
                    if let Err(e) = self.store_contributor(repository_id.clone(), user_id, contributor.contributions).await {
                        warn!("存储贡献者关系失败: {}", e);
                    } else {
                        debug!("已存储贡献者: {} -> 仓库ID: {}", user_details.login, repository_id);
                    }
                },
                Err(e) => warn!("存储用户失败 {}: {}", user_details.login, e)
            }
            
            // 添加小延迟避免触发GitHub API限制
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        // 查询该仓库的TOP 10贡献者
        self.query_top_contributors(&repository_id).await?;

        info!("仓库贡献者处理完成: {}/{}", owner, repo);
        Ok(())
    }
    
    // 查询仓库的TOP贡献者
    async fn query_top_contributors(&self, repository_id: &str) -> Result<(), tokio_postgres::Error> {
        let top_contributors_query = r#"
            SELECT gu.id, gu.login, rc.contributions 
            FROM repository_contributors rc
            JOIN github_users gu ON rc.user_id = gu.id
            WHERE rc.repository_id = $1
            ORDER BY rc.contributions DESC
            LIMIT 10
        "#;

        info!("查询仓库 ID {} 的TOP 10贡献者:", repository_id);

        match self.db_client.query(top_contributors_query, &[&repository_id]).await {
            Ok(rows) => {
                if rows.is_empty() {
                    info!("没有找到贡献者记录");
                } else {
                    info!("仓库TOP贡献者列表:");
                    info!("-------------------------------------");
                    info!("排名 | 用户名         | 贡献数");
                    info!("-------------------------------------");
                    
                    for (i, row) in rows.iter().enumerate() {
                        let user_id: i32 = row.get(0);
                        let login: String = row.get(1);
                        let contributions: i32 = row.get(2);
                        info!("{:>4} | {:<15} | {:>5} (ID: {})", 
                              i+1, login, contributions, user_id);
                    }
                    info!("-------------------------------------");
                }
            },
            Err(e) => {
                warn!("查询TOP贡献者失败: {}", e);
            }
        }
        
        Ok(())
    }
    
    // 获取数据库客户端引用
    pub fn get_db_client(&self) -> &Arc<PgClient> {
        &self.db_client
    }
    
    // 存储贡献者国别分析
    pub async fn store_contributor_location(&self, 
                                           repository_id: &str, 
                                           user_id: i32,
                                           analysis: &crate::contributor_analysis::ContributorAnalysis
    ) -> Result<(), tokio_postgres::Error> {
        info!("存储贡献者国别分析: repo_id={}, user_id={}, 用户={}", 
            repository_id, user_id, analysis.login);
        
        // 尝试解析repository_id为整数
        let numeric_id: i32 = match repository_id.parse::<i32>() {
            Ok(id) => id,
            Err(_) => {
                // 先查询programs表，获取仓库的数字ID
                let id_query = "SELECT id FROM programs WHERE id = $1";
                match self.db_client.query_opt(id_query, &[&repository_id]).await? {
                    Some(row) => row.get(0),
                    None => {
                        warn!("未找到ID为{}的仓库，使用ID=0作为回退", repository_id);
                        0 // 使用0作为回退
                    }
                }
            }
        };
        
        let is_from_china = crate::contributor_analysis::is_likely_from_china(analysis);
        
        // 将Map转换为JSON
        let timezone_stats = serde_json::to_value(&analysis.timezone_stats)
            .unwrap_or_else(|_| serde_json::Value::Null);
        let commit_hours = serde_json::to_value(&analysis.commit_hours)
            .unwrap_or_else(|_| serde_json::Value::Null);
            
        // 转换为字符串
        let timezone_stats_str = timezone_stats.to_string();
        let commit_hours_str = commit_hours.to_string();
        
        let query = r#"
        INSERT INTO contributor_locations 
        (repository_id, user_id, is_from_china, china_probability, common_timezone, timezone_stats, commit_hours, analyzed_at)
        VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7::jsonb, CURRENT_TIMESTAMP)
        ON CONFLICT (repository_id, user_id)
        DO UPDATE SET 
            is_from_china = $3,
            china_probability = $4,
            common_timezone = $5,
            timezone_stats = $6::jsonb,
            commit_hours = $7::jsonb,
            analyzed_at = CURRENT_TIMESTAMP"#;
            
        self.db_client.execute(
            query,
            &[
                &numeric_id, 
                &user_id, 
                &is_from_china, 
                &analysis.china_probability, 
                &analysis.common_timezone,
                &timezone_stats_str,
                &commit_hours_str
            ]
        ).await?;
        
        if is_from_china {
            info!("用户 {} 被分析为来自中国 (概率: {:.1}%)", 
                analysis.login, analysis.china_probability * 100.0);
        } else {
            debug!("用户 {} 被分析为非中国用户 (概率: {:.1}%)", 
                analysis.login, analysis.china_probability * 100.0);
        }
        
        Ok(())
    }
    
    // 获取仓库中国贡献者统计
    pub async fn get_repository_china_contributor_stats(&self, repository_id: &str) -> Result<(i64, i64, f64), tokio_postgres::Error> {
        // 尝试解析repository_id为整数
        let numeric_id: i32 = match repository_id.parse::<i32>() {
            Ok(id) => id,
            Err(_) => {
                // 先查询programs表，获取仓库的数字ID
                let id_query = "SELECT id FROM programs WHERE id = $1";
                match self.db_client.query_opt(id_query, &[&repository_id]).await? {
                    Some(row) => row.get(0),
                    None => {
                        warn!("未找到ID为{}的仓库，使用ID=0作为回退", repository_id);
                        0 // 使用0作为回退
                    }
                }
            }
        };
        
        let query = r#"
        SELECT 
            COUNT(*) as total_contributors,
            SUM(CASE WHEN is_from_china THEN 1 ELSE 0 END) as china_contributors,
            CASE 
                WHEN COUNT(*) > 0 THEN (SUM(CASE WHEN is_from_china THEN 1 ELSE 0 END)::float / COUNT(*)) * 100
                ELSE 0
            END as china_percentage
        FROM contributor_locations
        WHERE repository_id = $1"#;
        
        let row = self.db_client.query_one(query, &[&numeric_id]).await?;
        
        let total: i64 = row.get(0);
        let china: i64 = row.get(1);
        let percentage: f64 = row.get(2);
        
        info!("仓库 {} (数字ID: {}) 的中国贡献者统计: {}人中有{}人来自中国 ({:.1}%)", 
            repository_id, numeric_id, total, china, percentage);
            
        Ok((total, china, percentage))
    }
} 