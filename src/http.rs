use crate::error::{AppError, AppResult};
use crate::models::*;
use crate::service;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, Router};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::sync::Arc;
use tower_http::services::ServeDir;

#[derive(Clone)]
pub struct AppState {
    pub pool: Arc<SqlitePool>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

// ---------- 居民公开接口 ----------

async fn public_search(
    State(st): State<AppState>,
    Query(p): Query<SearchQuery>,
) -> AppResult<Json<Vec<TreePublic>>> {
    Ok(Json(service::public_search(&st.pool, p.q.as_deref()).await?))
}

async fn public_map(State(st): State<AppState>) -> AppResult<Json<Vec<TreePublic>>> {
    Ok(Json(service::public_map(&st.pool).await?))
}

async fn public_detail(State(st): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<TreePublicDetail>> {
    Ok(Json(service::public_detail(&st.pool, id).await?))
}

async fn public_notifications(State(st): State<AppState>) -> AppResult<Json<Vec<Notification>>> {
    Ok(Json(service::list_notifications(&st.pool, true).await?))
}

// ---------- 工作人员接口 ----------

async fn staff_trees(
    State(st): State<AppState>,
    Query(p): Query<SearchQuery>,
) -> AppResult<Json<Vec<Tree>>> {
    Ok(Json(service::list_trees(&st.pool, p.q.as_deref()).await?))
}

async fn staff_create_tree(
    State(st): State<AppState>,
    Json(input): Json<CreateTree>,
) -> AppResult<(StatusCode, Json<Tree>)> {
    let t = service::create_tree(&st.pool, input).await?;
    Ok((StatusCode::CREATED, Json(t)))
}

async fn staff_tree_history(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<TreeHistory>> {
    Ok(Json(service::tree_history(&st.pool, id).await?))
}

async fn staff_submit_sample(
    State(st): State<AppState>,
    Json(input): Json<SubmitSample>,
) -> AppResult<(StatusCode, Json<Sample>)> {
    let s = service::submit_sample(&st.pool, input).await?;
    Ok((StatusCode::CREATED, Json(s)))
}

async fn staff_return_sample(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<ReturnSample>,
) -> AppResult<Json<Sample>> {
    Ok(Json(service::return_sample(&st.pool, id, input).await?))
}

async fn staff_record_result(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<RecordResult>,
) -> AppResult<Json<LabResult>> {
    Ok(Json(service::record_result(&st.pool, id, input).await?))
}

async fn staff_verify_result(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<VerifyResult>,
) -> AppResult<Json<LabResult>> {
    Ok(Json(service::verify_result(&st.pool, id, input).await?))
}

async fn staff_issue_treatment(
    State(st): State<AppState>,
    Json(input): Json<IssueTreatment>,
) -> AppResult<(StatusCode, Json<Treatment>)> {
    let t = service::issue_treatment(&st.pool, input).await?;
    Ok((StatusCode::CREATED, Json(t)))
}

async fn staff_add_report(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<AddReport>,
) -> AppResult<Json<Treatment>> {
    Ok(Json(service::add_report(&st.pool, id, input).await?))
}

async fn staff_complete_treatment(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<Treatment>> {
    Ok(Json(service::complete_treatment(&st.pool, id).await?))
}

async fn staff_map(State(st): State<AppState>) -> AppResult<Json<Vec<serde_json::Value>>> {
    Ok(Json(service::staff_map(&st.pool).await?))
}

async fn staff_notifications(State(st): State<AppState>) -> AppResult<Json<Vec<Notification>>> {
    Ok(Json(service::list_notifications(&st.pool, false).await?))
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true }))
}

pub fn build_router(pool: SqlitePool) -> Router {
    let state = AppState { pool: Arc::new(pool) };

    let api = Router::new()
        // 居民公开
        .route("/public/trees", axum::routing::get(public_search))
        .route("/public/trees/{id}", axum::routing::get(public_detail))
        .route("/public/map", axum::routing::get(public_map))
        .route("/public/notifications", axum::routing::get(public_notifications))
        // 工作人员
        .route("/staff/trees", axum::routing::get(staff_trees).post(staff_create_tree))
        .route("/staff/trees/{id}/history", axum::routing::get(staff_tree_history))
        .route("/staff/map", axum::routing::get(staff_map))
        .route("/staff/samples", axum::routing::post(staff_submit_sample))
        .route("/staff/samples/{id}/return", axum::routing::post(staff_return_sample))
        .route("/staff/samples/{id}/result", axum::routing::put(staff_record_result))
        .route("/staff/samples/{id}/verify", axum::routing::post(staff_verify_result))
        .route("/staff/treatments", axum::routing::post(staff_issue_treatment))
        .route("/staff/treatments/{id}/reports", axum::routing::post(staff_add_report))
        .route("/staff/treatments/{id}/complete", axum::routing::post(staff_complete_treatment))
        .route("/staff/notifications", axum::routing::get(staff_notifications))
        .with_state(state);

    Router::new()
        .route("/api/health", axum::routing::get(health))
        .nest("/api", api)
        // 前端静态资源（生产构建产物）
        .fallback_service(ServeDir::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("web/dist"),
        ))
}

/// 便于测试：未匹配静态资源时的兜底（无前端构建时也不影响 API）
pub async fn start(database_url: &str, addr: &str) -> AppResult<()> {
    let pool = crate::db::init_pool(database_url).await?;
    crate::db::run_migrations(&pool).await?;
    crate::db::seed(&pool).await?;

    let app = build_router(pool);
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        AppError::Other(anyhow::anyhow!("无法监听 {addr}: {e}"))
    })?;
    tracing::info!("服务已启动：http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
