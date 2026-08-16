//! Differential tests for `orient2d` against an independent exact-rational
//! oracle (num-bigint/num-rational, dev-dependency only — never used by
//! `src/`, per ADR-005). This is a *small* CI-tier differential suite
//! (AGENTS.md §11 "常時CI: 小規模差分テスト"); large-scale random
//! differential testing is nightly/release-tier work, not implemented yet
//! (see tasks/todo.md).

use kika::{Orientation, Point2, orient2d};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, Zero};

/// Converts a finite `f64` to an exact `BigRational`. Independent of
/// Kika's own arithmetic (bit-level decomposition only) — this is the
/// ground truth the predicate is checked against, never used to implement
/// the predicate itself.
fn exact(x: f64) -> BigRational {
    assert!(x.is_finite());
    if x == 0.0 {
        return BigRational::zero();
    }
    let bits = x.to_bits();
    let sign = if (bits >> 63) & 1 == 1 { -1 } else { 1 };
    let exponent_bits = ((bits >> 52) & 0x7ff) as i64;
    let mantissa_bits = bits & 0xf_ffff_ffff_ffff;
    let (mantissa, exponent) = if exponent_bits == 0 {
        (mantissa_bits, -1074i64)
    } else {
        (mantissa_bits | (1 << 52), exponent_bits - 1075)
    };
    let mantissa = BigInt::from(mantissa) * BigInt::from(sign);
    let mant_rat = BigRational::from_integer(mantissa);
    if exponent >= 0 {
        mant_rat * BigRational::from_integer(BigInt::from(2).pow(exponent as u32))
    } else {
        mant_rat / BigRational::from_integer(BigInt::from(2).pow((-exponent) as u32))
    }
}

/// Exact-rational oracle for orient2d's sign, computed independently of
/// `kika::orient2d`'s implementation.
fn oracle_orient2d(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Orientation {
    let acx = exact(a.0) - exact(c.0);
    let acy = exact(a.1) - exact(c.1);
    let bcx = exact(b.0) - exact(c.0);
    let bcy = exact(b.1) - exact(c.1);
    let det = acx * bcy - acy * bcx;
    if det.is_positive() {
        Orientation::CounterClockwise
    } else if det.is_negative() {
        Orientation::Clockwise
    } else {
        Orientation::Collinear
    }
}

fn check(a: (f64, f64), b: (f64, f64), c: (f64, f64)) {
    let got = orient2d(
        Point2::new(a.0, a.1).unwrap(),
        Point2::new(b.0, b.1).unwrap(),
        Point2::new(c.0, c.1).unwrap(),
    );
    let want = oracle_orient2d(a, b, c);
    assert_eq!(
        got, want,
        "orient2d({a:?}, {b:?}, {c:?}): got {got:?}, oracle says {want:?}"
    );
}

/// Small deterministic xorshift PRNG — avoids adding a `rand` dependency
/// for a fixed-seed, reproducible test case generator (ponytail: a few
/// lines beats a dependency for this).
struct Xorshift64(u64);
impl Xorshift64 {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn next_f64_in(&mut self, scale: f64) -> f64 {
        // Map to [-1, 1] then apply scale; deterministic, no external crate.
        let bits = self.next_u64();
        let unit = (bits >> 11) as f64 * (1.0 / (1u64 << 53) as f64); // [0,1)
        (unit * 2.0 - 1.0) * scale
    }
}

#[test]
fn basic_orientations() {
    check((0.0, 0.0), (1.0, 0.0), (0.0, 1.0));
    check((0.0, 0.0), (0.0, 1.0), (1.0, 0.0));
    check((0.0, 0.0), (1.0, 1.0), (2.0, 2.0));
    check((-5.0, 3.0), (2.0, -7.0), (0.0, 0.0));
}

#[test]
fn duplicate_and_zero_length() {
    check((1.0, 1.0), (1.0, 1.0), (2.0, 2.0));
    check((0.0, 0.0), (0.0, 0.0), (0.0, 0.0));
    check((3.0, -3.0), (3.0, -3.0), (3.0, -3.0));
}

#[test]
fn signed_zero() {
    check((0.0, 0.0), (1.0, 0.0), (-0.0, -0.0));
    check((-0.0, 0.0), (0.0, -0.0), (1.0, 1.0));
}

#[test]
fn extreme_and_mixed_scale() {
    check((0.0, 0.0), (1e100, 0.0), (0.0, 1e-100));
    check((1e-100, 1e-100), (2e-100, 1e-100), (1e-100, 2e-100));
    check((1e10, 1e-10), (-1e10, 1e-10), (0.0, 1e10));
    check((1e100, 1e100), (1e100, -1e100), (-1e100, 0.0));
}

#[test]
fn nearly_collinear_across_scales() {
    for &scale in &[1.0_f64, 1e6, 1e-6, 1e50, 1e-50] {
        let a = (0.0, 0.0);
        let b = (scale, 0.0);
        // c perturbed by a tiny fraction of scale, or exactly on the line.
        for &eps in &[0.0_f64, 1e-12, -1e-12] {
            let c = (scale * 0.5, scale * eps);
            check(a, b, c);
        }
    }
}

#[test]
fn permutations_are_consistent_with_oracle() {
    let pts = [(0.0, 0.0), (3.0, 1.0), (1.0, 4.0)];
    let perms = [
        [0, 1, 2],
        [1, 0, 2],
        [0, 2, 1],
        [2, 1, 0],
        [1, 2, 0],
        [2, 0, 1],
    ];
    for p in perms {
        check(pts[p[0]], pts[p[1]], pts[p[2]]);
    }
}

#[test]
fn random_points_multiple_scales() {
    let mut rng = Xorshift64(0x9E3779B97F4A7C15);
    for &scale in &[1.0_f64, 1e-8, 1e8, 1e-100, 1e100] {
        for _ in 0..200 {
            let a = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let b = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let c = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            check(a, b, c);
        }
    }
}

#[test]
fn random_near_collinear_stresses_filter_boundary() {
    // Perturb a point that starts exactly on the a-b line by a tiny
    // random amount, at varied scales: this deliberately walks values
    // across the filter's conclusive/inconclusive boundary so both the
    // filter and the exact fallback get exercised against the oracle.
    let mut rng = Xorshift64(0xD1B54A32D192ED03);
    for &scale in &[1.0_f64, 1e-30, 1e30] {
        for _ in 0..200 {
            let a = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let b = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let t = rng.next_f64_in(1.0);
            let on_line = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
            let perturb = rng.next_f64_in(scale * 1e-10);
            let c = (on_line.0 + perturb, on_line.1);
            check(a, b, c);
        }
    }
}
