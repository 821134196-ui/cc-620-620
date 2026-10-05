use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::db::Db;
use crate::error::ApiResult;
use crate::models::*;

/// 本地模拟通知 outbox：所有事件通知落库，供查看（不真正外发）
pub async fn list_notifications(State(db): State<Db>) -> ApiResult<Json<Value>> {
    let rows = sqlx::query_as::<_, Notification>(
        "SELECT * FROM notifications ORDER BY id DESC LIMIT 200",
    )
    .fetch_all(&db)
    .await?;
    Ok(Json(json!({ "notifications": rows })))
}
