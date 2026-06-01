use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use fast2flow_contracts::{
    Candidate, FlowDoc, IndexEntryV1, IndexEntryV2, IndexManifestV1, IndexManifestV2,
    validate_scope,
};
use tracing::{debug, info};

/// Defense-in-depth: resolve `root.join(scope)` and verify the result is
/// contained within `root`. Rejects path-traversal attempts even if the
/// scope string somehow bypasses higher-level validation.
///
/// The function first validates `scope` via [`validate_scope`], then ensures
/// the resolved path does not escape `root`.
pub fn normalize_under_root(root: &Path, scope: &str) -> io::Result<PathBuf> {
    validate_scope(scope).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    // Canonicalize root (must exist).
    let canonical_root =
        fs::canonicalize(root).map_err(|e| io::Error::new(io::ErrorKind::NotFound, e))?;

    let joined = canonical_root.join(scope);

    // The scope dir may not exist yet (write path). Walk up to find an
    // existing ancestor, canonicalize that, then re-append the remainder.
    let resolved = if joined.exists() {
        fs::canonicalize(&joined)?
    } else {
        // Find the deepest existing ancestor.
        let mut ancestor = joined.as_path();
        let mut suffix_parts = Vec::new();
        loop {
            if let Some(parent) = ancestor.parent() {
                if parent.exists() {
                    // Collect the component we peeled off.
                    if let Some(file_name) = ancestor.file_name() {
                        suffix_parts.push(file_name.to_owned());
                    }
                    ancestor = parent;
                    // Canonicalize the existing ancestor.
                    let canonical_ancestor = fs::canonicalize(ancestor)?;
                    // Re-append the non-existent tail.
                    let mut result = canonical_ancestor;
                    for part in suffix_parts.into_iter().rev() {
                        result = result.join(part);
                    }
                    return if result.starts_with(&canonical_root) {
                        Ok(result)
                    } else {
                        Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            "scope path escapes root",
                        ))
                    };
                }
                // Parent doesn't exist either — keep walking up.
                if let Some(file_name) = ancestor.file_name() {
                    suffix_parts.push(file_name.to_owned());
                }
                ancestor = parent;
            } else {
                // Reached filesystem root without finding an existing dir.
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "no existing ancestor for scope path",
                ));
            }
        }
    };

    if resolved.starts_with(&canonical_root) {
        Ok(resolved)
    } else {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "scope path escapes root",
        ))
    }
}

#[derive(Debug, Clone)]
pub struct IndexStore {
    manifest: IndexManifestV2,
}

impl IndexStore {
    /// Construct from a v2 manifest directly.
    pub fn from_manifest(manifest: IndexManifestV2) -> Self {
        Self { manifest }
    }

    /// Construct from a legacy v1 manifest. Lifts to v2 internally.
    pub fn from_v1_manifest(manifest: IndexManifestV1) -> Self {
        Self {
            manifest: IndexManifestV2::from_v1(manifest),
        }
    }

    pub fn manifest(&self) -> &IndexManifestV2 {
        &self.manifest
    }

    pub fn search(&self, text: &str, limit: usize) -> Vec<Candidate> {
        let candidates = search_manifest(&self.manifest, text, limit);
        debug!(
            scope = %self.manifest.scope,
            query_len = text.trim().len(),
            entries = self.manifest.entries.len(),
            matched = candidates.len(),
            limit,
            "index search"
        );
        candidates
    }
}

/// Parse an index manifest from JSON bytes. Reads either v2 (preferred,
/// dispatched on `"version": "v2"`) or v1 (`"version": "v1"` or absent
/// version field, for compat with hand-rolled fixtures). v1 manifests
/// are lifted to v2 transparently.
pub fn parse_manifest(bytes: &[u8]) -> Result<IndexManifestV2> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).context("failed parsing index manifest json")?;
    let version = value
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("v1");
    match version {
        "v2" => {
            let m: IndexManifestV2 =
                serde_json::from_value(value).context("failed decoding v2 manifest")?;
            Ok(m)
        }
        "v1" => {
            let m: IndexManifestV1 =
                serde_json::from_value(value).context("failed decoding v1 manifest")?;
            Ok(IndexManifestV2::from_v1(m))
        }
        other => Err(anyhow::anyhow!("unknown index manifest version: {other}")),
    }
}

/// Build a v2 manifest from flow docs. Use this for new code.
pub fn build_manifest_v2(scope: &str, flows: &[FlowDoc], now_unix_ms: u64) -> IndexManifestV2 {
    let entries = flows
        .iter()
        .map(|flow| IndexEntryV2 {
            flow_id: flow.id.clone(),
            pack_id: flow.pack_id.clone(),
            target: flow.target.clone(),
            title: flow.title.clone(),
            tags: flow.tags.clone(),
            utterances: Vec::new(),
            node_ids: flow.node_ids.clone(),
        })
        .collect();

    IndexManifestV2 {
        version: "v2".to_string(),
        scope: scope.to_string(),
        generated_at_ms: now_unix_ms,
        entries,
    }
}

/// Build a v1 manifest. Retained for callers that still need to emit
/// the legacy shape during the migration window. Prefer
/// [`build_manifest_v2`] for new code.
#[deprecated(note = "use build_manifest_v2 + write v2 instead")]
pub fn build_manifest(scope: &str, flows: &[FlowDoc], now_unix_ms: u64) -> IndexManifestV1 {
    let entries = flows
        .iter()
        .map(|flow| IndexEntryV1 {
            flow_id: flow.id.clone(),
            node_ids: flow.node_ids.clone(),
            titles: vec![flow.title.clone()],
            tags: flow.tags.clone(),
            pack_id: flow.pack_id.clone(),
            target: flow.target.clone(),
        })
        .collect();

    IndexManifestV1 {
        version: "v1".to_string(),
        scope: scope.to_string(),
        generated_at_ms: now_unix_ms,
        entries,
    }
}

pub fn build_index(
    scope: &str,
    flows: &[FlowDoc],
    indexes_root: &Path,
    now_unix_ms: u64,
) -> Result<IndexManifestV2> {
    let manifest = build_manifest_v2(scope, flows, now_unix_ms);
    write_manifest(indexes_root, scope, &manifest)?;
    info!(
        scope,
        flows = flows.len(),
        entries = manifest.entries.len(),
        root = %indexes_root.display(),
        "built index manifest"
    );
    Ok(manifest)
}

pub fn load_latest(indexes_root: &Path, scope: &str) -> Result<IndexStore> {
    let scope_dir = normalize_under_root(indexes_root, scope)
        .with_context(|| format!("scope {scope:?} failed path validation"))?;
    let latest_path = scope_dir.join("latest");
    let latest_name = fs::read_to_string(&latest_path)
        .with_context(|| format!("failed reading {}", latest_path.display()))?
        .trim()
        .to_string();

    let manifest_path = scope_dir.join(latest_name);
    let payload = fs::read(&manifest_path)
        .with_context(|| format!("failed reading {}", manifest_path.display()))?;
    let manifest = parse_manifest(&payload)
        .with_context(|| format!("failed parsing {}", manifest_path.display()))?;
    debug!(
        scope,
        path = %manifest_path.display(),
        entries = manifest.entries.len(),
        version = manifest.version,
        "loaded index manifest"
    );
    Ok(IndexStore::from_manifest(manifest))
}

fn write_manifest(indexes_root: &Path, scope: &str, manifest: &IndexManifestV2) -> Result<()> {
    // Validate scope before joining to prevent path traversal on create_dir_all.
    validate_scope(scope)
        .map_err(|e| anyhow::anyhow!("scope validation failed: {e}"))
        .with_context(|| format!("scope {scope:?} is invalid"))?;
    let scope_dir = indexes_root.join(scope);
    fs::create_dir_all(&scope_dir)
        .with_context(|| format!("failed creating {}", scope_dir.display()))?;

    let final_name = "index.json";
    let tmp_path = unique_tmp_path(&scope_dir, final_name);
    let final_path = scope_dir.join(final_name);
    let latest_tmp = unique_tmp_path(&scope_dir, "latest");
    let latest_path = scope_dir.join("latest");

    let payload = serde_json::to_string_pretty(manifest)?;
    fs::write(&tmp_path, payload)
        .with_context(|| format!("failed writing {}", tmp_path.display()))?;
    fs::rename(&tmp_path, &final_path)
        .with_context(|| format!("failed atomically updating {}", final_path.display()))?;

    fs::write(&latest_tmp, format!("{final_name}\n"))
        .with_context(|| format!("failed writing {}", latest_tmp.display()))?;
    fs::rename(&latest_tmp, &latest_path)
        .with_context(|| format!("failed atomically updating {}", latest_path.display()))?;

    debug!(path = %final_path.display(), entries = manifest.entries.len(), "wrote index manifest");
    Ok(())
}

fn unique_tmp_path(scope_dir: &Path, base_name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_nanos();
    scope_dir.join(format!("{base_name}.{}.{}.tmp", process::id(), now))
}

fn search_manifest(manifest: &IndexManifestV2, text: &str, limit: usize) -> Vec<Candidate> {
    let mut scored = manifest
        .entries
        .iter()
        .map(|entry| {
            let query = normalize(text);
            let title_blob = normalize(&entry.title);
            let tags_blob = normalize(&entry.tags.join(" "));
            let utter_blob = normalize(&entry.utterances.join(" "));
            let mut score = overlap_score(&query, &title_blob);
            score = score.max(overlap_score(&query, &tags_blob));
            score = score.max(overlap_score(&query, &utter_blob));

            (
                score,
                Candidate {
                    target: entry.target.clone(),
                    flow_id: entry.flow_id.clone(),
                    title: entry.title.clone(),
                    tags: entry.tags.clone(),
                    score_hint: score,
                },
            )
        })
        .collect::<Vec<(f32, Candidate)>>();

    scored.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .total_cmp(left_score)
            .then_with(|| left.target.cmp(&right.target))
            .then_with(|| left.flow_id.cmp(&right.flow_id))
    });

    scored
        .into_iter()
        .filter(|(score, _)| *score > 0.0)
        .take(limit)
        .map(|(_, candidate)| candidate)
        .collect()
}

fn overlap_score(query: &str, corpus: &str) -> f32 {
    let query_tokens = tokenize(query);
    let corpus_tokens = tokenize(corpus);
    if query_tokens.is_empty() || corpus_tokens.is_empty() {
        return 0.0;
    }

    let overlap = query_tokens
        .iter()
        .filter(|token| corpus_tokens.contains(*token))
        .count();

    overlap as f32 / query_tokens.len() as f32
}

fn tokenize(input: &str) -> Vec<String> {
    input
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<String>>()
}

fn normalize(input: &str) -> String {
    input
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch.is_whitespace() {
                ch
            } else {
                ' '
            }
        })
        .collect::<String>()
}

pub fn default_indexes_root() -> PathBuf {
    PathBuf::from("/mnt/indexes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_v2_manifest_round_trips() {
        let v2 = build_manifest_v2(
            "demo:default",
            &[FlowDoc {
                id: "pipeline".into(),
                pack_id: "demo".into(),
                target: "demo/pipeline".into(),
                title: "View pipeline".into(),
                tags: vec!["pipeline".into()],
                node_ids: vec![],
            }],
            42,
        );
        let payload = serde_json::to_string_pretty(&v2).unwrap();
        let parsed = parse_manifest(payload.as_bytes()).unwrap();
        assert_eq!(parsed.version, "v2");
        assert_eq!(parsed.entries[0].title, "View pipeline");
    }

    #[test]
    fn parse_v1_manifest_lifts_to_v2() {
        // Hand-crafted v1 payload mirroring what hand-rolled demos shipped.
        let v1_payload = r#"{
            "version": "v1",
            "scope": "demo:default",
            "generated_at_ms": 0,
            "entries": [{
                "flow_id": "pipeline",
                "node_ids": ["pipeline_card"],
                "titles": ["View pipeline"],
                "tags": ["pipeline"],
                "pack_id": "demo",
                "target": "demo/pipeline"
            }]
        }"#;
        let parsed = parse_manifest(v1_payload.as_bytes()).unwrap();
        assert_eq!(parsed.version, "v2");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].title, "View pipeline");
        assert_eq!(parsed.entries[0].node_ids, vec!["pipeline_card"]);
    }

    #[test]
    fn parse_rejects_unknown_version() {
        let payload = r#"{"version":"v99","scope":"x","generated_at_ms":0,"entries":[]}"#;
        let err = parse_manifest(payload.as_bytes()).unwrap_err();
        assert!(err.to_string().contains("unknown index manifest version"));
    }

    #[test]
    fn search_uses_utterances_in_v2_scoring() {
        let manifest = IndexManifestV2 {
            version: "v2".into(),
            scope: "demo:default".into(),
            generated_at_ms: 0,
            entries: vec![IndexEntryV2 {
                flow_id: "meeting".into(),
                pack_id: "demo".into(),
                target: "demo/meeting".into(),
                title: "schedule a meeting".into(),
                tags: vec!["meeting".into()],
                utterances: vec!["book me a slot with sales".into()],
                node_ids: vec![],
            }],
        };
        let store = IndexStore::from_manifest(manifest);
        let cands = store.search("book me a slot please", 5);
        assert_eq!(cands.len(), 1);
        assert!(cands[0].score_hint > 0.0);
        assert_eq!(cands[0].flow_id, "meeting");
    }
}
