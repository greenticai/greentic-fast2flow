//! Location extractor: longest-match gazetteer + preposition-driven role.
//!
//! For each word-shape token, asks the gazetteer for the longest entry
//! starting there. On a hit, walks backwards (skipping whitespace +
//! punctuation) to the preceding word token. If that word is a known
//! location preposition in the active locale bundle, the candidate is
//! tagged with the corresponding role (`in`, `from`, `to`, `near`,
//! `around`). Empty gazetteer → no candidates.

use crate::context::IntentContext;
use crate::entity::{EntityEvidence, EntityKind};
use crate::extractors::{EntityCandidate, EntityExtractor};
use crate::locale::{LocaleBundle, LocationRole, ResolvedLocale};
use crate::resources::IntentResources;
use crate::token::{Token, TokenShape};

/// Gazetteer-driven location extractor.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocationExtractor;

impl EntityExtractor for LocationExtractor {
    fn name(&self) -> &'static str {
        "location"
    }

    fn extract(
        &self,
        tokens: &[Token],
        text: &str,
        _ctx: &IntentContext,
        locale: &ResolvedLocale,
        resources: &IntentResources,
    ) -> Vec<EntityCandidate> {
        let gazetteer = &resources.gazetteer;
        if gazetteer.is_empty() {
            return Vec::new();
        }
        let bundle = resources.locales.get(&locale.locale);
        let word_idxs: Vec<usize> = tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| is_wordish(t))
            .map(|(i, _)| i)
            .collect();

        let mut candidates = Vec::new();
        let mut w = 0;
        while w < word_idxs.len() {
            let lowers: Vec<&str> = word_idxs[w..]
                .iter()
                .map(|&i| tokens[i].lower.as_str())
                .collect();
            if let Some((span_words, record)) = gazetteer.longest_match(&lowers) {
                let head_tok_idx = word_idxs[w];
                let tail_tok_idx = word_idxs[w + span_words - 1];
                let head = &tokens[head_tok_idx];
                let tail = &tokens[tail_tok_idx];
                let raw = text[head.start..tail.end].to_string();
                let role = preceding_role(tokens, head_tok_idx, bundle).map(role_name);
                candidates.push(EntityCandidate {
                    kind: EntityKind::Location,
                    raw,
                    normalized: record.canonical.clone(),
                    canonical: Some(record.canonical.clone()),
                    start: head.start,
                    end: tail.end,
                    role: role.map(str::to_string),
                    confidence: 0.9,
                    locale: locale.locale.clone(),
                    evidence: vec![EntityEvidence {
                        extractor: "location".into(),
                        rule: "gazetteer_longest_match".into(),
                        detail: None,
                    }],
                });
                w += span_words;
            } else {
                w += 1;
            }
        }
        candidates
    }
}

fn is_wordish(t: &Token) -> bool {
    matches!(
        t.shape,
        TokenShape::Word | TokenShape::Number | TokenShape::Mixed
    )
}

/// Walks back from `head_idx`, skipping whitespace + punctuation, and
/// classifies the first preceding word token via the locale bundle's
/// preposition table.
fn preceding_role(
    tokens: &[Token],
    head_idx: usize,
    bundle: Option<&LocaleBundle>,
) -> Option<LocationRole> {
    let bundle = bundle?;
    let mut i = head_idx;
    while i > 0 {
        i -= 1;
        let t = &tokens[i];
        match t.shape {
            TokenShape::Whitespace | TokenShape::Punctuation => continue,
            _ => return bundle.location_prepositions.get(&t.lower).copied(),
        }
    }
    None
}

fn role_name(role: LocationRole) -> &'static str {
    match role {
        LocationRole::In => "in",
        LocationRole::From => "from",
        LocationRole::To => "to",
        LocationRole::Near => "near",
        LocationRole::Around => "around",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gazetteer::{Gazetteer, GazetteerRecord};
    use crate::tokenizer::{Tokenizer, WhitespaceTokenizer};
    use chrono::{TimeZone, Utc};

    fn ctx() -> IntentContext {
        IntentContext {
            reference_time: Utc.with_ymd_and_hms(2026, 5, 27, 12, 0, 0).unwrap(),
            timezone: "Europe/London".into(),
            preferred_locale: Some("en-GB".into()),
            tenant_locale: None,
            user_locale: None,
            allowed_languages: Vec::new(),
        }
    }

    fn locale() -> ResolvedLocale {
        ResolvedLocale {
            locale: "en-GB".into(),
            language: "en".into(),
            script: "Latin".into(),
        }
    }

    fn resources_with_gazetteer() -> IntentResources {
        let mut g = Gazetteer::default();
        g.insert(
            "london",
            GazetteerRecord {
                canonical: "London".into(),
                country: Some("GB".into()),
                kind_hint: Some("city".into()),
                aliases: vec!["londres".into()],
            },
        );
        g.insert(
            "new york",
            GazetteerRecord {
                canonical: "New York".into(),
                country: Some("US".into()),
                kind_hint: Some("city".into()),
                aliases: vec!["nyc".into()],
            },
        );
        let mut r = IntentResources {
            gazetteer: g,
            locales: Default::default(),
        };
        #[cfg(feature = "builtin-locales")]
        {
            r.locales
                .insert("en-GB".into(), crate::builtin::en_gb_bundle());
        }
        #[cfg(not(feature = "builtin-locales"))]
        {
            r.locales
                .insert("en-GB".into(), crate::locale::LocaleBundle::default());
        }
        r
    }

    fn extract(text: &str) -> Vec<EntityCandidate> {
        let tokens = WhitespaceTokenizer.tokenize(text);
        LocationExtractor.extract(
            &tokens,
            text,
            &ctx(),
            &locale(),
            &resources_with_gazetteer(),
        )
    }

    #[test]
    fn single_word_city_resolves_to_canonical() {
        let cands = extract("hello London");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "London");
        assert_eq!(cands[0].canonical.as_deref(), Some("London"));
        assert_eq!(cands[0].raw, "London");
    }

    #[test]
    fn multi_word_city_matches_longest() {
        let cands = extract("weather in New York please");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "New York");
        assert_eq!(cands[0].raw, "New York");
    }

    #[test]
    fn alias_resolves_to_canonical_form() {
        let cands = extract("bonjour Londres");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "London");
        assert_eq!(cands[0].raw, "Londres");
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn preposition_assigns_role() {
        let cands = extract("weather in London");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].role.as_deref(), Some("in"));
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn from_to_assign_distinct_roles() {
        let cands = extract("flight from London to New York");
        assert_eq!(cands.len(), 2);
        let by_norm: std::collections::HashMap<&str, Option<&str>> = cands
            .iter()
            .map(|c| (c.normalized.as_str(), c.role.as_deref()))
            .collect();
        assert_eq!(by_norm.get("London").copied().flatten(), Some("from"));
        assert_eq!(by_norm.get("New York").copied().flatten(), Some("to"));
    }

    #[test]
    fn no_preposition_yields_no_role() {
        let cands = extract("London is cold today");
        assert_eq!(cands.len(), 1);
        assert!(cands[0].role.is_none());
    }

    #[test]
    fn byte_offsets_match_original_text() {
        let text = "weather in London tomorrow?";
        let tokens = WhitespaceTokenizer.tokenize(text);
        let cands = LocationExtractor.extract(
            &tokens,
            text,
            &ctx(),
            &locale(),
            &resources_with_gazetteer(),
        );
        let cand = cands
            .iter()
            .find(|c| c.normalized == "London")
            .expect("london");
        assert_eq!(&text[cand.start..cand.end], "London");
    }

    #[test]
    fn empty_gazetteer_yields_no_candidates() {
        let text = "weather in London";
        let tokens = WhitespaceTokenizer.tokenize(text);
        let resources = IntentResources::default();
        let cands = LocationExtractor.extract(&tokens, text, &ctx(), &locale(), &resources);
        assert!(cands.is_empty());
    }

    #[test]
    fn consecutive_locations_resolved_independently() {
        let cands = extract("London London London");
        assert_eq!(cands.len(), 3);
    }

    #[test]
    fn non_location_text_yields_no_candidates() {
        let cands = extract("hello world");
        assert!(cands.is_empty());
    }

    #[test]
    fn punctuation_between_preposition_and_location_still_assigns_role() {
        let cands = extract("flight to, London");
        assert_eq!(cands.len(), 1);
        // First non-ws/punct word before London is `to`.
        #[cfg(feature = "builtin-locales")]
        assert_eq!(cands[0].role.as_deref(), Some("to"));
    }
}
