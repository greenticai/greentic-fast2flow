//! Integration hooks for gtc setup/start integration.
//!
//! These functions are designed to be called by the Greentic toolchain
//! during bundle setup and runtime operations.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fast2flow_contracts::{endpoint_scope, FlowDoc, IndexManifestV1};
use tracing::{info, warn};

use crate::index::{build_index_manifest, generate_intents_md, IndexManifest};
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

/// Phase M1: result of indexing a bundle for a messaging endpoint.
///
/// Distinct from [`IndexResult`] because the endpoint flow emits the lean
/// runtime [`IndexManifestV1`] (consumed by `MountedIndexLookup::load`),
/// not the heavy TF-IDF [`IndexManifest`] used by the legacy doc-gen path.
#[derive(Debug)]
pub struct EndpointIndexResult {
    /// The runtime-loadable manifest. `entries` is empty when the corpus
    /// is empty (which is itself written to disk to evict stale routes).
    pub manifest: IndexManifestV1,
    /// `<indexes_root>/endpoint:{endpoint_id}/index.json`.
    pub index_path: PathBuf,
    /// `<indexes_root>/endpoint:{endpoint_id}/intents.md` when generated.
    pub intents_path: Option<PathBuf>,
    /// Number of flows admitted into the corpus (post linked-bundles filter).
    pub flow_count: usize,
}

/// Phase M1: index a bundle scoped to a single messaging endpoint.
///
/// `linked_bundle_pack_ids` is the endpoint's `linked_bundles[*]` — only
/// flows whose `pack_id` is in this set make it into the corpus. An empty
/// set keeps the corpus empty (fail-closed: an endpoint with no linked
/// bundles can route nothing). The on-disk index is written via
/// [`fast2flow_indexer::build_index`] under
/// `<indexes_root>/endpoint:{endpoint_id}/index.json` + `latest`, so
/// `MountedIndexLookup::load` resolves it at routing time.
///
/// Empty corpora STILL write an empty manifest atomically — that is the
/// link/unlink eviction story. Without it, unlinking the last bundle
/// leaves the previous index on disk and stale routes keep dispatching.
pub fn index_bundle_for_endpoint(
    bundle_path: &Path,
    indexes_root: &Path,
    endpoint_id: &str,
    linked_bundle_pack_ids: &HashSet<String>,
    now_unix_ms: u64,
    generate_docs: bool,
) -> Result<EndpointIndexResult> {
    index_bundle_for_endpoint_internal(
        bundle_path,
        indexes_root,
        endpoint_id,
        linked_bundle_pack_ids,
        now_unix_ms,
        generate_docs,
        false,
    )
}

/// Phase M1: re-index for an endpoint on `link-bundle` / `unlink-bundle` /
/// bundle revision-warm. Verbose variant of [`index_bundle_for_endpoint`].
pub fn reindex_for_endpoint_on_pack_change(
    bundle_path: &Path,
    indexes_root: &Path,
    endpoint_id: &str,
    linked_bundle_pack_ids: &HashSet<String>,
    now_unix_ms: u64,
) -> Result<EndpointIndexResult> {
    index_bundle_for_endpoint_internal(
        bundle_path,
        indexes_root,
        endpoint_id,
        linked_bundle_pack_ids,
        now_unix_ms,
        true,
        true,
    )
}

fn index_bundle_for_endpoint_internal(
    bundle_path: &Path,
    indexes_root: &Path,
    endpoint_id: &str,
    linked_bundle_pack_ids: &HashSet<String>,
    now_unix_ms: u64,
    generate_docs: bool,
    verbose: bool,
) -> Result<EndpointIndexResult> {
    let all_entries = scan_bundle_verbose(bundle_path, verbose)?;
    let entries: Vec<FlowEntry> = all_entries
        .into_iter()
        .filter(|flow| linked_bundle_pack_ids.contains(&flow.pack_id))
        .collect();

    let scope = endpoint_scope(endpoint_id);
    let flow_count = entries.len();

    if flow_count == 0 {
        warn!(
            bundle = %bundle_path.display(),
            scope = %scope,
            linked_bundle_count = linked_bundle_pack_ids.len(),
            "no flows in endpoint corpus; writing empty manifest to evict stale routes",
        );
    }

    let flows: Vec<FlowDoc> = entries
        .iter()
        .map(|entry| FlowDoc {
            id: entry.flow_id.clone(),
            pack_id: entry.pack_id.clone(),
            // Convention: `<pack_id>/<flow_id>` mirrors the dispatch
            // target style used elsewhere (see refund_flow tests).
            target: format!("{}/{}", entry.pack_id, entry.flow_id),
            title: entry.title.clone(),
            tags: entry.tags.clone(),
            // FlowEntry does not retain node ids; downstream scoring
            // does not need them.
            node_ids: Vec::new(),
        })
        .collect();

    let manifest = fast2flow_indexer::build_index(&scope, &flows, indexes_root, now_unix_ms)
        .with_context(|| {
            format!(
                "failed building endpoint index at {}/{scope}",
                indexes_root.display()
            )
        })?;

    let scope_dir = indexes_root.join(&scope);
    let index_path = scope_dir.join("index.json");

    if verbose {
        info!(path = %index_path.display(), "wrote endpoint index");
    }

    let intents_path = if generate_docs && flow_count > 0 {
        // intents.md still groups by pack and renders identically. Reuse the
        // tenant/team-flavoured generator by passing the endpoint id in both
        // slots so the header reads `endpoint:<id>:<id>`. A dedicated
        // generator is out of scope for M1.3.
        let intents_md = generate_intents_md(&entries, endpoint_id, endpoint_id);
        let path = scope_dir.join("intents.md");
        std::fs::write(&path, &intents_md)
            .with_context(|| format!("Failed to write {}", path.display()))?;

        if verbose {
            info!(path = %path.display(), "wrote intents");
        }

        Some(path)
    } else {
        None
    };

    info!(
        flow_count,
        scope = %scope,
        index_key = %format!("fast2flow:index:{scope}"),
        index_path = %index_path.display(),
        "bundle indexed for endpoint",
    );

    Ok(EndpointIndexResult {
        manifest,
        index_path,
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
        let indexes_root = tempdir().unwrap();

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
            indexes_root.path(),
            "teams-legal",
            &linked,
            0,
            false,
        )
        .unwrap();

        assert_eq!(result.flow_count, 1, "only legal-pack flows must survive");
        assert_eq!(result.manifest.scope, "endpoint:teams-legal");
        let flow_ids: Vec<&str> = result
            .manifest
            .entries
            .iter()
            .map(|e| e.flow_id.as_str())
            .collect();
        assert_eq!(flow_ids, vec!["nda"]);

        // F2 regression: the writer must produce a runtime-loadable index
        // under `<root>/<scope>/`, so `MountedIndexLookup::load` can find it.
        let scope_dir = indexes_root.path().join("endpoint:teams-legal");
        assert!(scope_dir.join("index.json").exists());
        assert!(scope_dir.join("latest").exists());
    }

    #[test]
    fn index_bundle_for_endpoint_empty_linked_set_writes_empty_manifest() {
        // F1 regression: empty linked-set must still produce an on-disk
        // manifest so a follow-up `load_latest` sees the eviction. Returning
        // early would leave the previous index in place after `unlink-bundle`
        // and stale routes would keep dispatching.
        let bundle_dir = tempdir().unwrap();
        let indexes_root = tempdir().unwrap();
        write_flow(bundle_dir.path(), "any-pack", "any", "Any", "any");

        let linked: HashSet<String> = HashSet::new();
        let result = index_bundle_for_endpoint(
            bundle_dir.path(),
            indexes_root.path(),
            "teams-empty",
            &linked,
            0,
            false,
        )
        .unwrap();

        assert_eq!(result.flow_count, 0);
        assert_eq!(result.manifest.scope, "endpoint:teams-empty");
        assert!(result.manifest.entries.is_empty());
        assert!(
            result.index_path.exists(),
            "empty corpus must still write the manifest atomically",
        );

        let scope_dir = indexes_root.path().join("endpoint:teams-empty");
        assert!(scope_dir.join("latest").exists());
    }

    #[test]
    fn empty_reindex_evicts_previous_endpoint_routes() {
        // F1 regression: seed an old corpus, reindex with an empty linked
        // set, then load via `load_latest` and confirm zero entries —
        // proving that downstream routing sees the eviction.
        let bundle_dir = tempdir().unwrap();
        let indexes_root = tempdir().unwrap();

        write_flow(bundle_dir.path(), "pack-a", "alpha", "Alpha", "alpha");

        let mut linked: HashSet<String> = HashSet::new();
        linked.insert("pack-a".to_string());

        let seeded = index_bundle_for_endpoint(
            bundle_dir.path(),
            indexes_root.path(),
            "teams-x",
            &linked,
            0,
            false,
        )
        .unwrap();
        assert_eq!(seeded.flow_count, 1);

        let store = fast2flow_indexer::load_latest(indexes_root.path(), "endpoint:teams-x")
            .expect("seeded index must load");
        assert_eq!(store.manifest().entries.len(), 1);

        // Unlink the last bundle: empty set → corpus drops.
        let evicted = reindex_for_endpoint_on_pack_change(
            bundle_dir.path(),
            indexes_root.path(),
            "teams-x",
            &HashSet::new(),
            0,
        )
        .unwrap();
        assert_eq!(evicted.flow_count, 0);

        let store_after = fast2flow_indexer::load_latest(indexes_root.path(), "endpoint:teams-x")
            .expect("manifest must still load after eviction");
        assert!(
            store_after.manifest().entries.is_empty(),
            "stale alpha flow must NOT survive the eviction reindex",
        );
    }

    #[test]
    fn reindex_for_endpoint_on_pack_change_writes_verbose() {
        let bundle_dir = tempdir().unwrap();
        let indexes_root = tempdir().unwrap();
        write_flow(bundle_dir.path(), "pack", "flow", "Title", "tag");
        let mut linked = HashSet::new();
        linked.insert("pack".to_string());

        let result = reindex_for_endpoint_on_pack_change(
            bundle_dir.path(),
            indexes_root.path(),
            "teams-x",
            &linked,
            0,
        )
        .unwrap();

        assert_eq!(result.flow_count, 1);
        assert_eq!(result.manifest.scope, "endpoint:teams-x");
        assert!(result.index_path.exists());
        assert!(result.intents_path.as_ref().unwrap().exists());
    }

    #[test]
    fn endpoint_index_loads_via_load_latest_round_trip() {
        // F2 regression: end-to-end round trip — write via the hook,
        // load via the runtime-side primitive, prove the lean
        // `IndexManifestV1` shape is what's on disk.
        let bundle_dir = tempdir().unwrap();
        let indexes_root = tempdir().unwrap();
        write_flow(bundle_dir.path(), "pack-z", "z-flow", "Z Flow", "z-tag");
        let mut linked = HashSet::new();
        linked.insert("pack-z".to_string());

        let _ = index_bundle_for_endpoint(
            bundle_dir.path(),
            indexes_root.path(),
            "teams-z",
            &linked,
            0,
            false,
        )
        .unwrap();

        let store = fast2flow_indexer::load_latest(indexes_root.path(), "endpoint:teams-z")
            .expect("endpoint index must be loadable by routing-side primitive");
        let manifest = store.manifest();
        assert_eq!(manifest.scope, "endpoint:teams-z");
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].flow_id, "z-flow");
        assert_eq!(manifest.entries[0].pack_id, "pack-z");
        assert_eq!(manifest.entries[0].target, "pack-z/z-flow");
    }
}
