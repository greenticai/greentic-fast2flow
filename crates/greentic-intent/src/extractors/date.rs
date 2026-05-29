//! Date extractor: deterministic, locale-aware. Normalises to `YYYYMMDD`.
//!
//! Supports (Phase A):
//!   - relative days  (`today`, `tomorrow`, `yesterday`, …)
//!   - bare weekdays  (`saturday` → next Saturday, today included)
//!   - `next|this|last <weekday>` modifiers
//!   - `in <N> day[s]` / `in <N> week[s]`
//!
//! Relies on `LocaleBundle` for vocabulary. Empty bundles → no matches.

use chrono::{DateTime, Datelike, Duration, Utc};

use crate::context::IntentContext;
use crate::entity::{EntityEvidence, EntityKind};
use crate::extractors::{EntityCandidate, EntityExtractor};
use crate::locale::{LocaleBundle, RelativeModifier, ResolvedLocale};
use crate::resources::IntentResources;
use crate::token::{Token, TokenShape};

/// Deterministic, locale-aware date extractor.
#[derive(Debug, Default, Clone, Copy)]
pub struct DateExtractor;

impl EntityExtractor for DateExtractor {
    fn name(&self) -> &'static str {
        "date"
    }

    fn extract(
        &self,
        tokens: &[Token],
        _text: &str,
        ctx: &IntentContext,
        locale: &ResolvedLocale,
        resources: &IntentResources,
    ) -> Vec<EntityCandidate> {
        let Some(bundle) = resources.locales.get(&locale.locale) else {
            return Vec::new();
        };
        let mut candidates = Vec::new();
        let mut idx = 0;
        while idx < tokens.len() {
            if let Some((span, advance)) = match_pattern(tokens, idx, bundle, ctx, locale) {
                candidates.push(span);
                idx += advance;
            } else {
                idx += 1;
            }
        }
        candidates
    }
}

fn match_pattern(
    tokens: &[Token],
    idx: usize,
    bundle: &LocaleBundle,
    ctx: &IntentContext,
    locale: &ResolvedLocale,
) -> Option<(EntityCandidate, usize)> {
    let head = &tokens[idx];
    if !is_word_or_number(head) {
        return None;
    }

    // 1. Relative day word (`today`, `tomorrow`, …)
    if let Some(&offset) = bundle.relative_days.get(&head.lower) {
        let raw = head.text.clone();
        let normalized = format_offset_date(ctx.reference_time, offset as i64);
        return Some((
            make_candidate(
                raw,
                normalized,
                head.start,
                head.end,
                locale,
                "relative_day_word",
            ),
            1,
        ));
    }

    // 2. Modifier + weekday (e.g. `next monday`, `last friday`).
    if let Some(modifier) = bundle.modifiers.get(&head.lower).copied() {
        if let Some((rel_idx, rel_token)) = next_word_token(tokens, idx + 1) {
            // Modifier + weekday → adjusted weekday.
            if let Some(&weekday) = bundle.weekdays.get(&rel_token.lower) {
                let offset = weekday_offset_with_modifier(
                    ctx.reference_time.weekday().num_days_from_monday() as u8 + 1,
                    weekday,
                    modifier,
                )?;
                let normalized = format_offset_date(ctx.reference_time, offset);
                let raw = surface_span(tokens, idx, rel_idx);
                return Some((
                    make_candidate(
                        raw,
                        normalized,
                        head.start,
                        rel_token.end,
                        locale,
                        "modifier_weekday",
                    ),
                    rel_idx - idx + 1,
                ));
            }
            // Modifier `in` + number + unit (e.g. `in 3 days`).
            if matches!(modifier, RelativeModifier::In) && is_pure_number(rel_token) {
                if let Some((unit_idx, unit_token)) = next_word_token(tokens, rel_idx + 1) {
                    if let Some(unit) = parse_relative_unit(&unit_token.lower) {
                        if let Ok(n) = rel_token.text.parse::<i64>() {
                            let offset = n * unit_factor(unit);
                            let normalized = format_offset_date(ctx.reference_time, offset);
                            let raw = surface_span(tokens, idx, unit_idx);
                            return Some((
                                make_candidate(
                                    raw,
                                    normalized,
                                    head.start,
                                    unit_token.end,
                                    locale,
                                    "in_n_units",
                                ),
                                unit_idx - idx + 1,
                            ));
                        }
                    }
                }
            }
        }
    }

    // 3. Bare weekday (e.g. `saturday`) — picks today-or-later.
    if let Some(&weekday) = bundle.weekdays.get(&head.lower) {
        let today_dow = ctx.reference_time.weekday().num_days_from_monday() as u8 + 1;
        let offset = signed_offset_to_weekday(today_dow, weekday, RelativeModifier::This)?;
        let normalized = format_offset_date(ctx.reference_time, offset);
        return Some((
            make_candidate(
                head.text.clone(),
                normalized,
                head.start,
                head.end,
                locale,
                "bare_weekday",
            ),
            1,
        ));
    }

    None
}

#[derive(Clone, Copy)]
enum RelativeUnit {
    Day,
    Week,
}

fn parse_relative_unit(word: &str) -> Option<RelativeUnit> {
    match word {
        "day" | "days" => Some(RelativeUnit::Day),
        "week" | "weeks" => Some(RelativeUnit::Week),
        _ => None,
    }
}

fn unit_factor(unit: RelativeUnit) -> i64 {
    match unit {
        RelativeUnit::Day => 1,
        RelativeUnit::Week => 7,
    }
}

fn is_word_or_number(token: &Token) -> bool {
    matches!(
        token.shape,
        TokenShape::Word | TokenShape::Number | TokenShape::Mixed
    )
}

fn is_pure_number(token: &Token) -> bool {
    matches!(token.shape, TokenShape::Number)
}

/// Returns `(index, token)` of the next word-or-number token at or after
/// `from`, skipping whitespace + punctuation.
fn next_word_token(tokens: &[Token], from: usize) -> Option<(usize, &Token)> {
    tokens
        .iter()
        .enumerate()
        .skip(from)
        .find(|(_, t)| is_word_or_number(t))
}

/// Slice from the start of `tokens[from]` to the end of `tokens[to]`.
/// Falls back to single token text if indices are degenerate.
fn surface_span(tokens: &[Token], from: usize, to: usize) -> String {
    if from > to || to >= tokens.len() {
        return tokens.get(from).map(|t| t.text.clone()).unwrap_or_default();
    }
    let start = tokens[from].start;
    let end = tokens[to].end;
    // Reconstruct from the head/tail token surfaces + intervening tokens.
    let mut buf = String::with_capacity(end - start);
    for token in &tokens[from..=to] {
        buf.push_str(&token.text);
    }
    buf
}

fn format_offset_date(reference: DateTime<Utc>, offset_days: i64) -> String {
    let target = reference + Duration::days(offset_days);
    target.format("%Y%m%d").to_string()
}

/// `today_dow` and `target_dow` are 1=Monday … 7=Sunday.
fn signed_offset_to_weekday(
    today_dow: u8,
    target_dow: u8,
    modifier: RelativeModifier,
) -> Option<i64> {
    if !(1..=7).contains(&today_dow) || !(1..=7).contains(&target_dow) {
        return None;
    }
    let diff = (target_dow as i32 - today_dow as i32).rem_euclid(7) as i64;
    let base = match modifier {
        RelativeModifier::This => diff,
        RelativeModifier::Next => {
            // "next" semantics: smallest strictly-positive offset (a full
            // week out if the bare weekday is today/upcoming).
            if diff == 0 {
                7
            } else {
                diff + 7
            }
        }
        RelativeModifier::Last => {
            // Previous occurrence (could be today if today is target).
            if diff == 0 {
                0
            } else {
                diff - 7
            }
        }
        _ => return None,
    };
    Some(base)
}

fn weekday_offset_with_modifier(
    today_dow: u8,
    target_dow: u8,
    modifier: RelativeModifier,
) -> Option<i64> {
    signed_offset_to_weekday(today_dow, target_dow, modifier)
}

fn make_candidate(
    raw: String,
    normalized: String,
    start: usize,
    end: usize,
    locale: &ResolvedLocale,
    rule: &'static str,
) -> EntityCandidate {
    EntityCandidate {
        kind: EntityKind::Date,
        raw,
        normalized,
        canonical: None,
        start,
        end,
        role: None,
        confidence: 0.95,
        locale: locale.locale.clone(),
        evidence: vec![EntityEvidence {
            extractor: "date".to_string(),
            rule: rule.to_string(),
            detail: None,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::{Tokenizer, WhitespaceTokenizer};
    use chrono::TimeZone;

    fn reference() -> DateTime<Utc> {
        // 2026-05-27 is a Wednesday (1=Monday … 7=Sunday → dow=3).
        Utc.with_ymd_and_hms(2026, 5, 27, 12, 0, 0).unwrap()
    }

    fn ctx() -> IntentContext {
        IntentContext {
            reference_time: reference(),
            timezone: "Europe/London".into(),
            preferred_locale: Some("en-GB".into()),
            tenant_locale: None,
            user_locale: None,
            allowed_languages: Vec::new(),
        }
    }

    fn resources_with_en_gb() -> IntentResources {
        let mut r = IntentResources::default();
        #[cfg(feature = "builtin-locales")]
        {
            r.locales
                .insert("en-GB".into(), crate::builtin::en_gb_bundle());
        }
        #[cfg(not(feature = "builtin-locales"))]
        {
            use crate::locale::LocaleBundle;
            r.locales.insert("en-GB".into(), LocaleBundle::default());
        }
        r
    }

    fn locale() -> ResolvedLocale {
        ResolvedLocale {
            locale: "en-GB".into(),
            language: "en".into(),
            script: "Latin".into(),
        }
    }

    fn extract(text: &str) -> Vec<EntityCandidate> {
        let tokens = WhitespaceTokenizer.tokenize(text);
        DateExtractor.extract(&tokens, text, &ctx(), &locale(), &resources_with_en_gb())
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn relative_day_today() {
        let cands = extract("meeting today");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "20260527");
        assert_eq!(cands[0].raw, "today");
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn relative_day_tomorrow_with_reference_date() {
        let cands = extract("weather tomorrow");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "20260528");
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn relative_day_yesterday_resolves_negative() {
        let cands = extract("did anyone email yesterday");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "20260526");
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn bare_weekday_saturday_resolves_to_next_saturday() {
        // 2026-05-27 (Wed) + 3 = 2026-05-30 (Sat).
        let cands = extract("see you Saturday");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "20260530");
        assert_eq!(cands[0].raw, "Saturday");
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn next_monday_jumps_full_week() {
        // 2026-05-27 (Wed) → upcoming Monday is 2026-06-01.
        // "next Monday" jumps a further 7 days → 2026-06-08.
        let cands = extract("ship it next Monday");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "20260608");
        assert!(cands[0].raw.contains("next"));
        assert!(cands[0].raw.contains("Monday"));
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn last_friday_returns_previous_occurrence() {
        // 2026-05-27 (Wed) → last Friday is 2026-05-22.
        let cands = extract("we discussed last Friday");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "20260522");
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn in_3_days_resolves() {
        // 2026-05-27 + 3 days = 2026-05-30.
        let cands = extract("review in 3 days");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "20260530");
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn in_2_weeks_resolves() {
        // 2026-05-27 + 14 days = 2026-06-10.
        let cands = extract("retro in 2 weeks");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "20260610");
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn byte_offsets_match_original_text() {
        let text = "weather tomorrow";
        let tokens = WhitespaceTokenizer.tokenize(text);
        let cands =
            DateExtractor.extract(&tokens, text, &ctx(), &locale(), &resources_with_en_gb());
        let cand = &cands[0];
        assert_eq!(&text[cand.start..cand.end], "tomorrow");
    }

    #[test]
    fn empty_bundle_returns_no_candidates() {
        let mut empty = IntentResources::default();
        empty
            .locales
            .insert("en-GB".into(), crate::locale::LocaleBundle::default());
        let text = "tomorrow";
        let tokens = WhitespaceTokenizer.tokenize(text);
        let cands = DateExtractor.extract(&tokens, text, &ctx(), &locale(), &empty);
        assert!(cands.is_empty());
    }

    #[test]
    fn no_locale_bundle_returns_no_candidates() {
        let no_locale = IntentResources::default();
        let text = "tomorrow";
        let tokens = WhitespaceTokenizer.tokenize(text);
        let cands = DateExtractor.extract(&tokens, text, &ctx(), &locale(), &no_locale);
        assert!(cands.is_empty());
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn non_date_text_yields_no_candidates() {
        let cands = extract("hello world");
        assert!(cands.is_empty());
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn multiple_dates_in_one_sentence() {
        let cands = extract("review today and ship tomorrow");
        assert_eq!(cands.len(), 2);
        assert_eq!(cands[0].normalized, "20260527");
        assert_eq!(cands[1].normalized, "20260528");
    }
}
