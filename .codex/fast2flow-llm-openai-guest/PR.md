# fast2flow-llm-openai-guest

## Scope
Implement OpenAI-backed adapter for the Fast2Flow LLM interface.

## Required behavior
- Use OpenAI HTTP API with model configuration.
- Read API key from Greentic secrets integration path (injected config).
- Enforce strict JSON responses for routing decision schema.
- Enforce timeout at adapter boundary.
- No streaming and no tool-calling in phase 1.

## Integration points
- Implements `fast2flow-llm::LlmProvider`.
- Used optionally by `fast2flow-core`.
