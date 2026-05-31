use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageEnvelope {
    pub text: String,
    pub channel: Option<String>,
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Fast2FlowHookInV1 {
    pub scope: String,
    pub envelope: MessageEnvelope,
    pub session_active: bool,
    pub input_locale: String,
    pub time_budget_ms: u64,
    pub registry_path: String,
    pub indexes_path: String,
    pub now_unix_ms: u64,
    /// Phase M1: per-endpoint scoping. When set, the routing layer derives the
    /// effective index scope as [`endpoint_scope`] of this id and ignores
    /// `scope`. When absent, `scope` is used verbatim (legacy `tenant:team`
    /// callers stay working).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub messaging_endpoint_id: Option<String>,
}

impl Fast2FlowHookInV1 {
    /// Phase M1: per-endpoint scope key.
    ///
    /// Returns `endpoint:{messaging_endpoint_id}` when the new field is set,
    /// otherwise borrows `scope`. The scope string is consumed by
    /// `MountedIndexLookup` (for index file resolution + match guard) and by
    /// `RoutingPolicyV1::scope_overrides`; both treat it as an opaque key.
    ///
    /// `Cow` keeps the legacy `tenant:team` path allocation-free — the hot
    /// `canonicalize_scope` call on every request only takes ownership when
    /// it actually has to (the `messaging_endpoint_id` arm).
    pub fn effective_scope(&self) -> Cow<'_, str> {
        match self.messaging_endpoint_id.as_deref() {
            Some(id) => Cow::Owned(endpoint_scope(id)),
            None => Cow::Borrowed(&self.scope),
        }
    }
}

/// Phase M1: format a messaging-endpoint scope key.
///
/// Indexer producers + routing consumers share this helper so the
/// `endpoint:` prefix never drifts between sides.
pub fn endpoint_scope(endpoint_id: &str) -> String {
    format!("endpoint:{endpoint_id}")
}

pub type HookInV1 = Fast2FlowHookInV1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Fast2FlowHookOutV1 {
    pub directive: RoutingDirective,
}

pub type HookOutV1 = Fast2FlowHookOutV1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoutingDirective {
    Continue,
    Dispatch {
        target: String,
        confidence: f32,
        reason: String,
        /// Original user text echoed for downstream slot extraction.
        /// `None` on legacy producers — consumers must tolerate absence.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        utterance: Option<String>,
    },
    Respond {
        message: String,
    },
    Deny {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlowDoc {
    pub id: String,
    pub pack_id: String,
    pub target: String,
    pub title: String,
    pub tags: Vec<String>,
    pub node_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexEntryV1 {
    pub flow_id: String,
    pub node_ids: Vec<String>,
    pub titles: Vec<String>,
    pub tags: Vec<String>,
    pub pack_id: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexManifestV1 {
    pub version: String,
    pub scope: String,
    pub generated_at_ms: u64,
    pub entries: Vec<IndexEntryV1>,
}

pub type IndexManifest = IndexManifestV1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Candidate {
    pub target: String,
    pub flow_id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub score_hint: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Decision {
    pub target: String,
    pub confidence: f32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RespondRuleV1 {
    pub needle: String,
    pub message: String,
    #[serde(default)]
    pub mode: TextMatchModeV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextMatchModeV1 {
    Exact,
    Regex,
    #[default]
    Contains,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PolicyRuleV1 {
    pub min_confidence: Option<f32>,
    pub llm_min_confidence: Option<f32>,
    pub candidate_limit: Option<usize>,
    pub allow_channels: Option<Vec<String>>,
    pub deny_channels: Option<Vec<String>>,
    pub allow_providers: Option<Vec<String>>,
    pub deny_providers: Option<Vec<String>>,
    pub allow_scopes: Option<Vec<String>>,
    pub deny_scopes: Option<Vec<String>>,
    pub respond_rules: Option<Vec<RespondRuleV1>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScopePolicyOverrideV1 {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub priority: i32,
    pub scope: String,
    pub rules: PolicyRuleV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChannelPolicyOverrideV1 {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub priority: i32,
    pub channel: String,
    pub rules: PolicyRuleV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderPolicyOverrideV1 {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub priority: i32,
    pub provider: String,
    pub rules: PolicyRuleV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PolicyStageV1 {
    Scope,
    Channel,
    Provider,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingPolicyV1 {
    #[serde(default = "default_policy_stage_order")]
    pub stage_order: Vec<PolicyStageV1>,
    pub default: PolicyRuleV1,
    pub scope_overrides: Vec<ScopePolicyOverrideV1>,
    pub channel_overrides: Vec<ChannelPolicyOverrideV1>,
    pub provider_overrides: Vec<ProviderPolicyOverrideV1>,
}

impl Default for RoutingPolicyV1 {
    fn default() -> Self {
        Self {
            stage_order: default_policy_stage_order(),
            default: PolicyRuleV1::default(),
            scope_overrides: Vec::new(),
            channel_overrides: Vec::new(),
            provider_overrides: Vec::new(),
        }
    }
}

fn default_policy_stage_order() -> Vec<PolicyStageV1> {
    vec![
        PolicyStageV1::Scope,
        PolicyStageV1::Channel,
        PolicyStageV1::Provider,
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicyAppliedRuleV1 {
    pub source: String,
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicyEffectiveConfigV1 {
    pub min_confidence: f32,
    pub llm_min_confidence: f32,
    pub candidate_limit: usize,
    pub allow_channels: Option<Vec<String>>,
    pub deny_channels: Vec<String>,
    pub allow_providers: Option<Vec<String>>,
    pub deny_providers: Vec<String>,
    pub allow_scopes: Option<Vec<String>>,
    pub deny_scopes: Vec<String>,
    pub respond_rule_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicyResolutionV1 {
    pub applied: Vec<PolicyAppliedRuleV1>,
    pub warnings: Vec<String>,
    pub effective: PolicyEffectiveConfigV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingExecutionTraceV1 {
    pub policy: Option<PolicyResolutionV1>,
    pub directive: RoutingDirective,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_request(scope: &str) -> Fast2FlowHookInV1 {
        Fast2FlowHookInV1 {
            scope: scope.to_string(),
            envelope: MessageEnvelope {
                text: "hi".to_string(),
                channel: None,
                provider: None,
            },
            session_active: false,
            input_locale: "en-US".to_string(),
            time_budget_ms: 250,
            registry_path: String::new(),
            indexes_path: String::new(),
            now_unix_ms: 0,
            messaging_endpoint_id: None,
        }
    }

    #[test]
    fn effective_scope_falls_back_to_scope_field_when_endpoint_absent() {
        let req = base_request("acme:legal");
        assert_eq!(req.effective_scope(), "acme:legal");
    }

    #[test]
    fn effective_scope_uses_endpoint_prefix_when_set() {
        let mut req = base_request("acme:legal");
        req.messaging_endpoint_id = Some("teams-legal-bot".to_string());
        assert_eq!(req.effective_scope(), "endpoint:teams-legal-bot");
    }

    #[test]
    fn endpoint_scope_helper_is_stable_prefix() {
        assert_eq!(endpoint_scope("teams-x"), "endpoint:teams-x");
        assert_eq!(endpoint_scope(""), "endpoint:");
    }

    #[test]
    fn legacy_hook_json_without_messaging_endpoint_id_deserializes() {
        let payload = r#"{
            "scope": "acme:legal",
            "envelope": {"text": "hi", "channel": null, "provider": null},
            "session_active": false,
            "input_locale": "en-US",
            "time_budget_ms": 250,
            "registry_path": "",
            "indexes_path": "",
            "now_unix_ms": 0
        }"#;
        let req: Fast2FlowHookInV1 = serde_json::from_str(payload).expect("legacy parse");
        assert!(req.messaging_endpoint_id.is_none());
        assert_eq!(req.effective_scope(), "acme:legal");
    }

    #[test]
    fn dispatch_directive_round_trips_utterance() {
        let directive = RoutingDirective::Dispatch {
            target: "legal/nda_flow".to_string(),
            confidence: 0.87,
            reason: "deterministic".to_string(),
            utterance: Some("NDA between Acme and us by Friday".to_string()),
        };
        let json = serde_json::to_string(&directive).expect("serialize");
        let parsed: RoutingDirective = serde_json::from_str(&json).expect("round-trip");
        assert_eq!(parsed, directive);
    }

    #[test]
    fn dispatch_directive_omits_utterance_when_none() {
        // skip_serializing_if keeps the wire format backwards-compatible
        // for producers that never populate utterance.
        let directive = RoutingDirective::Dispatch {
            target: "support/refund_flow".to_string(),
            confidence: 0.91,
            reason: "deterministic".to_string(),
            utterance: None,
        };
        let json = serde_json::to_string(&directive).expect("serialize");
        assert!(
            !json.contains("utterance"),
            "utterance key must be skipped when None: {json}"
        );
    }

    #[test]
    fn legacy_dispatch_json_without_utterance_deserializes() {
        // Historical wire format (M1 and earlier) carried only target /
        // confidence / reason. M2.2 consumers must still parse it.
        let payload = r#"{
            "type": "dispatch",
            "target": "support/refund_flow",
            "confidence": 0.91,
            "reason": "deterministic"
        }"#;
        let directive: RoutingDirective = serde_json::from_str(payload).expect("legacy parse");
        match directive {
            RoutingDirective::Dispatch {
                utterance, target, ..
            } => {
                assert!(utterance.is_none(), "default to None on legacy payload");
                assert_eq!(target, "support/refund_flow");
            }
            other => panic!("expected dispatch, got {other:?}"),
        }
    }
}
