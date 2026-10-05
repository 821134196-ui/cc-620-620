use crate::db::{next_tree_code, next_yearly_code};
use crate::error::{AppError, AppResult};
use crate::models::*;
use crate::notify;
use sqlx::{Row, SqlitePool};

// ---------- 树木台账 ----------

pub async fn list_trees(pool: &SqlitePool, q: Option<&str>) -> AppResult<Vec<Tree>> {
    let trees = match q.filter(|s| !s.trim().is_empty()) {
        Some(kw) => {
            let like = format!("%{}%", kw.trim());
            sqlx::query_as::<_, Tree>(
                "SELECT * FROM trees WHERE tree_code LIKE ?1 OR road_name LIKE ?1
                 ORDER BY tree_code",
            )
            .bind(like)
            .fetch_all(pool)
            .await?
        }
        None => sqlx::query_as::<_, Tree>("SELECT * FROM trees ORDER BY tree_code")
            .fetch_all(pool)
            .await?,
    };
    Ok(trees)
}

pub async fn create_tree(pool: &SqlitePool, input: CreateTree) -> AppResult<Tree> {
    if input.road_name.trim().is_empty() || input.species.trim().is_empty() {
        return Err(AppError::bad_request("道路与树种不能为空"));
    }
    if !(0.0..=1.0).contains(&input.x) || !(0.0..=1.0).contains(&input.y) {
        return Err(AppError::bad_request("地图坐标必须在 0..1 之间"));
    }
    let code = if input.tree_code.trim().is_empty() {
        next_tree_code(pool).await?
    } else {
        input.tree_code.trim().to_string()
    };
    let res = sqlx::query(
        "INSERT INTO trees (tree_code, road_name, species, x, y, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(&code)
    .bind(&input.road_name)
    .bind(&input.species)
    .bind(input.x)
    .bind(input.y)
    .bind(&input.notes)
    .execute(pool)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db) = &e {
            if db.message().contains("UNIQUE") {
                return AppError::conflict(format!("树号 {code} 已存在"));
            }
        }
        AppError::Sqlx(e)
    })?;
    let tree = get_tree_by_id(pool, res.last_insert_rowid()).await?;
    notify::emit(
        pool,
        "web",
        "工作人员",
        "树木建档",
        &format!("{}（{} · {}）已建立固定树号档案", tree.tree_code, tree.road_name, tree.species),
        Some(tree.id),
        false,
    )
    .await?;
    Ok(tree)
}

pub async fn get_tree_by_id(pool: &SqlitePool, id: i64) -> AppResult<Tree> {
    sqlx::query_as::<_, Tree>("SELECT * FROM trees WHERE id = ?1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("树木不存在"))
}

// ---------- 送样 ----------

/// 计算某棵树的下一轮轮次
async fn next_round(pool: &SqlitePool, tree_id: i64) -> AppResult<i64> {
    let r: Option<i64> = sqlx::query_scalar("SELECT MAX(round) FROM samples WHERE tree_id = ?1")
        .bind(tree_id)
        .fetch_one(pool)
        .await?;
    Ok(r.unwrap_or(0) + 1)
}

pub async fn submit_sample(pool: &SqlitePool, input: SubmitSample) -> AppResult<Sample> {
    if input.inspector.trim().is_empty() || input.symptom.trim().is_empty() {
        return Err(AppError::bad_request("巡查员与疑似症状不能为空"));
    }
    let tree_id = input.tree_id.ok_or_else(|| AppError::bad_request("缺少 tree_id"))?;
    let tree = get_tree_by_id(pool, tree_id).await?;

    let (round, parent_id) = match input.parent_sample_id {
        Some(pid) => {
            // 退样重送：必须关联同一棵树上处于“已退样”状态的原样本，沿用原轮次
            let parent = sqlx::query_as::<_, Sample>("SELECT * FROM samples WHERE id = ?1")
                .bind(pid)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("原送样记录不存在"))?;
            if parent.tree_id != tree_id {
                return Err(AppError::bad_request("重送样本必须与原样本属于同一棵树"));
            }
            if parent.status != "returned" {
                return Err(AppError::conflict(format!(
                    "原样本 {} 不是退样状态，不能作为重送来源",
                    parent.sample_code
                )));
            }
            (parent.round, Some(parent.id))
        }
        None => (next_round(pool, tree_id).await?, None),
    };

    let code = next_yearly_code(pool, "samples", "sample_code", "S").await?;
    let res = sqlx::query(
        "INSERT INTO samples (sample_code, tree_id, round, parent_sample_id, status, inspector, symptom)
         VALUES (?1, ?2, ?3, ?4, 'submitted', ?5, ?6)",
    )
    .bind(&code)
    .bind(tree_id)
    .bind(round)
    .bind(parent_id)
    .bind(&input.inspector)
    .bind(&input.symptom)
    .execute(pool)
    .await?;
    let sample = sqlx::query_as::<_, Sample>("SELECT * FROM samples WHERE id = ?1")
        .bind(res.last_insert_rowid())
        .fetch_one(pool)
        .await?;

    let body = match parent_id {
        Some(pid) => {
            let pcode: String = sqlx::query_scalar("SELECT sample_code FROM samples WHERE id = ?1")
                .bind(pid)
                .fetch_one(pool)
                .await?;
            format!("巡查员 {} 就 {} 退样重送样本 {}（关联原送样 {}），症状：{}",
                input.inspector, tree.tree_code, code, pcode, input.symptom)
        }
        None => format!("巡查员 {} 发现 {} 疑似病虫害，已送样 {}（第 {} 轮），症状：{}",
            input.inspector, tree.tree_code, code, round, input.symptom),
    };
    // 送样信息未经实验室核实，仅内部通知
    notify::emit(pool, "web", "实验室/养护管理", "新送鉴样本", &body, Some(tree_id), false).await?;
    Ok(sample)
}

pub async fn return_sample(pool: &SqlitePool, sample_id: i64, input: ReturnSample) -> AppResult<Sample> {
    if input.reason.trim().is_empty() {
        return Err(AppError::bad_request("退样原因不能为空"));
    }
    let sample = get_sample(pool, sample_id).await?;
    if sample.status != "submitted" {
        return Err(AppError::conflict(format!(
            "样本 {} 当前为 {}，仅待检样本可以退样",
            sample.sample_code, status_text(&sample.status)
        )));
    }
    sqlx::query(
        "UPDATE samples SET status = 'returned', return_reason = ?1,
             returned_at = datetime('now','localtime') WHERE id = ?2",
    )
    .bind(&input.reason)
    .bind(sample_id)
    .execute(pool)
    .await?;
    let tree = get_tree_by_id(pool, sample.tree_id).await?;
    notify::emit(
        pool, "web", &format!("巡查员 {}", sample.inspector), "样本退样",
        &format!("样本 {}（树 {}）已退样：{}。可凭原送样记录重新送样。",
            sample.sample_code, tree.tree_code, input.reason),
        Some(sample.tree_id), false,
    ).await?;
    Ok(get_sample(pool, sample_id).await?)
}

async fn get_sample(pool: &SqlitePool, id: i64) -> AppResult<Sample> {
    sqlx::query_as::<_, Sample>("SELECT * FROM samples WHERE id = ?1")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::not_found("样本不存在"))
}

// ---------- 实验室鉴定 ----------

/// 记录（或在核实前修改）鉴定结果。结果一律先以草稿状态保存，不对居民公开。
pub async fn record_result(pool: &SqlitePool, sample_id: i64, input: RecordResult) -> AppResult<LabResult> {
    if !matches!(input.conclusion.as_str(), "pest_confirmed" | "healthy" | "undetermined") {
        return Err(AppError::bad_request("结论类型无效"));
    }
    if input.recorded_by.trim().is_empty() {
        return Err(AppError::bad_request("鉴定人不能为空"));
    }
    let sample = get_sample(pool, sample_id).await?;
    if sample.status == "returned" {
        return Err(AppError::conflict("退样样本不能登记鉴定结果"));
    }

    if let Some(existing) = sqlx::query_as::<_, LabResult>("SELECT * FROM lab_results WHERE sample_id = ?1")
        .bind(sample_id)
        .fetch_optional(pool)
        .await?
    {
        if existing.verified_at.is_some() {
            // 已核实结论锁定：不能覆盖之前结论
            return Err(AppError::conflict(format!(
                "样本 {} 的鉴定结论已核实并归档，不能覆盖；如病虫害复发请沿原树号新一轮送样",
                sample.sample_code
            )));
        }
        sqlx::query(
            "UPDATE lab_results SET conclusion=?1, diagnosis=?2, severity=?3, note=?4,
                 recorded_by=?5, is_draft=1, created_at=datetime('now','localtime')
             WHERE sample_id=?6",
        )
        .bind(&input.conclusion).bind(&input.diagnosis).bind(&input.severity)
        .bind(&input.note).bind(&input.recorded_by).bind(sample_id)
        .execute(pool).await?;
    } else {
        sqlx::query(
            "INSERT INTO lab_results (sample_id, conclusion, diagnosis, severity, note, recorded_by, is_draft)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
        )
        .bind(sample_id).bind(&input.conclusion).bind(&input.diagnosis)
        .bind(&input.severity).bind(&input.note).bind(&input.recorded_by)
        .execute(pool).await?;
        sqlx::query("UPDATE samples SET status='determined' WHERE id=?1")
            .bind(sample_id).execute(pool).await?;
    }

    let result = sqlx::query_as::<_, LabResult>("SELECT * FROM lab_results WHERE sample_id = ?1")
        .bind(sample_id)
        .fetch_one(pool)
        .await?;
    notify::emit(
        pool, "web", "实验室审核人", "鉴定结果草稿待核实",
        &format!("样本 {} 的鉴定结果已登记（{}），尚为草稿，核实后才对居民公开。",
            sample.sample_code, conclusion_text(&input.conclusion)),
        Some(sample.tree_id), false,
    ).await?;
    Ok(result)
}

/// 核实：草稿转正式结论。核实后锁定、对居民公开，并触发公开通知。
pub async fn verify_result(pool: &SqlitePool, sample_id: i64, input: VerifyResult) -> AppResult<LabResult> {
    if input.verified_by.trim().is_empty() {
        return Err(AppError::bad_request("核实人不能为空"));
    }
    let sample = get_sample(pool, sample_id).await?;
    let result = sqlx::query_as::<_, LabResult>("SELECT * FROM lab_results WHERE sample_id = ?1")
        .bind(sample_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::bad_request("该样本尚无鉴定结果，无法核实"))?;
    if result.verified_at.is_some() {
        return Err(AppError::conflict("该鉴定结果已经核实，不能重复核实"));
    }

    sqlx::query(
        "UPDATE lab_results SET is_draft=0, verified_by=?1,
             verified_at=datetime('now','localtime') WHERE id=?2",
    )
    .bind(&input.verified_by).bind(result.id)
    .execute(pool).await?;
    let result = sqlx::query_as::<_, LabResult>("SELECT * FROM lab_results WHERE id = ?1")
        .bind(result.id).fetch_one(pool).await?;
    let tree = get_tree_by_id(pool, sample.tree_id).await?;

    let detail = match result.conclusion.as_str() {
        "pest_confirmed" => format!("确诊{} {}", result.diagnosis, severity_text(&result.severity)),
        "healthy" => "本次鉴定未见病虫害".to_string(),
        _ => "样本无法判定，本次记录留存".to_string(),
    };
    // 内部通知
    notify::emit(
        pool, "web", "养护管理", "鉴定结论已核实",
        &format!("树 {} 样本 {}（第{}轮）：{}", tree.tree_code, sample.sample_code, sample.round, detail),
        Some(tree.id), false,
    ).await?;
    // 公开通知（公告板 + 模拟短信），仅已核实内容
    let pub_body = format!("【树木管护公示】{} {} 第{}轮鉴定：{}。核实人：{}",
        tree.road_name, tree.tree_code, sample.round, detail, input.verified_by);
    notify::emit(pool, "public_board", &format!("{} 沿线居民", tree.road_name),
        "病虫害鉴定结果公示", &pub_body, Some(tree.id), true).await?;
    notify::emit(pool, "sms", &format!("{}-居民(模拟)", tree.tree_code),
        "病虫害鉴定结果通知", &pub_body, Some(tree.id), true).await?;
    Ok(result)
}

// ---------- 处置单与治理回报 ----------

struct SampleWithResult {
    sample: Sample,
    result: Option<LabResult>,
}

async fn sample_with_result(pool: &SqlitePool, sample_id: i64) -> AppResult<SampleWithResult> {
    let sample = get_sample(pool, sample_id).await?;
    let result = sqlx::query_as::<_, LabResult>("SELECT * FROM lab_results WHERE sample_id = ?1")
        .bind(sample_id).fetch_optional(pool).await?;
    Ok(SampleWithResult { sample, result })
}

pub async fn issue_treatment(pool: &SqlitePool, input: IssueTreatment) -> AppResult<Treatment> {
    if input.assigned_unit.trim().is_empty() {
        return Err(AppError::bad_request("养护单位不能为空"));
    }
    let sw = sample_with_result(pool, input.sample_id).await?;
    let result = sw.result.as_ref().ok_or_else(|| {
        AppError::bad_request(format!("样本 {} 尚无鉴定结果，不能签发处置单", sw.sample.sample_code))
    })?;
    if result.is_draft != 0 || result.verified_at.is_none() {
        return Err(AppError::bad_request(format!(
            "样本 {} 的鉴定结果尚为草稿/未核实，不能签发处置单", sw.sample.sample_code
        )));
    }
    if result.conclusion != "pest_confirmed" {
        return Err(AppError::bad_request(format!(
            "样本 {} 结论为“{}”，未确诊病虫害，无需处置单",
            sw.sample.sample_code, conclusion_text(&result.conclusion)
        )));
    }
    if let Some(t) = sqlx::query_as::<_, Treatment>("SELECT * FROM treatments WHERE sample_id = ?1")
        .bind(input.sample_id).fetch_optional(pool).await?
    {
        return Err(AppError::conflict(format!("样本 {} 已签发处置单 {}", sw.sample.sample_code, t.treatment_code)));
    }

    let code = next_yearly_code(pool, "treatments", "treatment_code", "W").await?;
    let res = sqlx::query(
        "INSERT INTO treatments (treatment_code, tree_id, sample_id, round, assigned_unit, status)
         VALUES (?1, ?2, ?3, ?4, ?5, 'issued')",
    )
    .bind(&code).bind(sw.sample.tree_id).bind(sw.sample.id)
    .bind(sw.sample.round).bind(&input.assigned_unit)
    .execute(pool).await?;
    let treatment = sqlx::query_as::<_, Treatment>("SELECT * FROM treatments WHERE id = ?1")
        .bind(res.last_insert_rowid()).fetch_one(pool).await?;
    let tree = get_tree_by_id(pool, sw.sample.tree_id).await?;

    notify::emit(pool, "web", &input.assigned_unit, "处置单已下达",
        &format!("处置单 {} 已下发：树 {}（{}）确诊{}，请安排治理。",
            code, tree.tree_code, tree.road_name, result.diagnosis),
        Some(tree.id), false).await?;
    notify::emit(pool, "sms", &input.assigned_unit, "处置单通知",
        &format!("【处置单 {}】{} {} 确诊{}，请接单治理。", code, tree.tree_code, tree.road_name, result.diagnosis),
        Some(tree.id), true).await?;
    Ok(treatment)
}

pub async fn add_report(pool: &SqlitePool, treatment_id: i64, input: AddReport) -> AppResult<Treatment> {
    if input.content.trim().is_empty() || input.reporter.trim().is_empty() {
        return Err(AppError::bad_request("回报内容与报告人不能为空"));
    }
    let treatment = get_treatment(pool, treatment_id).await?;
    if treatment.status == "completed" {
        return Err(AppError::conflict(format!("处置单 {} 已结案，不能再回报", treatment.treatment_code)));
    }
    sqlx::query("INSERT INTO treatment_reports (treatment_id, content, reporter) VALUES (?1, ?2, ?3)")
        .bind(treatment_id).bind(&input.content).bind(&input.reporter)
        .execute(pool).await?;

    if treatment.status == "issued" {
        sqlx::query(
            "UPDATE treatments SET status='in_progress', accepted_at=datetime('now','localtime') WHERE id=?1",
        )
        .bind(treatment_id).execute(pool).await?;
    }
    sqlx::query("UPDATE treatments SET report_summary=?1 WHERE id=?2")
        .bind(&input.content).bind(treatment_id).execute(pool).await?;

    let tree = get_tree_by_id(pool, treatment.tree_id).await?;
    notify::emit(pool, "web", "养护管理", "治理回报",
        &format!("处置单 {}（树 {}）养护单位回报：{}（报告人 {}）",
            treatment.treatment_code, tree.tree_code, input.content, input.reporter),
        Some(tree.id), false).await?;
    notify::emit(pool, "public_board", &format!("{} 沿线居民", tree.road_name), "治理进展公示",
        &format!("【治理进展】{} 防治作业进展：{}", tree.tree_code, input.content),
        Some(tree.id), true).await?;
    Ok(get_treatment(pool, treatment_id).await?)
}

/// 结案守卫：没有“已核实的确诊鉴定结论”绝不允许标记治理完成
pub async fn complete_treatment(pool: &SqlitePool, treatment_id: i64) -> AppResult<Treatment> {
    let treatment = get_treatment(pool, treatment_id).await?;
    if treatment.status == "completed" {
        return Err(AppError::conflict(format!("处置单 {} 已结案", treatment.treatment_code)));
    }
    let result = sqlx::query_as::<_, LabResult>("SELECT lr.* FROM lab_results lr
            JOIN treatments t ON t.sample_id = lr.sample_id WHERE t.id = ?1")
        .bind(treatment_id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::conflict("处置单缺少鉴定结果，不能结案"))?;
    if result.is_draft != 0 || result.verified_at.is_none() {
        return Err(AppError::conflict("鉴定结论尚未核实，不能提前结案"));
    }
    if result.conclusion != "pest_confirmed" {
        return Err(AppError::conflict("鉴定结论不是确诊病虫害，不能以治理完成结案"));
    }
    let report_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM treatment_reports WHERE treatment_id = ?1",
    )
    .bind(treatment_id).fetch_one(pool).await?;
    if report_count == 0 {
        return Err(AppError::bad_request("养护单位尚未回报治理情况，不能结案"));
    }
    if treatment.status != "in_progress" {
        return Err(AppError::bad_request("处置单尚未被养护单位接收，不能结案"));
    }

    sqlx::query(
        "UPDATE treatments SET status='completed', completed_at=datetime('now','localtime') WHERE id=?1",
    )
    .bind(treatment_id).execute(pool).await?;
    let treatment = get_treatment(pool, treatment_id).await?;
    let tree = get_tree_by_id(pool, treatment.tree_id).await?;
    notify::emit(pool, "web", "养护管理", "处置结案",
        &format!("处置单 {}（树 {}）治理完成，已结案。", treatment.treatment_code, tree.tree_code),
        Some(tree.id), false).await?;
    notify::emit(pool, "public_board", &format!("{} 沿线居民", tree.road_name), "治理完成公示",
        &format!("【治理完成】{} 本轮病虫害防治已完成，感谢居民监督。如再次发现异常，将沿原树号开启新一轮处理。",
            tree.tree_code),
        Some(tree.id), true).await?;
    Ok(treatment)
}

async fn get_treatment(pool: &SqlitePool, id: i64) -> AppResult<Treatment> {
    sqlx::query_as::<_, Treatment>("SELECT * FROM treatments WHERE id = ?1")
        .bind(id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::not_found("处置单不存在"))
}

// ---------- 工作人员端：历次完整记录 ----------

async fn load_sample_full(pool: &SqlitePool, s: &Sample) -> AppResult<SampleFull> {
    let parent_code = match s.parent_sample_id {
        Some(pid) => Some(
            sqlx::query_scalar("SELECT sample_code FROM samples WHERE id=?1")
                .bind(pid).fetch_one(pool).await?,
        ),
        None => None,
    };
    let lab_result = sqlx::query_as::<_, LabResult>("SELECT * FROM lab_results WHERE sample_id=?1")
        .bind(s.id).fetch_optional(pool).await?;
    let treatment = match sqlx::query_as::<_, Treatment>("SELECT * FROM treatments WHERE sample_id=?1")
        .bind(s.id).fetch_optional(pool).await?
    {
        Some(t) => {
            let reports = sqlx::query_as::<_, TreatmentReport>(
                "SELECT * FROM treatment_reports WHERE treatment_id=?1 ORDER BY id",
            )
            .bind(t.id).fetch_all(pool).await?;
            Some(TreatmentWithReports { treatment: t, reports })
        }
        None => None,
    };
    Ok(SampleFull {
        sample: s.clone(),
        parent_sample_code: parent_code,
        lab_result,
        treatment,
    })
}

pub async fn tree_history(pool: &SqlitePool, tree_id: i64) -> AppResult<TreeHistory> {
    let tree = get_tree_by_id(pool, tree_id).await?;
    let rows = sqlx::query_as::<_, Sample>("SELECT * FROM samples WHERE tree_id=?1 ORDER BY round, id")
        .bind(tree_id).fetch_all(pool).await?;
    let mut samples = Vec::new();
    for s in rows {
        samples.push(load_sample_full(pool, &s).await?);
    }
    Ok(TreeHistory { tree, samples })
}

// ---------- 居民公开端：仅核实后内容 ----------

fn conclusion_text(c: &str) -> String {
    match c {
        "pest_confirmed" => "确诊病虫害".to_string(),
        "healthy" => "未见病虫害".to_string(),
        "undetermined" => "无法判定".to_string(),
        other => other.to_string(),
    }
}
fn severity_text(s: &str) -> String {
    match s {
        "mild" => "（轻度）".to_string(),
        "moderate" => "（中度）".to_string(),
        "severe" => "（重度）".to_string(),
        _ => String::new(),
    }
}
fn status_text(s: &str) -> String {
    match s {
        "submitted" => "待检".to_string(),
        "returned" => "已退样".to_string(),
        "testing" => "检测中".to_string(),
        "determined" => "已有结论".to_string(),
        other => other.to_string(),
    }
}

fn verified_timeline_sql() -> &'static str {
    r#"
    SELECT s.round, s.sample_code AS sample_code, ps.sample_code AS parent_sample_code,
           lr.conclusion, lr.diagnosis, lr.severity, lr.note, lr.verified_at AS at,
           t.treatment_code, t.status AS treatment_status, t.assigned_unit,
           COALESCE(t.report_summary,'') AS report_summary, t.completed_at
    FROM samples s
    JOIN lab_results lr ON lr.sample_id = s.id
    LEFT JOIN samples ps ON ps.id = s.parent_sample_id
    LEFT JOIN treatments t ON t.sample_id = s.id
    WHERE s.tree_id = ?1 AND lr.is_draft = 0 AND lr.verified_at IS NOT NULL
    ORDER BY lr.verified_at, s.id
    "#
}

fn row_to_timeline(r: &sqlx::sqlite::SqliteRow) -> PublicTimelineItem {
    PublicTimelineItem {
        kind: "sample_verified".to_string(),
        at: r.get("at"),
        round: r.get("round"),
        sample_code: r.get("sample_code"),
        parent_sample_code: r.get("parent_sample_code"),
        conclusion: r.get("conclusion"),
        diagnosis: r.get("diagnosis"),
        severity: r.get("severity"),
        note: r.get("note"),
        treatment_code: r.get("treatment_code"),
        treatment_status: r.get("treatment_status"),
        assigned_unit: r.get("assigned_unit"),
        report_summary: r.get("report_summary"),
        completed_at: r.get("completed_at"),
    }
}

/// 依据最近一轮已核实记录计算公开状态
async fn public_status_of(pool: &SqlitePool, tree_id: i64) -> AppResult<(String, String)> {
    let row = sqlx::query(
        "SELECT s.round, lr.conclusion, t.status FROM samples s
         JOIN lab_results lr ON lr.sample_id = s.id
         LEFT JOIN treatments t ON t.sample_id = s.id
         WHERE s.tree_id=?1 AND lr.is_draft=0 AND lr.verified_at IS NOT NULL
         ORDER BY s.round DESC, lr.verified_at DESC LIMIT 1",
    )
    .bind(tree_id)
    .fetch_optional(pool)
    .await?;
    let Some(r) = row else {
        return Ok(("no_verified".into(), "暂无核实结论".into()));
    };
    let conclusion: String = r.get("conclusion");
    let treatment_status: Option<String> = r.get("status");
    let pair = match (conclusion.as_str(), treatment_status.as_deref()) {
        ("pest_confirmed", Some("completed")) => ("treated", "本轮治理已完成"),
        ("pest_confirmed", Some("in_progress")) => ("treating", "确诊 · 治理进行中"),
        ("pest_confirmed", Some("issued")) => ("confirmed_pending", "已确诊 · 待养护单位处置"),
        ("pest_confirmed", None) => ("confirmed_pending", "已确诊 · 待下达处置单"),
        ("healthy", _) => ("healthy", "本次鉴定未见病虫害"),
        ("undetermined", _) => ("undetermined", "鉴定无法判定"),
        _ => ("no_verified", "暂无核实结论"),
    };
    Ok((pair.0.to_string(), pair.1.to_string()))
}

pub async fn public_search(pool: &SqlitePool, q: Option<&str>) -> AppResult<Vec<TreePublic>> {
    let trees = list_trees(pool, q).await?;
    let mut out = Vec::new();
    for t in trees {
        let (status, text) = public_status_of(pool, t.id).await?;
        out.push(TreePublic { tree: t, public_status: status, public_status_text: text });
    }
    Ok(out)
}

pub async fn public_detail(pool: &SqlitePool, tree_id: i64) -> AppResult<TreePublicDetail> {
    let tree = get_tree_by_id(pool, tree_id).await?;
    let (status, text) = public_status_of(pool, tree_id).await?;
    let rows = sqlx::query(verified_timeline_sql())
        .bind(tree_id).fetch_all(pool).await?;
    let timeline = rows.iter().map(row_to_timeline).collect();
    Ok(TreePublicDetail {
        tree,
        public_status: status,
        public_status_text: text,
        timeline,
    })
}

/// 居民公开地图：只暴露树号/道路/坐标与已核实状态
pub async fn public_map(pool: &SqlitePool) -> AppResult<Vec<TreePublic>> {
    public_search(pool, None).await
}

/// 工作人员地图：附加树的送样计数与未完成事项
pub async fn staff_map(pool: &SqlitePool) -> AppResult<Vec<serde_json::Value>> {
    let base = public_map(pool).await?;
    let mut out = Vec::new();
    for tp in base {
        let active: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM treatments WHERE tree_id=?1 AND status != 'completed'",
        )
        .bind(tp.tree.id).fetch_one(pool).await?;
        let drafts: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM samples s JOIN lab_results lr ON lr.sample_id=s.id
             WHERE s.tree_id=?1 AND lr.is_draft=1",
        )
        .bind(tp.tree.id).fetch_one(pool).await?;
        out.push(serde_json::json!({
            "tree": tp.tree,
            "public_status": tp.public_status, "public_status_text": tp.public_status_text,
            "active_treatments": active, "draft_results": drafts,
        }));
    }
    Ok(out)
}

// ---------- 通知（本地模拟） ----------

pub async fn list_notifications(pool: &SqlitePool, only_public: bool) -> AppResult<Vec<Notification>> {
    let q = if only_public {
        sqlx::query_as::<_, Notification>(
            "SELECT * FROM notifications WHERE public_visible=1 ORDER BY id DESC LIMIT 100",
        )
    } else {
        sqlx::query_as::<_, Notification>("SELECT * FROM notifications ORDER BY id DESC LIMIT 200")
    };
    Ok(q.fetch_all(pool).await?)
}
