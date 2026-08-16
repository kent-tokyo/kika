# ADR-001: Numeric robustness strategy

Status: Accepted (v0.1 Phase 1)

## Context

Geometric predicates (`orient2d`, `orient3d`, `incircle`, `insphere`) must
return a correct sign for every finite `f64` input, including degenerate and
near-degenerate configurations, without using a fixed epsilon. AGENTS.md §4.1
requires a staged evaluation: fast filter → error-bound-guaranteed sign →
adaptive precision → exact representation.

## Decision

**Two-stage model: static floating-point filter, then exact fallback.**

1. **Filter.** Compute the predicate determinant directly in `f64`, and
   independently compute a *forward error bound* for that computation from
   the magnitudes of the actual operands (not a constant). If
   `|det| > error_bound`, the computed sign is provably correct and is
   returned immediately.
2. **Exact fallback.** If the filter is inconclusive
   (`|det| <= error_bound`), recompute the determinant using exact
   expansion arithmetic (error-free transformations building a
   nonoverlapping floating-point expansion — see ADR-004). The sign of a
   correctly-formed nonoverlapping expansion equals the sign of its most
   significant nonzero component, which gives an exact answer with no
   further rounding. Critically, "exact" here means exact relative to the
   *original input coordinates*, not relative to any once-rounded
   intermediate: every coordinate difference the fallback needs is built
   as an exact expansion (`diff_expansion`) straight from the `Point2`/
   `Point3` values, not inherited from the filter's rounded `f64`
   subtraction. See `docs/numerical-model.md` "Known limitation (fixed):
   exactness starts at the original coordinates" for a real bug this
   distinction caught during development.

We deliberately do **not** implement Shewchuk's full three-tier "adaptive
precision" scheme (filter → increasing-precision estimate → exact), which
computes intermediate-precision estimates with running error bounds to avoid
paying for full exactness on every fallback. Our fallback always goes
straight to the fully exact expansion. This is a simpler implementation with
an honest cost: fallback cases are exact but not maximally fast.

This is *not* called "adaptive precision" anywhere in code, docs, or the
README — it is a static filter with an exact fallback. Shewchuk-style staged
adaptive precision is a documented upgrade path (see "Revisit" below), not a
present-tense claim.

## Why not use `f64::mul_add` (FMA) for the exact-arithmetic core

An FMA-based `two_product` (`p = a*b; e = a.mul_add(b, -p)`) is shorter than
Dekker's split-based `two_product`, but its exactness depends on the
platform FMA instruction (or its software fallback) being a single
correctly-rounded rounding — a property we did not want to take on faith
across x86_64, aarch64, and wasm32-unknown-unknown, where wasm32 has no
native FMA and must go through a software path.

Instead, the exact-arithmetic core (`two_sum`, `fast_two_sum`, `split`,
`two_product`) uses only `+`, `-`, `*` — Dekker's 1971 error-free
transformations, as described by Shewchuk (1997). Rust does not perform
floating-point contraction of separate `+`/`-`/`*` expressions into fused
operations (contraction only happens through explicit `.mul_add()` calls),
so these transformations are portable and exact on every target that has
IEEE-754-compliant `f64` arithmetic — which includes wasm32-unknown-unknown.
This removes the FMA-portability question rather than requiring an
empirical justification for it. We additionally keep a property test
(`two_product` exactness checked against the `num-rational` oracle) as a
regression guard for this assumption.

## Input policy

Public predicate functions take `Point2`/`Point3`, whose constructors
validate finite coordinates (see ADR-003). Predicates themselves therefore
never see NaN/Infinity and never need to return `Result`.

## Consequences

* Exact fallback is only reached near-degenerate inputs; measured fallback
  rate is reported per AGENTS.md §13, not assumed.
* No `unsafe` is required anywhere in the predicate core.
* If profiling later shows the exact-fallback path is a real bottleneck
  (high fallback rate on realistic workloads), add an intermediate-precision
  adaptive tier between filter and exact fallback. This is additive and does
  not change the public API.

## Revisit when

* Measured exact-fallback rate on realistic (non-adversarial) inputs is
  high enough to matter for a real workload, per §13 profiling.
