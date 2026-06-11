use fast2flow_routing_gtpack::{
    build_router_from_config, RouterBootstrapConfig, ENV_CANDIDATE_LIMIT, ENV_MIN_CONFIDENCE,
};

#[tokio::test]
#[serial_test::serial]
async fn build_router_from_default_config() {
    let config = RouterBootstrapConfig::default();
    build_router_from_config(config)
        .await
        .expect("deterministic router should build");
}

#[tokio::test]
#[serial_test::serial]
async fn from_env_parses_thresholds() {
    std::env::set_var(ENV_MIN_CONFIDENCE, "0.3");
    std::env::set_var(ENV_CANDIDATE_LIMIT, "7");

    let config = RouterBootstrapConfig::from_env().expect("config should parse");

    std::env::remove_var(ENV_MIN_CONFIDENCE);
    std::env::remove_var(ENV_CANDIDATE_LIMIT);

    assert!((config.min_confidence - 0.3).abs() < 1e-6);
    assert_eq!(config.candidate_limit, 7);
}
