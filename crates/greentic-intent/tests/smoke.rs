//! End-to-end smoke tests: assemble the engine the way real callers do.

use greentic_intent::extractors::date::DateExtractor;
use greentic_intent::extractors::location::LocationExtractor;
use greentic_intent::{EntityKind, IntentContext, IntentEngine};

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

#[cfg(all(feature = "builtin-locales", feature = "builtin-gazetteer"))]
#[test]
fn engine_with_builtin_resources_extracts_location_and_date_together() {
    use chrono::{TimeZone, Utc};

    let engine = IntentEngine::builder()
        .with_builtin_locales()
        .with_builtin_gazetteer()
        .with_default_extractors()
        .build();
    let ctx = IntentContext {
        reference_time: Utc.with_ymd_and_hms(2026, 5, 27, 12, 0, 0).unwrap(),
        timezone: "Europe/London".into(),
        preferred_locale: Some("en-GB".into()),
        tenant_locale: None,
        user_locale: None,
        allowed_languages: vec!["en".into()],
    };

    let result = engine.mark("what is the weather in London tomorrow?", &ctx);

    assert_eq!(result.entities.len(), 2);

    let london = result
        .entities
        .iter()
        .find(|e| e.kind == EntityKind::Location)
        .expect("london entity");
    assert_eq!(london.normalized, "London");
    assert_eq!(london.role.as_deref(), Some("in"));

    let tomorrow = result
        .entities
        .iter()
        .find(|e| e.kind == EntityKind::Date)
        .expect("date entity");
    assert_eq!(tomorrow.normalized, "20260528");

    // Type-only markers replace both spans, role-aware adds the preposition.
    assert_eq!(
        result.marked_text,
        "what is the weather in {{location}} {{date}}?"
    );
    assert!(result.marked_text_roles.contains("{{location:in}}"));
    assert!(result.marked_text_debug.contains("{{location: London}}"));
    assert!(result.marked_text_debug.contains("{{date: 20260528}}"));
}

#[cfg(all(feature = "builtin-locales", feature = "builtin-gazetteer"))]
#[test]
fn engine_extracts_date_time_and_location_together() {
    use chrono::{TimeZone, Utc};

    let engine = IntentEngine::builder()
        .with_builtin_locales()
        .with_builtin_gazetteer()
        .with_default_extractors()
        .build();
    let ctx = IntentContext {
        reference_time: Utc.with_ymd_and_hms(2026, 5, 27, 12, 0, 0).unwrap(),
        timezone: "Europe/London".into(),
        preferred_locale: Some("en-GB".into()),
        tenant_locale: None,
        user_locale: None,
        allowed_languages: vec!["en".into()],
    };

    let result = engine.mark("ship it tomorrow at 3pm in London", &ctx);

    assert_eq!(result.entities.len(), 3);

    let date = result
        .entities
        .iter()
        .find(|e| e.kind == EntityKind::Date)
        .expect("date entity");
    assert_eq!(date.normalized, "20260528");

    let time = result
        .entities
        .iter()
        .find(|e| e.kind == EntityKind::Time)
        .expect("time entity");
    assert_eq!(time.normalized, "15:00");

    let london = result
        .entities
        .iter()
        .find(|e| e.kind == EntityKind::Location)
        .expect("location entity");
    assert_eq!(london.normalized, "London");
    assert_eq!(london.role.as_deref(), Some("in"));

    assert_eq!(
        result.marked_text,
        "ship it {{date}} at {{time}} in {{location}}"
    );
    assert!(result.marked_text_debug.contains("{{time: 15:00}}"));
}

#[cfg(all(feature = "builtin-locales", feature = "builtin-gazetteer"))]
#[test]
fn engine_resolves_from_and_to_roles_for_multi_word_cities() {
    let engine = IntentEngine::builder()
        .with_builtin_locales()
        .with_builtin_gazetteer()
        .with_default_extractors()
        .build();
    let ctx = IntentContext::now_utc("Europe/London");

    let result = engine.mark("flight from London to New York", &ctx);

    assert_eq!(result.entities.len(), 2);
    assert_eq!(
        result.marked_text_roles,
        "flight from {{location:from}} to {{location:to}}"
    );
}

#[cfg(all(feature = "builtin-locales", feature = "builtin-gazetteer"))]
mod multilingual {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn ctx_for(locale: &str, tz: &str) -> IntentContext {
        IntentContext {
            // 2026-05-27 is a Wednesday → tomorrow == 2026-05-28 == "20260528".
            reference_time: Utc.with_ymd_and_hms(2026, 5, 27, 12, 0, 0).unwrap(),
            timezone: tz.into(),
            preferred_locale: Some(locale.into()),
            tenant_locale: None,
            user_locale: None,
            allowed_languages: vec![locale.split('-').next().unwrap_or("en").into()],
        }
    }

    fn engine() -> IntentEngine {
        IntentEngine::builder()
            .with_builtin_locales()
            .with_builtin_gazetteer()
            .with_default_extractors()
            .build()
    }

    #[test]
    fn french_weather_query_marks_location_and_date() {
        let result = engine().mark(
            "quel temps fera-t-il à Paris demain ?",
            &ctx_for("fr-FR", "Europe/Paris"),
        );
        assert_eq!(result.entities.len(), 2);
        let date = result
            .entities
            .iter()
            .find(|e| e.kind == EntityKind::Date)
            .expect("date");
        assert_eq!(date.normalized, "20260528");
        // `à` maps to `In` per the locale bundle.
        let paris = result
            .entities
            .iter()
            .find(|e| e.kind == EntityKind::Location)
            .expect("location");
        assert_eq!(paris.normalized, "Paris");
        assert_eq!(paris.role.as_deref(), Some("in"));
    }

    #[test]
    fn spanish_flight_query_resolves_from_and_to() {
        let result = engine().mark(
            "resérvame un vuelo de Londres a París mañana",
            &ctx_for("es-ES", "Europe/Madrid"),
        );
        assert_eq!(result.entities.len(), 3);
        let by_role: std::collections::HashMap<Option<&str>, &str> = result
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Location)
            .map(|e| (e.role.as_deref(), e.normalized.as_str()))
            .collect();
        assert_eq!(by_role.get(&Some("from")).copied(), Some("London"));
        assert_eq!(by_role.get(&Some("to")).copied(), Some("Paris"));
        let date = result
            .entities
            .iter()
            .find(|e| e.kind == EntityKind::Date)
            .expect("date");
        assert_eq!(date.normalized, "20260528");
    }

    #[test]
    fn dutch_flight_query_resolves_from_and_to() {
        let result = engine().mark(
            "boek een vlucht van Londen naar Parijs morgen",
            &ctx_for("nl-NL", "Europe/Amsterdam"),
        );
        assert_eq!(result.entities.len(), 3);
        let by_role: std::collections::HashMap<Option<&str>, &str> = result
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Location)
            .map(|e| (e.role.as_deref(), e.normalized.as_str()))
            .collect();
        assert_eq!(by_role.get(&Some("from")).copied(), Some("London"));
        assert_eq!(by_role.get(&Some("to")).copied(), Some("Paris"));
        let date = result
            .entities
            .iter()
            .find(|e| e.kind == EntityKind::Date)
            .expect("date");
        assert_eq!(date.normalized, "20260528");
    }

    #[test]
    fn german_weather_query_resolves_location_and_date() {
        let result = engine().mark(
            "wie wird das Wetter morgen in Berlin?",
            &ctx_for("de-DE", "Europe/Berlin"),
        );
        assert_eq!(result.entities.len(), 2);
        let berlin = result
            .entities
            .iter()
            .find(|e| e.kind == EntityKind::Location)
            .expect("location");
        assert_eq!(berlin.normalized, "Berlin");
        assert_eq!(berlin.role.as_deref(), Some("in"));
        let date = result
            .entities
            .iter()
            .find(|e| e.kind == EntityKind::Date)
            .expect("date");
        assert_eq!(date.normalized, "20260528");
    }
}
