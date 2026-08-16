//! Differential tests for `Segment2::relation_to` against an independent
//! exact-rational implementation of the same classification. Unlike the
//! four core predicates, this isn't just re-checking arithmetic exactness
//! (`orient2d` is already proven exact) — it's checking that the
//! *decision tree* combining collinearity with a range check is correct,
//! which is exactly the kind of thing that had a real bug (see
//! `tests/regression/point_in_triangle.rs`).

use kika::{Point2, PointSegmentRelation, Segment2};
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

/// Independent oracle: exact orientation via a BigRational determinant,
/// then an exact parametric range check — same logic shape as the
/// production code, deliberately reimplemented from scratch rather than
/// calling into `kika`.
fn oracle_relation(a: P2, b: P2, p: P2) -> PointSegmentRelation {
    if a == b {
        return if p == a {
            PointSegmentRelation::Endpoint
        } else {
            PointSegmentRelation::NotOnSegment
        };
    }
    let (ax, ay, bx, by, px, py) = (
        exact(a.0),
        exact(a.1),
        exact(b.0),
        exact(b.1),
        exact(p.0),
        exact(p.1),
    );
    let cross = (bx.clone() - ax.clone()) * (py.clone() - ay.clone())
        - (by.clone() - ay.clone()) * (px.clone() - ax.clone());
    if !cross.is_zero() {
        return PointSegmentRelation::NotOnSegment;
    }
    if p == a || p == b {
        return PointSegmentRelation::Endpoint;
    }
    // Exact betweenness via a dot-product-style parametrization: p is
    // between a and b iff (p-a).(b-a) is in [0, (b-a).(b-a)].
    let dx = bx - ax.clone();
    let dy = by - ay.clone();
    let t_num = (px - ax) * dx.clone() + (py - ay) * dy.clone();
    let t_den = dx.clone() * dx + dy.clone() * dy;
    if t_num.is_negative() || t_num > t_den {
        PointSegmentRelation::NotOnSegment
    } else {
        PointSegmentRelation::Interior
    }
}

fn check(a: P2, b: P2, p: P2) {
    let seg = Segment2::new(
        Point2::new(a.0, a.1).unwrap(),
        Point2::new(b.0, b.1).unwrap(),
    );
    let got = seg.relation_to(Point2::new(p.0, p.1).unwrap());
    let want = oracle_relation(a, b, p);
    assert_eq!(got, want, "relation_to(a={a:?}, b={b:?}, p={p:?})");
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
    check((0.0, 0.0), (4.0, 0.0), (2.0, 0.0));
    check((0.0, 0.0), (4.0, 0.0), (0.0, 0.0));
    check((0.0, 0.0), (4.0, 0.0), (5.0, 0.0));
    check((0.0, 0.0), (4.0, 0.0), (2.0, 1.0));
    check((1.0, 1.0), (1.0, 1.0), (1.0, 1.0));
    check((1.0, 1.0), (1.0, 1.0), (2.0, 1.0));
}

#[test]
fn random_points_on_and_off_random_segments() {
    let mut rng = Xorshift64(0x9E3779B97F4A7C15);
    for &scale in &[1.0_f64, 1e-8, 1e8, 1e-60, 1e60] {
        for _ in 0..150 {
            let a = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let b = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            // Random points both truly on the segment (via parametric t)
            // and generic random points off it.
            let t = rng.next_f64_in(1.0);
            let on_line = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
            check(a, b, on_line);
            let generic = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            check(a, b, generic);
        }
    }
}

#[test]
fn mixed_intra_call_magnitude() {
    let mut rng = Xorshift64(0x0FEDCBA987654321);
    let magnitudes = [1.0_f64, 1e3, 1e-3, 1e40, 1e-40];
    for _ in 0..300 {
        let coord = |rng: &mut Xorshift64| {
            let m = magnitudes[(rng.next_u64() as usize) % magnitudes.len()];
            rng.next_f64_in(m)
        };
        let a = (coord(&mut rng), coord(&mut rng));
        let b = (coord(&mut rng), coord(&mut rng));
        let p = (coord(&mut rng), coord(&mut rng));
        check(a, b, p);
    }
}
