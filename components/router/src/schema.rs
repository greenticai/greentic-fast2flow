//! Schema definitions for router component.

use std::collections::BTreeMap;

use greentic_types::cbor::canonical;
use greentic_types::schemas::common::schema_ir::{AdditionalProperties, SchemaIr};

/// Schema for route operation input.
pub fn input_schema() -> SchemaIr {
    object_schema(vec![
        ("message", message_schema(), true),
        ("match_result", match_result_schema(), true),
        ("tenant_id", string_schema(1, 128), true),
        ("team_id", string_schema(1, 128), false),
        ("config", router_config_schema(), false),
    ])
}

/// Schema for route operation output (ControlDirective).
pub fn output_schema() -> SchemaIr {
    object_schema(vec![
        ("action", action_enum_schema(), true),
        ("target", dispatch_target_schema(), false),
        ("response_text", string_schema(1, 4096), false),
        ("response_card", any_schema(), false),
        ("reason_code", string_schema(1, 64), false),
        (
            "status_code",
            SchemaIr::Int {
                min: Some(100),
                max: Some(599),
            },
            false,
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

fn message_schema() -> SchemaIr {
    object_schema(vec![
        ("id", string_schema(1, 256), true),
        ("text", string_schema(1, 65536), false),
        ("channel", string_schema(1, 64), true),
        ("session_id", string_schema(1, 256), true),
    ])
}

fn match_result_schema() -> SchemaIr {
    object_schema(vec![
        ("status", string_schema(1, 32), true),
        ("top_match", flow_ref_schema(), false),
        (
            "candidates",
            SchemaIr::Array {
                items: Box::new(flow_ref_schema()),
                min_items: None,
                max_items: None,
            },
            true,
        ),
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

fn flow_ref_schema() -> SchemaIr {
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

fn router_config_schema() -> SchemaIr {
    object_schema(vec![
        (
            "confidence_threshold",
            SchemaIr::Float {
                min: Some(0.0),
                max: Some(1.0),
            },
            false,
        ),
        (
            "ambiguity_threshold",
            SchemaIr::Float {
                min: Some(0.0),
                max: Some(1.0),
            },
            false,
        ),
        ("enable_llm_fallback", SchemaIr::Bool, false),
        ("llm_prompt_template", string_schema(1, 4096), false),
        (
            "blocked_intents",
            SchemaIr::Array {
                items: Box::new(string_schema(1, 128)),
                min_items: None,
                max_items: None,
            },
            false,
        ),
    ])
}

fn action_enum_schema() -> SchemaIr {
    SchemaIr::String {
        min_len: Some(1),
        max_len: Some(16),
        regex: Some(String::from("^(continue|dispatch|respond|deny)$")),
        format: None,
    }
}

fn dispatch_target_schema() -> SchemaIr {
    object_schema(vec![
        ("tenant", string_schema(1, 128), true),
        ("team", string_schema(1, 128), false),
        ("pack", string_schema(1, 256), true),
        ("flow", string_schema(1, 128), false),
        ("node", string_schema(1, 128), false),
    ])
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
