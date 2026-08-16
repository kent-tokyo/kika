use crate::polygon::Polygon2;
use crate::predicates::{Orientation, orient2d};
use crate::primitives::Point2;

/// Whether [`convex_hull2`] keeps points that lie exactly on the hull
/// boundary but are not themselves extreme (i.e. collinear with their two
/// neighbors on the boundary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HullBoundaryPoints {
    /// Drop boundary points that are collinear with their neighbors; every
    /// returned vertex is a genuine corner (strict convexity). Default.
    #[default]
    ExtremesOnly,
    /// Keep every input point that lies on the hull boundary, including
    /// ones collinear with their neighbors.
    KeepAllOnBoundary,
}

/// The 2D convex hull of `points`, via Andrew's monotone chain algorithm.
///
/// Returns a [`Polygon2`] with counterclockwise winding, starting at the
/// lexicographically smallest input point (by `(x, y)`) — both properties
/// are deterministic and do not depend on input order. Duplicate points
/// (exact coordinate equality, matching [`Point2`]'s equality policy) are
/// collapsed before hulling and do not affect the result.
///
/// Degenerate inputs are handled explicitly, not left to fall out of the
/// general algorithm:
/// - 0 or 1 distinct points: returned as-is (a `Polygon2` with that many
///   vertices; not a "hull" in any meaningful sense, but not rejected).
/// - 2 distinct points: both are returned, regardless of `boundary`.
/// - All points exactly collinear: the general lower/upper-chain algorithm
///   is not run. Applied naively, it retraces the same points on both the
///   lower and upper chain (nothing ever triggers a pop in either
///   direction), producing a self-overlapping result like
///   `[A, B, C, D, C, B]` for 4 collinear points. Instead: `ExtremesOnly`
///   returns just the two lexicographic extremes; `KeepAllOnBoundary`
///   returns every distinct point once, in sorted order, with **no**
///   returned-to-start closing point — see the note below.
///
/// # `KeepAllOnBoundary` and self-intersection
///
/// For a fully collinear input in `KeepAllOnBoundary` mode, the returned
/// `Polygon2`'s implicit closing edge (last vertex back to first) retraces
/// the same line as every other edge. [`Polygon2::find_self_intersection`]
/// will report overlaps on such a polygon — this is a real, documented
/// consequence of representing a zero-width "hull" as a vertex ring, not a
/// bug. Callers that need a self-intersection-free hull representation
/// specifically for the fully collinear case should check for it directly
/// (e.g. via [`Polygon2::orientation`] returning `Orientation::Collinear`).
///
/// # Complexity
///
/// O(n log n), dominated by the sort; the chain construction itself is
/// O(n).
pub fn convex_hull2(points: &[Point2], boundary: HullBoundaryPoints) -> Polygon2 {
    let pts = dedup_sorted(points);

    if pts.len() <= 2 {
        return Polygon2::new(pts);
    }

    if is_collinear(&pts) {
        return match boundary {
            HullBoundaryPoints::ExtremesOnly => {
                let last = pts[pts.len() - 1];
                Polygon2::new(vec![pts[0], last])
            }
            HullBoundaryPoints::KeepAllOnBoundary => Polygon2::new(pts),
        };
    }

    let keep_all = boundary == HullBoundaryPoints::KeepAllOnBoundary;

    let mut lower = chain(&pts, keep_all);
    let mut reversed = pts;
    reversed.reverse();
    let mut upper = chain(&reversed, keep_all);

    lower.pop();
    upper.pop();
    lower.extend(upper);

    Polygon2::new(lower)
}

/// Builds one monotone chain (lower, or upper when `pts` is given in
/// reverse) via the standard scan-and-pop construction: append each point,
/// then pop back while the last two hull points and the new point don't
/// make a left (counterclockwise) turn.
///
/// `keep_all = false` (`ExtremesOnly`) pops on `Clockwise` **or**
/// `Collinear`, so only strict corners survive. `keep_all = true`
/// (`KeepAllOnBoundary`) pops only on `Clockwise`, so collinear boundary
/// points are kept.
fn chain(pts: &[Point2], keep_all: bool) -> Vec<Point2> {
    let mut result: Vec<Point2> = Vec::with_capacity(pts.len());
    for &p in pts {
        while result.len() >= 2 {
            let a = result[result.len() - 2];
            let b = result[result.len() - 1];
            let turn = orient2d(a, b, p);
            let should_pop = if keep_all {
                turn == Orientation::Clockwise
            } else {
                turn != Orientation::CounterClockwise
            };
            if should_pop {
                result.pop();
            } else {
                break;
            }
        }
        result.push(p);
    }
    result
}

/// True iff every point in `pts` (already sorted, at least 2 elements) is
/// exactly collinear with the two lexicographic extremes `pts[0]` and
/// `pts[len - 1]`. Since those two points are themselves the sorted set's
/// endpoints, this exactly characterizes "the whole set lies on one line"
/// — no separate direction-vector bookkeeping needed.
///
/// Checked up front, before running the chain construction, rather than
/// inferred afterward from the chains' lengths: a length-based check (e.g.
/// "lower chain used every point") is *not* a reliable signal in
/// `KeepAllOnBoundary` mode — a genuinely 2D "valley" point set (e.g.
/// points on `y = x^2`) legitimately puts every point on the lower chain
/// while the upper chain stays trivial, with no collinearity at all.
fn is_collinear(pts: &[Point2]) -> bool {
    let first = pts[0];
    let last = pts[pts.len() - 1];
    pts.iter()
        .all(|&p| orient2d(first, last, p) == Orientation::Collinear)
}

/// Normalizes `-0.0` to `0.0` so that a `total_cmp`-based sort order agrees
/// with [`Point2`]'s `PartialEq` (which treats `-0.0 == 0.0`, per IEEE-754
/// and ADR-003). Without this, points like `(-0.0, 5.0)` and `(0.0, 5.0)`
/// can sort to non-adjacent positions relative to a third point like
/// `(0.0, 3.0)`, so a plain consecutive-element `dedup()` would miss the
/// duplicate.
fn normalize_zero(v: f64) -> f64 {
    if v == 0.0 { 0.0 } else { v }
}

fn dedup_sorted(points: &[Point2]) -> Vec<Point2> {
    let mut pts: Vec<Point2> = points.to_vec();
    pts.sort_by(|a, b| {
        normalize_zero(a.x())
            .total_cmp(&normalize_zero(b.x()))
            .then(normalize_zero(a.y()).total_cmp(&normalize_zero(b.y())))
    });
    pts.dedup();
    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn is_ccw(vertices: &[Point2]) -> bool {
        Polygon2::new(vertices.to_vec()).orientation() == Orientation::CounterClockwise
    }

    #[test]
    fn empty_input() {
        let hull = convex_hull2(&[], HullBoundaryPoints::ExtremesOnly);
        assert_eq!(hull.vertices(), &[]);
    }

    #[test]
    fn single_point() {
        let hull = convex_hull2(&[p(1.0, 1.0)], HullBoundaryPoints::ExtremesOnly);
        assert_eq!(hull.vertices(), &[p(1.0, 1.0)]);
    }

    #[test]
    fn two_points() {
        let pts = [p(1.0, 1.0), p(2.0, 2.0)];
        for mode in [
            HullBoundaryPoints::ExtremesOnly,
            HullBoundaryPoints::KeepAllOnBoundary,
        ] {
            let hull = convex_hull2(&pts, mode);
            assert_eq!(hull.vertices().len(), 2);
        }
    }

    #[test]
    fn duplicate_points_collapse() {
        let pts = [p(0.0, 0.0), p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0)];
        let hull = convex_hull2(&pts, HullBoundaryPoints::ExtremesOnly);
        assert_eq!(hull.vertices().len(), 3);
    }

    #[test]
    fn signed_zero_duplicate_collapses() {
        // The exact case the sort/dedup mismatch bug would miss: three
        // points where two are equal only via -0.0 == 0.0, and a third
        // point's y-value would otherwise separate them in sort order.
        let pts = [p(-0.0, 5.0), p(0.0, 3.0), p(0.0, 5.0)];
        let hull = convex_hull2(&pts, HullBoundaryPoints::KeepAllOnBoundary);
        assert_eq!(hull.vertices().len(), 2);
    }

    #[test]
    fn square_extremes_only_drops_no_corner() {
        let pts = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        let hull = convex_hull2(&pts, HullBoundaryPoints::ExtremesOnly);
        assert_eq!(hull.vertices().len(), 4);
        assert!(is_ccw(hull.vertices()));
    }

    #[test]
    fn square_with_edge_midpoint_extremes_only_drops_it() {
        let pts = [
            p(0.0, 0.0),
            p(2.0, 0.0), // collinear midpoint of the bottom edge
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
        ];
        let hull = convex_hull2(&pts, HullBoundaryPoints::ExtremesOnly);
        assert_eq!(hull.vertices().len(), 4);
        assert!(!hull.vertices().contains(&p(2.0, 0.0)));
    }

    #[test]
    fn square_with_edge_midpoint_keep_all_keeps_it() {
        let pts = [
            p(0.0, 0.0),
            p(2.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
        ];
        let hull = convex_hull2(&pts, HullBoundaryPoints::KeepAllOnBoundary);
        assert_eq!(hull.vertices().len(), 5);
        assert!(hull.vertices().contains(&p(2.0, 0.0)));
        assert!(is_ccw(hull.vertices()));
    }

    #[test]
    fn interior_point_dropped() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(2.0, 2.0), // strictly interior
        ];
        for mode in [
            HullBoundaryPoints::ExtremesOnly,
            HullBoundaryPoints::KeepAllOnBoundary,
        ] {
            let hull = convex_hull2(&pts, mode);
            assert_eq!(hull.vertices().len(), 4);
            assert!(!hull.vertices().contains(&p(2.0, 2.0)));
        }
    }

    #[test]
    fn fully_collinear_extremes_only() {
        let pts = [p(0.0, 0.0), p(3.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)];
        let hull = convex_hull2(&pts, HullBoundaryPoints::ExtremesOnly);
        assert_eq!(hull.vertices(), &[p(0.0, 0.0), p(3.0, 0.0)]);
    }

    #[test]
    fn fully_collinear_keep_all_no_duplication() {
        let pts = [p(0.0, 0.0), p(3.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)];
        let hull = convex_hull2(&pts, HullBoundaryPoints::KeepAllOnBoundary);
        assert_eq!(
            hull.vertices(),
            &[p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(3.0, 0.0)]
        );
    }

    #[test]
    fn fully_collinear_vertical_line() {
        // Sanity check the collinearity test isn't accidentally axis-specific.
        let pts = [p(5.0, 0.0), p(5.0, 3.0), p(5.0, 1.0)];
        let hull = convex_hull2(&pts, HullBoundaryPoints::ExtremesOnly);
        assert_eq!(hull.vertices(), &[p(5.0, 0.0), p(5.0, 3.0)]);
    }

    #[test]
    fn output_starts_at_lexicographically_smallest_point() {
        let pts = [p(4.0, 4.0), p(0.0, 0.0), p(4.0, 0.0), p(0.0, 4.0)];
        let hull = convex_hull2(&pts, HullBoundaryPoints::ExtremesOnly);
        assert_eq!(hull.vertices()[0], p(0.0, 0.0));
    }

    #[test]
    fn output_is_ccw() {
        // Feed points in clockwise input order; output must still be CCW.
        let pts = [p(0.0, 0.0), p(0.0, 4.0), p(4.0, 4.0), p(4.0, 0.0)];
        let hull = convex_hull2(&pts, HullBoundaryPoints::ExtremesOnly);
        assert!(is_ccw(hull.vertices()));
    }

    #[test]
    fn valley_shape_is_not_treated_as_collinear() {
        // Regression guard for the false-positive heuristic ruled out
        // during design: a "valley" point set can legitimately put every
        // point on one monotone chain without being collinear.
        let pts: Vec<Point2> = (-5..=5).map(|x| p(x as f64, (x * x) as f64)).collect();
        let hull = convex_hull2(&pts, HullBoundaryPoints::KeepAllOnBoundary);
        assert!(hull.vertices().len() > 2);
        assert_ne!(
            Polygon2::new(hull.vertices().to_vec()).orientation(),
            Orientation::Collinear
        );
    }

    #[test]
    fn permutation_invariant() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(2.0, 2.0),
            p(2.0, 0.0),
        ];
        let mut shuffled = pts;
        shuffled.reverse();
        for mode in [
            HullBoundaryPoints::ExtremesOnly,
            HullBoundaryPoints::KeepAllOnBoundary,
        ] {
            let a = convex_hull2(&pts, mode);
            let b = convex_hull2(&shuffled, mode);
            assert_eq!(a.vertices(), b.vertices());
        }
    }

    #[test]
    fn idempotent() {
        let pts = [
            p(0.0, 0.0),
            p(4.0, 0.0),
            p(4.0, 4.0),
            p(0.0, 4.0),
            p(2.0, 2.0),
        ];
        let once = convex_hull2(&pts, HullBoundaryPoints::ExtremesOnly);
        let twice = convex_hull2(once.vertices(), HullBoundaryPoints::ExtremesOnly);
        assert_eq!(once.vertices(), twice.vertices());
    }
}
