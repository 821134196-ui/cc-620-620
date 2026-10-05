<script>
  // 实验室工作台：录入草稿 → 核实归档；退样
  import { onMount } from 'svelte'
  import { api, RESULT_TYPE_LABEL, SAMPLE_STATUS_LABEL, fmtTime } from '../lib/api.js'

  let samples = []
  let filter = ''
  let error = ''
  let notice = ''
  let editing = null   // 正在录入结果的样本 id
  let form = { result_type: 'CONFIRMED', pest_name: '', confidence: '', notes: '', created_by: '' }
  let rejecting = null
  let rejectForm = { reason: '', created_by: '' }

  onMount(load)

  async function load() {
    const data = await api.get(`/lab/samples${filter ? `?status=${filter}` : ''}`)
    samples = data.samples
  }

  function startEdit(s) {
    editing = s.id
    form = { result_type: 'CONFIRMED', pest_name: '', confidence: '', notes: '', created_by: '' }
    error = ''; notice = ''
  }

  async function saveDraft(s) {
    error = ''
    try {
      await api.post(`/lab/samples/${s.id}/result`, form)
      notice = `样本 ${s.sample_no} 草稿已保存（核实前公众不可见）`
      editing = null
      await load()
    } catch (e) { error = e.message }
  }

  async function verify(s) {
    error = ''
    try {
      await api.post(`/lab/samples/${s.id}/verify`)
      notice = `样本 ${s.sample_no} 结果已核实归档，对公众可见`
      await load()
    } catch (e) { error = e.message }
  }

  async function reject(s) {
    error = ''
    try {
      await api.post(`/lab/samples/${s.id}/reject`, rejectForm)
      notice = `样本 ${s.sample_no} 已退样，记录保留，可由工作人员重送`
      rejecting = null
      rejectForm = { reason: '', created_by: '' }
      await load()
    } catch (e) { error = e.message }
  }
</script>

<div class="card">
  <h2>🔬 实验室工作台</h2>
  <div class="row">
    <select bind:value={filter} on:change={load}>
      <option value="">全部状态</option>
      <option value="SUBMITTED">待鉴定</option>
      <option value="RESULT_DRAFT">草稿待核实</option>
      <option value="RESULT_VERIFIED">已核实归档</option>
    </select>
    <button class="ghost" on:click={load}>刷新</button>
  </div>
  {#if notice}<p class="ok">{notice}</p>{/if}
  {#if error}<p class="err">{error}</p>{/if}

  <table>
    <thead><tr><th>样本编号</th><th>树木</th><th>症状</th><th>送检人 / 时间</th><th>状态</th><th>操作</th></tr></thead>
    <tbody>
    {#each samples as s}
      <tr>
        <td>
          {s.sample_no}
          {#if s.parent_sample_id}<div class="tag gray">退样重送</div>{/if}
        </td>
        <td>{s.tree_no}<div class="muted">{s.road} · {s.species}</div></td>
        <td>{s.symptom || '—'}</td>
        <td>{s.inspector}<div class="muted">{fmtTime(s.submitted_at)}</div></td>
        <td>
          <span class="tag {s.status === 'RESULT_VERIFIED' ? '' : 'orange'}">{SAMPLE_STATUS_LABEL[s.status]}</span>
          {#if s.result_type}<div class="tag blue">{RESULT_TYPE_LABEL[s.result_type]}</div>{/if}
        </td>
        <td>
          {#if s.status !== 'RESULT_VERIFIED'}
            <button class="ghost" on:click={() => startEdit(s)}>录入结果</button>
            <button class="ghost warn" on:click={() => { rejecting = s.id; error = ''; notice = '' }}>退样</button>
          {:else}
            <span class="muted">已归档</span>
          {/if}
        </td>
      </tr>
      {#if editing === s.id}
        <tr><td colspan="6">
          <div class="subform">
            <div class="row">
              <select bind:value={form.result_type}>
                <option value="CONFIRMED">确诊</option>
                <option value="NEGATIVE">未检出</option>
                <option value="INCONCLUSIVE">无法判定</option>
              </select>
              {#if form.result_type === 'CONFIRMED'}
                <input bind:value={form.pest_name} placeholder="病虫害名称 *" size="16" />
              {/if}
              <input bind:value={form.confidence} placeholder="置信度（如 高/中/低）" size="14" />
              <input bind:value={form.created_by} placeholder="记录人 *" size="10" />
            </div>
            <div class="row" style="margin-top:6px">
              <textarea bind:value={form.notes} placeholder="鉴定备注" rows="2" style="flex:1"></textarea>
            </div>
            <div class="row" style="margin-top:6px">
              <button on:click={() => saveDraft(s)} disabled={!form.created_by}>保存草稿</button>
              <button class="ghost" on:click={() => (editing = null)}>取消</button>
              <span class="muted">草稿保存后请在列表中点击「核实」归档</span>
            </div>
          </div>
        </td></tr>
      {/if}
      {#if s.status === 'RESULT_DRAFT'}
        <tr><td colspan="6">
          <button on:click={() => verify(s)}>✔ 核实该样本结果</button>
          <span class="muted">核实后不可修改，并对公众公开</span>
        </td></tr>
      {/if}
      {#if rejecting === s.id}
        <tr><td colspan="6">
          <div class="subform row">
            <input bind:value={rejectForm.reason} placeholder="退样原因 *（如 样本量不足）" size="30" />
            <input bind:value={rejectForm.created_by} placeholder="操作人 *" size="10" />
            <button class="warn" on:click={() => reject(s)} disabled={!rejectForm.reason || !rejectForm.created_by}>确认退样</button>
            <button class="ghost" on:click={() => (rejecting = null)}>取消</button>
          </div>
        </td></tr>
      {/if}
    {/each}
    </tbody>
  </table>
</div>

<style>
  .subform { background: #eef5ee; padding: 10px; border-radius: 8px; }
</style>
