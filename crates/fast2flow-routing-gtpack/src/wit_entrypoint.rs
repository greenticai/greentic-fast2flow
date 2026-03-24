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

super::generated_bindings::export!(Component with_types_in super::generated_bindings);
