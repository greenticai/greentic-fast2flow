# fast2flow-hookfilter-default

## Scope
Implement default hook filtering policy used by Fast2Flow before routing execution.

## Required behavior
- Default rule: do not run routing when `session_active = true`.
- Support allow/deny policy checks for:
  - channel/provider
  - tenant/team scope
- Enforce deterministic filter evaluation order.
- Return explicit filter outcome:
  - proceed
  - continue/fallback
  - deny with reason

## Integration points
- Request contract from `fast2flow-contracts`.
- Executed first stage by `fast2flow-core`.
