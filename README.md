<div align="center">

# Mycelix — Agentic Orchestrator (Aggregator)

**v0.0.1 · Rust rewrite**

</div>

Mycelix (UI wordmark **M Y C**) is being migrated from a Node/Next.js router/proxy into a Rust **agentic
orchestrator**: instead of forwarding a request 1:1 to one provider, the
orchestrator *aggregates* across many providers using strategies — **fallback**,
**round-robin**, and **fusion** (fan-out to a panel + judge synthesis).

This repository root is a Cargo workspace. The previous Next.js application is
archived, intact, under [`legacy/`](./legacy).

## Workspace

| Crate | Module | Responsibility |
|-------|--------|----------------|
| [`crates/aggregator`](./crates/aggregator) | **myc-core** | `Provider` trait + `Orchestrator` (fallback / round-robin / fusion). Pure, no HTTP. |
| [`crates/backend`](./crates/backend) | **myc-node** | Axum server: `/health`, `/version`, `/v1/models`, streaming `/v1/chat/completions`. `OpenAiCompatibleProvider` (reqwest). Binary: `myc`. |
| [`crates/frontend`](./crates/frontend) | **myc-web** | Leptos (CSR) status + test UI. |

## Configure

Providers and combos live in [`config.toml`](./config.toml). Keys come from env
vars (`api_key_env`) — never commit literals.

```toml
[[providers]]
id = "openai"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
models = [{ id = "gpt-4o-mini" }]

[[combos]]
name = "free-fallback"
strategy = "fallback"          # or "round_robin" (with `sticky`) / "fusion"
targets = [{ provider = "openai", model = "gpt-4o-mini" }, ...]
```

## Build & run

```bash
# core tests (fallback / round-robin / fusion)
cargo test -p myc-core

# backend (defaults to :20130; honors CONFIG_PATH) — also serves the dashboard
export OPENAI_API_KEY=sk-...
cargo run -p myc-node
# → dashboard at http://localhost:20130/dashboard

# frontend (Leptos) — build the dashboard the backend serves, or hot-reload it
cargo install trunk
cd crates/frontend && trunk build        # emits dist/ served at /dashboard
cd crates/frontend && trunk serve        # dev UI on :8080, proxies the API to the backend
```

### API

```bash
curl localhost:20127/v1/models
curl -N -X POST localhost:20127/v1/chat/completions \
  -H 'content-type: application/json' \
  -d '{"model":"free-fallback","messages":[{"role":"user","content":"hi"}],"stream":true}'
```

The response carries `x_myc_target` (the provider/model that answered) and
`x_myc_fused` (true when synthesized by a fusion judge).

## Status / scope

Done in this slice: the orchestrator (all three strategies, unit-tested), the
Axum backend (streaming OpenAI-compatible API), and a Leptos frontend.

Deferred (tracked): SQLite/DB, OAuth + auth-gated dashboard routes, cross-format
translation (Claude/Gemini), RTK/token-savers, per-provider executors beyond
OpenAI-compatible. The legacy Node app under `legacy/` remains the reference.

## License

MIT
