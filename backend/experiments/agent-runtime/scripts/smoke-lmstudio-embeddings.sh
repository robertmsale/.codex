#!/usr/bin/env bash
set -euo pipefail

if [[ "${ROBDEX_AGENT_RUNTIME_EMBEDDING_PROVIDER:-lmstudio}" != "lmstudio" ]]; then
  printf 'Skipping LM Studio smoke: set ROBDEX_AGENT_RUNTIME_EMBEDDING_PROVIDER=lmstudio to opt in.\n'
  exit 0
fi

BASE_URL="${ROBDEX_AGENT_RUNTIME_EMBEDDING_BASE_URL:-http://localhost:1234}"
MODEL="${ROBDEX_AGENT_RUNTIME_EMBEDDING_MODEL:-mlx-community/Qwen3-Embedding-4B-4bit-DWQ}"
URL="${BASE_URL%/}/v1/embeddings"
if [[ "${BASE_URL%/}" == */v1 ]]; then
  URL="${BASE_URL%/}/embeddings"
fi

BODY="$(mktemp /tmp/agent-runtime-lmstudio-embeddings.XXXXXX)"
HTTP_STATUS="$(curl --show-error --silent --output "$BODY" --write-out '%{http_code}' "$URL" \
  -H "Content-Type: application/json" \
  -d "{\"model\":\"$MODEL\",\"input\":\"workflow memory smoke test\"}")"
if [[ "$HTTP_STATUS" != 2* ]]; then
  printf 'LM Studio embeddings endpoint returned HTTP %s from %s\n' "$HTTP_STATUS" "$URL" >&2
  cat "$BODY" >&2
  exit 1
fi
python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); e=d.get("data",[{}])[0].get("embedding", []); print({"model": d.get("model"), "embedding_len": len(e), "accepted_halfvec_2560": len(e)==2560})' "$BODY"
