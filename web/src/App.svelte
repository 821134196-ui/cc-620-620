<script>
  import PublicView from './lib/PublicView.svelte';
  import StaffView from './lib/StaffView.svelte';

  let tab = $state('public');
</script>

<header class="topbar">
  <h1>🌳 街道树木病虫害样本送鉴与处置通知系统</h1>
  <span class="sub">巡查送样 · 实验室鉴定 · 养护处置 · 公开核实进展</span>
  <nav class="tabs">
    <button class:active={tab === 'public'} onclick={() => (tab = 'public')}>居民公开查询</button>
    <button class:active={tab === 'staff'} onclick={() => (tab = 'staff')}>工作人员台</button>
  </nav>
</header>

<main>
  {#if tab === 'public'}
    <PublicView />
  {:else}
    <StaffView />
  {/if}
</main>
