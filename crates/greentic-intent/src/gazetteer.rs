//! Gazetteer — longest-match lookup over a token trie.
//!
//! Phase A: minimal struct with insert + lookup stubs. The actual trie
//! lands in the next milestone.

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
    /// Lower-cased lookup keys → record. Phase A uses a plain map; trie
    /// replacement lands when the location extractor is implemented.
    records: std::collections::HashMap<String, GazetteerRecord>,
}

impl Gazetteer {
    /// Insert a record (lower-cases the key + every alias).
    pub fn insert(&mut self, key: impl Into<String>, record: GazetteerRecord) {
        let key = key.into().to_lowercase();
        for alias in &record.aliases {
            self.records.insert(alias.to_lowercase(), record.clone());
        }
        self.records.insert(key, record);
    }

    /// Exact-match lookup (case-insensitive).
    pub fn lookup(&self, key: &str) -> Option<&GazetteerRecord> {
        self.records.get(&key.to_lowercase())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_resolve_to_canonical() {
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
        assert_eq!(g.lookup("Londres").unwrap().canonical, "London");
        assert_eq!(g.lookup("LONDON").unwrap().country.as_deref(), Some("GB"));
    }
}
