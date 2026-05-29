//! Overlap resolution — pick a non-overlapping subset of entity candidates.
//!
//! Phase A: implements the rule chain described in the design doc:
//!   1. Higher confidence wins.
//!   2. Longer span wins.
//!   3. Higher kind priority wins.
//!   4. Extractor priority wins (caller-supplied order).
//!
//! Until per-kind priorities + extractor ordering are wired through, the
//! resolver uses confidence + span length only. Stable.

use crate::extractors::EntityCandidate;

/// Pick a non-overlapping subset of `candidates`.
pub fn resolve(mut candidates: Vec<EntityCandidate>) -> Vec<EntityCandidate> {
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| (b.end - b.start).cmp(&(a.end - a.start)))
    });
    let mut kept: Vec<EntityCandidate> = Vec::new();
    for cand in candidates {
        let overlaps = kept.iter().any(|k| spans_overlap(k, &cand));
        if !overlaps {
            kept.push(cand);
        }
    }
    kept.sort_by_key(|c| c.start);
    kept
}

fn spans_overlap(a: &EntityCandidate, b: &EntityCandidate) -> bool {
    a.start < b.end && b.start < a.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityKind;
    use crate::extractors::EntityCandidate;

    fn cand(start: usize, end: usize, conf: f32, kind: EntityKind) -> EntityCandidate {
        EntityCandidate {
            kind,
            raw: "x".into(),
            normalized: "x".into(),
            canonical: None,
            start,
            end,
            role: None,
            confidence: conf,
            locale: "en-GB".into(),
            evidence: Vec::new(),
        }
    }

    #[test]
    fn higher_confidence_wins_when_spans_overlap() {
        let a = cand(0, 5, 0.5, EntityKind::Location);
        let b = cand(2, 7, 0.9, EntityKind::Location);
        let kept = resolve(vec![a, b]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].start, 2);
    }

    #[test]
    fn longer_span_wins_when_confidence_equal() {
        let short = cand(0, 3, 0.5, EntityKind::Location);
        let long = cand(0, 8, 0.5, EntityKind::Location);
        let kept = resolve(vec![short, long]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].end, 8);
    }

    #[test]
    fn non_overlapping_spans_kept() {
        let a = cand(0, 5, 0.5, EntityKind::Location);
        let b = cand(6, 10, 0.5, EntityKind::Date);
        let kept = resolve(vec![a, b]);
        assert_eq!(kept.len(), 2);
    }
}
