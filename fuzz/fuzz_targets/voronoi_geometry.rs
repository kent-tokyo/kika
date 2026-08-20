#![no_main]

use kika::{Point2, VoronoiEdgeGeometry, delaunay2, voronoi2};
use libfuzzer_sys::fuzz_target;

fn f64_from(bytes: &[u8]) -> f64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(bytes);
    f64::from_bits(u64::from_le_bytes(buf))
}

// Voronoi geometry fuzz target (0.7.0, ADR-009 hardening round):
// exercises `Voronoi2::vertex_point`/`edge_geometry` across raw
// `f64::from_bits` coordinate patterns -- like `predicate_input_bytes`
// (not the small-integer grid `common.rs` uses for the
// combinatorial-topology targets), since this stresses
// magnitude/bit-pattern diversity rather than degenerate configurations.
// Checks exactly the invariant this hardening round is supposed to
// guarantee: every produced coordinate is finite, and a `Ray`'s
// `direction` is never the zero vector. `Err(VoronoiGeometryError)` is
// an accepted, correct outcome -- not every input has a representable
// circumcenter -- only a panic, or a non-finite/zero value slipping past
// a `Result::Ok`, counts as a failure.
fuzz_target!(|data: &[u8]| {
    let coords: Vec<f64> = data.chunks_exact(8).map(f64_from).take(60).collect();
    let pts: Vec<Point2> = coords
        .chunks_exact(2)
        .filter_map(|c| Point2::new(c[0], c[1]).ok())
        .take(30)
        .collect();
    if pts.len() < 3 {
        return;
    }

    let voronoi = voronoi2(delaunay2(&pts));

    for vertex in voronoi.vertices() {
        if let Ok(p) = voronoi.vertex_point(vertex) {
            assert!(
                p.x().is_finite() && p.y().is_finite(),
                "non-finite vertex_point: {p:?} (points {pts:?})"
            );
        }
    }

    for edge in voronoi.edges() {
        match voronoi.edge_geometry(edge) {
            Ok(VoronoiEdgeGeometry::Segment { start, end }) => {
                assert!(
                    start.x().is_finite() && start.y().is_finite(),
                    "non-finite Segment start: {start:?} (points {pts:?})"
                );
                assert!(
                    end.x().is_finite() && end.y().is_finite(),
                    "non-finite Segment end: {end:?} (points {pts:?})"
                );
            }
            Ok(VoronoiEdgeGeometry::Ray { origin, direction }) => {
                assert!(
                    origin.x().is_finite() && origin.y().is_finite(),
                    "non-finite Ray origin: {origin:?} (points {pts:?})"
                );
                assert!(
                    direction.x().is_finite() && direction.y().is_finite(),
                    "non-finite Ray direction: {direction:?} (points {pts:?})"
                );
                assert!(
                    direction.x() != 0.0 || direction.y() != 0.0,
                    "Ray direction must not be the zero vector (points {pts:?})"
                );
            }
            // `VoronoiEdgeGeometry` is `#[non_exhaustive]`, but only these
            // 2 variants exist today.
            Ok(_) => unreachable!("unexpected VoronoiEdgeGeometry variant"),
            Err(_) => {} // an accepted rejection, not a failure
        }
    }
});
