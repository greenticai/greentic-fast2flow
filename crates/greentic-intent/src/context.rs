//! Per-call extraction context (reference time, locale chain, language allow-list).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Caller-supplied context for a single `mark()` invocation. Date relatives
/// (`tomorrow`, `mañana`) resolve against `reference_time` + `timezone`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntentContext {
    /// Wall-clock anchor for relative date/time resolution.
    pub reference_time: DateTime<Utc>,
    /// IANA timezone identifier, e.g. `Europe/London`.
    pub timezone: String,
    /// Explicit locale supplied at the call site. Highest priority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_locale: Option<String>,
    /// Tenant default locale (per Greentic env). Lower priority than `preferred`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_locale: Option<String>,
    /// End-user profile locale. Lower priority than `tenant`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_locale: Option<String>,
    /// BCP-47 language tags the engine is allowed to detect.
    #[serde(default)]
    pub allowed_languages: Vec<String>,
}

impl IntentContext {
    /// Convenience constructor for tests + examples.
    pub fn now_utc(timezone: impl Into<String>) -> Self {
        Self {
            reference_time: Utc::now(),
            timezone: timezone.into(),
            preferred_locale: None,
            tenant_locale: None,
            user_locale: None,
            allowed_languages: Vec::new(),
        }
    }
}
