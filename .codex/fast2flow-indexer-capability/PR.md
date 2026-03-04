# fast2flow-indexer-capability

## Scope
Implement index build and query capabilities for Fast2Flow routing.

## Required behavior
- Build index from registry snapshot/flow docs/pack metadata into:
  - `/mnt/indexes/<scope>/index.json`
  - `/mnt/indexes/<scope>/latest`
- Use atomic update semantics when writing index files.
- Define and persist `IndexManifestV1` entries containing:
  - flow id
  - node ids
  - titles
  - tags
  - pack id
  - route target
- Provide search API: `search(text) -> Vec<Candidate>`.
- Router should load and query only `latest`.

## Integration points
- Shared schemas from `fast2flow-contracts`.
- Consumed by `fast2flow-core` through an index lookup adapter.
