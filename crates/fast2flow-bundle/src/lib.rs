//! Bundle scanning and indexing for fast2flow routing.
//!
//! This crate provides utilities to:
//! - Scan bundle directories for flow definitions (.ygtc files)
//! - Parse flow metadata and extract keywords
//! - Build TF-IDF indexes for fast intent matching
//! - Generate human-readable intents documentation
//!
//! # Example
//!
//! ```no_run
//! use fast2flow_bundle::{scan_bundle, build_index_manifest, generate_intents_md};
//! use std::path::Path;
//!
//! let bundle_path = Path::new("./my-bundle");
//! let flows = scan_bundle(bundle_path).unwrap();
//! let manifest = build_index_manifest(&flows, "tenant1", "default");
//! let docs = generate_intents_md(&flows, "tenant1", "default");
//! ```

mod index;
mod parser;
mod scanner;

pub mod hooks;

pub use index::{
    build_index_manifest, build_index_manifest_for_endpoint, build_index_manifest_with_scope,
    generate_intents_md, IndexManifest,
};
pub use parser::{extract_keywords, parse_flow_file, FlowEntry, FlowMeta};
pub use scanner::{derive_pack_id, find_flow_files, scan_bundle};
