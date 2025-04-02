use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DbConn;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![]
    }
}

pub async fn setup_database(db: &DbConn) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("正在设置数据库表结构...");
    Migrator::up(db, None).await?;
    tracing::info!("数据库表设置完成");
    Ok(())
}
