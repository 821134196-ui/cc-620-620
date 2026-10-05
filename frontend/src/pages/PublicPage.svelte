<script>
  // 公众页：地图 + 树号/道路搜索，只展示已核实的进展
  import MapView from '../lib/MapView.svelte'
  import { api, RESULT_TYPE_LABEL, DISPOSAL_STATUS_LABEL, ROUND_STATUS_LABEL, fmtTime } from '../lib/api.js'

  let q = ''
  let results = []
  let detail = null
  let error = ''
  let searched = false

  async function search() {
    error = ''; detail = null
    try {
      const data = await api.get(`/public/trees/search?q=${encodeURIComponent(q)}`)
      results = data.trees
      searched = true
      if (results.length === 1) pick(results[0])
    } catch (e) { error = e.message }
  }

  async function pick(tree) {
    error = ''
    try {
      detail = await api.get(`/public/trees/${tree.tree_no}`)
    } catch (e) { error = e.message; detail = null }
  }

  function onMapSelect(e) { pick(e.detail) }
</script>

<div class="card">
  <h2>🗺️ 街区树木地图（本地模拟）</h2>
  <MapView selectedTreeNo={detail?.tree?.tree_no} on:select={onMapSelect} />
</div>

<div class="card">
  <h2>🔍 查询树木进展</h2>
  <div class="row">
    <input bind:value={q} placeholder="输入树号（如 TR-0001）或道路名称" size="36"
      on:keydown={(e) => e.key === 'Enter' && search()} />
    <button on:click={search}>查询</button>
  </div>
  {#if error}<p class="err">{error}</p>{/if}
  {#if searched && results.length === 0}<p class="muted">没有找到匹配的树木</p>{/if}
  {#if results.length > 1}
    <table>
      <thead><tr><th>树号</th><th>树种</th><th>道路</th><th>位置</th><th></th></tr></thead>
      <tbody>
      {#each results as t}
        <tr>
          <td>{t.tree_no}</td><td>{t.species}</td><td>{t.road}</td><td>{t.address}</td>
          <td><button class="ghost" on:click={() => pick(t)}>查看进展</button></td>
        </tr>
      {/each}
      </tbody>
    </table>
  {/if}
</div>

{#if detail}
  <div class="card">
    <h2>🌳 {detail.tree.tree_no} · {detail.tree.species}</h2>
    <p class="muted">{detail.tree.road} · {detail.tree.address}</p>
    {#if detail.rounds.length === 0}
      <p class="muted">该树暂无送鉴/处置记录。</p>
    {/if}
    {#each detail.rounds as round}
      <div class="round">
        <h3>
          第 {round.round_no} 轮处理
          <span class="tag {round.status === 'OPEN' ? 'orange' : 'gray'}">{ROUND_STATUS_LABEL[round.status]}</span>
          <span class="muted">（{round.reason}，{fmtTime(round.opened_at)} 开始{#if round.closed_at}，{fmtTime(round.closed_at)} 结案{/if}）</span>
        </h3>

        {#each round.samples as s}
          <div class="item">
            <b>送鉴样本 {s.sample_no}</b>
            {#if s.is_resubmission}<span class="tag blue">退样后重送</span>{/if}
            <div class="muted">送样时间 {fmtTime(s.submitted_at)} · 疑似症状：{s.symptom || '—'}</div>
            {#if s.verified_result}
              <div>
                鉴定结论（已核实）：
                <span class="tag {s.verified_result.result_type === 'CONFIRMED' ? 'red' : s.verified_result.result_type === 'NEGATIVE' ? '' : 'gray'}">
                  {RESULT_TYPE_LABEL[s.verified_result.result_type]}
                </span>
                {#if s.verified_result.pest_name}<b>{s.verified_result.pest_name}</b>{/if}
                {#if s.verified_result.notes}<span class="muted"> · {s.verified_result.notes}</span>{/if}
              </div>
            {:else}
              <div class="muted">鉴定中，结果核实后公布</div>
            {/if}
          </div>
        {/each}

        {#each round.disposals as d}
          <div class="item disposal">
            <b>处置单 {d.disposal_no}</b>
            <span class="tag {d.status === 'COMPLETED' ? '' : 'orange'}">{DISPOSAL_STATUS_LABEL[d.status]}</span>
            <div class="muted">养护单位：{d.assignee} · 措施：{d.method || '—'} · 派单 {fmtTime(d.issued_at)}</div>
            {#each d.reports as r}
              <div class="report">↳ 养护回报（{fmtTime(r.reported_at)} {r.reporter}）：{r.content}</div>
            {/each}
            {#if d.completed_at}<div class="ok">✔ {fmtTime(d.completed_at)} 治理完成</div>{/if}
          </div>
        {/each}
      </div>
    {/each}
  </div>
{/if}

<style>
  .round { border-top: 1px dashed #cfd8cf; padding-top: 6px; margin-top: 10px; }
  .item { background: #f7faf7; border: 1px solid #e3ece3; border-radius: 8px; padding: 10px 12px; margin: 8px 0; }
  .item.disposal { background: #fdf9f3; border-color: #f0e6d2; }
  .report { font-size: 13px; color: #555; margin-top: 4px; }
</style>
