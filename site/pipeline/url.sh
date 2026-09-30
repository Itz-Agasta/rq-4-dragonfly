#!/usr/bin/env bash
# usage: url.sh <outPrefix>  prints the hosted result URL of a finished job (free status read)
HF=$(grep '^HF_CREDENTIALS=' "$(dirname "$0")/../../.env" | cut -d= -f2- | tr -d "\"'")
SU=$(python3 -c "import json,sys;print(json.load(open(sys.argv[1]+'.req.json'))['status_url'])" "$1")
curl -s --max-time 20 -H "Authorization: Key $HF" "$SU" | python3 -c "import json,sys;d=json.load(sys.stdin);print((d.get('images') or [d.get('video')])[0]['url'])"
