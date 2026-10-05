-- 街道树木病虫害样本送鉴与处置通知系统 · 初始结构

-- 每棵树一个固定树号，坐标供本地模拟地图使用（x/y 为 0..1 的归一化坐标）
CREATE TABLE trees (
    id          INTEGER PRIMARY KEY,
    tree_code   TEXT    NOT NULL UNIQUE,            -- 固定树号 T-0001
    road_name   TEXT    NOT NULL,                    -- 所在道路（道路搜索）
    species     TEXT    NOT NULL,                    -- 树种
    x           REAL    NOT NULL,                    -- 模拟地图横坐标
    y           REAL    NOT NULL,                    -- 模拟地图纵坐标
    notes       TEXT    NOT NULL DEFAULT '',
    created_at  TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 送鉴样本：每份样本单独编号；同一棵树可多次送样；round 为处理轮次（复发沿原树号新开一轮）；
-- parent_sample_id 把“退样重送”关联回原送样过程
CREATE TABLE samples (
    id                INTEGER PRIMARY KEY,
    sample_code       TEXT    NOT NULL UNIQUE,      -- 样本编号 S-2026-0001
    tree_id           INTEGER NOT NULL REFERENCES trees(id),
    round             INTEGER NOT NULL,             -- 该树第几轮处理
    parent_sample_id  INTEGER REFERENCES samples(id),-- 重送时指向原样本
    status            TEXT    NOT NULL
        CHECK (status IN ('submitted','returned','testing','determined')),
    inspector         TEXT    NOT NULL,             -- 巡查员
    symptom           TEXT    NOT NULL,             -- 疑似病虫害症状
    return_reason     TEXT    NOT NULL DEFAULT '',
    submitted_at      TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    returned_at       TEXT
);
CREATE INDEX idx_samples_tree ON samples(tree_id);

-- 鉴定结果：一样本至多一条；is_draft=1 为实验室草稿（不对居民公开），
-- verified_at 非空表示已经核实。结论类型含“无法判定”，仍作为该次记录永久保留
CREATE TABLE lab_results (
    id           INTEGER PRIMARY KEY,
    sample_id    INTEGER NOT NULL UNIQUE REFERENCES samples(id),
    conclusion   TEXT    NOT NULL
        CHECK (conclusion IN ('pest_confirmed','healthy','undetermined')),
    diagnosis    TEXT    NOT NULL DEFAULT '',       -- 病虫害名称
    severity     TEXT    NOT NULL DEFAULT '',       -- 轻微/中等/严重
    note         TEXT    NOT NULL DEFAULT '',
    is_draft     INTEGER NOT NULL DEFAULT 1,
    recorded_by  TEXT    NOT NULL DEFAULT '',
    verified_by  TEXT    NOT NULL DEFAULT '',
    created_at   TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    verified_at  TEXT
);

-- 处置单：仅在“确诊且核实”后签发；没有鉴定结论不允许进入完成状态
CREATE TABLE treatments (
    id             INTEGER PRIMARY KEY,
    treatment_code TEXT    NOT NULL UNIQUE,         -- 处置单编号 W-2026-0001
    tree_id        INTEGER NOT NULL REFERENCES trees(id),
    sample_id      INTEGER NOT NULL UNIQUE REFERENCES samples(id),
    round          INTEGER NOT NULL,
    assigned_unit  TEXT    NOT NULL,                -- 养护单位
    status         TEXT    NOT NULL
        CHECK (status IN ('issued','in_progress','completed')),
    report_summary TEXT    NOT NULL DEFAULT '',
    issued_at      TEXT    NOT NULL DEFAULT (datetime('now', 'localtime')),
    accepted_at    TEXT,
    completed_at   TEXT
);
CREATE INDEX idx_treatments_tree ON treatments(tree_id);

-- 养护单位治理回报（可多次回报，全部保留）
CREATE TABLE treatment_reports (
    id           INTEGER PRIMARY KEY,
    treatment_id INTEGER NOT NULL REFERENCES treatments(id),
    content      TEXT    NOT NULL,
    reporter     TEXT    NOT NULL,
    created_at   TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 本地模拟通知（短信/工作端/公开公告）。public_visible=1 仅在核实后产生的条目上置位
CREATE TABLE notifications (
    id              INTEGER PRIMARY KEY,
    channel         TEXT    NOT NULL,               -- sms / web / public_board
    target          TEXT    NOT NULL,
    title           TEXT    NOT NULL,
    body            TEXT    NOT NULL,
    related_tree_id INTEGER REFERENCES trees(id),
    public_visible  INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now', 'localtime'))
);
