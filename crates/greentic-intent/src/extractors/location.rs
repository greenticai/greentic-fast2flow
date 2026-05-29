//! Location extractor (gazetteer + longest-match). Phase A stub.

use crate::context::IntentContext;
use crate::extractors::{EntityCandidate, EntityExtractor};
use crate::locale::ResolvedLocale;
use crate::resources::IntentResources;
use crate::token::Token;

/// Gazetteer-driven location extractor. Phase A: stub returning no
/// candidates. Next milestone implements the longest-match token trie
/// + role detection from locale prepositions.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocationExtractor;

impl EntityExtractor for LocationExtractor {
    fn name(&self) -> &'static str {
        "location"
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
