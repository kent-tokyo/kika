#![no_main]

use kika::{Orientation, delaunay2};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

// Structural invariants from AGENTS.md §11: every produced triangle must
// be counterclockwise (never degenerate/clockwise), and every triangle
// vertex must trace back to an input point. Heavy on duplicate/collinear/
// cocircular configurations by construction (`common::points_from`'s
// small-integer grid) — exactly the class of input that found the real
// super-triangle bug during Phase 4 (a plain 4-point, non-adversarial
// input), so a fuzzer covering many such configurations quickly is a
// natural extension of that same discipline, not a new one.
fuzz_target!(|data: &[u8]| {
    let pts = common::points_from(data, 30);
    if pts.len() < 3 {
        return;
    }

    let triangulation = delaunay2(&pts);
    for tri in triangulation.triangles() {
        assert_eq!(
            tri.orientation(),
            Orientation::CounterClockwise,
            "non-CCW triangle {tri:?} in triangulation of {pts:?}"
        );
        for v in [tri.a(), tri.b(), tri.c()] {
            assert!(
                pts.contains(&v),
                "triangle vertex {v:?} is not one of the input points {pts:?}"
            );
        }
    }
});
