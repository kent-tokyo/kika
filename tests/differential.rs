//! Entry point for `tests/differential/*`, per AGENTS.md §6's required
//! directory layout. Cargo only auto-discovers `tests/*.rs` files, not
//! nested ones, so this just declares the submodules.

#[path = "differential/orient2d.rs"]
mod orient2d;
