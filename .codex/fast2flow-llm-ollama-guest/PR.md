# fast2flow-llm-ollama-guest

## Scope
Implement local Ollama-backed adapter for the Fast2Flow LLM interface.

## Required behavior
- Use Ollama HTTP API with configurable model.
- Enforce strict JSON response schema.
- Enforce timeout.
- No streaming and no tool-calling in phase 1.

## Integration points
- Implements `fast2flow-llm::LlmProvider`.
- Used optionally by `fast2flow-core`.
