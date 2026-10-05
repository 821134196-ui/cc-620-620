use treecare::http::start;

#[tokio::main]
async fn main() -> treecare::AppResult<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=warn".into()),
        )
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data/treecare.db?mode=rwc".to_string());
    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    // 文件库的目录确保存在
    if let Some(path) = database_url.strip_prefix("sqlite://") {
        let file = path.split('?').next().unwrap_or(path);
        if file != ":memory:" {
            if let Some(parent) = std::path::Path::new(file).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).ok();
                }
            }
        }
    }

    start(&database_url, &addr).await
}
