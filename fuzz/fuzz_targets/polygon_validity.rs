#![no_main]

use kika::{Polygon2, triangulate_polygon, triangulate_polygon_with_holes};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

// Polygon validity + triangulation robustness (AGENTS.md §12's "polygon
// validity" target). `Polygon2::basic_validity`/`find_self_intersection`/
// `relation_to`, and `triangulate_polygon`/`triangulate_polygon_with_holes`
// (0.4.0), must never panic on any input; any `Ok` triangulation must
// satisfy its own documented triangle-count invariant (`n - 2`, or
// `n + 2h - 2` with `h` holes) rather than just "didn't crash" -- exercised
// across many small-integer-grid point clouds (see `common::points_from`'s
// doc comment for why a grid, not raw floats), split into an outer ring
// candidate plus zero or more hole ring candidates.
fuzz_target!(|data: &[u8]| {
    let pts = common::points_from(data, 60);
    if pts.len() < 3 {
        return;
    }

    let mut chunks = pts.chunks(8);
    let Some(outer_pts) = chunks.next() else {
        return;
    };
    if outer_pts.len() < 3 {
        return;
    }
    let outer = Polygon2::new(outer_pts.to_vec());

    let _ = outer.basic_validity();
    let _ = outer.find_self_intersection();
    for &p in &pts {
        let _ = outer.relation_to(p);
    }

    if let Ok(t) = triangulate_polygon(&outer) {
        assert_eq!(
            t.len(),
            outer.len() - 2,
            "wrong triangle count for outer={outer_pts:?}"
        );
    }

    let holes: Vec<Polygon2> = chunks
        .filter(|c| c.len() >= 3)
        .map(|c| Polygon2::new(c.to_vec()))
        .collect();
    if holes.is_empty() {
        return;
    }

    let n: usize = outer.len() + holes.iter().map(|h| h.len()).sum::<usize>();
    if let Ok(t) = triangulate_polygon_with_holes(&outer, &holes) {
        assert_eq!(
            t.len(),
            n + 2 * holes.len() - 2,
            "wrong triangle count for outer={outer_pts:?} holes={holes:?}"
        );
    }
});
