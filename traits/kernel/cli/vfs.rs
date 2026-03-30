//! VFS re-exports from the shared `kernel_logic::vfs` module.
//!
//! The canonical definitions live in `kernel_logic::vfs` so the `Platform`
//! struct (also in `kernel_logic`) can reference the `Vfs` trait.  This file
//! just re-exports everything so callers inside `kernel/cli` don't need to
//! change their import paths.

pub use kernel_logic::vfs::{LayeredVfs, MemVfs, Vfs};
