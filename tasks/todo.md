# Todo

## Done (Phase 0 + Phase 1)

- [x] Phase 0: name-collision check, ecosystem survey, ADR-001..005
- [x] Expansion arithmetic core (`two_sum`, `split`, `two_product`,
      `product_expansion`, `diff_expansion`, `expansion_sum`,
      `scale_expansion`, `product_of_expansions`, `expansion_sign`)
- [x] `orient2d`, `orient3d`, `incircle`, `insphere` — filter + exact
      fallback, each checked against an independent exact-rational oracle
- [x] CI workflow (`.github/workflows/ci.yml`): fmt, clippy, test matrix
      (Linux/macOS/Windows), MSRV (1.85), wasm32 build, `cargo doc`,
      `cargo deny` (license + advisory check, `deny.toml`)
- [x] Three real bugs found and fixed during implementation (see
      `tasks/lessons.md` for the diagnostic trail):
      1. exact fallback wasn't exact relative to the original coordinates
      2. `orient3d`/`incircle` filter bound used post-cancellation
         magnitudes
      3. naive expansion merging was O(count²), making `insphere`'s
         exact fallback take 16s/call

## Known gaps, not yet closed (see docs/compatibility.md)

- [ ] CI workflow added but not yet exercised by an actual push/PR run —
      "should pass" based on local verification, not CI-confirmed
- [ ] wasm32: build verified, but no test execution under wasm32 (needs
      `wasm-bindgen-test`/`wasmtime`) — the "Rust never contracts +/-/*
      into FMA" argument in ADR-001 is a language guarantee, not
      re-verified empirically on this target
- [ ] `incircle`/`insphere` safe-magnitude-range bounds
      (`docs/numerical-model.md`) are empirically-checked, not tightly
      derived on the floor side

## Backlog (later phases, not started)

- [ ] Phase 2: Vector2/3, Segment2, Triangle2/3, Aabb2/3, point-on-segment,
      segment intersection, point-in-triangle, polygon signed
      area/orientation/validity, self-intersection detection
- [ ] Phase 3: 2D convex hull (monotone chain)
- [ ] Phase 4: 2D Delaunay triangulation
- [ ] Phase 5: exact construction model (re-open ADR-004)
- [ ] Phase 6: constrained Delaunay, polygon Boolean
- [ ] CGAL differential-test harness (separate program, §10)
- [ ] fuzz targets (§12) — none yet; Phase 1's differential/regression
      tests are hand-written and randomized, not coverage-guided fuzzing
- [ ] benches (§13) — predicate fast-path/fallback rate measurement not
      yet built; no performance numbers exist beyond the ad hoc timing
      used to catch and confirm the O(count²) merge bug
- [ ] Shewchuk-style multi-tier adaptive precision (ADR-001 "Revisit"),
      gated on measured fallback rate from real (non-adversarial) usage

## Deferred pending explicit user approval (§19)

- [ ] crates.io publish
- [ ] GitHub release / repo visibility change
- [ ] Any new runtime (non-dev) dependency
