# Lessons

Notes on decisions that took real investigation, so they aren't re-litigated.

- Chose Dekker split-based `two_product` over FMA-based (`f64::mul_add`)
  for the exact arithmetic core, specifically to avoid depending on FMA
  correct-rounding guarantees on wasm32-unknown-unknown (no native FMA).
  Rust does not contract separate `+`/`-`/`*` into fused ops, which is what
  makes the split-based approach portable. See ADR-001.
- Empirically verified (against an exact-rational oracle, and against a
  correctly-rounded-FMA emulation) that exact two-`f64` product
  representation has a hard floor around `|a*b| < 1.7e-292`, independent
  of split-vs-FMA. First guess at the threshold (naive `2^-1021` from a
  single 53-bit headroom argument) was wrong by ~50 bits; the real
  requirement is two rounds of 53-bit headroom (product rounding, then the
  error term's own precision), landing near `2^-968`. Don't trust a
  first-pass error-bound derivation without checking it against measured
  data — see `docs/numerical-model.md` "Known limitation".
