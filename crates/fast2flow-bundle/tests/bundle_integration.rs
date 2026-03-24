//! Integration tests for bundle scanning and indexing.

use std::path::PathBuf;

use fast2flow_bundle::hooks::{index_bundle_after_setup, validate_bundle};
use fast2flow_bundle::{build_index_manifest, generate_intents_md, scan_bundle};

fn sample_bundle_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/sample-bundle")
}

#[test]
fn scan_bundle_discovers_all_flows() {
    let entries = scan_bundle(&sample_bundle_path()).unwrap();

    assert_eq!(entries.len(), 5, "Expected 5 flows in sample bundle");

    let flow_ids: Vec<&str> = entries.iter().map(|e| e.flow_id.as_str()).collect();
    assert!(flow_ids.contains(&"refund_request"));
    assert!(flow_ids.contains(&"shipping_status"));
    assert!(flow_ids.contains(&"faq_lookup"));
    assert!(flow_ids.contains(&"leave_request"));
    assert!(flow_ids.contains(&"book_appointment"));
}

#[test]
fn scan_bundle_extracts_pack_ids() {
    let entries = scan_bundle(&sample_bundle_path()).unwrap();

    let support_flows: Vec<_> = entries
        .iter()
        .filter(|e| e.pack_id == "support-pack")
        .collect();
    assert_eq!(support_flows.len(), 3, "support-pack should have 3 flows");

    let hr_flows: Vec<_> = entries
        .iter()
        .filter(|e| e.pack_id == "hr-pack")
        .collect();
    assert_eq!(hr_flows.len(), 2, "hr-pack should have 2 flows");
}

#[test]
fn scan_bundle_extracts_metadata() {
    let entries = scan_bundle(&sample_bundle_path()).unwrap();

    let refund = entries
        .iter()
        .find(|e| e.flow_id == "refund_request")
        .expect("refund_request should exist");

    assert_eq!(refund.title, "Process Refund Request");
    assert_eq!(refund.flow_type, "messaging");
    assert!(refund.tags.contains(&"refund".to_string()));
    assert!(refund.tags.contains(&"payment".to_string()));
    assert!(!refund.description.is_empty());
}

#[test]
fn scan_bundle_extracts_keywords() {
    let entries = scan_bundle(&sample_bundle_path()).unwrap();

    let refund = entries
        .iter()
        .find(|e| e.flow_id == "refund_request")
        .expect("refund_request should exist");

    // Should have keywords from title + description + tags
    assert!(
        !refund.keywords.is_empty(),
        "Keywords should be extracted from metadata"
    );
    assert!(
        refund.keywords.contains(&"refund".to_string()),
        "Should contain 'refund' keyword from tags"
    );
}

#[test]
fn build_index_manifest_from_bundle() {
    let entries = scan_bundle(&sample_bundle_path()).unwrap();
    let manifest = build_index_manifest(&entries, "demo", "default");

    assert_eq!(manifest.version, "1.0");
    assert_eq!(manifest.scope, "demo:default");
    assert_eq!(manifest.flows.len(), 5);
    assert!(!manifest.term_frequencies.is_empty());
    assert!(!manifest.document_frequencies.is_empty());

    // Verify TF-IDF entries exist for all flows
    assert!(manifest
        .term_frequencies
        .contains_key("support-pack:refund_request"));
    assert!(manifest
        .term_frequencies
        .contains_key("hr-pack:book_appointment"));
}

#[test]
fn build_index_manifest_title_boosting() {
    let entries = scan_bundle(&sample_bundle_path()).unwrap();
    let manifest = build_index_manifest(&entries, "demo", "default");

    // Title words should have higher TF (2x boost)
    let refund_tf = manifest
        .term_frequencies
        .get("support-pack:refund_request")
        .unwrap();

    // "refund" appears in both title and keywords, so should have high TF
    let refund_count = refund_tf.get("refund").copied().unwrap_or(0);
    assert!(
        refund_count >= 2,
        "Title word 'refund' should have boosted TF, got {}",
        refund_count
    );
}

#[test]
fn generate_intents_md_from_bundle() {
    let entries = scan_bundle(&sample_bundle_path()).unwrap();
    let md = generate_intents_md(&entries, "demo", "default");

    assert!(md.contains("# Intent Index: demo:default"));
    assert!(md.contains("Total flows: 5"));

    // Check all packs are mentioned
    assert!(md.contains("### Pack: support-pack"));
    assert!(md.contains("### Pack: hr-pack"));

    // Check flow details
    assert!(md.contains("Process Refund Request"));
    assert!(md.contains("Book Meeting Room"));
    assert!(md.contains("Submit Leave Request"));

    // Check usage instructions
    assert!(md.contains("greentic-fast2flow bundle index"));
}

#[test]
fn index_bundle_after_setup_writes_files() {
    let output_dir = tempfile::tempdir().unwrap();

    let result = index_bundle_after_setup(
        &sample_bundle_path(),
        output_dir.path(),
        "test-tenant",
        "team-a",
        true,
    )
    .unwrap();

    assert_eq!(result.flow_count, 5);

    // Verify index.json
    let index_path = result.index_path.unwrap();
    assert!(index_path.exists());
    let index_json = std::fs::read_to_string(&index_path).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&index_json).unwrap();
    assert_eq!(manifest["scope"], "test-tenant:team-a");
    assert_eq!(manifest["flows"].as_array().unwrap().len(), 5);

    // Verify intents.md
    let intents_path = result.intents_path.unwrap();
    assert!(intents_path.exists());
    let intents_md = std::fs::read_to_string(&intents_path).unwrap();
    assert!(intents_md.contains("# Intent Index: test-tenant:team-a"));
}

#[test]
fn index_bundle_without_docs() {
    let output_dir = tempfile::tempdir().unwrap();

    let result = index_bundle_after_setup(
        &sample_bundle_path(),
        output_dir.path(),
        "demo",
        "default",
        false,
    )
    .unwrap();

    assert_eq!(result.flow_count, 5);
    assert!(result.index_path.is_some());
    assert!(result.intents_path.is_none(), "Should not generate intents.md when generate_docs=false");
}

#[test]
fn validate_bundle_returns_true_for_valid_bundle() {
    assert!(validate_bundle(&sample_bundle_path()));
}

#[test]
fn validate_bundle_returns_false_for_empty_dir() {
    let empty_dir = tempfile::tempdir().unwrap();
    assert!(!validate_bundle(empty_dir.path()));
}

#[test]
fn index_manifest_is_deserializable() {
    let output_dir = tempfile::tempdir().unwrap();

    index_bundle_after_setup(
        &sample_bundle_path(),
        output_dir.path(),
        "demo",
        "default",
        false,
    )
    .unwrap();

    // Read back and deserialize the index
    let index_path = output_dir.path().join("index.json");
    let index_json = std::fs::read_to_string(&index_path).unwrap();
    let manifest: fast2flow_bundle::IndexManifest =
        serde_json::from_str(&index_json).unwrap();

    assert_eq!(manifest.scope, "demo:default");
    assert_eq!(manifest.flows.len(), 5);
    assert!(!manifest.term_frequencies.is_empty());
    assert!(!manifest.document_frequencies.is_empty());
}
