//! Entity types: the structured side of a `MarkResult`.

use serde::{Deserialize, Serialize};

/// Language-neutral entity classification. Marker names derive directly
/// from these kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    /// Calendar date (no time component).
    Date,
    /// Time of day (no date).
    Time,
    /// Combined date + time.
    DateTime,
    /// Duration (e.g. `3 days`).
    Duration,
    /// Place / geo entity.
    Location,
    /// Email address.
    Email,
    /// Phone number.
    Phone,
    /// URL.
    Url,
    /// Person name.
    Person,
    /// Street address.
    Address,
    /// Monetary amount.
    Money,
    /// Bare number not classified as money/duration/id.
    Number,
    /// Domain identifier (order id, ticket id, etc.).
    Id,
    /// Organisation name.
    Organisation,
}

impl EntityKind {
    /// Stable marker name (e.g. `location`, `date`). Always lower-case and
    /// language-neutral — never localised.
    pub fn marker_name(self) -> &'static str {
        match self {
            EntityKind::Date => "date",
            EntityKind::Time => "time",
            EntityKind::DateTime => "datetime",
            EntityKind::Duration => "duration",
            EntityKind::Location => "location",
            EntityKind::Email => "email",
            EntityKind::Phone => "phone",
            EntityKind::Url => "url",
            EntityKind::Person => "person",
            EntityKind::Address => "address",
            EntityKind::Money => "money",
            EntityKind::Number => "number",
            EntityKind::Id => "id",
            EntityKind::Organisation => "organisation",
        }
    }
}

/// Optional evidence record explaining why an extractor picked a span.
/// Useful for debugging + auditing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityEvidence {
    /// Extractor that produced the evidence.
    pub extractor: String,
    /// Free-form rule / strategy identifier.
    pub rule: String,
    /// Optional structured detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

/// A single extracted entity in the user text.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    /// Stable identifier within the result (`e1`, `e2`, …).
    pub id: String,
    /// Entity kind.
    pub kind: EntityKind,
    /// Surface form as it appeared in the original text.
    pub raw: String,
    /// Normalised form (e.g. `London`, `20260528`, `GBP 25.00`).
    pub normalized: String,
    /// Canonical reference value (e.g. gazetteer record id) when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical: Option<String>,
    /// Inclusive byte offset of the span start.
    pub start: usize,
    /// Exclusive byte offset of the span end.
    pub end: usize,
    /// Optional role tag (e.g. `from`, `to`, `in`) derived from prepositions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Extractor confidence in [0, 1].
    pub confidence: f32,
    /// Locale active when this entity was extracted.
    pub locale: String,
    /// Optional evidence records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EntityEvidence>,
}
