//! Minimized regression fixtures for real bugs found during development.
//! See `tests/regression/orient2d.rs` for the convention.
//!
//! ## Found: filter error bound used post-cancellation term magnitudes
//!
//! `incircle`'s floating-point filter computed
//! `bound = FACTOR * (|term_a|+|term_b|+|term_c|)`, where each `term_X`
//! is already the result of an internal subtraction (`adX * (P*Q - R*S)`).
//! When two of the three defining points are both far from `d` in
//! roughly the same direction, `P*Q` and `R*S` are individually huge and
//! nearly equal, so `P*Q - R*S` suffers catastrophic cancellation — the
//! true error is proportional to the *pre-subtraction* magnitudes
//! (`|P*Q|+|R*S|`), not to `term_X`'s own (much smaller, post-
//! cancellation) magnitude. The old bound silently underestimated the
//! true uncertainty and let a wrong sign through as "conclusive".
//! Fixed in `src/predicates/incircle.rs` (and the identical latent flaw
//! in `src/predicates/orient3d.rs`, see
//! `tests/differential/orient3d.rs`'s `cofactor_cancellation_stress`) by
//! summing pre-subtraction cofactor magnitudes instead. See
//! `docs/numerical-model.md` "Known limitation (fixed): filter bound must
//! use pre-cancellation magnitudes".

use kika::{Point2, Sign, incircle};

fn pt(x: f64, y: f64) -> Point2 {
    Point2::new(x, y).unwrap()
}

#[test]
fn filter_bound_uses_pre_cancellation_magnitudes() {
    let a = pt(4.279901170086842e-31, 9.244019969596229e28);
    let b = pt(-2.9066661259516804e-31, -1.615387414877909e-11);
    let c = pt(-4.848195267428912e-11, -3.5698577664876387e-32);
    let d = pt(8.903344455112902e29, -2.2331234338332797e-11);

    // Before the fix, this incorrectly returned Sign::Negative (the
    // filter's own, wrongly "conclusive" answer).
    assert_eq!(incircle(a, b, c, d), Sign::Positive);
}
