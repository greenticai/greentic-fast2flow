//! `MarkResult` + marker renderer.

use serde::{Deserialize, Serialize};

use crate::entity::Entity;
use crate::language::ResolvedLanguage;

/// Non-fatal warning surfaced alongside a [`MarkResult`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntentWarning {
    /// Stable warning code (e.g. `locale_fallback`).
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

/// Latency breakdown for a single `mark()` call. Milliseconds.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct IntentLatency {
    /// Total elapsed wall-clock.
    pub total_ms: f32,
    /// Tokenization phase.
    pub tokenize_ms: f32,
    /// Sum of all extractor invocations.
    pub extract_ms: f32,
    /// Overlap resolution phase.
    pub resolve_ms: f32,
    /// Marker rendering phase.
    pub render_ms: f32,
}

/// Output of `IntentEngine::mark`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarkResult {
    /// Verbatim input text.
    pub original_text: String,
    /// Type-only markers — `{{location}} {{date}}`. Stable across languages.
    pub marked_text: String,
    /// Role-aware markers — `{{location:from}} {{location:to}} {{date}}`.
    pub marked_text_roles: String,
    /// Debug-friendly markers carrying normalized values:
    /// `{{location: London}} {{date: 20260528}}`.
    pub marked_text_debug: String,
    /// Resolved language metadata.
    pub language: ResolvedLanguage,
    /// Extracted entities (post overlap resolution).
    pub entities: Vec<Entity>,
    /// Non-fatal warnings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<IntentWarning>,
    /// Per-phase latency.
    pub latency: IntentLatency,
}

/// Renders the three marker flavors from an entity list. Replaces spans
/// right-to-left so byte offsets stay valid during substitution.
pub fn render_markers(text: &str, entities: &[Entity]) -> MarkResult {
    let mut sorted: Vec<&Entity> = entities.iter().collect();
    sorted.sort_by_key(|e| std::cmp::Reverse(e.start));

    let mut type_only = text.to_string();
    let mut roles = text.to_string();
    let mut debug = text.to_string();
    for entity in &sorted {
        let kind = entity.kind.marker_name();
        let value = &entity.normalized;
        let role_marker = match entity.role.as_deref() {
            Some(role) if !role.is_empty() => format!("{{{{{kind}:{role}}}}}"),
            _ => format!("{{{{{kind}}}}}"),
        };
        let debug_marker = format!("{{{{{kind}: {value}}}}}");
        let type_marker = format!("{{{{{kind}}}}}");
        type_only.replace_range(entity.start..entity.end, &type_marker);
        roles.replace_range(entity.start..entity.end, &role_marker);
        debug.replace_range(entity.start..entity.end, &debug_marker);
    }

    MarkResult {
        original_text: text.to_string(),
        marked_text: type_only,
        marked_text_roles: roles,
        marked_text_debug: debug,
        language: ResolvedLanguage::fallback_en_gb(),
        entities: entities.to_vec(),
        warnings: Vec::new(),
        latency: IntentLatency::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Entity, EntityKind};

    fn entity(start: usize, end: usize, raw: &str, kind: EntityKind, role: Option<&str>) -> Entity {
        Entity {
            id: "e1".into(),
            kind,
            raw: raw.into(),
            normalized: raw.into(),
            canonical: None,
            start,
            end,
            role: role.map(str::to_string),
            confidence: 1.0,
            locale: "en-GB".into(),
            evidence: Vec::new(),
        }
    }

    #[test]
    fn renders_type_only_marker() {
        let text = "weather in London tomorrow?";
        let london = entity(11, 17, "London", EntityKind::Location, Some("in"));
        let tomorrow = entity(18, 26, "tomorrow", EntityKind::Date, None);
        let result = render_markers(text, &[london, tomorrow]);
        assert_eq!(result.marked_text, "weather in {{location}} {{date}}?");
    }

    #[test]
    fn role_markers_when_role_present() {
        let text = "from London to Paris";
        let from = entity(5, 11, "London", EntityKind::Location, Some("from"));
        let to = entity(15, 20, "Paris", EntityKind::Location, Some("to"));
        let result = render_markers(text, &[from, to]);
        assert_eq!(
            result.marked_text_roles,
            "from {{location:from}} to {{location:to}}"
        );
    }

    #[test]
    fn debug_markers_include_normalized_value() {
        let text = "in London";
        let mut london = entity(3, 9, "London", EntityKind::Location, None);
        london.normalized = "London".into();
        let result = render_markers(text, &[london]);
        assert_eq!(result.marked_text_debug, "in {{location: London}}");
    }
}
