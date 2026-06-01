use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fast2flow_contracts::{
    Fast2FlowHookInV1, FlowDoc, MessageEnvelope, MessagingEndpointId, RoutingDirective,
};
use fast2flow_core::{CoreRouter, RouterConfig};
use fast2flow_hooks::DefaultHookFilter;
use fast2flow_indexer::build_index;
use fast2flow_routing_gtpack::handle_hook_from_mounts;
use fast2flow_strategy_phase1::Phase1DeterministicStrategy;

#[tokio::test]
async fn handle_hook_from_mounts_dispatches_for_refund() {
    let scope = "tenant-a:default";
    let indexes_root = temp_indexes_dir();
    let flows = vec![FlowDoc {
        id: "refund_flow".to_string(),
        pack_id: "support".to_string(),
        target: "support/refund_flow".to_string(),
        title: "Refund Request".to_string(),
        tags: vec!["refund".to_string(), "billing".to_string()],
        node_ids: vec!["start".to_string(), "issue_refund".to_string()],
    }];
    build_index(scope, &flows, &indexes_root, 0).expect("index build should succeed");

    let router = CoreRouter::new(
        Arc::new(Phase1DeterministicStrategy),
        vec![Arc::new(DefaultHookFilter::default())],
        None,
        RouterConfig::default(),
    );

    let output = handle_hook_from_mounts(
        &router,
        Fast2FlowHookInV1 {
            scope: scope.to_string(),
            envelope: MessageEnvelope {
                text: "refund please".to_string(),
                channel: Some("chat".to_string()),
                provider: Some("tests".to_string()),
            },
            session_active: false,
            input_locale: "en-US".to_string(),
            time_budget_ms: 200,
            registry_path: "/mnt/registry/latest.json".to_string(),
            indexes_path: indexes_root.display().to_string(),
            now_unix_ms: 0,
            messaging_endpoint_id: None,
        },
    )
    .await;

    match output.directive {
        RoutingDirective::Dispatch { target, .. } => {
            assert_eq!(target, "support/refund_flow");
        }
        other => panic!("expected dispatch, got {other:?}"),
    }
}

#[tokio::test]
async fn handle_hook_from_mounts_fails_open_when_index_missing() {
    let router = CoreRouter::new(
        Arc::new(Phase1DeterministicStrategy),
        vec![Arc::new(DefaultHookFilter::default())],
        None,
        RouterConfig::default(),
    );

    let output = handle_hook_from_mounts(
        &router,
        Fast2FlowHookInV1 {
            scope: "tenant-missing:default".to_string(),
            envelope: MessageEnvelope {
                text: "refund please".to_string(),
                channel: Some("chat".to_string()),
                provider: Some("tests".to_string()),
            },
            session_active: false,
            input_locale: "en-US".to_string(),
            time_budget_ms: 200,
            registry_path: "/mnt/registry/latest.json".to_string(),
            indexes_path: "/tmp/does-not-exist-fast2flow".to_string(),
            now_unix_ms: 0,
            messaging_endpoint_id: None,
        },
    )
    .await;

    assert!(matches!(output.directive, RoutingDirective::Continue));
}

fn temp_indexes_dir() -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_nanos();
    std::env::temp_dir().join(format!("fast2flow-routing-gtpack-{suffix}"))
}

#[tokio::test]
async fn handle_hook_from_mounts_resolves_endpoint_scope_when_set() {
    // Phase M1.3 — when `messaging_endpoint_id` is set, the routing layer
    // loads `endpoint:{id}` regardless of what `request.scope` carries.
    let indexes_root = temp_indexes_dir();
    let endpoint_id = MessagingEndpointId::new("teams-legal-bot").unwrap();

    let flows = vec![FlowDoc {
        id: "nda_flow".to_string(),
        pack_id: "legal".to_string(),
        target: "legal/nda_flow".to_string(),
        // Tokens match the request below so the phase1 strategy clears the
        // 0.5 default min_confidence (mirrors the refund test's shape).
        title: "NDA Form".to_string(),
        tags: vec!["nda".to_string(), "contract".to_string()],
        node_ids: vec!["start".to_string()],
    }];
    let endpoint_scope_key = fast2flow_contracts::endpoint_scope(&endpoint_id);
    build_index(&endpoint_scope_key, &flows, &indexes_root, 0).expect("endpoint index build");

    let router = CoreRouter::new(
        Arc::new(Phase1DeterministicStrategy),
        vec![Arc::new(DefaultHookFilter::default())],
        None,
        RouterConfig::default(),
    );

    let output = handle_hook_from_mounts(
        &router,
        Fast2FlowHookInV1 {
            // Deliberately stale scope — endpoint id must take precedence.
            scope: "stale:tenant".to_string(),
            envelope: MessageEnvelope {
                text: "nda please".to_string(),
                channel: Some("chat".to_string()),
                provider: Some("teams".to_string()),
            },
            session_active: false,
            input_locale: "en-US".to_string(),
            time_budget_ms: 200,
            registry_path: "/mnt/registry/latest.json".to_string(),
            indexes_path: indexes_root.display().to_string(),
            now_unix_ms: 0,
            messaging_endpoint_id: Some(endpoint_id),
        },
    )
    .await;

    match output.directive {
        RoutingDirective::Dispatch { target, .. } => {
            assert_eq!(target, "legal/nda_flow");
        }
        other => panic!(
            "expected dispatch via endpoint-scoped index, got {other:?}\n\
             (endpoint scope should override request.scope at the lookup step)"
        ),
    }
}

#[tokio::test]
async fn handle_hook_from_mounts_legacy_scope_path_unchanged() {
    // Phase M1.3 — when `messaging_endpoint_id` is None, the legacy
    // `tenant:team` scope continues to resolve the same way it did pre-M1.
    let indexes_root = temp_indexes_dir();
    let scope = "tenant-a:default";

    let flows = vec![FlowDoc {
        id: "refund_flow".to_string(),
        pack_id: "support".to_string(),
        target: "support/refund_flow".to_string(),
        title: "Refund Request".to_string(),
        tags: vec!["refund".to_string(), "billing".to_string()],
        node_ids: vec!["start".to_string()],
    }];
    build_index(scope, &flows, &indexes_root, 0).expect("legacy index build");

    let router = CoreRouter::new(
        Arc::new(Phase1DeterministicStrategy),
        vec![Arc::new(DefaultHookFilter::default())],
        None,
        RouterConfig::default(),
    );

    let output = handle_hook_from_mounts(
        &router,
        Fast2FlowHookInV1 {
            scope: scope.to_string(),
            envelope: MessageEnvelope {
                text: "refund please".to_string(),
                channel: Some("chat".to_string()),
                provider: Some("tests".to_string()),
            },
            session_active: false,
            input_locale: "en-US".to_string(),
            time_budget_ms: 200,
            registry_path: "/mnt/registry/latest.json".to_string(),
            indexes_path: indexes_root.display().to_string(),
            now_unix_ms: 0,
            messaging_endpoint_id: None,
        },
    )
    .await;

    match output.directive {
        RoutingDirective::Dispatch { target, .. } => {
            assert_eq!(target, "support/refund_flow");
        }
        other => panic!("legacy tenant:team path must still dispatch, got {other:?}"),
    }
}

#[tokio::test]
async fn handle_hook_from_mounts_endpoint_id_with_missing_index_continues() {
    // Phase M1.3 — endpoint-scoped lookup that misses the on-disk index
    // returns `Continue` (fail-open at the routing layer; the M1.4 admit
    // gate is responsible for hard rejection on unknown endpoints).
    let router = CoreRouter::new(
        Arc::new(Phase1DeterministicStrategy),
        vec![Arc::new(DefaultHookFilter::default())],
        None,
        RouterConfig::default(),
    );

    let output = handle_hook_from_mounts(
        &router,
        Fast2FlowHookInV1 {
            scope: "ignored:endpoint".to_string(),
            envelope: MessageEnvelope {
                text: "anything".to_string(),
                channel: None,
                provider: None,
            },
            session_active: false,
            input_locale: "en-US".to_string(),
            time_budget_ms: 200,
            registry_path: "/mnt/registry/latest.json".to_string(),
            indexes_path: "/tmp/does-not-exist-fast2flow-m1-3".to_string(),
            now_unix_ms: 0,
            messaging_endpoint_id: Some(MessagingEndpointId::new("teams-unknown").unwrap()),
        },
    )
    .await;

    assert!(matches!(output.directive, RoutingDirective::Continue));
}
