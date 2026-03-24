use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use fast2flow_contracts::{
    Fast2FlowHookInV1, PolicyAppliedRuleV1, PolicyEffectiveConfigV1, PolicyResolutionV1,
    PolicyRuleV1, PolicyStageV1, RoutingPolicyV1, TextMatchModeV1,
};
use fast2flow_core::RouterConfig;
use fast2flow_hooks::{DefaultHookFilter, RespondRule};

use crate::config::env_var;
use crate::{DEFAULT_POLICY_PATH, ENV_POLICY_PATH};

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

pub(crate) fn validate_policy(policy: &RoutingPolicyV1) -> Result<()> {
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

/// Resolves policy overrides for a request and returns the effective filter,
/// config, and an optional policy resolution trace.
pub(crate) fn resolve_policy(
    policy: &RoutingPolicyV1,
    request: &Fast2FlowHookInV1,
    base_filter: &DefaultHookFilter,
    base_config: &RouterConfig,
) -> (DefaultHookFilter, RouterConfig, PolicyResolutionV1) {
    let mut filter = base_filter.clone();
    let mut config = base_config.clone();
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
            PolicyStageV1::Provider => {
                apply_provider_overrides(policy, request, &mut filter, &mut config, &mut tracker)
            }
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

    (filter, config, trace)
}

fn apply_policy_rule(
    filter: &mut DefaultHookFilter,
    config: &mut RouterConfig,
    rule: &PolicyRuleV1,
    source: &str,
    tracker: &mut PolicyTracker,
) {
    if let Some(value) = rule.min_confidence {
        track_set(tracker, source, "min_confidence", format!("{:.4}", value));
        config.min_confidence = value;
    }
    if let Some(value) = rule.llm_min_confidence {
        track_set(
            tracker,
            source,
            "llm_min_confidence",
            format!("{:.4}", value),
        );
        config.llm_min_confidence = value;
    }
    if let Some(value) = rule.candidate_limit {
        track_set(tracker, source, "candidate_limit", value.to_string());
        config.candidate_limit = value;
    }
    if let Some(value) = &rule.allow_channels {
        track_set(
            tracker,
            source,
            "allow_channels",
            serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string()),
        );
        filter.allow_channels = Some(value.clone());
    }
    if let Some(value) = &rule.deny_channels {
        track_set(
            tracker,
            source,
            "deny_channels",
            serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string()),
        );
        filter.deny_channels = value.clone();
    }
    if let Some(value) = &rule.allow_providers {
        track_set(
            tracker,
            source,
            "allow_providers",
            serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string()),
        );
        filter.allow_providers = Some(value.clone());
    }
    if let Some(value) = &rule.deny_providers {
        track_set(
            tracker,
            source,
            "deny_providers",
            serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string()),
        );
        filter.deny_providers = value.clone();
    }
    if let Some(value) = &rule.allow_scopes {
        track_set(
            tracker,
            source,
            "allow_scopes",
            serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string()),
        );
        filter.allow_scopes = Some(value.clone());
    }
    if let Some(value) = &rule.deny_scopes {
        track_set(
            tracker,
            source,
            "deny_scopes",
            serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string()),
        );
        filter.deny_scopes = value.clone();
    }
    if let Some(value) = &rule.respond_rules {
        track_set(
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

fn track_set(tracker: &mut PolicyTracker, source: &str, field: &str, value: String) {
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
