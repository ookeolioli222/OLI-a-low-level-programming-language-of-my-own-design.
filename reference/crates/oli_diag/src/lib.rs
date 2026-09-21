//! Source files, spans and diagnostics shared by every stage of `olic`.
//!
//! Diagnostics are values, never panics: every stage pushes into a
//! [`Diagnostics`] sink and the driver renders them in the format fixed by
//! `spec/OLI_SYNTAX_V0.md` §8.

mod diagnostic;
mod render;
mod source;
mod span;

pub use diagnostic::{Diagnostic, Diagnostics, Severity};
pub use render::render;
pub use source::{SourceFile, SourceMap};
pub use span::Span;
