use kika::Point2;

/// Maps a fuzzer-supplied byte to a small-integer grid coordinate.
///
/// Chosen over mapping bytes to full-range `f64`s: continuous random floats
/// almost never coincide, so a naive byte-to-float mapping would almost
/// never generate the duplicate/collinear/cocircular configurations that
/// actually stress combinatorial algorithm logic (convex hull, Delaunay,
/// triangulation topology) — the class of bug this fuzzing round targets
/// (AGENTS.md §12); raw predicate magnitude/exactness edge cases are
/// already covered by `tests/differential`. A small integer grid makes
/// those degenerate configurations common instead of vanishingly rare.
fn coord(byte: u8) -> f64 {
    (byte % 33) as f64 - 16.0
}

/// Up to `max` points built from `data`, 2 bytes per point. `Point2::new`
/// cannot fail on this bounded integer grid, but the `filter_map` stays
/// defensive rather than assuming that.
pub fn points_from(data: &[u8], max: usize) -> Vec<Point2> {
    data.chunks_exact(2)
        .filter_map(|c| Point2::new(coord(c[0]), coord(c[1])).ok())
        .take(max)
        .collect()
}
