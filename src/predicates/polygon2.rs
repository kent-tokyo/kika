use super::expansion::{expansion_sign, expansion_sum, merge_all, product_expansion};
use super::sign::Orientation;
use crate::primitives::Point2;

/// The exact sign of (twice) a polygon's signed area — i.e. its winding
/// orientation — for `vertices` taken as an implicitly-closed ring
/// (`vertices[i]` to `vertices[(i+1) % n]`, including the wraparound
/// edge). `Sign::Positive` is counterclockwise, `Sign::Negative` is
/// clockwise, `Sign::Zero` covers every degenerate case (fewer than 3
/// vertices, all vertices collinear, or a self-canceling — e.g.
/// figure-eight — vertex order).
///
/// Exact: every edge's shoelace term (`x_i*y_j - x_j*y_i`) is built as an
/// exact 2-component expansion (the same `diff_expansion`/
/// `product_of_expansions` building blocks `orient2d` etc. use), and all
/// `n` terms are combined via [`merge_all`]'s balanced-tree merge before
/// taking the sign of the leading component — never a running `f64` sum,
/// which could round through cancellation for near-degenerate polygons.
///
/// ponytail: unlike the four core predicates, this has no fast
/// floating-point filter — every call goes straight to the exact
/// expansion sum. `merge_all` is already O(n log n), not O(n²), so this
/// is not the naive-quadratic mistake documented in
/// `docs/numerical-model.md`'s "naive expansion merging" section; it is
/// a deliberate simplification (skip a filter this codebase doesn't yet
/// need) with a clear upgrade path: add a filtered `f64` shoelace sum
/// with a computed error bound ahead of this, gated on measured need
/// (§13), if profiling ever shows the exact path dominates for
/// large polygons.
pub(crate) fn polygon_orientation(vertices: &[Point2]) -> Orientation {
    let n = vertices.len();
    if n < 3 {
        return Orientation::Collinear;
    }

    let terms: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            let j = (i + 1) % n;
            let (xi, yi) = (vertices[i].x(), vertices[i].y());
            let (xj, yj) = (vertices[j].x(), vertices[j].y());
            let pos = product_expansion(xi, yj);
            let neg = product_expansion(xj, yi);
            expansion_sum(&pos, &[-neg[0], -neg[1]])
        })
        .collect();

    let total = merge_all(terms);
    Orientation::from(expansion_sign(&total))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).unwrap()
    }

    #[test]
    fn ccw_square() {
        let v = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
        assert_eq!(polygon_orientation(&v), Orientation::CounterClockwise);
    }

    #[test]
    fn cw_square() {
        let v = [p(0.0, 0.0), p(0.0, 4.0), p(4.0, 4.0), p(4.0, 0.0)];
        assert_eq!(polygon_orientation(&v), Orientation::Clockwise);
    }

    #[test]
    fn too_few_vertices_is_collinear() {
        assert_eq!(polygon_orientation(&[]), Orientation::Collinear);
        assert_eq!(polygon_orientation(&[p(0.0, 0.0)]), Orientation::Collinear);
        assert_eq!(
            polygon_orientation(&[p(0.0, 0.0), p(1.0, 1.0)]),
            Orientation::Collinear
        );
    }

    #[test]
    fn all_collinear_vertices_is_collinear() {
        let v = [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(3.0, 0.0)];
        assert_eq!(polygon_orientation(&v), Orientation::Collinear);
    }
}
