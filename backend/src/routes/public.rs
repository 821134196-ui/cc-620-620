use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::Db;
use crate::error::{ApiError, ApiResult};
use crate::models::*;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

/// 公众：按树号/道路/树种搜索树木
pub async fn search_trees(State(db): State<Db>, Query(q): Query<SearchQuery>) -> ApiResult<Json<Value>> {
    let kw = q.q.unwrap_or_default();
    let like = format!("%{}%", kw.trim());
    let trees = sqlx::query_as::<_, Tree>(
        "SELECT * FROM trees WHERE tree_no LIKE ? OR road LIKE ? OR species LIKE ? OR address LIKE ?
         ORDER BY tree_no LIMIT 50",
    )
    .bind(&like)
    .bind(&like)
    .bind(&like)
    .bind(&like)
    .fetch_all(&db)
    .await?;
    Ok(Json(json!({ "trees": trees })))
}

/// 公众：树木已核实的进展时间线。
/// 只输出已核实(VERIFIED)的鉴定结果；实验室草稿一律不返回。
pub async fn public_tree(State(db): State<Db>, Path(tree_no): Path<String>) -> ApiResult<Json<Value>> {
    let tree = sqlx::query_as::<_, Tree>("SELECT * FROM trees WHERE tree_no = ?")
        .bind(&tree_no)
        .fetch_optional(&db)
        .await?
        .ok_or_else(|| ApiError::not_found("树木不存在"))?;

    let rounds = sqlx::query_as::<_, Round>(
        "SELECT * FROM rounds WHERE tree_id = ? ORDER BY round_no",
    )
    .bind(tree.id)
    .fetch_all(&db)
    .await?;

    let mut timeline = Vec::new();
    for r in &rounds {
        let samples = sqlx::query_as::<_, Sample>(
            "SELECT * FROM samples WHERE round_id = ? ORDER BY submitted_at, id",
        )
        .bind(r.id)
        .fetch_all(&db)
        .await?;
        let mut sample_json = Vec::new();
        for s in &samples {
            // 只取已核实结果；草稿对公众不可见
            let verified = sqlx::query_as::<_, SampleResult>(
                "SELECT * FROM sample_results WHERE sample_id = ? AND status = 'VERIFIED'",
            )
            .bind(s.id)
            .fetch_optional(&db)
            .await?;
            sample_json.push(json!({
                "sample_no": s.sample_no,
                "submitted_at": s.submitted_at,
                "symptom": s.symptom,
                "is_resubmission": s.parent_sample_id.is_some(),
                "verified_result": verified,   // null 表示鉴定中（草稿不公开）
            }));
        }
        let disposals = sqlx::query_as::<_, Disposal>(
            "SELECT * FROM disposals WHERE round_id = ? ORDER BY issued_at, id",
        )
        .bind(r.id)
        .fetch_all(&db)
        .await?;
        let mut disposal_json = Vec::new();
        for d in &disposals {
            let reports = sqlx::query_as::<_, TreatmentReport>(
                "SELECT * FROM treatment_reports WHERE disposal_id = ? ORDER BY reported_at, id",
            )
            .bind(d.id)
            .fetch_all(&db)
            .await?;
            disposal_json.push(json!({
                "disposal_no": d.disposal_no,
                "assignee": d.assignee,
                "method": d.method,
                "status": d.status,
                "issued_at": d.issued_at,
                "completed_at": d.completed_at,
                "reports": reports,
            }));
        }
        timeline.push(json!({
            "round_no": r.round_no,
            "status": r.status,
            "reason": r.reason,
            "opened_at": r.opened_at,
            "closed_at": r.closed_at,
            "samples": sample_json,
            "disposals": disposal_json,
        }));
    }

    Ok(Json(json!({
        "tree": {
            "tree_no": tree.tree_no,
            "species": tree.species,
            "road": tree.road,
            "address": tree.address,
            "lat": tree.lat,
            "lng": tree.lng,
        },
        "rounds": timeline,
    })))
}
