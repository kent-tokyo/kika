#![no_main]

use kika::{HullBoundaryPoints, convex_hull2};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

// Structural invariants from AGENTS.md §11's property-test list: every
// returned hull vertex must be one of the (deduplicated) input points, and
// the vertex count can never exceed the input's. Heavy on duplicate and
// collinear configurations by construction (see `common::points_from`),
// which is exactly the class of input this crate's own design notes
// (`tasks/lessons.md`) record as having caused real bugs during
// hand-written test design — a fuzzer explores far more of that space than
// any hand-written case list.
fuzz_target!(|data: &[u8]| {
    let pts = common::points_from(data, 40);
    if pts.len() < 3 {
        return;
    }

    for boundary in [
        HullBoundaryPoints::ExtremesOnly,
        HullBoundaryPoints::KeepAllOnBoundary,
    ] {
        let hull = convex_hull2(&pts, boundary);
        assert!(
            hull.vertices().len() <= pts.len(),
            "hull produced more vertices than input points: {:?}",
            hull.vertices()
        );
        for v in hull.vertices() {
            assert!(
                pts.contains(v),
                "hull vertex {v:?} is not one of the input points {pts:?}"
            );
        }
    }
});
