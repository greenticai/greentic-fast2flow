//! Gazetteer — longest-match lookup over a token map.
//!
//! Phase A keeps the storage simple: a `HashMap` keyed by lower-cased
//! whitespace-joined form. Matching is "longest first": the lookup walks
//! span sizes from `max_span_words` down to 1 and returns the first hit.
//! Cheap, deterministic, and avoids the complexity of a real trie until
//! we have a corpus large enough to need one.

use serde::{Deserialize, Serialize};

/// A gazetteer record. Phase A holds only the bare minimum.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GazetteerRecord {
    /// Canonical form (the normalized value an extractor reports).
    pub canonical: String,
    /// Optional ISO country / region code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// Optional entity kind hint (e.g. `city`, `country`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind_hint: Option<String>,
    /// Aliases that should resolve to the same canonical form.
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// Gazetteer container. Empty by default.
#[derive(Debug, Default, Clone)]
pub struct Gazetteer {
    records: std::collections::HashMap<String, GazetteerRecord>,
    max_span_words: usize,
}

impl Gazetteer {
    /// Insert a record (lower-cases the key + every alias). Tracks the
    /// longest entry's word count so longest-match can bound its search.
    pub fn insert(&mut self, key: impl Into<String>, record: GazetteerRecord) {
        let key = key.into().to_lowercase();
        self.max_span_words = self.max_span_words.max(word_count(&key));
        for alias in &record.aliases {
            let alias_lower = alias.to_lowercase();
            self.max_span_words = self.max_span_words.max(word_count(&alias_lower));
            self.records.insert(alias_lower, record.clone());
        }
        self.records.insert(key, record);
    }

    /// Exact-match lookup (case-insensitive). Multi-word keys must be
    /// joined by single spaces.
    pub fn lookup(&self, key: &str) -> Option<&GazetteerRecord> {
        self.records.get(&key.to_lowercase())
    }

    /// Longest matching entry starting at `words[0]`. `words` is a slice
    /// of already-lower-cased token surfaces. Returns the word count of
    /// the match plus the record.
    pub fn longest_match(&self, words: &[&str]) -> Option<(usize, &GazetteerRecord)> {
        if words.is_empty() || self.max_span_words == 0 {
            return None;
        }
        let max = words.len().min(self.max_span_words);
        for span in (1..=max).rev() {
            let key = words[..span].join(" ");
            if let Some(rec) = self.records.get(&key) {
                return Some((span, rec));
            }
        }
        None
    }

    /// Word count of the longest stored entry. Useful when bounding callers.
    pub fn max_span_words(&self) -> usize {
        self.max_span_words
    }

    /// Number of records (including aliases).
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the gazetteer is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Gazetteer {
        let mut g = Gazetteer::default();
        g.insert(
            "london",
            GazetteerRecord {
                canonical: "London".into(),
                country: Some("GB".into()),
                kind_hint: Some("city".into()),
                aliases: vec!["londres".into(), "londen".into()],
            },
        );
        g.insert(
            "new york",
            GazetteerRecord {
                canonical: "New York".into(),
                country: Some("US".into()),
                kind_hint: Some("city".into()),
                aliases: vec!["nyc".into(), "new york city".into()],
            },
        );
        g
    }

    #[test]
    fn aliases_resolve_to_canonical() {
        let g = fixture();
        assert_eq!(g.lookup("Londres").unwrap().canonical, "London");
        assert_eq!(g.lookup("LONDON").unwrap().country.as_deref(), Some("GB"));
    }

    #[test]
    fn longest_match_prefers_multi_word_entry() {
        let g = fixture();
        // "new york city" must beat "new york".
        let (span, rec) = g
            .longest_match(&["new", "york", "city", "weather"])
            .unwrap();
        assert_eq!(span, 3);
        assert_eq!(rec.canonical, "New York");
    }

    #[test]
    fn longest_match_falls_back_to_shorter() {
        let g = fixture();
        // No "new york skyline" — fall back to "new york".
        let (span, rec) = g.longest_match(&["new", "york", "skyline"]).unwrap();
        assert_eq!(span, 2);
        assert_eq!(rec.canonical, "New York");
    }

    #[test]
    fn longest_match_single_word() {
        let g = fixture();
        let (span, rec) = g.longest_match(&["london", "weather"]).unwrap();
        assert_eq!(span, 1);
        assert_eq!(rec.canonical, "London");
    }

    #[test]
    fn empty_gazetteer_returns_none() {
        let g = Gazetteer::default();
        assert!(g.longest_match(&["anything"]).is_none());
        assert_eq!(g.max_span_words(), 0);
    }

    #[test]
    fn max_span_tracks_longest_entry() {
        let g = fixture();
        // "new york city" is 3 words.
        assert_eq!(g.max_span_words(), 3);
    }
}
