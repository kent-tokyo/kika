#![no_main]

use kika::delaunay2;
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

// Topology validator: exercises `Triangulation2::validate_topology`
// (§6B, ADR-006) -- CCW faces, edge-manifold incidence (every edge shared
// by exactly 1 or 2 triangles), adjacency reciprocity, Euler's formula,
// and local-Delaunay -- across many small-integer-grid point clouds. This
// used to duplicate the CCW/edge-count/Euler checks by hand; now that
// `validate_topology` exists, calling it directly is both less code and
// broader coverage (adjacency reciprocity and local-Delaunay weren't
// checked here before).
fuzz_target!(|data: &[u8]| {
    let pts = common::points_from(data, 30);
    if pts.len() < 3 {
        return;
    }

    let triangulation = delaunay2(&pts);
    let errors = triangulation.validate_topology();
    assert!(
        errors.is_empty(),
        "validate_topology found violations: {errors:?} (points {pts:?})"
    );
});
