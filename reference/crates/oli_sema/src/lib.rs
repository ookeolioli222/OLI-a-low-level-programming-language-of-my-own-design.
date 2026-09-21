//! Semantic analysis of Oli-- V0: module loading, name resolution, layouts,
//! constant evaluation, type checking, definite assignment, reachability,
//! capabilities and escape analysis (`spec/OLI_SEMANTICS_V0.md`,
//! `spec/OLI_MEMORY_V0.md`).
//!
//! Internal item ids (`ProcId`, `LayoutId`, `LocalId`, …) are produced by the
//! checker itself, never by user input; indexing with them is a compiler bug
//! if it ever fails, which is why this crate allows slice indexing.
#![allow(clippy::indexing_slicing)]

pub mod hir;
pub mod printer;
pub mod regions;
pub mod ty;

mod loader;
mod sema;

#[cfg(test)]
mod tests;

pub use hir::{Program, Target};
pub use loader::{FsModuleSource, MemoryModuleSource, ModuleSource};

use oli_diag::{Diagnostics, SourceFile, SourceMap};

#[derive(Debug, Clone)]
pub struct Options {
    pub target: Target,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            target: Target::Hosted,
        }
    }
}

/// Analyzes the program rooted at `root`. Imported modules are fetched from
/// `source`; every file is registered in `sources` so diagnostics can be
/// rendered. The returned program is partial when errors were reported.
pub fn analyze(
    root: SourceFile,
    source: &dyn ModuleSource,
    opts: &Options,
    sources: &mut SourceMap,
    diags: &mut Diagnostics,
) -> Program {
    let modules = loader::load_all(root, source, sources, diags);
    sema::check_program(modules, opts, diags)
}
