#![no_main]

use kika::{Point2, Point3, incircle, insphere, orient2d, orient3d};
use libfuzzer_sys::fuzz_target;

/// AGENTS.md §12's "predicate input bytes" target: raw bit patterns via
/// `f64::from_bits`, not the curated small-integer grid `common.rs` uses
/// for the combinatorial-algorithm targets -- covers `NaN`, infinity,
/// subnormals, and the full magnitude range on the same footing.
/// `Point2::new`/`Point3::new` reject non-finite coordinates (`Result`),
/// so this simultaneously fuzzes that validation and, for whatever
/// coordinates survive it, the predicates built on top -- unlike the
/// grid-based targets (which stress degenerate/duplicate/cocircular
/// *configurations*), this one stresses raw magnitude/bit-pattern
/// diversity. Checks: never panics. Correctness on curated inputs is
/// already covered by `tests/differential`'s oracle comparisons; a
/// fuzzer exploring arbitrary bit patterns is for finding panics, not
/// wrong-but-non-crashing answers.
fn f64_from(bytes: &[u8]) -> f64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(bytes);
    f64::from_bits(u64::from_le_bytes(buf))
}

fuzz_target!(|data: &[u8]| {
    let coords: Vec<f64> = data.chunks_exact(8).map(f64_from).take(30).collect();

    let pts2: Vec<Point2> = coords
        .chunks_exact(2)
        .filter_map(|c| Point2::new(c[0], c[1]).ok())
        .collect();
    if pts2.len() >= 3 {
        let _ = orient2d(pts2[0], pts2[1], pts2[2]);
    }
    if pts2.len() >= 4 {
        let _ = incircle(pts2[0], pts2[1], pts2[2], pts2[3]);
    }

    let pts3: Vec<Point3> = coords
        .chunks_exact(3)
        .filter_map(|c| Point3::new(c[0], c[1], c[2]).ok())
        .collect();
    if pts3.len() >= 4 {
        let _ = orient3d(pts3[0], pts3[1], pts3[2], pts3[3]);
    }
    if pts3.len() >= 5 {
        let _ = insphere(pts3[0], pts3[1], pts3[2], pts3[3], pts3[4]);
    }
});
