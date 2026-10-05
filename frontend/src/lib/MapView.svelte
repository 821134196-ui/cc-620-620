<script>
  // 本地模拟地图：SVG 街区图，道路为线、树木为点（数据来自 /api/map）
  import { onMount, createEventDispatcher } from 'svelte'
  import { api } from '../lib/api.js'

  const dispatch = createEventDispatcher()
  export let selectedTreeNo = null

  let roads = []
  let trees = []
  let error = ''

  const W = 860, H = 420, PAD = 40
  let minLng = 0, maxLng = 1, minLat = 0, maxLat = 1

  onMount(load)

  async function load() {
    try {
      const data = await api.get('/map')
      roads = data.roads
      trees = data.trees
      const pts = roads.flatMap(r => r.path).concat(trees.map(t => [t.lng, t.lat]))
      if (pts.length) {
        minLng = Math.min(...pts.map(p => p[0]))
        maxLng = Math.max(...pts.map(p => p[0]))
        minLat = Math.min(...pts.map(p => p[1]))
        maxLat = Math.max(...pts.map(p => p[1]))
        const dLng = (maxLng - minLng) || 0.01, dLat = (maxLat - minLat) || 0.01
        minLng -= dLng * 0.08; maxLng += dLng * 0.08
        minLat -= dLat * 0.12; maxLat += dLat * 0.12
      }
    } catch (e) { error = e.message }
  }

  function x(lng) { return PAD + ((lng - minLng) / (maxLng - minLng)) * (W - 2 * PAD) }
  function y(lat) { return H - PAD - ((lat - minLat) / (maxLat - minLat)) * (H - 2 * PAD) }
  function roadPath(path) {
    return path.map((p, i) => `${i ? 'L' : 'M'}${x(p[0]).toFixed(1)},${y(p[1]).toFixed(1)}`).join(' ')
  }
  function roadLabelPos(path) {
    const mid = path[Math.floor(path.length / 2)]
    return { x: x(mid[0]), y: y(mid[1]) - 8 }
  }
</script>

<div class="map-wrap">
  {#if error}<p class="err">{error}</p>{/if}
  <svg viewBox="0 0 {W} {H}" role="img" aria-label="街区树木模拟地图">
    <rect x="0" y="0" width={W} height={H} fill="#eef5ee" rx="8" />
    {#each roads as road}
      <path d={roadPath(road.path)} stroke="#b0bec5" stroke-width="10" fill="none" stroke-linecap="round" />
      <path d={roadPath(road.path)} stroke="#eceff1" stroke-width="6" fill="none" stroke-linecap="round" />
      {@const lp = roadLabelPos(road.path)}
      <text x={lp.x} y={lp.y} text-anchor="middle" font-size="13" fill="#607d8b">{road.name}</text>
    {/each}
    {#each trees as t}
      <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
      <g class="tree" on:click={() => dispatch('select', t)}>
        <circle cx={x(t.lng)} cy={y(t.lat)} r={selectedTreeNo === t.tree_no ? 11 : 8}
          fill={t.has_open_round ? '#ef6c00' : '#2e7d32'}
          stroke={selectedTreeNo === t.tree_no ? '#1b5e20' : '#fff'} stroke-width="2" />
        <text x={x(t.lng)} y={y(t.lat) - 13} text-anchor="middle" font-size="11" fill="#37474f">{t.tree_no}</text>
      </g>
    {/each}
  </svg>
  <div class="legend">
    <span><i class="dot green"></i>正常</span>
    <span><i class="dot orange"></i>有进行中的处理</span>
    <span class="muted">点击树点查看公开进展</span>
  </div>
</div>

<style>
  .map-wrap { width: 100%; }
  svg { width: 100%; height: auto; display: block; }
  .tree { cursor: pointer; }
  .tree:hover circle { stroke: #1b5e20; }
  .legend { display: flex; gap: 16px; align-items: center; margin-top: 6px; font-size: 13px; }
  .dot { display: inline-block; width: 10px; height: 10px; border-radius: 50%; margin-right: 4px; }
  .dot.green { background: #2e7d32; }
  .dot.orange { background: #ef6c00; }
</style>
