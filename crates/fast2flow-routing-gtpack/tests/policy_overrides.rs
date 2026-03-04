use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fast2flow_contracts::{
    ChannelPolicyOverrideV1, Fast2FlowHookInV1, FlowDoc, MessageEnvelope, PolicyRuleV1,
    PolicyStageV1, ProviderPolicyOverrideV1, RespondRuleV1, RoutingDirective, RoutingPolicyV1,
    ScopePolicyOverrideV1, TextMatchModeV1,
};
use fast2flow_indexer::build_index;
use fast2flow_routing_gtpack::{load_policy_from_path, HostRuntime, RouterBootstrapConfig};

#[tokio::test]
async fn scope_override_can_tighten_confidence_threshold() {
    let scope = "tenant-a";
    let indexes_root = temp_indexes_dir();
    seed_refund_index(scope, &indexes_root);

    let policy = RoutingPolicyV1 {
        stage_order: vec![
            PolicyStageV1::Scope,
            PolicyStageV1::Channel,
            PolicyStageV1::Provider,
        ],
        default: PolicyRuleV1::default(),
        scope_overrides: vec![ScopePolicyOverrideV1 {
            id: None,
            priority: 0,
            scope: scope.to_string(),
            rules: PolicyRuleV1 {
                min_confidence: Some(0.95),
                ..PolicyRuleV1::default()
            },
        }],
        channel_overrides: vec![],
        provider_overrides: vec![],
    };

    let runtime =
        HostRuntime::boot_from_config_with_policy(RouterBootstrapConfig::default(), Some(policy))
            .await
            .expect("runtime should boot");

    let output = runtime
        .route_from_mounts(request(scope, "refund please", indexes_root.as_path()))
        .await;

    assert!(matches!(output.directive, RoutingDirective::Continue));
}

#[tokio::test]
async fn channel_override_can_force_respond() {
    let scope = "tenant-a";
    let indexes_root = temp_indexes_dir();
    seed_refund_index(scope, &indexes_root);

    let policy = RoutingPolicyV1 {
        stage_order: vec![
            PolicyStageV1::Scope,
            PolicyStageV1::Channel,
            PolicyStageV1::Provider,
        ],
        default: PolicyRuleV1::default(),
        scope_overrides: vec![],
        channel_overrides: vec![ChannelPolicyOverrideV1 {
            id: None,
            priority: 0,
            channel: "chat".to_string(),
            rules: PolicyRuleV1 {
                respond_rules: Some(vec![RespondRuleV1 {
                    needle: "refund".to_string(),
                    message: "Use the self-service refund form".to_string(),
                    mode: TextMatchModeV1::Contains,
                }]),
                ..PolicyRuleV1::default()
            },
        }],
        provider_overrides: vec![],
    };

    let runtime =
        HostRuntime::boot_from_config_with_policy(RouterBootstrapConfig::default(), Some(policy))
            .await
            .expect("runtime should boot");

    let output = runtime
        .route_from_mounts(request(scope, "refund please", indexes_root.as_path()))
        .await;

    match output.directive {
        RoutingDirective::Respond { message } => {
            assert_eq!(message, "Use the self-service refund form");
        }
        other => panic!("expected respond, got {other:?}"),
    }
}

#[tokio::test]
async fn load_policy_from_path_parses_valid_json() {
    let path = std::env::temp_dir().join(format!(
        "fast2flow-policy-{}.json",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_nanos()
    ));

    let payload = serde_json::to_string_pretty(&RoutingPolicyV1 {
        stage_order: vec![
            PolicyStageV1::Scope,
            PolicyStageV1::Channel,
            PolicyStageV1::Provider,
        ],
        default: PolicyRuleV1 {
            candidate_limit: Some(5),
            ..PolicyRuleV1::default()
        },
        scope_overrides: vec![],
        channel_overrides: vec![],
        provider_overrides: vec![],
    })
    .expect("policy json must serialize");

    std::fs::write(&path, payload).expect("must write temp policy");

    let policy = load_policy_from_path(&path)
        .expect("loading policy should succeed")
        .expect("policy should exist");

    assert_eq!(policy.default.candidate_limit, Some(5));

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn route_with_trace_reports_policy_overwrites() {
    let scope = "tenant-a";
    let indexes_root = temp_indexes_dir();
    seed_refund_index(scope, &indexes_root);

    let policy = RoutingPolicyV1 {
        stage_order: vec![
            PolicyStageV1::Scope,
            PolicyStageV1::Channel,
            PolicyStageV1::Provider,
        ],
        default: PolicyRuleV1 {
            min_confidence: Some(0.5),
            ..PolicyRuleV1::default()
        },
        scope_overrides: vec![ScopePolicyOverrideV1 {
            id: Some("scope-tight".to_string()),
            priority: 10,
            scope: scope.to_string(),
            rules: PolicyRuleV1 {
                min_confidence: Some(0.9),
                ..PolicyRuleV1::default()
            },
        }],
        channel_overrides: vec![ChannelPolicyOverrideV1 {
            id: Some("channel-mid".to_string()),
            priority: 20,
            channel: "chat".to_string(),
            rules: PolicyRuleV1 {
                min_confidence: Some(0.7),
                ..PolicyRuleV1::default()
            },
        }],
        provider_overrides: vec![ProviderPolicyOverrideV1 {
            id: Some("provider-low".to_string()),
            priority: 30,
            provider: "tests".to_string(),
            rules: PolicyRuleV1 {
                min_confidence: Some(0.6),
                ..PolicyRuleV1::default()
            },
        }],
    };

    let runtime =
        HostRuntime::boot_from_config_with_policy(RouterBootstrapConfig::default(), Some(policy))
            .await
            .expect("runtime should boot");

    let (_out, trace) = runtime
        .route_from_mounts_with_trace(request(scope, "refund please", indexes_root.as_path()))
        .await;

    let policy_trace = trace
        .policy
        .expect("trace should include policy resolution");
    assert!(policy_trace
        .warnings
        .iter()
        .any(|warning| warning.contains("overwritten")));
    assert_eq!(policy_trace.effective.min_confidence, 0.6);
}

#[tokio::test]
async fn stage_order_changes_effective_precedence() {
    let scope = "tenant-a";
    let indexes_root = temp_indexes_dir();
    seed_refund_index(scope, &indexes_root);

    let policy = RoutingPolicyV1 {
        stage_order: vec![
            PolicyStageV1::Provider,
            PolicyStageV1::Channel,
            PolicyStageV1::Scope,
        ],
        default: PolicyRuleV1 {
            min_confidence: Some(0.4),
            ..PolicyRuleV1::default()
        },
        scope_overrides: vec![ScopePolicyOverrideV1 {
            id: Some("scope-final".to_string()),
            priority: 0,
            scope: scope.to_string(),
            rules: PolicyRuleV1 {
                min_confidence: Some(0.9),
                ..PolicyRuleV1::default()
            },
        }],
        channel_overrides: vec![ChannelPolicyOverrideV1 {
            id: Some("channel-mid".to_string()),
            priority: 0,
            channel: "chat".to_string(),
            rules: PolicyRuleV1 {
                min_confidence: Some(0.7),
                ..PolicyRuleV1::default()
            },
        }],
        provider_overrides: vec![ProviderPolicyOverrideV1 {
            id: Some("provider-first".to_string()),
            priority: 0,
            provider: "tests".to_string(),
            rules: PolicyRuleV1 {
                min_confidence: Some(0.6),
                ..PolicyRuleV1::default()
            },
        }],
    };

    let runtime =
        HostRuntime::boot_from_config_with_policy(RouterBootstrapConfig::default(), Some(policy))
            .await
            .expect("runtime should boot");

    let (_out, trace) = runtime
        .route_from_mounts_with_trace(request(scope, "refund please", indexes_root.as_path()))
        .await;

    let policy_trace = trace
        .policy
        .expect("trace should include policy resolution");
    assert_eq!(policy_trace.effective.min_confidence, 0.9);
}

#[tokio::test]
async fn higher_priority_override_wins_within_stage() {
    let scope = "tenant-a";
    let indexes_root = temp_indexes_dir();
    seed_refund_index(scope, &indexes_root);

    let policy = RoutingPolicyV1 {
        stage_order: vec![PolicyStageV1::Scope],
        default: PolicyRuleV1 {
            min_confidence: Some(0.4),
            ..PolicyRuleV1::default()
        },
        scope_overrides: vec![
            ScopePolicyOverrideV1 {
                id: Some("low".to_string()),
                priority: 10,
                scope: scope.to_string(),
                rules: PolicyRuleV1 {
                    min_confidence: Some(0.65),
                    ..PolicyRuleV1::default()
                },
            },
            ScopePolicyOverrideV1 {
                id: Some("high".to_string()),
                priority: 100,
                scope: scope.to_string(),
                rules: PolicyRuleV1 {
                    min_confidence: Some(0.95),
                    ..PolicyRuleV1::default()
                },
            },
        ],
        channel_overrides: vec![],
        provider_overrides: vec![],
    };

    let runtime =
        HostRuntime::boot_from_config_with_policy(RouterBootstrapConfig::default(), Some(policy))
            .await
            .expect("runtime should boot");

    let (_out, trace) = runtime
        .route_from_mounts_with_trace(request(scope, "refund please", indexes_root.as_path()))
        .await;

    let policy_trace = trace
        .policy
        .expect("trace should include policy resolution");
    assert_eq!(policy_trace.effective.min_confidence, 0.95);
}

#[tokio::test]
async fn invalid_regex_policy_is_rejected() {
    let policy = RoutingPolicyV1 {
        stage_order: vec![PolicyStageV1::Channel],
        default: PolicyRuleV1::default(),
        scope_overrides: vec![],
        channel_overrides: vec![ChannelPolicyOverrideV1 {
            id: None,
            priority: 0,
            channel: "chat".to_string(),
            rules: PolicyRuleV1 {
                respond_rules: Some(vec![RespondRuleV1 {
                    needle: "(unclosed".to_string(),
                    message: "bad".to_string(),
                    mode: TextMatchModeV1::Regex,
                }]),
                ..PolicyRuleV1::default()
            },
        }],
        provider_overrides: vec![],
    };

    match HostRuntime::boot_from_config_with_policy(RouterBootstrapConfig::default(), Some(policy))
        .await
    {
        Ok(_) => panic!("invalid regex must fail validation"),
        Err(err) => {
            let msg = err.to_string();
            assert!(msg.contains("invalid regex"));
            assert!(msg.contains("channel:chat"));
        }
    }
}

fn seed_refund_index(scope: &str, indexes_root: &std::path::Path) {
    let flows = vec![FlowDoc {
        id: "refund_flow".to_string(),
        pack_id: "support".to_string(),
        target: "support/refund_flow".to_string(),
        title: "Refund Request".to_string(),
        tags: vec!["refund".to_string(), "billing".to_string()],
        node_ids: vec!["start".to_string(), "issue_refund".to_string()],
    }];
    build_index(scope, &flows, indexes_root, 0).expect("index build should succeed");
}

fn request(scope: &str, text: &str, indexes_root: &std::path::Path) -> Fast2FlowHookInV1 {
    Fast2FlowHookInV1 {
        scope: scope.to_string(),
        envelope: MessageEnvelope {
            text: text.to_string(),
            channel: Some("chat".to_string()),
            provider: Some("tests".to_string()),
        },
        session_active: false,
        input_locale: "en-US".to_string(),
        time_budget_ms: 250,
        registry_path: "/mnt/registry/latest.json".to_string(),
        indexes_path: indexes_root.display().to_string(),
        now_unix_ms: 0,
    }
}

fn temp_indexes_dir() -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_nanos();
    std::env::temp_dir().join(format!("fast2flow-policy-idx-{suffix}"))
}
