//! Index building and documentation generation.

use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::parser::FlowEntry;

/// Fast2flow index manifest containing flow entries and TF-IDF data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexManifest {
    /// Index format version.
    pub version: String,

    /// Index scope (tenant:team).
    pub scope: String,

    /// ISO 8601 timestamp of last update.
    pub last_updated: String,

    /// All indexed flow entries.
    pub flows: Vec<FlowEntry>,

    /// Term frequencies per document (doc_key -> term -> count).
    pub term_frequencies: HashMap<String, HashMap<String, u32>>,

    /// Document frequencies per term (term -> doc_count).
    pub document_frequencies: HashMap<String, u32>,
}

/// Builds an index manifest from flow entries.
///
/// Legacy doc-gen / inspection variant (heavy TF-IDF schema). Phase M1
/// endpoint indexing goes through `fast2flow_indexer::build_index`
/// instead — that produces the lean `IndexManifestV1` that the routing
/// layer actually loads.
pub fn build_index_manifest(entries: &[FlowEntry], tenant: &str, team: &str) -> IndexManifest {
    let mut term_frequencies: HashMap<String, HashMap<String, u32>> = HashMap::new();
    let mut document_frequencies: HashMap<String, u32> = HashMap::new();

    for entry in entries {
        let doc_key = format!("{}:{}", entry.pack_id, entry.flow_id);
        let mut doc_tf: HashMap<String, u32> = HashMap::new();

        for keyword in &entry.keywords {
            *doc_tf.entry(keyword.clone()).or_insert(0) += 1;
        }

        // Title words get a 2x boost.
        for word in entry.title.to_lowercase().split_whitespace() {
            let clean: String = word.chars().filter(|c| c.is_alphanumeric()).collect();
            if clean.len() >= 2 {
                *doc_tf.entry(clean).or_insert(0) += 2;
            }
        }

        for term in doc_tf.keys() {
            *document_frequencies.entry(term.clone()).or_insert(0) += 1;
        }

        term_frequencies.insert(doc_key, doc_tf);
    }

    IndexManifest {
        version: "1.0".to_string(),
        scope: format!("{tenant}:{team}"),
        last_updated: Utc::now().to_rfc3339(),
        flows: entries.to_vec(),
        term_frequencies,
        document_frequencies,
    }
}

/// Generates human-readable intent documentation in Markdown format.
///
/// The generated document includes:
/// - Summary table of all intents
/// - Detailed sections grouped by pack
/// - Usage examples
///
/// # Arguments
///
/// * `entries` - Slice of flow entries
/// * `tenant` - Tenant identifier
/// * `team` - Team identifier
///
/// # Returns
///
/// A Markdown-formatted string.
pub fn generate_intents_md(entries: &[FlowEntry], tenant: &str, team: &str) -> String {
    let mut md = String::new();

    md.push_str(&format!("# Intent Index: {}:{}\n\n", tenant, team));
    md.push_str(&format!(
        "Generated: {}\n\n",
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));
    md.push_str(&format!("Total flows: {}\n\n", entries.len()));

    md.push_str("## Available Intents\n\n");
    md.push_str("| Intent ID | Title | Type | Tags | Keywords |\n");
    md.push_str("|-----------|-------|------|------|----------|\n");

    // Group by pack
    let mut by_pack: HashMap<&str, Vec<&FlowEntry>> = HashMap::new();
    for entry in entries {
        by_pack.entry(&entry.pack_id).or_default().push(entry);
    }

    // Sort pack names for deterministic output
    let mut pack_names: Vec<_> = by_pack.keys().cloned().collect();
    pack_names.sort();

    for pack_id in &pack_names {
        let flows = by_pack.get(pack_id).unwrap();
        for flow in flows {
            let tags = if flow.tags.is_empty() {
                "-".to_string()
            } else {
                flow.tags.join(", ")
            };
            let keywords = if flow.keywords.is_empty() {
                "-".to_string()
            } else {
                flow.keywords
                    .iter()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            };

            md.push_str(&format!(
                "| `{}:{}` | {} | {} | {} | {} |\n",
                pack_id, flow.flow_id, flow.title, flow.flow_type, tags, keywords
            ));
        }
    }

    md.push_str("\n## Intent Details\n\n");

    for pack_id in &pack_names {
        let flows = by_pack.get(pack_id).unwrap();
        md.push_str(&format!("### Pack: {}\n\n", pack_id));

        for flow in flows {
            md.push_str(&format!("#### {} (`{}`)\n\n", flow.title, flow.flow_id));

            if !flow.description.is_empty() {
                md.push_str(&format!("{}\n\n", flow.description));
            }

            md.push_str(&format!("- **Type:** {}\n", flow.flow_type));
            md.push_str(&format!(
                "- **Intent ID:** `{}:{}`\n",
                pack_id, flow.flow_id
            ));

            if !flow.tags.is_empty() {
                md.push_str(&format!("- **Tags:** {}\n", flow.tags.join(", ")));
            }

            if !flow.keywords.is_empty() {
                md.push_str(&format!("- **Keywords:** {}\n", flow.keywords.join(", ")));
            }

            md.push('\n');
        }
    }

    md.push_str("---\n\n");
    md.push_str("## Usage with fast2flow\n\n");
    md.push_str("```bash\n");
    md.push_str("# Generate index from bundle\n");
    md.push_str(&format!(
        "greentic-fast2flow bundle index --bundle ./bundle --tenant {} --team {}\n\n",
        tenant, team
    ));
    md.push_str("# Copy index to fast2flow mount\n");
    md.push_str(&format!(
        "cp index.json /mnt/indexes/{}:{}/index.json\n",
        tenant, team
    ));
    md.push_str("```\n");

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<FlowEntry> {
        vec![
            FlowEntry {
                pack_id: "test-pack".to_string(),
                flow_id: "booking".to_string(),
                title: "Book Appointment".to_string(),
                description: "Schedule meetings and appointments".to_string(),
                tags: vec!["booking".to_string(), "calendar".to_string()],
                keywords: vec![
                    "book".to_string(),
                    "appointment".to_string(),
                    "schedule".to_string(),
                ],
                flow_type: "messaging".to_string(),
                file_path: "/path/to/flow.ygtc".to_string(),
            },
            FlowEntry {
                pack_id: "test-pack".to_string(),
                flow_id: "weather".to_string(),
                title: "Check Weather".to_string(),
                description: "Get weather forecasts".to_string(),
                tags: vec!["weather".to_string()],
                keywords: vec!["weather".to_string(), "forecast".to_string()],
                flow_type: "messaging".to_string(),
                file_path: "/path/to/weather.ygtc".to_string(),
            },
        ]
    }

    #[test]
    fn test_build_index_manifest() {
        let entries = sample_entries();
        let manifest = build_index_manifest(&entries, "demo", "default");

        assert_eq!(manifest.version, "1.0");
        assert_eq!(manifest.scope, "demo:default");
        assert_eq!(manifest.flows.len(), 2);
        assert!(!manifest.term_frequencies.is_empty());
        assert!(!manifest.document_frequencies.is_empty());
    }

    #[test]
    fn test_build_index_manifest_tf_idf() {
        let entries = sample_entries();
        let manifest = build_index_manifest(&entries, "demo", "default");

        // Check term frequencies exist for documents
        assert!(manifest.term_frequencies.contains_key("test-pack:booking"));
        assert!(manifest.term_frequencies.contains_key("test-pack:weather"));

        // Check document frequencies
        let booking_tf = manifest.term_frequencies.get("test-pack:booking").unwrap();
        assert!(booking_tf.contains_key("book"));
        assert!(booking_tf.contains_key("appointment"));
    }

    #[test]
    fn test_generate_intents_md() {
        let entries = sample_entries();
        let md = generate_intents_md(&entries, "demo", "default");

        assert!(md.contains("# Intent Index: demo:default"));
        assert!(md.contains("Total flows: 2"));
        assert!(md.contains("| `test-pack:booking`"));
        assert!(md.contains("### Pack: test-pack"));
        assert!(md.contains("#### Book Appointment"));
        assert!(md.contains("## Usage with fast2flow"));
    }

    #[test]
    fn test_generate_intents_md_empty() {
        let entries: Vec<FlowEntry> = vec![];
        let md = generate_intents_md(&entries, "demo", "default");

        assert!(md.contains("Total flows: 0"));
    }
}
