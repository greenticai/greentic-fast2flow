use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use fast2flow_contracts::{Candidate, FlowDoc, IndexEntryV1, IndexManifestV1};
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct IndexStore {
    manifest: IndexManifestV1,
}

impl IndexStore {
    pub fn from_manifest(manifest: IndexManifestV1) -> Self {
        Self { manifest }
    }

    pub fn manifest(&self) -> &IndexManifestV1 {
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
) -> Result<IndexManifestV1> {
    let manifest = build_manifest(scope, flows, now_unix_ms);
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
    let scope_dir = indexes_root.join(scope);
    let latest_path = scope_dir.join("latest");
    let latest_name = fs::read_to_string(&latest_path)
        .with_context(|| format!("failed reading {}", latest_path.display()))?
        .trim()
        .to_string();

    let manifest_path = scope_dir.join(latest_name);
    let payload = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed reading {}", manifest_path.display()))?;
    let manifest = serde_json::from_str::<IndexManifestV1>(&payload)
        .with_context(|| format!("failed parsing {}", manifest_path.display()))?;
    debug!(
        scope,
        path = %manifest_path.display(),
        entries = manifest.entries.len(),
        "loaded index manifest"
    );
    Ok(IndexStore::from_manifest(manifest))
}

fn write_manifest(indexes_root: &Path, scope: &str, manifest: &IndexManifestV1) -> Result<()> {
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

fn search_manifest(manifest: &IndexManifestV1, text: &str, limit: usize) -> Vec<Candidate> {
    let mut scored = manifest
        .entries
        .iter()
        .map(|entry| {
            let query = normalize(text);
            let title_blob = normalize(&entry.titles.join(" "));
            let tags_blob = normalize(&entry.tags.join(" "));
            let mut score = overlap_score(&query, &title_blob);
            score = score.max(overlap_score(&query, &tags_blob));

            (
                score,
                Candidate {
                    target: entry.target.clone(),
                    flow_id: entry.flow_id.clone(),
                    title: entry.titles.first().cloned().unwrap_or_default(),
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
