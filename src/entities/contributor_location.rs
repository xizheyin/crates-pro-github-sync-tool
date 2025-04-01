use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue::NotSet;
use sea_orm::Set;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "contributor_locations")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub repository_id: String,
    pub user_id: i32,
    pub is_from_china: bool,
    pub common_timezone: Option<String>,
    pub analyzed_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::program::Entity",
        from = "Column::RepositoryId",
        to = "super::program::Column::Id"
    )]
    Program,
    #[sea_orm(
        belongs_to = "super::github_user::Entity",
        from = "Column::UserId",
        to = "super::github_user::Column::Id"
    )]
    GithubUser,
}

impl Related<super::program::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Program.def()
    }
}

impl Related<super::github_user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GithubUser.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// 转换函数，将分析结果转换为数据库模型
impl From<(&str, i32, &crate::contributor_analysis::ContributorAnalysis)> for ActiveModel {
    fn from(
        (repo_id, user_id, analysis): (
            &str,
            i32,
            &crate::contributor_analysis::ContributorAnalysis,
        ),
    ) -> Self {
        let now = chrono::Utc::now().naive_utc();

        Self {
            id: NotSet,
            repository_id: Set(repo_id.to_string()),
            user_id: Set(user_id),
            is_from_china: Set(analysis.from_china),
            common_timezone: Set(Some(analysis.common_timezone.clone())),
            analyzed_at: Set(now),
        }
    }
}
