# Contributing

## License and provenance

Kika is `MIT OR Apache-2.0`. Do not copy, translate, or port code from
CGAL or any GPL-licensed project. CGAL may be used only as an external
oracle to compare outputs, never as source material or a linked
dependency. Implement algorithms from published papers, textbooks, public
specifications, or original design. If you're unsure whether a source is
safe to consult, ask before opening a PR.

## Numerical correctness rules

* No fixed-epsilon comparisons in geometric predicates
  (`if x.abs() < 1e-9`-style code). Error bounds must be derived from input
  magnitudes. See `docs/numerical-model.md`.
* Predicates return typed `Sign`/`Orientation`, never a raw determinant or
  an ambiguous `bool`.
* Public API functions must not panic. Degenerate/edge-case behavior must
  be explicit and tested, not assumed "rare."
* `unsafe` is disallowed by default (see `docs/architecture.md` and
  AGENTS.md §15); adding it requires profiling evidence, a safety-invariant
  writeup, Miri coverage, and its own isolated commit.

## Commit style

Small, single-purpose commits. Don't mix API changes, new algorithms,
large refactors, benchmarks, docs, and CI changes into one commit. Example
subjects: `feat(predicates): add expansion arithmetic primitives`,
`test(predicates): add exact rational oracle`.

## Testing expectations

New geometric code needs, at minimum: unit tests, a property test where
one naturally applies (antisymmetry, invariance under translation/positive
uniform scale), and adversarial cases for the degeneracies relevant to
that code (see `docs/degeneracy-policy.md`). Predicates additionally need a
differential test against the `num-rational`-based oracle in
`tests/differential/`.

Run before submitting:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps
```
