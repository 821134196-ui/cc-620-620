<script>
  // 养护单位：接收处置单、回报治理情况、标记完成
  import { onMount } from 'svelte'
  import { api, DISPOSAL_STATUS_LABEL, fmtTime } from '../lib/api.js'

  let disposals = []
  let assignee = ''
  let error = ''
  let notice = ''
  let reporting = null
  let reportForm = { reporter: '', content: '' }

  onMount(load)

  async function load() {
    const qs = assignee ? `?assignee=${encodeURIComponent(assignee)}` : ''
    const data = await api.get(`/disposals${qs}`)
    disposals = data.disposals
  }

  async function report(d) {
    error = ''
    try {
      await api.post(`/disposals/${d.id}/reports`, reportForm)
      notice = `处置单 ${d.disposal_no} 已回报`
      reporting = null
      reportForm = { reporter: '', content: '' }
      await load()
    } catch (e) { error = e.message }
  }

  async function complete(d) {
    error = ''; notice = ''
    try {
      await api.post(`/disposals/${d.id}/complete`)
      notice = `处置单 ${d.disposal_no} 已标记完成`
      await load()
    } catch (e) { error = e.message }
  }
</script>

<div class="card">
  <h2>🛠️ 养护单位工作台</h2>
  <div class="row">
    <input bind:value={assignee} placeholder="养护单位名称（如 绿城养护队）" size="26" />
    <button on:click={load}>查询我的处置单</button>
    <button class="ghost" on:click={() => { assignee = ''; load() }}>全部</button>
  </div>
  {#if notice}<p class="ok">{notice}</p>{/if}
  {#if error}<p class="err">{error}</p>{/if}

  <table>
    <thead><tr><th>处置单号</th><th>树木 / 轮次</th><th>养护单位</th><th>措施</th><th>状态</th><th>操作</th></tr></thead>
    <tbody>
    {#each disposals as d}
      <tr>
        <td>{d.disposal_no}<div class="muted">{fmtTime(d.issued_at)}</div></td>
        <td>{d.tree_no}<div class="muted">{d.road} · 第 {d.round_no} 轮</div></td>
        <td>{d.assignee}</td>
        <td>{d.method || '—'}</td>
        <td><span class="tag {d.status === 'COMPLETED' ? '' : 'orange'}">{DISPOSAL_STATUS_LABEL[d.status]}</span></td>
        <td>
          {#if d.status !== 'COMPLETED'}
            <button class="ghost" on:click={() => { reporting = d.id; error = ''; notice = '' }}>回报治理情况</button>
            <button on:click={() => complete(d)}>标记完成</button>
          {:else}
            <span class="muted">✔ {fmtTime(d.completed_at)}</span>
          {/if}
        </td>
      </tr>
      {#if reporting === d.id}
        <tr><td colspan="6">
          <div class="subform row">
            <input bind:value={reportForm.reporter} placeholder="回报人 *" size="10" />
            <input bind:value={reportForm.content} placeholder="治理情况 *" size="44" />
            <button on:click={() => report(d)} disabled={!reportForm.reporter || !reportForm.content}>提交回报</button>
            <button class="ghost" on:click={() => (reporting = null)}>取消</button>
          </div>
        </td></tr>
      {/if}
    {/each}
    </tbody>
  </table>
  <p class="muted">提示：所在轮次没有「已核实的确诊结果」时，系统不允许标记完成。</p>
</div>

<style>
  .subform { background: #eef5ee; padding: 10px; border-radius: 8px; }
</style>
