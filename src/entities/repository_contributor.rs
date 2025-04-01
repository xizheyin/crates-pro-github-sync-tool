use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "repository_contributors")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub repository_id: i32,
    pub user_id: i32,
    pub contributions: i32,
    pub inserted_at: DateTime,
    pub updated_at: DateTime,
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
