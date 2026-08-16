#![no_main]

use kika::{Segment2, SegmentIntersection2, segment_intersection};
use libfuzzer_sys::fuzz_target;

#[path = "common.rs"]
mod common;

// Panic-freedom and output-finiteness for `segment_intersection` across
// many small-integer-grid configurations (duplicate/collinear/overlapping
// segments are common on this grid, unlike continuous random floats).
// Correctness of the classification and the `Proper` construction itself
// is already covered by `tests/differential/segment_intersection.rs` and
// `tests/differential/line_intersection.rs`; this target is about crashes
// and NaN/Infinity escaping from finite input, not re-deriving that oracle.
fuzz_target!(|data: &[u8]| {
    let pts = common::points_from(data, 4);
    if pts.len() < 4 {
        return;
    }
    let s1 = Segment2::new(pts[0], pts[1]);
    let s2 = Segment2::new(pts[2], pts[3]);

    if let SegmentIntersection2::Point(p) = segment_intersection(s1, s2) {
        assert!(
            p.x().is_finite() && p.y().is_finite(),
            "Proper intersection point escaped to non-finite for finite input: {s1:?} {s2:?} -> {p:?}"
        );
    }
});
