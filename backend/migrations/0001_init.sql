-- 街道树木病虫害样本送鉴与处置通知系统 - 初始 schema
-- 设计要点：
--  * 每棵树固定 tree_no；每份样本固定 sample_no；处置单固定 disposal_no（均不可复用）
--  * 一棵树可有多轮处理（rounds），病虫害复发沿原树号开新轮
--  * 一份样本只有一条鉴定结果记录：先草稿(DRAFT)后核实(VERIFIED)，核实后不可改
--    退样(REJECTED)/无法判定(INCONCLUSIVE)作为该次样本的最终记录保留，不覆盖历史
--  * 退样重送通过 samples.parent_sample_id 关联原送样过程，形成完整链条
--  * 处置单完成的前置条件在应用层强制：所属轮次必须存在已核实的确诊结果

CREATE TABLE trees (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    tree_no     TEXT NOT NULL UNIQUE,          -- 固定树号，如 TR-0001
    species     TEXT NOT NULL,                 -- 树种
    road        TEXT NOT NULL,                 -- 所属道路（道路搜索）
    address     TEXT NOT NULL DEFAULT '',      -- 位置描述
    lat         REAL NOT NULL,                 -- 模拟地图坐标
    lng         REAL NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE rounds (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    tree_id     INTEGER NOT NULL REFERENCES trees(id),
    round_no    INTEGER NOT NULL,              -- 该树的第几轮处理
    status      TEXT NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN','CLOSED')),
    reason      TEXT NOT NULL DEFAULT '',      -- 开轮原因（初发/复发）
    opened_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    closed_at   TEXT,
    UNIQUE (tree_id, round_no)
);

CREATE TABLE samples (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    sample_no         TEXT NOT NULL UNIQUE,    -- 样本编号，如 SP-2026-000001
    tree_id           INTEGER NOT NULL REFERENCES trees(id),
    round_id          INTEGER NOT NULL REFERENCES rounds(id),
    parent_sample_id  INTEGER REFERENCES samples(id),  -- 退样重送时指向原样本
    inspector         TEXT NOT NULL,           -- 送检巡查员
    symptom           TEXT NOT NULL DEFAULT '',-- 疑似症状描述
    note              TEXT NOT NULL DEFAULT '',
    status            TEXT NOT NULL DEFAULT 'SUBMITTED'
                      CHECK (status IN ('SUBMITTED','RESULT_DRAFT','RESULT_VERIFIED')),
    submitted_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_samples_tree  ON samples(tree_id);
CREATE INDEX idx_samples_round ON samples(round_id);
CREATE INDEX idx_samples_parent ON samples(parent_sample_id);

-- 每份样本至多一条结果记录；核实后应用层禁止修改
CREATE TABLE sample_results (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    sample_id    INTEGER NOT NULL UNIQUE REFERENCES samples(id),
    result_type  TEXT NOT NULL CHECK (result_type IN
                 ('CONFIRMED','NEGATIVE','INCONCLUSIVE','REJECTED')),
    pest_name    TEXT NOT NULL DEFAULT '',     -- 确诊病虫名称
    confidence   TEXT NOT NULL DEFAULT '',
    notes        TEXT NOT NULL DEFAULT '',
    status       TEXT NOT NULL DEFAULT 'DRAFT' CHECK (status IN ('DRAFT','VERIFIED')),
    created_by   TEXT NOT NULL,                -- 实验室记录人
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    verified_at  TEXT
);

CREATE TABLE disposals (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    disposal_no  TEXT NOT NULL UNIQUE,         -- 处置单号，如 DP-2026-000001
    round_id     INTEGER NOT NULL REFERENCES rounds(id),
    tree_id      INTEGER NOT NULL REFERENCES trees(id),
    sample_id    INTEGER REFERENCES samples(id), -- 处置依据样本（可为空，但完成时轮次内必须有已核实确诊）
    assignee     TEXT NOT NULL,                -- 养护单位
    method       TEXT NOT NULL DEFAULT '',     -- 处置措施
    status       TEXT NOT NULL DEFAULT 'ISSUED'
                 CHECK (status IN ('ISSUED','IN_PROGRESS','COMPLETED')),
    issued_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    completed_at TEXT
);
CREATE INDEX idx_disposals_round ON disposals(round_id);
CREATE INDEX idx_disposals_tree  ON disposals(tree_id);

CREATE TABLE treatment_reports (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    disposal_id INTEGER NOT NULL REFERENCES disposals(id),
    reporter    TEXT NOT NULL,                 -- 养护回报人
    content     TEXT NOT NULL,
    reported_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX idx_reports_disposal ON treatment_reports(disposal_id);

-- 本地模拟通知 outbox：不真正发送，仅记录并可查询
CREATE TABLE notifications (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    recipient  TEXT NOT NULL,                  -- 接收方（实验室/养护单位/公众...）
    channel    TEXT NOT NULL DEFAULT 'LOCAL',  -- 模拟通道
    title      TEXT NOT NULL,
    body       TEXT NOT NULL,
    ref_type   TEXT NOT NULL DEFAULT '',
    ref_id     INTEGER,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- 编号发号器（事务内自增，保证唯一）
CREATE TABLE counters (
    key   TEXT PRIMARY KEY,
    value INTEGER NOT NULL
);

-- 模拟地图：道路线数据（前端按此绘制街区底图）
CREATE TABLE map_roads (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    name    TEXT NOT NULL UNIQUE,
    path    TEXT NOT NULL                      -- JSON 数组 [[lng,lat],...]
);
