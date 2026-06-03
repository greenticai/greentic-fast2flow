//! Per-kind entity extractors. Phase A defines the trait + a stub module
//! per planned extractor; the actual extraction logic lands incrementally.

pub mod date;
pub mod location;
pub mod person;
pub mod time;

#[cfg(feature = "email")]
pub mod email;
#[cfg(feature = "phone")]
pub mod phone;
#[cfg(feature = "url")]
pub mod url;

use crate::context::IntentContext;
use crate::entity::{EntityEvidence, EntityKind};
use crate::locale::ResolvedLocale;
use crate::resources::IntentResources;
use crate::token::Token;

/// Pre-resolution candidate produced by an extractor. Identical shape to
/// [`crate::Entity`] but without the result-id field (which is assigned
/// after overlap resolution).
#[derive(Clone, Debug)]
pub struct EntityCandidate {
    /// Entity kind.
    pub kind: EntityKind,
    /// Raw surface text.
    pub raw: String,
    /// Normalised value.
    pub normalized: String,
    /// Canonical reference, when applicable.
    pub canonical: Option<String>,
    /// Inclusive byte offset of the span start.
    pub start: usize,
    /// Exclusive byte offset of the span end.
    pub end: usize,
    /// Role tag, when derivable from local context.
    pub role: Option<String>,
    /// Confidence in [0, 1].
    pub confidence: f32,
    /// Locale active when this candidate was extracted.
    pub locale: String,
    /// Evidence records.
    pub evidence: Vec<EntityEvidence>,
}

/// An entity extractor. Phase A: implementations are stubs returning
/// `Vec::new()`; logic fills in per-milestone.
pub trait EntityExtractor: Send + Sync {
    /// Stable extractor name.
    fn name(&self) -> &'static str;
    /// BCP-47 locales this extractor supports. Empty means locale-agnostic.
    fn supported_locales(&self) -> &[&'static str] {
        &[]
    }
    /// Extract candidates from `tokens` / `text` under `ctx`.
    fn extract(
        &self,
        tokens: &[Token],
        text: &str,
        ctx: &IntentContext,
        locale: &ResolvedLocale,
        resources: &IntentResources,
    ) -> Vec<EntityCandidate>;
}
