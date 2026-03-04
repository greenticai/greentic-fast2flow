# fast2flow-llm-interface

## Scope
Define provider-agnostic LLM contract used as optional routing fallback.

## Required behavior
- Expose trait:
  - `async fn complete(prompt, timeout) -> Result<LlmResponse>`
- Require strict JSON output shape only.
- No tool-calling.
- No streaming.
- Support hard timeout semantics.

## Integration points
- Implemented by provider adapters (`openai`, `ollama`).
- Invoked by `fast2flow-core` only after deterministic strategy confidence miss.
