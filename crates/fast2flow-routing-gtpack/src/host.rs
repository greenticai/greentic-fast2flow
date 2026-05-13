use std::sync::Arc;

use anyhow::Result;
use fast2flow_contracts::{
    Fast2FlowHookInV1, Fast2FlowHookOutV1, RoutingExecutionTraceV1, RoutingPolicyV1,
};
use fast2flow_core::{CoreRouter, RouterConfig};
use fast2flow_hooks::DefaultHookFilter;
use fast2flow_llm::LlmProvider;
use fast2flow_strategy::RoutingStrategy;
use fast2flow_strategy_phase1::Phase1DeterministicStrategy;
use tracing::{debug, info, warn};

use crate::config::{build_llm, RouterBootstrapConfig};
use crate::handle_hook_from_mounts;
use crate::policy::{load_policy_from_env, resolve_policy, validate_policy};

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
        let (filter, config, _) = self.resolve_request_policy(&request);
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
        let (filter, config, policy_trace) = self.resolve_request_policy(&request);
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
