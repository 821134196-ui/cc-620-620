use sqlx::SqlitePool;

/// 首次启动时写入演示数据：道路底图 + 行道树 + 一条已核实的完整流程
pub async fn seed_if_empty(pool: &SqlitePool) {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM trees")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    if count > 0 {
        return;
    }
    tracing::info!("seeding demo data...");

    // 模拟街区底图：三条道路（经纬度假坐标）
    let roads = [
        ("人民路", vec![(116.320, 39.900), (116.330, 39.900), (116.340, 39.900), (116.350, 39.900)]),
        ("解放大道", vec![(116.320, 39.905), (116.330, 39.906), (116.340, 39.907), (116.350, 39.908)]),
        ("建设街", vec![(116.328, 39.896), (116.328, 39.902), (116.328, 39.908), (116.328, 39.912)]),
    ];
    for (name, path) in roads {
        let path_json = serde_json::to_string(&path).unwrap();
        sqlx::query("INSERT INTO map_roads(name, path) VALUES (?, ?)")
            .bind(name)
            .bind(path_json)
            .execute(pool)
            .await
            .expect("seed roads");
    }

    // 行道树
    let trees = [
        ("TR-0001", "国槐", "人民路", "人民路 12 号门前", 39.9002, 116.3220),
        ("TR-0002", "国槐", "人民路", "人民路 18 号门前", 39.9002, 116.3260),
        ("TR-0003", "银杏", "人民路", "人民路 26 号门前", 39.9003, 116.3310),
        ("TR-0004", "银杏", "人民路", "人民路 34 号门前", 39.9003, 116.3360),
        ("TR-0005", "悬铃木", "解放大道", "解放大道 5 号", 39.9052, 116.3230),
        ("TR-0006", "悬铃木", "解放大道", "解放大道 11 号", 39.9058, 116.3300),
        ("TR-0007", "香樟", "解放大道", "解放大道 19 号", 39.9065, 116.3380),
        ("TR-0008", "香樟", "建设街", "建设街 3 号", 39.8975, 116.3281),
        ("TR-0009", "白蜡", "建设街", "建设街 9 号", 39.9010, 116.3281),
        ("TR-0010", "白蜡", "建设街", "建设街 15 号", 39.9060, 116.3281),
    ];
    for (no, species, road, addr, lat, lng) in trees {
        sqlx::query("INSERT INTO trees(tree_no, species, road, address, lat, lng) VALUES (?,?,?,?,?,?)")
            .bind(no)
            .bind(species)
            .bind(road)
            .bind(addr)
            .bind(lat)
            .bind(lng)
            .execute(pool)
            .await
            .expect("seed trees");
    }
    sqlx::query("INSERT INTO counters(key, value) VALUES ('tree', 10) ON CONFLICT(key) DO UPDATE SET value = 10")
        .execute(pool)
        .await
        .expect("seed counter");

    // 演示流程：TR-0001 一次已核实确诊 + 处置进行中
    let mut tx = pool.begin().await.expect("seed tx");
    let round_id = sqlx::query("INSERT INTO rounds(tree_id, round_no, reason) VALUES (1, 1, '首次发现')")
        .execute(&mut *tx)
        .await
        .unwrap()
        .last_insert_rowid();
    let sample_no = crate::db::next_number(&mut tx, "sample", "SP").await.unwrap();
    let sample_id = sqlx::query(
        "INSERT INTO samples(sample_no, tree_id, round_id, inspector, symptom)
         VALUES (?, 1, ?, '张巡查', '叶片出现褐色斑点并有蚜虫聚集')",
    )
    .bind(&sample_no)
    .bind(round_id)
    .execute(&mut *tx)
    .await
    .unwrap()
    .last_insert_rowid();
    sqlx::query(
        "INSERT INTO sample_results(sample_id, result_type, pest_name, confidence, notes, status, created_by, verified_at)
         VALUES (?, 'CONFIRMED', '蚜虫（槐蚜）', '高', '镜检见大量槐蚜若虫', 'VERIFIED', '李鉴定',
                 strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
    )
    .bind(sample_id)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("UPDATE samples SET status = 'RESULT_VERIFIED' WHERE id = ?")
        .bind(sample_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    let disposal_no = crate::db::next_number(&mut tx, "disposal", "DP").await.unwrap();
    let disposal_id = sqlx::query(
        "INSERT INTO disposals(disposal_no, round_id, tree_id, sample_id, assignee, method, status)
         VALUES (?, ?, 1, ?, '绿城养护队', '喷施吡虫啉 1000 倍液，一周后复查', 'IN_PROGRESS')",
    )
    .bind(&disposal_no)
    .bind(round_id)
    .bind(sample_id)
    .execute(&mut *tx)
    .await
    .unwrap()
    .last_insert_rowid();
    sqlx::query(
        "INSERT INTO treatment_reports(disposal_id, reporter, content)
         VALUES (?, '王养护', '已完成首次喷药，蚜虫数量明显减少')",
    )
    .bind(disposal_id)
    .execute(&mut *tx)
    .await
    .unwrap();
    crate::db::notify(&mut tx, "public", &format!("样本 {sample_no} 鉴定结果已核实"), "树木 TR-0001 鉴定结论：确诊 蚜虫（槐蚜）", "sample", sample_id)
        .await
        .unwrap();
    crate::db::notify(&mut tx, "绿城养护队", &format!("新处置单 {disposal_no}"), "树木 TR-0001 第 1 轮处置任务，请及时处理", "disposal", disposal_id)
        .await
        .unwrap();
    tx.commit().await.expect("seed commit");
    tracing::info!("seed done");
}
