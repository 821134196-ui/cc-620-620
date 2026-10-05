<script>
  import { get } from '../lib/api.js';
  import { publicStatusBadge } from '../lib/format.js';
  import MapView from '../lib/MapView.svelte';

  let keyword = $state('');
  let trees = $state([]);
  let detail = $state(null);
  let notices = $state([]);
  let loading = $state(false);

  async function loadList(q = '') {
    loading = true;
    trees = await get(`/api/public/trees?q=${encodeURIComponent(q)}`);
    loading = false;
  }
  async function openDetail(id) {
    detail = await get(`/api/public/trees/${id}`);
  }
  async function loadNotices() {
    notices = await get('/api/public/notifications');
  }

  loadList();
  loadNotices();

  let timer;
  function onSearch(e) {
    keyword = e.target.value;
    clearTimeout(timer);
    timer = setTimeout(() => loadList(keyword), 200);
  }
</script>

<div class="grid two">
  <div class="card">
    <h2>树木分布与核实进展（居民公开页）</h2>
    <div class="row" style="margin-bottom:10px">
      <input
        type="search"
        placeholder="按道路或固定树号搜索，如：林荫路 / T-0001"
        value={keyword}
        oninput={onSearch}
        style="flex:1;min-width:240px"
      />
    </div>
    <MapView trees={trees} selectedId={detail?.tree.id ?? null} onSelect={openDetail} />
    <ul class="clean" style="margin-top:10px;max-height:220px;overflow:auto">
      {#each trees as t (t.tree.id)}
        <li class="tree-item" class:selected={detail?.tree.id === t.tree.id} onclick={() => openDetail(t.tree.id)}>
          <div class="row">
            <span class="code">{t.tree.tree_code}</span>
            <span class="badge {publicStatusBadge(t.public_status)}">{t.public_status_text}</span>
          </div>
          <div class="meta">{t.tree.road_name} · {t.tree.species}</div>
        </li>
      {/each}
      {#if trees.length === 0 && !loading}
        <li class="muted">未找到匹配树木</li>
      {/if}
    </ul>
  </div>

  <div class="card">
    <h2>{detail ? `${detail.tree.tree_code} · 已核实进展` : '选择树木查看核实后的进展'}</h2>
    {#if !detail}
      <p class="muted">点击地图点位或左侧树号，可查看该树已经核实的鉴定结论与治理进展。<br />
        未经核实的实验室草稿不会出现在本页。</p>
    {:else}
      <div class="kv"><b>固定树号：</b>{detail.tree.tree_code}</div>
      <div class="kv"><b>道路：</b>{detail.tree.road_name}</div>
      <div class="kv"><b>树种：</b>{detail.tree.species}</div>
      <div class="row" style="margin:8px 0">
        <span class="badge {publicStatusBadge(detail.public_status)}">{detail.public_status_text}</span>
      </div>

      {#if detail.timeline.length === 0}
        <p class="muted">该树暂无已核实的公开记录。巡查送样、实验室草稿阶段的信息不对外展示。</p>
      {:else}
        <div class="timeline">
          {#each detail.timeline as item}
            <div class="t-item">
              <div class="time">第 {item.round} 轮 · 样本 {item.sample_code}
                {#if item.parent_sample_code}<span class="muted">（退样重送，原样本 {item.parent_sample_code}）</span>{/if}
              </div>
              <div><b>鉴定结论（{item.at} 核实）：</b>
                {item.conclusion === 'pest_confirmed'
                  ? `确诊病虫害：${item.diagnosis}（${item.severity || '—'}）`
                  : item.conclusion === 'healthy' ? '本次鉴定未见病虫害' : '样本无法判定，该次记录留存'}</div>
              {#if item.note}<div class="muted">备注：{item.note}</div>{/if}
              {#if item.treatment_code}
                <div style="margin-top:4px">
                  <b>处置：</b>{item.treatment_code} · 养护单位 {item.assigned_unit} ·
                  {item.treatment_status === 'completed' ? '治理完成' : item.treatment_status === 'in_progress' ? '治理进行中' : '待接单'}
                  {#if item.completed_at}<span class="muted">（{item.completed_at} 结案）</span>{/if}
                </div>
                {#if item.report_summary}<div class="muted">最新回报：{item.report_summary}</div>{/if}
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</div>

<div class="card" style="margin-top:16px">
  <h2>公开通知公告板（本地模拟短信/公示）</h2>
  {#if notices.length === 0}
    <p class="muted">暂无公开通知。鉴定结论核实、治理进展与结案时会在此公示。</p>
  {:else}
    {#each notices as n (n.id)}
      <div class="notice">
        <div class="row">
          <span class="nt">{n.title}</span>
          <span class="muted">[{n.channel === 'sms' ? '模拟短信' : '公告板'} → {n.target}]</span>
        </div>
        <div class="nb">{n.body}</div>
        <div class="nt-time">{n.created_at}</div>
      </div>
    {/each}
  {/if}
</div>
