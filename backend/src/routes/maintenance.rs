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
pub struct DisposalQuery {
    pub assignee: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct DisposalRow {
    pub id: i64,
    pub disposal_no: String,
    pub round_id: i64,
    pub tree_id: i64,
    pub sample_id: Option<i64>,
    pub assignee: String,
    pub method: String,
    pub status: String,
    pub issued_at: String,
    pub completed_at: Option<String>,
    pub tree_no: String,
    pub road: String,
    pub round_no: i64,
}

/// 养护单位/工作人员：处置单列表
pub async fn list_disposals(
    State(db): State<Db>,
    Query(q): Query<DisposalQuery>,
) -> ApiResult<Json<Value>> {
    let mut sql = String::from(
        "SELECT d.*, t.tree_no, t.road, r.round_no
         FROM disposals d
         JOIN trees t ON t.id = d.tree_id
         JOIN rounds r ON r.id = d.round_id WHERE 1=1",
    );
    let mut binds: Vec<String> = Vec::new();
    if let Some(a) = &q.assignee {
        sql.push_str(" AND d.assignee = ?");
        binds.push(a.clone());
    }
    if let Some(s) = &q.status {
        sql.push_str(" AND d.status = ?");
        binds.push(s.clone());
    }
    sql.push_str(" ORDER BY d.issued_at DESC, d.id DESC");
    let mut query = sqlx::query_as::<_, DisposalRow>(&sql);
    for b in binds {
        query = query.bind(b);
    }
    let rows = query.fetch_all(&db).await?;
    Ok(Json(json!({ "disposals": rows })))
}

/// 工作人员：下达处置单（养护单位将收到本地模拟通知）
pub async fn create_disposal(
    State(db): State<Db>,
    Json(body): Json<CreateDisposal>,
) -> ApiResult<Json<Value>> {
    if body.assignee.trim().is_empty() {
        return Err(ApiError::bad_request("养护单位不能为空"));
    }
    let round = sqlx::query_as::<_, Round>("SELECT * FROM rounds WHERE id = ?")
        .bind(body.round_id)
        .fetch_optional(&db)
        .await?
        .ok_or_else(|| ApiError::not_found("轮次不存在"))?;
    if round.status != "OPEN" {
        return Err(ApiError::conflict("轮次已结案，不能下达处置单"));
    }
    if let Some(sid) = body.sample_id {
        let ok = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM samples WHERE id = ? AND round_id = ?",
        )
        .bind(sid)
        .bind(round.id)
        .fetch_one(&db)
        .await?;
        if ok == 0 {
            return Err(ApiError::bad_request("依据样本不属于该轮次"));
        }
    }

    let mut tx = db.begin().await?;
    let disposal_no = db::next_number(&mut tx, "disposal", "DP").await?;
    let res = sqlx::query(
        "INSERT INTO disposals(disposal_no, round_id, tree_id, sample_id, assignee, method)
         VALUES (?,?,?,?,?,?)",
    )
    .bind(&disposal_no)
    .bind(round.id)
    .bind(round.tree_id)
    .bind(body.sample_id)
    .bind(body.assignee.trim())
    .bind(body.method.unwrap_or_default())
    .execute(&mut *tx)
    .await?;
    let disposal_id = res.last_insert_rowid();
    let tree_no = sqlx::query_scalar::<_, String>("SELECT tree_no FROM trees WHERE id = ?")
        .bind(round.tree_id)
        .fetch_one(&mut *tx)
        .await?;
    db::notify(
        &mut tx,
        body.assignee.trim(),
        &format!("新处置单 {disposal_no}"),
        &format!("树木 {tree_no} 第 {} 轮处置任务，请及时处理", round.round_no),
        "disposal",
        disposal_id,
    )
    .await?;
    tx.commit().await?;

    let disposal = sqlx::query_as::<_, Disposal>("SELECT * FROM disposals WHERE id = ?")
        .bind(disposal_id)
        .fetch_one(&db)
        .await?;
    Ok(Json(json!({ "disposal": disposal })))
}

/// 养护单位：回报治理情况（首次回报自动转为处理中）
pub async fn add_report(
    State(db): State<Db>,
    Path(id): Path<i64>,
    Json(body): Json<CreateReport>,
) -> ApiResult<Json<Value>> {
    let disposal = sqlx::query_as::<_, Disposal>("SELECT * FROM disposals WHERE id = ?")
        .bind(id)
        .fetch_optional(&db)
        .await?
        .ok_or_else(|| ApiError::not_found("处置单不存在"))?;
    if disposal.status == "COMPLETED" {
        return Err(ApiError::conflict("处置单已完成，不能再回报"));
    }
    if body.reporter.trim().is_empty() || body.content.trim().is_empty() {
        return Err(ApiError::bad_request("回报人和内容不能为空"));
    }
    let mut tx = db.begin().await?;
    let res = sqlx::query(
        "INSERT INTO treatment_reports(disposal_id, reporter, content) VALUES (?,?,?)",
    )
    .bind(disposal.id)
    .bind(body.reporter.trim())
    .bind(body.content.trim())
    .execute(&mut *tx)
    .await?;
    let report_id = res.last_insert_rowid();
    if disposal.status == "ISSUED" {
        sqlx::query("UPDATE disposals SET status = 'IN_PROGRESS' WHERE id = ?")
            .bind(disposal.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    let report = sqlx::query_as::<_, TreatmentReport>(
        "SELECT * FROM treatment_reports WHERE id = ?",
    )
    .bind(report_id)
    .fetch_one(&db)
    .await?;
    Ok(Json(json!({ "report": report })))
}

/// 养护单位：标记治理完成。
/// 硬性规则：所属轮次必须存在【已核实的确诊结果】，否则不能结案。
pub async fn complete_disposal(State(db): State<Db>, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let disposal = sqlx::query_as::<_, Disposal>("SELECT * FROM disposals WHERE id = ?")
        .bind(id)
        .fetch_optional(&db)
        .await?
        .ok_or_else(|| ApiError::not_found("处置单不存在"))?;
    if disposal.status == "COMPLETED" {
        return Err(ApiError::conflict("处置单已完成"));
    }

    let confirmed = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sample_results r
         JOIN samples s ON s.id = r.sample_id
         WHERE s.round_id = ? AND r.status = 'VERIFIED' AND r.result_type = 'CONFIRMED'",
    )
    .bind(disposal.round_id)
    .fetch_one(&db)
    .await?;
    if confirmed == 0 {
        return Err(ApiError::conflict(
            "该轮次尚无已核实的确诊鉴定结果，不能标记治理完成",
        ));
    }

    let mut tx = db.begin().await?;
    sqlx::query(
        "UPDATE disposals SET status = 'COMPLETED',
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(disposal.id)
    .execute(&mut *tx)
    .await?;
    let tree_no = sqlx::query_scalar::<_, String>("SELECT tree_no FROM trees WHERE id = ?")
        .bind(disposal.tree_id)
        .fetch_one(&mut *tx)
        .await?;
    db::notify(
        &mut tx,
        "staff",
        &format!("处置单 {} 已完成", disposal.disposal_no),
        &format!("树木 {tree_no} 治理完成，请核查结案"),
        "disposal",
        disposal.id,
    )
    .await?;
    tx.commit().await?;

    let disposal = sqlx::query_as::<_, Disposal>("SELECT * FROM disposals WHERE id = ?")
        .bind(id)
        .fetch_one(&db)
        .await?;
    Ok(Json(json!({ "disposal": disposal })))
}
