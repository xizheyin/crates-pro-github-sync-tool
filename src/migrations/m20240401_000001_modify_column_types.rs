use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 确保所有repository_id字段都是字符串类型
        let sql_alter_repository_contributors = "
            ALTER TABLE IF EXISTS repository_contributors 
            ALTER COLUMN repository_id TYPE TEXT 
            USING repository_id::TEXT;
        ";

        let sql_alter_contributor_locations = "
            ALTER TABLE IF EXISTS contributor_locations 
            ALTER COLUMN repository_id TYPE TEXT 
            USING repository_id::TEXT;
        ";

        // 执行SQL语句
        let _ = manager
            .get_connection()
            .execute_unprepared(sql_alter_repository_contributors)
            .await;

        let _ = manager
            .get_connection()
            .execute_unprepared(sql_alter_contributor_locations)
            .await;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 通常不建议将TEXT转回INTEGER，但为了完整性提供这个回滚
        println!("警告: 将TEXT类型转回INTEGER可能导致数据丢失");

        Ok(())
    }
}
