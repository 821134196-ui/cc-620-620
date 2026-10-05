use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{self, Db};
use crate::error::{ApiError, ApiResult};
use crate::models::*;

#[derive(Debug, Deserialize)]
pub struct LabQuery {
    pub status: Option<String>, // SUBMITTED / RESULT_DRAFT / RESULT_VERIFIED
}

/// 实验室：样本工作台（含树号，便于定位）
pub async fn list_samples(State(db): State<Db>, Query(q): Query<LabQuery>) -> ApiResult<Json<Value>> {
    let mut sql = String::from(
        "SELECT s.*, t.tree_no AS tree_no, t.road AS road, t.species AS species,
                r.result_type AS result_type, r.status AS result_status
         FROM samples s
         JOIN trees t ON t.id = s.tree_id
         LEFT JOIN sample_results r ON r.sample_id = s.id",
    );
    let mut binds: Vec<String> = Vec::new();
    if let Some(st) = &q.status {
        sql.push_str(" WHERE s.status = ?");
        binds.push(st.clone());
    }
    sql.push_str(" ORDER BY s.submitted_at DESC, s.id DESC");
    let mut query = sqlx::query_as::<_, SampleRow>(&sql);
    for b in binds {
        query = query.bind(b);
    }
    let rows = query.fetch_all(&db).await?;
    Ok(Json(json!({ "samples": rows })))
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct SampleRow {
    pub id: i64,
    pub sample_no: String,
    pub tree_id: i64,
    pub round_id: i64,
    pub parent_sample_id: Option<i64>,
    pub inspector: String,
    pub symptom: String,
    pub note: String,
    pub status: String,
    pub submitted_at: String,
    pub tree_no: String,
    pub road: String,
    pub species: String,
    pub result_type: Option<String>,
    pub result_status: Option<String>,
}

/// 实验室：录入/修改鉴定结果（草稿）。已核实的样本禁止再改，保证结论不被覆盖。
pub async fn record_result(
    State(db): State<Db>,
    Path(id): Path<i64>,
    Json(body): Json<RecordResult>,
) -> ApiResult<Json<Value>> {
    let sample = sqlx::query_as::<_, Sample>("SELECT * FROM samples WHERE id = ?")
        .bind(id)
        .fetch_optional(&db)
        .await?
        .ok_or_else(|| ApiError::not_found("样本不存在"))?;
    if sample.status == "RESULT_VERIFIED" {
        return Err(ApiError::conflict("该样本结果已核实归档，不能修改；如有新发现请重新送样"));
    }
    let rt = body.result_type.as_str();
    if !matches!(rt, "CONFIRMED" | "NEGATIVE" | "INCONCLUSIVE") {
        return Err(ApiError::bad_request(
            "result_type 须为 CONFIRMED / NEGATIVE / INCONCLUSIVE（退样请用退样接口）",
        ));
    }
    if rt == "CONFIRMED" && body.pest_name.as_deref().unwrap_or("").trim().is_empty() {
        return Err(ApiError::bad_request("确诊结果必须填写病虫害名称"));
    }
    if body.created_by.trim().is_empty() {
        return Err(ApiError::bad_request("记录人不能为空"));
    }

    let mut tx = db.begin().await?;
    // 每份样本一条结果记录：草稿阶段可覆盖修改，核实后锁定
    sqlx::query(
        "INSERT INTO sample_results(sample_id, result_type, pest_name, confidence, notes, status, created_by)
         VALUES (?,?,?,?,?,'DRAFT',?)
         ON CONFLICT(sample_id) DO UPDATE SET
            result_type = excluded.result_type,
            pest_name   = excluded.pest_name,
            confidence  = excluded.confidence,
            notes       = excluded.notes,
            created_by  = excluded.created_by",
    )
    .bind(sample.id)
    .bind(rt)
    .bind(body.pest_name.unwrap_or_default())
    .bind(body.confidence.unwrap_or_default())
    .bind(body.notes.unwrap_or_default())
    .bind(body.created_by.trim())
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE samples SET status = 'RESULT_DRAFT' WHERE id = ?")
        .bind(sample.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    let result = sqlx::query_as::<_, SampleResult>("SELECT * FROM sample_results WHERE sample_id = ?")
        .bind(sample.id)
        .fetch_one(&db)
        .await?;
    Ok(Json(json!({ "result": result })))
}

/// 实验室：核实草稿结果 —— 核实后对公众可见，且不可再改
pub async fn verify_result(State(db): State<Db>, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let sample = sqlx::query_as::<_, Sample>("SELECT * FROM samples WHERE id = ?")
        .bind(id)
        .fetch_optional(&db)
        .await?
        .ok_or_else(|| ApiError::not_found("样本不存在"))?;
    let result = sqlx::query_as::<_, SampleResult>("SELECT * FROM sample_results WHERE sample_id = ?")
        .bind(sample.id)
        .fetch_optional(&db)
        .await?
        .ok_or_else(|| ApiError::conflict("该样本还没有鉴定草稿，无法核实"))?;
    if result.status == "VERIFIED" {
        return Err(ApiError::conflict("该样本结果已核实"));
    }

    let mut tx = db.begin().await?;
    sqlx::query(
        "UPDATE sample_results SET status = 'VERIFIED',
            verified_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(result.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE samples SET status = 'RESULT_VERIFIED' WHERE id = ?")
        .bind(sample.id)
        .execute(&mut *tx)
        .await?;
    let tree_no = sqlx::query_scalar::<_, String>("SELECT tree_no FROM trees WHERE id = ?")
        .bind(sample.tree_id)
        .fetch_one(&mut *tx)
        .await?;
    db::notify(
        &mut tx,
        "public",
        &format!("样本 {} 鉴定结果已核实", sample.sample_no),
        &format!("树木 {tree_no} 鉴定结论：{}", describe_result(&result)),
        "sample",
        sample.id,
    )
    .await?;
    tx.commit().await?;

    let result = sqlx::query_as::<_, SampleResult>("SELECT * FROM sample_results WHERE id = ?")
        .bind(result.id)
        .fetch_one(&db)
        .await?;
    Ok(Json(json!({ "result": result })))
}

/// 实验室：退样 —— 记录为该次样本的最终结论（VERIFIED/REJECTED），保留经过
pub async fn reject_sample(
    State(db): State<Db>,
    Path(id): Path<i64>,
    Json(body): Json<RejectSample>,
) -> ApiResult<Json<Value>> {
    let sample = sqlx::query_as::<_, Sample>("SELECT * FROM samples WHERE id = ?")
        .bind(id)
        .fetch_optional(&db)
        .await?
        .ok_or_else(|| ApiError::not_found("样本不存在"))?;
    if sample.status == "RESULT_VERIFIED" {
        return Err(ApiError::conflict("该样本已有核实结论，不能退样"));
    }
    if body.reason.trim().is_empty() {
        return Err(ApiError::bad_request("退样必须填写原因"));
    }

    let mut tx = db.begin().await?;
    sqlx::query(
        "INSERT INTO sample_results(sample_id, result_type, pest_name, confidence, notes, status, created_by, verified_at)
         VALUES (?, 'REJECTED', '', '', ?, 'VERIFIED', ?, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
         ON CONFLICT(sample_id) DO UPDATE SET
            result_type = 'REJECTED', notes = excluded.notes,
            status = 'VERIFIED', created_by = excluded.created_by,
            verified_at = excluded.verified_at",
    )
    .bind(sample.id)
    .bind(format!("退样原因：{}", body.reason.trim()))
    .bind(body.created_by.trim())
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE samples SET status = 'RESULT_VERIFIED' WHERE id = ?")
        .bind(sample.id)
        .execute(&mut *tx)
        .await?;
    db::notify(
        &mut tx,
        "staff",
        &format!("样本 {} 被退样", sample.sample_no),
        &format!("退样原因：{}", body.reason.trim()),
        "sample",
        sample.id,
    )
    .await?;
    tx.commit().await?;

    let result = sqlx::query_as::<_, SampleResult>("SELECT * FROM sample_results WHERE sample_id = ?")
        .bind(sample.id)
        .fetch_one(&db)
        .await?;
    Ok(Json(json!({ "result": result })))
}

fn describe_result(r: &SampleResult) -> String {
    match r.result_type.as_str() {
        "CONFIRMED" => format!("确诊 {}", r.pest_name),
        "NEGATIVE" => "未检出病虫害".to_string(),
        "INCONCLUSIVE" => "样本无法判定".to_string(),
        "REJECTED" => "样本被退回".to_string(),
        _ => r.result_type.clone(),
    }
}
