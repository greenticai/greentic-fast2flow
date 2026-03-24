//! Router component for fast2flow.
//!
//! Orchestrates the full routing pipeline with optional LLM fallback.

use greentic_interfaces_guest::component_v0_6::node;

pub mod route;
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
            summary: Some("Intelligent flow routing with LLM fallback".to_string()),
            capabilities: Vec::new(),
            ops: vec![node::Op {
                name: "route".to_string(),
                summary: Some("Route message to appropriate flow".to_string()),
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
            "route" => route::route_message(envelope.payload_cbor),
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

#[cfg(target_arch = "wasm32")]
greentic_interfaces_guest::export_component_v060!(Component);
