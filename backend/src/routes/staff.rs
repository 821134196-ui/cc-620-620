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
pub struct TreeQuery {
    pub q: Option<String>,
    pub road: Option<String>,
}

/// 工作人员：树木列表（含未核实的内部状态概览）
pub async fn list_trees(State(db): State<Db>, Query(q): Query<TreeQuery>) -> ApiResult<Json<Value>> {
    let mut sql = String::from("SELECT * FROM trees WHERE 1=1");
    let mut binds: Vec<String> = Vec::new();
    if let Some(s) = &q.q {
        sql.push_str(" AND (tree_no LIKE ? OR road LIKE ? OR species LIKE ? OR address LIKE ?)");
        let like = format!("%{s}%");
        binds.extend([like.clone(), like.clone(), like.clone(), like]);
    }
    if let Some(r) = &q.road {
        sql.push_str(" AND road = ?");
        binds.push(r.clone());
    }
    sql.push_str(" ORDER BY tree_no");
    let mut query = sqlx::query_as::<_, Tree>(&sql);
    for b in binds {
        query = query.bind(b);
    }
    let trees = query.fetch_all(&db).await?;
    Ok(Json(json!({ "trees": trees })))
}

pub async fn create_tree(State(db): State<Db>, Json(body): Json<CreateTree>) -> ApiResult<Json<Value>> {
    if body.species.trim().is_empty() || body.road.trim().is_empty() {
        return Err(ApiError::bad_request("树种和道路不能为空"));
    }
    let mut tx = db.begin().await?;
    let tree_no = match body.tree_no.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(no) => no.to_string(),
        None => db::next_tree_no(&mut tx).await?,
    };
    let res = sqlx::query(
        "INSERT INTO trees(tree_no, species, road, address, lat, lng) VALUES (?,?,?,?,?,?)",
    )
    .bind(&tree_no)
    .bind(body.species.trim())
    .bind(body.road.trim())
    .bind(body.address.unwrap_or_default())
    .bind(body.lat)
    .bind(body.lng)
    .execute(&mut *tx)
    .await;
    match res {
        Ok(r) => {
            let id = r.last_insert_rowid();
            tx.commit().await?;
            let tree = sqlx::query_as::<_, Tree>("SELECT * FROM trees WHERE id = ?")
                .bind(id)
                .fetch_one(&db)
                .await?;
            Ok(Json(json!({ "tree": tree })))
        }
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Err(ApiError::conflict(format!("树号 {tree_no} 已存在")))
        }
        Err(e) => Err(ApiError::Db(e)),
    }
}

/// 工作人员：树木完整档案 —— 历次轮次、每次送样（含草稿结果）、退样重送链、处置单与回报
pub async fn tree_detail(State(db): State<Db>, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let tree = sqlx::query_as::<_, Tree>("SELECT * FROM trees WHERE id = ?")
        .bind(id)
        .fetch_optional(&db)
        .await?
        .ok_or_else(|| ApiError::not_found("树木不存在"))?;

    let rounds = sqlx::query_as::<_, Round>(
        "SELECT * FROM rounds WHERE tree_id = ? ORDER BY round_no DESC",
    )
    .bind(id)
    .fetch_all(&db)
    .await?;

    let mut round_json = Vec::new();
    for r in &rounds {
        let samples = sqlx::query_as::<_, Sample>(
            "SELECT * FROM samples WHERE round_id = ? ORDER BY submitted_at, id",
        )
        .bind(r.id)
        .fetch_all(&db)
        .await?;

        let mut sample_json = Vec::new();
        for s in &samples {
            let result = sqlx::query_as::<_, SampleResult>(
                "SELECT * FROM sample_results WHERE sample_id = ?",
            )
            .bind(s.id)
            .fetch_optional(&db)
            .await?;
            let parent_no = match s.parent_sample_id {
                Some(pid) => sqlx::query_scalar::<_, String>(
                    "SELECT sample_no FROM samples WHERE id = ?",
                )
                .bind(pid)
                .fetch_optional(&db)
                .await?,
                None => None,
            };
            // 重送链：哪些样本是本次的后续重送
            let children = sqlx::query_scalar::<_, String>(
                "SELECT sample_no FROM samples WHERE parent_sample_id = ? ORDER BY id",
            )
            .bind(s.id)
            .fetch_all(&db)
            .await?;
            sample_json.push(json!({
                "sample": s,
                "result": result,           // 工作人员可见草稿
                "parent_sample_no": parent_no,
                "resubmitted_as": children,
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
            let basis_sample_no = match d.sample_id {
                Some(sid) => {
                    sqlx::query_scalar::<_, String>("SELECT sample_no FROM samples WHERE id = ?")
                        .bind(sid)
                        .fetch_optional(&db)
                        .await?
                }
                None => None,
            };
            disposal_json.push(json!({
                "disposal": d,
                "reports": reports,
                "basis_sample_no": basis_sample_no,
            }));
        }

        round_json.push(json!({
            "round": r,
            "samples": sample_json,
            "disposals": disposal_json,
        }));
    }

    Ok(Json(json!({ "tree": tree, "rounds": round_json })))
}

/// 工作人员：手动开新一轮（复发时使用）；已有 OPEN 轮次则冲突
pub async fn open_round(
    State(db): State<Db>,
    Path(tree_id): Path<i64>,
    Json(body): Json<OpenRound>,
) -> ApiResult<Json<Value>> {
    let tree = sqlx::query_as::<_, Tree>("SELECT * FROM trees WHERE id = ?")
        .bind(tree_id)
        .fetch_optional(&db)
        .await?
        .ok_or_else(|| ApiError::not_found("树木不存在"))?;

    let mut tx = db.begin().await?;
    let open = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM rounds WHERE tree_id = ? AND status = 'OPEN'",
    )
    .bind(tree_id)
    .fetch_one(&mut *tx)
    .await?;
    if open > 0 {
        return Err(ApiError::conflict("该树已有进行中的处理轮次，请先结案再开新轮"));
    }
    let round_no = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(round_no), 0) + 1 FROM rounds WHERE tree_id = ?",
    )
    .bind(tree_id)
    .fetch_one(&mut *tx)
    .await?;
    let reason = body
        .reason
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| if round_no == 1 { "首次发现".into() } else { "病虫害复发".into() });
    let res = sqlx::query("INSERT INTO rounds(tree_id, round_no, reason) VALUES (?,?,?)")
        .bind(tree_id)
        .bind(round_no)
        .bind(&reason)
        .execute(&mut *tx)
        .await?;
    let round_id = res.last_insert_rowid();
    db::notify(
        &mut tx,
        "staff",
        &format!("树木 {} 开启第 {} 轮处理", tree.tree_no, round_no),
        &reason,
        "round",
        round_id,
    )
    .await?;
    tx.commit().await?;
    let round = sqlx::query_as::<_, Round>("SELECT * FROM rounds WHERE id = ?")
        .bind(round_id)
        .fetch_one(&db)
        .await?;
    Ok(Json(json!({ "round": round })))
}

/// 工作人员：轮次结案 —— 要求所有处置单已完成
pub async fn close_round(State(db): State<Db>, Path(round_id): Path<i64>) -> ApiResult<Json<Value>> {
    let round = sqlx::query_as::<_, Round>("SELECT * FROM rounds WHERE id = ?")
        .bind(round_id)
        .fetch_optional(&db)
        .await?
        .ok_or_else(|| ApiError::not_found("轮次不存在"))?;
    if round.status == "CLOSED" {
        return Err(ApiError::conflict("轮次已结案"));
    }
    let unfinished = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM disposals WHERE round_id = ? AND status != 'COMPLETED'",
    )
    .bind(round_id)
    .fetch_one(&db)
    .await?;
    if unfinished > 0 {
        return Err(ApiError::conflict(format!(
            "尚有 {unfinished} 张处置单未完成，不能结案"
        )));
    }
    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?")
        .bind(round_id)
        .execute(&db)
        .await?;
    let round = sqlx::query_as::<_, Round>("SELECT * FROM rounds WHERE id = ?")
        .bind(round_id)
        .fetch_one(&db)
        .await?;
    Ok(Json(json!({ "round": round })))
}

/// 巡查员送样：若该树无 OPEN 轮次则自动开新一轮（复发场景沿原树号）
pub async fn create_sample(
    State(db): State<Db>,
    Json(body): Json<CreateSample>,
) -> ApiResult<Json<Value>> {
    if body.inspector.trim().is_empty() {
        return Err(ApiError::bad_request("送检人不能为空"));
    }
    let tree = sqlx::query_as::<_, Tree>("SELECT * FROM trees WHERE id = ?")
        .bind(body.tree_id)
        .fetch_optional(&db)
        .await?
        .ok_or_else(|| ApiError::not_found("树木不存在"))?;

    let mut tx = db.begin().await?;
    let round_id = match sqlx::query_scalar::<_, i64>(
        "SELECT id FROM rounds WHERE tree_id = ? AND status = 'OPEN'",
    )
    .bind(tree.id)
    .fetch_optional(&mut *tx)
    .await?
    {
        Some(rid) => rid,
        None => {
            // 自动开新一轮：首次或复发
            let round_no = sqlx::query_scalar::<_, i64>(
                "SELECT COALESCE(MAX(round_no), 0) + 1 FROM rounds WHERE tree_id = ?",
            )
            .bind(tree.id)
            .fetch_one(&mut *tx)
            .await?;
            let reason = if round_no == 1 { "首次发现" } else { "病虫害复发" };
            let res = sqlx::query("INSERT INTO rounds(tree_id, round_no, reason) VALUES (?,?,?)")
                .bind(tree.id)
                .bind(round_no)
                .bind(reason)
                .execute(&mut *tx)
                .await?;
            res.last_insert_rowid()
        }
    };
    let sample_no = db::next_number(&mut tx, "sample", "SP").await?;
    let res = sqlx::query(
        "INSERT INTO samples(sample_no, tree_id, round_id, inspector, symptom, note)
         VALUES (?,?,?,?,?,?)",
    )
    .bind(&sample_no)
    .bind(tree.id)
    .bind(round_id)
    .bind(body.inspector.trim())
    .bind(body.symptom.unwrap_or_default())
    .bind(body.note.unwrap_or_default())
    .execute(&mut *tx)
    .await?;
    let sample_id = res.last_insert_rowid();
    db::notify(
        &mut tx,
        "lab",
        &format!("新送鉴样本 {sample_no}"),
        &format!("树木 {} 疑似病虫害，请鉴定", tree.tree_no),
        "sample",
        sample_id,
    )
    .await?;
    tx.commit().await?;
    let sample = sqlx::query_as::<_, Sample>("SELECT * FROM samples WHERE id = ?")
        .bind(sample_id)
        .fetch_one(&db)
        .await?;
    Ok(Json(json!({ "sample": sample })))
}

/// 退样/无法判定后重送：新样本关联原样本，沿用原轮次
pub async fn resubmit_sample(State(db): State<Db>, Path(id): Path<i64>, Json(body): Json<CreateSample>) -> ApiResult<Json<Value>> {
    let parent = sqlx::query_as::<_, Sample>("SELECT * FROM samples WHERE id = ?")
        .bind(id)
        .fetch_optional(&db)
        .await?
        .ok_or_else(|| ApiError::not_found("原样本不存在"))?;

    // 仅当原样本已有核实的退样/无法判定结论时才允许重送
    let parent_result = sqlx::query_as::<_, SampleResult>(
        "SELECT * FROM sample_results WHERE sample_id = ? AND status = 'VERIFIED'",
    )
    .bind(parent.id)
    .fetch_optional(&db)
    .await?
    .ok_or_else(|| ApiError::conflict("原样本尚无核实的退样/无法判定结论，不能重送"))?;
    if parent_result.result_type != "REJECTED" && parent_result.result_type != "INCONCLUSIVE" {
        return Err(ApiError::conflict("仅退样或无法判定的样本可以重送"));
    }

    let round = sqlx::query_as::<_, Round>("SELECT * FROM rounds WHERE id = ?")
        .bind(parent.round_id)
        .fetch_one(&db)
        .await?;
    if round.status != "OPEN" {
        return Err(ApiError::conflict("原轮次已结案，请开新一轮后重新送样"));
    }

    let mut tx = db.begin().await?;
    let sample_no = db::next_number(&mut tx, "sample", "SP").await?;
    let res = sqlx::query(
        "INSERT INTO samples(sample_no, tree_id, round_id, parent_sample_id, inspector, symptom, note)
         VALUES (?,?,?,?,?,?,?)",
    )
    .bind(&sample_no)
    .bind(parent.tree_id)
    .bind(parent.round_id)
    .bind(parent.id)
    .bind(if body.inspector.trim().is_empty() { &parent.inspector } else { body.inspector.trim() })
    .bind(body.symptom.unwrap_or_else(|| parent.symptom.clone()))
    .bind(body.note.unwrap_or_default())
    .execute(&mut *tx)
    .await?;
    let sample_id = res.last_insert_rowid();
    db::notify(
        &mut tx,
        "lab",
        &format!("退样重送 {sample_no}"),
        &format!("原样本 {} 退样/无法判定后重送", parent.sample_no),
        "sample",
        sample_id,
    )
    .await?;
    tx.commit().await?;
    let sample = sqlx::query_as::<_, Sample>("SELECT * FROM samples WHERE id = ?")
        .bind(sample_id)
        .fetch_one(&db)
        .await?;
    Ok(Json(json!({ "sample": sample, "parent_sample_no": parent.sample_no })))
}
