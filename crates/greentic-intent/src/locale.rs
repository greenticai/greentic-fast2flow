//! Locale resolution — exposes the active locale to extractors.

use serde::{Deserialize, Serialize};

/// Resolved locale handed to each [`crate::EntityExtractor`]. Carries
/// pointers into preloaded resources (date words, prepositions, …) once
/// the builtin-locales feature fills them in.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolvedLocale {
    /// BCP-47 locale tag, e.g. `en-GB`.
    pub locale: String,
    /// Language subtag, e.g. `en`.
    pub language: String,
    /// Script subtag, e.g. `Latin`.
    pub script: String,
}

impl ResolvedLocale {
    /// Fallback locale (`en-GB / Latin`).
    pub fn fallback_en_gb() -> Self {
        Self {
            locale: "en-GB".into(),
            language: "en".into(),
            script: "Latin".into(),
        }
    }
}

/// A locale resource bundle (date words, prepositions, modifiers, …).
/// Populated by `builtin-locales` feature or by caller-supplied resources.
/// Empty by default — extractors must handle an empty bundle gracefully.
#[derive(Clone, Debug, Default)]
pub struct LocaleBundle {
    /// Tag this bundle applies to.
    pub locale: String,
    /// `today=0`, `tomorrow=1`, `yesterday=-1`, …
    pub relative_days: std::collections::HashMap<String, i32>,
    /// `monday=1` … `sunday=7`. Case-folded keys.
    pub weekdays: std::collections::HashMap<String, u8>,
    /// `next=Next`, `prochain=Next`, `pasado=Last`, …
    pub modifiers: std::collections::HashMap<String, RelativeModifier>,
    /// `in=In`, `à=In`, `from=From`, `to=To`, …
    pub location_prepositions: std::collections::HashMap<String, LocationRole>,
}

/// Locale-neutral modifier classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelativeModifier {
    /// `next`, `prochain`, `próximo`, `volgende`
    Next,
    /// `this`, `ce`, `esta`, `deze`
    This,
    /// `last`, `dernier`, `pasado`, `vorige`
    Last,
    /// `in`, `dans`, `en`, `over`
    In,
    /// `ago`, `il y a`, `hace`, `geleden`
    Ago,
}

/// Locale-neutral preposition role for locations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocationRole {
    /// `in`, `at`, `à`, `en`
    In,
    /// `from`, `de`, `depuis`, `van`
    From,
    /// `to`, `vers`, `a`, `naar`
    To,
    /// `near`, `bij`
    Near,
    /// `around`
    Around,
}
