//! Recursive-descent parser for Oli-- V0 (`spec/OLI_SYNTAX_V0.md` §3–§7).
//!
//! Design:
//! - keyword-directed, one token of lookahead (two in a few documented spots);
//! - newlines are tokens; inside unclosed `( [ {` they are trivia (§1);
//! - every error is a diagnostic, never a panic; a failed statement is
//!   dropped and parsing resumes at the next line; a failed block header
//!   keeps its body so `end` still closes the right block;
//! - the tree returned is always complete enough for `--show-ast`.

mod decl;
mod expr;
mod machine;
mod parser;
mod stmt;
mod types;

#[cfg(test)]
mod tests;

use oli_ast::Module;
use oli_diag::{Diagnostics, SourceFile};

/// Lexes and parses one file. Diagnostics go to `diags`; the returned module
/// is partial when errors were reported.
pub fn parse_source(file: &SourceFile, diags: &mut Diagnostics) -> Module {
    let tokens = oli_lexer::lex(file, diags);
    parser::Parser::new(tokens, file, diags).parse_module()
}
