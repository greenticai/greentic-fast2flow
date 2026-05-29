//! End-to-end smoke tests: assemble the engine the way real callers do.

use greentic_intent::extractors::date::DateExtractor;
use greentic_intent::extractors::location::LocationExtractor;
use greentic_intent::{IntentContext, IntentEngine};

#[test]
fn engine_with_extractors_but_no_locale_resources_returns_text_unchanged() {
    let engine = IntentEngine::builder()
        .with_extractor(DateExtractor)
        .with_extractor(LocationExtractor)
        .build();
    let ctx = IntentContext::now_utc("Europe/London");
    let result = engine.mark("what is the weather in London tomorrow?", &ctx);

    // No locale bundle loaded → date extractor produces no candidates.
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

#[cfg(feature = "builtin-locales")]
#[test]
fn engine_with_builtin_en_gb_extracts_relative_dates_and_marks_them() {
    use chrono::{TimeZone, Utc};

    let engine = IntentEngine::builder()
        .with_builtin_locales()
        .with_extractor(DateExtractor)
        .build();
    let ctx = IntentContext {
        // 2026-05-27 is a Wednesday.
        reference_time: Utc.with_ymd_and_hms(2026, 5, 27, 12, 0, 0).unwrap(),
        timezone: "Europe/London".into(),
        preferred_locale: Some("en-GB".into()),
        tenant_locale: None,
        user_locale: None,
        allowed_languages: vec!["en".into()],
    };

    let result = engine.mark("ship it tomorrow and review next Monday", &ctx);

    // Two dates: `tomorrow` → 20260528, `next Monday` → 20260608.
    assert_eq!(result.entities.len(), 2);
    let dates: Vec<&str> = result
        .entities
        .iter()
        .map(|e| e.normalized.as_str())
        .collect();
    assert_eq!(dates, vec!["20260528", "20260608"]);

    // Type-only marker rendering replaces both spans.
    assert_eq!(result.marked_text, "ship it {{date}} and review {{date}}");
    // Debug rendering carries the normalized values.
    assert!(result.marked_text_debug.contains("{{date: 20260528}}"));
    assert!(result.marked_text_debug.contains("{{date: 20260608}}"));
}
