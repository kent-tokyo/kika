#![no_main]

use kika::{PointLocation, PointSegmentRelation, PointTriangleRelation, Segment2, delaunay2};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

// Point location (0.6.0, ADR-008): exercises Triangulation2::locate
// across many small-integer-grid point clouds and query points -- never
// panics, and its result agrees with the same primitives it's built
// from (Triangle2::relation_to / Segment2::relation_to). This is the
// same self-consistency property tests/differential/locate.rs checks
// against an independent BigRational oracle at a small, fixed scale;
// fuzzing explores far more input combinations, cheaply, using the
// crate's own (already independently verified) primitives instead.
fuzz_target!(|data: &[u8]| {
    let pts = common::points_from(data, 30);
    if pts.len() < 3 {
        return;
    }
    let queries = common::points_from(&data[data.len() / 2..], 10);

    let t = delaunay2(&pts);
    for q in queries.iter().chain(pts.iter()) {
        match t.locate(*q) {
            PointLocation::Vertex(id) => {
                let (_, p) = t.vertices().find(|&(vid, _)| vid == id).unwrap();
                assert_eq!(p, *q, "Vertex postcondition violated for {q:?}");
            }
            PointLocation::Edge(id) => {
                let (u, v) = t.edge_vertices(id);
                let pu = t.vertices().find(|&(vid, _)| vid == u).unwrap().1;
                let pv = t.vertices().find(|&(vid, _)| vid == v).unwrap().1;
                assert_ne!(
                    Segment2::new(pu, pv).relation_to(*q),
                    PointSegmentRelation::NotOnSegment,
                    "Edge postcondition violated for {q:?}"
                );
            }
            PointLocation::Face(id) => {
                let idx = t.faces().position(|f| f == id).unwrap();
                assert_eq!(
                    t.triangles()[idx].relation_to(*q),
                    PointTriangleRelation::Inside,
                    "Face postcondition violated for {q:?}"
                );
            }
            PointLocation::Outside => {
                assert!(
                    t.triangles()
                        .iter()
                        .all(|tri| tri.relation_to(*q) == PointTriangleRelation::Outside),
                    "Outside postcondition violated for {q:?}"
                );
            }
        }
    }
});
