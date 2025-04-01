pub mod m20240101_000001_create_tables;
pub mod m20240401_000001_modify_column_types;
pub mod m20240512_000001_fix_repository_id_type;

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DbConn;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_create_tables::Migration),
            Box::new(m20240401_000001_modify_column_types::Migration),
            Box::new(m20240512_000001_fix_repository_id_type::Migration),
        ]
    }
}

pub async fn setup_database(db: &DbConn) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("正在设置数据库表结构...");
    Migrator::up(db, None).await?;
    tracing::info!("数据库表设置完成");
    Ok(())
}
