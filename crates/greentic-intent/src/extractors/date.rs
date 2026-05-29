//! Date extractor (deterministic). Phase A stub.

use crate::context::IntentContext;
use crate::extractors::{EntityCandidate, EntityExtractor};
use crate::locale::ResolvedLocale;
use crate::resources::IntentResources;
use crate::token::Token;

/// Deterministic, locale-aware date extractor. Phase A: stub that always
/// returns no candidates. Next milestone implements relative-day words
/// (today/tomorrow/yesterday/mañana/demain/…) and weekday resolution.
#[derive(Debug, Default, Clone, Copy)]
pub struct DateExtractor;

impl EntityExtractor for DateExtractor {
    fn name(&self) -> &'static str {
        "date"
    }

    fn extract(
        &self,
        _tokens: &[Token],
        _text: &str,
        _ctx: &IntentContext,
        _locale: &ResolvedLocale,
        _resources: &IntentResources,
    ) -> Vec<EntityCandidate> {
        Vec::new()
    }
}
