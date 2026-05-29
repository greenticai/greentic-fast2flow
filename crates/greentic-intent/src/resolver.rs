//! Overlap resolution — pick a non-overlapping subset of entity candidates.
//!
//! Rule chain (from the design doc):
//!   1. Higher confidence wins.
//!   2. Longer span wins.
//!   3. Higher kind priority wins (see [`kind_priority`]).
//!   4. Extractor priority — caller-supplied order; not yet plumbed
//!      through [`EntityCandidate`], so rule 4 is currently a no-op.

use crate::entity::EntityKind;
use crate::extractors::EntityCandidate;

/// Pick a non-overlapping subset of `candidates`.
pub fn resolve(mut candidates: Vec<EntityCandidate>) -> Vec<EntityCandidate> {
    candidates.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| (b.end - b.start).cmp(&(a.end - a.start)))
            .then_with(|| kind_priority(b.kind).cmp(&kind_priority(a.kind)))
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

/// Per-kind priority used as rule 3 of the overlap chain. Higher wins.
/// Order mirrors the design doc (top to bottom): email, url, phone,
/// datetime, date, time, address, money, id, location, person,
/// organisation, number. `Duration` is not in the design doc list and
/// sits at the bottom.
pub fn kind_priority(kind: EntityKind) -> u8 {
    match kind {
        EntityKind::Email => 13,
        EntityKind::Url => 12,
        EntityKind::Phone => 11,
        EntityKind::DateTime => 10,
        EntityKind::Date => 9,
        EntityKind::Time => 8,
        EntityKind::Address => 7,
        EntityKind::Money => 6,
        EntityKind::Id => 5,
        EntityKind::Location => 4,
        EntityKind::Person => 3,
        EntityKind::Organisation => 2,
        EntityKind::Number => 1,
        EntityKind::Duration => 0,
    }
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

    #[test]
    fn kind_priority_breaks_tie_on_confidence_and_span() {
        // Same span + confidence: Email (13) beats Location (4).
        let email = cand(0, 10, 0.9, EntityKind::Email);
        let location = cand(0, 10, 0.9, EntityKind::Location);
        let kept = resolve(vec![location, email]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].kind, EntityKind::Email);
    }

    #[test]
    fn date_beats_time_on_kind_priority() {
        let date = cand(0, 8, 0.9, EntityKind::Date);
        let time = cand(0, 8, 0.9, EntityKind::Time);
        let kept = resolve(vec![time, date]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].kind, EntityKind::Date);
    }

    #[test]
    fn confidence_still_dominates_kind_priority() {
        // Number (1) at high confidence still beats Email (13) at low confidence.
        let number = cand(0, 5, 0.95, EntityKind::Number);
        let email = cand(0, 5, 0.7, EntityKind::Email);
        let kept = resolve(vec![email, number]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].kind, EntityKind::Number);
    }

    #[test]
    fn longer_span_still_beats_kind_priority_when_confidence_equal() {
        // Span trumps kind: a 10-byte Number beats a 5-byte Email at equal conf.
        let number = cand(0, 10, 0.9, EntityKind::Number);
        let email = cand(0, 5, 0.9, EntityKind::Email);
        let kept = resolve(vec![email, number]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].kind, EntityKind::Number);
    }

    #[test]
    fn kind_priority_table_matches_design_doc_order() {
        // Spot-check a few rungs of the ladder.
        assert!(kind_priority(EntityKind::Email) > kind_priority(EntityKind::Url));
        assert!(kind_priority(EntityKind::Url) > kind_priority(EntityKind::Phone));
        assert!(kind_priority(EntityKind::DateTime) > kind_priority(EntityKind::Date));
        assert!(kind_priority(EntityKind::Date) > kind_priority(EntityKind::Time));
        assert!(kind_priority(EntityKind::Location) > kind_priority(EntityKind::Person));
        assert!(kind_priority(EntityKind::Person) > kind_priority(EntityKind::Organisation));
        assert!(kind_priority(EntityKind::Organisation) > kind_priority(EntityKind::Number));
    }
}
