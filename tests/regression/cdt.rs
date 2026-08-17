//! Minimized regression fixture for a real bug found during development.
//! See `tests/regression/orient2d.rs` for the convention.
//!
//! ## Found: constrained Delaunay segment recovery could oscillate
//! forever instead of converging
//!
//! Phase 6C's `insert_constraint_edge` originally re-scanned every
//! currently-crossing edge each iteration and flipped whichever one
//! appeared first in array-index order among the flippable ones. That is
//! not just a slower version of the standard algorithm (as the code's own
//! doc comment mistakenly described it) -- it isn't guaranteed to
//! terminate. Always re-selecting "whichever crossing edge is first in
//! scan order" can settle into a 2-cycle: flip edge A to its other
//! diagonal, which is still a crossing edge and still happens to sort
//! first next scan, flip it back, repeat, with the crossing-edge count
//! never shrinking.
//!
//! Found via `benches/sanity.rs` (Phase 6D's small-scale sanity
//! benchmarks) on a single, otherwise-unremarkable long constraint in a
//! 300-point random cloud -- every existing unit test used either small
//! (~8 point) grids or short constraints, none long/dense enough to
//! exercise this. `constrained_delaunay2` returned
//! `CdtError::ConstraintInsertionFailed` after exhausting the flip bound,
//! for an input with no degenerate collinearity at all.
//!
//! Fixed by replacing the rescan-and-pick-first approach with a
//! persistent FIFO queue of crossing edges (the actual standard
//! Sloan-style algorithm): pop an edge, flip it if its quad is convex
//! (requeuing the fresh diagonal only if it's still crossing), or push it
//! to the back to retry later if not yet convex. See
//! `src/triangulation/cdt.rs`'s `insert_constraint_edge` doc comment for
//! the correctness argument (a flip only changes the existence of the
//! popped edge and its replacement, so no other edge's crossing status
//! can change -- the queue never needs a full rescan).
//!
//! ## Found: `constrained_delaunay2` panicked on any degenerate point
//! set, even with zero constraints
//!
//! `constrained_delaunay2` called `delaunay2(points)` and then mapped
//! every input point to its `VertexId` by an exhaustive coordinate
//! search ending in `.expect("every input point has a VertexId:
//! duplicates were already rejected")`. `delaunay2` returns an *empty*
//! `Triangulation2` (0 vertices) for its own documented degenerate-input
//! policy (fewer than 3 points, or all points collinear) rather than an
//! error -- so that `.expect` panicked for every point in `points`
//! whenever the point set was degenerate, even when `constraints` was
//! empty and there was nothing at all for a constraint to conflict with.
//! Reproduced with as little as a single point and zero constraints --
//! the smallest possible triggering input, well short of anything
//! involving `constraints`.
//!
//! Fixed by checking `triangulation.is_empty()` immediately after the
//! `delaunay2` call, before the coordinate-search closure is ever
//! called: an empty `constraints` list now returns `Ok` wrapping that
//! same empty triangulation (matching `delaunay2`'s own "degenerate is a
//! valid, representable value" policy), and a non-empty one returns the
//! new `CdtError::DegeneratePointSet` (no triangulation face exists for
//! any constraint to become an edge of). See
//! `src/triangulation/cdt.rs`'s `constrained_delaunay2` and
//! `docs/degeneracy-policy.md`'s CDT table.

use kika::{CdtError, Point2, constrained_delaunay2, validate_cdt_topology};

fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn random_points(seed: u64, n: usize, scale: f64) -> Vec<Point2> {
    let mut state = seed;
    let mut seen = std::collections::HashSet::new();
    let mut pts = Vec::with_capacity(n);
    while pts.len() < n {
        let x = ((xorshift(&mut state) % 1_000_000) as f64 / 1_000_000.0) * scale;
        let y = ((xorshift(&mut state) % 1_000_000) as f64 / 1_000_000.0) * scale;
        if seen.insert((x.to_bits(), y.to_bits())) {
            pts.push(Point2::new(x, y).unwrap());
        }
    }
    pts
}

#[test]
fn long_constraint_in_a_300_point_cloud_does_not_oscillate() {
    let pts = random_points(0xC2B2_AE3D_27D4_EB4F ^ 300u64, 300, 1000.0);
    // Before the fix, this single isolated constraint failed with
    // `ConstraintInsertionFailed` after the rescan-and-pick-first
    // approach oscillated between two triangulation states without ever
    // converging.
    let cdt = constrained_delaunay2(&pts, &[(0, 15)]).expect("must not oscillate/fail");
    assert!(validate_cdt_topology(&cdt).is_empty());
}

#[test]
fn single_point_no_constraints_does_not_panic() {
    let pts = [Point2::new(0.0, 0.0).unwrap()];
    let cdt = constrained_delaunay2(&pts, &[]).expect("must not panic");
    assert!(cdt.triangulation().is_empty());
    assert!(validate_cdt_topology(&cdt).is_empty());
}

#[test]
fn collinear_points_with_a_constraint_is_a_typed_error_not_a_panic() {
    let pts = [
        Point2::new(0.0, 0.0).unwrap(),
        Point2::new(1.0, 0.0).unwrap(),
    ];
    assert_eq!(
        constrained_delaunay2(&pts, &[(0, 1)]),
        Err(CdtError::DegeneratePointSet)
    );
}
