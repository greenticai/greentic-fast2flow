//! End-to-end test of the intent prefill stage.
//!
//! Demonstrates the contract greentic-intent has with greentic-fast2flow:
//!
//!   raw query → intent.mark(query, ctx) → marked_text + entities
//!   marked_text → IndexStore.search() → ranked candidates
//!
//! The index entries here carry `utterances` with marker templates
//! (`{{location}}`, `{{date}}`, `{{time}}`) — the same stable tokens
//! greentic-intent emits in `marked_text`. Token-overlap between query
//! markers and corpus markers is what makes the routing language-neutral.

use chrono::{TimeZone, Utc};
use fast2flow_contracts::{FlowExecutionType, IndexEntryV2, IndexManifestV2};
use fast2flow_indexer::IndexStore;
use greentic_intent::{IntentContext, IntentEngine};

fn build_intent_engine() -> IntentEngine {
    IntentEngine::builder()
        .with_builtin_locales()
        .with_builtin_gazetteer()
        .with_default_extractors()
        .build()
}

fn build_test_index() -> IndexStore {
    let entries = vec![
        IndexEntryV2 {
            flow_id: "weather_lookup".into(),
            pack_id: "demo".into(),
            target: "demo/weather".into(),
            title: "Look up weather forecast".into(),
            // Multilingual intent vocabulary: a real pack would ship
            // one tag list per supported language. Mixing them in one
            // entry is fine for the demo since BM25 is token-based.
            tags: vec![
                "weather".into(),
                "forecast".into(),
                "rain".into(),
                "temps".into(),
                "météo".into(),
                "meteo".into(),
                "tiempo".into(),
                "weer".into(),
            ],
            utterances: vec![
                "weather in {{location}} {{date}}".into(),
                "what is the weather in {{location}}".into(),
                "is it raining in {{location}}".into(),
                "quel temps fait il à {{location}} {{date}}".into(),
                "qué tiempo hará en {{location}} {{date}}".into(),
                "wat is het weer in {{location}} {{date}}".into(),
            ],
            node_ids: vec!["forecast_card".into()],
            flow_type: FlowExecutionType::Deterministic,
        },
        IndexEntryV2 {
            flow_id: "book_flight".into(),
            pack_id: "demo".into(),
            target: "demo/flight".into(),
            title: "Book a flight".into(),
            tags: vec![
                "flight".into(),
                "booking".into(),
                "travel".into(),
                "vol".into(),
                "vuelo".into(),
                "vlucht".into(),
            ],
            utterances: vec![
                "book flight from {{location}} to {{location}}".into(),
                "flight from {{location}} to {{location}} on {{date}}".into(),
                "fly to {{location}}".into(),
                "réserve moi un vol de {{location}} à {{location}}".into(),
                "vuelo de {{location}} a {{location}}".into(),
                "boek een vlucht van {{location}} naar {{location}}".into(),
            ],
            node_ids: vec!["booking_card".into()],
            flow_type: FlowExecutionType::Deterministic,
        },
        IndexEntryV2 {
            flow_id: "schedule_meeting".into(),
            pack_id: "demo".into(),
            target: "demo/meeting".into(),
            title: "Schedule a meeting".into(),
            tags: vec!["meeting".into(), "calendar".into(), "schedule".into()],
            utterances: vec![
                "schedule meeting on {{date}} at {{time}}".into(),
                "book meeting at {{time}}".into(),
                "set up call for {{date}}".into(),
            ],
            node_ids: vec!["calendar_card".into()],
            flow_type: FlowExecutionType::Deterministic,
        },
    ];

    IndexStore::from_manifest(IndexManifestV2 {
        version: "v2".into(),
        scope: "test:default".into(),
        generated_at_ms: 0,
        entries,
    })
}

fn ctx_en_gb() -> IntentContext {
    IntentContext {
        // 2026-05-27 (Wed) so `tomorrow` resolves to 20260528.
        reference_time: Utc.with_ymd_and_hms(2026, 5, 27, 12, 0, 0).unwrap(),
        timezone: "Europe/London".into(),
        preferred_locale: Some("en-GB".into()),
        tenant_locale: None,
        user_locale: None,
        allowed_languages: vec!["en".into()],
    }
}

fn run_prefill(engine: &IntentEngine, index: &IndexStore, query: &str) -> Vec<(String, f32)> {
    let result = engine.mark(query, &ctx_en_gb());
    let candidates = index.search(&result.marked_text, 5);
    candidates
        .into_iter()
        .map(|c| (c.flow_id, c.score_hint))
        .collect()
}

#[test]
fn prefill_routes_weather_query_to_weather_flow() {
    let engine = build_intent_engine();
    let index = build_test_index();
    let candidates = run_prefill(&engine, &index, "what is the weather in London tomorrow?");
    assert!(
        !candidates.is_empty(),
        "expected at least one candidate, got {candidates:?}"
    );
    assert_eq!(
        candidates[0].0, "weather_lookup",
        "expected weather_lookup to win; got {candidates:?}"
    );
}

#[test]
fn prefill_routes_flight_query_to_flight_flow() {
    let engine = build_intent_engine();
    let index = build_test_index();
    let candidates = run_prefill(
        &engine,
        &index,
        "book me a flight from London to Paris for Saturday",
    );
    assert!(!candidates.is_empty());
    assert_eq!(
        candidates[0].0, "book_flight",
        "expected book_flight to win; got {candidates:?}"
    );
}

#[test]
fn prefill_routes_meeting_query_to_meeting_flow() {
    let engine = build_intent_engine();
    let index = build_test_index();
    let candidates = run_prefill(&engine, &index, "schedule a meeting tomorrow at 3pm");
    assert!(!candidates.is_empty());
    assert_eq!(
        candidates[0].0, "schedule_meeting",
        "expected schedule_meeting to win; got {candidates:?}"
    );
}

#[test]
fn prefill_improves_score_over_raw_text() {
    // Compare: raw query vs marked query. Both go through the same
    // overlap scorer; the marked version should win bigger because
    // `{{location}}` / `{{date}}` tokens line up with the index's
    // utterance markers.
    let engine = build_intent_engine();
    let index = build_test_index();
    let ctx = ctx_en_gb();
    let query = "what is the weather in London tomorrow?";

    let result = engine.mark(query, &ctx);
    let prefill_cands = index.search(&result.marked_text, 5);
    let raw_cands = index.search(query, 5);

    let prefill_top = prefill_cands
        .iter()
        .find(|c| c.flow_id == "weather_lookup")
        .expect("weather_lookup in prefill results");
    let raw_top = raw_cands.iter().find(|c| c.flow_id == "weather_lookup");

    if let Some(raw_top) = raw_top {
        assert!(
            prefill_top.score_hint > raw_top.score_hint,
            "prefill score {} should beat raw score {} (prefill: {:?}, raw: {:?})",
            prefill_top.score_hint,
            raw_top.score_hint,
            prefill_cands,
            raw_cands
        );
    } else {
        // Raw query scored zero against the index — the strongest possible
        // signal that prefill is doing real work.
    }
}

#[test]
fn prefill_emits_expected_marked_text_and_entities() {
    let engine = build_intent_engine();
    let result = engine.mark("what is the weather in London tomorrow?", &ctx_en_gb());

    assert_eq!(
        result.marked_text,
        "what is the weather in {{location}} {{date}}?"
    );
    // Two entities: London (location, role=in) and tomorrow (date).
    assert_eq!(result.entities.len(), 2);
    let london = result
        .entities
        .iter()
        .find(|e| e.kind == greentic_intent::EntityKind::Location)
        .expect("location entity");
    assert_eq!(london.normalized, "London");
    assert_eq!(london.role.as_deref(), Some("in"));
    let tomorrow = result
        .entities
        .iter()
        .find(|e| e.kind == greentic_intent::EntityKind::Date)
        .expect("date entity");
    assert_eq!(tomorrow.normalized, "20260528");
}

/// Visible end-to-end demo. Run with:
///   cargo test -p fast2flow-indexer --test intent_prefill -- --nocapture print_prefill_pipeline_demo
#[test]
fn print_prefill_pipeline_demo() {
    let engine = build_intent_engine();
    let index = build_test_index();

    let queries: &[(&str, &str)] = &[
        ("en-GB", "what is the weather in London tomorrow?"),
        (
            "en-GB",
            "book me a flight from London to Paris for Saturday",
        ),
        ("en-GB", "schedule a meeting tomorrow at 3pm"),
        ("fr-FR", "quel temps fera-t-il à Paris demain ?"),
        ("es-ES", "qué tiempo hará en Madrid mañana"),
        ("nl-NL", "boek een vlucht van Londen naar Parijs morgen"),
    ];

    println!();
    println!("=== intent prefill → fast2flow routing ===");
    for (tag, query) in queries {
        let ctx = IntentContext {
            reference_time: Utc.with_ymd_and_hms(2026, 5, 27, 12, 0, 0).unwrap(),
            timezone: "Europe/London".into(),
            preferred_locale: Some((*tag).into()),
            tenant_locale: None,
            user_locale: None,
            allowed_languages: vec![tag.split('-').next().unwrap_or("en").into()],
        };
        let result = engine.mark(query, &ctx);
        let candidates = index.search(&result.marked_text, 3);

        println!();
        println!("[{tag}] query   : {query}");
        println!("        marked  : {}", result.marked_text);
        if !result.entities.is_empty() {
            let entities = result
                .entities
                .iter()
                .map(|e| {
                    let role = e
                        .role
                        .as_deref()
                        .map(|r| format!(":{r}"))
                        .unwrap_or_default();
                    format!(
                        "{}{} = {} ({})",
                        e.kind.marker_name(),
                        role,
                        e.normalized,
                        e.raw
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            println!("        entities: {entities}");
        }
        if candidates.is_empty() {
            println!("        ROUTING : (no candidates)");
        } else {
            for (i, cand) in candidates.iter().enumerate() {
                println!(
                    "        rank #{i:>1}: {:<18} score={:.3}  target={}",
                    cand.flow_id, cand.score_hint, cand.target
                );
            }
        }
    }
    println!();
}

#[test]
fn multilingual_queries_route_to_same_flow_via_markers() {
    // Different languages, identical marker templates → same routing.
    let engine = build_intent_engine();
    let index = build_test_index();

    let queries = [
        ("en-GB", "what is the weather in London tomorrow?"),
        ("fr-FR", "quel temps fera-t-il à Paris demain ?"),
        ("es-ES", "qué tiempo hará en Madrid mañana"),
        ("nl-NL", "wat is het weer in Amsterdam morgen"),
    ];

    for (tag, query) in queries {
        let ctx = IntentContext {
            reference_time: Utc.with_ymd_and_hms(2026, 5, 27, 12, 0, 0).unwrap(),
            timezone: "Europe/London".into(),
            preferred_locale: Some(tag.into()),
            tenant_locale: None,
            user_locale: None,
            allowed_languages: vec![tag.split('-').next().unwrap_or("en").into()],
        };
        let result = engine.mark(query, &ctx);
        let candidates = index.search(&result.marked_text, 5);
        assert!(
            !candidates.is_empty(),
            "[{tag}] empty candidates for `{query}` → marked={:?}",
            result.marked_text
        );
        assert_eq!(
            candidates[0].flow_id, "weather_lookup",
            "[{tag}] expected weather_lookup to win for `{query}`; got {candidates:?}"
        );
    }
}
