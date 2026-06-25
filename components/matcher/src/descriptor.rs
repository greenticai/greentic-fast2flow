//! Component descriptor for the matcher.

use std::collections::BTreeMap;

use greentic_types::cbor::canonical;
use greentic_types::schemas::common::schema_ir::{AdditionalProperties, SchemaIr};
use greentic_types::schemas::component::v0_6_0::{ComponentDescribe, ComponentInfo};

/// Returns component metadata.
pub fn info() -> ComponentInfo {
    ComponentInfo {
        id: "fast2flow.matcher".to_string(),
        version: "0.1.0".to_string(),
        role: "matcher".to_string(),
        display_name: None,
    }
}

/// Returns serialized component info as CBOR.
pub fn info_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&info()).unwrap_or_default()
}

/// Returns full component descriptor.
pub fn describe() -> ComponentDescribe {
    ComponentDescribe {
        info: info(),
        provided_capabilities: vec!["fast2flow:matcher".to_string()],
        required_capabilities: vec![],
        metadata: BTreeMap::new(),
        operations: vec![],
        config_schema: SchemaIr::Object {
            properties: BTreeMap::new(),
            required: Vec::new(),
            additional: AdditionalProperties::Allow,
        },
    }
}

/// Returns serialized component descriptor as CBOR.
pub fn describe_cbor() -> Vec<u8> {
    canonical::to_canonical_cbor_allow_floats(&describe()).unwrap_or_default()
}
