# 街道树木病虫害样本送鉴与处置通知系统

巡查员发现疑似病虫害后送样鉴定，实验室记录并核实鉴定结果，养护单位接收处置单并回报治理情况；居民可在公开页查询**已核实**的进展。全流程关联到固定树号，支持地图与道路搜索。

## 一条命令启动

```bash
./start.sh            # 调试构建；./start.sh --release 用优化构建
```

启动后访问 <http://localhost:8080>。首次启动自动建库（`backend/treecare.db`）、执行迁移并写入演示数据（10 棵行道树 + 一条已核实流程）。

前端开发模式：`cd frontend && npm run dev`（代理 `/api` 到 8080）。

## 运行测试

```bash
cd backend && cargo test
```

集成测试覆盖需求验证点：

| 测试 | 验证点 |
|---|---|
| `multiple_samples_keep_separate_results` | 同一棵树多份样本各自保留鉴定结果，已核实结论不可被覆盖 |
| `rejected_sample_can_be_resubmitted_with_linkage` | 退样记录保留，可重送且关联原送样过程（链式可查） |
| `disposal_cannot_complete_without_verified_result` | 无鉴定结果 / 只有草稿 / 仅"无法判定"时处置单都不能结案；有已核实确诊后才能完成 |
| `public_view_hides_lab_drafts` | 公开页只展示已核实内容，不泄露实验室草稿 |
| `recurrence_opens_new_round_on_same_tree` | 病虫害复发沿原树号自动开新一轮，历史轮次完整保留 |
| `disposal_reports_and_notifications` | 养护回报流转、结案约束、本地模拟通知 outbox |

## 角色与页面

| 页面 | 角色 | 能力 |
|---|---|---|
| `#/` 公众查询 | 居民 | 地图找树、树号/道路搜索、查看**已核实**的送鉴与处置进展 |
| `#/staff` 工作人员 | 巡查/管理 | 树木台账、完整历史（含实验室草稿）、送样、退样重送、开轮/结案、下达处置单 |
| `#/lab` 实验室 | 鉴定人 | 录入鉴定草稿、核实归档、退样 |
| `#/maintenance` 养护单位 | 养护队 | 接收处置单、回报治理情况、标记完成 |
| `#/notify` 通知记录 | — | 本地模拟通知 outbox（不实际外发） |

## 核心业务规则

- **固定编号**：树号 `TR-0001…`、样本号 `SP-YYYY-NNNNNN`、处置单号 `DP-YYYY-NNNNNN`，事务内发号，唯一不复用。
- **多次送样**：同一棵树可任意多次送样；每份样本一条结果记录（草稿→核实），核实后只读。退样（REJECTED）与无法判定（INCONCLUSIVE）作为该次样本的最终记录保留，不覆盖任何历史结论。
- **退样重送**：仅当原样本有已核实的退样/无法判定结论时可重送；新样本通过 `parent_sample_id` 关联原样本并沿用原轮次，形成完整链条。
- **结案约束**：处置单只有在所属轮次存在**已核实的确诊（CONFIRMED）结果**时才能标记完成；轮次结案要求所有处置单已完成。
- **复发处理**：上一轮结案后再次送样，自动沿原树号开启新一轮（round_no 递增），历史轮次不丢。
- **公开边界**：`/api/public/*` 只输出 `VERIFIED` 结果；实验室草稿（DRAFT）仅工作人员/实验室接口可见。

## 技术栈与结构

- 后端：Rust + Axum 0.8 + SQLx（SQLite），迁移在 `backend/migrations/`
- 前端：Svelte 5 + Vite，构建产物由 Axum 静态托管（单端口交付）
- 地图与通知均为本地模拟：地图是 `/api/map` 返回道路线 + 树点的 SVG 街区图；通知写入 `notifications` outbox 表

```
backend/
  src/main.rs        入口（建库、迁移、种子、起服务）
  src/routes/        public / staff / lab / maintenance / map / notify
  migrations/0001_init.sql
  tests/api.rs       端到端集成测试
frontend/
  src/pages/         PublicPage / StaffPage / LabPage / MaintenancePage / NotifyPage
  src/lib/MapView.svelte  本地模拟地图
start.sh             一条命令启动
```

## 主要 API

```
GET  /api/public/trees/search?q=        公众搜索（树号/道路/树种）
GET  /api/public/trees/{tree_no}        公众时间线（仅已核实）
GET  /api/map                           模拟地图（道路线+树点）
GET  /api/trees  POST /api/trees        工作人员：树木台账
GET  /api/trees/{id}                    完整档案（含草稿、重送链、回报）
POST /api/trees/{id}/rounds             开新一轮（复发）
POST /api/rounds/{id}/close             轮次结案
POST /api/samples                       送样（无 OPEN 轮次自动开轮）
POST /api/samples/{id}/resubmit         退样重送（关联原样本）
GET  /api/lab/samples                   实验室工作台
POST /api/lab/samples/{id}/result       录入/修改草稿
POST /api/lab/samples/{id}/verify       核实归档
POST /api/lab/samples/{id}/reject       退样
GET  /api/disposals  POST /api/disposals  处置单
POST /api/disposals/{id}/reports        养护回报
POST /api/disposals/{id}/complete       标记完成（需已核实确诊）
GET  /api/notifications                 模拟通知 outbox
```
