use sqlx::{sqlite::SqlitePoolOptions, Row, Sqlite, SqlitePool, Transaction};

pub type Db = SqlitePool;

pub async fn connect(url: &str) -> Db {
    SqlitePoolOptions::new()
        .max_connections(8)
        .connect(url)
        .await
        .expect("connect sqlite")
}

pub async fn migrate(pool: &Db) {
    sqlx::migrate!("./migrations").run(pool).await.expect("run migrations");
}

/// 事务内发号：counters 表自增，格式 PREFIX-YYYY-NNNNNN
pub async fn next_number(
    tx: &mut Transaction<'_, Sqlite>,
    key: &str,
    prefix: &str,
) -> Result<String, sqlx::Error> {
    let year = chrono::Utc::now().format("%Y").to_string();
    let full_key = format!("{key}:{year}");
    sqlx::query("INSERT INTO counters(key, value) VALUES (?, 0) ON CONFLICT(key) DO NOTHING")
        .bind(&full_key)
        .execute(&mut **tx)
        .await?;
    let row = sqlx::query("UPDATE counters SET value = value + 1 WHERE key = ? RETURNING value")
        .bind(&full_key)
        .fetch_one(&mut **tx)
        .await?;
    let v: i64 = row.get("value");
    Ok(format!("{prefix}-{year}-{v:06}"))
}

/// 固定树号 TR-0001 ...（不随年份重置）
pub async fn next_tree_no(tx: &mut Transaction<'_, Sqlite>) -> Result<String, sqlx::Error> {
    sqlx::query("INSERT INTO counters(key, value) VALUES ('tree', 0) ON CONFLICT(key) DO NOTHING")
        .execute(&mut **tx)
        .await?;
    let row = sqlx::query("UPDATE counters SET value = value + 1 WHERE key = 'tree' RETURNING value")
        .fetch_one(&mut **tx)
        .await?;
    let v: i64 = row.get("value");
    Ok(format!("TR-{v:04}"))
}

/// 写入本地模拟通知 outbox
pub async fn notify(
    tx: &mut Transaction<'_, Sqlite>,
    recipient: &str,
    title: &str,
    body: &str,
    ref_type: &str,
    ref_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO notifications(recipient, channel, title, body, ref_type, ref_id)
         VALUES (?, 'LOCAL', ?, ?, ?, ?)",
    )
    .bind(recipient)
    .bind(title)
    .bind(body)
    .bind(ref_type)
    .bind(ref_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
