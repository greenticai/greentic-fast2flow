use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use fast2flow_contracts::{
    Candidate, Fast2FlowHookInV1, Fast2FlowHookOutV1, PolicyAppliedRuleV1, PolicyEffectiveConfigV1,
    PolicyResolutionV1, PolicyRuleV1, PolicyStageV1, RoutingDirective, RoutingExecutionTraceV1,
    RoutingPolicyV1, TextMatchModeV1,
};
use fast2flow_core::{CandidateIndex, CoreRouter, RouterConfig};
use fast2flow_hooks::{DefaultHookFilter, RespondRule};
use fast2flow_indexer::{load_latest, IndexStore};
use fast2flow_llm::LlmProvider;
use fast2flow_llm_ollama::OllamaProvider;
use fast2flow_llm_openai::OpenAiProvider;
use fast2flow_strategy::RoutingStrategy;
use fast2flow_strategy_phase1::Phase1DeterministicStrategy;

pub const REGISTRY_MOUNT: &str = "/mnt/registry";
pub const INDEXES_MOUNT: &str = "/mnt/indexes";
pub const ENV_LLM_PROVIDER: &str = "FAST2FLOW_LLM_PROVIDER";
pub const ENV_MIN_CONFIDENCE: &str = "FAST2FLOW_MIN_CONFIDENCE";
pub const ENV_LLM_MIN_CONFIDENCE: &str = "FAST2FLOW_LLM_MIN_CONFIDENCE";
pub const ENV_CANDIDATE_LIMIT: &str = "FAST2FLOW_CANDIDATE_LIMIT";
pub const ENV_OPENAI_API_KEY_PATH: &str = "FAST2FLOW_OPENAI_API_KEY_PATH";
pub const ENV_OPENAI_MODEL_PATH: &str = "FAST2FLOW_OPENAI_MODEL_PATH";
pub const ENV_OLLAMA_ENDPOINT_PATH: &str = "FAST2FLOW_OLLAMA_ENDPOINT_PATH";
pub const ENV_OLLAMA_MODEL_PATH: &str = "FAST2FLOW_OLLAMA_MODEL_PATH";
pub const ENV_POLICY_PATH: &str = "FAST2FLOW_POLICY_PATH";
pub const ENV_TRACE_POLICY: &str = "FAST2FLOW_TRACE_POLICY";
pub const DEFAULT_POLICY_PATH: &str = "/mnt/registry/fast2flow-policy.json";

pub async fn handle_hook(
    router: &CoreRouter,
    index: &dyn CandidateIndex,
    request: Fast2FlowHookInV1,
) -> Fast2FlowHookOutV1 {
    router.route(request, index).await
}

pub async fn handle_hook_from_mounts(
    router: &CoreRouter,
    request: Fast2FlowHookInV1,
) -> Fast2FlowHookOutV1 {
    let indexes_path = if request.indexes_path.is_empty() {
        INDEXES_MOUNT
    } else {
        request.indexes_path.as_str()
    };

    let lookup = match MountedIndexLookup::load(indexes_path, &request.scope) {
        Ok(lookup) => lookup,
        Err(_) => {
            return Fast2FlowHookOutV1 {
                directive: RoutingDirective::Continue,
            };
        }
    };

    router.route(request, &lookup).await
}

#[derive(Debug, Clone)]
pub struct MountedIndexLookup {
    scope: String,
    store: IndexStore,
}

impl MountedIndexLookup {
    pub fn load(indexes_path: &str, scope: &str) -> anyhow::Result<Self> {
        let store = load_latest(Path::new(indexes_path), scope)?;
        Ok(Self {
            scope: scope.to_string(),
            store,
        })
    }
}

impl CandidateIndex for MountedIndexLookup {
    fn search(&self, scope: &str, text: &str, limit: usize) -> Vec<Candidate> {
        if scope != self.scope {
            return Vec::new();
        }
        self.store.search(text, limit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmRuntimeConfig {
    Disabled,
    OpenAi {
        api_key_secret_path: String,
        model_secret_path: Option<String>,
    },
    Ollama {
        endpoint_secret_path: Option<String>,
        model_secret_path: String,
    },
}

#[derive(Debug, Clone)]
pub struct RouterBootstrapConfig {
    pub min_confidence: f32,
    pub llm_min_confidence: f32,
    pub candidate_limit: usize,
    pub llm: LlmRuntimeConfig,
}

impl Default for RouterBootstrapConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.5,
            llm_min_confidence: 0.5,
            candidate_limit: 20,
            llm: LlmRuntimeConfig::Disabled,
        }
    }
}

impl RouterBootstrapConfig {
    pub fn from_env() -> Result<Self> {
        let mut config = Self::default();

        if let Some(value) = env_var(ENV_MIN_CONFIDENCE) {
            config.min_confidence =
                f32::from_str(&value).with_context(|| format!("invalid {}", ENV_MIN_CONFIDENCE))?;
        }
        if let Some(value) = env_var(ENV_LLM_MIN_CONFIDENCE) {
            config.llm_min_confidence = f32::from_str(&value)
                .with_context(|| format!("invalid {}", ENV_LLM_MIN_CONFIDENCE))?;
        }
        if let Some(value) = env_var(ENV_CANDIDATE_LIMIT) {
            config.candidate_limit = usize::from_str(&value)
                .with_context(|| format!("invalid {}", ENV_CANDIDATE_LIMIT))?;
        }

        config.llm = parse_llm_from_env()?;
        Ok(config)
    }
}

pub async fn build_router_from_config(config: RouterBootstrapConfig) -> Result<CoreRouter> {
    let strategy = Arc::new(Phase1DeterministicStrategy);
    let filter = Arc::new(DefaultHookFilter::default());
    let llm = build_llm(&config.llm).await?;
    Ok(CoreRouter::new(
        strategy,
        vec![filter],
        llm,
        RouterConfig {
            min_confidence: config.min_confidence,
            llm_min_confidence: config.llm_min_confidence,
            candidate_limit: config.candidate_limit,
        },
    ))
}

pub async fn build_router_from_env() -> Result<CoreRouter> {
    let config = RouterBootstrapConfig::from_env()?;
    build_router_from_config(config).await
}

pub struct HostRuntime {
    strategy: Arc<dyn RoutingStrategy>,
    llm: Option<Arc<dyn LlmProvider>>,
    base_config: RouterConfig,
    base_filter: DefaultHookFilter,
    policy: Option<RoutingPolicyV1>,
}

impl HostRuntime {
    pub async fn boot_from_env() -> Result<Self> {
        let config = RouterBootstrapConfig::from_env()?;
        let policy = load_policy_from_env()?;
        Self::boot_from_config_with_policy(config, policy).await
    }

    pub async fn boot_from_config(config: RouterBootstrapConfig) -> Result<Self> {
        Self::boot_from_config_with_policy(config, None).await
    }

    pub async fn boot_from_config_with_policy(
        config: RouterBootstrapConfig,
        policy: Option<RoutingPolicyV1>,
    ) -> Result<Self> {
        if let Some(policy) = policy.as_ref() {
            validate_policy(policy)?;
        }
        let strategy: Arc<dyn RoutingStrategy> = Arc::new(Phase1DeterministicStrategy);
        let llm = build_llm(&config.llm).await?;
        Ok(Self {
            strategy,
            llm,
            base_config: RouterConfig {
                min_confidence: config.min_confidence,
                llm_min_confidence: config.llm_min_confidence,
                candidate_limit: config.candidate_limit,
            },
            base_filter: DefaultHookFilter::default(),
            policy,
        })
    }

    pub async fn route_from_mounts(&self, request: Fast2FlowHookInV1) -> Fast2FlowHookOutV1 {
        let (filter, config, _) = self.resolve_request_policy_with_trace(&request);
        let router = CoreRouter::new(
            Arc::clone(&self.strategy),
            vec![Arc::new(filter)],
            self.llm.clone(),
            config,
        );
        handle_hook_from_mounts(&router, request).await
    }

    pub async fn route_from_mounts_with_trace(
        &self,
        request: Fast2FlowHookInV1,
    ) -> (Fast2FlowHookOutV1, RoutingExecutionTraceV1) {
        let (filter, config, policy_trace) = self.resolve_request_policy_with_trace(&request);
        let router = CoreRouter::new(
            Arc::clone(&self.strategy),
            vec![Arc::new(filter)],
            self.llm.clone(),
            config,
        );
        let output = handle_hook_from_mounts(&router, request).await;
        let trace = RoutingExecutionTraceV1 {
            policy: policy_trace,
            directive: output.directive.clone(),
        };
        (output, trace)
    }

    pub fn policy(&self) -> Option<&RoutingPolicyV1> {
        self.policy.as_ref()
    }

    pub fn build_base_router(&self) -> CoreRouter {
        CoreRouter::new(
            Arc::clone(&self.strategy),
            vec![Arc::new(self.base_filter.clone())],
            self.llm.clone(),
            self.base_config.clone(),
        )
    }

    fn resolve_request_policy_with_trace(
        &self,
        request: &Fast2FlowHookInV1,
    ) -> (DefaultHookFilter, RouterConfig, Option<PolicyResolutionV1>) {
        let mut filter = self.base_filter.clone();
        let mut config = self.base_config.clone();
        let Some(policy) = self.policy.as_ref() else {
            return (filter, config, None);
        };

        let mut tracker = PolicyTracker::default();
        apply_policy_rule(
            &mut filter,
            &mut config,
            &policy.default,
            "default",
            &mut tracker,
        );
        let mut seen_stages = HashSet::new();
        let mut stage_order = if policy.stage_order.is_empty() {
            vec![
                PolicyStageV1::Scope,
                PolicyStageV1::Channel,
                PolicyStageV1::Provider,
            ]
        } else {
            policy.stage_order.clone()
        };
        for stage in stage_order.drain(..) {
            if !seen_stages.insert(stage.clone()) {
                continue;
            }
            match stage {
                PolicyStageV1::Scope => {
                    apply_scope_overrides(policy, request, &mut filter, &mut config, &mut tracker)
                }
                PolicyStageV1::Channel => {
                    apply_channel_overrides(policy, request, &mut filter, &mut config, &mut tracker)
                }
                PolicyStageV1::Provider => apply_provider_overrides(
                    policy,
                    request,
                    &mut filter,
                    &mut config,
                    &mut tracker,
                ),
            }
        }

        let effective = PolicyEffectiveConfigV1 {
            min_confidence: config.min_confidence,
            llm_min_confidence: config.llm_min_confidence,
            candidate_limit: config.candidate_limit,
            allow_channels: filter.allow_channels.clone(),
            deny_channels: filter.deny_channels.clone(),
            allow_providers: filter.allow_providers.clone(),
            deny_providers: filter.deny_providers.clone(),
            allow_scopes: filter.allow_scopes.clone(),
            deny_scopes: filter.deny_scopes.clone(),
            respond_rule_count: filter.respond_rules.len(),
        };
        let trace = PolicyResolutionV1 {
            applied: tracker.applied,
            warnings: tracker.warnings,
            effective,
        };

        (filter, config, Some(trace))
    }
}

fn apply_policy_rule(
    filter: &mut DefaultHookFilter,
    config: &mut RouterConfig,
    rule: &PolicyRuleV1,
    source: &str,
    tracker: &mut PolicyTracker,
) {
    if let Some(value) = rule.min_confidence {
        track_policy_set(tracker, source, "min_confidence", format!("{:.4}", value));
        config.min_confidence = value;
    }
    if let Some(value) = rule.llm_min_confidence {
        track_policy_set(
            tracker,
            source,
            "llm_min_confidence",
            format!("{:.4}", value),
        );
        config.llm_min_confidence = value;
    }
    if let Some(value) = rule.candidate_limit {
        track_policy_set(tracker, source, "candidate_limit", value.to_string());
        config.candidate_limit = value;
    }
    if let Some(value) = &rule.allow_channels {
        track_policy_set(
            tracker,
            source,
            "allow_channels",
            serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string()),
        );
        filter.allow_channels = Some(value.clone());
    }
    if let Some(value) = &rule.deny_channels {
        track_policy_set(
            tracker,
            source,
            "deny_channels",
            serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string()),
        );
        filter.deny_channels = value.clone();
    }
    if let Some(value) = &rule.allow_providers {
        track_policy_set(
            tracker,
            source,
            "allow_providers",
            serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string()),
        );
        filter.allow_providers = Some(value.clone());
    }
    if let Some(value) = &rule.deny_providers {
        track_policy_set(
            tracker,
            source,
            "deny_providers",
            serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string()),
        );
        filter.deny_providers = value.clone();
    }
    if let Some(value) = &rule.allow_scopes {
        track_policy_set(
            tracker,
            source,
            "allow_scopes",
            serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string()),
        );
        filter.allow_scopes = Some(value.clone());
    }
    if let Some(value) = &rule.deny_scopes {
        track_policy_set(
            tracker,
            source,
            "deny_scopes",
            serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string()),
        );
        filter.deny_scopes = value.clone();
    }
    if let Some(value) = &rule.respond_rules {
        track_policy_set(
            tracker,
            source,
            "respond_rules",
            serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string()),
        );
        filter.respond_rules = value
            .iter()
            .map(|entry| RespondRule {
                needle: entry.needle.clone(),
                message: entry.message.clone(),
                mode: entry.mode.clone(),
            })
            .collect();
    }
}

fn apply_scope_overrides(
    policy: &RoutingPolicyV1,
    request: &Fast2FlowHookInV1,
    filter: &mut DefaultHookFilter,
    config: &mut RouterConfig,
    tracker: &mut PolicyTracker,
) {
    let mut matching = policy
        .scope_overrides
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.scope.eq_ignore_ascii_case(&request.scope))
        .collect::<Vec<_>>();
    matching.sort_by_key(|(index, entry)| (entry.priority, *index));
    for (_, entry) in matching {
        apply_policy_rule(
            filter,
            config,
            &entry.rules,
            &source_label("scope", &entry.scope, entry.priority, entry.id.as_deref()),
            tracker,
        );
    }
}

fn apply_channel_overrides(
    policy: &RoutingPolicyV1,
    request: &Fast2FlowHookInV1,
    filter: &mut DefaultHookFilter,
    config: &mut RouterConfig,
    tracker: &mut PolicyTracker,
) {
    let Some(channel) = request.envelope.channel.as_ref() else {
        return;
    };
    let mut matching = policy
        .channel_overrides
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.channel.eq_ignore_ascii_case(channel))
        .collect::<Vec<_>>();
    matching.sort_by_key(|(index, entry)| (entry.priority, *index));
    for (_, entry) in matching {
        apply_policy_rule(
            filter,
            config,
            &entry.rules,
            &source_label(
                "channel",
                &entry.channel,
                entry.priority,
                entry.id.as_deref(),
            ),
            tracker,
        );
    }
}

fn apply_provider_overrides(
    policy: &RoutingPolicyV1,
    request: &Fast2FlowHookInV1,
    filter: &mut DefaultHookFilter,
    config: &mut RouterConfig,
    tracker: &mut PolicyTracker,
) {
    let Some(provider) = request.envelope.provider.as_ref() else {
        return;
    };
    let mut matching = policy
        .provider_overrides
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.provider.eq_ignore_ascii_case(provider))
        .collect::<Vec<_>>();
    matching.sort_by_key(|(index, entry)| (entry.priority, *index));
    for (_, entry) in matching {
        apply_policy_rule(
            filter,
            config,
            &entry.rules,
            &source_label(
                "provider",
                &entry.provider,
                entry.priority,
                entry.id.as_deref(),
            ),
            tracker,
        );
    }
}

fn source_label(kind: &str, key: &str, priority: i32, id: Option<&str>) -> String {
    match id {
        Some(value) if !value.trim().is_empty() => {
            format!("{kind}:{key}#{}@{priority}", value.trim())
        }
        _ => format!("{kind}:{key}@{priority}"),
    }
}

#[derive(Debug, Default)]
struct PolicyTracker {
    seen: HashMap<String, (String, String)>,
    applied: Vec<PolicyAppliedRuleV1>,
    warnings: Vec<String>,
}

fn track_policy_set(tracker: &mut PolicyTracker, source: &str, field: &str, value: String) {
    if let Some((previous_source, previous_value)) = tracker.seen.get(field) {
        if previous_value != &value {
            tracker.warnings.push(format!(
                "field '{}' overwritten: '{}' from {} -> '{}' from {}",
                field, previous_value, previous_source, value, source
            ));
        }
    }
    tracker
        .seen
        .insert(field.to_string(), (source.to_string(), value.clone()));
    tracker.applied.push(PolicyAppliedRuleV1 {
        source: source.to_string(),
        field: field.to_string(),
        value,
    });
}

pub fn load_policy_from_path(path: &Path) -> Result<Option<RoutingPolicyV1>> {
    if !path.exists() {
        return Ok(None);
    }
    let payload =
        fs::read_to_string(path).with_context(|| format!("failed reading {}", path.display()))?;
    let policy = serde_json::from_str::<RoutingPolicyV1>(&payload)
        .with_context(|| format!("failed parsing {}", path.display()))?;
    validate_policy(&policy).with_context(|| format!("invalid policy in {}", path.display()))?;
    Ok(Some(policy))
}

pub fn load_policy_from_env() -> Result<Option<RoutingPolicyV1>> {
    if let Some(path) = env_var(ENV_POLICY_PATH) {
        return load_policy_from_path(Path::new(&path));
    }
    load_policy_from_path(&PathBuf::from(DEFAULT_POLICY_PATH))
}

async fn build_llm(config: &LlmRuntimeConfig) -> Result<Option<Arc<dyn LlmProvider>>> {
    let provider: Option<Arc<dyn LlmProvider>> = match config {
        LlmRuntimeConfig::Disabled => None,
        LlmRuntimeConfig::OpenAi {
            api_key_secret_path,
            model_secret_path,
        } => {
            let provider =
                OpenAiProvider::from_secrets(api_key_secret_path, model_secret_path.as_deref())
                    .await
                    .map_err(|err| anyhow!("openai secrets bootstrap failed: {err}"))?;
            Some(Arc::new(provider))
        }
        LlmRuntimeConfig::Ollama {
            endpoint_secret_path,
            model_secret_path,
        } => {
            let provider =
                OllamaProvider::from_secrets(endpoint_secret_path.as_deref(), model_secret_path)
                    .await
                    .map_err(|err| anyhow!("ollama secrets bootstrap failed: {err}"))?;
            Some(Arc::new(provider))
        }
    };
    Ok(provider)
}

fn parse_llm_from_env() -> Result<LlmRuntimeConfig> {
    let provider = env_var(ENV_LLM_PROVIDER)
        .unwrap_or_else(|| "disabled".to_string())
        .to_ascii_lowercase();

    match provider.as_str() {
        "" | "disabled" | "none" => Ok(LlmRuntimeConfig::Disabled),
        "openai" => {
            let api_key_secret_path =
                env_var(ENV_OPENAI_API_KEY_PATH).unwrap_or_else(|| "OPENAI_API_KEY".to_string());
            let model_secret_path = env_var(ENV_OPENAI_MODEL_PATH);
            Ok(LlmRuntimeConfig::OpenAi {
                api_key_secret_path,
                model_secret_path,
            })
        }
        "ollama" => {
            let model_secret_path = env_var(ENV_OLLAMA_MODEL_PATH).ok_or_else(|| {
                anyhow!(
                    "{} is required when FAST2FLOW_LLM_PROVIDER=ollama",
                    ENV_OLLAMA_MODEL_PATH
                )
            })?;
            let endpoint_secret_path = env_var(ENV_OLLAMA_ENDPOINT_PATH);
            Ok(LlmRuntimeConfig::Ollama {
                endpoint_secret_path,
                model_secret_path,
            })
        }
        _ => Err(anyhow!(
            "unsupported {} value: {}",
            ENV_LLM_PROVIDER,
            provider
        )),
    }
}

fn env_var(key: &str) -> Option<String> {
    std::env::var(key).ok().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn validate_policy(policy: &RoutingPolicyV1) -> Result<()> {
    if policy.stage_order.is_empty() {
        return Err(anyhow!("policy stage_order must not be empty"));
    }

    let mut seen_stages = HashSet::new();
    for stage in &policy.stage_order {
        if !seen_stages.insert(stage) {
            return Err(anyhow!("policy stage_order contains duplicate stage"));
        }
    }

    validate_policy_rule(&policy.default, "default")?;
    for entry in &policy.scope_overrides {
        let scope = entry.scope.trim();
        if scope.is_empty() {
            return Err(anyhow!("scope override has empty scope"));
        }
        if matches!(entry.id.as_deref(), Some(id) if id.trim().is_empty()) {
            return Err(anyhow!("scope override id must not be empty"));
        }
        validate_policy_rule(&entry.rules, &format!("scope:{scope}"))?;
    }
    for entry in &policy.channel_overrides {
        let channel = entry.channel.trim();
        if channel.is_empty() {
            return Err(anyhow!("channel override has empty channel"));
        }
        if matches!(entry.id.as_deref(), Some(id) if id.trim().is_empty()) {
            return Err(anyhow!("channel override id must not be empty"));
        }
        validate_policy_rule(&entry.rules, &format!("channel:{channel}"))?;
    }
    for entry in &policy.provider_overrides {
        let provider = entry.provider.trim();
        if provider.is_empty() {
            return Err(anyhow!("provider override has empty provider"));
        }
        if matches!(entry.id.as_deref(), Some(id) if id.trim().is_empty()) {
            return Err(anyhow!("provider override id must not be empty"));
        }
        validate_policy_rule(&entry.rules, &format!("provider:{provider}"))?;
    }

    Ok(())
}

fn validate_policy_rule(rule: &PolicyRuleV1, source: &str) -> Result<()> {
    if let Some(value) = rule.min_confidence {
        if !(0.0..=1.0).contains(&value) {
            return Err(anyhow!(
                "{source} min_confidence must be between 0 and 1, got {value}"
            ));
        }
    }
    if let Some(value) = rule.llm_min_confidence {
        if !(0.0..=1.0).contains(&value) {
            return Err(anyhow!(
                "{source} llm_min_confidence must be between 0 and 1, got {value}"
            ));
        }
    }
    if matches!(rule.candidate_limit, Some(0)) {
        return Err(anyhow!("{source} candidate_limit must be > 0"));
    }
    if let Some(rules) = &rule.respond_rules {
        for (idx, respond) in rules.iter().enumerate() {
            if respond.needle.trim().is_empty() {
                return Err(anyhow!(
                    "{source} respond_rules[{idx}] needle must not be empty"
                ));
            }
            if respond.message.trim().is_empty() {
                return Err(anyhow!(
                    "{source} respond_rules[{idx}] message must not be empty"
                ));
            }
            if respond.mode == TextMatchModeV1::Regex {
                regex::Regex::new(&respond.needle)
                    .map_err(|err| anyhow!("{source} respond_rules[{idx}] invalid regex: {err}"))?;
            }
        }
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
mod generated_bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "fast2flow-routing",
    });
}

#[cfg(target_arch = "wasm32")]
pub mod wit_entrypoint {
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::OnceLock;

    use fast2flow_contracts::{
        Fast2FlowHookInV1, Fast2FlowHookOutV1, MessageEnvelope, RoutingDirective,
    };
    use fast2flow_core::CoreRouter;
    use fast2flow_indexer::load_latest;
    use futures::executor::block_on;

    use super::generated_bindings::exports::greentic::fast2flow::routing_hook::Guest;
    use super::generated_bindings::greentic::fast2flow::routing_types::{
        Fast2flowHookInV1 as WitIn, Fast2flowHookOutV1 as WitOut, MessageEnvelope as WitEnvelope,
        RoutingDirective as WitDirective,
    };
    use super::{MountedIndexLookup, INDEXES_MOUNT};

    pub trait WitRoutingRuntime: Send + Sync {
        fn route(&self, request: Fast2FlowHookInV1) -> Fast2FlowHookOutV1;
    }

    static ROUTING_RUNTIME: OnceLock<Box<dyn WitRoutingRuntime>> = OnceLock::new();

    pub fn install_runtime(runtime: Box<dyn WitRoutingRuntime>) -> Result<(), &'static str> {
        ROUTING_RUNTIME
            .set(runtime)
            .map_err(|_| "routing runtime already installed")
    }

    pub struct MountedRuntime {
        router: Arc<CoreRouter>,
    }

    impl MountedRuntime {
        pub fn new(router: Arc<CoreRouter>) -> Self {
            Self { router }
        }
    }

    impl WitRoutingRuntime for MountedRuntime {
        fn route(&self, request: Fast2FlowHookInV1) -> Fast2FlowHookOutV1 {
            let indexes_path = if request.indexes_path.is_empty() {
                INDEXES_MOUNT
            } else {
                request.indexes_path.as_str()
            };
            let scope = request.scope.clone();
            let store = match load_latest(Path::new(indexes_path), &scope) {
                Ok(store) => store,
                Err(_) => {
                    return Fast2FlowHookOutV1 {
                        directive: RoutingDirective::Continue,
                    }
                }
            };
            let lookup = MountedIndexLookup { scope, store };
            block_on(self.router.route(request, &lookup))
        }
    }

    pub fn install_mounted_runtime(router: Arc<CoreRouter>) -> Result<(), &'static str> {
        install_runtime(Box::new(MountedRuntime::new(router)))
    }

    pub fn install_mounted_runtime_from_env() -> Result<(), String> {
        let router = block_on(super::build_router_from_env())
            .map_err(|err| format!("failed to build router from env: {err}"))?;
        install_mounted_runtime(Arc::new(router)).map_err(|err| err.to_string())
    }

    pub struct Component;

    impl Guest for Component {
        fn handle_hook(request: WitIn) -> WitOut {
            let request = map_in(request);
            let output = ROUTING_RUNTIME
                .get()
                .map(|runtime| runtime.route(request))
                .unwrap_or(Fast2FlowHookOutV1 {
                    directive: RoutingDirective::Continue,
                });
            map_out(output)
        }
    }

    fn map_in(request: WitIn) -> Fast2FlowHookInV1 {
        let envelope = request.envelope;
        Fast2FlowHookInV1 {
            scope: request.scope,
            envelope: MessageEnvelope {
                text: envelope.text,
                channel: envelope.channel,
                provider: envelope.provider,
            },
            session_active: request.session_active,
            input_locale: request.input_locale,
            time_budget_ms: request.time_budget_ms,
            registry_path: request.registry_path,
            indexes_path: request.indexes_path,
            now_unix_ms: request.now_unix_ms,
        }
    }

    fn map_out(output: Fast2FlowHookOutV1) -> WitOut {
        let directive = match output.directive {
            RoutingDirective::Continue => WitDirective::Continue,
            RoutingDirective::Dispatch {
                target,
                confidence,
                reason,
            } => WitDirective::Dispatch((target, confidence, reason)),
            RoutingDirective::Respond { message } => WitDirective::Respond(message),
            RoutingDirective::Deny { reason } => WitDirective::Deny(reason),
        };
        WitOut { directive }
    }

    #[allow(dead_code)]
    fn _ensure_wit_types_used(_value: WitEnvelope) {}
}
