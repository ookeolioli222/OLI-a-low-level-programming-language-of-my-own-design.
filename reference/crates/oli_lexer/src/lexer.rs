use crate::{Kw, Prim, Token, TokenKind};
use oli_diag::{Diagnostic, Diagnostics, SourceFile, Span};

/// Tokenizes a whole file. Lexical errors are reported into `diags`; the
/// returned stream is always complete and ends with `Eof`.
pub fn lex(file: &SourceFile, diags: &mut Diagnostics) -> Vec<Token> {
    let mut lx = Lexer {
        text: file.text(),
        src: file.text().as_bytes(),
        pos: 0,
        tokens: Vec::new(),
        diags,
    };
    lx.run();
    lx.tokens
}

struct Lexer<'a> {
    text: &'a str,
    src: &'a [u8],
    pos: usize,
    tokens: Vec<Token>,
    diags: &'a mut Diagnostics,
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

impl Lexer<'_> {
    /// Byte at `i`, or NUL past the end (NUL is never a valid source byte).
    fn at(&self, i: usize) -> u8 {
        self.src.get(i).copied().unwrap_or(0)
    }

    fn cur(&self) -> u8 {
        self.at(self.pos)
    }

    fn peek(&self) -> u8 {
        self.at(self.pos + 1)
    }

    fn eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn span(&self, start: usize) -> Span {
        Span::new(start as u32, self.pos as u32)
    }

    fn push(&mut self, kind: TokenKind, start: usize) {
        let span = self.span(start);
        self.tokens.push(Token { kind, span });
    }

    fn report(&mut self, d: Diagnostic) {
        self.diags.push(d);
    }

    fn run(&mut self) {
        loop {
            self.skip_blanks();
            if self.eof() {
                break;
            }
            let c = self.cur();
            if c == b'\n' {
                self.newline();
            } else if c == b'-' && self.peek() == b'-' {
                self.comment();
            } else if c == b'"' {
                self.string();
            } else if c == b'\'' {
                self.char_lit();
            } else if c.is_ascii_digit() {
                self.number();
            } else if is_ident_start(c) {
                self.word();
            } else {
                self.symbol();
            }
        }
        let end = self.src.len();
        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span::point(end as u32),
        });
    }

    fn skip_blanks(&mut self) {
        while matches!(self.cur(), b' ' | b'\t' | b'\r' | 0x0C) && !self.eof() {
            self.pos += 1;
        }
    }

    fn newline(&mut self) {
        let start = self.pos;
        self.pos += 1;
        let collapse = matches!(
            self.tokens.last(),
            None | Some(Token {
                kind: TokenKind::Newline,
                ..
            })
        );
        if !collapse {
            self.push(TokenKind::Newline, start);
        }
    }

    fn comment(&mut self) {
        let start = self.pos;
        let is_doc = self.at(self.pos + 2) == b'-' && self.at(self.pos + 3) != b'-';
        self.pos += if is_doc { 3 } else { 2 };
        let text_start = self.pos;
        while !self.eof() && self.cur() != b'\n' {
            self.pos += 1;
        }
        if is_doc {
            let body = self.text.get(text_start..self.pos).unwrap_or("");
            let body = body
                .strip_prefix(' ')
                .unwrap_or(body)
                .trim_end()
                .to_string();
            self.push(TokenKind::Doc(body), start);
        }
    }

    fn word(&mut self) {
        let start = self.pos;
        while is_ident_char(self.cur()) && !self.eof() {
            self.pos += 1;
        }
        let word = self.text.get(start..self.pos).unwrap_or("");
        let kind = if let Some(k) = Kw::from_word(word) {
            TokenKind::Kw(k)
        } else if let Some(p) = Prim::from_word(word) {
            TokenKind::Prim(p)
        } else {
            TokenKind::Ident(word.to_string())
        };
        self.push(kind, start);
    }

    fn number(&mut self) {
        let start = self.pos;
        let (radix, prefix_len) = match (self.cur(), self.peek()) {
            (b'0', b'x' | b'X') => (16, 2),
            (b'0', b'b' | b'B') => (2, 2),
            (b'0', b'o' | b'O') => (8, 2),
            _ => (10, 0),
        };
        self.pos += prefix_len;
        let mut value: u128 = 0;
        let mut overflow = false;
        let mut digits = 0usize;
        loop {
            let c = self.cur();
            if c == b'_' && !self.eof() {
                self.pos += 1;
                continue;
            }
            let Some(d) = (c as char).to_digit(radix) else {
                break;
            };
            digits += 1;
            self.pos += 1;
            match value
                .checked_mul(radix as u128)
                .and_then(|v| v.checked_add(d as u128))
            {
                Some(v) => value = v,
                None => overflow = true,
            }
        }
        if digits == 0 {
            let span = self.span(start);
            self.report(
                Diagnostic::error("E0004", "expected digits after the radix prefix", span)
                    .with_label("integer literal has no digits"),
            );
        }
        if radix == 10 {
            let scale: u128 = match self.cur() {
                b'K' => 1 << 10,
                b'M' => 1 << 20,
                b'G' => 1 << 30,
                _ => 1,
            };
            if scale != 1 {
                self.pos += 1;
                match value.checked_mul(scale) {
                    Some(v) => value = v,
                    None => overflow = true,
                }
            }
        }
        if is_ident_char(self.cur()) {
            let bad_start = self.pos;
            while is_ident_char(self.cur()) && !self.eof() {
                self.pos += 1;
            }
            let bad = self.text.get(bad_start..self.pos).unwrap_or("").to_string();
            let span = Span::new(bad_start as u32, self.pos as u32);
            self.report(
                Diagnostic::error(
                    "E0004",
                    format!("invalid suffix `{bad}` on integer literal"),
                    span,
                )
                .with_note("only decimal literals take a size suffix: K, M or G"),
            );
        }
        if overflow {
            let span = self.span(start);
            self.report(
                Diagnostic::error("E0003", "integer literal is too large", span)
                    .with_label("does not fit in 128 bits"),
            );
            value = u128::MAX;
        }
        self.push(TokenKind::Int(value), start);
    }

    /// Parses one escape sequence after a backslash at `self.pos`; returns the byte.
    /// On an invalid escape the character itself is returned and an error reported.
    fn escape(&mut self) -> u8 {
        let start = self.pos; // at the backslash
        self.pos += 1;
        let c = self.cur();
        let byte = match c {
            b'n' => Some(b'\n'),
            b't' => Some(b'\t'),
            b'r' => Some(b'\r'),
            b'0' => Some(0),
            b'\\' => Some(b'\\'),
            b'"' => Some(b'"'),
            b'\'' => Some(b'\''),
            b'x' => {
                let hi = (self.at(self.pos + 1) as char).to_digit(16);
                let lo = (self.at(self.pos + 2) as char).to_digit(16);
                match (hi, lo) {
                    (Some(h), Some(l)) => {
                        self.pos += 2;
                        Some((h * 16 + l) as u8)
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        if !self.eof() && c != b'\n' {
            self.pos += 1;
        }
        match byte {
            Some(b) => b,
            None => {
                let span = Span::new(start as u32, self.pos as u32);
                self.report(
                    Diagnostic::error("E0006", "invalid escape sequence", span)
                        .with_note(r#"valid escapes: \n \t \r \0 \\ \" \' \xHH"#),
                );
                c
            }
        }
    }

    fn string(&mut self) {
        let start = self.pos;
        self.pos += 1;
        let mut bytes = Vec::new();
        loop {
            let c = self.cur();
            if self.eof() || c == b'\n' {
                let span = self.span(start);
                self.report(
                    Diagnostic::error("E0002", "unterminated string literal", span)
                        .with_label("string starts here and never closes")
                        .with_note("string literals cannot span lines; use \\n for a line break"),
                );
                break;
            }
            if c == b'"' {
                self.pos += 1;
                break;
            }
            if c == b'\\' {
                let b = self.escape();
                bytes.push(b);
            } else {
                bytes.push(c);
                self.pos += 1;
            }
        }
        self.push(TokenKind::Str(bytes), start);
    }

    fn char_lit(&mut self) {
        let start = self.pos;
        self.pos += 1;
        let c = self.cur();
        let value = if c == b'\'' || c == b'\n' || self.eof() {
            let span = self.span(start);
            self.report(
                Diagnostic::error("E0005", "empty character literal", span)
                    .with_label("expected one byte between the quotes"),
            );
            if c == b'\'' {
                self.pos += 1;
            }
            self.push(TokenKind::Char(0), start);
            return;
        } else if c == b'\\' {
            self.escape()
        } else if c.is_ascii() {
            self.pos += 1;
            c
        } else {
            // A multi-byte UTF-8 character: consume it whole for a clean span.
            let ch_len = self
                .text
                .get(self.pos..)
                .and_then(|s| s.chars().next())
                .map_or(1, char::len_utf8);
            self.pos += ch_len;
            let span = self.span(start);
            self.report(
                Diagnostic::error("E0005", "character literal must be a single byte", span)
                    .with_note("use a string literal for UTF-8 text"),
            );
            b'?'
        };
        if self.cur() == b'\'' {
            self.pos += 1;
        } else {
            // Look for a closing quote nearby on the same line so that `'ab'`
            // produces one error, not an error plus stray tokens.
            let close = (self.pos..self.pos.saturating_add(16).min(self.src.len()))
                .take_while(|&i| self.at(i) != b'\n')
                .find(|&i| self.at(i) == b'\'');
            let (msg, label) = match close {
                Some(i) => {
                    self.pos = i + 1;
                    (
                        "character literal holds more than one byte",
                        "use a string literal for several bytes",
                    )
                }
                None => ("unterminated character literal", "expected closing `'`"),
            };
            let span = self.span(start);
            self.report(Diagnostic::error("E0005", msg, span).with_label(label));
        }
        self.push(TokenKind::Char(value), start);
    }

    fn symbol(&mut self) {
        let start = self.pos;
        let two: [u8; 2] = [self.cur(), self.peek()];
        let kind2 = match &two {
            b":=" => Some(TokenKind::Bind),
            b"<-" => Some(TokenKind::Store),
            b"<~" => Some(TokenKind::Move),
            b"->" => Some(TokenKind::Arrow),
            b".." => Some(TokenKind::DotDot),
            b"==" => Some(TokenKind::EqEq),
            b"!=" => Some(TokenKind::Ne),
            b"<=" => Some(TokenKind::Le),
            b">=" => Some(TokenKind::Ge),
            b"<<" => Some(TokenKind::Shl),
            b">>" => Some(TokenKind::Shr),
            _ => None,
        };
        if let Some(kind) = kind2 {
            self.pos += 2;
            self.push(kind, start);
            return;
        }
        let kind1 = match self.cur() {
            b':' => Some(TokenKind::Colon),
            b',' => Some(TokenKind::Comma),
            b'.' => Some(TokenKind::Dot),
            b'+' => Some(TokenKind::Plus),
            b'-' => Some(TokenKind::Minus),
            b'*' => Some(TokenKind::Star),
            b'/' => Some(TokenKind::Slash),
            b'%' => Some(TokenKind::Percent),
            b'<' => Some(TokenKind::Lt),
            b'>' => Some(TokenKind::Gt),
            b'&' => Some(TokenKind::Amp),
            b'|' => Some(TokenKind::Pipe),
            b'^' => Some(TokenKind::Caret),
            b'~' => Some(TokenKind::Tilde),
            b'(' => Some(TokenKind::LParen),
            b')' => Some(TokenKind::RParen),
            b'[' => Some(TokenKind::LBracket),
            b']' => Some(TokenKind::RBracket),
            b'{' => Some(TokenKind::LBrace),
            b'}' => Some(TokenKind::RBrace),
            _ => None,
        };
        if let Some(kind) = kind1 {
            self.pos += 1;
            self.push(kind, start);
            return;
        }
        self.invalid_char();
    }

    fn invalid_char(&mut self) {
        let start = self.pos;
        let ch = self
            .text
            .get(self.pos..)
            .and_then(|s| s.chars().next())
            .unwrap_or('\u{FFFD}');
        self.pos += ch.len_utf8().max(1);
        let span = self.span(start);
        let shown = if ch.is_control() {
            format!("U+{:04X}", ch as u32)
        } else {
            format!("`{ch}`")
        };
        let mut d = Diagnostic::error("E0001", format!("invalid character {shown}"), span);
        d = match ch {
            '=' => d.with_note("Oli-- binds with `:=`, stores with `<-` and compares with `==`"),
            ';' => d.with_note("statements end at the end of the line; `;` is not used"),
            '!' => d.with_note("`!` appears only in `!=`; boolean negation is `not`"),
            '#' | '@' => d.with_note("Oli-- has no preprocessor or attributes; procedure clauses go on the lines after the header"),
            _ => d,
        };
        self.report(d);
    }
}
