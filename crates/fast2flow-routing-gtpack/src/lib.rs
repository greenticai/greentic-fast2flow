use std::path::Path;

use fast2flow_contracts::{
    Candidate, Fast2FlowHookInV1, Fast2FlowHookOutV1, PolicyResolutionV1, RoutingDirective,
};
use fast2flow_core::{CandidateIndex, CoreRouter};
use fast2flow_indexer::{load_latest, IndexStore};

mod config;
mod host;
mod policy;

#[cfg(not(target_arch = "wasm32"))]
pub mod telemetry;

pub use config::{
    build_router_from_config, build_router_from_env, LlmRuntimeConfig, RouterBootstrapConfig,
};
pub use host::HostRuntime;
pub use policy::{load_policy_from_env, load_policy_from_path};

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

/// Phase M1: canonicalize `request.scope` to `effective_scope()` in place.
///
/// Idempotent. Every entry point that touches scope (policy resolution,
/// index lookup, candidate-match guard) MUST run this first so they all
/// see the same string. Without it, a request carrying both
/// `messaging_endpoint_id` AND a stale `scope` resolves policy against
/// the stale scope but routes against the endpoint index.
pub(crate) fn canonicalize_scope(request: &mut Fast2FlowHookInV1) {
    let effective = request.effective_scope();
    if effective != request.scope {
        request.scope = effective;
    }
}

pub async fn handle_hook_from_mounts(
    router: &CoreRouter,
    mut request: Fast2FlowHookInV1,
) -> Fast2FlowHookOutV1 {
    canonicalize_scope(&mut request);

    let indexes_path = if request.indexes_path.is_empty() {
        INDEXES_MOUNT.to_string()
    } else {
        request.indexes_path.clone()
    };

    let lookup = match MountedIndexLookup::load(&indexes_path, &request.scope) {
        Ok(lookup) => lookup,
        Err(err) => {
            tracing::warn!(
                scope = %request.scope,
                indexes_path = %indexes_path,
                error = %err,
                "failed to load mounted index; routing → continue"
            );
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

#[cfg(target_arch = "wasm32")]
mod generated_bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "fast2flow-routing",
    });
}

#[cfg(target_arch = "wasm32")]
pub mod wit_entrypoint;
