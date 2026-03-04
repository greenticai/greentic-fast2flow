# Repository Overview

## 1. High-Level Purpose
This repository implements Fast2Flow as a commercial Greentic routing extension in a Rust workspace. It provides a deterministic routing pipeline that maps incoming hook requests to routing directives (`Continue`, `Dispatch`, `Respond`, `Deny`) using filtering, indexed candidate lookup, strategy scoring, confidence thresholds, and optional LLM fallback.

The codebase is split into domain contracts, runtime-independent orchestration, pluggable strategy/filter/index layers, optional provider adapters (OpenAI/Ollama), and a Greentic integration boundary crate. Developer operations are standardized through `ci/local_check.sh` and GitHub workflows.

## 2. Main Components and Functionality
- **Path:** `Cargo.toml`
- **Role:** Workspace root manifest and shared dependency/version policy.
- **Key functionality:**
  - Defines workspace members for all Fast2Flow crates and CLI.
  - Uses lockstep versioning via `[workspace.package]` (`version = 0.4.0`, `edition = 2021`, `rust-version = 1.91`, `license = Commercial`).
  - Centralizes shared dependencies in `[workspace.dependencies]`.
  - Uses `greentic-secrets-lib = 0.4` from crates.io for adapter secrets integration.
  - Defines release authority metadata at `workspace.metadata.fast2flow.authoritative_crate`.
- **Key dependencies / integration points:**
  - All crates consume workspace version/edition/license and shared dependencies.

- **Path:** `crates/fast2flow-contracts`
- **Role:** Shared schemas and routing contracts.
- **Key functionality:**
  - Defines `Fast2FlowHookInV1`, `Fast2FlowHookOutV1`, `RoutingDirective`.
  - Defines index/flow types (`FlowDoc`, `IndexManifestV1`, `IndexEntryV1`) and scoring types (`Candidate`, `Decision`).
  - Defines policy model types (`RoutingPolicyV1`, `PolicyRuleV1`, scope/channel/provider overrides, `RespondRuleV1`).
  - Defines execution diagnostics types (`PolicyAppliedRuleV1`, `PolicyResolutionV1`, `PolicyEffectiveConfigV1`, `RoutingExecutionTraceV1`).
- **Key dependencies / integration points:**
  - Imported across all other Fast2Flow crates.

- **Path:** `crates/fast2flow-core`
- **Role:** Runtime-independent routing orchestrator.
- **Key functionality:**
  - Exposes `CoreRouter::route` implementing pipeline:
    - hook filter evaluation
    - candidate index search
    - deterministic strategy evaluation
    - confidence gating
    - optional LLM fallback
    - directive generation
  - Enforces request time budget and fail-open behavior (`Continue`) when exhausted or fallback fails.
  - Supports filter short-circuit directives for `Respond` and `Deny`.
  - Uses trait-based index abstraction (`CandidateIndex`) and optional trait-object LLM provider.
- **Key dependencies / integration points:**
  - Depends on `fast2flow-hooks`, `fast2flow-strategy`, `fast2flow-llm`, and contracts.

- **Path:** `crates/fast2flow-strategy`
- **Role:** Strategy abstraction and scoring helpers.
- **Key functionality:**
  - Defines `RoutingStrategy` trait (`evaluate(query, candidates) -> Option<Decision>`).
  - Provides tokenization and token-similarity helpers.

- **Path:** `crates/fast2flow-strategy-phase1`
- **Role:** Deterministic phase-1 strategy implementation.
- **Key functionality:**
  - Implements token-based ranking and deterministic tie-breaks.
  - Produces confidence-scored `Decision` values.

- **Path:** `crates/fast2flow-indexer`
- **Role:** Index manifest build/load/query.
- **Key functionality:**
  - Builds `IndexManifestV1` from `FlowDoc` entries.
  - Writes scope indexes to `<indexes_root>/<scope>/index.json` and `latest` with atomic rename update.
  - Loads `latest` manifest and performs overlap-based text search returning ranked candidates.
- **Key dependencies / integration points:**
  - Consumed by CLI/tests and by core via adapter implementing `CandidateIndex`.

- **Path:** `crates/fast2flow-hooks`
- **Role:** Hook filtering rules.
- **Key functionality:**
  - Defines filter trait and outcomes (`Proceed`, `Continue`, `Respond`, `Deny`).
  - Default filter supports session-active short-circuit and allow/deny lists for scope/channel/provider.
  - Default filter supports immediate response rules (`respond_rules`) with `contains`, `exact`, and `regex` text matching modes.

- **Path:** `crates/fast2flow-llm`
- **Role:** Provider-agnostic LLM contract.
- **Key functionality:**
  - Defines async `LlmProvider` trait with timeout-aware completion.
  - Defines strict response envelope (`LlmResponse`) and error taxonomy (`LlmError`).

- **Path:** `crates/fast2flow-llm-openai`
- **Role:** OpenAI adapter.
- **Key functionality:**
  - Calls OpenAI chat completion endpoint.
  - Requests JSON-only output and parses structured routing response.
  - Enforces hard timeout via `tokio::time::timeout`.
  - Adds secrets-backed constructor (`from_secrets`) using `greentic-secrets-lib` environment manager.

- **Path:** `crates/fast2flow-llm-ollama`
- **Role:** Ollama adapter.
- **Key functionality:**
  - Calls local Ollama HTTP API with JSON response format.
  - Parses structured routing response.
  - Enforces hard timeout via `tokio::time::timeout`.
  - Adds secrets-backed constructor (`from_secrets`) using `greentic-secrets-lib` environment manager.

- **Path:** `crates/fast2flow-routing-gtpack`
- **Role:** Greentic integration boundary.
- **Key functionality:**
  - Exposes `handle_hook` bridge from hook input to core router output.
  - Exposes `handle_hook_from_mounts` that loads scope index from request mount path and routes through core.
  - Includes `MountedIndexLookup` adapter implementing `CandidateIndex` over loaded index snapshots.
  - Adds environment-driven bootstrap (`RouterBootstrapConfig`, `build_router_from_env`, `build_router_from_config`) for host startup.
  - Adds `HostRuntime` bootstrap abstraction for deployed host processes.
  - Loads optional policy JSON from `FAST2FLOW_POLICY_PATH` (or `/mnt/registry/fast2flow-policy.json`) and applies scope/channel/provider overrides per request.
  - Validates loaded policy data before use (stage-order integrity, confidence ranges, candidate-limit bounds, non-empty override identifiers/keys, and regex validity for regex respond rules).
  - Merges policy overrides into router thresholds and hook filter settings before routing execution, with configurable stage precedence (`stage_order`) and per-override `priority`.
  - Exposes `route_from_mounts_with_trace` returning output + policy/directive trace payload.
  - Detects and records override conflicts (field overwrite warnings) during policy resolution, including override source labels and IDs.
  - Supports optional LLM backend selection from environment:
    - disabled
    - OpenAI via secrets paths
    - Ollama via secrets paths
  - Declares mount constants for `/mnt/registry` and `/mnt/indexes`.
  - Includes WIT world definitions (`wit/fast2flow.wit`) and `wit-bindgen` generated bindings under `wasm32`.
  - Implements generated guest entrypoint wiring with type mapping plus installable mounted runtime (`install_mounted_runtime`, `install_mounted_runtime_from_env`).
- **Key dependencies / integration points:**
  - Intentionally the only crate designated for Greentic-specific boundary logic.

- **Path:** `crates/fast2flow-routing-gtpack/src/bin/fast2flow-routing-host.rs`
- **Role:** Host process entrypoint for startup and routing execution.
- **Key functionality:**
  - Boots `HostRuntime` from environment.
  - Reads `Fast2FlowHookInV1` JSON from stdin.
  - Executes mount-backed routing and prints `Fast2FlowHookOutV1` JSON.
  - Optionally emits policy trace JSON to stderr when `FAST2FLOW_TRACE_POLICY` is enabled.

- **Path:** `cli/fast2flow-cli`
- **Role:** Developer tooling.
- **Key functionality:**
  - `index build`: builds index from flow docs JSON.
  - `index inspect`: reads latest index summary.
  - `route simulate`: runs routing simulation against loaded index.
  - `policy validate --file <path>`: validates policy JSON using runtime validator (same checks used by host bootstrap).
  - `policy print-default`: prints a valid baseline `RoutingPolicyV1` JSON template.

- **Path:** `crates/fast2flow-core/tests/routing_pipeline.rs`, `tests/fixtures/flows.json`
- **Role:** Integration coverage for routing behavior.
- **Key functionality:**
  - Validates deterministic routing dispatch (`refund please` -> `support/refund_flow`).
  - Validates LLM fallback dispatch on deterministic miss.
  - Validates fail-open `Continue` on LLM timeout.
  - Validates filter-based `Respond` and `Deny` short-circuit directives.
  - Validates session-active and zero-budget continue behavior.

- **Path:** `crates/fast2flow-routing-gtpack/tests/mounted_runtime.rs`
- **Role:** Integration coverage for mount-based gtpack runtime wiring.
- **Key functionality:**
  - Validates dispatch via `handle_hook_from_mounts` with on-disk index.
  - Validates fail-open `Continue` when mounted index is missing.

- **Path:** `crates/fast2flow-routing-gtpack/tests/bootstrap.rs`
- **Role:** Integration coverage for host bootstrap configuration.
- **Key functionality:**
  - Validates default router build with LLM disabled.
  - Validates OpenAI secrets-based bootstrap path.
  - Validates environment parsing constraints (e.g., Ollama model secret required).

- **Path:** `crates/fast2flow-routing-gtpack/tests/host_startup_e2e.rs`
- **Role:** End-to-end smoke coverage for host startup path.
- **Key functionality:**
  - Boots runtime from environment and routes using mounted index artifacts.
  - Validates error behavior for unsupported LLM provider configuration.

- **Path:** `crates/fast2flow-routing-gtpack/tests/policy_overrides.rs`
- **Role:** Integration coverage for policy-driven runtime overrides.
- **Key functionality:**
  - Validates policy file loading/parsing from disk.
  - Validates scope-level confidence override behavior.
  - Validates channel-level respond-rule override behavior.
  - Validates trace warnings for policy field overwrite conflicts and final effective values with staged precedence.
  - Validates stage-order precedence behavior and per-stage priority ordering.
  - Validates policy validation failure path for invalid regex rules.

- **Path:** `crates/fast2flow-hooks/tests/respond_match_modes.rs`
- **Role:** Unit coverage for respond-rule text matching.
- **Key functionality:**
  - Verifies `contains` matching is case-insensitive.
  - Verifies `exact` requires full-string match.
  - Verifies `regex` supports pattern matching and safely ignores invalid patterns.

- **Path:** `cli/fast2flow-cli/src/main.rs` (tests module)
- **Role:** CLI policy command coverage.
- **Key functionality:**
  - Verifies policy validate accepts a valid policy file.
  - Verifies policy validate fails for missing files.
  - Verifies policy validate rejects invalid regex policies.

- **Path:** `tests/fixtures/policy.sample.json`
- **Role:** Example operator policy configuration.
- **Key functionality:**
  - Demonstrates `stage_order`, override `id`/`priority`, and respond-rule `mode`.

- **Path:** `.github/workflows/publish.yml`
- **Role:** Release automation for commercial/private distribution.
- **Key functionality:**
  - Triggers on `master` pushes (and optional manual dispatch).
  - Resolves release version from workspace metadata and enforces tag/version match.
  - Auto-creates `v<version>` tag on `master` push if missing.
  - Builds six release artifacts in parallel:
    - Linux `x86_64` + `aarch64`
    - macOS 15 `x86_64` + `aarch64`
    - Windows `x86_64` + `aarch64`
  - Packages Unix artifacts as `.tar.gz`, Windows artifacts as `.zip`, emits SHA-256 files, and attaches all artifacts to GitHub Release.

- **Path:** `.codex/*/PR.md`
- **Role:** Updated PR scope notes.
- **Key functionality:**
  - Replaced placeholders with implementation requirements per module.

## 3. Work In Progress, TODOs, and Stubs
- **Location:** `crates/fast2flow-routing-gtpack/src/lib.rs` and `crates/fast2flow-routing-gtpack/wit/fast2flow.wit`
- **Status:** partial runtime integration
- **Short description:** Mount-based runtime, bootstrap hooks, and host binary entrypoint are implemented; deployment-specific host process integration remains environment-dependent.

- **Location:** `crates/fast2flow-llm-openai/src/lib.rs`, `crates/fast2flow-llm-ollama/src/lib.rs`
- **Status:** partial production hardening
- **Short description:** Adapters now use secrets manager constructors, but still lack retry/backoff and telemetry instrumentation.

- **Location:** routing policy authoring workflow
- **Status:** partial feature depth
- **Short description:** Runtime validation and CLI `policy validate` are implemented, but there is not yet schema-first operator tooling (e.g., JSON Schema + editor integration).

- **Location:** repository-wide marker scan (`rg TODO|FIXME|XXX|HACK|BROKEN|unimplemented!|todo!`)
- **Status:** no inline TODO markers found
- **Short description:** No explicit TODO/FIXME markers currently present.

## 4. Broken, Failing, or Conflicting Areas
No functional build/test failures were observed in the current snapshot.

- **Location:** workspace checks (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo build`, `cargo doc`)
- **Evidence:** `bash ci/local_check.sh` completes successfully.
- **Likely cause / nature of issue:** N/A; checks pass.

## 5. Notes for Future Work
- Add richer strategy tuning and configurable thresholds per scope/channel.
- Add schema-first policy authoring support (JSON Schema + validation integration in editor/CI).
- Add CI smoke tests that validate cross-target release artifact naming/layout conventions.
