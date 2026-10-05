//! 端到端集成测试：真实启动 HTTP 服务 + 独立 SQLite 库
//! 覆盖需求验证点：
//!  1. 同一棵树多份样本各自保留鉴定结果，互不覆盖
//!  2. 退样可以重送，且重送样本关联原送样过程
//!  3. 没有已核实鉴定结论的处置不能提前结案
//!  4. 公开页不展示实验室草稿
//!  5. 病虫害复发沿原树号开新一轮处理

use reqwest::Client;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

struct TestApp {
    base: String,
    client: Client,
    _dir: std::path::PathBuf,
}

async fn spawn_app() -> TestApp {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("treecare-test-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.db");
    let url = format!("sqlite://{}?mode=rwc", db_path.display());

    let pool = treecare::db::connect(&url).await;
    treecare::db::migrate(&pool).await;
    // 测试不跑种子数据，保证场景干净

    let app = treecare::routes::build_router(pool, "/nonexistent-static");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    TestApp {
        base: format!("http://127.0.0.1:{port}/api"),
        client: Client::new(),
        _dir: dir,
    }
}

impl TestApp {
    async fn post(&self, path: &str, body: Value) -> (u16, Value) {
        let resp = self
            .client
            .post(format!("{}{}", self.base, path))
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap();
        let json = serde_json::from_str(&text).unwrap_or(json!({"raw": text}));
        (status, json)
    }
    async fn get(&self, path: &str) -> (u16, Value) {
        let resp = self.client.get(format!("{}{}", self.base, path)).send().await.unwrap();
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap();
        let json = serde_json::from_str(&text).unwrap_or(json!({"raw": text}));
        (status, json)
    }

    async fn make_tree(&self) -> Value {
        let (st, body) = self
            .post(
                "/trees",
                json!({"species": "国槐", "road": "测试路", "lat": 39.9, "lng": 116.3}),
            )
            .await;
        assert_eq!(st, 200, "create tree: {body}");
        body["tree"].clone()
    }

    async fn make_sample(&self, tree_id: i64) -> Value {
        let (st, body) = self
            .post(
                "/samples",
                json!({"tree_id": tree_id, "inspector": "张巡查", "symptom": "叶片发黄"}),
            )
            .await;
        assert_eq!(st, 200, "create sample: {body}");
        body["sample"].clone()
    }

    async fn verify_sample(&self, sample_id: i64, result_type: &str, pest: &str) {
        let (st, body) = self
            .post(
                &format!("/lab/samples/{sample_id}/result"),
                json!({"result_type": result_type, "pest_name": pest, "created_by": "李鉴定"}),
            )
            .await;
        assert_eq!(st, 200, "record result: {body}");
        let (st, body) = self
            .post(&format!("/lab/samples/{sample_id}/verify"), json!({}))
            .await;
        assert_eq!(st, 200, "verify result: {body}");
    }
}

/// 场景一：同一棵树多次送样，每份样本的鉴定结果独立保留、互不覆盖
#[tokio::test]
async fn multiple_samples_keep_separate_results() {
    let app = spawn_app().await;
    let tree = app.make_tree().await;
    let tree_id = tree["id"].as_i64().unwrap();

    // 第一份样本：确诊蚜虫
    let s1 = app.make_sample(tree_id).await;
    app.verify_sample(s1["id"].as_i64().unwrap(), "CONFIRMED", "蚜虫").await;

    // 第二份样本：无法判定
    let s2 = app.make_sample(tree_id).await;
    app.verify_sample(s2["id"].as_i64().unwrap(), "INCONCLUSIVE", "").await;

    // 第三份样本：确诊天牛
    let s3 = app.make_sample(tree_id).await;
    app.verify_sample(s3["id"].as_i64().unwrap(), "CONFIRMED", "天牛").await;

    // 工作人员视图：三份样本的结果都独立保留
    let (st, detail) = app.get(&format!("/trees/{tree_id}")).await;
    assert_eq!(st, 200);
    let samples = detail["rounds"][0]["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 3, "三次送样都应保留");
    let types: Vec<&str> = samples
        .iter()
        .map(|s| s["result"]["result_type"].as_str().unwrap())
        .collect();
    assert_eq!(types, vec!["CONFIRMED", "INCONCLUSIVE", "CONFIRMED"]);
    // 第一次的确诊结论没有被后续样本覆盖
    assert_eq!(samples[0]["result"]["pest_name"], "蚜虫");
    assert_eq!(samples[2]["result"]["pest_name"], "天牛");

    // 已核实结果不可再修改（结论不被覆盖）
    let (st, _) = app
        .post(
            &format!("/lab/samples/{}/result", s1["id"].as_i64().unwrap()),
            json!({"result_type": "NEGATIVE", "created_by": "李鉴定"}),
        )
        .await;
    assert_eq!(st, 409, "已核实结论禁止修改");

    // 公众视图同样能看到三份已核实结果
    let tree_no = tree["tree_no"].as_str().unwrap();
    let (st, public) = app.get(&format!("/public/trees/{tree_no}")).await;
    assert_eq!(st, 200);
    let pub_samples = public["rounds"][0]["samples"].as_array().unwrap();
    assert_eq!(pub_samples.len(), 3);
    assert!(pub_samples.iter().all(|s| s["verified_result"].is_object()));
}

/// 场景二：退样保留记录，可重送并关联原送样过程
#[tokio::test]
async fn rejected_sample_can_be_resubmitted_with_linkage() {
    let app = spawn_app().await;
    let tree = app.make_tree().await;
    let tree_id = tree["id"].as_i64().unwrap();

    let s1 = app.make_sample(tree_id).await;
    let s1_id = s1["id"].as_i64().unwrap();

    // 未出结论前不允许重送
    let (st, _) = app
        .post(&format!("/samples/{s1_id}/resubmit"), json!({"tree_id": tree_id, "inspector": "张巡查"}))
        .await;
    assert_eq!(st, 409, "原样本尚无退样结论时不能重送");

    // 实验室退样
    let (st, body) = app
        .post(
            &format!("/lab/samples/{s1_id}/reject"),
            json!({"reason": "样本量不足，无法镜检", "created_by": "李鉴定"}),
        )
        .await;
    assert_eq!(st, 200, "reject: {body}");
    assert_eq!(body["result"]["result_type"], "REJECTED");
    assert_eq!(body["result"]["status"], "VERIFIED");

    // 退样重送：关联原样本
    let (st, body) = app
        .post(
            &format!("/samples/{s1_id}/resubmit"),
            json!({"tree_id": tree_id, "inspector": "张巡查", "symptom": "叶片发黄（补样）"}),
        )
        .await;
    assert_eq!(st, 200, "resubmit: {body}");
    let s2 = body["sample"].clone();
    assert_eq!(s2["parent_sample_id"].as_i64().unwrap(), s1_id, "重送样本必须关联原样本");
    assert_eq!(s2["round_id"], s1["round_id"], "重送沿用原轮次");
    assert_ne!(s2["sample_no"], s1["sample_no"], "重送样本有独立编号");

    // 重送样本完成鉴定
    app.verify_sample(s2["id"].as_i64().unwrap(), "CONFIRMED", "红蜘蛛").await;

    // 工作人员视图：退样记录与重送链完整可见
    let (st, detail) = app.get(&format!("/trees/{tree_id}")).await;
    assert_eq!(st, 200);
    let samples = detail["rounds"][0]["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0]["result"]["result_type"], "REJECTED", "退样结论作为该次记录保留");
    assert_eq!(
        samples[0]["resubmitted_as"][0].as_str().unwrap(),
        s2["sample_no"].as_str().unwrap(),
        "原样本应能看到重送去向"
    );
    assert_eq!(
        samples[1]["parent_sample_no"].as_str().unwrap(),
        s1["sample_no"].as_str().unwrap(),
        "重送样本应能回溯原样本"
    );
    assert_eq!(samples[1]["result"]["pest_name"], "红蜘蛛");
}

/// 场景三：没有已核实鉴定结论的处置不能提前结案
#[tokio::test]
async fn disposal_cannot_complete_without_verified_result() {
    let app = spawn_app().await;
    let tree = app.make_tree().await;
    let tree_id = tree["id"].as_i64().unwrap();

    // 送样后立刻下处置单（尚未有任何鉴定结果）
    let s1 = app.make_sample(tree_id).await;
    let s1_id = s1["id"].as_i64().unwrap();
    let round_id = s1["round_id"].as_i64().unwrap();

    let (st, body) = app
        .post(
            "/disposals",
            json!({"round_id": round_id, "sample_id": s1_id, "assignee": "绿城养护队", "method": "预防性喷药"}),
        )
        .await;
    assert_eq!(st, 200, "create disposal: {body}");
    let disposal_id = body["disposal"]["id"].as_i64().unwrap();

    // 无鉴定结果 → 不能完成
    let (st, body) = app.post(&format!("/disposals/{disposal_id}/complete"), json!({})).await;
    assert_eq!(st, 409, "无鉴定结果不能结案: {body}");

    // 只有实验室草稿 → 仍不能完成
    let (st, _) = app
        .post(
            &format!("/lab/samples/{s1_id}/result"),
            json!({"result_type": "CONFIRMED", "pest_name": "蚜虫", "created_by": "李鉴定"}),
        )
        .await;
    assert_eq!(st, 200);
    let (st, _) = app.post(&format!("/disposals/{disposal_id}/complete"), json!({})).await;
    assert_eq!(st, 409, "草稿未核实不能结案");

    // 核实的是"无法判定" → 仍不能结案（需确诊结论）
    let s2 = app.make_sample(tree_id).await;
    app.verify_sample(s2["id"].as_i64().unwrap(), "INCONCLUSIVE", "").await;
    let (st, _) = app.post(&format!("/disposals/{disposal_id}/complete"), json!({})).await;
    assert_eq!(st, 409, "无法判定不等于确诊，不能结案");

    // 第一份样本核实确诊 → 可以完成
    let (st, _) = app.post(&format!("/lab/samples/{s1_id}/verify"), json!({})).await;
    assert_eq!(st, 200);
    let (st, body) = app.post(&format!("/disposals/{disposal_id}/complete"), json!({})).await;
    assert_eq!(st, 200, "有已核实确诊后应能结案: {body}");
    assert_eq!(body["disposal"]["status"], "COMPLETED");
    assert!(body["disposal"]["completed_at"].is_string());
}

/// 场景四：公开页只展示已核实内容，不泄露实验室草稿
#[tokio::test]
async fn public_view_hides_lab_drafts() {
    let app = spawn_app().await;
    let tree = app.make_tree().await;
    let tree_id = tree["id"].as_i64().unwrap();
    let tree_no = tree["tree_no"].as_str().unwrap().to_string();

    let s1 = app.make_sample(tree_id).await;
    let s1_id = s1["id"].as_i64().unwrap();
    // 实验室录入草稿（未核实）
    let (st, _) = app
        .post(
            &format!("/lab/samples/{s1_id}/result"),
            json!({"result_type": "CONFIRMED", "pest_name": "秘密病虫", "notes": "草稿备注", "created_by": "李鉴定"}),
        )
        .await;
    assert_eq!(st, 200);

    // 公众视图：样本可见但结果为 null，且整份响应不包含草稿内容
    let (st, public) = app.get(&format!("/public/trees/{tree_no}")).await;
    assert_eq!(st, 200);
    let sample = &public["rounds"][0]["samples"][0];
    assert!(sample["verified_result"].is_null(), "草稿结果不得公开");
    let raw = serde_json::to_string(&public).unwrap();
    assert!(!raw.contains("秘密病虫") && !raw.contains("草稿备注"), "公开响应不得泄露草稿内容");

    // 核实后公众可见
    let (st, _) = app.post(&format!("/lab/samples/{s1_id}/verify"), json!({})).await;
    assert_eq!(st, 200);
    let (_, public) = app.get(&format!("/public/trees/{tree_no}")).await;
    let sample = &public["rounds"][0]["samples"][0];
    assert_eq!(sample["verified_result"]["pest_name"], "秘密病虫");
}

/// 场景五：病虫害复发沿原树号新建一轮处理
#[tokio::test]
async fn recurrence_opens_new_round_on_same_tree() {
    let app = spawn_app().await;
    let tree = app.make_tree().await;
    let tree_id = tree["id"].as_i64().unwrap();
    let tree_no = tree["tree_no"].as_str().unwrap().to_string();

    // 第一轮：送样 → 确诊 → 处置 → 完成 → 结案
    let s1 = app.make_sample(tree_id).await;
    app.verify_sample(s1["id"].as_i64().unwrap(), "CONFIRMED", "蚜虫").await;
    let round1_id = s1["round_id"].as_i64().unwrap();
    let (st, body) = app
        .post("/disposals", json!({"round_id": round1_id, "assignee": "绿城养护队"}))
        .await;
    assert_eq!(st, 200);
    let d1 = body["disposal"]["id"].as_i64().unwrap();
    let (st, _) = app.post(&format!("/disposals/{d1}/complete"), json!({})).await;
    assert_eq!(st, 200);
    let (st, body) = app.post(&format!("/rounds/{round1_id}/close"), json!({})).await;
    assert_eq!(st, 200, "close round: {body}");
    assert_eq!(body["round"]["status"], "CLOSED");

    // 复发：再次送样 → 自动沿原树号开第二轮
    let s2 = app.make_sample(tree_id).await;
    assert_ne!(s2["round_id"], s1["round_id"], "复发应开新一轮");

    let (st, detail) = app.get(&format!("/trees/{tree_id}")).await;
    assert_eq!(st, 200);
    let rounds = detail["rounds"].as_array().unwrap();
    assert_eq!(rounds.len(), 2, "同一棵树应有两轮处理记录");
    let round_nos: Vec<i64> = rounds.iter().map(|r| r["round"]["round_no"].as_i64().unwrap()).collect();
    assert!(round_nos.contains(&1) && round_nos.contains(&2));
    assert_eq!(detail["tree"]["tree_no"], tree_no, "树号不变");

    // 第二轮进行期间不允许再开新轮
    let (st, _) = app.post(&format!("/trees/{tree_id}/rounds"), json!({})).await;
    assert_eq!(st, 409, "有进行中轮次时不能重复开轮");

    // 公众可按树号查到两轮进展
    let (st, public) = app.get(&format!("/public/trees/{tree_no}")).await;
    assert_eq!(st, 200);
    assert_eq!(public["rounds"].as_array().unwrap().len(), 2);
}

/// 附加：处置回报流程 + 结案前不能关闭轮次 + 通知 outbox 记录
#[tokio::test]
async fn disposal_reports_and_notifications() {
    let app = spawn_app().await;
    let tree = app.make_tree().await;
    let tree_id = tree["id"].as_i64().unwrap();

    let s1 = app.make_sample(tree_id).await;
    app.verify_sample(s1["id"].as_i64().unwrap(), "CONFIRMED", "天牛").await;
    let round_id = s1["round_id"].as_i64().unwrap();

    let (_, body) = app
        .post("/disposals", json!({"round_id": round_id, "assignee": "绿城养护队", "method": "注药防治"}))
        .await;
    let d1 = body["disposal"]["id"].as_i64().unwrap();

    // 回报治理情况：首次回报后转为处理中
    let (st, body) = app
        .post(
            &format!("/disposals/{d1}/reports"),
            json!({"reporter": "王养护", "content": "已钻孔注药"}),
        )
        .await;
    assert_eq!(st, 200, "report: {body}");
    let (_, list) = app.get("/disposals?assignee=绿城养护队").await;
    assert_eq!(list["disposals"][0]["status"], "IN_PROGRESS");

    // 处置单未完成时轮次不能结案
    let (st, _) = app.post(&format!("/rounds/{round_id}/close"), json!({})).await;
    assert_eq!(st, 409, "处置单未完成不能结案");

    // 完成后结案
    let (st, _) = app.post(&format!("/disposals/{d1}/complete"), json!({})).await;
    assert_eq!(st, 200);
    let (st, _) = app.post(&format!("/rounds/{round_id}/close"), json!({})).await;
    assert_eq!(st, 200);

    // 通知 outbox 记录了关键事件（送样/核实/派单/完成）
    let (_, notifs) = app.get("/notifications").await;
    let titles: Vec<&str> = notifs["notifications"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["title"].as_str().unwrap())
        .collect();
    assert!(titles.iter().any(|t| t.contains("新送鉴样本")));
    assert!(titles.iter().any(|t| t.contains("鉴定结果已核实")));
    assert!(titles.iter().any(|t| t.contains("新处置单")));
    assert!(titles.iter().any(|t| t.contains("已完成")));
}
