//! Routing logic for fast2flow.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use fast2flow_contracts::{
    Candidate, Fast2FlowHookInV1, MessageEnvelope, RoutingDirective as Fast2FlowRoutingDirective,
};
use fast2flow_core::{CandidateIndex, CoreRouter, RouterConfig as CoreRouterConfig};
use fast2flow_hooks::DefaultHookFilter;
use fast2flow_indexer::{IndexStore, load_latest};
use fast2flow_strategy_phase1::Phase1DeterministicStrategy;
use futures::executor::block_on;
use greentic_types::cbor::canonical;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Default confidence threshold for routing decisions.
const DEFAULT_CONFIDENCE_THRESHOLD: f64 = 0.7;

/// Default ambiguity threshold (if second candidate is within this ratio, it's ambiguous).
const DEFAULT_AMBIGUITY_THRESHOLD: f64 = 0.9;

/// Input message structure.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MessageInput {
    pub id: String,
    #[serde(default)]
    pub text: Option<String>,
    pub channel: String,
    pub session_id: String,
}

/// Flow reference from matcher.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlowRef {
    pub pack_id: String,
    pub flow_id: String,
    pub title: String,
    pub confidence: f64,
}

/// Match result from the matcher component.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchResult {
    pub status: String, // "match", "ambiguous", "no_match", "timeout"
    #[serde(default)]
    pub top_match: Option<FlowRef>,
    #[serde(default)]
    pub candidates: Vec<FlowRef>,
    #[serde(default)]
    pub latency_ms: u64,
}

/// Router configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RouterConfig {
    #[serde(default)]
    pub confidence_threshold: Option<f64>,
    #[serde(default)]
    pub ambiguity_threshold: Option<f64>,
    #[serde(default)]
    pub enable_llm_fallback: Option<bool>,
    #[serde(default)]
    pub llm_prompt_template: Option<String>,
    #[serde(default)]
    pub blocked_intents: Option<Vec<String>>,
}

/// Input for route operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RouteInput {
    pub message: MessageInput,
    pub match_result: MatchResult,
    pub tenant_id: String,
    #[serde(default)]
    pub team_id: Option<String>,
    #[serde(default)]
    pub config: RouterConfig,
}

/// Dispatch target for routing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DispatchTarget {
    pub tenant: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    pub pack: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
}

/// Control directive output.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ControlDirective {
    pub action: String, // "continue", "dispatch", "respond", "deny"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<DispatchTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_card: Option<JsonValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fast2FlowMessageEnvelope {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fast2FlowRouteRequest {
    pub scope: String,
    pub envelope: Fast2FlowMessageEnvelope,
    pub session_active: bool,
    pub input_locale: String,
    pub time_budget_ms: u64,
    pub registry_path: String,
    pub indexes_path: String,
    pub now_unix_ms: u64,
    #[serde(default)]
    pub metadata: BTreeMap<String, JsonValue>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fast2FlowRouteResult {
    pub directive: GreenticXFast2FlowDirective,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, JsonValue>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GreenticXFast2FlowDirective {
    Continue,
    Dispatch {
        target: String,
        confidence: f32,
        reason: String,
    },
    Respond {
        message: String,
    },
    Deny {
        reason: String,
    },
}

#[derive(Debug, Clone)]
struct MountedIndexLookup {
    scope: String,
    store: IndexStore,
}

impl MountedIndexLookup {
    fn load(indexes_path: &str, scope: &str) -> Result<Self, String> {
        let store = load_latest(Path::new(indexes_path), scope)
            .map_err(|err| format!("failed to load Fast2Flow index: {err}"))?;
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

/// Execute routing decision.
pub fn route_message(input: Vec<u8>) -> Vec<u8> {
    let result = do_route_message(&input);
    canonical::to_canonical_cbor_allow_floats(&result).unwrap_or_default()
}

/// Route a Greentic-X Fast2Flow request directly from mounted indexes.
pub fn route_intent(input: Vec<u8>) -> Vec<u8> {
    let result = do_route_intent(&input);
    canonical::to_canonical_cbor_allow_floats(&result).unwrap_or_default()
}

fn do_route_message(input: &[u8]) -> JsonValue {
    let input_value: JsonValue = match canonical::from_cbor(input) {
        Ok(v) => v,
        Err(e) => {
            return serde_json::json!({
                "error": format!("failed to parse input: {}", e)
            });
        }
    };

    let route_input: RouteInput = match serde_json::from_value(input_value) {
        Ok(v) => v,
        Err(e) => {
            return serde_json::json!({
                "error": format!("invalid input structure: {}", e)
            });
        }
    };

    let config = &route_input.config;
    let _confidence_threshold = config
        .confidence_threshold
        .unwrap_or(DEFAULT_CONFIDENCE_THRESHOLD);
    let _ambiguity_threshold = config
        .ambiguity_threshold
        .unwrap_or(DEFAULT_AMBIGUITY_THRESHOLD);

    let match_result = &route_input.match_result;
    let directive = match match_result.status.as_str() {
        "match" => {
            // High confidence single match
            if let Some(ref top_match) = match_result.top_match {
                // Check if intent is blocked
                if is_blocked_intent(top_match, config.blocked_intents.as_ref()) {
                    create_deny_directive("blocked_intent", "This action is not allowed.")
                } else {
                    create_dispatch_directive(
                        &route_input.tenant_id,
                        route_input.team_id.as_deref(),
                        top_match,
                    )
                }
            } else {
                create_continue_directive()
            }
        }
        "ambiguous" => {
            // Multiple candidates with similar confidence
            let candidates = &match_result.candidates;
            if candidates.len() >= 2 {
                // Check if any is blocked
                let allowed_candidates: Vec<_> = candidates
                    .iter()
                    .filter(|c| !is_blocked_intent(c, config.blocked_intents.as_ref()))
                    .collect();

                if allowed_candidates.is_empty() {
                    create_deny_directive("all_blocked", "All matching intents are blocked.")
                } else if allowed_candidates.len() == 1 {
                    // Only one allowed candidate, dispatch to it
                    create_dispatch_directive(
                        &route_input.tenant_id,
                        route_input.team_id.as_deref(),
                        allowed_candidates[0],
                    )
                } else {
                    // Ask for clarification
                    create_clarification_directive(&allowed_candidates)
                }
            } else {
                create_continue_directive()
            }
        }
        "no_match" => {
            // No confident match found
            // Could trigger LLM fallback here if enabled
            create_continue_directive()
        }
        "timeout" => {
            // Matcher timed out, continue without routing
            create_continue_directive()
        }
        _ => create_continue_directive(),
    };

    serde_json::to_value(directive).unwrap_or_else(|_| serde_json::json!({}))
}

fn do_route_intent(input: &[u8]) -> Fast2FlowRouteResult {
    let input_value: JsonValue = match canonical::from_cbor(input) {
        Ok(v) => v,
        Err(err) => return continue_result("invalid_request", err.to_string()),
    };
    let request: Fast2FlowRouteRequest = match serde_json::from_value(input_value) {
        Ok(v) => v,
        Err(err) => return continue_result("invalid_request", err.to_string()),
    };

    if request.time_budget_ms == 0 {
        return continue_result("zero_time_budget", "time_budget_ms is 0");
    }
    if request.envelope.text.trim().is_empty() {
        return continue_result("empty_text", "message text is empty");
    }

    let indexes_path = if request.indexes_path.trim().is_empty() {
        "/mnt/indexes"
    } else {
        request.indexes_path.as_str()
    };
    let index = match MountedIndexLookup::load(indexes_path, &request.scope) {
        Ok(index) => index,
        Err(err) => return continue_result("index_unavailable", err),
    };

    let router = CoreRouter::new(
        Arc::new(Phase1DeterministicStrategy),
        vec![Arc::new(DefaultHookFilter::default())],
        None,
        CoreRouterConfig::default(),
    );
    let output = block_on(router.route(
        Fast2FlowHookInV1 {
            scope: request.scope,
            envelope: MessageEnvelope {
                text: request.envelope.text,
                channel: request.envelope.channel,
                provider: request.envelope.provider,
            },
            session_active: request.session_active,
            input_locale: request.input_locale,
            time_budget_ms: request.time_budget_ms,
            registry_path: request.registry_path,
            indexes_path: request.indexes_path,
            now_unix_ms: request.now_unix_ms,
            messaging_endpoint_id: None,
        },
        &index,
    ));

    map_hook_output(output.directive)
}

fn map_hook_output(directive: Fast2FlowRoutingDirective) -> Fast2FlowRouteResult {
    match directive {
        Fast2FlowRoutingDirective::Continue => continue_result("no_match", "no route matched"),
        Fast2FlowRoutingDirective::Dispatch {
            target,
            confidence,
            reason,
            ..
        } => Fast2FlowRouteResult {
            directive: GreenticXFast2FlowDirective::Dispatch {
                target,
                confidence,
                reason,
            },
            metadata: BTreeMap::new(),
        },
        Fast2FlowRoutingDirective::Respond { message } => Fast2FlowRouteResult {
            directive: GreenticXFast2FlowDirective::Respond { message },
            metadata: BTreeMap::new(),
        },
        Fast2FlowRoutingDirective::Deny { reason } => Fast2FlowRouteResult {
            directive: GreenticXFast2FlowDirective::Deny { reason },
            metadata: BTreeMap::new(),
        },
    }
}

fn continue_result(reason: impl Into<String>, detail: impl Into<String>) -> Fast2FlowRouteResult {
    Fast2FlowRouteResult {
        directive: GreenticXFast2FlowDirective::Continue,
        metadata: BTreeMap::from([
            ("reason".to_string(), JsonValue::String(reason.into())),
            ("detail".to_string(), JsonValue::String(detail.into())),
        ]),
    }
}

/// Check if an intent is in the blocked list.
fn is_blocked_intent(flow: &FlowRef, blocked: Option<&Vec<String>>) -> bool {
    if let Some(blocked_list) = blocked {
        let flow_key = format!("{}:{}", flow.pack_id, flow.flow_id);
        blocked_list
            .iter()
            .any(|b| b == &flow.pack_id || b == &flow.flow_id || b == &flow_key)
    } else {
        false
    }
}

/// Create a continue directive (pass through).
fn create_continue_directive() -> ControlDirective {
    ControlDirective {
        action: "continue".to_string(),
        target: None,
        response_text: None,
        response_card: None,
        reason_code: None,
        status_code: None,
    }
}

/// Create a dispatch directive to route to a specific flow.
fn create_dispatch_directive(
    tenant_id: &str,
    team_id: Option<&str>,
    flow: &FlowRef,
) -> ControlDirective {
    ControlDirective {
        action: "dispatch".to_string(),
        target: Some(DispatchTarget {
            tenant: tenant_id.to_string(),
            team: team_id.map(|s| s.to_string()),
            pack: flow.pack_id.clone(),
            flow: Some(flow.flow_id.clone()),
            node: None,
        }),
        response_text: None,
        response_card: None,
        reason_code: None,
        status_code: None,
    }
}

/// Create a respond directive to ask for clarification.
fn create_clarification_directive(candidates: &[&FlowRef]) -> ControlDirective {
    let options: Vec<String> = candidates
        .iter()
        .take(4)
        .map(|c| format!("- {}", c.title))
        .collect();

    let response_text = format!(
        "I'm not sure which action you want. Did you mean one of these?\n{}",
        options.join("\n")
    );

    ControlDirective {
        action: "respond".to_string(),
        target: None,
        response_text: Some(response_text),
        response_card: None,
        reason_code: Some("clarification_needed".to_string()),
        status_code: Some(200),
    }
}

/// Create a deny directive for blocked intents.
fn create_deny_directive(reason_code: &str, message: &str) -> ControlDirective {
    ControlDirective {
        action: "deny".to_string(),
        target: None,
        response_text: Some(message.to_string()),
        response_card: None,
        reason_code: Some(reason_code.to_string()),
        status_code: Some(403),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use fast2flow_contracts::FlowDoc;
    use fast2flow_indexer::build_index;

    use super::*;

    #[test]
    fn test_create_dispatch_directive() {
        let flow = FlowRef {
            pack_id: "test-pack".to_string(),
            flow_id: "test-flow".to_string(),
            title: "Test Flow".to_string(),
            confidence: 0.9,
        };

        let directive = create_dispatch_directive("tenant1", Some("team1"), &flow);
        assert_eq!(directive.action, "dispatch");
        assert!(directive.target.is_some());

        let target = directive.target.unwrap();
        assert_eq!(target.tenant, "tenant1");
        assert_eq!(target.pack, "test-pack");
        assert_eq!(target.flow, Some("test-flow".to_string()));
    }

    #[test]
    fn test_is_blocked_intent() {
        let flow = FlowRef {
            pack_id: "admin".to_string(),
            flow_id: "delete_all".to_string(),
            title: "Delete All Data".to_string(),
            confidence: 0.95,
        };

        let blocked = vec!["admin:delete_all".to_string()];
        assert!(is_blocked_intent(&flow, Some(&blocked)));

        let blocked = vec!["admin".to_string()];
        assert!(is_blocked_intent(&flow, Some(&blocked)));

        let blocked = vec!["other".to_string()];
        assert!(!is_blocked_intent(&flow, Some(&blocked)));
    }

    #[test]
    fn route_intent_dispatches_from_mounted_index() {
        let scope = "tenant-e2e:default";
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
        let input = canonical::to_canonical_cbor_allow_floats(&serde_json::json!({
            "scope": scope,
            "envelope": {
                "text": "refund please",
                "channel": "chat",
                "provider": "tests"
            },
            "session_active": false,
            "input_locale": "en-US",
            "time_budget_ms": 250,
            "registry_path": "",
            "indexes_path": indexes_root.display().to_string(),
            "now_unix_ms": 0
        }))
        .unwrap();

        let output: Fast2FlowRouteResult = canonical::from_cbor(&route_intent(input)).unwrap();

        match output.directive {
            GreenticXFast2FlowDirective::Dispatch { target, .. } => {
                assert_eq!(target, "support/refund_flow");
            }
            other => panic!("expected dispatch, got {other:?}"),
        }
    }

    #[test]
    fn route_intent_returns_continue_for_missing_index() {
        let input = canonical::to_canonical_cbor_allow_floats(&serde_json::json!({
            "scope": "demo",
            "envelope": {
                "text": "show inbound traffic"
            },
            "session_active": false,
            "input_locale": "en",
            "time_budget_ms": 250,
            "registry_path": "",
            "indexes_path": "/path/that/does/not/exist",
            "now_unix_ms": 0
        }))
        .unwrap();

        let output: Fast2FlowRouteResult = canonical::from_cbor(&route_intent(input)).unwrap();

        assert!(matches!(
            output.directive,
            GreenticXFast2FlowDirective::Continue
        ));
        assert_eq!(
            output.metadata.get("reason").and_then(JsonValue::as_str),
            Some("index_unavailable")
        );
    }

    fn temp_indexes_dir() -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!("fast2flow-router-component-{suffix}"))
    }
}
