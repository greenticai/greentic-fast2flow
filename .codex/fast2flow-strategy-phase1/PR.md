# fast2flow-strategy-phase1

## Scope
Implement phase-1 deterministic routing strategy.

## Required behavior
- Token similarity based ranking.
- Deterministic ordering and tie-break rules.
- Confidence scoring returned with route target.
- No external model dependency.

## Output
- `Decision { target, confidence, reason }`

## Integration points
- Implements trait from `fast2flow-strategy`.
- Invoked by `fast2flow-core` after index candidate retrieval.
