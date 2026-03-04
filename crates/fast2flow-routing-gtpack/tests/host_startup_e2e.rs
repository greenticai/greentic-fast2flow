use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fast2flow_contracts::{Fast2FlowHookInV1, FlowDoc, MessageEnvelope, RoutingDirective};
use fast2flow_indexer::build_index;
use fast2flow_routing_gtpack::{HostRuntime, ENV_LLM_PROVIDER};

#[tokio::test]
#[serial_test::serial]
async fn host_runtime_boot_from_env_routes_with_mounted_indexes() {
    std::env::set_var(ENV_LLM_PROVIDER, "disabled");

    let scope = "tenant-e2e";
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

    let runtime = HostRuntime::boot_from_env()
        .await
        .expect("host runtime should bootstrap from env");

    let output = runtime
        .route_from_mounts(Fast2FlowHookInV1 {
            scope: scope.to_string(),
            envelope: MessageEnvelope {
                text: "refund please".to_string(),
                channel: Some("chat".to_string()),
                provider: Some("tests".to_string()),
            },
            session_active: false,
            input_locale: "en-US".to_string(),
            time_budget_ms: 250,
            registry_path: "/mnt/registry/latest.json".to_string(),
            indexes_path: indexes_root.display().to_string(),
            now_unix_ms: 0,
        })
        .await;

    std::env::remove_var(ENV_LLM_PROVIDER);

    match output.directive {
        RoutingDirective::Dispatch { target, .. } => {
            assert_eq!(target, "support/refund_flow");
        }
        other => panic!("expected dispatch, got {other:?}"),
    }
}

#[tokio::test]
#[serial_test::serial]
async fn host_runtime_boot_from_env_rejects_unknown_provider() {
    std::env::set_var(ENV_LLM_PROVIDER, "unknown-provider");

    let result = HostRuntime::boot_from_env().await;

    std::env::remove_var(ENV_LLM_PROVIDER);
    assert!(result.is_err());
}

fn temp_indexes_dir() -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_nanos();
    std::env::temp_dir().join(format!("fast2flow-host-e2e-{suffix}"))
}
