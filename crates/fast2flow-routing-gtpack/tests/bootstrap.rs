use fast2flow_routing_gtpack::{
    build_router_from_config, LlmRuntimeConfig, RouterBootstrapConfig, ENV_LLM_PROVIDER,
    ENV_OLLAMA_MODEL_PATH, ENV_OPENAI_API_KEY_PATH,
};

#[tokio::test]
#[serial_test::serial]
async fn build_router_from_config_supports_disabled_llm() {
    let config = RouterBootstrapConfig::default();
    build_router_from_config(config)
        .await
        .expect("router with disabled llm should build");
}

#[tokio::test]
#[serial_test::serial]
async fn build_router_from_config_builds_openai_from_secret_env() {
    let key_var = "TEST_FAST2FLOW_OPENAI_KEY_1";
    std::env::set_var(key_var, "sk-test-123");

    let config = RouterBootstrapConfig {
        llm: LlmRuntimeConfig::OpenAi {
            api_key_secret_path: key_var.to_string(),
            model_secret_path: None,
        },
        ..RouterBootstrapConfig::default()
    };

    let result = build_router_from_config(config).await;

    std::env::remove_var(key_var);
    result.expect("openai config should build when secret env var is set");
}

#[tokio::test]
#[serial_test::serial]
async fn from_env_requires_ollama_model_path() {
    std::env::set_var(ENV_LLM_PROVIDER, "ollama");
    std::env::remove_var(ENV_OLLAMA_MODEL_PATH);

    let result = RouterBootstrapConfig::from_env();

    std::env::remove_var(ENV_LLM_PROVIDER);
    assert!(result.is_err());
}

#[tokio::test]
#[serial_test::serial]
async fn from_env_openai_uses_default_secret_key_name() {
    std::env::set_var(ENV_LLM_PROVIDER, "openai");
    std::env::remove_var(ENV_OPENAI_API_KEY_PATH);

    let config = RouterBootstrapConfig::from_env().expect("openai env config should parse");

    std::env::remove_var(ENV_LLM_PROVIDER);
    match config.llm {
        LlmRuntimeConfig::OpenAi {
            api_key_secret_path,
            model_secret_path,
        } => {
            assert_eq!(api_key_secret_path, "OPENAI_API_KEY");
            assert!(model_secret_path.is_none());
        }
        other => panic!("expected openai llm config, got {other:?}"),
    }
}
