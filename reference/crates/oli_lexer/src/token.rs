use oli_diag::Span;
use std::fmt;

/// Reserved words of Oli-- V0 (`spec/OLI_SYNTAX_V0.md` §2.3).
///
/// Words reserved for later versions (`bit`, `bits`, `atomic`, `thread`,
/// `extern`, `generic`, `const`, `static`, `volatile`) are lexed as
/// identifiers; the parser refuses to *declare* them (`E0010`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[rustfmt::skip]
pub enum Kw {
    Module, Import, Pub, Proc, Permit, Calls, Section, Align, Entry, Export, Traps,
    Ret, Fail, If, Then, Elif, Else, End, While, Each, In, Loop, Break, Continue,
    Zone, At, From, Case, When, Layout, Choice, Packed, Machine, Clobber,
    And, Or, Not, True, False, None, Never,
    Rw, Mmio, Addr, Ref, View, Own, Port, Be, Le,
    Wrap, Sat, Checked,
}

#[rustfmt::skip]
const KEYWORDS: &[(&str, Kw)] = &[
    ("module", Kw::Module), ("import", Kw::Import), ("pub", Kw::Pub), ("proc", Kw::Proc),
    ("permit", Kw::Permit), ("calls", Kw::Calls), ("section", Kw::Section),
    ("align", Kw::Align), ("entry", Kw::Entry), ("export", Kw::Export), ("traps", Kw::Traps),
    ("ret", Kw::Ret), ("fail", Kw::Fail), ("if", Kw::If), ("then", Kw::Then),
    ("elif", Kw::Elif), ("else", Kw::Else), ("end", Kw::End), ("while", Kw::While),
    ("each", Kw::Each), ("in", Kw::In), ("loop", Kw::Loop), ("break", Kw::Break),
    ("continue", Kw::Continue), ("zone", Kw::Zone), ("at", Kw::At), ("from", Kw::From),
    ("case", Kw::Case), ("when", Kw::When), ("layout", Kw::Layout), ("choice", Kw::Choice),
    ("packed", Kw::Packed), ("machine", Kw::Machine), ("clobber", Kw::Clobber),
    ("and", Kw::And), ("or", Kw::Or), ("not", Kw::Not), ("true", Kw::True),
    ("false", Kw::False), ("none", Kw::None), ("never", Kw::Never),
    ("rw", Kw::Rw), ("mmio", Kw::Mmio), ("addr", Kw::Addr), ("ref", Kw::Ref),
    ("view", Kw::View), ("own", Kw::Own), ("port", Kw::Port), ("be", Kw::Be), ("le", Kw::Le),
    ("wrap", Kw::Wrap), ("sat", Kw::Sat), ("checked", Kw::Checked),
];

impl Kw {
    pub fn from_word(s: &str) -> Option<Kw> {
        KEYWORDS.iter().find(|(w, _)| *w == s).map(|(_, k)| *k)
    }

    pub fn as_str(self) -> &'static str {
        KEYWORDS
            .iter()
            .find(|(_, k)| *k == self)
            .map_or("?", |(w, _)| w)
    }
}

/// Words reserved for later language versions: they cannot name anything.
pub const RESERVED_WORDS: &[&str] = &[
    "bit", "bits", "atomic", "thread", "extern", "generic", "const", "static", "volatile",
];

/// Primitive type names. They are not keywords in the grammar sense but the
/// lexer classifies them so the parser can recognize conversions (`u8(x)`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[rustfmt::skip]
pub enum Prim {
    U8, U16, U32, U64, S8, S16, S32, S64, Word, Uword, Byte, Bool, Physaddr, F32, F64,
}

#[rustfmt::skip]
const PRIMS: &[(&str, Prim)] = &[
    ("u8", Prim::U8), ("u16", Prim::U16), ("u32", Prim::U32), ("u64", Prim::U64),
    ("s8", Prim::S8), ("s16", Prim::S16), ("s32", Prim::S32), ("s64", Prim::S64),
    ("word", Prim::Word), ("uword", Prim::Uword), ("byte", Prim::Byte),
    ("bool", Prim::Bool), ("physaddr", Prim::Physaddr), ("f32", Prim::F32), ("f64", Prim::F64),
];

impl Prim {
    pub fn from_word(s: &str) -> Option<Prim> {
        PRIMS.iter().find(|(w, _)| *w == s).map(|(_, p)| *p)
    }

    pub fn as_str(self) -> &'static str {
        PRIMS
            .iter()
            .find(|(_, p)| *p == self)
            .map_or("?", |(w, _)| w)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Ident(String),
    /// Integer literal after suffix scaling (`64K` = 65536). Overflow is
    /// reported by the lexer and the value is clamped to `u128::MAX`.
    Int(u128),
    Char(u8),
    Str(Vec<u8>),
    Kw(Kw),
    Prim(Prim),
    /// `--- text` documentation comment (text without the marker, trimmed).
    Doc(String),
    // Operators and punctuation (§2.5)
    Bind,   // :=
    Store,  // <-
    Move,   // <~
    Arrow,  // ->
    DotDot, // ..
    Colon,
    Comma,
    Dot,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Amp,
    Pipe,
    Caret,
    Tilde,
    Shl,
    Shr,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    /// One or more line terminators (runs are collapsed).
    Newline,
    Eof,
}

#[rustfmt::skip]
const SYMBOLS: &[(&str, TokenKind)] = &[
    (":=", TokenKind::Bind), ("<-", TokenKind::Store), ("<~", TokenKind::Move),
    ("->", TokenKind::Arrow), ("..", TokenKind::DotDot), (":", TokenKind::Colon),
    (",", TokenKind::Comma), (".", TokenKind::Dot), ("+", TokenKind::Plus),
    ("-", TokenKind::Minus), ("*", TokenKind::Star), ("/", TokenKind::Slash),
    ("%", TokenKind::Percent), ("==", TokenKind::EqEq), ("!=", TokenKind::Ne),
    ("<", TokenKind::Lt), ("<=", TokenKind::Le), (">", TokenKind::Gt), (">=", TokenKind::Ge),
    ("&", TokenKind::Amp), ("|", TokenKind::Pipe), ("^", TokenKind::Caret),
    ("~", TokenKind::Tilde), ("<<", TokenKind::Shl), (">>", TokenKind::Shr),
    ("(", TokenKind::LParen), (")", TokenKind::RParen), ("[", TokenKind::LBracket),
    ("]", TokenKind::RBracket), ("{", TokenKind::LBrace), ("}", TokenKind::RBrace),
];

impl TokenKind {
    /// Source spelling of an operator/punctuation token ("" for others).
    pub fn symbol(&self) -> &'static str {
        SYMBOLS
            .iter()
            .find(|(_, k)| k == self)
            .map_or("", |(s, _)| s)
    }

    /// Human-readable name used in "expected X, found Y" diagnostics.
    pub fn describe(&self) -> String {
        match self {
            TokenKind::Ident(s) => format!("identifier `{s}`"),
            TokenKind::Int(v) => format!("integer `{v}`"),
            TokenKind::Char(_) => "character literal".to_string(),
            TokenKind::Str(_) => "string literal".to_string(),
            TokenKind::Kw(k) => format!("`{}`", k.as_str()),
            TokenKind::Prim(p) => format!("type `{}`", p.as_str()),
            TokenKind::Doc(_) => "doc comment".to_string(),
            TokenKind::Newline => "end of line".to_string(),
            TokenKind::Eof => "end of file".to_string(),
            other => format!("`{}`", other.symbol()),
        }
    }

    /// Short kind name for `--show-tokens`.
    pub fn kind_name(&self) -> &'static str {
        match self {
            TokenKind::Ident(_) => "Ident",
            TokenKind::Int(_) => "Int",
            TokenKind::Char(_) => "Char",
            TokenKind::Str(_) => "Str",
            TokenKind::Kw(_) => "Kw",
            TokenKind::Prim(_) => "Prim",
            TokenKind::Doc(_) => "Doc",
            TokenKind::Newline => "Newline",
            TokenKind::Eof => "Eof",
            _ => "Op",
        }
    }

    pub fn is_kw(&self, k: Kw) -> bool {
        matches!(self, TokenKind::Kw(x) if *x == k)
    }

    pub fn is_ident(&self, name: &str) -> bool {
        matches!(self, TokenKind::Ident(s) if s == name)
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.describe())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
