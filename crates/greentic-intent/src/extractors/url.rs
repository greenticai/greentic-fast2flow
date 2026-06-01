//! URL extractor (feature `url`). Walks the text for `http://` /
//! `https://` anchors, expands rightward over the RFC-3986 character
//! set, trims trailing sentence punctuation, then asks the `url` crate
//! to validate the candidate.

use crate::context::IntentContext;
use crate::entity::{EntityEvidence, EntityKind};
use crate::extractors::{EntityCandidate, EntityExtractor};
use crate::locale::ResolvedLocale;
use crate::resources::IntentResources;
use crate::token::Token;

/// Validated URL extractor.
#[derive(Debug, Default, Clone, Copy)]
pub struct UrlExtractor;

impl EntityExtractor for UrlExtractor {
    fn name(&self) -> &'static str {
        "url"
    }

    fn extract(
        &self,
        _tokens: &[Token],
        text: &str,
        _ctx: &IntentContext,
        locale: &ResolvedLocale,
        _resources: &IntentResources,
    ) -> Vec<EntityCandidate> {
        let mut out = Vec::new();
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let tail = &text[i..];
            let prefix_len = if tail.starts_with("https://") {
                Some(8)
            } else if tail.starts_with("http://") {
                Some(7)
            } else {
                None
            };
            if let Some(plen) = prefix_len {
                let mut end = i + plen;
                while end < bytes.len() && is_url_char(bytes[end]) {
                    end += 1;
                }
                while end > i + plen
                    && matches!(
                        bytes[end - 1],
                        b'.' | b',' | b';' | b':' | b'!' | b'?' | b')' | b']' | b'}'
                    )
                {
                    end -= 1;
                }
                let candidate = &text[i..end];
                if let Ok(u) = ::url::Url::parse(candidate) {
                    if u.has_host() {
                        out.push(EntityCandidate {
                            kind: EntityKind::Url,
                            raw: candidate.to_string(),
                            normalized: u.as_str().to_string(),
                            canonical: None,
                            start: i,
                            end,
                            role: None,
                            confidence: 0.97,
                            locale: locale.locale.clone(),
                            evidence: vec![EntityEvidence {
                                extractor: "url".into(),
                                rule: "url_parse_validated".into(),
                                detail: None,
                            }],
                        });
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

fn is_url_char(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'-' | b'_'
                | b'.'
                | b'~'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'/'
                | b'@'
                | b'?'
                | b'#'
                | b'['
                | b']'
                | b'%'
        )
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
        UrlExtractor.extract(&tokens, text, &ctx(), &locale(), &resources())
    }

    #[test]
    fn extracts_https_with_host_only() {
        let cands = extract("see https://example.com for details");
        assert_eq!(cands.len(), 1);
        // url crate canonicalises with trailing slash.
        assert!(cands[0].normalized.starts_with("https://example.com"));
        assert_eq!(cands[0].raw, "https://example.com");
    }

    #[test]
    fn extracts_http_with_path_and_query() {
        let cands = extract("ping http://foo.bar/baz?q=1 thanks");
        assert_eq!(cands.len(), 1);
        assert!(cands[0].raw.starts_with("http://foo.bar/baz?q=1"));
    }

    #[test]
    fn trims_trailing_sentence_punctuation() {
        let cands = extract("read https://example.com/path.");
        assert_eq!(cands.len(), 1);
        assert!(!cands[0].raw.ends_with('.'));
    }

    #[test]
    fn ignores_bare_scheme_without_host() {
        let cands = extract("malformed https:// ok?");
        assert!(cands.is_empty());
    }

    #[test]
    fn ignores_text_without_scheme() {
        let cands = extract("plain text with example.com inside");
        assert!(cands.is_empty());
    }

    #[test]
    fn multiple_urls_in_text() {
        let cands = extract("see https://a.com and http://b.org now");
        assert_eq!(cands.len(), 2);
    }

    #[test]
    fn byte_offsets_match_original_text() {
        let text = "go to https://example.com/path today";
        let tokens = WhitespaceTokenizer.tokenize(text);
        let cands = UrlExtractor.extract(&tokens, text, &ctx(), &locale(), &resources());
        let cand = &cands[0];
        assert_eq!(&text[cand.start..cand.end], "https://example.com/path");
    }
}
