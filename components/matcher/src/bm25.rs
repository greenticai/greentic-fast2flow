//! BM25 (Best Matching 25) implementation for fast text matching.

use std::collections::BTreeMap;

use greentic_types::cbor::canonical;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// BM25 algorithm parameters.
const BM25_K1: f64 = 1.5;
const BM25_B: f64 = 0.75;

/// Default confidence threshold.
const DEFAULT_THRESHOLD: f64 = 0.7;

/// Default maximum results.
const DEFAULT_MAX_RESULTS: usize = 5;

/// Flow entry from the index.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlowEntry {
    pub pack_id: String,
    pub flow_id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

/// The flow index structure.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlowIndex {
    pub version: String,
    pub flows: Vec<FlowEntry>,
    #[serde(default)]
    pub term_frequencies: BTreeMap<String, BTreeMap<String, u32>>,
    #[serde(default)]
    pub document_frequencies: BTreeMap<String, u32>,
}

/// Input for match operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchInput {
    pub query: String,
    pub index: FlowIndex,
    #[serde(default)]
    pub threshold: Option<f64>,
    #[serde(default)]
    pub max_results: Option<usize>,
}

/// A single match result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchResult {
    pub pack_id: String,
    pub flow_id: String,
    pub title: String,
    pub confidence: f64,
}

/// Match status enumeration.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchStatus {
    Match,
    Ambiguous,
    NoMatch,
    Timeout,
}

/// Output for match operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchOutput {
    pub status: MatchStatus,
    pub top_match: Option<MatchResult>,
    pub candidates: Vec<MatchResult>,
    pub latency_ms: u64,
}

/// Execute BM25 matching against the index.
pub fn match_query(input: Vec<u8>) -> Vec<u8> {
    let start = std::time::Instant::now();
    let result = do_match_query(&input, start);
    canonical::to_canonical_cbor_allow_floats(&result).unwrap_or_default()
}

fn do_match_query(input: &[u8], start: std::time::Instant) -> JsonValue {
    let input_value: JsonValue = match canonical::from_cbor(input) {
        Ok(v) => v,
        Err(e) => {
            return serde_json::json!({
                "error": format!("failed to parse input: {}", e)
            });
        }
    };

    let match_input: MatchInput = match serde_json::from_value(input_value) {
        Ok(v) => v,
        Err(e) => {
            return serde_json::json!({
                "error": format!("invalid input structure: {}", e)
            });
        }
    };

    let threshold = match_input.threshold.unwrap_or(DEFAULT_THRESHOLD);
    let max_results = match_input.max_results.unwrap_or(DEFAULT_MAX_RESULTS);

    // Tokenize the query
    let query_terms = tokenize(&match_input.query);
    if query_terms.is_empty() {
        return serde_json::to_value(MatchOutput {
            status: MatchStatus::NoMatch,
            top_match: None,
            candidates: vec![],
            latency_ms: start.elapsed().as_millis() as u64,
        })
        .unwrap_or_else(|_| serde_json::json!({}));
    }

    // Calculate BM25 scores for each document
    let scores = calculate_bm25_scores(
        &query_terms,
        &match_input.index.flows,
        &match_input.index.term_frequencies,
        &match_input.index.document_frequencies,
    );

    // Sort by score descending
    let mut scored_flows: Vec<(usize, f64)> = scores.into_iter().enumerate().collect();
    scored_flows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Normalize scores to confidence (0-1 range)
    let max_score = scored_flows.first().map(|(_, s)| *s).unwrap_or(1.0);
    let candidates: Vec<MatchResult> = scored_flows
        .iter()
        .take(max_results)
        .filter(|(_, score)| *score > 0.0)
        .map(|(idx, score)| {
            let flow = &match_input.index.flows[*idx];
            MatchResult {
                pack_id: flow.pack_id.clone(),
                flow_id: flow.flow_id.clone(),
                title: flow.title.clone(),
                confidence: if max_score > 0.0 {
                    score / max_score
                } else {
                    0.0
                },
            }
        })
        .collect();

    let latency_ms = start.elapsed().as_millis() as u64;

    // Determine status based on results
    let (status, top_match) = if candidates.is_empty() {
        (MatchStatus::NoMatch, None)
    } else if candidates[0].confidence >= threshold {
        // Check if there's ambiguity (second candidate close in score)
        if candidates.len() > 1 && candidates[1].confidence >= threshold * 0.9 {
            (MatchStatus::Ambiguous, Some(candidates[0].clone()))
        } else {
            (MatchStatus::Match, Some(candidates[0].clone()))
        }
    } else {
        (MatchStatus::NoMatch, None)
    };

    let output = MatchOutput {
        status,
        top_match,
        candidates,
        latency_ms,
    };

    serde_json::to_value(output).unwrap_or_else(|_| serde_json::json!({}))
}

/// Calculate BM25 scores for all documents.
fn calculate_bm25_scores(
    query_terms: &[String],
    flows: &[FlowEntry],
    term_frequencies: &BTreeMap<String, BTreeMap<String, u32>>,
    document_frequencies: &BTreeMap<String, u32>,
) -> Vec<f64> {
    let n = flows.len() as f64; // Total number of documents

    // Calculate average document length
    let total_length: usize = term_frequencies
        .values()
        .map(|tf| tf.values().sum::<u32>() as usize)
        .sum();
    let avgdl = if flows.is_empty() {
        1.0
    } else {
        total_length as f64 / n
    };

    let mut scores = vec![0.0; flows.len()];

    for (idx, flow) in flows.iter().enumerate() {
        let doc_id = format!("{}:{}", flow.pack_id, flow.flow_id);
        let doc_tf = term_frequencies.get(&doc_id);

        if let Some(tf_map) = doc_tf {
            let doc_length: u32 = tf_map.values().sum();

            for term in query_terms {
                let tf = tf_map.get(term).copied().unwrap_or(0) as f64;
                let df = document_frequencies.get(term).copied().unwrap_or(0) as f64;

                if tf > 0.0 {
                    // IDF calculation
                    let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();

                    // BM25 score for this term
                    let numerator = tf * (BM25_K1 + 1.0);
                    let denominator =
                        tf + BM25_K1 * (1.0 - BM25_B + BM25_B * (doc_length as f64 / avgdl));

                    scores[idx] += idf * (numerator / denominator);
                }
            }
        }
    }

    scores
}

/// Simple tokenizer that splits on whitespace and punctuation.
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        .filter(|s| !s.is_empty() && s.len() >= 2)
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize() {
        let tokens = tokenize("Book an appointment please!");
        assert!(tokens.contains(&"book".to_string()));
        assert!(tokens.contains(&"appointment".to_string()));
        assert!(tokens.contains(&"please".to_string()));
    }

    #[test]
    fn test_bm25_basic() {
        let flows = vec![
            FlowEntry {
                pack_id: "test".to_string(),
                flow_id: "booking".to_string(),
                title: "Book Appointment".to_string(),
                description: Some("Schedule meetings and appointments".to_string()),
                tags: vec!["booking".to_string()],
                keywords: vec!["schedule".to_string(), "meeting".to_string()],
            },
            FlowEntry {
                pack_id: "test".to_string(),
                flow_id: "weather".to_string(),
                title: "Check Weather".to_string(),
                description: Some("Get weather forecasts".to_string()),
                tags: vec!["weather".to_string()],
                keywords: vec!["forecast".to_string(), "temperature".to_string()],
            },
        ];

        let mut term_frequencies: BTreeMap<String, BTreeMap<String, u32>> = BTreeMap::new();
        let mut document_frequencies: BTreeMap<String, u32> = BTreeMap::new();

        // Build simple index
        for flow in &flows {
            let doc_id = format!("{}:{}", flow.pack_id, flow.flow_id);
            let mut tf: BTreeMap<String, u32> = BTreeMap::new();

            let text = format!(
                "{} {}",
                flow.title,
                flow.description.as_deref().unwrap_or("")
            );
            for term in tokenize(&text) {
                *tf.entry(term.clone()).or_insert(0) += 1;
            }

            for term in tf.keys() {
                *document_frequencies.entry(term.clone()).or_insert(0) += 1;
            }

            term_frequencies.insert(doc_id, tf);
        }

        let query_terms = tokenize("I want to book an appointment");
        let scores = calculate_bm25_scores(
            &query_terms,
            &flows,
            &term_frequencies,
            &document_frequencies,
        );

        // Booking flow should score higher for this query
        assert!(scores[0] > scores[1]);
    }
}
