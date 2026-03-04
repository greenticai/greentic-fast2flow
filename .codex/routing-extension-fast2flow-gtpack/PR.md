# routing-extension-fast2flow-gtpack

## Scope
Implement Greentic routing extension boundary for Fast2Flow.

## Required behavior
- Provide hook entrypoint mapping Greentic hook request to Fast2Flow contracts.
- Load mounted resources from:
  - `/mnt/registry`
  - `/mnt/indexes`
- Invoke core router and return `RoutingDirective` output.
- Keep Greentic-specific details isolated to this crate.

## Non-goals
- No modifications to `greentic-pack`, `greentic-flow`, or `greentic-component`.
