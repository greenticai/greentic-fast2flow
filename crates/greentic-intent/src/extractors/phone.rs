//! Phone extractor (feature `phone`). Walks the text for digit-heavy
//! runs, then asks the `phonenumber` crate to validate them in E.164.
//! Default region is derived from the active locale (`en-GB → GB`).
//! Numbers in international form (`+44 …`) parse without a region.

use crate::context::IntentContext;
use crate::entity::{EntityEvidence, EntityKind};
use crate::extractors::{EntityCandidate, EntityExtractor};
use crate::locale::ResolvedLocale;
use crate::resources::IntentResources;
use crate::token::Token;

use phonenumber::country;

/// Phone extractor backed by the `phonenumber` crate.
#[derive(Debug, Default, Clone, Copy)]
pub struct PhoneExtractor;

impl EntityExtractor for PhoneExtractor {
    fn name(&self) -> &'static str {
        "phone"
    }

    fn extract(
        &self,
        _tokens: &[Token],
        text: &str,
        _ctx: &IntentContext,
        locale: &ResolvedLocale,
        _resources: &IntentResources,
    ) -> Vec<EntityCandidate> {
        let default_region = region_from_locale(&locale.locale);
        let mut out = Vec::new();
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if starts_phone_run(bytes[i]) {
                let start = i;
                let mut end = i;
                let mut digit_count = 0usize;
                while end < bytes.len() && is_phone_char(bytes[end]) {
                    if bytes[end].is_ascii_digit() {
                        digit_count += 1;
                    }
                    end += 1;
                }
                let mut clean_end = end;
                while clean_end > start && !bytes[clean_end - 1].is_ascii_digit() {
                    clean_end -= 1;
                }
                if digit_count >= 7 && clean_end > start {
                    let candidate = &text[start..clean_end];
                    if let Ok(pn) = phonenumber::parse(default_region, candidate) {
                        if pn.is_valid() {
                            let canonical = pn.format().mode(phonenumber::Mode::E164).to_string();
                            out.push(EntityCandidate {
                                kind: EntityKind::Phone,
                                raw: candidate.to_string(),
                                normalized: canonical.clone(),
                                canonical: Some(canonical),
                                start,
                                end: clean_end,
                                role: None,
                                confidence: 0.97,
                                locale: locale.locale.clone(),
                                evidence: vec![EntityEvidence {
                                    extractor: "phone".into(),
                                    rule: "phonenumber_validated".into(),
                                    detail: None,
                                }],
                            });
                        }
                    }
                }
                i = end.max(i + 1);
            } else {
                i += 1;
            }
        }
        out
    }
}

fn starts_phone_run(b: u8) -> bool {
    b.is_ascii_digit() || b == b'+'
}

fn is_phone_char(b: u8) -> bool {
    b.is_ascii_digit() || matches!(b, b'+' | b'(' | b')' | b'-' | b'.' | b' ')
}

fn region_from_locale(locale: &str) -> Option<country::Id> {
    let region = locale.split('-').nth(1)?;
    Some(match region {
        "GB" => country::Id::GB,
        "US" => country::Id::US,
        "FR" => country::Id::FR,
        "ES" => country::Id::ES,
        "NL" => country::Id::NL,
        "DE" => country::Id::DE,
        "BE" => country::Id::BE,
        "IT" => country::Id::IT,
        "JP" => country::Id::JP,
        "IE" => country::Id::IE,
        "PT" => country::Id::PT,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::{Tokenizer, WhitespaceTokenizer};

    fn locale_gb() -> ResolvedLocale {
        ResolvedLocale::fallback_en_gb()
    }
    fn ctx() -> IntentContext {
        IntentContext::now_utc("Europe/London")
    }
    fn resources() -> IntentResources {
        IntentResources::default()
    }

    fn extract_with(text: &str, locale: ResolvedLocale) -> Vec<EntityCandidate> {
        let tokens = WhitespaceTokenizer.tokenize(text);
        PhoneExtractor.extract(&tokens, text, &ctx(), &locale, &resources())
    }

    #[test]
    fn extracts_international_form_without_default_region() {
        // International form (`+44 …`) parses regardless of region.
        let none_locale = ResolvedLocale {
            locale: "xx-XX".into(),
            language: "xx".into(),
            script: "Latin".into(),
        };
        let cands = extract_with("call me at +44 20 7946 0958 anytime", none_locale);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "+442079460958");
    }

    #[test]
    fn extracts_national_form_with_locale_region() {
        // National GB form needs a default region.
        let cands = extract_with("ring 020 7946 0958 in the morning", locale_gb());
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "+442079460958");
    }

    #[test]
    fn rejects_too_short_digit_run() {
        // Six digits is below the threshold for a phone candidate.
        let cands = extract_with("ref 123456 today", locale_gb());
        assert!(cands.is_empty());
    }

    #[test]
    fn rejects_invalid_phonenumber_even_with_enough_digits() {
        // Random 10 digits that don't parse as a real number.
        let cands = extract_with("token 0000000000 noted", locale_gb());
        assert!(cands.is_empty());
    }

    #[test]
    fn span_trims_trailing_non_digits() {
        let cands = extract_with("call (020) 7946 0958.", locale_gb());
        assert_eq!(cands.len(), 1);
        // The trailing `.` must not be part of the span.
        assert!(!cands[0].raw.ends_with('.'));
        assert_eq!(cands[0].normalized, "+442079460958");
    }

    #[test]
    fn byte_offsets_match_original_text() {
        let text = "call +44 20 7946 0958 thanks";
        let tokens = WhitespaceTokenizer.tokenize(text);
        let cands = PhoneExtractor.extract(&tokens, text, &ctx(), &locale_gb(), &resources());
        let cand = &cands[0];
        assert_eq!(&text[cand.start..cand.end], "+44 20 7946 0958");
    }

    #[test]
    fn ignores_text_without_phone_chars() {
        let cands = extract_with("there is no phone here", locale_gb());
        assert!(cands.is_empty());
    }
}
