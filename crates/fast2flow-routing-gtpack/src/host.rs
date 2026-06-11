use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Utc};
use fast2flow_contracts::{
    Fast2FlowHookInV1, Fast2FlowHookOutV1, RoutingDirective, RoutingEntity,
    RoutingExecutionTraceV1, RoutingPolicyV1,
};
use fast2flow_core::{CoreRouter, RouterConfig};
use fast2flow_hooks::DefaultHookFilter;
use fast2flow_llm::LlmProvider;
use fast2flow_strategy::RoutingStrategy;
use fast2flow_strategy_phase1::Phase1DeterministicStrategy;
use greentic_intent::{IntentContext, IntentEngine};
use tracing::{debug, info, warn};

use crate::config::{build_llm, RouterBootstrapConfig};
use crate::policy::{load_policy_from_env, resolve_policy, validate_policy};
use crate::{canonicalize_scope, handle_hook_from_mounts};

pub struct HostRuntime {
    strategy: Arc<dyn RoutingStrategy>,
    llm: Option<Arc<dyn LlmProvider>>,
    base_config: RouterConfig,
    base_filter: DefaultHookFilter,
    policy: Option<RoutingPolicyV1>,
    intent_engine: Arc<IntentEngine>,
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
        let policy_loaded = policy.is_some();
        let strategy: Arc<dyn RoutingStrategy> = Arc::new(Phase1DeterministicStrategy);
        let llm = build_llm(&config.llm).await?;
        info!(
            min_confidence = config.min_confidence,
            llm_min_confidence = config.llm_min_confidence,
            candidate_limit = config.candidate_limit,
            llm_enabled = llm.is_some(),
            policy_loaded,
            "fast2flow host runtime booted"
        );
        let intent_engine = Arc::new(
            IntentEngine::builder()
                .with_builtin_locales()
                .with_builtin_gazetteer()
                .with_default_extractors()
                .build(),
        );

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
            intent_engine,
        })
    }

    pub async fn route_from_mounts(&self, mut request: Fast2FlowHookInV1) -> Fast2FlowHookOutV1 {
        // M1.3: canonicalize BEFORE policy so `scope_overrides` keyed on
        // `endpoint:{id}` match when the caller supplied a stale scope
        // alongside a `messaging_endpoint_id`. Without this, policy
        // resolves against the stale scope while the routing layer below
        // dispatches via the endpoint index — split-brain.
        canonicalize_scope(&mut request);
        let entities = self.extract_entities(&request);
        let (filter, config, _) = self.resolve_request_policy(&request);
        let router = CoreRouter::new(
            Arc::clone(&self.strategy),
            vec![Arc::new(filter)],
            self.llm.clone(),
            config,
        );
        let mut output = handle_hook_from_mounts(&router, request).await;
        attach_entities(&mut output, entities);
        output
    }

    pub async fn route_from_mounts_with_trace(
        &self,
        mut request: Fast2FlowHookInV1,
    ) -> (Fast2FlowHookOutV1, RoutingExecutionTraceV1) {
        canonicalize_scope(&mut request);
        let entities = self.extract_entities(&request);
        let (filter, config, policy_trace) = self.resolve_request_policy(&request);
        let router = CoreRouter::new(
            Arc::clone(&self.strategy),
            vec![Arc::new(filter)],
            self.llm.clone(),
            config,
        );
        let mut output = handle_hook_from_mounts(&router, request).await;
        attach_entities(&mut output, entities);
        let trace = RoutingExecutionTraceV1 {
            policy: policy_trace,
            directive: output.directive.clone(),
        };
        (output, trace)
    }

    /// Run intent over the inbound text, returning prefill entities.
    fn extract_entities(&self, request: &Fast2FlowHookInV1) -> Vec<RoutingEntity> {
        let text = request.envelope.text.as_str();
        if text.trim().is_empty() {
            return Vec::new();
        }
        let reference_time = DateTime::<Utc>::from_timestamp_millis(request.now_unix_ms as i64)
            .unwrap_or_else(Utc::now);
        let language_tag = request
            .input_locale
            .split('-')
            .next()
            .unwrap_or("en")
            .to_string();
        let ctx = IntentContext {
            reference_time,
            timezone: "UTC".into(),
            preferred_locale: Some(request.input_locale.clone()),
            tenant_locale: None,
            user_locale: None,
            allowed_languages: vec![language_tag],
        };
        let result = self.intent_engine.mark(text, &ctx);
        result
            .entities
            .into_iter()
            .map(|e| {
                let kind = e.kind.marker_name().to_string();
                let formats = entity_formats(&kind, &e.normalized);
                RoutingEntity {
                    kind,
                    normalized: e.normalized,
                    role: e.role,
                    formats,
                }
            })
            .collect()
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

    fn resolve_request_policy(
        &self,
        request: &Fast2FlowHookInV1,
    ) -> (
        DefaultHookFilter,
        RouterConfig,
        Option<crate::PolicyResolutionV1>,
    ) {
        let Some(policy) = self.policy.as_ref() else {
            return (self.base_filter.clone(), self.base_config.clone(), None);
        };

        let (filter, config, trace) =
            resolve_policy(policy, request, &self.base_filter, &self.base_config);
        if trace.warnings.is_empty() {
            debug!(
                scope = %request.scope,
                applied = trace.applied.len(),
                "policy resolved for request"
            );
        } else {
            warn!(
                scope = %request.scope,
                applied = trace.applied.len(),
                warnings = ?trace.warnings,
                "policy resolution produced warnings"
            );
        }
        (filter, config, Some(trace))
    }
}

/// Alternate serializations per entity kind. Empty when none apply.
fn entity_formats(kind: &str, normalized: &str) -> std::collections::BTreeMap<String, String> {
    let mut formats = std::collections::BTreeMap::new();
    if kind == "date" && normalized.len() == 8 && normalized.bytes().all(|b| b.is_ascii_digit()) {
        // Adaptive Input.Date + ISO 8601 want YYYY-MM-DD, not YYYYMMDD.
        formats.insert(
            "iso".to_string(),
            format!(
                "{}-{}-{}",
                &normalized[..4],
                &normalized[4..6],
                &normalized[6..]
            ),
        );
    }
    formats
}

/// Write entities into a Dispatch directive; no-op for other variants.
fn attach_entities(output: &mut Fast2FlowHookOutV1, entities: Vec<RoutingEntity>) {
    if entities.is_empty() {
        return;
    }
    if let RoutingDirective::Dispatch {
        entities: ref mut slot,
        ..
    } = output.directive
    {
        *slot = entities;
    }
}
