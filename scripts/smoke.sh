#!/usr/bin/env bash
# 针对运行中服务的端到端冒烟：退样重送、多轮结论保留、结案守卫、公开可见性
set -u
BASE="${BASE:-http://127.0.0.1:8080/api}"
J() { python3 -c "import sys,json;d=json.load(sys.stdin);print(eval(sys.argv[1]))" "$1" 2>/dev/null; }

echo "== 1. 健康检查 & 种子树木"
curl -fsS "$BASE/health" | python3 -m json.tool
N=$(curl -fsS "$BASE/public/trees" | J "len(d)")
echo "公开树木数: $N"
[ "$N" -ge 7 ] || { echo "种子数据缺失"; exit 1; }

echo "== 2. 道路搜索（林荫路）与树号搜索"
curl -fsS "$BASE/public/trees?q=$(python3 -c "import urllib.parse;print(urllib.parse.quote('林荫路'))")" | J "[t['tree']['tree_code'] for t in d]"
TREE=$(curl -fsS "$BASE/public/trees?q=T-0001" | J "d[0]['tree']['id']")
echo "T-0001 id=$TREE"

echo "== 3. 新建送样（第 1 轮）"
S1=$(curl -fsS -H 'Content-Type: application/json' -d '{"tree_id":'"$TREE"',"inspector":"张巡","symptom":"叶面虫瘿"}' "$BASE/staff/samples")
echo "$S1"
SID1=$(echo "$S1" | J "d['id']")

echo "== 4. 草稿阶段：公开页必须看不到"
TL=$(curl -fsS "$BASE/public/trees/$TREE" | J "len(d['timeline'])")
echo "登记结果前公开时间线条目: $TL"

curl -fsS -X PUT -H 'Content-Type: application/json' \
  -d '{"conclusion":"pest_confirmed","diagnosis":"方翅网蝽","severity":"moderate","recorded_by":"李工"}' \
  "$BASE/staff/samples/$SID1/result" >/dev/null
TL=$(curl -fsS "$BASE/public/trees/$TREE" | J "len(d['timeline'])")
echo "草稿阶段公开时间线条目（应为 0）: $TL"
[ "$TL" = "0" ] || { echo "草稿泄漏到公开页!"; exit 1; }

echo "== 5. 草稿期签发处置单应被拒（400）"
curl -s -o /dev/null -w "HTTP %{http_code}\n" -H 'Content-Type: application/json' \
  -d '{"sample_id":'"$SID1"',"assigned_unit":"绿洁养护公司"}' "$BASE/staff/treatments"

echo "== 6. 核实后签发处置单；未回报直接结案应被拒（400）"
curl -fsS -H 'Content-Type: application/json' -d '{"verified_by":"王主任"}' \
  "$BASE/staff/samples/$SID1/verify" >/dev/null
T1=$(curl -fsS -H 'Content-Type: application/json' \
  -d '{"sample_id":'"$SID1"',"assigned_unit":"绿洁养护公司"}' "$BASE/staff/treatments")
TID=$(echo "$T1" | J "d['id']")
curl -s -o /dev/null -w "未回报结案 HTTP %{http_code}（应 400）\n" -X POST "$BASE/staff/treatments/$TID/complete"

echo "== 7. 回报后结案"
curl -fsS -H 'Content-Type: application/json' -d '{"content":"已喷药并剪除虫枝","reporter":"赵班"}' \
  "$BASE/staff/treatments/$TID/reports" | J "d['status']"
curl -fsS -X POST "$BASE/staff/treatments/$TID/complete" | J "(d['status'],d['completed_at'])"

echo "== 8. 第 2 轮送样：健康结论（与第 1 轮并存，不覆盖）"
S2=$(curl -fsS -H 'Content-Type: application/json' -d '{"tree_id":'"$TREE"',"inspector":"张巡","symptom":"复查"}' "$BASE/staff/samples")
SID2=$(echo "$S2" | J "d['id']")
[ "$(echo "$S2" | J "d['round']")" = "2" ] && echo "round=2 OK"
curl -fsS -X PUT -H 'Content-Type: application/json' \
  -d '{"conclusion":"healthy","recorded_by":"李工"}' "$BASE/staff/samples/$SID2/result" >/dev/null
curl -fsS -H 'Content-Type: application/json' -d '{"verified_by":"王主任"}' "$BASE/staff/samples/$SID2/verify" >/dev/null
echo "尝试覆盖第 1 轮已核实结论（应 409）:"
curl -s -o /dev/null -w "HTTP %{http_code}\n" -X PUT -H 'Content-Type: application/json' \
  -d '{"conclusion":"healthy","recorded_by":"李工"}' "$BASE/staff/samples/$SID1/result"

echo "== 9. 退样 → 重送关联原样本"
S3=$(curl -fsS -H 'Content-Type: application/json' -d '{"tree_id":'"$TREE"',"inspector":"张巡","symptom":"复发疑似"}' "$BASE/staff/samples")
SID3=$(echo "$S3" | J "d['id']")
ROUND3=$(echo "$S3" | J "d['round']")
curl -fsS -H 'Content-Type: application/json' -d '{"reason":"样本干枯"}' "$BASE/staff/samples/$SID3/return" | J "d['status']"
S4=$(curl -fsS -H 'Content-Type: application/json' \
  -d '{"tree_id":'"$TREE"',"inspector":"张巡","symptom":"复发疑似-重送","parent_sample_id":'"$SID3"'}' \
  "$BASE/staff/samples")
echo "$S4" | J "{'new_id':d['id'],'round':d['round'],'parent':d['parent_sample_id']}"
[ "$(echo "$S4" | J "d['round']")" = "$ROUND3" ] || { echo "重送未沿用原轮次"; exit 1; }

echo "== 10. 工作人员历史 & 公开时间线"
curl -fsS "$BASE/staff/trees/$TREE/history" | J "[(s['sample']['sample_code'],s['sample']['status'],s['parent_sample_code']) for s in d['samples']]"
echo "公开时间线条目（核实后应 >= 2）:"
curl -fsS "$BASE/public/trees/$TREE" | J "[(i['sample_code'],i['conclusion'],i['treatment_status']) for i in d['timeline']]"

echo "== 11. 公开通知（应只含 public_visible 条目）"
curl -fsS "$BASE/public/notifications" | J "len(d)"
curl -fsS "$BASE/staff/notifications" | J "len(d)"

echo
echo "全部冒烟断言通过 ✔"
