//! Resolved language information emitted alongside each [`MarkResult`].

use serde::{Deserialize, Serialize};

/// How the engine arrived at the resolved language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageSource {
    /// Came from `IntentContext::preferred_locale`.
    Context,
    /// Came from `IntentContext::user_locale`.
    User,
    /// Came from `IntentContext::tenant_locale`.
    Tenant,
    /// Detected by the lightweight language detector.
    Detected,
    /// Fallback when nothing else applied (typically en-GB).
    Fallback,
}

/// Language metadata for a single extraction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolvedLanguage {
    /// BCP-47 language subtag (e.g. `en`, `fr`).
    pub language: String,
    /// Full BCP-47 locale (e.g. `en-GB`).
    pub locale: String,
    /// Script subtag (e.g. `Latin`).
    pub script: String,
    /// Provenance.
    pub source: LanguageSource,
    /// Detector confidence — 1.0 if explicitly supplied.
    pub confidence: f32,
}

impl ResolvedLanguage {
    /// Fallback `en-GB / Latin` resolution. Used until the language module
    /// implements detection + locale chain resolution.
    pub fn fallback_en_gb() -> Self {
        Self {
            language: "en".into(),
            locale: "en-GB".into(),
            script: "Latin".into(),
            source: LanguageSource::Fallback,
            confidence: 0.0,
        }
    }
}
