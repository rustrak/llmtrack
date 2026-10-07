#!/usr/bin/env bash
# Benchmarks llmtrack against LiteLLM, both in front of the same mock
# provider, and writes results/REPORT.md.
#
#   SCENARIOS="chat stream" CONCURRENCY="1 16 64" DURATION=20 bash scripts/compare.sh
set -euo pipefail
cd "$(dirname "$0")/.."

SCENARIOS=${SCENARIOS:-"chat stream"}
CONCURRENCY=${CONCURRENCY:-"1 16 64"}
DURATION=${DURATION:-20}
OUT=${OUT:-results/$(date +%Y%m%d-%H%M%S)}
BENCH=target/release/llmtrack-bench

cargo build --release --quiet
docker compose up -d --build

wait_for() {
  for _ in $(seq 1 120); do curl -fsS "$1" >/dev/null 2>&1 && return 0; sleep 1; done
  echo "timed out waiting for $1" >&2; exit 1
}
wait_for http://localhost:9000/health
wait_for http://localhost:4100/health/ready
wait_for http://localhost:4200/health/liveliness

# llmtrack: a model on the mock, a team, a key.
jar=$(mktemp)
curl -fsS -c "$jar" -H 'content-type: application/json' \
  -d '{"email":"bench@example.com","password":"benchmark-password"}' http://localhost:4100/auth/login >/dev/null
curl -sS -b "$jar" -H 'content-type: application/json' -d '{"name":"bench-model","provider":"openai_compatible",
  "upstream_model":"bench-model","api_base":"http://mock:9000/v1","pricing":{"input":1,"output":2}}' \
  http://localhost:4100/api/models >/dev/null
team=$(curl -fsS -b "$jar" -H 'content-type: application/json' -d '{"name":"bench"}' http://localhost:4100/api/teams \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')
LLMTRACK_KEY=$(curl -fsS -b "$jar" -H 'content-type: application/json' -d "{\"team_id\":$team,\"name\":\"bench\"}" \
  http://localhost:4100/api/keys | python3 -c 'import json,sys; print(json.load(sys.stdin)["key"])')

# LiteLLM: a virtual key, so it authenticates and logs spend like llmtrack.
LITELLM_KEY=$(curl -fsS -H 'Authorization: Bearer sk-bench-master' -H 'content-type: application/json' \
  -d '{"models":["bench-model"]}' http://localhost:4200/key/generate \
  | python3 -c 'import json,sys; print(json.load(sys.stdin)["key"])')

for scenario in $SCENARIOS; do
  for c in $CONCURRENCY; do
    $BENCH run --label direct   --base-url http://localhost:9000/v1 --scenario "$scenario" --concurrency "$c" \
      --duration-secs "$DURATION" --container bench-mock --output "$OUT"
    $BENCH run --label llmtrack --base-url http://localhost:4100/v1 --api-key "$LLMTRACK_KEY" --scenario "$scenario" \
      --concurrency "$c" --duration-secs "$DURATION" --container bench-llmtrack --output "$OUT"
    $BENCH run --label litellm  --base-url http://localhost:4200/v1 --api-key "$LITELLM_KEY" --scenario "$scenario" \
      --concurrency "$c" --duration-secs "$DURATION" --container bench-litellm --output "$OUT"
  done
done

idle() { docker stats --no-stream --format '{{.Name}} {{.MemUsage}}' bench-llmtrack bench-litellm; }
{
  $BENCH compare "$OUT"/*.json --baseline direct
  echo "## Memory after the run"
  echo
  echo '```'
  idle
  echo '```'
} > "$OUT/REPORT.md"
cp "$OUT/REPORT.md" results/REPORT.md
echo "report: $OUT/REPORT.md"
[ "${KEEP:-0}" = 1 ] || docker compose down -v
