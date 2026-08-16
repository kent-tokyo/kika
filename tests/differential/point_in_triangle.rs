//! Differential tests for `Triangle2::relation_to` against an independent
//! exact-rational implementation of the same classification. See
//! `tests/differential/point_on_segment.rs`'s module doc for why this
//! matters even though `orient2d` itself is already proven exact — this
//! is checking the *decision tree*, not the arithmetic, and that decision
//! tree had a real bug for degenerate triangles (found while writing this
//! predicate; see `tests/regression/point_in_triangle.rs`).

use kika::{Point2, PointTriangleRelation, Triangle2};
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
type Rat = BigRational;

fn cross(o: (Rat, Rat), p: (Rat, Rat), q: (Rat, Rat)) -> Rat {
    (p.0 - o.0.clone()) * (q.1 - o.1.clone()) - (p.1 - o.1) * (q.0 - o.0)
}

fn exact_pt(p: P2) -> (Rat, Rat) {
    (exact(p.0), exact(p.1))
}

/// Independent oracle. Non-degenerate case: standard barycentric-sign
/// same-side test via exact rational cross products. Degenerate
/// (collinear) case: exact dot-product range membership against
/// whichever of the 3 point pairs actually spans the point (mirrors the
/// production fix, reimplemented from scratch).
fn oracle_relation(a: P2, b: P2, c: P2, p: P2) -> PointTriangleRelation {
    let (ea, eb, ec, ep) = (exact_pt(a), exact_pt(b), exact_pt(c), exact_pt(p));
    let area2 = cross(ea.clone(), eb.clone(), ec.clone());

    if area2.is_zero() {
        let on_segment = |u: (Rat, Rat), v: (Rat, Rat)| -> bool {
            let cr = (v.0.clone() - u.0.clone()) * (ep.1.clone() - u.1.clone())
                - (v.1.clone() - u.1.clone()) * (ep.0.clone() - u.0.clone());
            if !cr.is_zero() {
                return false;
            }
            if u == v {
                return ep == u;
            }
            let dx = v.0.clone() - u.0.clone();
            let dy = v.1.clone() - u.1.clone();
            let t_num = (ep.0.clone() - u.0) * dx.clone() + (ep.1.clone() - u.1) * dy.clone();
            let t_den = dx.clone() * dx + dy.clone() * dy;
            !(t_num.is_negative() || t_num > t_den)
        };
        return if on_segment(ea.clone(), eb.clone())
            || on_segment(eb, ec.clone())
            || on_segment(ec, ea)
        {
            PointTriangleRelation::OnBoundary
        } else {
            PointTriangleRelation::Outside
        };
    }

    let s1 = cross(ea.clone(), eb.clone(), ep.clone());
    let s2 = cross(eb, ec.clone(), ep.clone());
    let s3 = cross(ec, ea, ep);

    let has_pos = [&s1, &s2, &s3].iter().any(|s| s.is_positive());
    let has_neg = [&s1, &s2, &s3].iter().any(|s| s.is_negative());
    if has_pos && has_neg {
        PointTriangleRelation::Outside
    } else if s1.is_zero() || s2.is_zero() || s3.is_zero() {
        PointTriangleRelation::OnBoundary
    } else {
        PointTriangleRelation::Inside
    }
}

fn check(a: P2, b: P2, c: P2, p: P2) {
    let t = Triangle2::new(
        Point2::new(a.0, a.1).unwrap(),
        Point2::new(b.0, b.1).unwrap(),
        Point2::new(c.0, c.1).unwrap(),
    );
    let got = t.relation_to(Point2::new(p.0, p.1).unwrap());
    let want = oracle_relation(a, b, c, p);
    assert_eq!(got, want, "relation_to(a={a:?}, b={b:?}, c={c:?}, p={p:?})");
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
fn basic_and_degenerate_cases() {
    check((0.0, 0.0), (4.0, 0.0), (0.0, 4.0), (1.0, 1.0));
    check((0.0, 0.0), (4.0, 0.0), (0.0, 4.0), (5.0, 5.0));
    check((0.0, 0.0), (4.0, 0.0), (0.0, 4.0), (2.0, 0.0));
    // Degenerate: found bug lived here (point on shared line, outside span).
    check((0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (10.0, 0.0));
    check((0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (0.5, 0.0));
    check((0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (-5.0, 0.0));
    check((0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (0.5, 1.0));
}

#[test]
fn random_triangles_random_points() {
    let mut rng = Xorshift64(0xC0FFEEC0FFEEC0FF);
    for &scale in &[1.0_f64, 1e-6, 1e6, 1e-50, 1e50] {
        for _ in 0..150 {
            let pt = |rng: &mut Xorshift64| (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let (a, b, c) = (pt(&mut rng), pt(&mut rng), pt(&mut rng));
            check(a, b, c, pt(&mut rng));
            // Also stress boundary/interior via barycentric combination.
            let (u, v) = (rng.next_f64_in(1.0), rng.next_f64_in(1.0));
            let w = 1.0 - u - v;
            let combo = (w * a.0 + u * b.0 + v * c.0, w * a.1 + u * b.1 + v * c.1);
            check(a, b, c, combo);
        }
    }
}

#[test]
fn random_degenerate_triangles() {
    let mut rng = Xorshift64(0xABCDEF0123456789);
    for &scale in &[1.0_f64, 1e-30, 1e30] {
        for _ in 0..150 {
            // 3 collinear points via a shared direction + distinct t's.
            let origin = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            let dir = (rng.next_f64_in(1.0), rng.next_f64_in(1.0));
            let along = |t: f64| (origin.0 + t * dir.0, origin.1 + t * dir.1);
            let a = along(rng.next_f64_in(scale));
            let b = along(rng.next_f64_in(scale));
            let c = along(rng.next_f64_in(scale));
            let p_on_line = along(rng.next_f64_in(scale * 2.0));
            check(a, b, c, p_on_line);
            let p_off_line = (rng.next_f64_in(scale), rng.next_f64_in(scale));
            check(a, b, c, p_off_line);
        }
    }
}

#[test]
fn mixed_intra_call_magnitude() {
    let mut rng = Xorshift64(0x1032547698BADCFE);
    let magnitudes = [1.0_f64, 1e5, 1e-5, 1e30, 1e-30];
    for _ in 0..250 {
        let coord = |rng: &mut Xorshift64| {
            let m = magnitudes[(rng.next_u64() as usize) % magnitudes.len()];
            rng.next_f64_in(m)
        };
        let pt = |rng: &mut Xorshift64| (coord(rng), coord(rng));
        let (a, b, c) = (pt(&mut rng), pt(&mut rng), pt(&mut rng));
        check(a, b, c, pt(&mut rng));
    }
}
