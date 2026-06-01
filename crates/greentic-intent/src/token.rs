//! Tokens carry byte offsets through the whole pipeline.

use serde::{Deserialize, Serialize};

/// Shape classification of a token. Cheap signal for downstream extractors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenShape {
    /// Pure ASCII / Unicode letters.
    Word,
    /// Pure digits.
    Number,
    /// Mix of letters and digits.
    Mixed,
    /// Punctuation only.
    Punctuation,
    /// Whitespace.
    Whitespace,
    /// Anything else.
    Other,
}

/// A token with byte offsets into the original text.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Token {
    /// Surface text (original casing).
    pub text: String,
    /// Lowercased convenience copy. Lazily filled in by the tokenizer.
    pub lower: String,
    /// Inclusive byte offset of the first byte of the token.
    pub start: usize,
    /// Exclusive byte offset one past the last byte.
    pub end: usize,
    /// Shape classification.
    pub shape: TokenShape,
    /// Script (best-effort, BCP-47 script subtag) — e.g. `Latin`, `Arabic`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
}
