# llmtrack benchmarks

What llmtrack costs on top of a provider, measured: added latency, throughput,
time to first byte of a stream, and the CPU and memory of the gateway, side by
side with LiteLLM. Root context: `/CLAUDE.md`.

## How it measures

- **A mock provider** (`llmtrack-bench mock`) answers OpenAI chat completions
  instantly (or after `--latency-ms`), with usage, streaming or not. With the
  provider out of the way, the difference between calling it directly and
  calling it through a gateway is the gateway.
- **Closed-loop load** (`llmtrack-bench run`): N workers, each sends the next
  request when the previous one finished. One hdrhistogram per worker, merged
  at the end; no request is abandoned at the deadline. (Rustrak's runner
  shared one histogram behind `try_lock` and silently dropped samples.)
- **Resources** from `docker stats` (`--container`) or `ps` (`--pid`), sampled
  every 500 ms while the load runs.
- **Fair comparison** (`docker-compose.yml`): both gateways run in containers
  with the same CPU and memory limits, both authenticate a virtual key and log
  spend (LiteLLM with its Postgres), both proxy to the same mock.

## Commands

```bash
cargo test                                # unit + mock-backed load tests
bash scripts/compare.sh                   # full comparison → results/REPORT.md
SCENARIOS=chat CONCURRENCY="1 64" DURATION=30 bash scripts/compare.sh
MOCK_LATENCY_MS=300 bash scripts/compare.sh   # a provider that takes 300 ms

# Pieces by hand
cargo run --release -- mock --port 9000
cargo run --release -- run --label llmtrack --base-url http://localhost:4000/v1 \
  --api-key sk-... --model bench-model --scenario stream --concurrency 32 --pid $(pgrep llmtrack)
cargo run --release -- compare results/*.json --baseline direct
```

The crate stays out of the server's Cargo workspace, like Rustrak's, so its
dependencies never touch the server's lockfile.
