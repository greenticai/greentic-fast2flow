# Fast2Flow

Fast2Flow is a Greentic routing extension that decides which flow should handle an incoming message.

In plain terms:

- a user sends a message such as "refund please"
- Fast2Flow looks at the flows available for that tenant
- it picks the best matching flow
- it returns a routing decision such as "send this to the refund flow"

The project is implemented as a Rust workspace, but the product goal is simple: route incoming messages to the right flow quickly, predictably, and safely.

## What Problem It Solves

Without Fast2Flow, every incoming message must be handled by hand-written routing logic or sent directly to a larger decision system.

Fast2Flow gives you a middle layer that:

- uses deterministic matching first, so common cases stay fast and explainable
- supports tenant-specific indexes, so each tenant routes against its own flows
- can fall back to an LLM only when needed
- fails open to `Continue` when it cannot make a safe decision

This makes it useful for cases like:

- customer support message routing
- internal request triage
- directing messages into known Greentic flows

## Core Concepts

You only need four ideas to understand Fast2Flow:

1. A `flow` is something Fast2Flow can route to, such as `support/refund_flow`.
2. A `scope` is the routing boundary, usually a tenant or environment, such as `tenant-a`.
3. An `index` is the searchable snapshot of the flows available in a scope.
4. A `directive` is Fast2Flow's answer back to the caller.

Possible directives are:

- `Dispatch`: route to a specific flow
- `Respond`: return a fixed response immediately
- `Deny`: block routing because policy says not to continue
- `Continue`: do nothing and let the caller decide what happens next

## How Fast2Flow Works

When a message comes in, Fast2Flow runs this pipeline:

1. Policy and hook checks run first.
   This can deny a request early, return a fixed response, or allow routing to continue.
2. Fast2Flow loads the index for the request scope.
   The index contains the flows that are valid for that tenant or routing scope.
3. The deterministic strategy scores likely matches.
   This is the normal path and is intended to handle the majority of traffic.
4. If the top match is confident enough, Fast2Flow returns `Dispatch`.
5. If the deterministic result is not strong enough, Fast2Flow can optionally ask an LLM for help.
6. If no safe answer is available, Fast2Flow returns `Continue`.

The important design rule is that Fast2Flow prefers predictable routing first and only uses the LLM as a fallback.

## Typical Request Lifecycle

Example:

1. You define a flow called `support/refund_flow`.
2. You build an index for scope `tenant-a`.
3. A message arrives with text `refund please`.
4. Fast2Flow checks tenant-a's index.
5. It sees that the refund flow is the best match.
6. It returns a directive telling the caller to dispatch to `support/refund_flow`.

If the message is unclear, such as `help`, the deterministic stage may not be confident enough. In that case Fast2Flow either asks the configured LLM for a better guess or returns `Continue`, depending on runtime configuration and thresholds.

## Two Ways To Run It

There are two common ways to use Fast2Flow:

1. Developer CLI
   Use this to build indexes, inspect them, validate policy files, and simulate routing locally.
2. Routing host / gtpack integration
   Use this when Fast2Flow is running as part of a real Greentic routing setup.

If you are new to the project, start with the CLI because it makes the routing behavior easier to see and test.

### Greentic-X Runner Component Operation

For Greentic-X runner dispatch, use the packaged Greentic component `fast2flow.router` with operation `route-intent`.
That operation accepts the Greentic-X `Fast2FlowRouteRequest` JSON shape, loads the mounted index from `indexes_path`, runs deterministic Fast2Flow routing, and returns the Greentic-X `Fast2FlowRouteResult` JSON shape.

The legacy `route` operation remains available for flows that already call matcher first and pass a precomputed `match_result`.

The router can also be packaged as a direct OCI component for `greentic-distributor-client` / `greentic-component-runner` consumption:

```bash
bash scripts/build_components.sh
bash scripts/package_components.sh
# optional publish step for registry owners
bash scripts/publish_components.sh
```

This publishes/resolves the component reference declared in `components/manifest.json`, for example `oci://ghcr.io/greenticai/components/fast2flow/fast2flow-router:latest`.

Required runtime inputs for `route-intent`:

- `scope`: index scope to load.
- `envelope.text`: user message to route.
- `indexes_path`: root containing Fast2Flow indexes.
- `time_budget_ms`: non-zero routing budget.

## Toolchain

- Rust `1.95.0` (pinned via `rust-toolchain.toml`)

## Quickstart

This quickstart is the easiest way to see how Fast2Flow works end to end.

1. Build the workspace:

```bash
cargo build --all-features
```

2. Build an index from the sample flows.

This creates the searchable routing data for scope `tenant-a`.

```bash
cargo run -p greentic-fast2flow -- index build \
  --scope tenant-a \
  --flows tests/fixtures/flows.json \
  --output /tmp/indexes
```

3. Verify the index exists and is readable:

```bash
cargo run -p greentic-fast2flow -- index inspect \
  --scope tenant-a \
  --input /tmp/indexes
```

4. Run a local routing simulation.

This asks Fast2Flow: "If a tenant-a user says `refund please`, where should it go?"

```bash
cargo run -p greentic-fast2flow -- route simulate \
  --scope tenant-a \
  --text "refund please" \
  --indexes-path /tmp/indexes
```

Expected result: a `Dispatch` directive targeting the refund flow from `tests/fixtures/flows.json`.

5. Start the host binary with a real hook request.

This uses the same routing logic, but through the host binary that a real integration can call.

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
cargo run -p fast2flow-routing-gtpack --bin greentic-fast2flow-routing-host < /tmp/hook_request.json
```

6. Optional: enable policy overrides.

Policies let you control routing behavior without changing code.

```bash
cargo run -p greentic-fast2flow -- policy print-default > /tmp/fast2flow-policy.json
cargo run -p greentic-fast2flow -- policy validate --file /tmp/fast2flow-policy.json

FAST2FLOW_POLICY_PATH=/tmp/fast2flow-policy.json \
FAST2FLOW_LLM_PROVIDER=disabled \
cargo run -p fast2flow-routing-gtpack --bin greentic-fast2flow-routing-host < /tmp/hook_request.json
```

7. Optional: enable an LLM provider.

Use this only when deterministic matching is not enough for your use case.

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
- Host process runtime: run `greentic-fast2flow-routing-host` and pass hook JSON on `stdin` / read directive JSON on `stdout`.

## What You Need In Production

To run Fast2Flow in a real environment, you typically need:

- a registry mounted at `/mnt/registry`
- built indexes mounted at `/mnt/indexes/<scope>/...`
- a chosen LLM mode via `FAST2FLOW_LLM_PROVIDER`
- an optional routing policy file

In other words, production use is:

1. define your flows
2. build indexes for each scope
3. mount those indexes where the runtime can read them
4. send Fast2Flow a hook request
5. act on the returned directive

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
  - wizard replay trace file `wizard.launcher.applied.answers.json`
  - generated routing-hook schema location

Index/runtime deployment requirements (for either runtime mode):

- Mount registry at `/mnt/registry`.
- Mount indexes at `/mnt/indexes/<scope>/index.json` and `/mnt/indexes/<scope>/latest`.
- Set `FAST2FLOW_LLM_PROVIDER` (`disabled|openai|ollama`).
- Optionally set `FAST2FLOW_POLICY_PATH` and provider secret-path env vars.

Publish behavior in this repository:

- Push to `master` triggers `.github/workflows/ci.yml` release jobs.
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
- `cli/fast2flow-cli`: developer CLI binary `greentic-fast2flow` (`index build|inspect`, `route simulate`).
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

## Policy Model

Policies are how you change routing behavior without rebuilding the application.

They can be used to:

- set confidence thresholds
- limit candidate counts
- allow or deny certain scopes, channels, or providers
- define direct response rules
- override behavior for specific routing situations

This is useful when a tenant needs stricter controls or different routing behavior than the default.

## Developer CLI

```bash
cargo run -p greentic-fast2flow -- index build --scope tenant-a --flows tests/fixtures/flows.json --output /tmp/indexes
cargo run -p greentic-fast2flow -- index inspect --scope tenant-a --input /tmp/indexes
cargo run -p greentic-fast2flow -- route simulate --scope tenant-a --text "refund please" --indexes-path /tmp/indexes
cargo run -p greentic-fast2flow -- policy print-default
cargo run -p greentic-fast2flow -- policy validate --file /tmp/fast2flow-policy.json
```

## Host Bootstrap

`fast2flow-routing-gtpack` provides host bootstrap helpers:

- `HostRuntime::boot_from_env()` builds router strategy/filter/optional LLM from environment.
- `HostRuntime::route_from_mounts(...)` executes routing using mounted indexes.

A host binary entrypoint is included:

```bash
cargo run -p fast2flow-routing-gtpack --bin greentic-fast2flow-routing-host < hook_request.json
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
- CI enforces `publish = false` for all workspace crates via `ci/publish_dry_run.sh`.
- On `master` push, workflow ensures tag `v<version>` exists and creates/updates the GitHub release.
- It builds and uploads versioned binary archives for both executables:
  - `greentic-fast2flow-v<version>-<target>.(tar.gz|zip)`
  - `greentic-fast2flow-routing-host-v<version>-<target>.(tar.gz|zip)`
  - targets: Linux (`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`), macOS 15 (`x86_64-apple-darwin`, `aarch64-apple-darwin`), Windows (`x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`)
- `cargo-binstall` can install from release assets with explicit URL format, for example:
  - `cargo binstall greentic-fast2flow --version <version> --pkg-url "https://github.com/<owner>/greentic-fast2flow/releases/download/v<version>/greentic-fast2flow-v<version>-<target>.<archive-format>"`
  - `cargo binstall greentic-fast2flow-routing-host --version <version> --pkg-url "https://github.com/<owner>/greentic-fast2flow/releases/download/v<version>/greentic-fast2flow-routing-host-v<version>-<target>.<archive-format>"`
- It also publishes a bundled `fast2flow.gtpack` OCI artifact to GHCR:
  - `ghcr.io/<owner>/providers/routing-hook/fast2flow.gtpack:v<version>`
  - `ghcr.io/<owner>/providers/routing-hook/fast2flow.gtpack:latest`
