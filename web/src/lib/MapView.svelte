<script>
  import { statusColor } from '../lib/format.js';

  let { trees = [], selectedId = null, onSelect = () => {} } = $props();

  // 本地模拟地图：归一化坐标 + 几条装饰道路
  const roads = [
    { d: 'M20,180 L560,150', label: '林荫路', lx: 250, ly: 150 },
    { d: 'M60,330 L580,300', label: '人民大道', lx: 320, ly: 328 },
    { d: 'M120,20 L520,380', label: '建设街', lx: 430, ly: 320 },
    { d: 'M300,380 L360,40', label: '滨河路', lx: 300, ly: 220 },
  ];

  // 公开点位为嵌套 {tree:{...}}，工作人员点位为平铺，两种都兼容
  const tid = (t) => t.id ?? t.tree?.id;
  const tx = (t) => t.x ?? t.tree?.x ?? 0;
  const ty = (t) => t.y ?? t.tree?.y ?? 0;
  const code = (t) => t.tree_code ?? t.tree?.tree_code ?? '';
  const road = (t) => t.road_name ?? t.tree?.road_name ?? '';
  const species = (t) => t.species ?? t.tree?.species ?? '';

  function cx(t) { return 20 + tx(t) * 560; }
  function cy(t) { return 30 + ty(t) * 340; }
</script>

<div class="map-wrap">
  <svg class="map" viewBox="0 0 600 400" role="img" aria-label="树木分布模拟地图">
    {#each roads as r}
      <path class="road" d={r.d} />
      <text class="road-label" x={r.lx} y={r.ly}>{r.label}</text>
    {/each}
    {#each trees as t (tid(t))}
      {@const active = selectedId === tid(t)}
      <g
        role="button"
        tabindex="0"
        aria-label={`查看树木 ${code(t)}`}
        onclick={() => onSelect(tid(t))}
        onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && onSelect(tid(t))}
      >
        <circle
          class="tree-point"
          cx={cx(t)}
          cy={cy(t)}
          r={active ? 11 : 8}
          fill={statusColor(t.public_status)}
        >
          <title>{code(t)} · {road(t)} · {species(t)} — {t.public_status_text ?? ''}</title>
        </circle>
        <text class="tree-code" x={cx(t)} y={cy(t) - 12}>{code(t)}</text>
      </g>
    {/each}
  </svg>
</div>
<p class="muted" style="margin-top:6px">地图为本地模拟：树木按固定坐标绘制，点击点位可查看详情。</p>
