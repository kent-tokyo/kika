//! Entry point for `tests/adversarial/*`, per AGENTS.md §6's required
//! directory layout. Cargo only auto-discovers `tests/*.rs` files, not
//! nested ones, so this just declares the submodules.

#[path = "adversarial/orient2d.rs"]
mod orient2d;
