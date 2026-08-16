//! Differential tests for `insphere` against an independent exact-rational
//! oracle. See `tests/differential/orient2d.rs` for the rationale.

use kika::{Point3, Sign, insphere};
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
type Rat = BigRational;

fn lift(p: P3, e: P3) -> (Rat, Rat, Rat, Rat) {
    let dx = exact(p.0) - exact(e.0);
    let dy = exact(p.1) - exact(e.1);
    let dz = exact(p.2) - exact(e.2);
    let dw = dx.clone() * dx.clone() + dy.clone() * dy.clone() + dz.clone() * dz.clone();
    (dx, dy, dz, dw)
}

fn det3(p: (Rat, Rat, Rat), q: (Rat, Rat, Rat), r: (Rat, Rat, Rat)) -> Rat {
    p.0.clone() * (q.1.clone() * r.2.clone() - q.2.clone() * r.1.clone())
        - p.1.clone() * (q.0.clone() * r.2.clone() - q.2.clone() * r.0.clone())
        + p.2 * (q.0 * r.1 - q.1 * r.0)
}

fn oracle_insphere(a: P3, b: P3, c: P3, d: P3, e: P3) -> Sign {
    let (adx, ady, adz, adw) = lift(a, e);
    let (bdx, bdy, bdz, bdw) = lift(b, e);
    let (cdx, cdy, cdz, cdw) = lift(c, e);
    let (ddx, ddy, ddz, ddw) = lift(d, e);

    let m11 = det3(
        (bdy.clone(), bdz.clone(), bdw.clone()),
        (cdy.clone(), cdz.clone(), cdw.clone()),
        (ddy.clone(), ddz.clone(), ddw.clone()),
    );
    let m12 = det3(
        (bdx.clone(), bdz.clone(), bdw.clone()),
        (cdx.clone(), cdz.clone(), cdw.clone()),
        (ddx.clone(), ddz.clone(), ddw.clone()),
    );
    let m13 = det3(
        (bdx.clone(), bdy.clone(), bdw),
        (cdx.clone(), cdy.clone(), cdw),
        (ddx.clone(), ddy.clone(), ddw),
    );
    let m14 = det3((bdx, bdy, bdz), (cdx, cdy, cdz), (ddx, ddy, ddz));

    let det = adx * m11 - ady * m12 + adz * m13 - adw * m14;

    if det.is_positive() {
        Sign::Positive
    } else if det.is_negative() {
        Sign::Negative
    } else {
        Sign::Zero
    }
}

fn check(a: P3, b: P3, c: P3, d: P3, e: P3) {
    let got = insphere(
        Point3::new(a.0, a.1, a.2).unwrap(),
        Point3::new(b.0, b.1, b.2).unwrap(),
        Point3::new(c.0, c.1, c.2).unwrap(),
        Point3::new(d.0, d.1, d.2).unwrap(),
        Point3::new(e.0, e.1, e.2).unwrap(),
    );
    let want = oracle_insphere(a, b, c, d, e);
    assert_eq!(
        got, want,
        "insphere({a:?}, {b:?}, {c:?}, {d:?}, {e:?}): got {got:?}, oracle says {want:?}"
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

fn regular_tetrahedron() -> (P3, P3, P3, P3) {
    let h = 1.0 / 2.0_f64.sqrt();
    (
        (1.0, 0.0, -h),
        (-1.0, 0.0, -h),
        (0.0, 1.0, h),
        (0.0, -1.0, h),
    )
}

#[test]
fn basic_cases() {
    let (a, b, c, d) = regular_tetrahedron();
    check(a, b, c, d, (0.0, 0.0, 0.0));
    check(a, b, c, d, (100.0, 100.0, 100.0));
}

#[test]
fn coplanar_and_duplicate() {
    check(
        (0.0, 0.0, 0.0),
        (1.0, 0.0, 0.0),
        (0.0, 1.0, 0.0),
        (1.0, 1.0, 0.0),
        (0.5, 0.5, 1.0),
    );
    check(
        (1.0, 1.0, 1.0),
        (1.0, 1.0, 1.0),
        (2.0, 3.0, 4.0),
        (5.0, 6.0, 7.0),
        (0.0, 0.0, 0.0),
    );
}

#[test]
fn extreme_and_mixed_scale() {
    // insphere is degree 5: even narrower safe range than incircle's
    // degree 4. See docs/numerical-model.md.
    check(
        (1e40, 0.0, 0.0),
        (0.0, 1e40, 0.0),
        (0.0, 0.0, 1e40),
        (-1e40, 0.0, 0.0),
        (0.0, 0.0, 0.0),
    );
    check(
        (1e-40, 0.0, 0.0),
        (0.0, 1e-40, 0.0),
        (0.0, 0.0, 1e-40),
        (-1e-40, 0.0, 0.0),
        (1e-40, 1e-40, 1e-40),
    );
}

#[test]
fn random_points_multiple_scales() {
    let mut rng = Xorshift64(0xC0FFEEC0FFEEC0FF);
    for &scale in &[1.0_f64, 1e-6, 1e6, 1e-40, 1e40] {
        for _ in 0..80 {
            let pt = |rng: &mut Xorshift64| {
                (
                    rng.next_f64_in(scale),
                    rng.next_f64_in(scale),
                    rng.next_f64_in(scale),
                )
            };
            let a = pt(&mut rng);
            let b = pt(&mut rng);
            let c = pt(&mut rng);
            let d = pt(&mut rng);
            let e = pt(&mut rng);
            check(a, b, c, d, e);
        }
    }
}

#[test]
fn mixed_intra_call_magnitude() {
    // See tests/differential/orient2d.rs's identically-named test for the
    // rationale. Conservative magnitudes: insphere's degree-5 scaling
    // overflows/underflows sooner than incircle's degree-4.
    let mut rng = Xorshift64(0xFEEDFACEFEEDFACE);
    let magnitudes = [1.0_f64, 1e5, 1e-5, 1e15, 1e-15];
    for _ in 0..250 {
        let coord = |rng: &mut Xorshift64| {
            let m = magnitudes[(rng.next_u64() as usize) % magnitudes.len()];
            rng.next_f64_in(m)
        };
        let pt = |rng: &mut Xorshift64| (coord(rng), coord(rng), coord(rng));
        let a = pt(&mut rng);
        let b = pt(&mut rng);
        let c = pt(&mut rng);
        let d = pt(&mut rng);
        let e = pt(&mut rng);
        check(a, b, c, d, e);
    }
}

#[test]
fn random_near_cospherical_stresses_filter_boundary() {
    let mut rng = Xorshift64(0xABCDEF0123456789);
    for &scale in &[1.0_f64, 1e-15, 1e15] {
        for _ in 0..80 {
            // Random point on a sphere of the given scale via normalized
            // Gaussian-ish sampling (uniform cube then normalize, good
            // enough for a stress generator, not for uniformity).
            let on_sphere = |rng: &mut Xorshift64| {
                let mut v = (
                    rng.next_f64_in(1.0),
                    rng.next_f64_in(1.0),
                    rng.next_f64_in(1.0),
                );
                let norm = (v.0 * v.0 + v.1 * v.1 + v.2 * v.2).sqrt().max(1e-12);
                v = (v.0 / norm, v.1 / norm, v.2 / norm);
                (scale * v.0, scale * v.1, scale * v.2)
            };
            let a = on_sphere(&mut rng);
            let b = on_sphere(&mut rng);
            let c = on_sphere(&mut rng);
            let d = on_sphere(&mut rng);
            let (ex, ey, ez) = on_sphere(&mut rng);
            let perturb = 1.0 + rng.next_f64_in(1e-9);
            let e = (ex * perturb, ey * perturb, ez * perturb);
            check(a, b, c, d, e);
        }
    }
}
