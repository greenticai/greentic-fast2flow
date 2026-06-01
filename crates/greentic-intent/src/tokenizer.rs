//! Tokenizers split text into [`Token`]s with byte offsets preserved.

use crate::token::{Token, TokenShape};

/// Tokenizer abstraction. Implementations must preserve byte offsets such
/// that `&text[token.start..token.end] == token.text`.
pub trait Tokenizer: Send + Sync {
    /// Tokenize `text`. The returned tokens MAY include whitespace tokens
    /// (filtered out by downstream extractors).
    fn tokenize(&self, text: &str) -> Vec<Token>;
}

/// Default tokenizer: splits on Unicode whitespace + emits a separate
/// token for each punctuation glyph. Conservative; good enough to keep
/// the offset invariant while richer tokenizers (ICU, unicode-segmentation)
/// land later.
#[derive(Debug, Default, Clone, Copy)]
pub struct WhitespaceTokenizer;

impl Tokenizer for WhitespaceTokenizer {
    fn tokenize(&self, text: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut iter = text.char_indices().peekable();
        while let Some(&(start, ch)) = iter.peek() {
            let shape = classify(ch);
            match shape {
                TokenShape::Whitespace => {
                    let mut end = start + ch.len_utf8();
                    iter.next();
                    while let Some(&(idx, c)) = iter.peek() {
                        if classify(c) == TokenShape::Whitespace {
                            end = idx + c.len_utf8();
                            iter.next();
                        } else {
                            break;
                        }
                    }
                    tokens.push(make_token(text, start, end, TokenShape::Whitespace));
                }
                TokenShape::Punctuation => {
                    let end = start + ch.len_utf8();
                    iter.next();
                    tokens.push(make_token(text, start, end, TokenShape::Punctuation));
                }
                _ => {
                    let mut end = start + ch.len_utf8();
                    let initial = shape;
                    iter.next();
                    while let Some(&(idx, c)) = iter.peek() {
                        let s = classify(c);
                        if s == TokenShape::Whitespace || s == TokenShape::Punctuation {
                            break;
                        }
                        end = idx + c.len_utf8();
                        iter.next();
                        if s != initial {
                            // Mark as mixed shape if we crossed boundaries.
                            // We update the eventual shape after the loop.
                        }
                    }
                    let final_shape = if text[start..end].chars().all(|c| c.is_alphabetic()) {
                        TokenShape::Word
                    } else if text[start..end].chars().all(|c| c.is_ascii_digit()) {
                        TokenShape::Number
                    } else {
                        TokenShape::Mixed
                    };
                    tokens.push(make_token(text, start, end, final_shape));
                }
            }
        }
        tokens
    }
}

fn classify(c: char) -> TokenShape {
    if c.is_whitespace() {
        TokenShape::Whitespace
    } else if c.is_alphabetic() {
        TokenShape::Word
    } else if c.is_ascii_digit() {
        TokenShape::Number
    } else if c.is_ascii_punctuation() {
        TokenShape::Punctuation
    } else {
        TokenShape::Other
    }
}

fn make_token(text: &str, start: usize, end: usize, shape: TokenShape) -> Token {
    let surface = &text[start..end];
    Token {
        text: surface.to_string(),
        lower: surface.to_lowercase(),
        start,
        end,
        shape,
        script: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_byte_offsets_for_ascii() {
        let tok = WhitespaceTokenizer;
        let text = "what is the weather in London tomorrow?";
        let tokens = tok.tokenize(text);
        for t in &tokens {
            assert_eq!(&text[t.start..t.end], t.text);
        }
    }

    #[test]
    fn punctuation_emitted_as_own_token() {
        let tok = WhitespaceTokenizer;
        let tokens = tok.tokenize("hi!");
        let non_ws: Vec<&Token> = tokens
            .iter()
            .filter(|t| t.shape != TokenShape::Whitespace)
            .collect();
        assert_eq!(non_ws.len(), 2);
        assert_eq!(non_ws[0].text, "hi");
        assert_eq!(non_ws[1].text, "!");
    }

    #[test]
    fn multibyte_utf8_offsets_match() {
        let tok = WhitespaceTokenizer;
        let text = "mañana";
        let tokens = tok.tokenize(text);
        assert_eq!(tokens.len(), 1);
        assert_eq!(&text[tokens[0].start..tokens[0].end], "mañana");
    }
}
