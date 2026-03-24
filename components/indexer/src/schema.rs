//! Schema definitions for indexer component.

use std::collections::BTreeMap;

use greentic_types::cbor::canonical;
use greentic_types::schemas::common::schema_ir::{AdditionalProperties, SchemaIr};

/// Schema for build operation input.
pub fn build_input_schema() -> SchemaIr {
    object_schema(vec![
        ("flows", flows_array_schema(), true),
        ("tenant_id", string_schema(1, 128), true),
        ("team_id", string_schema(1, 128), false),
    ])
}

/// Schema for update operation input.
pub fn update_input_schema() -> SchemaIr {
    object_schema(vec![
        ("flows", flows_array_schema(), true),
        ("tenant_id", string_schema(1, 128), true),
        ("team_id", string_schema(1, 128), false),
        ("mode", mode_enum_schema(), true), // "add", "remove", "replace"
    ])
}

/// Schema for build/update output.
pub fn build_output_schema() -> SchemaIr {
    object_schema(vec![
        ("version", string_schema(1, 32), true),
        ("last_updated", string_schema(1, 64), true),
        (
            "flow_count",
            SchemaIr::Int {
                min: Some(0),
                max: None,
            },
            true,
        ),
        ("index_key", string_schema(1, 256), true),
    ])
}

pub fn build_input_schema_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&build_input_schema()).unwrap_or_default()
}

pub fn update_input_schema_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&update_input_schema()).unwrap_or_default()
}

pub fn build_output_schema_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&build_output_schema()).unwrap_or_default()
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

fn flows_array_schema() -> SchemaIr {
    SchemaIr::Array {
        items: Box::new(flow_entry_schema()),
        min_items: None,
        max_items: None,
    }
}

fn flow_entry_schema() -> SchemaIr {
    object_schema(vec![
        ("pack_id", string_schema(1, 256), true),
        ("flow_id", string_schema(1, 128), true),
        ("title", string_schema(1, 256), true),
        ("description", string_schema(1, 1024), false),
        ("tags", tags_array_schema(), false),
        ("keywords", keywords_array_schema(), false),
    ])
}

fn tags_array_schema() -> SchemaIr {
    SchemaIr::Array {
        items: Box::new(string_schema(1, 64)),
        min_items: None,
        max_items: None,
    }
}

fn keywords_array_schema() -> SchemaIr {
    SchemaIr::Array {
        items: Box::new(string_schema(1, 64)),
        min_items: None,
        max_items: None,
    }
}

fn mode_enum_schema() -> SchemaIr {
    SchemaIr::String {
        min_len: Some(1),
        max_len: Some(16),
        regex: Some(String::from("^(add|remove|replace)$")),
        format: None,
    }
}
