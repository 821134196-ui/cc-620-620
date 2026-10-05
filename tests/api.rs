// 端到端业务规则验证（Axum oneshot + 内存 SQLite）
use treecare::db;
use treecare::http::build_router;
use treecare::models::*;

use axum::body::Body;
use axum::Router;
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn app() -> Router {
    let pool = db::memory_pool().await.expect("memory pool");
    db::run_migrations(&pool).await.expect("migrations");
    build_router(pool)
}

async fn send(
    app: Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (u16, serde_json::Value) {
    let mut builder = axum::http::Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let req = match body {
        Some(v) => builder.body(Body::from(v.to_string())).unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status().as_u16();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    (status, json)
}

async fn make_tree(app: &Router, code: &str) -> i64 {
    let (s, j) = send(
        app.clone(),
        "POST",
        "/api/staff/trees",
        Some(serde_json::json!({
            "tree_code": code, "road_name": "林荫路", "species": "悬铃木",
            "x": 0.2, "y": 0.3
        })),
    )
    .await;
    assert_eq!(s, 201, "建木失败: {j}");
    j["id"].as_i64().unwrap()
}

async fn submit(app: &Router, tree_id: i64, parent: Option<i64>) -> (u16, serde_json::Value) {
    let mut v = serde_json::json!({
        "tree_id": tree_id, "inspector": "张巡", "symptom": "叶面密布虫瘿"
    });
    if let Some(p) = parent {
        v["parent_sample_id"] = serde_json::json!(p);
    }
    send(app.clone(), "POST", "/api/staff/samples", Some(v)).await
}

async fn record(app: &Router, sample_id: i64, conclusion: &str, diagnosis: &str) -> (u16, serde_json::Value) {
    send(
        app.clone(),
        "PUT",
        &format!("/api/staff/samples/{sample_id}/result"),
        Some(serde_json::json!({
            "conclusion": conclusion, "diagnosis": diagnosis,
            "severity": "moderate", "recorded_by": "李工"
        })),
    )
    .await
}

async fn verify(app: &Router, sample_id: i64) -> (u16, serde_json::Value) {
    send(
        app.clone(),
        "POST",
        &format!("/api/staff/samples/{sample_id}/verify"),
        Some(serde_json::json!({ "verified_by": "王主任" })),
    )
    .await
}

// ---------- 场景 1：同一棵树多份样本分别保留结论，不互相覆盖 ----------

#[tokio::test]
async fn multiple_samples_keep_separate_results() {
    let app = app().await;
    let tree_id = make_tree(&app, "T-1001").await;

    // 第一份样本：确诊方翅网蝽，核实归档
    let (_, s1) = submit(&app, tree_id, None).await;
    let id1 = s1["id"].as_i64().unwrap();
    assert_eq!(s1["round"], 1);
    assert_eq!(record(&app, id1, "pest_confirmed", "方翅网蝽").await.0, 200);
    assert_eq!(verify(&app, id1).await.0, 200);

    // 第二份样本（新一轮）：鉴定为健康，结论独立保留
    let (_, s2) = submit(&app, tree_id, None).await;
    let id2 = s2["id"].as_i64().unwrap();
    assert_eq!(s2["round"], 2, "新送样应进入第 2 轮");
    assert_eq!(record(&app, id2, "healthy", "").await.0, 200);
    assert_eq!(verify(&app, id2).await.0, 200);

    // 第三份：无法判定也作为该次记录永久保留
    let (_, s3) = submit(&app, tree_id, None).await;
    let id3 = s3["id"].as_i64().unwrap();
    assert_eq!(s3["round"], 3);
    assert_eq!(record(&app, id3, "undetermined", "").await.0, 200);
    assert_eq!(verify(&app, id3).await.0, 200);

    // 试图覆盖第一份已核实结论 → 409
    let (s_overwrite, j_over) = record(&app, id1, "healthy", "想改结论").await;
    assert_eq!(s_overwrite, 409, "已核实结论不应被覆盖: {j_over}");

    // 工作人员端历史：三条样本各自结论完整保留
    let (_, hist) = send(app.clone(), "GET", &format!("/api/staff/trees/{tree_id}/history"), None).await;
    let samples = hist["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 3, "历次送样均应保留");
    assert_eq!(samples[0]["lab_result"]["conclusion"], "pest_confirmed");
    assert_eq!(samples[0]["lab_result"]["diagnosis"], "方翅网蝽");
    assert_eq!(samples[0]["lab_result"]["verified_at"].is_string(), true);
    assert_eq!(samples[1]["lab_result"]["conclusion"], "healthy");
    assert_eq!(samples[2]["lab_result"]["conclusion"], "undetermined");
    // 旧结论未被覆盖
    assert_eq!(samples[0]["lab_result"]["diagnosis"], "方翅网蝽");

    // 公开时间线三条核实记录都在
    let (_, detail) = send(app.clone(), "GET", &format!("/api/public/trees/{tree_id}"), None).await;
    assert_eq!(detail["timeline"].as_array().unwrap().len(), 3);
}

// ---------- 场景 2：退样可以重送，且关联原送样过程 ----------

#[tokio::test]
async fn returned_sample_can_be_resent_with_link() {
    let app = app().await;
    let tree_id = make_tree(&app, "T-1002").await;

    let (_, original) = submit(&app, tree_id, None).await;
    let orig_id = original["id"].as_i64().unwrap();
    let orig_code = original["sample_code"].as_str().unwrap().to_string();

    // 实验室退样
    let (s_ret, j_ret) = send(
        app.clone(),
        "POST",
        &format!("/api/staff/samples/{orig_id}/return"),
        Some(serde_json::json!({ "reason": "样本干枯无法制片" })),
    )
    .await;
    assert_eq!(s_ret, 200, "退样失败: {j_ret}");
    assert_eq!(j_ret["status"], "returned");

    // 未退样的样本不能作为重送来源；先送一个新样本作对照
    let (_, other) = submit(&app, tree_id, None).await;
    let other_id = other["id"].as_i64().unwrap();
    let (s_bad, _) = submit(&app, tree_id, Some(other_id)).await;
    assert_eq!(s_bad, 409, "非退样样本不允许作为重送来源");

    // 正确重送：关联原样本，沿用同一轮次
    let (s_rs, resent) = submit(&app, tree_id, Some(orig_id)).await;
    assert_eq!(s_rs, 201, "退样重送失败: {resent}");
    let new_id = resent["id"].as_i64().unwrap();
    assert_ne!(new_id, orig_id, "重送必须是独立的新样本编号");
    assert_eq!(resent["round"], original["round"], "重送沿用原轮次");
    assert_eq!(resent["parent_sample_id"], orig_id);

    // 历史记录中：原退样记录保留，重送样本挂着 parent_sample_code；对照样本独立无父链
    let (_, hist) = send(app.clone(), "GET", &format!("/api/staff/trees/{tree_id}/history"), None).await;
    let samples = hist["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 3, "原样本 + 对照新样本 + 重送样本，均应保留");

    let returned = samples.iter().find(|x| x["sample"]["sample_code"] == orig_code).unwrap();
    assert_eq!(returned["sample"]["status"], "returned", "退样记录仍保留");
    assert_eq!(returned["sample"]["return_reason"], "样本干枯无法制片");
    assert!(returned["lab_result"].is_null(), "退样样本没有鉴定结果");

    let resent_rec = samples.iter().find(|x| x["sample"]["id"] == new_id).unwrap();
    assert_eq!(resent_rec["parent_sample_code"], orig_code, "重送须关联原送样编号");

    let control = samples.iter().find(|x| x["sample"]["id"] == other_id).unwrap();
    assert_eq!(control["sample"]["parent_sample_id"], serde_json::Value::Null);
    assert_eq!(control["sample"]["round"], 2, "对照的普通新送样进入下一轮");
}

// ---------- 场景 3：没有鉴定结论不能提前结案 ----------

#[tokio::test]
async fn cannot_close_treatment_without_verified_diagnosis() {
    let app = app().await;
    let tree_id = make_tree(&app, "T-1003").await;

    let (_, s1) = submit(&app, tree_id, None).await;
    let sid = s1["id"].as_i64().unwrap();

    let issue = |sample_id: i64| {
        let a = app.clone();
        async move {
            send(
                a,
                "POST",
                "/api/staff/treatments",
                Some(serde_json::json!({ "sample_id": sample_id, "assigned_unit": "绿洁养护公司" })),
            )
            .await
        }
    };

    // 3.1 完全没有鉴定结果 → 不能签发处置单
    let (s0, j0) = issue(sid).await;
    assert_eq!(s0, 400, "无鉴定结果不应签发处置单: {j0}");

    // 3.2 草稿（未核实）→ 仍不能签发
    assert_eq!(record(&app, sid, "pest_confirmed", "美国白蛾").await.0, 200);
    let (s_draft, j_draft) = issue(sid).await;
    assert_eq!(s_draft, 400, "草稿未核实不应签发处置单: {j_draft}");
    // 草稿不对居民公开
    let (_, pd) = send(app.clone(), "GET", &format!("/api/public/trees/{tree_id}"), None).await;
    assert_eq!(pd["timeline"].as_array().unwrap().len(), 0, "草稿不得出现在公开页");

    // 3.3 核实后签发；处置单 issued 状态直接结案 → 必须先接收/回报
    assert_eq!(verify(&app, sid).await.0, 200);
    let (s_issue, t1) = issue(sid).await;
    assert_eq!(s_issue, 201, "核实后应能签发处置单: {t1}");
    let tid = t1["id"].as_i64().unwrap();

    let (s_early, _) = send(app.clone(), "POST", &format!("/api/staff/treatments/{tid}/complete"), None).await;
    assert_eq!(s_early, 400, "未接收回报前不能结案");

    // 3.4 养护单位接收并回报 → 进入 in_progress，之后才能结案
    let (s_rep, j_rep) = send(
        app.clone(),
        "POST",
        &format!("/api/staff/treatments/{tid}/reports"),
        Some(serde_json::json!({ "content": "已喷洒生物药剂并剪除虫枝", "reporter": "赵班" })),
    )
    .await;
    assert_eq!(s_rep, 200, "回报失败: {j_rep}");
    assert_eq!(j_rep["status"], "in_progress");

    let (s_done, done) = send(app.clone(), "POST", &format!("/api/staff/treatments/{tid}/complete"), None).await;
    assert_eq!(s_done, 200, "核实确诊+已回报后应允许结案: {done}");
    assert_eq!(done["status"], "completed");
    assert!(done["completed_at"].is_string());

    // 3.5 结案后不能重复结案/再回报
    assert_eq!(send(app.clone(), "POST", &format!("/api/staff/treatments/{tid}/complete"), None).await.0, 409);
    let (s_r2, _) = send(
        app.clone(),
        "POST",
        &format!("/api/staff/treatments/{tid}/reports"),
        Some(serde_json::json!({ "content": "x", "reporter": "y" })),
    )
    .await;
    assert_eq!(s_r2, 409, "结案后不应再接受回报");
}

// ---------- 场景 4：健康/无法判定核实后公开，但无需处置单 ----------

#[tokio::test]
async fn non_pest_conclusions_public_but_no_treatment() {
    let app = app().await;
    let tree_id = make_tree(&app, "T-1004").await;
    let (_, s1) = submit(&app, tree_id, None).await;
    let sid = s1["id"].as_i64().unwrap();
    assert_eq!(record(&app, sid, "undetermined", "").await.0, 200);
    assert_eq!(verify(&app, sid).await.0, 200);

    let (s_issue, _) = send(
        app.clone(),
        "POST",
        "/api/staff/treatments",
        Some(serde_json::json!({ "sample_id": sid, "assigned_unit": "绿洁养护公司" })),
    )
    .await;
    assert_eq!(s_issue, 400, "无法判定不应签发处置单");

    let (_, detail) = send(app.clone(), "GET", &format!("/api/public/trees/{tree_id}"), None).await;
    assert_eq!(detail["timeline"][0]["conclusion"], "undetermined");
    assert_eq!(detail["public_status"], "undetermined");
}

// ---------- 场景 5：病虫害复发沿原树号新开一轮，结案后可走完整新流程 ----------

#[tokio::test]
async fn recurrence_starts_new_round_on_same_tree() {
    let app = app().await;
    let tree_id = make_tree(&app, "T-1005").await;

    // 第 1 轮完整闭环
    let (_, s1) = submit(&app, tree_id, None).await;
    let id1 = s1["id"].as_i64().unwrap();
    record(&app, id1, "pest_confirmed", "蚜虫").await;
    verify(&app, id1).await;
    let (_, t1) = send(
        app.clone(),
        "POST",
        "/api/staff/treatments",
        Some(serde_json::json!({ "sample_id": id1, "assigned_unit": "绿洁养护公司" })),
    )
    .await;
    let tid1 = t1["id"].as_i64().unwrap();
    send(
        app.clone(),
        "POST",
        &format!("/api/staff/treatments/{tid1}/reports"),
        Some(serde_json::json!({ "content": "喷药完成", "reporter": "赵班" })),
    )
    .await;
    send(app.clone(), "POST", &format!("/api/staff/treatments/{tid1}/complete"), None).await;

    // 复发：同一固定树号，第 2 轮送样
    let (s2, j2) = submit(&app, tree_id, None).await;
    assert_eq!(s2, 201);
    assert_eq!(j2["round"], 2);
    assert_eq!(j2["parent_sample_id"], serde_json::Value::Null, "复发不是重送，无父样本");
    let id2 = j2["id"].as_i64().unwrap();
    record(&app, id2, "pest_confirmed", "蚜虫复发").await;
    verify(&app, id2).await;
    let (_, t2) = send(
        app.clone(),
        "POST",
        "/api/staff/treatments",
        Some(serde_json::json!({ "sample_id": id2, "assigned_unit": "绿洁养护公司" })),
    )
    .await;
    assert_eq!(t2["round"], 2, "新处置单属于第 2 轮");

    // 树号不变，历史含两轮
    let (_, hist) = send(app.clone(), "GET", &format!("/api/staff/trees/{tree_id}/history"), None).await;
    assert_eq!(hist["tree"]["tree_code"], "T-1005");
    assert_eq!(hist["samples"].as_array().unwrap().len(), 2);
}

// ---------- 场景 6：道路搜索与树号搜索 ----------

#[tokio::test]
async fn search_by_road_and_tree_code() {
    let app = app().await;
    make_tree(&app, "T-2001").await;
    let (_, by_road) = send(app.clone(), "GET", "/api/public/trees?q=林荫", None).await;
    assert!(by_road.as_array().unwrap().iter().any(|t| t["tree"]["tree_code"] == "T-2001"));
    let (_, by_code) = send(app.clone(), "GET", "/api/public/trees?q=T-2001", None).await;
    assert_eq!(by_code.as_array().unwrap().len(), 1);
    // 地图接口
    let (s_map, map) = send(app.clone(), "GET", "/api/public/map", None).await;
    assert_eq!(s_map, 200);
    assert!(map.as_array().unwrap().iter().any(|t| t["tree"]["x"].as_f64().unwrap() == 0.2));
}
