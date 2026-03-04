use fast2flow_contracts::{Candidate, Decision};

pub trait RoutingStrategy: Send + Sync {
    fn evaluate(&self, query: &str, candidates: &[Candidate]) -> Option<Decision>;
}

pub fn tokenize(input: &str) -> Vec<String> {
    input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

pub fn token_similarity(a: &str, b: &str) -> f32 {
    let a_tokens = tokenize(a);
    let b_tokens = tokenize(b);
    if a_tokens.is_empty() || b_tokens.is_empty() {
        return 0.0;
    }

    let overlap = a_tokens
        .iter()
        .filter(|token| b_tokens.contains(*token))
        .count();

    overlap as f32 / a_tokens.len().max(b_tokens.len()) as f32
}
