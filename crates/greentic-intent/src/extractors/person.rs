//! Person extractor: context-cue driven, locale-aware.
//!
//! For each `people_context_before` cue word in the locale bundle,
//! treat the next word-token as a candidate person. Filters out
//! obvious non-names via a stop-word list + a capitalization /
//! length heuristic.
//!
//! Phase A handles cue → name only. The `context_after` pattern
//! (`Bella is here`) lands later.

use crate::context::IntentContext;
use crate::entity::{EntityEvidence, EntityKind};
use crate::extractors::{EntityCandidate, EntityExtractor};
use crate::locale::{LocaleBundle, ResolvedLocale};
use crate::resources::IntentResources;
use crate::token::{Token, TokenShape};

/// Cue-driven person extractor.
#[derive(Debug, Default, Clone, Copy)]
pub struct PersonExtractor;

impl EntityExtractor for PersonExtractor {
    fn name(&self) -> &'static str {
        "person"
    }

    fn extract(
        &self,
        tokens: &[Token],
        _text: &str,
        _ctx: &IntentContext,
        locale: &ResolvedLocale,
        resources: &IntentResources,
    ) -> Vec<EntityCandidate> {
        let Some(bundle) = resources.locales.get(&locale.locale) else {
            return Vec::new();
        };
        if bundle.people_context_before.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            if let Some((cand, advance)) = match_pattern(tokens, i, locale, bundle) {
                out.push(cand);
                i += advance;
            } else {
                i += 1;
            }
        }
        out
    }
}

fn match_pattern(
    tokens: &[Token],
    idx: usize,
    locale: &ResolvedLocale,
    bundle: &LocaleBundle,
) -> Option<(EntityCandidate, usize)> {
    let head = &tokens[idx];
    if head.shape != TokenShape::Word {
        return None;
    }
    if !bundle.people_context_before.contains(&head.lower) {
        return None;
    }
    let (nxt_idx, nxt) = next_word_token(tokens, idx + 1)?;
    if nxt.shape != TokenShape::Word {
        return None;
    }
    if is_stop_word(&nxt.lower) {
        return None;
    }
    // Cue itself shouldn't echo as the candidate.
    if bundle.people_context_before.contains(&nxt.lower) {
        return None;
    }
    // Confidence heuristic: capitalized words are clearly names; bare
    // lower-case 3+ char words can be too (chat users skip caps).
    let leads_upper = nxt
        .text
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false);
    if !leads_upper && nxt.lower.len() < 3 {
        return None;
    }
    Some((
        EntityCandidate {
            kind: EntityKind::Person,
            raw: nxt.text.clone(),
            normalized: nxt.text.clone(),
            canonical: None,
            start: nxt.start,
            end: nxt.end,
            role: None,
            // Below location/date so gazetteer + date wins on overlap;
            // above number so any candidate beats a bare digit.
            confidence: 0.7,
            locale: locale.locale.clone(),
            evidence: vec![EntityEvidence {
                extractor: "person".into(),
                rule: "context_before_cue".into(),
                detail: None,
            }],
        },
        nxt_idx - idx + 1,
    ))
}

fn next_word_token(tokens: &[Token], from: usize) -> Option<(usize, &Token)> {
    tokens.iter().enumerate().skip(from).find(|(_, t)| {
        matches!(
            t.shape,
            TokenShape::Word | TokenShape::Number | TokenShape::Mixed
        )
    })
}

/// Words that are never a person name. Kept extractor-local rather
/// than per-locale so it's trivial to audit; if a real deployment
/// needs per-locale stops, lift this into `LocaleBundle` later.
fn is_stop_word(lower: &str) -> bool {
    matches!(
        lower,
        // articles + pronouns
        "a" | "an" | "the" | "my" | "his" | "her" | "your" | "our" | "their" | "its"
        | "that" | "this" | "those" | "these"
        | "some" | "any" | "all" | "no" | "every" | "each"
        | "one" | "two" | "three" | "four" | "five"
        | "and" | "or" | "but" | "so" | "if" | "then"
        // time-ish words
        | "today" | "tomorrow" | "yesterday" | "now" | "later" | "soon"
        | "monday" | "tuesday" | "wednesday" | "thursday"
        | "friday" | "saturday" | "sunday"
        | "mon" | "tue" | "tues" | "wed" | "thu" | "thurs" | "fri" | "sat" | "sun"
        | "noon" | "midday" | "midnight" | "am" | "pm"
        // common adjectives
        | "new" | "old" | "big" | "small" | "regular" | "special"
        // pet-type words (also cues — never names themselves)
        | "pet" | "dog" | "cat" | "animal" | "puppy" | "kitten"
        // domain nouns
        | "daycare" | "kennel" | "room" | "vet" | "appointment" | "meeting"
        // prepositions (also cues but never the name)
        | "in" | "for" | "with" | "about" | "on" | "off"
        | "at" | "by" | "from" | "of" | "to"
        // misc
        | "here" | "there" | "anywhere" | "somewhere"
        | "ok" | "yes" | "please" | "thanks"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn resources_en_gb() -> IntentResources {
        let mut r = IntentResources::default();
        #[cfg(feature = "builtin-locales")]
        {
            r.locales
                .insert("en-GB".into(), crate::builtin::en_gb_bundle());
        }
        #[cfg(not(feature = "builtin-locales"))]
        {
            use crate::locale::LocaleBundle;
            let mut bundle = LocaleBundle {
                locale: "en-GB".into(),
                ..Default::default()
            };
            bundle.people_context_before.insert("for".into());
            bundle.people_context_before.insert("with".into());
            bundle.people_context_before.insert("check".into());
            bundle.people_context_before.insert("in".into());
            bundle.people_context_before.insert("dog".into());
            r.locales.insert("en-GB".into(), bundle);
        }
        r
    }

    fn extract(text: &str) -> Vec<EntityCandidate> {
        let tokens = WhitespaceTokenizer.tokenize(text);
        PersonExtractor.extract(&tokens, text, &ctx(), &locale(), &resources_en_gb())
    }

    #[test]
    fn extracts_after_action_verb_cue() {
        let cands = extract("check in Bella for the day");
        assert!(cands.iter().any(|c| c.normalized == "Bella"));
    }

    #[test]
    fn extracts_after_preposition_cue() {
        let cands = extract("note for Cooper: didn't eat");
        assert!(cands.iter().any(|c| c.normalized == "Cooper"));
    }

    #[test]
    fn extracts_after_pet_type_cue() {
        let cands = extract("register a new dog Max");
        assert!(cands.iter().any(|c| c.normalized == "Max"));
    }

    #[test]
    fn skips_stop_words_following_cue() {
        // "for the meeting" — "the" should never become a person.
        let cands = extract("for the meeting");
        assert!(cands.iter().all(|c| c.normalized.to_lowercase() != "the"));
    }

    #[test]
    fn skips_cue_word_following_cue() {
        // "check in for" — second cue shouldn't surface as a candidate.
        let cands = extract("check in for Bella");
        assert!(cands.iter().all(|c| c.normalized.to_lowercase() != "for"));
        // The cascade still extracts the real name.
        assert!(cands.iter().any(|c| c.normalized == "Bella"));
    }

    #[test]
    fn no_cue_no_extraction() {
        let cands = extract("hello world");
        assert!(cands.is_empty());
    }

    #[test]
    fn lowercase_short_words_rejected() {
        // "for ok" — short lowercase word, rejected.
        let cands = extract("for ok please");
        assert!(cands.is_empty());
    }

    #[test]
    fn span_matches_original_text() {
        let text = "check in Bella tomorrow";
        let tokens = WhitespaceTokenizer.tokenize(text);
        let cands = PersonExtractor.extract(&tokens, text, &ctx(), &locale(), &resources_en_gb());
        let bella = cands.iter().find(|c| c.normalized == "Bella").unwrap();
        assert_eq!(&text[bella.start..bella.end], "Bella");
    }

    #[test]
    fn empty_cue_set_yields_no_candidates() {
        use crate::locale::LocaleBundle;
        let mut r = IntentResources::default();
        r.locales.insert(
            "en-GB".into(),
            LocaleBundle {
                locale: "en-GB".into(),
                ..Default::default()
            },
        );
        let tokens = WhitespaceTokenizer.tokenize("check in Bella");
        let cands = PersonExtractor.extract(&tokens, "check in Bella", &ctx(), &locale(), &r);
        assert!(cands.is_empty());
    }
}
