#!/bin/zsh
# merge.sh <KEY>...: Local-merge each ticket with the message Otaq proposes, as the Approve button would.
# Merging is what lets dependants start (a ticket waits until its parents are merged), so a ticket and the one that
# waits for it can't be merged in one call: merge the parent, wait for the child to reach review, then merge it.
DEMO=${OTAQ_DEMO:-/tmp/otaqdemo}
export OTAQ_HOME=$DEMO/home
R=(python3 -I ${0:A:h}/otaq_req.py)
for k in "$@"; do
  msg=$("${R[@]}" ticket.merge_proposal "{\"key\":\"$k\"}" | python3 -c 'import json,sys; print(json.dumps(json.load(sys.stdin)["reply"]["merge_proposal"]["message"]))') || { echo "$k: no merge proposal" >&2; continue; }
  "${R[@]}" ticket.local_merge "{\"key\":\"$k\",\"message\":$msg}" \
    | python3 -c 'import json,sys; m=json.load(sys.stdin); print(sys.argv[1]+":", m.get("error",{}).get("message") or m["reply"]["local_merge"]["outcome"])' $k
done
