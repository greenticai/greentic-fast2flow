use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use fast2flow_contracts::{
    Candidate, Fast2FlowHookInV1, FlowDoc, MessageEnvelope, RoutingDirective, TextMatchModeV1,
};
use fast2flow_core::{CandidateIndex, CoreRouter, RouterConfig};
use fast2flow_hooks::{DefaultHookFilter, RespondRule};
use fast2flow_indexer::{build_index, load_latest, IndexStore};
use fast2flow_llm::{LlmError, LlmProvider, LlmResponse};
use fast2flow_strategy_phase1::Phase1DeterministicStrategy;

#[tokio::test]
async fn deterministic_routing_dispatches_refund_flow() {
    let store = build_test_index("tenant-a:default", fixture_flows());
    let lookup = TestLookup { store };
    let router = default_router(None, RouterConfig::default());

    let output = router
        .route(
            request("tenant-a:default", "refund please", false, 200),
            &lookup,
        )
        .await;

    match output.directive {
        RoutingDirective::Dispatch {
            target, utterance, ..
        } => {
            assert_eq!(target, "support/refund_flow");
            assert_eq!(utterance.as_deref(), Some("refund please"));
        }
        other => panic!("expected dispatch, got {other:?}"),
    }
}

#[tokio::test]
async fn llm_fallback_dispatches_when_deterministic_misses() {
    let store = build_test_index("tenant-a:default", fixture_flows());
    let lookup = TestLookup { store };
    let llm = Arc::new(MockLlmProvider::dispatch(
        "assistant/general_help",
        0.82,
        "llm_fallback",
    ));
    let config = RouterConfig {
        min_confidence: 0.95,
        llm_min_confidence: 0.5,
        candidate_limit: 10,
    };
    let router = default_router(Some(llm), config);

    let output = router
        .route(
            request("tenant-a:default", "something unrelated", false, 200),
            &lookup,
        )
        .await;

    match output.directive {
        RoutingDirective::Dispatch {
            target,
            reason,
            utterance,
            ..
        } => {
            assert_eq!(target, "assistant/general_help");
            assert_eq!(reason, "llm_fallback");
            assert_eq!(utterance.as_deref(), Some("something unrelated"));
        }
        other => panic!("expected llm dispatch, got {other:?}"),
    }
}

#[tokio::test]
async fn llm_timeout_fails_open_to_continue() {
    let store = build_test_index("tenant-a:default", fixture_flows());
    let lookup = TestLookup { store };
    let llm = Arc::new(MockLlmProvider::timeout());
    let config = RouterConfig {
        min_confidence: 0.99,
        llm_min_confidence: 0.5,
        candidate_limit: 10,
    };
    let router = default_router(Some(llm), config);

    let output = router
        .route(
            request("tenant-a:default", "need help now", false, 30),
            &lookup,
        )
        .await;

    assert!(matches!(output.directive, RoutingDirective::Continue));
}

#[tokio::test]
async fn filter_deny_short_circuits_with_deny_directive() {
    let store = build_test_index("tenant-a:default", fixture_flows());
    let lookup = TestLookup { store };

    let strategy = Arc::new(Phase1DeterministicStrategy);
    let filter = Arc::new(DefaultHookFilter {
        deny_scopes: vec!["tenant-a:default".to_string()],
        ..DefaultHookFilter::default()
    });
    let router = CoreRouter::new(strategy, vec![filter], None, RouterConfig::default());

    let output = router
        .route(
            request("tenant-a:default", "refund please", false, 200),
            &lookup,
        )
        .await;

    match output.directive {
        RoutingDirective::Deny { reason } => {
            assert_eq!(reason, "scope denied by policy");
        }
        other => panic!("expected deny, got {other:?}"),
    }
}

#[tokio::test]
async fn filter_respond_short_circuits_with_respond_directive() {
    let store = build_test_index("tenant-a:default", fixture_flows());
    let lookup = TestLookup { store };

    let strategy = Arc::new(Phase1DeterministicStrategy);
    let filter = Arc::new(DefaultHookFilter {
        respond_rules: vec![RespondRule {
            needle: "business hours".to_string(),
            message: "Support hours are 9-5 UTC".to_string(),
            mode: TextMatchModeV1::Contains,
        }],
        ..DefaultHookFilter::default()
    });
    let router = CoreRouter::new(strategy, vec![filter], None, RouterConfig::default());

    let output = router
        .route(
            request(
                "tenant-a:default",
                "what are your business hours?",
                false,
                200,
            ),
            &lookup,
        )
        .await;

    match output.directive {
        RoutingDirective::Respond { message } => {
            assert_eq!(message, "Support hours are 9-5 UTC");
        }
        other => panic!("expected respond, got {other:?}"),
    }
}

#[tokio::test]
async fn session_active_filter_returns_continue() {
    let store = build_test_index("tenant-a:default", fixture_flows());
    let lookup = TestLookup { store };
    let router = default_router(None, RouterConfig::default());

    let output = router
        .route(
            request("tenant-a:default", "refund please", true, 200),
            &lookup,
        )
        .await;

    assert!(matches!(output.directive, RoutingDirective::Continue));
}

#[tokio::test]
async fn zero_time_budget_returns_continue() {
    let store = build_test_index("tenant-a:default", fixture_flows());
    let lookup = TestLookup { store };
    let router = default_router(None, RouterConfig::default());

    let output = router
        .route(
            request("tenant-a:default", "refund please", false, 0),
            &lookup,
        )
        .await;

    assert!(matches!(output.directive, RoutingDirective::Continue));
}

fn default_router(llm: Option<Arc<dyn LlmProvider>>, config: RouterConfig) -> CoreRouter {
    let strategy = Arc::new(Phase1DeterministicStrategy);
    let filter = Arc::new(DefaultHookFilter::default());
    CoreRouter::new(strategy, vec![filter], llm, config)
}

fn request(scope: &str, text: &str, session_active: bool, budget_ms: u64) -> Fast2FlowHookInV1 {
    Fast2FlowHookInV1 {
        scope: scope.to_string(),
        envelope: MessageEnvelope {
            text: text.to_string(),
            channel: Some("chat".to_string()),
            provider: Some("tests".to_string()),
        },
        session_active,
        input_locale: "en-US".to_string(),
        time_budget_ms: budget_ms,
        registry_path: "/mnt/registry/latest.json".to_string(),
        indexes_path: "/mnt/indexes".to_string(),
        now_unix_ms: 0,
        messaging_endpoint_id: None,
    }
}

fn build_test_index(scope: &str, docs: Vec<FlowDoc>) -> IndexStore {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_nanos();
    let root = std::env::temp_dir().join(format!("fast2flow-index-{suffix}"));
    build_index(scope, &docs, &root, 0).expect("index build should succeed");
    load_latest(&root, scope).expect("latest index should load")
}

fn fixture_flows() -> Vec<FlowDoc> {
    serde_json::from_str(include_str!("../../../tests/fixtures/flows.json"))
        .expect("fixture must be valid flow docs")
}

struct TestLookup {
    store: IndexStore,
}

impl CandidateIndex for TestLookup {
    fn search(&self, _scope: &str, text: &str, limit: usize) -> Vec<Candidate> {
        self.store.search(text, limit)
    }
}

#[derive(Debug, Clone)]
struct MockLlmProvider {
    outcome: MockOutcome,
}

#[derive(Debug, Clone)]
enum MockOutcome {
    Dispatch {
        target: String,
        confidence: f32,
        reason: String,
    },
    Timeout,
}

impl MockLlmProvider {
    fn dispatch(target: &str, confidence: f32, reason: &str) -> Self {
        Self {
            outcome: MockOutcome::Dispatch {
                target: target.to_string(),
                confidence,
                reason: reason.to_string(),
            },
        }
    }

    fn timeout() -> Self {
        Self {
            outcome: MockOutcome::Timeout,
        }
    }
}

#[async_trait]
impl LlmProvider for MockLlmProvider {
    async fn complete(&self, _prompt: &str, _timeout: Duration) -> Result<LlmResponse, LlmError> {
        match &self.outcome {
            MockOutcome::Dispatch {
                target,
                confidence,
                reason,
            } => Ok(LlmResponse {
                target: target.clone(),
                confidence: *confidence,
                reason: reason.clone(),
            }),
            MockOutcome::Timeout => Err(LlmError::Timeout),
        }
    }
}
