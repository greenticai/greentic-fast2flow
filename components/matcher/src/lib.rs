//! BM25-based matcher component for fast2flow.
//!
//! Provides fast intent matching against a pre-built flow index.

use greentic_interfaces_guest::component_v0_6::{component_i18n, component_qa, node};

pub mod bm25;
#[allow(dead_code)]
mod descriptor;
mod schema;

#[cfg(target_arch = "wasm32")]
#[used]
#[unsafe(link_section = ".greentic.wasi")]
static WASI_TARGET_MARKER: [u8; 13] = *b"wasm32-wasip2";

struct Component;

impl node::Guest for Component {
    fn describe() -> node::ComponentDescriptor {
        let info = descriptor::info();
        node::ComponentDescriptor {
            name: info.id,
            version: info.version,
            summary: Some("Fast BM25-based intent matching".to_string()),
            capabilities: Vec::new(),
            ops: vec![node::Op {
                name: "match".to_string(),
                summary: Some("Match query against flow index".to_string()),
                input: node::IoSchema {
                    schema: node::SchemaSource::InlineCbor(schema::input_schema_cbor()),
                    content_type: "application/cbor".to_string(),
                    schema_version: None,
                },
                output: node::IoSchema {
                    schema: node::SchemaSource::InlineCbor(schema::output_schema_cbor()),
                    content_type: "application/cbor".to_string(),
                    schema_version: None,
                },
                examples: Vec::new(),
            }],
            schemas: Vec::new(),
            setup: None,
        }
    }

    fn invoke(
        operation: String,
        envelope: node::InvocationEnvelope,
    ) -> Result<node::InvocationResult, node::NodeError> {
        let output = match operation.as_str() {
            "match" => bm25::match_query(envelope.payload_cbor),
            _ => greentic_types::cbor::canonical::to_canonical_cbor_allow_floats(
                &serde_json::json!({
                    "error": format!("unsupported operation: {operation}")
                }),
            )
            .unwrap_or_default(),
        };

        Ok(node::InvocationResult {
            ok: true,
            output_cbor: output,
            output_metadata_cbor: None,
        })
    }
}

impl component_qa::Guest for Component {
    fn qa_spec(_mode: component_qa::QaMode) -> Vec<u8> {
        vec![]
    }

    fn apply_answers(
        _mode: component_qa::QaMode,
        _current_config: Vec<u8>,
        _answers: Vec<u8>,
    ) -> Vec<u8> {
        vec![]
    }
}

impl component_i18n::Guest for Component {
    fn i18n_keys() -> Vec<String> {
        vec![]
    }
}

#[cfg(target_arch = "wasm32")]
greentic_interfaces_guest::export_component_v060!(
    Component,
    component_qa: Component,
    component_i18n: Component,
);
