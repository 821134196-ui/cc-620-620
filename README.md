# 街道树木病虫害样本送鉴与处置通知系统

巡查员发现疑似病虫害后，把**送样 → 实验室鉴定 → 养护处置**关联到具体的固定树号树木；居民可查询
**核实后**的公开进展。技术栈：Rust + Axum + SQLx + SQLite（后端）、Svelte 5 + Vite（前端），
地图与短信/公告通知均在本地模拟。

## 一条命令启动

```bash
./start.sh
```

脚本会自动：安装前端依赖 → 构建 Svelte → `cargo run --release` 启动 Axum
（首次启动自动建表迁移、写入 7 棵演示树木）。打开 <http://127.0.0.1:8080> 即可，
页面内可切换「居民公开查询」与「工作人员台」。

可用环境变量：`BIND_ADDR`（默认 127.0.0.1:8080）、`DATABASE_URL`（默认 sqlite://data/treecare.db?mode=rwc）。

前端独立开发（热更新，/api 代理到 8080）：

```bash
cargo run            # 终端 1
cd web && npm run dev# 终端 2，打开 http://127.0.0.1:5173
```

运行测试：`cargo test`

## 业务规则

| 规则 | 实现 |
| --- | --- |
| 每棵树固定树号 | `trees.tree_code` 唯一（T-0001…），复发不换号 |
| 每份样本单独编号 | `samples.sample_code` 唯一（S-2026-0001…） |
| 地图与道路搜索 | 模拟 SVG 地图按归一化坐标布点；列表支持道路/树号关键字 |
| 同树多次送样、结论互不覆盖 | 每次送样独立成行；鉴定结果核实后**锁定**，修改返回 409 |
| 退样 / 无法判定仍保留 | 退样样本保留 `returned` 状态与原因；`undetermined` 结论永久归档 |
| 退样重送关联原过程 | 新样本 `parent_sample_id` 指向原样本，沿用同一处理轮次 |
| 复发沿原树号新开一轮 | 普通新送样自动取 `MAX(round)+1`，与父链重送相区分 |
| 无鉴定结论不能结案 | 签发处置单要求结论已核实且确诊；结案还要求已接单且已有治理回报 |
| 公开页不含草稿 | 公开接口只返回 `is_draft=0 AND verified_at IS NOT NULL` 的记录；通知也分公开/内部 |
| 工作人员看历次记录 | `/api/staff/trees/{id}/history` 返回每棵树全部样本、草稿、处置单与回报 |
| 通知本地模拟 | 写入 `notifications` 表：内部 `web`，公开 `sms`（模拟短信）/ `public_board`（公告板） |

### 处置状态机

```
送样 submitted ──退样──▶ returned ──退样重送(parent_sample_id)──▶ 新 submitted 样本
        │登记结果(草稿)
        ▼
determined + lab_results(is_draft=1) ──核实──▶ is_draft=0（锁定、公开）
        │确诊且核实
        ▼
处置单 issued ──养护单位首次回报──▶ in_progress ──结案守卫通过──▶ completed
```

## API 一览

公开（只读，仅核实内容）：

- `GET /api/public/trees?q=` 道路/树号搜索 + 公开状态
- `GET /api/public/trees/{id}` 已核实时间线（含退样重送的原样本号、处置进展）
- `GET /api/public/map` 地图点位
- `GET /api/public/notifications` 公开通知公告板

工作人员：

- `GET|POST /api/staff/trees`，`GET /api/staff/trees/{id}/history`，`GET /api/staff/map`
- `POST /api/staff/samples`（body 可带 `parent_sample_id` 表示退样重送）
- `POST /api/staff/samples/{id}/return`
- `PUT /api/staff/samples/{id}/result`（草稿，可反复改）
- `POST /api/staff/samples/{id}/verify`（核实锁定并触发公开通知）
- `POST /api/staff/treatments`、`POST /api/staff/treatments/{id}/reports`、
  `POST /api/staff/treatments/{id}/complete`
- `GET /api/staff/notifications`

## 验证要点（`cargo test`，tests/api.rs）

1. `multiple_samples_keep_separate_results` — 同树三份样本（确诊/健康/无法判定）分别保留，
   覆盖已核实结论被 409 拒绝；公开时间线三条俱全。
2. `returned_sample_can_be_resent_with_link` — 退样记录保留；非退样样本不能作重送来源；
   重送样本沿用原轮次且带 `parent_sample_code`。
3. `cannot_close_treatment_without_verified_diagnosis` — 无结果不能签单、草稿不能签单、
   草稿不公开、未回报不能结案、结案后不能重复结案/再回报。
4. 另含「无法判定公开但不发处置单」「复发沿原树号新开第二轮」「道路/树号搜索与地图」三个补充场景。
