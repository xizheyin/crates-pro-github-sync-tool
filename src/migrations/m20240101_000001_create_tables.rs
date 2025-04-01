use sea_orm_migration::prelude::*;

use crate::entities;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 创建github_users表
        manager
            .create_table(
                Table::create()
                    .table(GithubUsers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GithubUsers::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(GithubUsers::GithubId)
                            .big_integer()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(GithubUsers::Login).string().not_null())
                    .col(ColumnDef::new(GithubUsers::Name).string())
                    .col(ColumnDef::new(GithubUsers::Email).string())
                    .col(ColumnDef::new(GithubUsers::AvatarUrl).text())
                    .col(ColumnDef::new(GithubUsers::Company).string())
                    .col(ColumnDef::new(GithubUsers::Location).string())
                    .col(ColumnDef::new(GithubUsers::Bio).text())
                    .col(ColumnDef::new(GithubUsers::PublicRepos).integer())
                    .col(ColumnDef::new(GithubUsers::Followers).integer())
                    .col(ColumnDef::new(GithubUsers::Following).integer())
                    .col(ColumnDef::new(GithubUsers::CreatedAt).timestamp())
                    .col(ColumnDef::new(GithubUsers::UpdatedAt).timestamp())
                    .col(
                        ColumnDef::new(GithubUsers::InsertedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(GithubUsers::UpdatedAtLocal)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建repository_contributors表
        manager
            .create_table(
                Table::create()
                    .table(RepositoryContributors::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RepositoryContributors::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RepositoryContributors::RepositoryId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RepositoryContributors::UserId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RepositoryContributors::Contributions)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(RepositoryContributors::InsertedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(RepositoryContributors::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // 创建contributor_locations表
        manager
            .create_table(
                Table::create()
                    .table(ContributorLocations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ContributorLocations::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ContributorLocations::RepositoryId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContributorLocations::UserId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContributorLocations::IsFromChina)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContributorLocations::ChinaProbability)
                            .float()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ContributorLocations::CommonTimezone).string())
                    .col(
                        ColumnDef::new(ContributorLocations::TimezoneStats)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContributorLocations::CommitHours)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContributorLocations::AnalyzedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // 添加唯一约束
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_repository_contributors_unique")
                    .table(RepositoryContributors::Table)
                    .col(RepositoryContributors::RepositoryId)
                    .col(RepositoryContributors::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_contributor_locations_unique")
                    .table(ContributorLocations::Table)
                    .col(ContributorLocations::RepositoryId)
                    .col(ContributorLocations::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // 添加外键约束 - 只有在确定programs表存在时才添加
        if manager.has_table("programs").await? {
            // Foreign key: repository_contributors.repository_id -> programs.id
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_repository_contributors_repository_id")
                        .from(
                            RepositoryContributors::Table,
                            RepositoryContributors::RepositoryId,
                        )
                        .to(entities::program::Entity, entities::program::PrimaryKey::Id)
                        .to_owned(),
                )
                .await?;

            // Foreign key: contributor_locations.repository_id -> programs.id
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_contributor_locations_repository_id")
                        .from(
                            ContributorLocations::Table,
                            ContributorLocations::RepositoryId,
                        )
                        .to(entities::program::Entity, entities::program::PrimaryKey::Id)
                        .to_owned(),
                )
                .await?;
        }

        // Foreign key: repository_contributors.user_id -> github_users.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_repository_contributors_user_id")
                    .from(
                        RepositoryContributors::Table,
                        RepositoryContributors::UserId,
                    )
                    .to(GithubUsers::Table, GithubUsers::Id)
                    .to_owned(),
            )
            .await?;

        // Foreign key: contributor_locations.user_id -> github_users.id
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_contributor_locations_user_id")
                    .from(ContributorLocations::Table, ContributorLocations::UserId)
                    .to(GithubUsers::Table, GithubUsers::Id)
                    .to_owned(),
            )
            .await?;

        // 添加索引
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_github_users_github_id")
                    .table(GithubUsers::Table)
                    .col(GithubUsers::GithubId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_github_users_login")
                    .table(GithubUsers::Table)
                    .col(GithubUsers::Login)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_repository_contributors_repo_user")
                    .table(RepositoryContributors::Table)
                    .col(RepositoryContributors::RepositoryId)
                    .col(RepositoryContributors::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_contributor_locations_repo_id")
                    .table(ContributorLocations::Table)
                    .col(ContributorLocations::RepositoryId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_contributor_locations_is_from_china")
                    .table(ContributorLocations::Table)
                    .col(ContributorLocations::IsFromChina)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ContributorLocations::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(RepositoryContributors::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(GithubUsers::Table).to_owned())
            .await?;

        Ok(())
    }
}

/// Github Users
#[derive(DeriveIden)]
enum GithubUsers {
    Table,
    Id,
    GithubId,
    Login,
    Name,
    Email,
    AvatarUrl,
    Company,
    Location,
    Bio,
    PublicRepos,
    Followers,
    Following,
    CreatedAt,
    UpdatedAt,
    InsertedAt,
    UpdatedAtLocal,
}

/// Repository Contributors
#[derive(DeriveIden)]
enum RepositoryContributors {
    Table,
    Id,
    RepositoryId,
    UserId,
    Contributions,
    InsertedAt,
    UpdatedAt,
}

/// Contributor Locations
#[derive(DeriveIden)]
enum ContributorLocations {
    Table,
    Id,
    RepositoryId,
    UserId,
    IsFromChina,
    ChinaProbability,
    CommonTimezone,
    TimezoneStats,
    CommitHours,
    AnalyzedAt,
}
