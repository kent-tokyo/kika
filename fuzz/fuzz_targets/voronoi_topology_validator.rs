#![no_main]

use kika::{delaunay2, voronoi2};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

// Voronoi topology validator (0.5.0, ADR-007): exercises
// `Voronoi2::validate_voronoi_topology` -- face-group/edge invariants,
// canonical ordering, and cell_edges()'s coverage check -- across many
// small-integer-grid point clouds. The integer grid (shared with
// `triangulation_topology_validator`) is what makes this target useful
// specifically for Voronoi: it produces cocircular and collinear
// configurations often, which is exactly what stresses the cocircular
// tie-break union-find grouping this module exists to normalize.
fuzz_target!(|data: &[u8]| {
    let pts = common::points_from(data, 30);
    if pts.len() < 3 {
        return;
    }

    let voronoi = voronoi2(delaunay2(&pts));
    let errors = voronoi.validate_voronoi_topology();
    assert!(
        errors.is_empty(),
        "validate_voronoi_topology found violations: {errors:?} (points {pts:?})"
    );
});
