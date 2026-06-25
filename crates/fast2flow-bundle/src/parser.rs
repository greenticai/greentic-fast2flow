//! Flow file parsing and metadata extraction.

use std::path::Path;

use anyhow::{Context, Result};
use fast2flow_contracts::FlowExecutionType;
use serde::{Deserialize, Serialize};

/// Flow metadata extracted from .ygtc files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowMeta {
    /// Unique flow identifier.
    pub id: String,

    /// Human-readable title.
    #[serde(default)]
    pub title: Option<String>,

    /// Flow description.
    #[serde(default)]
    pub description: Option<String>,

    /// Flow type (e.g., "messaging", "events").
    #[serde(default, rename = "type")]
    pub flow_type: Option<String>,

    /// Downstream execution path selected by Fast2Flow.
    #[serde(default, alias = "flow_execution_type")]
    pub execution_type: FlowExecutionType,

    /// Tags for categorization.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Starting node identifier.
    #[serde(default)]
    pub start: Option<String>,
}

/// Flow entry for the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowEntry {
    /// Pack identifier containing this flow.
    pub pack_id: String,

    /// Flow identifier.
    pub flow_id: String,

    /// Human-readable title.
    pub title: String,

    /// Flow description.
    pub description: String,

    /// Tags for categorization.
    pub tags: Vec<String>,

    /// Extracted keywords for matching.
    pub keywords: Vec<String>,

    /// Flow type.
    pub flow_type: String,

    /// Execution path selected after Fast2Flow dispatch.
    pub execution_type: FlowExecutionType,

    /// Original file path.
    pub file_path: String,
}

/// Parses a flow file and extracts metadata.
///
/// # Arguments
///
/// * `path` - Path to the .ygtc file
/// * `pack_id` - Pack identifier for this flow
///
/// # Returns
///
/// A `FlowEntry` containing parsed metadata and extracted keywords.
pub fn parse_flow_file(path: &Path, pack_id: &str) -> Result<FlowEntry> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let meta: FlowMeta = serde_yaml_bw::from_str(&content)
        .with_context(|| format!("Failed to parse YAML in {}", path.display()))?;

    let title = meta.title.unwrap_or_else(|| meta.id.clone());
    let description = meta.description.unwrap_or_default();
    let flow_type = meta.flow_type.unwrap_or_else(|| "unknown".to_string());

    // Extract keywords from title and description
    let keywords = extract_keywords(&title, &description, &meta.tags);

    Ok(FlowEntry {
        pack_id: pack_id.to_string(),
        flow_id: meta.id,
        title,
        description,
        tags: meta.tags,
        keywords,
        flow_type,
        execution_type: meta.execution_type,
        file_path: path.to_string_lossy().to_string(),
    })
}

/// Extracts keywords from text fields for indexing.
///
/// Keywords are extracted by:
/// 1. Combining title, description, and tags
/// 2. Tokenizing and lowercasing
/// 3. Filtering out stop words and short tokens
/// 4. Deduplicating
///
/// # Arguments
///
/// * `title` - Flow title
/// * `description` - Flow description
/// * `tags` - Flow tags
///
/// # Returns
///
/// A sorted, deduplicated list of keywords.
pub fn extract_keywords(title: &str, description: &str, tags: &[String]) -> Vec<String> {
    let mut keywords = Vec::new();

    // Extract words from title and description
    let text = format!("{} {}", title, description).to_lowercase();
    for word in text.split_whitespace() {
        let clean: String = word.chars().filter(|c| c.is_alphanumeric()).collect();
        if clean.len() >= 3 && !is_stop_word(&clean) {
            keywords.push(clean);
        }
    }

    // Add tags as keywords
    for tag in tags {
        keywords.push(tag.to_lowercase());
    }

    // Deduplicate
    keywords.sort();
    keywords.dedup();
    keywords
}

/// Checks if a word is a common stop word.
fn is_stop_word(word: &str) -> bool {
    const STOP_WORDS: &[&str] = &[
        "the", "and", "for", "are", "but", "not", "you", "all", "can", "had", "her", "was", "one",
        "our", "out", "has", "have", "been", "were", "said", "each", "she", "which", "their",
        "will", "would", "there", "could", "this", "from", "word", "what", "some", "with", "when",
        "then", "than", "them", "into", "your", "just", "over", "such", "demo", "flow", "test",
    ];
    STOP_WORDS.contains(&word)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_extract_keywords() {
        let keywords = extract_keywords(
            "Book Appointment",
            "Schedule a meeting or appointment",
            &["booking".to_string(), "calendar".to_string()],
        );

        assert!(keywords.contains(&"book".to_string()));
        assert!(keywords.contains(&"appointment".to_string()));
        assert!(keywords.contains(&"schedule".to_string()));
        assert!(keywords.contains(&"meeting".to_string()));
        assert!(keywords.contains(&"booking".to_string()));
        assert!(keywords.contains(&"calendar".to_string()));
    }

    #[test]
    fn test_extract_keywords_filters_stop_words() {
        let keywords = extract_keywords("The quick brown fox", "and the lazy dog", &[]);

        assert!(!keywords.contains(&"the".to_string()));
        assert!(!keywords.contains(&"and".to_string()));
        assert!(keywords.contains(&"quick".to_string()));
        assert!(keywords.contains(&"brown".to_string()));
        assert!(keywords.contains(&"lazy".to_string()));
    }

    #[test]
    fn test_parse_flow_file() {
        let dir = tempdir().unwrap();
        let flow_path = dir.path().join("test.ygtc");

        let content = r#"
id: test_flow
title: Test Flow Title
description: A test flow for unit tests
type: messaging
tags:
  - test
  - unit-test
start: begin
"#;
        fs::write(&flow_path, content).unwrap();

        let entry = parse_flow_file(&flow_path, "test-pack").unwrap();

        assert_eq!(entry.flow_id, "test_flow");
        assert_eq!(entry.pack_id, "test-pack");
        assert_eq!(entry.title, "Test Flow Title");
        assert_eq!(entry.flow_type, "messaging");
        assert!(entry.tags.contains(&"test".to_string()));
        assert!(entry.keywords.contains(&"unit-test".to_string()));
    }

    #[test]
    fn test_parse_flow_file_minimal() {
        let dir = tempdir().unwrap();
        let flow_path = dir.path().join("minimal.ygtc");

        let content = "id: minimal_flow\n";
        fs::write(&flow_path, content).unwrap();

        let entry = parse_flow_file(&flow_path, "pack").unwrap();

        assert_eq!(entry.flow_id, "minimal_flow");
        assert_eq!(entry.title, "minimal_flow"); // Falls back to id
        assert_eq!(entry.flow_type, "unknown"); // Default
    }
}
