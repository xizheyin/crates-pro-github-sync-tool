# GitHub仓库贡献者分析工具

这是一个用Rust编写的命令行工具，用于分析GitHub仓库的贡献者信息，包括他们的基本信息、地理位置分布和贡献统计。该工具还支持将数据存储到PostgreSQL数据库中进行持久化和高级查询。

## 功能特点

- 根据GitHub仓库URL注册并分析仓库
- 获取仓库的所有贡献者信息
- 分析贡献者的地理位置（特别是识别来自中国的贡献者）
- 生成贡献者统计报告
- 查询仓库的贡献者统计数据
- 支持GitHub API令牌轮换，避免触发API限制
- 将所有数据存储到PostgreSQL数据库

## 安装

### 前提条件

- Rust编译环境（rustc, cargo）
- PostgreSQL数据库
- Git

### 安装步骤

1. 克隆仓库：
   ```bash
   git clone <仓库URL>
   cd github-handler
   ```

2. 编译项目：
   ```bash
   cargo build --release
   ```

3. 创建配置文件：
   ```bash
   cp config.sample.json config.json
   ```

4. 编辑配置文件，添加GitHub令牌和数据库连接信息。

## 配置

可以通过以下两种方式进行配置：

### 1. 配置文件（推荐）

创建一个`config.json`文件，格式如下：

```json
{
  "github": {
    "tokens": [
      "YOUR_GITHUB_TOKEN_1",
      "YOUR_GITHUB_TOKEN_2",
      "YOUR_GITHUB_TOKEN_3"
    ]
  },
  "database": {
    "url": "postgresql://username:password@localhost:5432/dbname"
  }
}
```

### 2. 环境变量

也可以使用环境变量进行配置：

- `GITHUB_TOKEN`: 单个GitHub令牌
- `GITHUB_TOKEN_1`, `GITHUB_TOKEN_2`, ... : 多个GitHub令牌（用于轮换）
- `DATABASE_URL`: PostgreSQL数据库连接URL
- `CONFIG_PATH`: 可选，指定配置文件的路径

## 使用方法

### 注册仓库

```bash
cargo run -- register --url https://github.com/owner/repo
```

或者使用编译后的二进制文件：

```bash
./github-handler register --url https://github.com/owner/repo
```

### 分析仓库贡献者

```bash
cargo run -- analyze owner repo
```

### 查询仓库贡献者统计

```bash
cargo run -- query owner repo
```

### 生成贡献者地理位置分析报告

```bash
cargo run -- --analyze-contributors /path/to/local/repo/clone
```

## 数据库架构

该工具使用PostgreSQL数据库存储以下信息：

- GitHub用户信息
- 仓库信息
- 贡献者与仓库的关系
- 贡献者地理位置信息

数据库模式会在首次运行时自动创建。

## 开发说明

该项目使用以下主要依赖：

- `tokio`: 异步运行时
- `reqwest`: HTTP客户端，用于GitHub API调用
- `sea-orm`: 数据库ORM
- `serde`: 序列化/反序列化
- `clap`: 命令行参数解析
- `tracing`: 日志记录

### 项目结构

- `src/main.rs`: 程序入口点和CLI接口
- `src/config.rs`: 配置管理
- `src/contributor_analysis.rs`: 贡献者分析逻辑
- `src/services/`: 服务层实现
  - `github_api.rs`: GitHub API客户端
  - `database.rs`: 数据库操作
- `src/entities/`: 数据库实体定义
- `src/migrations/`: 数据库迁移脚本

## 许可证

[项目许可证]
