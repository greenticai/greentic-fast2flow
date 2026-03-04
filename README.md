# Fast2Flow

Fast2Flow is a commercial Greentic routing extension implemented as a Rust workspace. It provides deterministic message-to-flow routing with optional LLM fallback, plus a gtpack integration boundary.

## Toolchain

- Rust `1.91.0` (pinned via `rust-toolchain.toml`)

## Workspace Layout

- `crates/fast2flow-contracts`: shared routing and index contracts.
- `crates/fast2flow-core`: routing orchestration pipeline.
- `crates/fast2flow-strategy`: strategy traits and scoring utilities.
- `crates/fast2flow-strategy-phase1`: deterministic phase-1 strategy implementation.
- `crates/fast2flow-indexer`: index build/load/query utilities.
- `crates/fast2flow-hooks`: default hook filter policies.
- `crates/fast2flow-llm`: provider interface.
- `crates/fast2flow-llm-openai`: OpenAI adapter.
- `crates/fast2flow-llm-ollama`: Ollama adapter.
- `crates/fast2flow-routing-gtpack`: Greentic-specific routing extension layer.
- `cli/fast2flow-cli`: developer CLI (`index build|inspect`, `route simulate`).
  - Policy tooling: `policy validate`, `policy print-default`.

## Routing Contract

Input contract: `Fast2FlowHookInV1`

- `scope`
- `envelope`
- `session_active`
- `input_locale`
- `time_budget_ms`
- `registry_path`
- `indexes_path`
- `now_unix_ms`

Output contract: `Fast2FlowHookOutV1` with `RoutingDirective`

- `Continue`
- `Dispatch { target, confidence, reason }`
- `Respond { message }`
- `Deny { reason }`

## Routing Pipeline

1. Hook filter
2. Index query
3. Deterministic strategy
4. Confidence threshold
5. Optional LLM fallback
6. Directive output

LLM fallback is optional. Timeout/unavailable states fail open to `Continue`.

## Developer CLI

```bash
cargo run -p fast2flow-cli -- index build --scope tenant-a --flows tests/fixtures/flows.json --output /tmp/indexes
cargo run -p fast2flow-cli -- index inspect --scope tenant-a --input /tmp/indexes
cargo run -p fast2flow-cli -- route simulate --scope tenant-a --text "refund please" --indexes-path /tmp/indexes
cargo run -p fast2flow-cli -- policy print-default
cargo run -p fast2flow-cli -- policy validate --file /tmp/fast2flow-policy.json
```

## Host Bootstrap

`fast2flow-routing-gtpack` provides host bootstrap helpers:

- `HostRuntime::boot_from_env()` builds router strategy/filter/optional LLM from environment.
- `HostRuntime::route_from_mounts(...)` executes routing using mounted indexes.

A host binary entrypoint is included:

```bash
cargo run -p fast2flow-routing-gtpack --bin fast2flow-routing-host < hook_request.json
```

Relevant env vars:

- `FAST2FLOW_LLM_PROVIDER` (`disabled` | `openai` | `ollama`)
- `FAST2FLOW_OPENAI_API_KEY_PATH`
- `FAST2FLOW_OPENAI_MODEL_PATH`
- `FAST2FLOW_OLLAMA_ENDPOINT_PATH`
- `FAST2FLOW_OLLAMA_MODEL_PATH`
- `FAST2FLOW_POLICY_PATH` (optional, defaults to `/mnt/registry/fast2flow-policy.json`)
- `FAST2FLOW_TRACE_POLICY` (`1|true|yes` prints policy resolution trace JSON to stderr in host binary)

Policy file supports default + scope/channel/provider overrides for:

- `min_confidence`
- `llm_min_confidence`
- `candidate_limit`
- allow/deny channel/provider/scope lists
- `respond_rules` (`needle`, `message`, `mode: contains|exact|regex`)
- override metadata (`id`, `priority`) and `stage_order` precedence

Policy validation is enforced both in runtime bootstrap and via CLI `policy validate`.

## CI and Release

Run local checks:

```bash
bash ci/local_check.sh
```

Release workflow is artifact-only (no crates.io publishing) and runs on `master` pushes.

- Source of truth version: `workspace.package.version` in root `Cargo.toml`.
- Authoritative crate key: `workspace.metadata.fast2flow.authoritative_crate`.
- On `master` push, workflow ensures tag `v<version>` exists and creates/updates the GitHub release.
- It builds and uploads 6 artifacts:
  - Linux: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` (`.tar.gz`)
  - macOS 15: `x86_64-apple-darwin`, `aarch64-apple-darwin` (`.tar.gz`)
  - Windows: `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` (`.zip`)
