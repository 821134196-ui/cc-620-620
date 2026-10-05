<script>
  import { get, post } from '../lib/api.js';
  import MapView from './MapView.svelte';
  import SampleCard from './SampleCard.svelte';

  let trees = $state([]);
  let mapTrees = $state([]);
  let history = $state(null);
  let keyword = $state('');
  let notices = $state([]);
  let err = $state('');

  // 新树建档表单
  let nCode = $state('');
  let nRoad = $state('');
  let nSpecies = $state('');
  let nX = $state(0.5);
  let nY = $state(0.5);
  let showNewTree = $state(false);

  // 新送样表单
  let symptom = $state('');
  let inspector = $state('张巡');

  async function loadTrees(q = '') {
    trees = await get(`/api/staff/trees?q=${encodeURIComponent(q)}`);
  }
  async function loadMap() {
    mapTrees = await get('/api/staff/map');
  }
  async function loadNotices() {
    notices = await get('/api/staff/notifications');
  }
  async function openTree(id) {
    history = await get(`/api/staff/trees/${id}/history`);
  }
  async function refresh() {
    if (history) await openTree(history.tree.id);
    await Promise.all([loadTrees(keyword), loadMap(), loadNotices()]);
  }

  loadTrees();
  loadMap();
  loadNotices();

  let timer;
  function onSearch(e) {
    keyword = e.target.value;
    clearTimeout(timer);
    timer = setTimeout(() => loadTrees(keyword), 200);
  }

  async function createTree() {
    err = '';
    try {
      const t = await post('/api/staff/trees', {
        tree_code: nCode, road_name: nRoad, species: nSpecies,
        x: Number(nX), y: Number(nY),
      });
      showNewTree = false;
      nCode = ''; nRoad = ''; nSpecies = '';
      await refresh();
      await openTree(t.id);
    } catch (e) { err = e.message; }
  }

  async function submitSample() {
    err = '';
    try {
      await post('/api/staff/samples', {
        tree_id: history.tree.id, inspector, symptom,
      });
      symptom = '';
      await openTree(history.tree.id);
      await Promise.all([loadMap(), loadNotices()]);
    } catch (e) { err = e.message; }
  }

  function mapSelect(id) {
    openTree(id);
  }
</script>

<div class="grid staff">
  <div>
    <div class="card">
      <h2>树木台账（固定树号）</h2>
      <input type="search" placeholder="道路 / 树号搜索" value={keyword} oninput={onSearch} style="width:100%;margin-bottom:10px" />
      <ul class="clean" style="max-height:300px;overflow:auto">
        {#each trees as t (t.id)}
          <li class="tree-item" class:selected={history?.tree.id === t.id} onclick={() => openTree(t.id)}>
            <div class="code">{t.tree_code}</div>
            <div class="meta">{t.road_name} · {t.species}</div>
          </li>
        {/each}
      </ul>
      <button class="btn small secondary" style="margin-top:8px" onclick={() => (showNewTree = !showNewTree)}>
        {showNewTree ? '收起' : '新树建档'}
      </button>
      {#if showNewTree}
        <div class="form-inline">
          <label class="field">树号（留空自动生成 T-xxxx）<input bind:value={nCode} placeholder="T-0008" /></label>
          <label class="field">道路<input bind:value={nRoad} placeholder="如：林荫路" /></label>
          <label class="field">树种<input bind:value={nSpecies} placeholder="如：香樟" /></label>
          <div class="row">
            <label class="field">x（0~1）<input type="number" step="0.01" min="0" max="1" bind:value={nX} /></label>
            <label class="field">y（0~1）<input type="number" step="0.01" min="0" max="1" bind:value={nY} /></label>
          </div>
          <button class="btn small" onclick={createTree} disabled={!nRoad.trim() || !nSpecies.trim()}>建档</button>
        </div>
      {/if}
    </div>

    <div class="card" style="margin-top:16px">
      <h2>工作通知流（含内部）</h2>
      <div style="max-height:300px;overflow:auto">
        {#each notices.slice(0, 30) as n (n.id)}
          <div class="notice">
            <div class="row">
              <span class="nt">{n.title}</span>
              <span class="badge {n.public_visible ? 'green' : 'gray'}">{n.public_visible ? '已公开' : '内部'}</span>
            </div>
            <div class="nb">{n.body}</div>
            <div class="nt-time">[{n.channel}] → {n.target} · {n.created_at}</div>
          </div>
        {/each}
      </div>
    </div>
  </div>

  <div>
    {#if err}<div class="error">{err}</div>{/if}
    <div class="card">
      <h2>工作人员地图（红点=待处置，琥珀=治理中，绿=完成/健康）</h2>
      <MapView trees={mapTrees} selectedId={history?.tree.id ?? null} onSelect={mapSelect} />
    </div>

    {#if !history}
      <div class="card" style="margin-top:16px"><p class="muted">从左侧台账或地图选择一棵树，查看其历次送样、鉴定与处置记录。</p></div>
    {:else}
      <div class="card" style="margin-top:16px">
        <h2>{history.tree.tree_code} · 历次记录（{history.tree.road_name} · {history.tree.species}）</h2>

        <div class="row" style="align-items:flex-end">
          <label class="field">巡查员<input bind:value={inspector} style="width:110px" /></label>
          <label class="field" style="flex:1">疑似症状（新一轮送样，沿本固定树号）
            <input bind:value={symptom} placeholder="如：叶片出现虫瘿、枝条流胶" />
          </label>
          <button class="btn" onclick={submitSample} disabled={!symptom.trim()}>新建送样</button>
        </div>
        <p class="muted" style="margin:6px 0 0">同一棵树可多次送样；复发时沿原树号新建一轮。退样后的重送请在对应样本卡片内操作，以保留与原送样的关联。</p>
      </div>

      <div style="margin-top:16px">
        {#each history.samples as item (item.sample.id)}
          <SampleCard {item} onChange={refresh} />
        {/each}
      </div>
    {/if}
  </div>
</div>
