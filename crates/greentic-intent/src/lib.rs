//! Deterministic multilingual entity detection + marker rendering.
//!
//! See the crate README for design rules and the workspace root design
//! doc for the Phase A → E plan.

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

#[cfg(feature = "builtin-locales")]
pub mod builtin;
pub mod context;
pub mod engine;
pub mod entity;
pub mod extractors;
pub mod gazetteer;
pub mod language;
pub mod locale;
pub mod marker;
pub mod resolver;
pub mod resources;
pub mod token;
pub mod tokenizer;

pub use context::IntentContext;
pub use engine::{IntentEngine, IntentEngineBuilder};
pub use entity::{Entity, EntityEvidence, EntityKind};
pub use extractors::{EntityCandidate, EntityExtractor};
pub use language::ResolvedLanguage;
pub use locale::ResolvedLocale;
pub use marker::{IntentLatency, IntentWarning, MarkResult};
pub use token::{Token, TokenShape};

/// Crate-level error type.
#[derive(Debug, thiserror::Error)]
pub enum IntentError {
    /// A locale resource referenced by context was not found.
    #[error("locale not found: {0}")]
    LocaleNotFound(String),
    /// A required resource (gazetteer, dictionary) was missing.
    #[error("resource missing: {0}")]
    MissingResource(String),
    /// Internal extractor failure surfaced for diagnostics.
    #[error("extractor failed: {extractor}: {reason}")]
    Extractor {
        /// Extractor name.
        extractor: &'static str,
        /// Failure reason.
        reason: String,
    },
}

/// Convenience alias.
pub type IntentResult<T> = Result<T, IntentError>;
