//! Integration hooks for gtc setup/start integration.
//!
//! These functions are designed to be called by the Greentic toolchain
//! during bundle setup and runtime operations.

use std::path::Path;

use anyhow::{Context, Result};

use crate::index::{build_index_manifest, generate_intents_md, IndexManifest};
use crate::scanner::scan_bundle_verbose;

/// Result of an indexing operation.
#[derive(Debug)]
pub struct IndexResult {
    /// The generated index manifest.
    pub manifest: IndexManifest,

    /// Path to the written index.json file (if written).
    pub index_path: Option<std::path::PathBuf>,

    /// Path to the written intents.md file (if written).
    pub intents_path: Option<std::path::PathBuf>,

    /// Number of flows indexed.
    pub flow_count: usize,
}

/// Indexes a bundle after setup completion.
///
/// This function is designed to be called by `gtc setup` after provider
/// configuration is complete. It:
/// 1. Scans the bundle for flow definitions
/// 2. Builds a TF-IDF index
/// 3. Writes index.json to the output directory
/// 4. Optionally generates intents.md documentation
///
/// # Arguments
///
/// * `bundle_path` - Path to the bundle root
/// * `output_path` - Directory to write index files
/// * `tenant` - Tenant identifier
/// * `team` - Team identifier
/// * `generate_docs` - Whether to generate intents.md
///
/// # Returns
///
/// An `IndexResult` containing the generated artifacts.
pub fn index_bundle_after_setup(
    bundle_path: &Path,
    output_path: &Path,
    tenant: &str,
    team: &str,
    generate_docs: bool,
) -> Result<IndexResult> {
    index_bundle_internal(bundle_path, output_path, tenant, team, generate_docs, false)
}

/// Re-indexes a bundle on pack changes.
///
/// This function is designed to be called by `gtc start` when packs are
/// deployed or updated. It follows the same process as `index_bundle_after_setup`
/// but with verbose output enabled.
///
/// # Arguments
///
/// * `bundle_path` - Path to the bundle root
/// * `output_path` - Directory to write index files
/// * `tenant` - Tenant identifier
/// * `team` - Team identifier
///
/// # Returns
///
/// An `IndexResult` containing the generated artifacts.
pub fn reindex_on_pack_change(
    bundle_path: &Path,
    output_path: &Path,
    tenant: &str,
    team: &str,
) -> Result<IndexResult> {
    index_bundle_internal(bundle_path, output_path, tenant, team, true, true)
}

/// Internal implementation for bundle indexing.
fn index_bundle_internal(
    bundle_path: &Path,
    output_path: &Path,
    tenant: &str,
    team: &str,
    generate_docs: bool,
    verbose: bool,
) -> Result<IndexResult> {
    // Scan bundle for flows
    let entries = scan_bundle_verbose(bundle_path, verbose)?;

    if entries.is_empty() {
        if verbose {
            eprintln!("No flows found in bundle");
        }
        return Ok(IndexResult {
            manifest: build_index_manifest(&[], tenant, team),
            index_path: None,
            intents_path: None,
            flow_count: 0,
        });
    }

    // Build index manifest
    let manifest = build_index_manifest(&entries, tenant, team);

    // Ensure output directory exists
    std::fs::create_dir_all(output_path).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_path.display()
        )
    })?;

    // Write index.json
    let index_path = output_path.join("index.json");
    let index_json =
        serde_json::to_string_pretty(&manifest).context("Failed to serialize index manifest")?;
    std::fs::write(&index_path, &index_json)
        .with_context(|| format!("Failed to write {}", index_path.display()))?;

    if verbose {
        eprintln!("Wrote index: {}", index_path.display());
    }

    // Generate intents.md if requested
    let intents_path = if generate_docs {
        let intents_md = generate_intents_md(&entries, tenant, team);
        let path = output_path.join("intents.md");
        std::fs::write(&path, &intents_md)
            .with_context(|| format!("Failed to write {}", path.display()))?;

        if verbose {
            eprintln!("Wrote intents: {}", path.display());
        }

        Some(path)
    } else {
        None
    };

    let flow_count = entries.len();

    if verbose {
        eprintln!("\nIndex summary:");
        eprintln!("  - Flows indexed: {}", flow_count);
        eprintln!("  - Scope: {}:{}", tenant, team);
        eprintln!("  - Index key: fast2flow:index:{}:{}", tenant, team);
    }

    Ok(IndexResult {
        manifest,
        index_path: Some(index_path),
        intents_path,
        flow_count,
    })
}

/// Validates that a bundle contains indexable flows.
///
/// This is a lightweight check that can be called before full indexing
/// to verify the bundle structure is valid.
///
/// # Arguments
///
/// * `bundle_path` - Path to the bundle root
///
/// # Returns
///
/// `true` if the bundle contains at least one parseable flow.
pub fn validate_bundle(bundle_path: &Path) -> bool {
    crate::scanner::find_flow_files(bundle_path)
        .map(|files| !files.is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_index_bundle_after_setup() {
        let bundle_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();

        // Create a test flow
        let flow_dir = bundle_dir.path().join("packs/test-pack/flows");
        fs::create_dir_all(&flow_dir).unwrap();
        fs::write(
            flow_dir.join("test.ygtc"),
            r#"
id: test_flow
title: Test Flow
description: A test flow
type: messaging
tags:
  - test
"#,
        )
        .unwrap();

        let result = index_bundle_after_setup(
            bundle_dir.path(),
            output_dir.path(),
            "demo",
            "default",
            true,
        )
        .unwrap();

        assert_eq!(result.flow_count, 1);
        assert!(result.index_path.is_some());
        assert!(result.intents_path.is_some());

        // Verify files were written
        let index_path = result.index_path.unwrap();
        assert!(index_path.exists());

        let intents_path = result.intents_path.unwrap();
        assert!(intents_path.exists());
    }

    #[test]
    fn test_validate_bundle() {
        let bundle_dir = tempdir().unwrap();

        // Empty bundle
        assert!(!validate_bundle(bundle_dir.path()));

        // Create a flow
        let flow_dir = bundle_dir.path().join("packs/test-pack/flows");
        fs::create_dir_all(&flow_dir).unwrap();
        fs::write(flow_dir.join("test.ygtc"), "id: test").unwrap();

        assert!(validate_bundle(bundle_dir.path()));
    }
}
