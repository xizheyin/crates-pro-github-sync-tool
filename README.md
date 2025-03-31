1. github_users 表
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