//! Bundle directory scanning and flow discovery.

use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

use crate::parser::{parse_flow_file, FlowEntry};

/// Scans a bundle directory and returns all parsed flow entries.
///
/// # Arguments
///
/// * `bundle_path` - Path to the bundle root directory
///
/// # Returns
///
/// A vector of successfully parsed flow entries. Unparseable flows are skipped
/// with a warning logged to stderr.
pub fn scan_bundle(bundle_path: &Path) -> Result<Vec<FlowEntry>> {
    let flow_files = find_flow_files(bundle_path)?;
    let mut entries = Vec::new();

    for (pack_id, flow_path) in &flow_files {
        match parse_flow_file(flow_path, pack_id) {
            Ok(entry) => entries.push(entry),
            Err(e) => {
                warn!(path = %flow_path.display(), error = %e, "failed to parse flow file");
            }
        }
    }

    Ok(entries)
}

/// Scans a bundle directory and returns all parsed flow entries with verbose output.
///
/// # Arguments
///
/// * `bundle_path` - Path to the bundle root directory
/// * `verbose` - Whether to print verbose output
///
/// # Returns
///
/// A vector of successfully parsed flow entries.
pub fn scan_bundle_verbose(bundle_path: &Path, verbose: bool) -> Result<Vec<FlowEntry>> {
    if verbose {
        info!(bundle = %bundle_path.display(), "scanning bundle");
    }

    let flow_files = find_flow_files(bundle_path)?;

    if verbose {
        info!(count = flow_files.len(), "found flow files");
    }

    let mut entries = Vec::new();

    for (pack_id, flow_path) in &flow_files {
        match parse_flow_file(flow_path, pack_id) {
            Ok(entry) => {
                if verbose {
                    debug!(
                        pack_id = %entry.pack_id,
                        flow_id = %entry.flow_id,
                        title = %entry.title,
                        "parsed flow"
                    );
                }
                entries.push(entry);
            }
            Err(e) => {
                warn!(path = %flow_path.display(), error = %e, "failed to parse flow file");
            }
        }
    }

    Ok(entries)
}

/// Finds all .ygtc flow files in a bundle directory.
///
/// # Arguments
///
/// * `bundle_path` - Path to the bundle root directory
///
/// # Returns
///
/// A vector of tuples containing (pack_id, flow_path).
pub fn find_flow_files(bundle_path: &Path) -> Result<Vec<(String, PathBuf)>> {
    let mut flows = Vec::new();

    for entry in WalkDir::new(bundle_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "ygtc") {
            let pack_id = derive_pack_id(bundle_path, path);
            flows.push((pack_id, path.to_path_buf()));
        }
    }

    Ok(flows)
}

/// Derives the pack_id from the directory structure.
///
/// The function attempts to extract pack_id based on common bundle structures:
/// - `bundle/packs/pack-name/flows/flow.ygtc` → `pack-name`
/// - `bundle/apps/app-name/flows/flow.ygtc` → `app-name`
///
/// Falls back to parent directory name if structure doesn't match.
///
/// # Arguments
///
/// * `bundle_path` - Path to the bundle root
/// * `flow_path` - Path to the flow file
///
/// # Returns
///
/// The derived pack_id string.
pub fn derive_pack_id(bundle_path: &Path, flow_path: &Path) -> String {
    if let Ok(relative) = flow_path.strip_prefix(bundle_path) {
        let components: Vec<_> = relative.components().collect();
        if components.len() >= 3 {
            // Return the directory name (packs/apps name)
            if let Some(name) = components.get(1) {
                return name.as_os_str().to_string_lossy().to_string();
            }
        }
    }

    // Fallback: use parent directory name
    flow_path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "default".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_derive_pack_id_from_packs_structure() {
        let dir = tempdir().unwrap();
        let bundle = dir.path();

        // Create structure: bundle/packs/my-pack/flows/flow.ygtc
        let flow_dir = bundle.join("packs/my-pack/flows");
        fs::create_dir_all(&flow_dir).unwrap();
        let flow_path = flow_dir.join("test.ygtc");
        fs::write(&flow_path, "id: test").unwrap();

        let pack_id = derive_pack_id(bundle, &flow_path);
        assert_eq!(pack_id, "my-pack");
    }

    #[test]
    fn test_derive_pack_id_from_apps_structure() {
        let dir = tempdir().unwrap();
        let bundle = dir.path();

        // Create structure: bundle/apps/my-app/flows/flow.ygtc
        let flow_dir = bundle.join("apps/my-app/flows");
        fs::create_dir_all(&flow_dir).unwrap();
        let flow_path = flow_dir.join("test.ygtc");
        fs::write(&flow_path, "id: test").unwrap();

        let pack_id = derive_pack_id(bundle, &flow_path);
        assert_eq!(pack_id, "my-app");
    }

    #[test]
    fn test_find_flow_files() {
        let dir = tempdir().unwrap();
        let bundle = dir.path();

        // Create some flow files
        let flow_dir = bundle.join("packs/test-pack/flows");
        fs::create_dir_all(&flow_dir).unwrap();
        fs::write(flow_dir.join("flow1.ygtc"), "id: flow1").unwrap();
        fs::write(flow_dir.join("flow2.ygtc"), "id: flow2").unwrap();
        fs::write(flow_dir.join("not-a-flow.yaml"), "id: skip").unwrap();

        let flows = find_flow_files(bundle).unwrap();
        assert_eq!(flows.len(), 2);
    }
}
