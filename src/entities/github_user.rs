use sea_orm::{entity::prelude::*, ActiveValue::NotSet, Set};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "github_users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub github_id: i64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub company: Option<String>,
    pub location: Option<String>,
    pub bio: Option<String>,
    pub public_repos: Option<i32>,
    pub followers: Option<i32>,
    pub following: Option<i32>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub inserted_at: DateTime,
    pub updated_at_local: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::repository_contributor::Entity")]
    RepositoryContributor,
    #[sea_orm(has_many = "super::contributor_location::Entity")]
    ContributorLocation,
}

impl Related<super::repository_contributor::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RepositoryContributor.def()
    }
}

impl Related<super::contributor_location::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContributorLocation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// 转换函数，用于将GitHub API返回的用户转换为数据库模型
impl From<crate::services::github_api::GitHubUser> for ActiveModel {
    fn from(user: crate::services::github_api::GitHubUser) -> Self {
        let now = chrono::Utc::now().naive_utc();

        Self {
            id: NotSet,
            github_id: Set(user.id),
            login: Set(user.login),
            name: Set(user.name),
            email: Set(user.email),
            avatar_url: Set(user.avatar_url),
            company: Set(user.company),
            location: Set(user.location),
            bio: Set(user.bio),
            public_repos: Set(user.public_repos),
            followers: Set(user.followers),
            following: Set(user.following),
            created_at: Set(user.created_at),
            updated_at: Set(user.updated_at),
            inserted_at: Set(now),
            updated_at_local: Set(now),
        }
    }
}
