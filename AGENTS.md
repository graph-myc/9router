# AGENTS.md — 9router → Mycelix migration

> Read this first. Applies to Claude Code, GitHub Copilot, and any AI agent working in this repo.

## Goal

Rewrite the legacy Next.js **9router** as a Rust application branded **Mycelix** (UI wordmark **M Y C**), and bring the
Rust app to **full feature parity** with the legacy app. Work proceeds **one feature at a time**,
testing each ported feature against the running legacy app. **Once parity is reached, the
`legacy/` folder is deleted.**

- Brand wordmark renders as `M Y C` (spaced) everywhere user-facing. Do not reintroduce "9Router".
- Persistence stays JSON (`state.json`), not SQLite. DB export/import = whole-state serialize.
- Provider scope: OpenAI-compatible **plus** Anthropic + Gemini format translation.
- Delivery: **phased, one feature group per commit**, build + verify each before moving on.

## Architecture

Rust Cargo workspace (`Cargo.toml`):

- `crates/backend` (package `myc-node`, binary `myc`) — Axum HTTP server. Serves the dashboard at `/` (root), OpenAI-compatible API
  under `/v1`, management API under `/api`. Entry: `crates/backend/src/main.rs`.
  - `state.rs` — mutable app state + `state.json` persistence + orchestrator rebuild.
  - `provider_openai.rs` — OpenAI-compatible provider HTTP client (chat once/stream, model fetch, quota capture).
  - `config.rs` — `config.toml` seed config. `logbuf.rs` — tracing ring buffer → SSE.
- `crates/aggregator` (package `myc-core`) — routing engine. `orchestrator.rs` strategies: fallback, round-robin, fusion.
- `crates/frontend` (package `myc-web`) — Leptos 0.7 (CSR/WASM) dashboard. Single file: `crates/frontend/src/main.rs`.
  Built with `trunk` → `crates/frontend/dist`, served at the site root (see `Trunk.toml` `public_url = "/"`).
- `legacy/` — the original Next.js app, kept only as the parity reference. **Do not add features here.**

## Ports

| App | Port | URL |
|-----|------|-----|
| **Rust migration (Mycelix)** | **20130** | http://localhost:20130/ |
| **Legacy (Next.js, reference)** | **20129** | http://localhost:20129 |

Legacy requires login by default; password is `123456`.

## Build & run

### Rust app (port 20130)

```powershell
# IMPORTANT: stop the running server before rebuilding (Windows locks myc.exe).
Get-Process myc -ErrorAction SilentlyContinue | Stop-Process -Force

# Backend (package myc-node, binary myc)
cargo build -p myc-node

# Frontend (WASM dashboard, package myc-web)
cd crates\frontend; trunk build; cd ..\..

# Run (serves dashboard + API on :20130)
Start-Process -FilePath ".\target\debug\myc.exe" -WindowStyle Hidden
```

### Legacy app (port 20129, reference only)

```powershell
cd legacy
npm install            # first time only
npm run build          # first time / after legacy changes
$env:PORT=20129; .\node_modules\.bin\next start -p 20129
```

## Testing workflow (one feature at a time)

For each feature being ported:

1. **Observe legacy** behavior at `http://localhost:20129` (UI page + its `/api/...` calls).
2. **Implement** the equivalent in the Rust app (backend route + Leptos view).
3. **Build** backend and frontend (commands above). Always stop `backend.exe` first.
4. **Verify** the Rust app on `:20130`:
   - Probe new endpoints, e.g.
     `Invoke-WebRequest http://localhost:20130/api/<route> -UseBasicParsing`
   - Smoke-test the dashboard at `http://localhost:20130/` (hash routes like `/#/providers`).
   - Compare output/shape against the legacy endpoint on `:20129`.
5. **Add tests** where logic lives in a crate (`cargo test -p aggregator`, etc.).
6. **Commit** the feature group with a conventional message (`feat(...)`, `fix(...)`).

Verification baseline (must stay green): `http://localhost:20130/` → 200,
`http://localhost:20130/v1/models` → 200.

## Phased plan (A → L)

Tracked in detail in session memory `/memories/session/plan.md`. Summary:

- **A** Expand state/persistence + `/api/settings` + DB export/import.
- **B** Auth & login gate (password + JWT, localhost/CLI token, optional OIDC).
- **C** API Keys UI + Settings UI.
- **D** Models management (aliases/custom/disabled) + capability-aware routing.
- **E** Pricing + Usage analytics (cost, history, chart, details, by-provider, live stream).
- **F** Provider enhancements (validate/batch-test/suggested) + provider-nodes + outbound proxy.
- **G** Format translation engine (new `crates/translator`) + Anthropic/Gemini endpoints.
- **H** Media APIs (embeddings, image, TTS, STT, search, web-fetch).
- **I** Proxy pools.
- **J** Translator / request inspector UI.
- **K** CLI tools, skills, tags, locale, version/update/shutdown, health/init.
- **L** Heavy host-specific: tunnels, MITM, OAuth imports, headroom/RTK/caveman/ponytail (highest risk, last).

Core user value lands by Phase F. Phase L depends on external binaries/credentials and may be partial.

## Conventions & gotchas

- Stop `myc.exe` before any `cargo build` (Windows file lock, error 5).
- Frontend asset URLs are root-relative via `Trunk.toml` `public_url = "/"`; the backend serves the SPA at `/` with a `fallback_service`.
- `state.json` is gitignored (may contain provider API keys). Never commit it.
- Chat passthrough headers use `x_myc_*` (renamed from `x_9router_*`).
- Reuse existing patterns: `OpenAiCompatibleProvider`, orchestrator strategy enum, `LogBuffer`
  broadcast → SSE, frontend `fetch_json`/`post_json`/`delete_req` helpers, the `ProviderCard` view.
- Keep changes surgical and scoped to the current phase; one feature group per commit.
