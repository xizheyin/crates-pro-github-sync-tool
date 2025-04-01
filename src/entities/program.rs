use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "programs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub github_url: Option<String>,
    // 添加其他数据库中可能存在的字段
    // 这里只列出了我们实际使用的字段
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
