export const conclusionLabel = (c) =>
  ({ pest_confirmed: '确诊病虫害', healthy: '未见病虫害', undetermined: '无法判定' }[c] || c || '—');

export const severityLabel = (s) =>
  ({ mild: '轻度', moderate: '中度', severe: '重度', '': '—' }[s] || '—');

export const sampleStatusLabel = (s) =>
  ({ submitted: '待检', returned: '已退样', testing: '检测中', determined: '已有结论' }[s] || s);

export const sampleStatusBadge = (s) =>
  ({ submitted: 'blue', returned: 'gray', testing: 'amber', determined: 'green' }[s] || 'gray');

export const treatmentStatusLabel = (s) =>
  ({ issued: '待接单', in_progress: '治理中', completed: '已结案' }[s] || s || '');

export const treatmentStatusBadge = (s) =>
  ({ issued: 'amber', in_progress: 'blue', completed: 'green' }[s] || 'gray');

export const publicStatusBadge = (s) =>
  ({
    no_verified: 'gray',
    confirmed_pending: 'red',
    treating: 'amber',
    treated: 'green',
    healthy: 'green',
    undetermined: 'gray',
  }[s] || 'gray');

// 模拟地图点位颜色
export const statusColor = (s) =>
  ({
    no_verified: '#8a9a90',
    confirmed_pending: '#b3261e',
    treating: '#b26a00',
    treated: '#2e7d46',
    healthy: '#2e7d46',
    undetermined: '#8a9a90',
  }[s] || '#8a9a90');
