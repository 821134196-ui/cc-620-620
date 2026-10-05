<script>
  // 工作人员页：树木台账 + 全量历史（含实验室草稿）+ 送样/开轮/派单/结案
  import { onMount } from 'svelte'
  import { api, RESULT_TYPE_LABEL, SAMPLE_STATUS_LABEL, DISPOSAL_STATUS_LABEL, ROUND_STATUS_LABEL, fmtTime } from '../lib/api.js'

  let trees = []
  let q = ''
  let detail = null
  let error = ''
  let notice = ''

  // 新建树木表单
  let showCreate = false
  let newTree = { tree_no: '', species: '', road: '', address: '', lat: 39.9, lng: 116.3 }

  // 送样表单
  let sampleForm = { inspector: '', symptom: '', note: '' }
  // 派单表单
  let disposalForm = { round_id: null, sample_id: '', assignee: '', method: '' }
  // 重送表单
  let resubmitFor = null
  let resubmitForm = { inspector: '', symptom: '', note: '' }

  onMount(loadTrees)

  async function loadTrees() {
    const data = await api.get(`/trees?q=${encodeURIComponent(q)}`)
    trees = data.trees
  }

  async function openDetail(t) {
    error = ''; notice = ''
    try { detail = await api.get(`/trees/${t.id}`) } catch (e) { error = e.message }
  }

  async function createTree() {
    error = ''
    try {
      const body = { ...newTree, lat: +newTree.lat, lng: +newTree.lng }
      if (!body.tree_no) delete body.tree_no
      await api.post('/trees', body)
      showCreate = false
      newTree = { tree_no: '', species: '', road: '', address: '', lat: 39.9, lng: 116.3 }
      await loadTrees()
      notice = '树木已登记'
    } catch (e) { error = e.message }
  }

  async function submitSample() {
    error = ''
    try {
      const r = await api.post('/samples', { tree_id: detail.tree.id, ...sampleForm })
      notice = `已送样，样本编号 ${r.sample.sample_no}（所属轮次：第 ${await currentRoundNo()} 轮）`
      sampleForm = { inspector: '', symptom: '', note: '' }
      await openDetail(detail.tree)
    } catch (e) { error = e.message }
  }

  async function currentRoundNo() {
    const d = await api.get(`/trees/${detail.tree.id}`)
    return d.rounds[0]?.round.round_no ?? 1
  }

  async function openRound() {
    error = ''
    try {
      const r = await api.post(`/trees/${detail.tree.id}/rounds`, {})
      notice = `已开启第 ${r.round.round_no} 轮处理`
      await openDetail(detail.tree)
    } catch (e) { error = e.message }
  }

  async function closeRound(round) {
    error = ''
    try {
      await api.post(`/rounds/${round.id}/close`, {})
      notice = `第 ${round.round_no} 轮已结案`
      await openDetail(detail.tree)
    } catch (e) { error = e.message }
  }

  async function issueDisposal() {
    error = ''
    try {
      const body = {
        round_id: disposalForm.round_id,
        assignee: disposalForm.assignee,
        method: disposalForm.method,
        ...(disposalForm.sample_id ? { sample_id: +disposalForm.sample_id } : {}),
      }
      const r = await api.post('/disposals', body)
      notice = `处置单 ${r.disposal.disposal_no} 已下达并通知养护单位`
      disposalForm = { round_id: null, sample_id: '', assignee: '', method: '' }
      await openDetail(detail.tree)
    } catch (e) { error = e.message }
  }

  async function resubmit(sample) {
    error = ''
    try {
      const r = await api.post(`/samples/${sample.id}/resubmit`, { tree_id: detail.tree.id, ...resubmitForm })
      notice = `已重送，新样本编号 ${r.sample.sample_no}（关联原样本 ${r.parent_sample_no}）`
      resubmitFor = null
      resubmitForm = { inspector: '', symptom: '', note: '' }
      await openDetail(detail.tree)
    } catch (e) { error = e.message }
  }

  const canResubmit = (item) =>
    item.result && item.result.status === 'VERIFIED' &&
    (item.result.result_type === 'REJECTED' || item.result.result_type === 'INCONCLUSIVE') &&
    item.resubmitted_as.length === 0
</script>

<div class="card">
  <h2>🌲 树木台账（工作人员）</h2>
  <div class="row">
    <input bind:value={q} placeholder="树号 / 道路 / 树种" size="24" on:keydown={(e) => e.key === 'Enter' && loadTrees()} />
    <button on:click={loadTrees}>搜索</button>
    <button class="ghost" on:click={() => (showCreate = !showCreate)}>＋ 登记树木</button>
  </div>
  {#if notice}<p class="ok">{notice}</p>{/if}
  {#if error}<p class="err">{error}</p>{/if}

  {#if showCreate}
    <div class="subform">
      <div class="row">
        <input bind:value={newTree.tree_no} placeholder="树号（留空自动编号）" size="16" />
        <input bind:value={newTree.species} placeholder="树种 *" size="10" />
        <input bind:value={newTree.road} placeholder="道路 *" size="12" />
        <input bind:value={newTree.address} placeholder="位置描述" size="16" />
        <input bind:value={newTree.lat} type="number" step="0.0001" placeholder="纬度" size="9" />
        <input bind:value={newTree.lng} type="number" step="0.0001" placeholder="经度" size="9" />
        <button on:click={createTree}>保存</button>
      </div>
    </div>
  {/if}

  <table>
    <thead><tr><th>树号</th><th>树种</th><th>道路</th><th>位置</th><th></th></tr></thead>
    <tbody>
    {#each trees as t}
      <tr>
        <td>{t.tree_no}</td><td>{t.species}</td><td>{t.road}</td><td>{t.address}</td>
        <td><button class="ghost" on:click={() => openDetail(t)}>档案 / 送样</button></td>
      </tr>
    {/each}
    </tbody>
  </table>
</div>

{#if detail}
  <div class="card">
    <h2>📁 {detail.tree.tree_no} · {detail.tree.species} 完整档案</h2>
    <p class="muted">{detail.tree.road} · {detail.tree.address} · 坐标 ({detail.tree.lat}, {detail.tree.lng})</p>

    <h3>📮 新送样（无进行中轮次时将自动开新一轮）</h3>
    <div class="row">
      <input bind:value={sampleForm.inspector} placeholder="送检人 *" size="10" />
      <input bind:value={sampleForm.symptom} placeholder="疑似症状" size="28" />
      <input bind:value={sampleForm.note} placeholder="备注" size="16" />
      <button on:click={submitSample} disabled={!sampleForm.inspector}>提交送样</button>
      <button class="ghost" on:click={openRound}>手动开新一轮（复发）</button>
    </div>

    {#each detail.rounds as r}
      <div class="round">
        <h3>
          第 {r.round.round_no} 轮
          <span class="tag {r.round.status === 'OPEN' ? 'orange' : 'gray'}">{ROUND_STATUS_LABEL[r.round.status]}</span>
          <span class="muted">{r.round.reason} · {fmtTime(r.round.opened_at)}</span>
          {#if r.round.status === 'OPEN'}
            <button class="ghost" on:click={() => closeRound(r.round)}>本轮结案</button>
          {/if}
        </h3>

        <h3>送鉴样本（含实验室草稿，仅工作人员可见）</h3>
        {#if r.samples.length === 0}<p class="muted">本轮暂无样本</p>{/if}
        {#each r.samples as item}
          <div class="item">
            <b>{item.sample.sample_no}</b>
            <span class="tag blue">{SAMPLE_STATUS_LABEL[item.sample.status]}</span>
            {#if item.parent_sample_no}<span class="tag gray">重送自 {item.parent_sample_no}</span>{/if}
            {#each item.resubmitted_as as child}<span class="tag gray">已重送为 {child}</span>{/each}
            <div class="muted">
              送检人 {item.sample.inspector} · {fmtTime(item.sample.submitted_at)} · 症状：{item.sample.symptom || '—'}
            </div>
            {#if item.result}
              <div>
                结果：<span class="tag {item.result.status === 'DRAFT' ? 'orange' : item.result.result_type === 'CONFIRMED' ? 'red' : ''}">
                  {RESULT_TYPE_LABEL[item.result.result_type]}{item.result.status === 'DRAFT' ? '（草稿未核实）' : '（已核实）'}
                </span>
                {#if item.result.pest_name}<b>{item.result.pest_name}</b>{/if}
                {#if item.result.notes}<span class="muted"> · {item.result.notes}</span>{/if}
                <span class="muted"> · 记录人 {item.result.created_by}</span>
              </div>
            {:else}
              <div class="muted">实验室尚未录入结果</div>
            {/if}
            {#if canResubmit(item)}
              {#if resubmitFor === item.sample.id}
                <div class="row subform">
                  <input bind:value={resubmitForm.inspector} placeholder="送检人（默认原送检人）" size="16" />
                  <input bind:value={resubmitForm.symptom} placeholder="症状（默认沿用）" size="20" />
                  <input bind:value={resubmitForm.note} placeholder="备注" size="12" />
                  <button on:click={() => resubmit(item.sample)}>确认重送</button>
                  <button class="ghost" on:click={() => (resubmitFor = null)}>取消</button>
                </div>
              {:else}
                <button class="ghost" on:click={() => (resubmitFor = item.sample.id)}>退样/无法判定 → 重送</button>
              {/if}
            {/if}
          </div>
        {/each}

        <h3>处置单</h3>
        {#each r.disposals as d}
          <div class="item disposal">
            <b>{d.disposal.disposal_no}</b>
            <span class="tag {d.disposal.status === 'COMPLETED' ? '' : 'orange'}">{DISPOSAL_STATUS_LABEL[d.disposal.status]}</span>
            {#if d.basis_sample_no}<span class="tag gray">依据样本 {d.basis_sample_no}</span>{/if}
            <div class="muted">{d.disposal.assignee} · {d.disposal.method || '—'} · 派单 {fmtTime(d.disposal.issued_at)}</div>
            {#each d.reports as rep}
              <div class="report">↳ 回报（{fmtTime(rep.reported_at)} {rep.reporter}）：{rep.content}</div>
            {/each}
            {#if d.disposal.completed_at}<div class="ok">✔ {fmtTime(d.disposal.completed_at)} 完成</div>{/if}
          </div>
        {/each}

        {#if r.round.status === 'OPEN'}
          {#if disposalForm.round_id === r.round.id}
            <div class="row subform">
              <select bind:value={disposalForm.sample_id}>
                <option value="">依据样本（可选）</option>
                {#each r.samples as item}
                  <option value={item.sample.id}>{item.sample.sample_no}</option>
                {/each}
              </select>
              <input bind:value={disposalForm.assignee} placeholder="养护单位 *" size="14" />
              <input bind:value={disposalForm.method} placeholder="处置措施" size="24" />
              <button on:click={issueDisposal} disabled={!disposalForm.assignee}>下达处置单</button>
              <button class="ghost" on:click={() => (disposalForm.round_id = null)}>取消</button>
            </div>
          {:else}
            <button class="ghost" on:click={() => (disposalForm = { round_id: r.round.id, sample_id: '', assignee: '', method: '' })}>＋ 下达处置单</button>
          {/if}
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .round { border-top: 1px dashed #cfd8cf; padding-top: 6px; margin-top: 12px; }
  .item { background: #f7faf7; border: 1px solid #e3ece3; border-radius: 8px; padding: 10px 12px; margin: 8px 0; }
  .item.disposal { background: #fdf9f3; border-color: #f0e6d2; }
  .report { font-size: 13px; color: #555; margin-top: 4px; }
  .subform { background: #eef5ee; padding: 10px; border-radius: 8px; margin: 8px 0; }
</style>
