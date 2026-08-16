use crate::predicates::{Orientation, line_intersection, orient2d};
use crate::primitives::{Aabb2, Point2, PointSegmentRelation, Segment2};

/// The kind of intersection between two 2D segments, per AGENTS.md §9
/// Phase 2's required classification. This is the *predicate* side
/// (§4.2): computing it never constructs new coordinates or divides —
/// see [`segment_intersection`] for the separate construction side.
///
/// A zero-length input segment is not a separate variant: it is handled
/// explicitly (not assumed to fall out of the general algorithm — see
/// `docs/degeneracy-policy.md` for why that assumption has bitten this
/// project twice already) and folds into `EndpointTouch`/`None`, since a
/// degenerate segment's single point is trivially "its own endpoint".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentIntersectionKind {
    /// The segments share no point.
    None,
    /// A single interior crossing point; neither segment's endpoint.
    Proper,
    /// A single shared point that is an endpoint of at least one segment,
    /// and the segments are not collinear.
    EndpointTouch,
    /// Collinear segments sharing exactly one point.
    CollinearTouch,
    /// Collinear segments overlapping along a sub-segment (more than one
    /// shared point).
    CollinearOverlap,
}

/// The constructed geometry of a segment intersection (§4.2/§8's
/// construction side, separate from [`SegmentIntersectionKind`]'s
/// classification).
///
/// `Point` covers `Proper`, `EndpointTouch`, and `CollinearTouch` alike
/// (all a single point) — coarser than `SegmentIntersectionKind`; use
/// that instead if the distinction matters. For `EndpointTouch` and
/// `CollinearTouch` the point is exactly one of the four input
/// coordinates (no arithmetic, hence exact); for `Proper` it is a
/// certified construction (ADR-004, Phase 5): the `f64` nearest to the
/// true, infinite-precision crossing point, computed via
/// `predicates::line_intersection` — not merely a good approximation, see
/// that function's doc comment for the correctly-rounded-division
/// algorithm and its verified-safe magnitude range (measured wider than
/// `incircle`'s). `Overlap`'s endpoints are likewise exactly two of the
/// four input coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SegmentIntersection2 {
    /// The segments share no point.
    None,
    /// A single shared point (covers `Proper`, `EndpointTouch`, and
    /// `CollinearTouch`).
    Point(Point2),
    /// Collinear segments overlapping along a sub-segment.
    Overlap(Segment2),
}

enum Classification {
    None,
    Proper,
    EndpointTouch(Point2),
    CollinearTouch(Point2),
    CollinearOverlap(Point2, Point2),
}

fn opposite_signs(x: Orientation, y: Orientation) -> bool {
    matches!(
        (x, y),
        (Orientation::Clockwise, Orientation::CounterClockwise)
            | (Orientation::CounterClockwise, Orientation::Clockwise)
    )
}

fn classify(s1: Segment2, s2: Segment2) -> Classification {
    if !Aabb2::from_segment(s1).overlaps(&Aabb2::from_segment(s2)) {
        return Classification::None;
    }

    let (a, b) = (s1.a(), s1.b());
    let (c, d) = (s2.a(), s2.b());

    if s1.is_zero_length() && s2.is_zero_length() {
        return if a == c {
            Classification::EndpointTouch(a)
        } else {
            Classification::None
        };
    }
    if s1.is_zero_length() {
        return match s2.relation_to(a) {
            PointSegmentRelation::NotOnSegment => Classification::None,
            _ => Classification::EndpointTouch(a),
        };
    }
    if s2.is_zero_length() {
        return match s1.relation_to(c) {
            PointSegmentRelation::NotOnSegment => Classification::None,
            _ => Classification::EndpointTouch(c),
        };
    }

    // Both segments have positive length from here on.
    let d1 = orient2d(c, d, a);
    let d2 = orient2d(c, d, b);

    if d1 == Orientation::Collinear && d2 == Orientation::Collinear {
        return collinear_overlap(s1, s2);
    }

    let d3 = orient2d(a, b, c);
    let d4 = orient2d(a, b, d);

    if opposite_signs(d1, d2) && opposite_signs(d3, d4) {
        return Classification::Proper;
    }

    if d1 == Orientation::Collinear && s2.relation_to(a) != PointSegmentRelation::NotOnSegment {
        return Classification::EndpointTouch(a);
    }
    if d2 == Orientation::Collinear && s2.relation_to(b) != PointSegmentRelation::NotOnSegment {
        return Classification::EndpointTouch(b);
    }
    if d3 == Orientation::Collinear && s1.relation_to(c) != PointSegmentRelation::NotOnSegment {
        return Classification::EndpointTouch(c);
    }
    if d4 == Orientation::Collinear && s1.relation_to(d) != PointSegmentRelation::NotOnSegment {
        return Classification::EndpointTouch(d);
    }

    Classification::None
}

/// Both segments already known collinear (both endpoints of `s2` are on
/// the line through `s1`). Projects onto whichever axis actually varies
/// (exact: `s1` has positive length, so exactly one of its x/y
/// coordinates differs between its endpoints) and intersects the two
/// resulting ranges — all direct coordinate comparisons, no arithmetic.
fn collinear_overlap(s1: Segment2, s2: Segment2) -> Classification {
    let (a, b) = (s1.a(), s1.b());
    let (c, d) = (s2.a(), s2.b());
    let use_x = a.x() != b.x();
    let axis = |p: Point2| if use_x { p.x() } else { p.y() };
    let sorted = |p: Point2, q: Point2| if axis(p) <= axis(q) { (p, q) } else { (q, p) };

    let (lo1, hi1) = sorted(a, b);
    let (lo2, hi2) = sorted(c, d);
    let lo = if axis(lo1) >= axis(lo2) { lo1 } else { lo2 };
    let hi = if axis(hi1) <= axis(hi2) { hi1 } else { hi2 };

    if axis(lo) > axis(hi) {
        Classification::None
    } else if axis(lo) == axis(hi) {
        Classification::CollinearTouch(lo)
    } else {
        Classification::CollinearOverlap(lo, hi)
    }
}

/// Classifies the intersection of `s1` and `s2`. Never constructs a new
/// coordinate or divides — see the type's doc comment.
pub fn segment_intersection_kind(s1: Segment2, s2: Segment2) -> SegmentIntersectionKind {
    match classify(s1, s2) {
        Classification::None => SegmentIntersectionKind::None,
        Classification::Proper => SegmentIntersectionKind::Proper,
        Classification::EndpointTouch(_) => SegmentIntersectionKind::EndpointTouch,
        Classification::CollinearTouch(_) => SegmentIntersectionKind::CollinearTouch,
        Classification::CollinearOverlap(_, _) => SegmentIntersectionKind::CollinearOverlap,
    }
}

/// Constructs the intersection geometry of `s1` and `s2`. See
/// [`SegmentIntersection2`]'s doc comment for exactness caveats — only
/// the `Proper` case computes a new coordinate (and divides); every
/// other case reuses an original input point exactly.
pub fn segment_intersection(s1: Segment2, s2: Segment2) -> SegmentIntersection2 {
    match classify(s1, s2) {
        Classification::None => SegmentIntersection2::None,
        Classification::Proper => SegmentIntersection2::Point(proper_intersection_point(s1, s2)),
        Classification::EndpointTouch(p) | Classification::CollinearTouch(p) => {
            SegmentIntersection2::Point(p)
        }
        Classification::CollinearOverlap(p, q) => {
            SegmentIntersection2::Overlap(Segment2::new(p, q))
        }
    }
}

/// Certified line-line intersection (ADR-004). Only reached when
/// `classify` has already established a `Proper` crossing, which
/// guarantees the two lines are non-parallel (opposite-sign straddling on
/// both sides is impossible for parallel lines) — `predicates::line_intersection`'s
/// precondition. See that function's doc comment for the correctly-rounded
/// construction and its magnitude-range limitation; for astronomically
/// extreme, near-parallel inputs the result's finiteness is still not
/// guaranteed — a known, documented gap, not silently assumed away.
fn proper_intersection_point(s1: Segment2, s2: Segment2) -> Point2 {
    line_intersection(s1.a(), s1.b(), s2.a(), s2.b())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }
    fn s(ax: f64, ay: f64, bx: f64, by: f64) -> Segment2 {
        Segment2::new(p(ax, ay), p(bx, by))
    }

    #[test]
    fn proper_crossing() {
        let s1 = s(0.0, 0.0, 4.0, 4.0);
        let s2 = s(0.0, 4.0, 4.0, 0.0);
        assert_eq!(
            segment_intersection_kind(s1, s2),
            SegmentIntersectionKind::Proper
        );
        assert_eq!(
            segment_intersection(s1, s2),
            SegmentIntersection2::Point(p(2.0, 2.0))
        );
    }

    #[test]
    fn disjoint_rejected_by_aabb() {
        let s1 = s(0.0, 0.0, 1.0, 0.0);
        let s2 = s(5.0, 5.0, 6.0, 6.0);
        assert_eq!(
            segment_intersection_kind(s1, s2),
            SegmentIntersectionKind::None
        );
        assert_eq!(segment_intersection(s1, s2), SegmentIntersection2::None);
    }

    #[test]
    fn shared_endpoint() {
        let s1 = s(0.0, 0.0, 1.0, 1.0);
        let s2 = s(0.0, 0.0, 1.0, -1.0);
        assert_eq!(
            segment_intersection_kind(s1, s2),
            SegmentIntersectionKind::EndpointTouch
        );
        assert_eq!(
            segment_intersection(s1, s2),
            SegmentIntersection2::Point(p(0.0, 0.0))
        );
    }

    #[test]
    fn t_junction_touches_interior() {
        let s1 = s(0.0, 0.0, 4.0, 0.0);
        let s2 = s(2.0, 0.0, 2.0, 3.0);
        assert_eq!(
            segment_intersection_kind(s1, s2),
            SegmentIntersectionKind::EndpointTouch
        );
        assert_eq!(
            segment_intersection(s1, s2),
            SegmentIntersection2::Point(p(2.0, 0.0))
        );
    }

    #[test]
    fn collinear_touch_end_to_end() {
        let s1 = s(0.0, 0.0, 2.0, 0.0);
        let s2 = s(2.0, 0.0, 4.0, 0.0);
        assert_eq!(
            segment_intersection_kind(s1, s2),
            SegmentIntersectionKind::CollinearTouch
        );
        assert_eq!(
            segment_intersection(s1, s2),
            SegmentIntersection2::Point(p(2.0, 0.0))
        );
    }

    #[test]
    fn collinear_overlap() {
        let s1 = s(0.0, 0.0, 4.0, 0.0);
        let s2 = s(2.0, 0.0, 6.0, 0.0);
        assert_eq!(
            segment_intersection_kind(s1, s2),
            SegmentIntersectionKind::CollinearOverlap
        );
        assert_eq!(
            segment_intersection(s1, s2),
            SegmentIntersection2::Overlap(s(2.0, 0.0, 4.0, 0.0))
        );
    }

    #[test]
    fn collinear_no_overlap() {
        let s1 = s(0.0, 0.0, 1.0, 0.0);
        let s2 = s(2.0, 0.0, 3.0, 0.0);
        assert_eq!(
            segment_intersection_kind(s1, s2),
            SegmentIntersectionKind::None
        );
    }

    #[test]
    fn parallel_non_collinear_never_intersects() {
        let s1 = s(0.0, 0.0, 4.0, 0.0);
        let s2 = s(0.0, 1.0, 4.0, 1.0);
        assert_eq!(
            segment_intersection_kind(s1, s2),
            SegmentIntersectionKind::None
        );
    }

    #[test]
    fn zero_length_segments() {
        let point_seg = s(2.0, 2.0, 2.0, 2.0);
        let same_point = s(2.0, 2.0, 2.0, 2.0);
        let different_point = s(9.0, 9.0, 9.0, 9.0);
        let through = s(0.0, 0.0, 4.0, 4.0);
        let elsewhere = s(0.0, 0.0, 1.0, 0.0);

        assert_eq!(
            segment_intersection_kind(point_seg, same_point),
            SegmentIntersectionKind::EndpointTouch
        );
        assert_eq!(
            segment_intersection_kind(point_seg, different_point),
            SegmentIntersectionKind::None
        );
        assert_eq!(
            segment_intersection_kind(point_seg, through),
            SegmentIntersectionKind::EndpointTouch
        );
        assert_eq!(
            segment_intersection_kind(point_seg, elsewhere),
            SegmentIntersectionKind::None
        );
    }

    #[test]
    fn symmetry() {
        // segment_intersection_kind(s1, s2) must agree with (s2, s1) for
        // every case exercised above.
        let cases = [
            (s(0.0, 0.0, 4.0, 4.0), s(0.0, 4.0, 4.0, 0.0)),
            (s(0.0, 0.0, 1.0, 1.0), s(0.0, 0.0, 1.0, -1.0)),
            (s(0.0, 0.0, 4.0, 0.0), s(2.0, 0.0, 2.0, 3.0)),
            (s(0.0, 0.0, 2.0, 0.0), s(2.0, 0.0, 4.0, 0.0)),
            (s(0.0, 0.0, 4.0, 0.0), s(2.0, 0.0, 6.0, 0.0)),
        ];
        for (s1, s2) in cases {
            assert_eq!(
                segment_intersection_kind(s1, s2),
                segment_intersection_kind(s2, s1),
                "asymmetric for {s1:?}, {s2:?}"
            );
        }
    }
}
