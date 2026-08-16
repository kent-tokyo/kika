//! Differential tests for `incircle` against an independent exact-rational
//! oracle. See `tests/differential/orient2d.rs` for the rationale.

use kika::{Point2, Sign, incircle};
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

type P2 = (f64, f64);

fn oracle_incircle(a: P2, b: P2, c: P2, d: P2) -> Sign {
    let lift = |p: P2| {
        let dx = exact(p.0) - exact(d.0);
        let dy = exact(p.1) - exact(d.1);
        let dz = dx.clone() * dx.clone() + dy.clone() * dy.clone();
        (dx, dy, dz)
    };
    let (adx, ady, adz) = lift(a);
    let (bdx, bdy, bdz) = lift(b);
    let (cdx, cdy, cdz) = lift(c);

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

fn check(a: P2, b: P2, c: P2, d: P2) {
    let got = incircle(
        Point2::new(a.0, a.1).unwrap(),
        Point2::new(b.0, b.1).unwrap(),
        Point2::new(c.0, c.1).unwrap(),
        Point2::new(d.0, d.1).unwrap(),
    );
    let want = oracle_incircle(a, b, c, d);
    assert_eq!(
        got, want,
        "incircle({a:?}, {b:?}, {c:?}, {d:?}): got {got:?}, oracle says {want:?}"
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
fn basic_cases() {
    check((1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, 0.0));
    check((1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (100.0, 100.0));
    check((1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0));
}

#[test]
fn collinear_and_duplicate() {
    check((0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (0.5, 0.0));
    check((0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (0.5, 1.0));
    check((1.0, 1.0), (1.0, 1.0), (2.0, 3.0), (4.0, 5.0));
}

#[test]
fn extreme_and_mixed_scale() {
    // incircle's determinant is degree-4 in the coordinate differences
    // (the paraboloid lift squares two of the three matrix columns), so
    // its safe f64 exponent range is much narrower than orient2d/
    // orient3d's degree-2/3: see docs/numerical-model.md "Known
    // limitation: incircle/insphere have a narrower safe magnitude range"
    // for the derivation (~1e77 ceiling, ~1e-73 floor for uniform-scale
    // coordinates). 1e50/1e-50 stay comfortably inside both.
    check((1e50, 0.0), (0.0, 1e50), (-1e50, 0.0), (0.0, 0.0));
    check((1e-50, 0.0), (0.0, 1e-50), (-1e-50, 0.0), (1e-50, 1e-50));
}

#[test]
fn random_points_multiple_scales() {
    let mut rng = Xorshift64(0xA5A5A5A5A5A5A5A5);
    for &scale in &[1.0_f64, 1e-8, 1e8, 1e-50, 1e50] {
        for _ in 0..150 {
            let a = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let b = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let c = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let d = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            check(a, b, c, d);
        }
    }
}

#[test]
fn mixed_intra_call_magnitude() {
    // See tests/differential/orient2d.rs's identically-named test for the
    // rationale. Magnitudes are much more conservative than orient2d's
    // (which used 2^60/2^-40) because incircle's degree-4 scaling
    // overflows/underflows far sooner — see extreme_and_mixed_scale above.
    let mut rng = Xorshift64(0x5A5A5A5A5A5A5A5A);
    let magnitudes = [1.0_f64, 1e10, 1e-10, 1e30, 1e-30];
    for _ in 0..400 {
        let coord = |rng: &mut Xorshift64| {
            let m = magnitudes[(rng.next_u64() as usize) % magnitudes.len()];
            rng.next_f64_in(m)
        };
        let a = (coord(&mut rng), coord(&mut rng));
        let b = (coord(&mut rng), coord(&mut rng));
        let c = (coord(&mut rng), coord(&mut rng));
        let d = (coord(&mut rng), coord(&mut rng));
        check(a, b, c, d);
    }
}

#[test]
fn random_near_cocircular_stresses_filter_boundary() {
    let mut rng = Xorshift64(0x2F2F2F2F2F2F2F2F);
    for &scale in &[1.0_f64, 1e-20, 1e20] {
        for _ in 0..200 {
            // Three points on a circle of the given scale (parameterized
            // by angle), plus a 4th perturbed slightly off the circle.
            let angle = |rng: &mut Xorshift64| rng.next_f64_in(std::f64::consts::PI);
            let on_circle = |t: f64| (scale * t.cos(), scale * t.sin());
            let a = on_circle(angle(&mut rng));
            let b = on_circle(angle(&mut rng));
            let c = on_circle(angle(&mut rng));
            let t_d = angle(&mut rng);
            let radius_perturb = 1.0 + rng.next_f64_in(1e-10);
            let d = (
                scale * radius_perturb * t_d.cos(),
                scale * radius_perturb * t_d.sin(),
            );
            check(a, b, c, d);
        }
    }
}
