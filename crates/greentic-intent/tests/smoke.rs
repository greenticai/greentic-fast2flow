//! End-to-end smoke test: Phase A engine with stub extractors registered.

use greentic_intent::extractors::date::DateExtractor;
use greentic_intent::extractors::location::LocationExtractor;
use greentic_intent::{IntentContext, IntentEngine};

#[test]
fn engine_pipes_through_extractors_with_no_panic() {
    let engine = IntentEngine::builder()
        .with_extractor(DateExtractor)
        .with_extractor(LocationExtractor)
        .build();
    let ctx = IntentContext::now_utc("Europe/London");
    let result = engine.mark("what is the weather in London tomorrow?", &ctx);

    // Phase A: stubs return no candidates → markers equal original text.
    assert_eq!(result.original_text, result.marked_text);
    assert!(result.entities.is_empty());
    assert!(result.latency.total_ms >= 0.0);
}

#[test]
fn mark_result_serializes_to_json() {
    let engine = IntentEngine::builder().build();
    let ctx = IntentContext::now_utc("Europe/London");
    let result = engine.mark("hello", &ctx);
    let json = serde_json::to_string(&result).expect("serialize");
    assert!(json.contains("\"marked_text\":\"hello\""));
    assert!(json.contains("\"language\""));
}
