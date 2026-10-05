// 统一 API 访问封装
async function req(path, options = {}) {
  const resp = await fetch(`/api${path}`, {
    headers: { 'Content-Type': 'application/json' },
    ...options,
  })
  const text = await resp.text()
  let data
  try { data = JSON.parse(text) } catch { data = { raw: text } }
  if (!resp.ok) {
    throw new Error(data.error || `请求失败 (${resp.status})`)
  }
  return data
}

export const api = {
  get: (path) => req(path),
  post: (path, body = {}) => req(path, { method: 'POST', body: JSON.stringify(body) }),
}

export const RESULT_TYPE_LABEL = {
  CONFIRMED: '确诊',
  NEGATIVE: '未检出',
  INCONCLUSIVE: '无法判定',
  REJECTED: '退样',
}
export const SAMPLE_STATUS_LABEL = {
  SUBMITTED: '已送样（待鉴定）',
  RESULT_DRAFT: '鉴定中（草稿未核实）',
  RESULT_VERIFIED: '结果已核实',
}
export const DISPOSAL_STATUS_LABEL = {
  ISSUED: '已派单',
  IN_PROGRESS: '处理中',
  COMPLETED: '已完成',
}
export const ROUND_STATUS_LABEL = { OPEN: '进行中', CLOSED: '已结案' }

export function fmtTime(t) {
  return t ? t.replace('T', ' ').slice(0, 19) : '—'
}
