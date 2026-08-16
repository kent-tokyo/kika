use crate::intersections::{SegmentIntersectionKind, segment_intersection_kind};
use crate::predicates::{Orientation, polygon_orientation};
use crate::primitives::{Point2, Segment2};

/// A 2D polygon ring: `vertices[i]` to `vertices[(i+1) % n]`, including
/// the wraparound edge back to `vertices[0]` — **implicitly** closed
/// (unlike e.g. GeoJSON, the first vertex is not repeated at the end).
///
/// No validation at construction: an empty, single-point, self-crossing,
/// or otherwise degenerate vertex list is a valid, representable
/// `Polygon2`, matching every other primitive type in this crate
/// (`Segment2`, `Triangle2`). Use [`Polygon2::basic_validity`] and
/// [`Polygon2::find_self_intersection`] to check, explicitly, for the
/// properties you need.
#[derive(Debug, Clone, PartialEq)]
pub struct Polygon2 {
    vertices: Vec<Point2>,
}

/// The outcome of [`Polygon2::basic_validity`] — the *cheap* structural
/// checks (§9 Phase 2 "polygon基本validity検査"), not the separate,
/// O(n²) [`Polygon2::find_self_intersection`] check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolygonBasicValidity {
    /// Fewer than 3 vertices: cannot enclose any area.
    TooFewVertices,
    /// Two consecutive vertices (including the wraparound edge) are
    /// exactly equal — a zero-length edge.
    ConsecutiveDuplicateVertices,
    /// At least 3 vertices, no consecutive duplicates, but the exact
    /// signed area is zero anyway (e.g. all vertices collinear).
    ZeroArea,
    /// None of the above cheap checks found a problem.
    Valid,
}

/// A found self-intersection, from [`Polygon2::find_self_intersection`]:
/// the two (non-adjacent) edge indices and the kind of intersection
/// between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolygonSelfIntersection {
    /// Index of the first edge (`edge_a` to `edge_a + 1`).
    pub edge_a: usize,
    /// Index of the second edge.
    pub edge_b: usize,
    /// How `edge_a` and `edge_b` intersect.
    pub kind: SegmentIntersectionKind,
}

impl Polygon2 {
    /// Creates a polygon ring from `vertices`, in order. No validation —
    /// see the type's doc comment.
    pub fn new(vertices: Vec<Point2>) -> Self {
        Polygon2 { vertices }
    }

    /// The ring's vertices, in order.
    pub fn vertices(&self) -> &[Point2] {
        &self.vertices
    }

    /// The number of vertices.
    pub fn len(&self) -> usize {
        self.vertices.len()
    }

    /// `true` iff there are no vertices.
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// The edge from vertex `i` to vertex `(i+1) % len()`. Panics if
    /// `i >= self.len()` or `self.len() < 2` (mirrors slice indexing —
    /// this is an internal-shape query, not a boundary predicate; callers
    /// with a possibly-too-short polygon should check `len()` first, the
    /// same way they'd check before indexing a slice).
    pub fn edge(&self, i: usize) -> Segment2 {
        let n = self.vertices.len();
        Segment2::new(self.vertices[i], self.vertices[(i + 1) % n])
    }

    /// The polygon's signed area (positive for counterclockwise winding,
    /// negative for clockwise), via the ordinary (not exact) shoelace
    /// formula. A plain numeric *construction*, not a predicate — for the
    /// exact winding *sign*, use [`Polygon2::orientation`] instead of
    /// comparing this against `0.0` (this can round through cancellation
    /// for near-degenerate polygons the same way any `f64` sum can; see
    /// `docs/numerical-model.md`).
    pub fn signed_area(&self) -> f64 {
        let n = self.vertices.len();
        if n < 3 {
            return 0.0;
        }
        let mut sum = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            sum += self.vertices[i].x() * self.vertices[j].y()
                - self.vertices[j].x() * self.vertices[i].y();
        }
        sum * 0.5
    }

    /// The exact sign of the polygon's signed area — its winding.
    /// `Orientation::Collinear` covers every degenerate case (fewer than
    /// 3 vertices, all vertices collinear, or a self-canceling vertex
    /// order), not just literal collinearity — see
    /// `predicates::polygon_orientation`'s doc comment for the exact
    /// arithmetic behind this.
    pub fn orientation(&self) -> Orientation {
        polygon_orientation(&self.vertices)
    }

    /// The cheap structural validity checks — see
    /// [`PolygonBasicValidity`]. Does **not** check for self-intersection
    /// (that's the separate, more expensive
    /// [`Polygon2::find_self_intersection`]).
    pub fn basic_validity(&self) -> PolygonBasicValidity {
        let n = self.vertices.len();
        if n < 3 {
            return PolygonBasicValidity::TooFewVertices;
        }
        for i in 0..n {
            let j = (i + 1) % n;
            if self.vertices[i] == self.vertices[j] {
                return PolygonBasicValidity::ConsecutiveDuplicateVertices;
            }
        }
        if self.orientation() == Orientation::Collinear {
            return PolygonBasicValidity::ZeroArea;
        }
        PolygonBasicValidity::Valid
    }

    /// Finds a self-intersection between two non-adjacent edges, if any
    /// (the first one found in edge-index order; not necessarily the
    /// "first" in any geometric sense). Adjacent edges (which always
    /// share exactly one endpoint, by construction) are correctly
    /// excluded — that shared vertex is expected, not a self-intersection
    /// — including the wraparound adjacency between the first and last
    /// edge. O(n²): every non-adjacent edge pair is checked once; no
    /// attempt at a sweep-line speedup in Phase 2 (§9's own guidance:
    /// prioritize correctness over performance at this stage).
    pub fn find_self_intersection(&self) -> Option<PolygonSelfIntersection> {
        let n = self.vertices.len();
        if n < 2 {
            return None;
        }
        for i in 0..n {
            for j in (i + 1)..n {
                let adjacent = j == i + 1 || (i == 0 && j == n - 1);
                if adjacent {
                    continue;
                }
                let kind = segment_intersection_kind(self.edge(i), self.edge(j));
                if kind != SegmentIntersectionKind::None {
                    return Some(PolygonSelfIntersection {
                        edge_a: i,
                        edge_b: j,
                        kind,
                    });
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    fn square_ccw() -> Polygon2 {
        Polygon2::new(vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)])
    }

    #[test]
    fn signed_area_ccw_positive_cw_negative() {
        let ccw = square_ccw();
        assert_eq!(ccw.signed_area(), 16.0);
        let cw = Polygon2::new(vec![p(0.0, 0.0), p(0.0, 4.0), p(4.0, 4.0), p(4.0, 0.0)]);
        assert_eq!(cw.signed_area(), -16.0);
    }

    #[test]
    fn orientation_matches_winding() {
        assert_eq!(square_ccw().orientation(), Orientation::CounterClockwise);
    }

    #[test]
    fn basic_validity_too_few_vertices() {
        assert_eq!(
            Polygon2::new(vec![]).basic_validity(),
            PolygonBasicValidity::TooFewVertices
        );
        assert_eq!(
            Polygon2::new(vec![p(0.0, 0.0), p(1.0, 1.0)]).basic_validity(),
            PolygonBasicValidity::TooFewVertices
        );
    }

    #[test]
    fn basic_validity_consecutive_duplicate() {
        let poly = Polygon2::new(vec![p(0.0, 0.0), p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0)]);
        assert_eq!(
            poly.basic_validity(),
            PolygonBasicValidity::ConsecutiveDuplicateVertices
        );
    }

    #[test]
    fn basic_validity_wraparound_duplicate() {
        // Last vertex equals the first: the wraparound edge is zero-length.
        let poly = Polygon2::new(vec![p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0), p(0.0, 0.0)]);
        assert_eq!(
            poly.basic_validity(),
            PolygonBasicValidity::ConsecutiveDuplicateVertices
        );
    }

    #[test]
    fn basic_validity_zero_area_collinear() {
        let poly = Polygon2::new(vec![p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)]);
        assert_eq!(poly.basic_validity(), PolygonBasicValidity::ZeroArea);
    }

    #[test]
    fn basic_validity_valid_triangle() {
        let poly = Polygon2::new(vec![p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0)]);
        assert_eq!(poly.basic_validity(), PolygonBasicValidity::Valid);
    }

    #[test]
    fn triangle_never_self_intersects() {
        let poly = Polygon2::new(vec![p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0)]);
        assert_eq!(poly.find_self_intersection(), None);
    }

    #[test]
    fn simple_square_does_not_self_intersect() {
        assert_eq!(square_ccw().find_self_intersection(), None);
    }

    #[test]
    fn bowtie_quadrilateral_self_intersects() {
        // (0,0) -> (4,4) -> (4,0) -> (0,4) -> back to (0,0): edges 0 and 2
        // (the two diagonals) cross.
        let poly = Polygon2::new(vec![p(0.0, 0.0), p(4.0, 4.0), p(4.0, 0.0), p(0.0, 4.0)]);
        let found = poly
            .find_self_intersection()
            .expect("bowtie must self-intersect");
        assert_eq!(found.edge_a, 0);
        assert_eq!(found.edge_b, 2);
        assert_eq!(found.kind, SegmentIntersectionKind::Proper);
    }

    #[test]
    fn adjacent_edges_shared_vertex_is_not_reported() {
        // A convex polygon's adjacent edges always touch at their shared
        // vertex; that must never be reported as a self-intersection.
        assert_eq!(square_ccw().find_self_intersection(), None);
        let triangle = Polygon2::new(vec![p(0.0, 0.0), p(1.0, 0.0), p(0.5, 1.0)]);
        assert_eq!(triangle.find_self_intersection(), None);
    }
}
