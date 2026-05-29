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

/// Slimmer index entry. Collapses `titles: Vec<String>` (always
/// populated with a single element in v1) to a singular `title`, and
/// propagates `utterances` so example phrases reach the matcher
/// (dropped on the v1 path). All non-key fields skip-serialize when
/// empty so a minimal entry is just `flow_id` + `pack_id` + `target`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexEntryV2 {
    pub flow_id: String,
    pub pack_id: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub utterances: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub node_ids: Vec<String>,
}

impl IndexEntryV2 {
    /// Lift a v1 entry into v2: drops all but the first `titles` element,
    /// leaves `utterances` empty (v1 never carried them).
    pub fn from_v1(entry: IndexEntryV1) -> Self {
        Self {
            flow_id: entry.flow_id,
            pack_id: entry.pack_id,
            target: entry.target,
            title: entry.titles.into_iter().next().unwrap_or_default(),
            tags: entry.tags,
            utterances: Vec::new(),
            node_ids: entry.node_ids,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexManifestV2 {
    /// Always `"v2"` on write.
    pub version: String,
    pub scope: String,
    pub generated_at_ms: u64,
    pub entries: Vec<IndexEntryV2>,
}

impl IndexManifestV2 {
    /// Lift a v1 manifest into v2.
    pub fn from_v1(manifest: IndexManifestV1) -> Self {
        Self {
            version: "v2".to_string(),
            scope: manifest.scope,
            generated_at_ms: manifest.generated_at_ms,
            entries: manifest
                .entries
                .into_iter()
                .map(IndexEntryV2::from_v1)
                .collect(),
        }
    }
}

/// `IndexManifest` is the in-tree alias readers should consume going
/// forward. It now points at v2; the v1 type stays for migration paths.
pub type IndexManifest = IndexManifestV2;

#[cfg(test)]
mod index_v2_tests {
    use super::*;

    #[test]
    fn v1_entry_lifts_to_v2_keeping_first_title() {
        let v1 = IndexEntryV1 {
            flow_id: "pipeline".into(),
            node_ids: vec!["pipeline_card".into()],
            titles: vec!["View pipeline".into(), "ignored second".into()],
            tags: vec!["pipeline".into(), "deals".into()],
            pack_id: "demo".into(),
            target: "demo/pipeline".into(),
        };
        let v2 = IndexEntryV2::from_v1(v1);
        assert_eq!(v2.title, "View pipeline");
        assert_eq!(v2.utterances, Vec::<String>::new());
        assert_eq!(v2.tags, vec!["pipeline", "deals"]);
        assert_eq!(v2.node_ids, vec!["pipeline_card"]);
    }

    #[test]
    fn v1_manifest_lifts_to_v2_with_v2_version_tag() {
        let v1 = IndexManifestV1 {
            version: "v1".into(),
            scope: "demo:default".into(),
            generated_at_ms: 42,
            entries: vec![],
        };
        let v2 = IndexManifestV2::from_v1(v1);
        assert_eq!(v2.version, "v2");
        assert_eq!(v2.scope, "demo:default");
        assert_eq!(v2.generated_at_ms, 42);
    }

    #[test]
    fn v2_entry_skips_empty_optional_fields_when_serializing() {
        let entry = IndexEntryV2 {
            flow_id: "x".into(),
            pack_id: "p".into(),
            target: "p/x".into(),
            title: String::new(),
            tags: Vec::new(),
            utterances: Vec::new(),
            node_ids: Vec::new(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("\"title\""));
        assert!(!json.contains("\"tags\""));
        assert!(!json.contains("\"utterances\""));
        assert!(!json.contains("\"node_ids\""));
        assert!(json.contains("\"flow_id\":\"x\""));
        assert!(json.contains("\"pack_id\":\"p\""));
        assert!(json.contains("\"target\":\"p/x\""));
    }

    #[test]
    fn v2_entry_round_trips_with_optional_fields_populated() {
        let original = IndexEntryV2 {
            flow_id: "pipeline".into(),
            pack_id: "demo".into(),
            target: "demo/pipeline".into(),
            title: "View pipeline".into(),
            tags: vec!["pipeline".into()],
            utterances: vec!["show me the pipeline".into()],
            node_ids: vec!["pipeline_card".into()],
        };
        let json = serde_json::to_string(&original).unwrap();
        let decoded: IndexEntryV2 = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, original);
    }
}

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
