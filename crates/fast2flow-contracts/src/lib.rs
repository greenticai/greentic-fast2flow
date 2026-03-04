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
