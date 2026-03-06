# Fast2Flow

Fast2Flow is a commercial Greentic routing extension implemented as a Rust workspace. It provides deterministic message-to-flow routing with optional LLM fallback, plus a gtpack integration boundary.

## Toolchain

- Rust `1.91.0` (pinned via `rust-toolchain.toml`)

## Quickstart

1. Build the workspace:

```bash
cargo build --all-features
```

2. Build an index from the sample flows:

```bash
cargo run -p fast2flow-cli -- index build \
  --scope tenant-a \
  --flows tests/fixtures/flows.json \
  --output /tmp/indexes
```

3. Verify the index exists and is readable:

```bash
cargo run -p fast2flow-cli -- index inspect \
  --scope tenant-a \
  --input /tmp/indexes
```

4. Run a local routing simulation:

```bash
cargo run -p fast2flow-cli -- route simulate \
  --scope tenant-a \
  --text "refund please" \
  --indexes-path /tmp/indexes
```

Expected result: a `Dispatch` directive targeting the refund flow from `tests/fixtures/flows.json`.

5. Start the host binary with a real hook request:

```bash
cat > /tmp/hook_request.json <<'JSON'
{
  "scope": "tenant-a",
  "envelope": {
    "text": "refund please",
    "channel": "web",
    "provider": "demo"
  },
  "session_active": false,
  "input_locale": "en-US",
  "time_budget_ms": 250,
  "registry_path": "/mnt/registry/latest.json",
  "indexes_path": "/tmp/indexes",
  "now_unix_ms": 0
}
JSON

FAST2FLOW_LLM_PROVIDER=disabled \
cargo run -p fast2flow-routing-gtpack --bin fast2flow-routing-host < /tmp/hook_request.json
```

6. Optional: enable policy overrides:

```bash
cargo run -p fast2flow-cli -- policy print-default > /tmp/fast2flow-policy.json
cargo run -p fast2flow-cli -- policy validate --file /tmp/fast2flow-policy.json

FAST2FLOW_POLICY_PATH=/tmp/fast2flow-policy.json \
FAST2FLOW_LLM_PROVIDER=disabled \
cargo run -p fast2flow-routing-gtpack --bin fast2flow-routing-host < /tmp/hook_request.json
```

7. Optional: enable an LLM provider:

- OpenAI:
  - Set `FAST2FLOW_LLM_PROVIDER=openai`.
  - Set `FAST2FLOW_OPENAI_API_KEY_PATH` to the secret key path.
  - Optionally set `FAST2FLOW_OPENAI_MODEL_PATH`.
- Ollama:
  - Set `FAST2FLOW_LLM_PROVIDER=ollama`.
  - Set `FAST2FLOW_OLLAMA_MODEL_PATH` to the secret key path containing the model name.
  - Optionally set `FAST2FLOW_OLLAMA_ENDPOINT_PATH`.

8. Build/package as a gtpack routing extension:

This is a **Greentic routing hook extension**.

- WIT package: `greentic:fast2flow`
- WIT world: `fast2flow-routing`
- Exported interface: `routing-hook.handle-hook`
- WIT source: `crates/fast2flow-routing-gtpack/wit/fast2flow.wit`

Runtime options:

- Component runtime: use the wasm entrypoint (`wit_entrypoint`) exported by `fast2flow-routing-gtpack`.
- Host process runtime: run `fast2flow-routing-host` and pass hook JSON on `stdin` / read directive JSON on `stdout`.

Local quality gate before packaging:

```bash
bash ci/local_check.sh
```

Pack build internals:

- `ci/build_gtpack.sh` now scaffolds and mutates pack sources via `greentic-pack` CLI commands.
- Generated pack sources declare a dependency on `routing.ingress.control.chain` and require capability `greentic.cap.ingress.control.v1`.
- Non-interactive wizard replay runs via `greentic-pack wizard run --answers ci/wizard/finalize.answers.json`.
- Flow replay runs via `greentic-flow wizard . --answers-file ci/wizard/flow.answers.json`.
- Component WASM defaults to an auto-build from `fast2flow-routing-gtpack` (`cargo build -p fast2flow-routing-gtpack --lib --target wasm32-wasip2 --release`).
- You can override the component WASM input with `FAST2FLOW_COMPONENT_WASM=/abs/path/to/fast2flow.wasm`.
- `FAST2FLOW_ALLOW_PLACEHOLDER_WASM=1` remains as a temporary local fallback only.
- Pack assembly normalizes the `fast2flow` component metadata to `version: <workspace version>` and `world: greentic:fast2flow/fast2flow-routing`.
- Build uses `greentic-pack build --allow-pack-schema` while component manifest wiring is being hardened.
- Routing-hook config schema is sourced from `ci/templates/routing-hook-fast2flow-config.schema.json`.

Pack replay validation test:

- `bash ci/test_gtpack_replay.sh` runs a full `ci/build_gtpack.sh` flow, exports generated pack sources, and asserts:
  - dependency declaration for `routing.ingress.control.chain` with required capability `greentic.cap.ingress.control.v1`
  - normalized component world `greentic:fast2flow/fast2flow-routing`
  - wizard replay trace file `wizard.finalize.applied.answers.json`
  - generated routing-hook schema location

Index/runtime deployment requirements (for either runtime mode):

- Mount registry at `/mnt/registry`.
- Mount indexes at `/mnt/indexes/<scope>/index.json` and `/mnt/indexes/<scope>/latest`.
- Set `FAST2FLOW_LLM_PROVIDER` (`disabled|openai|ollama`).
- Optionally set `FAST2FLOW_POLICY_PATH` and provider secret-path env vars.

Publish behavior in this repository:

- Push to `master` triggers `.github/workflows/publish.yml`.
- That workflow creates a GitHub Release and uploads platform binaries.
- That workflow also publishes `fast2flow.gtpack` to GHCR at:
  - `ghcr.io/<owner>/providers/routing-hook/fast2flow.gtpack:v<version>`
  - `ghcr.io/<owner>/providers/routing-hook/fast2flow.gtpack:latest`

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
- It also publishes a bundled `fast2flow.gtpack` OCI artifact to GHCR:
  - `ghcr.io/<owner>/providers/routing-hook/fast2flow.gtpack:v<version>`
  - `ghcr.io/<owner>/providers/routing-hook/fast2flow.gtpack:latest`
