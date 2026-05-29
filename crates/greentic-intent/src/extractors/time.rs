//! Time extractor: deterministic, locale-aware. Normalises to `HH:MM`
//! (24-hour, zero-padded).
//!
//! Supports (Phase A):
//!   - locale time words (`noon`, `midnight`, `midi`, `mediodía`, `mittag`)
//!   - 24-hour `HH:MM`         (`15:00`, `09:30`)
//!   - fused 12-hour `Hpm`     (`3pm`, `9am`)
//!   - split 12-hour `H pm`    (`3 pm`, `9 am`)
//!
//! Skips for Phase A: `3:30pm`, `half past three`, `quarter to four`,
//! `o'clock`, relative times (`in 2 hours`).

use crate::context::IntentContext;
use crate::entity::{EntityEvidence, EntityKind};
use crate::extractors::{EntityCandidate, EntityExtractor};
use crate::locale::{LocaleBundle, ResolvedLocale};
use crate::resources::IntentResources;
use crate::token::{Token, TokenShape};

/// Deterministic, locale-aware time extractor.
#[derive(Debug, Default, Clone, Copy)]
pub struct TimeExtractor;

impl EntityExtractor for TimeExtractor {
    fn name(&self) -> &'static str {
        "time"
    }

    fn extract(
        &self,
        tokens: &[Token],
        text: &str,
        _ctx: &IntentContext,
        locale: &ResolvedLocale,
        resources: &IntentResources,
    ) -> Vec<EntityCandidate> {
        let bundle = resources.locales.get(&locale.locale);
        let mut candidates = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            if let Some((cand, advance)) = match_pattern(tokens, i, text, locale, bundle) {
                candidates.push(cand);
                i += advance;
            } else {
                i += 1;
            }
        }
        candidates
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Meridiem {
    Am,
    Pm,
}

fn match_pattern(
    tokens: &[Token],
    idx: usize,
    text: &str,
    locale: &ResolvedLocale,
    bundle: Option<&LocaleBundle>,
) -> Option<(EntityCandidate, usize)> {
    let head = &tokens[idx];

    // 1. Locale time word (`noon`, `midnight`, `midi`, ...).
    if let Some(b) = bundle {
        if let Some(&(h, m)) = b.time_words.get(&head.lower) {
            return Some((
                make_candidate(
                    head.text.clone(),
                    format_time(h, m),
                    head.start,
                    head.end,
                    locale,
                    "time_word",
                    0.95,
                ),
                1,
            ));
        }
    }

    // 2. & 3. require a numeric head token.
    if head.shape == TokenShape::Number {
        if let Ok(h) = head.text.parse::<u8>() {
            // 2. 24-hour `HH:MM` — three-token pattern: number, ':', number.
            if idx + 2 < tokens.len()
                && tokens[idx + 1].text == ":"
                && tokens[idx + 2].shape == TokenShape::Number
                && tokens[idx + 2].text.len() == 2
            {
                if let Ok(m) = tokens[idx + 2].text.parse::<u8>() {
                    if (0..=23).contains(&h) && (0..=59).contains(&m) {
                        let end = tokens[idx + 2].end;
                        return Some((
                            make_candidate(
                                text[head.start..end].to_string(),
                                format_time(h, m),
                                head.start,
                                end,
                                locale,
                                "hh_mm_24h",
                                0.95,
                            ),
                            3,
                        ));
                    }
                }
            }

            // 3. Split 12-hour `H am|pm` — number, optional ws/punct, then am/pm word.
            if (1..=12).contains(&h) {
                if let Some((nxt_idx, nxt)) = next_word_token(tokens, idx + 1) {
                    if let Some(mer) = parse_meridiem(&nxt.lower) {
                        return Some((
                            make_candidate(
                                text[head.start..nxt.end].to_string(),
                                format_time(to_24h(h, mer), 0),
                                head.start,
                                nxt.end,
                                locale,
                                "h_meridiem_split",
                                0.9,
                            ),
                            nxt_idx - idx + 1,
                        ));
                    }
                }
            }
        }
    }

    // 4. Fused 12-hour `Hpm` — single mixed token like `3pm`.
    if head.shape == TokenShape::Mixed {
        if let Some((h, mer)) = parse_h_meridiem(&head.lower) {
            return Some((
                make_candidate(
                    head.text.clone(),
                    format_time(to_24h(h, mer), 0),
                    head.start,
                    head.end,
                    locale,
                    "h_meridiem_fused",
                    0.9,
                ),
                1,
            ));
        }
    }

    None
}

fn parse_meridiem(s: &str) -> Option<Meridiem> {
    match s {
        "am" => Some(Meridiem::Am),
        "pm" => Some(Meridiem::Pm),
        _ => None,
    }
}

fn parse_h_meridiem(s: &str) -> Option<(u8, Meridiem)> {
    let split = s.bytes().position(|b| !b.is_ascii_digit())?;
    if split == 0 {
        return None;
    }
    let digits = &s[..split];
    let suffix = &s[split..];
    let mer = parse_meridiem(suffix)?;
    let h: u8 = digits.parse().ok()?;
    if (1..=12).contains(&h) {
        Some((h, mer))
    } else {
        None
    }
}

fn to_24h(hour: u8, mer: Meridiem) -> u8 {
    match (hour, mer) {
        (12, Meridiem::Am) => 0,
        (h, Meridiem::Am) => h,
        (12, Meridiem::Pm) => 12,
        (h, Meridiem::Pm) => h + 12,
    }
}

fn format_time(h: u8, m: u8) -> String {
    format!("{h:02}:{m:02}")
}

fn next_word_token(tokens: &[Token], from: usize) -> Option<(usize, &Token)> {
    tokens.iter().enumerate().skip(from).find(|(_, t)| {
        matches!(
            t.shape,
            TokenShape::Word | TokenShape::Number | TokenShape::Mixed
        )
    })
}

fn make_candidate(
    raw: String,
    normalized: String,
    start: usize,
    end: usize,
    locale: &ResolvedLocale,
    rule: &'static str,
    confidence: f32,
) -> EntityCandidate {
    EntityCandidate {
        kind: EntityKind::Time,
        raw,
        normalized,
        canonical: None,
        start,
        end,
        role: None,
        confidence,
        locale: locale.locale.clone(),
        evidence: vec![EntityEvidence {
            extractor: "time".into(),
            rule: rule.into(),
            detail: None,
        }],
    }
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
            r.locales
                .insert("en-GB".into(), crate::locale::LocaleBundle::default());
        }
        r
    }

    fn extract(text: &str) -> Vec<EntityCandidate> {
        let tokens = WhitespaceTokenizer.tokenize(text);
        TimeExtractor.extract(&tokens, text, &ctx(), &locale(), &resources_en_gb())
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn locale_word_noon_resolves_to_12_00() {
        let cands = extract("meeting at noon");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "12:00");
        assert_eq!(cands[0].raw, "noon");
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn locale_word_midnight_resolves_to_00_00() {
        let cands = extract("call me at midnight");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "00:00");
    }

    #[test]
    fn fused_pm_token_resolves_to_24h() {
        let cands = extract("ship it 3pm sharp");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "15:00");
        assert_eq!(cands[0].raw, "3pm");
    }

    #[test]
    fn fused_am_token_resolves_to_24h() {
        let cands = extract("call at 9am");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "09:00");
    }

    #[test]
    fn split_h_meridiem_resolves() {
        let cands = extract("meeting at 3 pm");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "15:00");
        assert_eq!(cands[0].raw, "3 pm");
    }

    #[test]
    fn twenty_four_hour_resolves() {
        let cands = extract("slot at 15:30");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "15:30");
        assert_eq!(cands[0].raw, "15:30");
    }

    #[test]
    fn twenty_four_hour_zero_padded_minutes() {
        let cands = extract("at 09:00 sharp");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "09:00");
    }

    #[test]
    fn twelve_am_is_midnight_in_24h() {
        let cands = extract("starts at 12am");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "00:00");
    }

    #[test]
    fn twelve_pm_is_noon_in_24h() {
        let cands = extract("ends at 12pm");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "12:00");
    }

    #[test]
    fn invalid_24h_hour_rejected() {
        let cands = extract("ship at 25:00");
        assert!(cands.is_empty());
    }

    #[test]
    fn invalid_24h_minute_rejected() {
        let cands = extract("ship at 09:75");
        assert!(cands.is_empty());
    }

    #[test]
    fn bare_number_alone_is_not_a_time() {
        let cands = extract("we have 5 items");
        assert!(cands.is_empty());
    }

    #[test]
    fn meridiem_must_pair_with_one_to_twelve() {
        // `13pm` is invalid for 12-hour form — Mixed token rejected.
        let cands = extract("scheduled 13pm");
        assert!(cands.is_empty());
    }

    #[test]
    fn byte_offsets_match_original_text() {
        let text = "ship it at 3pm";
        let tokens = WhitespaceTokenizer.tokenize(text);
        let cands = TimeExtractor.extract(&tokens, text, &ctx(), &locale(), &resources_en_gb());
        assert_eq!(cands.len(), 1);
        let cand = &cands[0];
        assert_eq!(&text[cand.start..cand.end], "3pm");
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn multiple_times_in_one_sentence() {
        let cands = extract("start at 9am and finish at 5pm");
        assert_eq!(cands.len(), 2);
        assert_eq!(cands[0].normalized, "09:00");
        assert_eq!(cands[1].normalized, "17:00");
    }

    #[test]
    fn empty_bundle_still_handles_numeric_forms() {
        let mut empty = IntentResources::default();
        empty
            .locales
            .insert("en-GB".into(), crate::locale::LocaleBundle::default());
        let text = "at 15:00";
        let tokens = WhitespaceTokenizer.tokenize(text);
        let cands = TimeExtractor.extract(&tokens, text, &ctx(), &locale(), &empty);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "15:00");
    }
}
