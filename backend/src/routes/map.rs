use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::db::Db;
use crate::error::ApiResult;
use crate::models::*;

/// 本地模拟地图接口：返回道路线与树木标注点（不依赖外部地图服务）
pub async fn map_data(State(db): State<Db>) -> ApiResult<Json<Value>> {
    let roads = sqlx::query_as::<_, MapRoad>("SELECT * FROM map_roads ORDER BY name")
        .fetch_all(&db)
        .await?;
    let trees = sqlx::query_as::<_, TreeMarker>(
        "SELECT t.id, t.tree_no, t.species, t.road, t.lat, t.lng,
                EXISTS(SELECT 1 FROM rounds r WHERE r.tree_id = t.id AND r.status = 'OPEN') AS has_open_round
         FROM trees t ORDER BY t.tree_no",
    )
    .fetch_all(&db)
    .await?;
    let roads_json: Vec<Value> = roads
        .iter()
        .map(|r| {
            json!({
                "name": r.name,
                "path": serde_json::from_str::<Value>(&r.path).unwrap_or(Value::Null),
            })
        })
        .collect();
    Ok(Json(json!({ "roads": roads_json, "trees": trees })))
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct TreeMarker {
    pub id: i64,
    pub tree_no: String,
    pub species: String,
    pub road: String,
    pub lat: f64,
    pub lng: f64,
    pub has_open_round: bool,
}
