//! Adversarial and property tests for segment intersection. See
//! `tests/adversarial/orient2d.rs` for the rationale.

use kika::{Point2, Segment2, SegmentIntersectionKind, segment_intersection_kind};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}
fn s(ax: f64, ay: f64, bx: f64, by: f64) -> Segment2 {
    Segment2::new(p(ax, ay), p(bx, by))
}

/// `kind(s1, s2) == kind(s2, s1)` for every non-trivial category.
#[test]
fn symmetric_under_segment_swap() {
    let cases: &[(Segment2, Segment2)] = &[
        (s(0.0, 0.0, 4.0, 4.0), s(0.0, 4.0, 4.0, 0.0)),
        (s(0.0, 0.0, 4.0, 0.0), s(5.0, 5.0, 6.0, 6.0)),
        (s(0.0, 0.0, 1.0, 1.0), s(0.0, 0.0, 1.0, -1.0)),
        (s(0.0, 0.0, 4.0, 0.0), s(2.0, 0.0, 2.0, 3.0)),
        (s(0.0, 0.0, 2.0, 0.0), s(2.0, 0.0, 4.0, 0.0)),
        (s(0.0, 0.0, 4.0, 0.0), s(2.0, 0.0, 6.0, 0.0)),
        (s(0.0, 0.0, 1.0, 0.0), s(2.0, 0.0, 3.0, 0.0)),
    ];
    for &(s1, s2) in cases {
        assert_eq!(
            segment_intersection_kind(s1, s2),
            segment_intersection_kind(s2, s1),
            "asymmetric for {s1:?}, {s2:?}"
        );
    }
}

/// Swapping a single segment's own endpoints must not change the kind.
#[test]
fn invariant_under_endpoint_order_within_a_segment() {
    let cases: &[(Segment2, Segment2)] = &[
        (s(0.0, 0.0, 4.0, 4.0), s(0.0, 4.0, 4.0, 0.0)),
        (s(0.0, 0.0, 4.0, 0.0), s(2.0, 0.0, 2.0, 3.0)),
        (s(0.0, 0.0, 4.0, 0.0), s(2.0, 0.0, 6.0, 0.0)),
    ];
    for &(s1, s2) in cases {
        let flipped1 = Segment2::new(s1.b(), s1.a());
        let flipped2 = Segment2::new(s2.b(), s2.a());
        assert_eq!(
            segment_intersection_kind(s1, s2),
            segment_intersection_kind(flipped1, s2)
        );
        assert_eq!(
            segment_intersection_kind(s1, s2),
            segment_intersection_kind(s1, flipped2)
        );
    }
}

/// Translation by an exactly-representable vector must not change the
/// kind (power-of-two coordinates/translations: no new rounding).
#[test]
fn translation_invariance() {
    let s1 = s(0.0, 0.0, 8.0, 8.0);
    let s2 = s(0.0, 8.0, 8.0, 0.0);
    let want = segment_intersection_kind(s1, s2);
    for &(tx, ty) in &[(32.0, 0.0), (0.0, -64.0), (16.0, 16.0)] {
        let t = |seg: Segment2| {
            Segment2::new(
                p(seg.a().x() + tx, seg.a().y() + ty),
                p(seg.b().x() + tx, seg.b().y() + ty),
            )
        };
        assert_eq!(
            segment_intersection_kind(t(s1), t(s2)),
            want,
            "translation ({tx},{ty})"
        );
    }
}

#[test]
fn zero_length_both_same_point() {
    let a = s(3.0, 3.0, 3.0, 3.0);
    let b = s(3.0, 3.0, 3.0, 3.0);
    assert_eq!(
        segment_intersection_kind(a, b),
        SegmentIntersectionKind::EndpointTouch
    );
}

#[test]
fn near_subnormal_scale_does_not_panic() {
    let scale = 1e-140_f64;
    let s1 = s(0.0, 0.0, scale, scale);
    let s2 = s(0.0, scale, scale, 0.0);
    let _ = segment_intersection_kind(s1, s2);
}

#[test]
fn extreme_large_scale_does_not_panic() {
    let scale = 1e140_f64;
    let s1 = s(0.0, 0.0, scale, scale);
    let s2 = s(0.0, scale, scale, 0.0);
    let _ = segment_intersection_kind(s1, s2);
}
