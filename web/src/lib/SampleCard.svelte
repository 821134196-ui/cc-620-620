<script>
  import { post, put } from './api.js';
  import {
    conclusionLabel, severityLabel, sampleStatusLabel, sampleStatusBadge,
    treatmentStatusLabel, treatmentStatusBadge,
  } from './format.js';

  let { item, onChange = () => {} } = $props();
  const s = $derived(item.sample);
  const lr = $derived(item.lab_result);
  const t = $derived(item.treatment);

  let err = $state('');
  let busy = $state(false);
  let showReturn = $state(false);
  let showResult = $state(false);
  let showVerify = $state(false);
  let showIssue = $state(false);
  let returnReason = $state('');
  let fConclusion = $state('pest_confirmed');
  let fDiagnosis = $state('');
  let fSeverity = $state('moderate');
  let fNote = $state('');
  let fRecorder = $state('');
  let fVerifier = $state('');
  let fUnit = $state('绿洁养护公司');
  let fReport = $state('');
  let fReporter = $state('');

  async function run(fn) {
    err = '';
    busy = true;
    try { await fn(); await onChange(); }
    catch (e) { err = e.message; }
    finally { busy = false; }
  }

  const doResubmit = () => run(() =>
    post('/api/staff/samples', { tree_id: s.tree_id, inspector: '张巡', symptom: s.symptom, parent_sample_id: s.id }));
  const doReturn = () => run(async () => {
    await post(`/api/staff/samples/${s.id}/return`, { reason: returnReason });
    showReturn = false; returnReason = '';
  });
  const doSaveResult = () => run(async () => {
    await put(`/api/staff/samples/${s.id}/result`, {
      conclusion: fConclusion, diagnosis: fDiagnosis, severity: fSeverity,
      note: fNote, recorded_by: fRecorder,
    });
    showResult = false;
  });
  const doVerify = () => run(async () => {
    await post(`/api/staff/samples/${s.id}/verify`, { verified_by: fVerifier });
    showVerify = false;
  });
  const doIssue = () => run(async () => {
    await post('/api/staff/treatments', { sample_id: s.id, assigned_unit: fUnit });
    showIssue = false;
  });
  const doReport = () => run(async () => {
    await post(`/api/staff/treatments/${t.treatment.id}/reports`, { content: fReport, reporter: fReporter });
    fReport = '';
  });
  const doComplete = () => run(() => post(`/api/staff/treatments/${t.treatment.id}/complete`, {}));

  function openResultForm() {
    fConclusion = lr?.conclusion ?? 'pest_confirmed';
    fDiagnosis = lr?.diagnosis ?? '';
    fSeverity = lr?.severity ?? 'moderate';
    fNote = lr?.note ?? '';
    fRecorder = lr?.recorded_by ?? '';
    showResult = !showResult;
  }
</script>

<div class="sample-card">
  <div class="head">
    <strong>{s.sample_code}</strong>
    <span class="badge {sampleStatusBadge(s.status)}">{sampleStatusLabel(s.status)}</span>
    <span class="muted">第 {s.round} 轮 · {s.submitted_at}</span>
    {#if s.parent_sample_id}
      <span class="badge blue">退样重送，关联原样本 {item.parent_sample_code}</span>
    {/if}
  </div>
  <div class="kv">巡查员：{s.inspector}　疑似症状：{s.symptom}</div>
  {#if s.status === 'returned'}
    <div class="kv">退样原因：{s.return_reason}（{s.returned_at}）</div>
  {/if}

  {#if err}<div class="error">{err}</div>{/if}

  {#if lr}
    <div class="kv" style="margin-top:6px">
      <b>鉴定结论：</b>{conclusionLabel(lr.conclusion)}
      {#if lr.conclusion === 'pest_confirmed'}— {lr.diagnosis}（{severityLabel(lr.severity)}）{/if}
      ｜鉴定人：{lr.recorded_by}
      {#if lr.is_draft}
        <span class="badge amber">实验室草稿 · 不公开</span>
      {:else}
        <span class="badge green">已核实（{lr.verified_by} · {lr.verified_at}）· 已锁定不可覆盖</span>
      {/if}
    </div>
    {#if lr.note}<div class="kv muted">鉴定备注：{lr.note}</div>{/if}
  {/if}

  <!-- 操作区 -->
  <div class="row" style="margin-top:8px">
    {#if s.status === 'submitted'}
      <button class="btn small blue" onclick={openResultForm} disabled={busy}>
        {lr ? '修改草稿' : '实验室登记鉴定结果'}
      </button>
      <button class="btn small secondary" onclick={() => (showReturn = !showReturn)} disabled={busy}>退样</button>
    {:else if s.status === 'returned'}
      <button class="btn small" onclick={doResubmit} disabled={busy}>退样重送（关联本原样本）</button>
    {:else if lr?.is_draft}
      <button class="btn small blue" onclick={openResultForm} disabled={busy}>修改草稿</button>
      <button class="btn small" onclick={() => (showVerify = !showVerify)} disabled={busy}>审核核实并发布</button>
    {/if}

    {#if lr && !lr.is_draft && lr.conclusion === 'pest_confirmed' && !t}
      <button class="btn small amber" onclick={() => (showIssue = !showIssue)} disabled={busy}>签发处置单</button>
    {/if}
  </div>

  {#if showReturn}
    <div class="form-inline">
      <label class="field">退样原因
        <input bind:value={returnReason} placeholder="如：样本干枯、污染无法制片" />
      </label>
      <button class="btn small red" onclick={doReturn} disabled={busy || !returnReason.trim()}>确认退样</button>
    </div>
  {/if}

  {#if showResult}
    <div class="form-inline">
      <div class="row">
        <label class="field">结论
          <select bind:value={fConclusion}>
            <option value="pest_confirmed">确诊病虫害</option>
            <option value="healthy">未见病虫害</option>
            <option value="undetermined">无法判定</option>
          </select>
        </label>
        {#if fConclusion === 'pest_confirmed'}
          <label class="field">病虫害名称
            <input bind:value={fDiagnosis} placeholder="如：方翅网蝽" />
          </label>
          <label class="field">程度
            <select bind:value={fSeverity}>
              <option value="mild">轻度</option>
              <option value="moderate">中度</option>
              <option value="severe">重度</option>
            </select>
          </label>
        {/if}
      </div>
      <label class="field">备注
        <input bind:value={fNote} placeholder="鉴定说明（可选）" />
      </label>
      <label class="field">鉴定人
        <input bind:value={fRecorder} placeholder="实验室鉴定人姓名" />
      </label>
      <button class="btn small blue" onclick={doSaveResult} disabled={busy || !fRecorder.trim()}>
        {lr ? '更新草稿' : '保存为草稿'}
      </button>
      <span class="muted">草稿仅工作人员可见，核实后才对居民公开。</span>
    </div>
  {/if}

  {#if showVerify}
    <div class="form-inline">
      <label class="field">核实人
        <input bind:value={fVerifier} placeholder="审核核实人姓名" />
      </label>
      <button class="btn small" onclick={doVerify} disabled={busy || !fVerifier.trim()}>确认核实（结论锁定并公示）</button>
    </div>
  {/if}

  {#if showIssue}
    <div class="form-inline">
      <label class="field">养护单位
        <input bind:value={fUnit} />
      </label>
      <button class="btn small amber" onclick={doIssue} disabled={busy || !fUnit.trim()}>下达处置单</button>
    </div>
  {/if}

  {#if t}
    {@const tr = t.treatment}
    <div class="treatment-box">
      <div class="row">
        <strong>处置单 {tr.treatment_code}</strong>
        <span class="badge {treatmentStatusBadge(tr.status)}">{treatmentStatusLabel(tr.status)}</span>
        <span class="muted">养护单位：{tr.assigned_unit}</span>
      </div>
      <div class="muted" style="margin:3px 0">
        下达 {tr.issued_at}{#if tr.accepted_at} ｜ 接单 {tr.accepted_at}{/if}{#if tr.completed_at} ｜ 结案 {tr.completed_at}{/if}
      </div>
      {#if t.reports.length === 0}
        <div class="muted">养护单位尚未回报；未取得治理回报前不能结案。</div>
      {:else}
        {#each t.reports as r}
          <div class="report-item">{r.content} <span class="muted">— {r.reporter} · {r.created_at}</span></div>
        {/each}
      {/if}
      {#if tr.status !== 'completed'}
        <div class="form-inline">
          <div class="row">
            <input bind:value={fReport} placeholder="治理情况回报" style="flex:1;min-width:200px" />
            <input bind:value={fReporter} placeholder="报告人" style="width:110px" />
            <button class="btn small blue" onclick={doReport} disabled={busy || !fReport.trim() || !fReporter.trim()}>
              {tr.status === 'issued' ? '接单并回报' : '补充回报'}
            </button>
            <button class="btn small" onclick={doComplete} disabled={busy}>标记治理完成（结案）</button>
          </div>
          <span class="muted">结案守卫：鉴定结论须已核实确诊且已有治理回报，否则后端拒绝。</span>
        </div>
      {:else}
        <div class="ok">本轮治理已结案。如病虫害复发，请沿原树号新建一轮送样。</div>
      {/if}
    </div>
  {/if}
</div>
