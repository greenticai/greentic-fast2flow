# fast2flow-core

## Scope
Implement production-ready routing orchestration for Fast2Flow using workspace-shared contracts.

## Required behavior
- Expose `route(request) -> Fast2FlowHookOutV1` pipeline:
  1. Hook filter
  2. Index query
  3. Deterministic strategy
  4. Confidence threshold check
  5. Optional LLM fallback
  6. Directive output
- Enforce `time_budget_ms` for the full routing cycle.
- LLM must be optional and never required.
- If LLM is unavailable or times out: fail-open to `RoutingDirective::Continue`.
- Keep crate runtime-independent and free of Greentic runtime dependencies.

## Integration points
- Input/output types from `fast2flow-contracts`.
- Filtering via `fast2flow-hooks`.
- Strategy via `fast2flow-strategy`.
- Optional LLM via `fast2flow-llm` trait object.
- Candidate lookup via an index trait consumed from indexer-backed adapters.
