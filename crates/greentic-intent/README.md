# greentic-intent

Deterministic multilingual entity detection + marker rendering. The first
stage of Greentic's natural-language routing pipeline:

```
text → greentic-intent (markers + entities) → greentic-fast2flow (routing + binding)
```

## Design rules

- No LLM on the default path.
- No network calls.
- No embeddings.
- No heavy model loading at request time.
- Target: < 50ms for entity marking on prompts ≤ 1 KB in release mode with
  preloaded resources.
- Preserve byte offsets through the whole pipeline.
- Entity marker names are language-neutral
  (`{{location}}`, `{{date}}` — never `{{lieu}}` or `{{fecha}}`).

## Status

Phase A scaffold: public API types are defined, `IntentEngine::mark()`
returns the original text unchanged with no entities until extractors are
filled in. Locale resources, gazetteer, and the per-kind extractors are
the next milestones.

See the workspace root design doc for the full Phase A → E plan.
