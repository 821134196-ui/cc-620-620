use std::net::SocketAddr;
use treecare::{db, routes, seed};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "treecare=info".into()),
        )
        .init();

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://./treecare.db?mode=rwc".to_string());
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "../frontend/dist".to_string());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let pool = db::connect(&db_url).await;
    db::migrate(&pool).await;
    seed::seed_if_empty(&pool).await;

    let app = routes::build_router(pool, &static_dir);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("listening on http://{addr}  (static: {static_dir})");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, app).await.expect("serve");
}
