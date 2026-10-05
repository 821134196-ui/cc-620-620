pub mod lab;
pub mod maintenance;
pub mod map;
pub mod notify;
pub mod public;
pub mod staff;

use axum::{routing::{get, post}, Router};
use tower_http::{cors::CorsLayer, services::ServeDir};

use crate::db::Db;

pub fn build_router(db: Db, static_dir: &str) -> Router {
    let api = Router::new()
        // 公众
        .route("/public/trees/search", get(public::search_trees))
        .route("/public/trees/{tree_no}", get(public::public_tree))
        // 模拟地图
        .route("/map", get(map::map_data))
        // 工作人员
        .route("/trees", get(staff::list_trees).post(staff::create_tree))
        .route("/trees/{id}", get(staff::tree_detail))
        .route("/trees/{id}/rounds", post(staff::open_round))
        .route("/rounds/{id}/close", post(staff::close_round))
        .route("/samples", post(staff::create_sample))
        .route("/samples/{id}/resubmit", post(staff::resubmit_sample))
        // 实验室
        .route("/lab/samples", get(lab::list_samples))
        .route("/lab/samples/{id}/result", post(lab::record_result))
        .route("/lab/samples/{id}/verify", post(lab::verify_result))
        .route("/lab/samples/{id}/reject", post(lab::reject_sample))
        // 养护处置
        .route("/disposals", get(maintenance::list_disposals).post(maintenance::create_disposal))
        .route("/disposals/{id}/reports", post(maintenance::add_report))
        .route("/disposals/{id}/complete", post(maintenance::complete_disposal))
        // 模拟通知
        .route("/notifications", get(notify::list_notifications))
        .with_state(db);

    Router::new()
        .nest("/api", api)
        .fallback_service(ServeDir::new(static_dir.to_string()))
        .layer(CorsLayer::permissive())
}
