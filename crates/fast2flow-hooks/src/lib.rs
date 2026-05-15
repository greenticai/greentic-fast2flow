use fast2flow_contracts::{Fast2FlowHookInV1, TextMatchModeV1};
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterDecision {
    Proceed,
    Continue,
    Respond(String),
    Deny(String),
}

pub trait HookFilter: Send + Sync {
    fn evaluate(&self, request: &Fast2FlowHookInV1) -> FilterDecision;
}

#[derive(Debug, Clone, Default)]
pub struct DefaultHookFilter {
    pub allow_channels: Option<Vec<String>>,
    pub deny_channels: Vec<String>,
    pub allow_providers: Option<Vec<String>>,
    pub deny_providers: Vec<String>,
    pub allow_scopes: Option<Vec<String>>,
    pub deny_scopes: Vec<String>,
    pub respond_rules: Vec<RespondRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespondRule {
    pub needle: String,
    pub message: String,
    pub mode: TextMatchModeV1,
}

impl HookFilter for DefaultHookFilter {
    fn evaluate(&self, request: &Fast2FlowHookInV1) -> FilterDecision {
        if request.session_active {
            debug!(scope = %request.scope, "hook filter: session active → continue");
            return FilterDecision::Continue;
        }

        if self
            .deny_scopes
            .iter()
            .any(|scope| scope.eq_ignore_ascii_case(&request.scope))
        {
            debug!(scope = %request.scope, "hook filter: scope denied by policy");
            return FilterDecision::Deny("scope denied by policy".to_string());
        }

        let text = request.envelope.text.as_str();
        if let Some(rule) = self
            .respond_rules
            .iter()
            .find(|rule| matches_rule(rule, text))
        {
            debug!(needle = %rule.needle, mode = ?rule.mode, "hook filter: respond rule matched");
            return FilterDecision::Respond(rule.message.clone());
        }

        if let Some(allow_scopes) = &self.allow_scopes {
            let allowed = allow_scopes
                .iter()
                .any(|scope| scope.eq_ignore_ascii_case(&request.scope));
            if !allowed {
                debug!(scope = %request.scope, "hook filter: scope not in allow list → continue");
                return FilterDecision::Continue;
            }
        }

        if let Some(channel) = request.envelope.channel.as_ref() {
            if self
                .deny_channels
                .iter()
                .any(|item| item.eq_ignore_ascii_case(channel))
            {
                debug!(%channel, "hook filter: channel denied by policy");
                return FilterDecision::Deny("channel denied by policy".to_string());
            }
            if let Some(allow_channels) = &self.allow_channels {
                let allowed = allow_channels
                    .iter()
                    .any(|item| item.eq_ignore_ascii_case(channel));
                if !allowed {
                    debug!(%channel, "hook filter: channel not in allow list → continue");
                    return FilterDecision::Continue;
                }
            }
        }

        if let Some(provider) = request.envelope.provider.as_ref() {
            if self
                .deny_providers
                .iter()
                .any(|item| item.eq_ignore_ascii_case(provider))
            {
                debug!(%provider, "hook filter: provider denied by policy");
                return FilterDecision::Deny("provider denied by policy".to_string());
            }
            if let Some(allow_providers) = &self.allow_providers {
                let allowed = allow_providers
                    .iter()
                    .any(|item| item.eq_ignore_ascii_case(provider));
                if !allowed {
                    debug!(%provider, "hook filter: provider not in allow list → continue");
                    return FilterDecision::Continue;
                }
            }
        }

        FilterDecision::Proceed
    }
}

fn matches_rule(rule: &RespondRule, text: &str) -> bool {
    let needle = rule.needle.trim();
    if needle.is_empty() {
        return false;
    }
    match rule.mode {
        TextMatchModeV1::Contains => text
            .to_ascii_lowercase()
            .contains(&needle.to_ascii_lowercase()),
        TextMatchModeV1::Exact => text.eq_ignore_ascii_case(needle),
        TextMatchModeV1::Regex => regex::Regex::new(needle)
            .map(|pattern| pattern.is_match(text))
            .unwrap_or(false),
    }
}

pub fn evaluate_filters(
    filters: &[Box<dyn HookFilter>],
    request: &Fast2FlowHookInV1,
) -> FilterDecision {
    for filter in filters {
        match filter.evaluate(request) {
            FilterDecision::Proceed => continue,
            decision => return decision,
        }
    }
    FilterDecision::Proceed
}
