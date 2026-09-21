use oli_diag::{Diagnostic, Diagnostics, SourceFile, Span};
use oli_lexer::{Kw, Token, TokenKind, RESERVED_WORDS};

/// Marker: an error was already reported; the caller must recover.
#[derive(Debug)]
pub(crate) struct Abort;

pub(crate) type PResult<T> = Result<T, Abort>;

/// Deepest nesting of expressions/blocks the parser accepts before giving up
/// with `E0030`. Generous for real code, small enough to keep the recursive
/// descent within a 2 MiB thread stack.
pub(crate) const MAX_DEPTH: u32 = 96;

pub(crate) struct Parser<'a> {
    tokens: Vec<Token>,
    eof: Token,
    pos: usize,
    diags: &'a mut Diagnostics,
    /// Depth of unclosed `( [ {`; newlines are trivia while it is positive.
    nest: u32,
    depth: u32,
    /// Set while parsing the statement after a one-line `if ... then`.
    pub(crate) in_then: bool,
    /// Set when a body contains a declaration keyword: unwind to module level.
    pub(crate) abort_to_decl: bool,
    /// Doc comments seen since the last statement/declaration boundary.
    pub(crate) pending_docs: Vec<(String, Span)>,
    /// End offset of the last consumed significant (non-newline) token;
    /// anchors "expected X here".
    last_end: u32,
    /// A line ended between the last significant token and the current one.
    pub(crate) nl_since_last: bool,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(
        tokens: Vec<Token>,
        file: &'a SourceFile,
        diags: &'a mut Diagnostics,
    ) -> Parser<'a> {
        let eof = Token {
            kind: TokenKind::Eof,
            span: Span::point(file.len()),
        };
        let mut p = Parser {
            tokens,
            eof,
            pos: 0,
            diags,
            nest: 0,
            depth: 0,
            in_then: false,
            abort_to_decl: false,
            pending_docs: Vec::new(),
            last_end: 0,
            nl_since_last: false,
        };
        p.skip_trivia();
        p
    }

    // ------------------------------------------------------------ cursor

    fn raw(&self, i: usize) -> &Token {
        self.tokens.get(i).unwrap_or(&self.eof)
    }

    fn is_trivia(&self, t: &Token) -> bool {
        match t.kind {
            TokenKind::Doc(_) => true,
            TokenKind::Newline => self.nest > 0,
            _ => false,
        }
    }

    fn skip_trivia(&mut self) {
        loop {
            let t = self.raw(self.pos);
            if !self.is_trivia(t) {
                break;
            }
            match &t.kind {
                TokenKind::Doc(text) => {
                    let span = t.span;
                    self.pending_docs.push((text.clone(), span));
                }
                TokenKind::Newline => self.nl_since_last = true,
                _ => {}
            }
            self.pos += 1;
        }
    }

    /// The current token (never trivia).
    pub(crate) fn peek(&self) -> &Token {
        self.raw(self.pos)
    }

    pub(crate) fn kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    /// The raw token right after the current one (trivia included); used to
    /// detect a `{` that ends its line.
    pub(crate) fn nth_raw_after_current(&self) -> &TokenKind {
        &self.raw(self.pos + 1).kind
    }

    /// The `n`-th token after the current one, skipping trivia.
    pub(crate) fn nth(&self, n: usize) -> &Token {
        let mut i = self.pos;
        let mut left = n;
        loop {
            let t = self.raw(i);
            if matches!(t.kind, TokenKind::Eof) {
                return t;
            }
            if !self.is_trivia(t) {
                if left == 0 {
                    return t;
                }
                left -= 1;
            }
            i += 1;
        }
    }

    pub(crate) fn span(&self) -> Span {
        self.peek().span
    }

    /// End offset of the last consumed token.
    pub(crate) fn last_end_offset(&self) -> u32 {
        self.last_end
    }

    /// Span from `start` to the end of the last consumed token.
    pub(crate) fn span_from(&self, start: Span) -> Span {
        Span::new(start.start, self.last_end.max(start.start))
    }

    /// Consumes the current token and returns it.
    pub(crate) fn bump(&mut self) -> Token {
        let t = self.raw(self.pos).clone();
        if !matches!(t.kind, TokenKind::Eof) {
            self.pos += 1;
        }
        match t.kind {
            TokenKind::Newline => self.nl_since_last = true,
            TokenKind::Eof => {}
            _ => {
                self.last_end = t.span.end;
                self.nl_since_last = false;
            }
        }
        match t.kind {
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => self.nest += 1,
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                self.nest = self.nest.saturating_sub(1)
            }
            _ => {}
        }
        self.skip_trivia();
        t
    }

    pub(crate) fn at(&self, k: &TokenKind) -> bool {
        self.kind() == k
    }

    pub(crate) fn at_kw(&self, k: Kw) -> bool {
        self.kind().is_kw(k)
    }

    pub(crate) fn at_eof(&self) -> bool {
        matches!(self.kind(), TokenKind::Eof)
    }

    pub(crate) fn at_line_end(&self) -> bool {
        matches!(self.kind(), TokenKind::Newline | TokenKind::Eof)
    }

    pub(crate) fn eat(&mut self, k: &TokenKind) -> bool {
        if self.at(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    pub(crate) fn eat_kw(&mut self, k: Kw) -> bool {
        if self.at_kw(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Skips newlines for a line continuation (after an operator). Keeps the
    /// "a line ended" flag so a missing operand is reported on the previous line.
    pub(crate) fn skip_newlines(&mut self) {
        while matches!(self.kind(), TokenKind::Newline) {
            self.bump();
        }
    }

    /// Statement boundary: skips blank lines, drops doc comments seen inside
    /// bodies, and resets error anchoring to the new line.
    pub(crate) fn start_line(&mut self) {
        self.skip_newlines();
        self.drop_docs();
        self.nl_since_last = false;
    }

    // ------------------------------------------------------- diagnostics

    pub(crate) fn report(&mut self, d: Diagnostic) {
        self.diags.push(d);
    }

    pub(crate) fn error(
        &mut self,
        code: &'static str,
        msg: impl Into<String>,
        span: Span,
    ) -> Abort {
        self.diags.push(Diagnostic::error(code, msg, span));
        Abort
    }

    /// Span for "expected X here": the current token, or — when the line
    /// has ended since the previous significant token — the point right
    /// after that token, so a missing operand is reported where it belongs.
    pub(crate) fn here(&self) -> Span {
        if self.at_line_end() || self.nl_since_last {
            Span::point(self.last_end)
        } else {
            self.span()
        }
    }

    pub(crate) fn expected(&mut self, code: &'static str, what: &str) -> Abort {
        let found = self.kind().describe();
        let span = self.here();
        let msg = format!("expected {what}, found {found}");
        self.diags
            .push(Diagnostic::error(code, msg, span).with_label(format!("{what} expected here")));
        Abort
    }

    /// Consumes `k` or reports `E0011`.
    pub(crate) fn expect(&mut self, k: &TokenKind, what: &str) -> PResult<Span> {
        if self.at(k) {
            Ok(self.bump().span)
        } else {
            Err(self.expected("E0011", what))
        }
    }

    pub(crate) fn expect_kw(&mut self, k: Kw) -> PResult<Span> {
        if self.at_kw(k) {
            Ok(self.bump().span)
        } else {
            Err(self.expected("E0011", &format!("`{}`", k.as_str())))
        }
    }

    /// An identifier in a *use* position (no reserved-word check).
    pub(crate) fn expect_ident(&mut self, what: &str) -> PResult<oli_ast::Ident> {
        match self.kind() {
            TokenKind::Ident(name) => {
                let name = name.clone();
                let span = self.bump().span;
                Ok(oli_ast::Ident { name, span })
            }
            _ => Err(self.expected("E0011", what)),
        }
    }

    /// A member name after `.`: an identifier or any keyword (`v.addr`,
    /// `Header.at(v)`, `io.port`). Keywords are ordinary names in this position.
    pub(crate) fn expect_member(&mut self, what: &str) -> PResult<oli_ast::Ident> {
        let name = match self.kind() {
            TokenKind::Ident(name) => name.clone(),
            TokenKind::Kw(k) => k.as_str().to_string(),
            TokenKind::Prim(p) => p.as_str().to_string(),
            _ => return Err(self.expected("E0011", what)),
        };
        let span = self.bump().span;
        Ok(oli_ast::Ident { name, span })
    }

    /// An identifier being *declared*: reserved words are rejected (`E0010`)
    /// but still returned so parsing continues.
    pub(crate) fn expect_name(&mut self, what: &str) -> PResult<oli_ast::Ident> {
        let id = self.expect_ident(what)?;
        if RESERVED_WORDS.contains(&id.name.as_str()) {
            let msg = format!(
                "`{}` is a reserved word and cannot be used as a name",
                id.name
            );
            self.report(
                Diagnostic::error("E0010", msg, id.span)
                    .with_note("reserved for a later version of Oli--"),
            );
        }
        Ok(id)
    }

    /// End of statement: a newline (consumed) or end of file. Anything else is
    /// `E0020`; the rest of the line is skipped so parsing resumes cleanly.
    pub(crate) fn expect_nl(&mut self) {
        match self.kind() {
            TokenKind::Newline => {
                self.bump();
            }
            TokenKind::Eof => {}
            TokenKind::Store => {
                let span = self.span();
                self.report(
                    Diagnostic::error("E0020", "expected end of line, found `<-`", span)
                        .with_label("store operator in an expression")
                        .with_note("`<-` stores into a place; for a comparison against a negative number write `< -1`"),
                );
                self.sync_line();
            }
            _ => {
                self.expected("E0020", "end of line");
                self.sync_line();
            }
        }
    }

    // ---------------------------------------------------------- recovery

    /// Skips to the end of the current line without consuming the newline.
    ///
    /// A block keyword (`end`, `else`, `elif`, `when`) that starts a line is
    /// never skipped: the error happened *before* it (typically a dangling
    /// operator on the previous line) and the keyword must still close its block.
    pub(crate) fn recover_to_line_end(&mut self) {
        self.nest = 0;
        if self.nl_since_last && self.at_block_keyword() {
            return;
        }
        while !self.at_line_end() {
            self.bump();
        }
    }

    fn at_block_keyword(&self) -> bool {
        matches!(
            self.kind(),
            TokenKind::Kw(Kw::End | Kw::Else | Kw::Elif | Kw::When)
        )
    }

    /// Skips the rest of the line including its newline.
    pub(crate) fn sync_line(&mut self) {
        self.recover_to_line_end();
        if matches!(self.kind(), TokenKind::Newline) {
            self.bump();
        }
    }

    pub(crate) fn descend(&mut self) -> PResult<()> {
        if self.depth >= MAX_DEPTH {
            let span = self.span();
            return Err(self.error("E0030", "code is nested too deeply", span));
        }
        self.depth += 1;
        Ok(())
    }

    pub(crate) fn ascend(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    pub(crate) fn take_docs(&mut self) -> Vec<String> {
        self.pending_docs.drain(..).map(|(t, _)| t).collect()
    }

    pub(crate) fn drop_docs(&mut self) {
        self.pending_docs.clear();
    }
}
