# GitHub Handler

这是一个用于处理GitHub和Gitee仓库的Rust应用程序。它可以克隆仓库、获取仓库的贡献者信息，并将这些信息存储在PostgreSQL数据库中。

## 功能

- 从PostgreSQL数据库中读取仓库信息
- 克隆或更新本地仓库
- 获取GitHub仓库的贡献者信息
- 将贡献者信息存储在数据库中
- 支持GitHub和Gitee平台
- 支持多个GitHub令牌自动轮换，避免API速率限制
- 支持批量处理多个仓库或处理单个指定仓库

## 环境要求

- Rust 1.52.0 或更高版本
- PostgreSQL 12.0 或更高版本
- Git

## 安装

1. 克隆此代码库
2. 使用Cargo构建项目:

```bash
cargo build --release
```

## 配置

此应用程序支持两种配置方式：配置文件和环境变量。

### 配置文件

可以创建一个`config.json`文件来配置GitHub令牌和数据库连接信息。要生成样例配置文件，请运行:

```bash
cargo run -- --sample-config
# 或者指定路径
cargo run -- --sample-config my-config.json
```

配置文件结构如下:

```json
{
  "github": {
    "tokens": [
      "your_github_token_1",
      "your_github_token_2",
      "your_github_token_3"
    ]
  },
  "database": {
    "url": "postgresql://user:password@localhost:5432/dbname"
  }
}
```

您可以通过设置`CONFIG_PATH`环境变量来指定配置文件路径:

```bash
export CONFIG_PATH="/path/to/your/config.json"
```

### 多令牌轮换

当您配置多个GitHub令牌时，程序会在每次API调用时自动轮换使用不同的令牌。这有效地解决了GitHub API的速率限制问题。

每个令牌默认有5,000次/小时的请求限制，如果配置了3个令牌，您实际上可以每小时执行15,000次请求。

### 环境变量

除了配置文件外，该应用程序还支持通过环境变量进行配置:

- `GITHUB_TOKEN`: 主GitHub个人访问令牌
- `GITHUB_TOKEN_1`, `GITHUB_TOKEN_2`, `GITHUB_TOKEN_3`, ...：额外的GitHub令牌，用于轮换
- `DATABASE_URL`: PostgreSQL数据库连接字符串（默认: `postgresql://mega:mega@localhost:30432/cratespro`）
- `CONFIG_PATH`: 配置文件路径（默认: `config.json`）

### 获取GitHub令牌

1. 登录您的GitHub账户
2. 访问 Settings -> Developer settings -> Personal access tokens -> Generate new token
3. 为令牌选择适当的权限（至少需要`repo`权限）
4. 生成令牌并复制它
5. 添加到配置文件或设置为环境变量:

```bash
export GITHUB_TOKEN="your_token_here"
# 添加多个令牌
export GITHUB_TOKEN_1="your_token_1_here"
export GITHUB_TOKEN_2="your_token_2_here"
```

## 使用方法

### 处理单个仓库

要处理单个仓库，请提供所有者和仓库名称作为命令行参数:

```bash
cargo run -- owner repo_name
```

例如，处理Tokio仓库:

```bash
cargo run -- tokio-rs tokio
```

### 批量处理

不带参数运行程序将处理数据库中`programs`表里所有有效的仓库:

```bash
cargo run
```

## 数据库结构

该应用使用以下主要表:

- `programs`: 存储仓库的基本信息
- `github_users`: 存储GitHub用户信息
- `repository_contributors`: 存储仓库的贡献者关系

## 安全注意事项

- **不要**在代码中硬编码GitHub令牌
- 始终使用环境变量或配置文件来存储敏感信息
- 定期轮换您的GitHub令牌以增强安全性
- 确保`config.json`文件的访问权限有适当限制

## 故障排除

### API限制

GitHub API有速率限制。未经身份验证的请求限制为每小时60次，而经过身份验证的请求限制为每小时5,000次。如果您仍然收到API限制错误:

1. 通过配置文件或环境变量添加更多的GitHub令牌
2. 减少并发任务数量（在代码中修改`MAX_CONCURRENT_TASKS`常量）
3. 实现更长的延迟时间，避免短时间内发送太多请求

我们建议配置至少3-5个不同账号的令牌，可以有效解决速率限制问题。

### 数据库连接问题

如果遇到数据库连接问题:

1. 确保PostgreSQL服务正在运行
2. 验证数据库连接字符串是否正确
3. 检查数据库用户是否有适当的权限

## 贡献

欢迎提交问题报告和拉取请求！

### github_users 表
```
CREATE TABLE IF NOT EXISTS github_users (
    id SERIAL PRIMARY KEY,              -- 自增主键ID
    github_id BIGINT UNIQUE NOT NULL,   -- GitHub用户ID，唯一非空
    login VARCHAR(255) NOT NULL,        -- GitHub用户名
    name VARCHAR(255),                  -- 用户真实姓名
    email VARCHAR(255),                 -- 用户邮箱
    avatar_url TEXT,                    -- 头像URL
    company VARCHAR(255),               -- 公司
    location VARCHAR(255),              -- 位置
    bio TEXT,                           -- 简介
    public_repos INTEGER,               -- 公开仓库数量
    followers INTEGER,                  -- 粉丝数量
    following INTEGER,                  -- 关注数量
    created_at TIMESTAMP,               -- GitHub账号创建时间
    updated_at TIMESTAMP,               -- GitHub账号更新时间
    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,  -- 记录插入时间
    updated_at_local TIMESTAMP DEFAULT CURRENT_TIMESTAMP -- 记录本地更新时间
)
```

### repository_contributors 表

```
CREATE TABLE IF NOT EXISTS repository_contributors (
    id SERIAL PRIMARY KEY,              -- 自增主键ID
    repository_id INTEGER NOT NULL,     -- 仓库ID
    user_id INTEGER NOT NULL,           -- 用户ID
    contributions INTEGER NOT NULL DEFAULT 0, -- 贡献数量
    inserted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP, -- 记录插入时间
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,  -- 记录更新时间
    UNIQUE(repository_id, user_id)      -- 唯一约束：一个仓库的一个用户只能有一条记录
)
```