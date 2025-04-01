use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 为了确保安全，使用try_execute_unprepared
        let sql = "ALTER TABLE IF EXISTS contributor_locations 
                  ALTER COLUMN repository_id TYPE TEXT 
                  USING repository_id::TEXT;";

        // 执行SQL，如果列类型已经是TEXT或表不存在，会简单地返回
        match manager.get_connection().execute_unprepared(sql).await {
            Ok(_) => {
                println!("成功修改contributor_locations表的repository_id列类型为TEXT");
                Ok(())
            }
            Err(e) => {
                if e.to_string().contains("does not exist")
                    || e.to_string().contains("already exists")
                {
                    println!("警告: {}", e);
                    Ok(()) // 忽略"表不存在"或"列已经是TEXT类型"错误
                } else {
                    Err(e) // 返回其他类型的错误
                }
            }
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 通常不应该尝试将TEXT转回INTEGER，可能会有数据丢失
        // 但如果需要回滚，可以尝试将能转换为数字的TEXT值转回INTEGER
        let sql = "ALTER TABLE IF EXISTS contributor_locations 
                  ALTER COLUMN repository_id TYPE INTEGER 
                  USING CASE WHEN repository_id ~ E'^\\d+$' THEN repository_id::INTEGER ELSE NULL END;";

        match manager.get_connection().execute_unprepared(sql).await {
            Ok(_) => {
                println!("成功回滚contributor_locations表的repository_id列类型为INTEGER");
                Ok(())
            }
            Err(e) => {
                if e.to_string().contains("does not exist") {
                    println!("警告: {}", e);
                    Ok(()) // 忽略"表不存在"错误
                } else {
                    Err(e) // 返回其他类型的错误
                }
            }
        }
    }
}
