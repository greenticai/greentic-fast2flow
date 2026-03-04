use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fast2flow_contracts::{Fast2FlowHookInV1, FlowDoc, MessageEnvelope, RoutingDirective};
use fast2flow_core::{CoreRouter, RouterConfig};
use fast2flow_hooks::DefaultHookFilter;
use fast2flow_indexer::build_index;
use fast2flow_routing_gtpack::handle_hook_from_mounts;
use fast2flow_strategy_phase1::Phase1DeterministicStrategy;

#[tokio::test]
async fn handle_hook_from_mounts_dispatches_for_refund() {
    let scope = "tenant-a";
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
            scope: "tenant-missing".to_string(),
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
