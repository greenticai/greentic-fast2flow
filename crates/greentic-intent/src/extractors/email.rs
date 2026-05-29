//! Email extractor (feature `email`). Scans the text for `@` anchors,
//! expands left and right over RFC-5321-ish character classes, and
//! validates the resulting span with the `email_address` crate.

use crate::context::IntentContext;
use crate::entity::{EntityEvidence, EntityKind};
use crate::extractors::{EntityCandidate, EntityExtractor};
use crate::locale::ResolvedLocale;
use crate::resources::IntentResources;
use crate::token::Token;

/// Validated email extractor.
#[derive(Debug, Default, Clone, Copy)]
pub struct EmailExtractor;

impl EntityExtractor for EmailExtractor {
    fn name(&self) -> &'static str {
        "email"
    }

    fn extract(
        &self,
        _tokens: &[Token],
        text: &str,
        _ctx: &IntentContext,
        locale: &ResolvedLocale,
        _resources: &IntentResources,
    ) -> Vec<EntityCandidate> {
        let bytes = text.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'@' {
                let mut local_start = i;
                while local_start > 0 && is_local_char(bytes[local_start - 1]) {
                    local_start -= 1;
                }
                let mut domain_end = i + 1;
                while domain_end < bytes.len() && is_domain_char(bytes[domain_end]) {
                    domain_end += 1;
                }
                while domain_end > i + 1 && bytes[domain_end - 1] == b'.' {
                    domain_end -= 1;
                }
                if local_start < i && domain_end > i + 1 {
                    let candidate = &text[local_start..domain_end];
                    if email_address::EmailAddress::is_valid(candidate) {
                        out.push(EntityCandidate {
                            kind: EntityKind::Email,
                            raw: candidate.to_string(),
                            normalized: candidate.to_lowercase(),
                            canonical: None,
                            start: local_start,
                            end: domain_end,
                            role: None,
                            confidence: 0.98,
                            locale: locale.locale.clone(),
                            evidence: vec![EntityEvidence {
                                extractor: "email".into(),
                                rule: "validated_at_sign_expansion".into(),
                                detail: None,
                            }],
                        });
                    }
                }
                i = domain_end.max(i + 1);
            } else {
                i += 1;
            }
        }
        out
    }
}

fn is_local_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+')
}

fn is_domain_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::{Tokenizer, WhitespaceTokenizer};

    fn locale() -> ResolvedLocale {
        ResolvedLocale::fallback_en_gb()
    }
    fn ctx() -> IntentContext {
        IntentContext::now_utc("UTC")
    }
    fn resources() -> IntentResources {
        IntentResources::default()
    }

    fn extract(text: &str) -> Vec<EntityCandidate> {
        let tokens = WhitespaceTokenizer.tokenize(text);
        EmailExtractor.extract(&tokens, text, &ctx(), &locale(), &resources())
    }

    #[test]
    fn extracts_simple_email() {
        let cands = extract("contact alice@example.com please");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "alice@example.com");
        assert_eq!(cands[0].raw, "alice@example.com");
    }

    #[test]
    fn extracts_email_with_plus_addressing() {
        let cands = extract("send to support+billing@foo.io");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "support+billing@foo.io");
    }

    #[test]
    fn extracts_email_with_dotted_local_and_multilevel_domain() {
        let cands = extract("write to bob.smith@team.example.co.uk thanks");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "bob.smith@team.example.co.uk");
    }

    #[test]
    fn ignores_text_without_at_sign() {
        let cands = extract("there is no email here");
        assert!(cands.is_empty());
    }

    #[test]
    fn ignores_bare_at_sign() {
        let cands = extract("we met @ noon today");
        assert!(cands.is_empty());
    }

    #[test]
    fn normalized_form_is_lowercased() {
        let cands = extract("Alice@Example.COM");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].normalized, "alice@example.com");
        assert_eq!(cands[0].raw, "Alice@Example.COM");
    }

    #[test]
    fn span_excludes_trailing_sentence_dot() {
        let cands = extract("ping alice@example.com.");
        assert_eq!(cands.len(), 1);
        assert!(!cands[0].raw.ends_with('.'));
    }

    #[test]
    fn multiple_emails_in_text() {
        let cands = extract("cc alice@example.com and bob@example.org");
        assert_eq!(cands.len(), 2);
        let norms: Vec<&str> = cands.iter().map(|c| c.normalized.as_str()).collect();
        assert!(norms.contains(&"alice@example.com"));
        assert!(norms.contains(&"bob@example.org"));
    }

    #[test]
    fn byte_offsets_match_original_text() {
        let text = "ping alice@example.com today";
        let tokens = WhitespaceTokenizer.tokenize(text);
        let cands = EmailExtractor.extract(&tokens, text, &ctx(), &locale(), &resources());
        let cand = &cands[0];
        assert_eq!(&text[cand.start..cand.end], "alice@example.com");
    }
}
