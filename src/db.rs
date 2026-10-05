use crate::error::AppResult;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

/// 连接池：文件库或内存库均开启外键约束
pub async fn init_pool(database_url: &str) -> AppResult<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .foreign_keys(true)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    Ok(pool)
}

/// 测试用内存库：单连接，保证所有请求看到同一个内存数据库
pub async fn memory_pool() -> AppResult<SqlitePool> {
    let options = SqliteConnectOptions::new().in_memory(true).foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    Ok(pool)
}

/// 执行 migrations/ 下的 SQL（按文件名顺序），用 _migrations 表记录
pub async fn run_migrations(pool: &SqlitePool) -> AppResult<()> {
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _migrations (
            name TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )",
    )
    .execute(pool)
    .await?;

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut files: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "sql").unwrap_or(false))
        .collect();
    files.sort();

    for path in files {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let already: i64 =
            sqlx::query("SELECT COUNT(*) FROM _migrations WHERE name = ?1")
                .bind(&name)
                .fetch_one(pool)
                .await?
                .get(0);
        if already > 0 {
            continue;
        }
        let sql = std::fs::read_to_string(&path)?;
        let mut tx = pool.begin().await?;
        sqlx::raw_sql(&sql).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO _migrations (name) VALUES (?1)")
            .bind(&name)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        tracing::info!(migration = %name, "已执行数据库迁移");
    }
    Ok(())
}

/// 按年份递增的业务编号：S-2026-0001 / W-2026-0001
pub async fn next_yearly_code(pool: &SqlitePool, table: &str, column: &str, prefix: &str) -> AppResult<String> {
    // 表名/列名为固定内部常量，不接受用户输入
    let sql = format!(
        "SELECT {column} FROM {table} WHERE {column} LIKE ?1 ORDER BY id DESC LIMIT 1"
    );
    let year = chrono::Local::now().format("%Y").to_string();
    let like = format!("{prefix}-{year}-%");
    let last: Option<String> = sqlx::query_scalar(&sql)
        .bind(like)
        .fetch_optional(pool)
        .await?;
    let seq = last
        .and_then(|c| c.rsplit('-').next().map(|s| s.parse::<i64>().unwrap_or(0)))
        .unwrap_or(0)
        + 1;
    Ok(format!("{prefix}-{year}-{seq:04}"))
}

pub async fn next_tree_code(pool: &SqlitePool) -> AppResult<String> {
    let last: Option<String> =
        sqlx::query_scalar("SELECT tree_code FROM trees ORDER BY id DESC LIMIT 1")
            .fetch_optional(pool)
            .await?;
    let seq = last
        .and_then(|c| c.strip_prefix("T-").map(|s| s.parse::<i64>().unwrap_or(0)))
        .unwrap_or(0)
        + 1;
    Ok(format!("T-{seq:04}"))
}

/// 首次启动时写入演示树木
pub async fn seed(pool: &SqlitePool) -> AppResult<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trees")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(());
    }
    let demo: Vec<(String, String, f64, f64)> = vec![
        ("林荫路".into(), "悬铃木".into(), 0.18, 0.30),
        ("林荫路".into(), "香樟".into(), 0.27, 0.46),
        ("人民大道".into(), "悬铃木".into(), 0.55, 0.24),
        ("人民大道".into(), "银杏".into(), 0.63, 0.52),
        ("建设街".into(), "国槐".into(), 0.72, 0.62),
        ("滨河路".into(), "垂柳".into(), 0.40, 0.78),
        ("建设街".into(), "白蜡".into(), 0.80, 0.40),
    ];
    for (road, species, x, y) in demo {
        let code = next_tree_code(pool).await?;
        sqlx::query(
            "INSERT INTO trees (tree_code, road_name, species, x, y, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, '')",
        )
        .bind(&code)
        .bind(&road)
        .bind(&species)
        .bind(x)
        .bind(y)
        .execute(pool)
        .await?;
    }
    tracing::info!("已写入演示树木数据");
    Ok(())
}
