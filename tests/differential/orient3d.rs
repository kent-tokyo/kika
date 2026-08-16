//! Differential tests for `orient3d` against an independent exact-rational
//! oracle. See `tests/differential/orient2d.rs` for the rationale; this
//! mirrors its structure for 3D.

use kika::{Point3, Sign, orient3d};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, Zero};

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

type P3 = (f64, f64, f64);

fn oracle_orient3d(a: P3, b: P3, c: P3, d: P3) -> Sign {
    let adx = exact(a.0) - exact(d.0);
    let ady = exact(a.1) - exact(d.1);
    let adz = exact(a.2) - exact(d.2);
    let bdx = exact(b.0) - exact(d.0);
    let bdy = exact(b.1) - exact(d.1);
    let bdz = exact(b.2) - exact(d.2);
    let cdx = exact(c.0) - exact(d.0);
    let cdy = exact(c.1) - exact(d.1);
    let cdz = exact(c.2) - exact(d.2);

    let det = adx.clone() * (bdy.clone() * cdz.clone() - bdz.clone() * cdy.clone())
        - ady.clone() * (bdx.clone() * cdz.clone() - bdz.clone() * cdx.clone())
        + adz * (bdx * cdy - bdy * cdx);

    if det.is_positive() {
        Sign::Positive
    } else if det.is_negative() {
        Sign::Negative
    } else {
        Sign::Zero
    }
}

fn check(a: P3, b: P3, c: P3, d: P3) {
    let got = orient3d(
        Point3::new(a.0, a.1, a.2).unwrap(),
        Point3::new(b.0, b.1, b.2).unwrap(),
        Point3::new(c.0, c.1, c.2).unwrap(),
        Point3::new(d.0, d.1, d.2).unwrap(),
    );
    let want = oracle_orient3d(a, b, c, d);
    assert_eq!(
        got, want,
        "orient3d({a:?}, {b:?}, {c:?}, {d:?}): got {got:?}, oracle says {want:?}"
    );
}

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
        let bits = self.next_u64();
        let unit = (bits >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
        (unit * 2.0 - 1.0) * scale
    }
}

#[test]
fn basic_orientations() {
    check(
        (0.0, 0.0, 0.0),
        (1.0, 0.0, 0.0),
        (0.0, 1.0, 0.0),
        (0.0, 0.0, 1.0),
    );
    check(
        (0.0, 0.0, 0.0),
        (0.0, 1.0, 0.0),
        (1.0, 0.0, 0.0),
        (0.0, 0.0, 1.0),
    );
    check(
        (1.0, 2.0, 3.0),
        (-1.0, 0.0, 4.0),
        (2.0, -3.0, 1.0),
        (0.0, 0.0, 0.0),
    );
}

#[test]
fn coplanar_and_duplicate() {
    check(
        (0.0, 0.0, 0.0),
        (1.0, 0.0, 0.0),
        (0.0, 1.0, 0.0),
        (1.0, 1.0, 0.0),
    );
    check(
        (1.0, 1.0, 1.0),
        (1.0, 1.0, 1.0),
        (2.0, 3.0, 4.0),
        (5.0, 6.0, 7.0),
    );
}

#[test]
fn extreme_and_mixed_scale() {
    check(
        (0.0, 0.0, 0.0),
        (1e100, 0.0, 0.0),
        (0.0, 1e-100, 0.0),
        (0.0, 0.0, 1e50),
    );
    check(
        (1e-100, 1e-100, 1e-100),
        (2e-100, 1e-100, 1e-100),
        (1e-100, 2e-100, 1e-100),
        (1e-100, 1e-100, 2e-100),
    );
}

#[test]
fn permutation_swaps_match_oracle() {
    let (a, b, c, d) = (
        (0.0, 0.0, 0.0),
        (3.0, 1.0, -2.0),
        (1.0, 4.0, 0.5),
        (-1.0, -1.0, 3.0),
    );
    check(a, b, c, d);
    check(b, a, c, d);
    check(a, c, b, d);
    check(d, b, c, a);
}

#[test]
fn random_points_multiple_scales() {
    let mut rng = Xorshift64(0x243F6A8885A308D3);
    for &scale in &[1.0_f64, 1e-8, 1e8, 1e-100, 1e100] {
        for _ in 0..150 {
            let a = (
                rng.next_f64_in(scale),
                rng.next_f64_in(scale),
                rng.next_f64_in(scale),
            );
            let b = (
                rng.next_f64_in(scale),
                rng.next_f64_in(scale),
                rng.next_f64_in(scale),
            );
            let c = (
                rng.next_f64_in(scale),
                rng.next_f64_in(scale),
                rng.next_f64_in(scale),
            );
            let d = (
                rng.next_f64_in(scale),
                rng.next_f64_in(scale),
                rng.next_f64_in(scale),
            );
            check(a, b, c, d);
        }
    }
}

#[test]
fn mixed_intra_call_magnitude() {
    // See the identically-named test in tests/differential/orient2d.rs:
    // regression coverage for the exact-fallback exactness bug found
    // during development, which only manifests with wide intra-call
    // dynamic range.
    let mut rng = Xorshift64(0x1032547698BADCFE);
    let magnitudes = [1.0_f64, 1e3, 1e-3, 2.0_f64.powi(60), 2.0_f64.powi(-40)];
    for _ in 0..400 {
        let coord = |rng: &mut Xorshift64| {
            let m = magnitudes[(rng.next_u64() as usize) % magnitudes.len()];
            rng.next_f64_in(m)
        };
        let a = (coord(&mut rng), coord(&mut rng), coord(&mut rng));
        let b = (coord(&mut rng), coord(&mut rng), coord(&mut rng));
        let c = (coord(&mut rng), coord(&mut rng), coord(&mut rng));
        let d = (coord(&mut rng), coord(&mut rng), coord(&mut rng));
        check(a, b, c, d);
    }
}

#[test]
fn random_near_coplanar_stresses_filter_boundary() {
    let mut rng = Xorshift64(0x13198A2E03707344);
    for &scale in &[1.0_f64, 1e-30, 1e30] {
        for _ in 0..150 {
            let a = (
                rng.next_f64_in(scale),
                rng.next_f64_in(scale),
                rng.next_f64_in(scale),
            );
            let b = (
                rng.next_f64_in(scale),
                rng.next_f64_in(scale),
                rng.next_f64_in(scale),
            );
            let c = (
                rng.next_f64_in(scale),
                rng.next_f64_in(scale),
                rng.next_f64_in(scale),
            );
            // Point on the plane through a, b, c via barycentric combo,
            // then perturbed slightly out of plane.
            let (u, v) = (rng.next_f64_in(1.0), rng.next_f64_in(1.0));
            let w = 1.0 - u - v;
            let on_plane = (
                w * a.0 + u * b.0 + v * c.0,
                w * a.1 + u * b.1 + v * c.1,
                w * a.2 + u * b.2 + v * c.2,
            );
            let perturb = rng.next_f64_in(scale * 1e-10);
            let d = (on_plane.0, on_plane.1, on_plane.2 + perturb);
            check(a, b, c, d);
        }
    }
}
