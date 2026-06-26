use std::sync::Arc;
use std::sync::OnceLock;

use fast2flow_contracts::{
    Fast2FlowHookInV1, Fast2FlowHookOutV1, FlowExecutionType, MessageEnvelope, MessagingEndpointId,
    RoutingDirective, validate_scope,
};
use fast2flow_core::CoreRouter;
use futures::executor::block_on;

use super::generated_bindings::exports::greentic::fast2flow::routing_hook::Guest;
use super::generated_bindings::greentic::fast2flow::routing_types::{
    DispatchPayload as WitDispatchPayload, Fast2flowHookInV1 as WitIn,
    Fast2flowHookOutV1 as WitOut, FlowExecutionType as WitFlowExecutionType,
    MessageEnvelope as WitEnvelope, RoutingDirective as WitDirective,
};
use super::handle_hook_from_mounts;

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
        // F3 fix: delegate to the shared async path so scope canonicalization
        // + mount lookup match the host runtime exactly. The Guest trait is
        // sync, so we hop through `block_on`.
        block_on(handle_hook_from_mounts(&self.router, request))
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
        // M1.3: validate at the trust boundary. If the endpoint_id or scope
        // is malformed, fail closed — return Continue so no routing occurs.
        let endpoint_id = match request.messaging_endpoint_id.as_deref() {
            Some(raw) => match MessagingEndpointId::try_from(raw) {
                Ok(id) => Some(id),
                Err(_) => {
                    return WitOut {
                        directive: WitDirective::Continue,
                    };
                }
            },
            None => None,
        };

        // Validate scope when no endpoint_id overrides it.
        if endpoint_id.is_none() && !request.scope.is_empty() {
            if validate_scope(&request.scope).is_err() {
                return WitOut {
                    directive: WitDirective::Continue,
                };
            }
        }

        let request = map_in_validated(request, endpoint_id);
        let output = ROUTING_RUNTIME
            .get()
            .map(|runtime| runtime.route(request))
            .unwrap_or(Fast2FlowHookOutV1 {
                directive: RoutingDirective::Continue,
            });
        map_out(output)
    }
}

fn map_in_validated(request: WitIn, endpoint_id: Option<MessagingEndpointId>) -> Fast2FlowHookInV1 {
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
        messaging_endpoint_id: endpoint_id,
    }
}

fn map_out(output: Fast2FlowHookOutV1) -> WitOut {
    let directive = match output.directive {
        RoutingDirective::Continue => WitDirective::Continue,
        RoutingDirective::Dispatch {
            target,
            confidence,
            reason,
            // WIT world has no Vec<Entity> type yet; drop on the wasm path.
            // The native binary path carries entities through unchanged.
            entities: _,
            utterance,
            flow_type,
        } => WitDirective::Dispatch(WitDispatchPayload {
            target,
            confidence,
            reason,
            utterance,
            flow_type: map_flow_type(flow_type),
        }),
        RoutingDirective::Respond { message } => WitDirective::Respond(message),
        RoutingDirective::Deny { reason } => WitDirective::Deny(reason),
    };
    WitOut { directive }
}

fn map_flow_type(flow_type: FlowExecutionType) -> WitFlowExecutionType {
    match flow_type {
        FlowExecutionType::Deterministic => WitFlowExecutionType::Deterministic,
        FlowExecutionType::Agentic => WitFlowExecutionType::Agentic,
    }
}

#[allow(dead_code)]
fn _ensure_wit_types_used(_value: WitEnvelope) {}

super::generated_bindings::export!(Component with_types_in super::generated_bindings);
