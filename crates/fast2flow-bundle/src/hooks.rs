//! Integration hooks for gtc setup/start integration.
//!
//! These functions are designed to be called by the Greentic toolchain
//! during bundle setup and runtime operations.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::{info, warn};

use crate::index::{
    build_index_manifest, build_index_manifest_for_endpoint, generate_intents_md, IndexManifest,
};
use crate::parser::FlowEntry;
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

/// Phase M1: index a bundle scoped to a single messaging endpoint.
///
/// `linked_bundle_pack_ids` is the endpoint's `linked_bundles[*]` — only
/// flows whose `pack_id` is in this set make it into the corpus. An empty
/// set keeps the corpus empty (fail-closed: an endpoint with no linked
/// bundles can route nothing). The on-disk index is written under
/// `endpoint:{endpoint_id}` per the routing scope contract.
pub fn index_bundle_for_endpoint(
    bundle_path: &Path,
    output_path: &Path,
    endpoint_id: &str,
    linked_bundle_pack_ids: &HashSet<String>,
    generate_docs: bool,
) -> Result<IndexResult> {
    index_bundle_for_endpoint_internal(
        bundle_path,
        output_path,
        endpoint_id,
        linked_bundle_pack_ids,
        generate_docs,
        false,
    )
}

/// Phase M1: re-index for an endpoint on `link-bundle` / `unlink-bundle` /
/// bundle revision-warm. Verbose variant of [`index_bundle_for_endpoint`].
pub fn reindex_for_endpoint_on_pack_change(
    bundle_path: &Path,
    output_path: &Path,
    endpoint_id: &str,
    linked_bundle_pack_ids: &HashSet<String>,
) -> Result<IndexResult> {
    index_bundle_for_endpoint_internal(
        bundle_path,
        output_path,
        endpoint_id,
        linked_bundle_pack_ids,
        true,
        true,
    )
}

fn index_bundle_for_endpoint_internal(
    bundle_path: &Path,
    output_path: &Path,
    endpoint_id: &str,
    linked_bundle_pack_ids: &HashSet<String>,
    generate_docs: bool,
    verbose: bool,
) -> Result<IndexResult> {
    let all_entries = scan_bundle_verbose(bundle_path, verbose)?;
    let entries: Vec<FlowEntry> = all_entries
        .into_iter()
        .filter(|flow| linked_bundle_pack_ids.contains(&flow.pack_id))
        .collect();

    let scope_label = format!("endpoint:{endpoint_id}");

    if entries.is_empty() {
        warn!(
            bundle = %bundle_path.display(),
            scope = %scope_label,
            linked_bundle_count = linked_bundle_pack_ids.len(),
            "no flows in endpoint corpus",
        );
        return Ok(IndexResult {
            manifest: build_index_manifest_for_endpoint(&[], endpoint_id),
            index_path: None,
            intents_path: None,
            flow_count: 0,
        });
    }

    let manifest = build_index_manifest_for_endpoint(&entries, endpoint_id);

    std::fs::create_dir_all(output_path).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            output_path.display()
        )
    })?;

    let index_path = output_path.join("index.json");
    let index_json =
        serde_json::to_string_pretty(&manifest).context("Failed to serialize index manifest")?;
    std::fs::write(&index_path, &index_json)
        .with_context(|| format!("Failed to write {}", index_path.display()))?;

    if verbose {
        info!(path = %index_path.display(), "wrote endpoint index");
    }

    let intents_path = if generate_docs {
        // intents.md still groups by pack and renders identically. Reuse the
        // tenant/team-flavoured generator by passing the endpoint id in both
        // slots so the header reads `endpoint:<id>:<id>`. A dedicated
        // generator is out of scope for M1.3.
        let intents_md = generate_intents_md(&entries, endpoint_id, endpoint_id);
        let path = output_path.join("intents.md");
        std::fs::write(&path, &intents_md)
            .with_context(|| format!("Failed to write {}", path.display()))?;

        if verbose {
            info!(path = %path.display(), "wrote intents");
        }

        Some(path)
    } else {
        None
    };

    let flow_count = entries.len();

    info!(
        flow_count,
        scope = %scope_label,
        index_key = %format!("fast2flow:index:endpoint:{endpoint_id}"),
        index_path = %index_path.display(),
        "bundle indexed for endpoint",
    );

    Ok(IndexResult {
        manifest,
        index_path: Some(index_path),
        intents_path,
        flow_count,
    })
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
        warn!(bundle = %bundle_path.display(), scope = %format!("{tenant}:{team}"), "no flows found in bundle");
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
        info!(path = %index_path.display(), "wrote index");
    }

    // Generate intents.md if requested
    let intents_path = if generate_docs {
        let intents_md = generate_intents_md(&entries, tenant, team);
        let path = output_path.join("intents.md");
        std::fs::write(&path, &intents_md)
            .with_context(|| format!("Failed to write {}", path.display()))?;

        if verbose {
            info!(path = %path.display(), "wrote intents");
        }

        Some(path)
    } else {
        None
    };

    let flow_count = entries.len();

    info!(
        flow_count,
        scope = %format!("{tenant}:{team}"),
        index_key = %format!("fast2flow:index:{tenant}:{team}"),
        index_path = %index_path.display(),
        "bundle indexed"
    );

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

    fn write_flow(dir: &Path, pack_id: &str, flow_id: &str, title: &str, tag: &str) {
        let flow_dir = dir.join(format!("packs/{pack_id}/flows"));
        fs::create_dir_all(&flow_dir).unwrap();
        fs::write(
            flow_dir.join(format!("{flow_id}.ygtc")),
            format!(
                "id: {flow_id}\ntitle: {title}\ndescription: ignored\ntype: messaging\ntags:\n  - {tag}\n",
            ),
        )
        .unwrap();
    }

    #[test]
    fn index_bundle_for_endpoint_corpus_is_union_of_linked_bundles_only() {
        let bundle_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();

        write_flow(bundle_dir.path(), "legal-pack", "nda", "Legal NDA", "legal");
        write_flow(
            bundle_dir.path(),
            "accounting-pack",
            "expense",
            "Accounting Expense",
            "accounting",
        );

        let mut linked: HashSet<String> = HashSet::new();
        linked.insert("legal-pack".to_string());

        let result = index_bundle_for_endpoint(
            bundle_dir.path(),
            output_dir.path(),
            "teams-legal",
            &linked,
            false,
        )
        .unwrap();

        assert_eq!(result.flow_count, 1, "only legal-pack flows must survive");
        assert_eq!(result.manifest.scope, "endpoint:teams-legal");
        let flow_ids: Vec<&str> = result
            .manifest
            .flows
            .iter()
            .map(|f| f.flow_id.as_str())
            .collect();
        assert_eq!(flow_ids, vec!["nda"]);
    }

    #[test]
    fn index_bundle_for_endpoint_empty_linked_set_yields_empty_corpus() {
        let bundle_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();
        write_flow(bundle_dir.path(), "any-pack", "any", "Any", "any");

        let linked: HashSet<String> = HashSet::new();
        let result = index_bundle_for_endpoint(
            bundle_dir.path(),
            output_dir.path(),
            "teams-empty",
            &linked,
            false,
        )
        .unwrap();

        assert_eq!(result.flow_count, 0);
        assert!(result.index_path.is_none(), "no on-disk write when empty");
        assert_eq!(result.manifest.scope, "endpoint:teams-empty");
    }

    #[test]
    fn reindex_for_endpoint_on_pack_change_writes_verbose() {
        let bundle_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();
        write_flow(bundle_dir.path(), "pack", "flow", "Title", "tag");
        let mut linked = HashSet::new();
        linked.insert("pack".to_string());

        let result = reindex_for_endpoint_on_pack_change(
            bundle_dir.path(),
            output_dir.path(),
            "teams-x",
            &linked,
        )
        .unwrap();

        assert_eq!(result.flow_count, 1);
        assert_eq!(result.manifest.scope, "endpoint:teams-x");
        assert!(result.index_path.as_ref().unwrap().exists());
        assert!(result.intents_path.as_ref().unwrap().exists());
    }
}
