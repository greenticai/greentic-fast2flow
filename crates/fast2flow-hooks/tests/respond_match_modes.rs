use fast2flow_contracts::{Fast2FlowHookInV1, MessageEnvelope, TextMatchModeV1};
use fast2flow_hooks::{DefaultHookFilter, FilterDecision, HookFilter, RespondRule};

fn request(text: &str) -> Fast2FlowHookInV1 {
    Fast2FlowHookInV1 {
        scope: "tenant-a".to_string(),
        envelope: MessageEnvelope {
            text: text.to_string(),
            channel: Some("chat".to_string()),
            provider: Some("tests".to_string()),
        },
        session_active: false,
        input_locale: "en-US".to_string(),
        time_budget_ms: 250,
        registry_path: "/mnt/registry/latest.json".to_string(),
        indexes_path: "/mnt/indexes".to_string(),
        now_unix_ms: 0,
    }
}

#[test]
fn contains_mode_is_case_insensitive() {
    let filter = DefaultHookFilter {
        respond_rules: vec![RespondRule {
            needle: "refund".to_string(),
            message: "respond".to_string(),
            mode: TextMatchModeV1::Contains,
        }],
        ..Default::default()
    };

    let decision = filter.evaluate(&request("Need REFUND now"));
    assert_eq!(decision, FilterDecision::Respond("respond".to_string()));
}

#[test]
fn exact_mode_requires_full_match() {
    let filter = DefaultHookFilter {
        respond_rules: vec![RespondRule {
            needle: "refund please".to_string(),
            message: "respond".to_string(),
            mode: TextMatchModeV1::Exact,
        }],
        ..Default::default()
    };

    assert_eq!(
        filter.evaluate(&request("refund please")),
        FilterDecision::Respond("respond".to_string())
    );
    assert_eq!(
        filter.evaluate(&request("refund please now")),
        FilterDecision::Proceed
    );
}

#[test]
fn regex_mode_supports_patterns() {
    let filter = DefaultHookFilter {
        respond_rules: vec![RespondRule {
            needle: "(?i)refund\\s+please".to_string(),
            message: "respond".to_string(),
            mode: TextMatchModeV1::Regex,
        }],
        ..Default::default()
    };

    assert_eq!(
        filter.evaluate(&request("REFUND please")),
        FilterDecision::Respond("respond".to_string())
    );
}

#[test]
fn invalid_regex_is_ignored() {
    let filter = DefaultHookFilter {
        respond_rules: vec![RespondRule {
            needle: "(unclosed".to_string(),
            message: "respond".to_string(),
            mode: TextMatchModeV1::Regex,
        }],
        ..Default::default()
    };

    assert_eq!(
        filter.evaluate(&request("refund please")),
        FilterDecision::Proceed
    );
}
