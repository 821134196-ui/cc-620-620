<script>
  // 本地模拟通知 outbox
  import { onMount } from 'svelte'
  import { api, fmtTime } from '../lib/api.js'

  let notifications = []
  onMount(async () => {
    const data = await api.get('/notifications')
    notifications = data.notifications
  })
</script>

<div class="card">
  <h2>📨 通知记录（本地模拟，不实际外发）</h2>
  <table>
    <thead><tr><th>时间</th><th>接收方</th><th>标题</th><th>内容</th></tr></thead>
    <tbody>
    {#each notifications as n}
      <tr>
        <td class="muted">{fmtTime(n.created_at)}</td>
        <td><span class="tag blue">{n.recipient}</span></td>
        <td>{n.title}</td>
        <td>{n.body}</td>
      </tr>
    {/each}
    </tbody>
  </table>
</div>
