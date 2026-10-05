use crate::error::AppResult;
use sqlx::SqlitePool;

/// 本地模拟通知：不接短信网关，只写入 notifications 表，
/// 工作端通知流与居民公开公告板从这里读取。
pub async fn emit(
    pool: &SqlitePool,
    channel: &str,
    target: &str,
    title: &str,
    body: &str,
    related_tree_id: Option<i64>,
    public_visible: bool,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO notifications
            (channel, target, title, body, related_tree_id, public_visible)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(channel)
    .bind(target)
    .bind(title)
    .bind(body)
    .bind(related_tree_id)
    .bind(public_visible as i64)
    .execute(pool)
    .await?;
    Ok(())
}
