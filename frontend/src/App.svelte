<script>
  import { onMount } from 'svelte'
  import PublicPage from './pages/PublicPage.svelte'
  import StaffPage from './pages/StaffPage.svelte'
  import LabPage from './pages/LabPage.svelte'
  import MaintenancePage from './pages/MaintenancePage.svelte'
  import NotifyPage from './pages/NotifyPage.svelte'

  let route = '#/'
  onMount(() => {
    const sync = () => { route = window.location.hash || '#/' }
    sync()
    window.addEventListener('hashchange', sync)
    return () => window.removeEventListener('hashchange', sync)
  })

  const nav = [
    ['#/', '公众查询'],
    ['#/staff', '工作人员'],
    ['#/lab', '实验室'],
    ['#/maintenance', '养护单位'],
    ['#/notify', '通知记录'],
  ]
</script>

<div class="shell">
  <header>
    <h1>🌳 街道树木病虫害送鉴与处置系统</h1>
    <nav>
      {#each nav as [href, label]}
        <a {href} class:active={route === href}>{label}</a>
      {/each}
    </nav>
  </header>
  <main>
    {#if route === '#/'}
      <PublicPage />
    {:else if route === '#/staff'}
      <StaffPage />
    {:else if route === '#/lab'}
      <LabPage />
    {:else if route === '#/maintenance'}
      <MaintenancePage />
    {:else if route === '#/notify'}
      <NotifyPage />
    {:else}
      <PublicPage />
    {/if}
  </main>
</div>

<style>
  :global(body) {
    margin: 0;
    font-family: 'PingFang SC', 'Microsoft YaHei', system-ui, sans-serif;
    background: #f4f6f4;
    color: #24352a;
  }
  :global(*) { box-sizing: border-box; }
  .shell { max-width: 1180px; margin: 0 auto; padding: 0 16px 48px; }
  header {
    display: flex; flex-wrap: wrap; align-items: center; gap: 12px;
    padding: 14px 0; border-bottom: 2px solid #2e7d32; margin-bottom: 18px;
  }
  h1 { font-size: 20px; margin: 0; color: #1b5e20; flex: 1 1 320px; }
  nav { display: flex; gap: 6px; flex-wrap: wrap; }
  nav a {
    padding: 6px 14px; border-radius: 16px; text-decoration: none;
    color: #2e7d32; border: 1px solid #c8e6c9; background: #fff; font-size: 14px;
  }
  nav a.active { background: #2e7d32; color: #fff; border-color: #2e7d32; }
  :global(.card) {
    background: #fff; border: 1px solid #dfe7df; border-radius: 10px;
    padding: 16px; margin-bottom: 14px; box-shadow: 0 1px 2px rgba(0,0,0,.04);
  }
  :global(h2) { font-size: 17px; margin: 0 0 12px; color: #1b5e20; }
  :global(h3) { font-size: 15px; margin: 14px 0 8px; color: #33691e; }
  :global(button) {
    background: #2e7d32; color: #fff; border: none; border-radius: 6px;
    padding: 7px 14px; cursor: pointer; font-size: 14px;
  }
  :global(button:hover) { background: #1b5e20; }
  :global(button.ghost) { background: #fff; color: #2e7d32; border: 1px solid #2e7d32; }
  :global(button.warn) { background: #c62828; }
  :global(button:disabled) { opacity: .5; cursor: not-allowed; }
  :global(input), :global(select), :global(textarea) {
    border: 1px solid #c5d5c5; border-radius: 6px; padding: 7px 10px;
    font-size: 14px; font-family: inherit; background: #fff;
  }
  :global(table) { width: 100%; border-collapse: collapse; font-size: 14px; }
  :global(th), :global(td) { text-align: left; padding: 7px 9px; border-bottom: 1px solid #e8efe8; vertical-align: top; }
  :global(th) { color: #557; background: #f2f7f2; }
  :global(.tag) {
    display: inline-block; padding: 1px 8px; border-radius: 10px;
    font-size: 12px; background: #e8f5e9; color: #2e7d32; margin-right: 4px;
  }
  :global(.tag.gray) { background: #eceff1; color: #546e7a; }
  :global(.tag.orange) { background: #fff3e0; color: #e65100; }
  :global(.tag.red) { background: #ffebee; color: #c62828; }
  :global(.tag.blue) { background: #e3f2fd; color: #1565c0; }
  :global(.err) { color: #c62828; font-size: 13px; margin-top: 6px; }
  :global(.ok) { color: #2e7d32; font-size: 13px; margin-top: 6px; }
  :global(.muted) { color: #78909c; font-size: 13px; }
  :global(.row) { display: flex; gap: 10px; flex-wrap: wrap; align-items: center; }
</style>
