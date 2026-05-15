use fast2flow_contracts::{Candidate, Decision};
use fast2flow_strategy::{token_similarity, RoutingStrategy};
use tracing::{debug, trace};

#[derive(Debug, Clone, Default)]
pub struct Phase1DeterministicStrategy;

impl RoutingStrategy for Phase1DeterministicStrategy {
    fn evaluate(&self, query: &str, candidates: &[Candidate]) -> Option<Decision> {
        let mut ranked: Vec<(f32, &Candidate)> = candidates
            .iter()
            .map(|candidate| {
                let title_score = token_similarity(query, &candidate.title);
                let tag_score = candidate
                    .tags
                    .iter()
                    .map(|tag| token_similarity(query, tag))
                    .fold(0.0_f32, f32::max);
                let score = 0.7 * title_score + 0.2 * tag_score + 0.1 * candidate.score_hint;
                trace!(
                    target = %candidate.target,
                    flow_id = %candidate.flow_id,
                    title_score,
                    tag_score,
                    score_hint = candidate.score_hint,
                    score,
                    "phase1 candidate scored"
                );
                (score, candidate)
            })
            .collect();

        ranked.sort_by(|(left_score, left), (right_score, right)| {
            right_score
                .total_cmp(left_score)
                .then_with(|| left.target.cmp(&right.target))
                .then_with(|| left.flow_id.cmp(&right.flow_id))
        });

        let decision = ranked.first().map(|(score, winner)| Decision {
            target: winner.target.clone(),
            confidence: *score,
            reason: "phase1_deterministic_rank".to_string(),
        });
        match &decision {
            Some(d) => debug!(
                target = %d.target,
                confidence = d.confidence,
                candidates = candidates.len(),
                "phase1 selected candidate"
            ),
            None => debug!(candidates = candidates.len(), "phase1 has no candidates"),
        }
        decision
    }
}
