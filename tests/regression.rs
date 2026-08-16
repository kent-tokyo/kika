//! Entry point for `tests/regression/*`, per AGENTS.md §6's required
//! directory layout. Cargo only auto-discovers `tests/*.rs` files, not
//! nested ones, so this just declares the submodules.

#[path = "regression/incircle.rs"]
mod incircle;
#[path = "regression/orient2d.rs"]
mod orient2d;
