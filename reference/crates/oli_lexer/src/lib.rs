//! Lexer for Oli-- V0 (`spec/OLI_SYNTAX_V0.md` §2).
//!
//! The lexer never fails: every lexical error becomes a diagnostic and the
//! offending characters are skipped, so the parser always receives a
//! complete token stream ending in [`TokenKind::Eof`].

mod lexer;
mod token;

pub use lexer::lex;
pub use token::{Kw, Prim, Token, TokenKind, RESERVED_WORDS};

#[cfg(test)]
mod tests;
