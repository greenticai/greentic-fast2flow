//! Schema definitions for matcher component.

use std::collections::BTreeMap;

use greentic_types::cbor::canonical;
use greentic_types::schemas::common::schema_ir::{AdditionalProperties, SchemaIr};

/// Schema for match operation input.
pub fn input_schema() -> SchemaIr {
    object_schema(vec![
        ("query", string_schema(1, 1024), true),
        ("index", index_schema(), true),
        (
            "threshold",
            SchemaIr::Float {
                min: Some(0.0),
                max: Some(1.0),
            },
            false,
        ),
        (
            "max_results",
            SchemaIr::Int {
                min: Some(1),
                max: Some(100),
            },
            false,
        ),
    ])
}

/// Schema for match operation output.
pub fn output_schema() -> SchemaIr {
    object_schema(vec![
        ("status", match_status_schema(), true),
        ("top_match", match_result_schema(), false),
        ("candidates", candidates_array_schema(), true),
        (
            "latency_ms",
            SchemaIr::Int {
                min: Some(0),
                max: None,
            },
            true,
        ),
    ])
}

pub fn input_schema_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&input_schema()).unwrap_or_default()
}

pub fn output_schema_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&output_schema()).unwrap_or_default()
}

// Helper functions

fn string_schema(min: u64, max: u64) -> SchemaIr {
    SchemaIr::String {
        min_len: Some(min),
        max_len: Some(max),
        regex: None,
        format: None,
    }
}

/// Creates an object schema with explicit required tracking.
fn object_schema(props: Vec<(&str, SchemaIr, bool)>) -> SchemaIr {
    let mut properties = BTreeMap::new();
    let mut required = Vec::new();
    for (name, schema, is_required) in props {
        properties.insert(String::from(name), schema);
        if is_required {
            required.push(String::from(name));
        }
    }
    SchemaIr::Object {
        properties,
        required,
        additional: AdditionalProperties::Allow,
    }
}

fn index_schema() -> SchemaIr {
    object_schema(vec![
        ("version", string_schema(1, 32), true),
        (
            "flows",
            SchemaIr::Array {
                items: Box::new(flow_entry_schema()),
                min_items: None,
                max_items: None,
            },
            true,
        ),
        ("term_frequencies", any_schema(), false),
        ("document_frequencies", any_schema(), false),
    ])
}

fn flow_entry_schema() -> SchemaIr {
    object_schema(vec![
        ("pack_id", string_schema(1, 256), true),
        ("flow_id", string_schema(1, 128), true),
        ("title", string_schema(1, 256), true),
        ("description", string_schema(1, 1024), false),
        (
            "tags",
            SchemaIr::Array {
                items: Box::new(string_schema(1, 64)),
                min_items: None,
                max_items: None,
            },
            false,
        ),
        (
            "keywords",
            SchemaIr::Array {
                items: Box::new(string_schema(1, 64)),
                min_items: None,
                max_items: None,
            },
            false,
        ),
    ])
}

fn match_status_schema() -> SchemaIr {
    SchemaIr::String {
        min_len: Some(1),
        max_len: Some(32),
        regex: Some(String::from("^(match|ambiguous|no_match|timeout)$")),
        format: None,
    }
}

fn match_result_schema() -> SchemaIr {
    object_schema(vec![
        ("pack_id", string_schema(1, 256), true),
        ("flow_id", string_schema(1, 128), true),
        ("title", string_schema(1, 256), true),
        (
            "confidence",
            SchemaIr::Float {
                min: Some(0.0),
                max: Some(1.0),
            },
            true,
        ),
    ])
}

fn candidates_array_schema() -> SchemaIr {
    SchemaIr::Array {
        items: Box::new(match_result_schema()),
        min_items: None,
        max_items: None,
    }
}

/// Creates an "any" schema using OneOf with common types.
fn any_schema() -> SchemaIr {
    SchemaIr::OneOf {
        variants: vec![
            SchemaIr::Null,
            SchemaIr::Bool,
            SchemaIr::String {
                min_len: None,
                max_len: None,
                regex: None,
                format: None,
            },
            SchemaIr::Int {
                min: None,
                max: None,
            },
            SchemaIr::Float {
                min: None,
                max: None,
            },
            SchemaIr::Object {
                properties: BTreeMap::new(),
                required: Vec::new(),
                additional: AdditionalProperties::Allow,
            },
            SchemaIr::Array {
                items: Box::new(SchemaIr::Null),
                min_items: None,
                max_items: None,
            },
        ],
    }
}
