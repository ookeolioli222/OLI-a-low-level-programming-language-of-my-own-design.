use crate::parser::{Abort, PResult, Parser};
use oli_ast::{
    Arg, BinOp, Expr, ExprKind, FieldInit, Handler, Ident, Mode, Path, Type, TypeKind, UnOp,
};
use oli_diag::{Diagnostic, Span};
use oli_lexer::{Kw, Prim, TokenKind};

/// Binary operator precedence levels, loosest first (`spec` §5).
const LEVELS: usize = 9;

impl Parser<'_> {
    /// `expr := range [ "else" handler ]`
    pub(crate) fn parse_expr(&mut self) -> PResult<Expr> {
        self.descend()?;
        let r = self.parse_fallback();
        self.ascend();
        r
    }

    /// Parses an expression; on failure reports, skips to the end of the
    /// line (without consuming it) and returns an `Error` node so that the
    /// enclosing block statement can still be built.
    pub(crate) fn parse_expr_or_recover(&mut self) -> Expr {
        match self.parse_expr() {
            Ok(e) => e,
            Err(Abort) => {
                let span = self.here();
                self.recover_to_line_end();
                Expr {
                    kind: ExprKind::Error,
                    span,
                }
            }
        }
    }

    /// Where a fallback handler's operand may end without an expression.
    fn at_expr_end(&self) -> bool {
        matches!(
            self.kind(),
            TokenKind::Newline
                | TokenKind::Eof
                | TokenKind::RParen
                | TokenKind::RBracket
                | TokenKind::RBrace
                | TokenKind::Comma
                | TokenKind::Kw(Kw::Then | Kw::At | Kw::From | Kw::Else | Kw::End)
        )
    }

    fn parse_fallback(&mut self) -> PResult<Expr> {
        let e = self.parse_binary(0)?;
        if !self.at_kw(Kw::Else) {
            return Ok(e);
        }
        if self.in_then {
            let span = self.span();
            self.report(
                Diagnostic::error(
                    "E0016",
                    "ambiguous `else` inside a one-line `if ... then`",
                    span,
                )
                .with_label("is this a fallback or an else-branch?")
                .with_note("use the block form: `if cond` / statements / `else` / ... / `end`"),
            );
            return Err(Abort);
        }
        self.bump();
        let handler = if self.eat_kw(Kw::Fail) {
            Handler::Fail
        } else if self.eat_kw(Kw::Ret) {
            if self.at_expr_end() {
                Handler::Ret(None)
            } else {
                Handler::Ret(Some(Box::new(self.parse_binary(0)?)))
            }
        } else {
            Handler::Default(Box::new(self.parse_binary(0)?))
        };
        let span = self.span_from(e.span);
        Ok(Expr {
            kind: ExprKind::Fallback {
                expr: Box::new(e),
                handler,
            },
            span,
        })
    }

    /// `range := or_expr [ ".." [ or_expr ] ]` — inside `[ ]` and after `each ... in`.
    pub(crate) fn parse_range_expr(&mut self) -> PResult<Expr> {
        self.descend()?;
        let r = self.parse_range_inner();
        self.ascend();
        r
    }

    fn parse_range_inner(&mut self) -> PResult<Expr> {
        let start = self.parse_binary(0)?;
        if !self.eat(&TokenKind::DotDot) {
            return Ok(start);
        }
        let end = if self.at(&TokenKind::RBracket) || self.at_line_end() {
            None
        } else {
            Some(Box::new(self.parse_binary(0)?))
        };
        let span = self.span_from(start.span);
        Ok(Expr {
            kind: ExprKind::Range {
                start: Box::new(start),
                end,
            },
            span,
        })
    }

    fn binop_at_level(&self, level: usize) -> Option<BinOp> {
        let k = self.kind();
        match level {
            0 => matches!(k, TokenKind::Kw(Kw::Or)).then_some(BinOp::Or),
            1 => matches!(k, TokenKind::Kw(Kw::And)).then_some(BinOp::And),
            2 => match k {
                TokenKind::EqEq => Some(BinOp::Eq),
                TokenKind::Ne => Some(BinOp::Ne),
                TokenKind::Lt => Some(BinOp::Lt),
                TokenKind::Le => Some(BinOp::Le),
                TokenKind::Gt => Some(BinOp::Gt),
                TokenKind::Ge => Some(BinOp::Ge),
                _ => None,
            },
            3 => matches!(k, TokenKind::Pipe).then_some(BinOp::BitOr),
            4 => matches!(k, TokenKind::Caret).then_some(BinOp::BitXor),
            5 => matches!(k, TokenKind::Amp).then_some(BinOp::BitAnd),
            6 => match k {
                TokenKind::Shl => Some(BinOp::Shl),
                TokenKind::Shr => Some(BinOp::Shr),
                _ => None,
            },
            7 => match k {
                TokenKind::Plus => Some(BinOp::Add),
                TokenKind::Minus => Some(BinOp::Sub),
                _ => None,
            },
            8 => match k {
                TokenKind::Star => Some(BinOp::Mul),
                TokenKind::Slash => Some(BinOp::Div),
                TokenKind::Percent => Some(BinOp::Rem),
                _ => None,
            },
            _ => None,
        }
    }

    fn parse_binary(&mut self, level: usize) -> PResult<Expr> {
        if level >= LEVELS {
            return self.parse_unary();
        }
        let mut lhs = self.parse_binary(level + 1)?;
        while let Some(op) = self.binop_at_level(level) {
            self.bump();
            self.skip_newlines(); // a line ending in an operator continues (§1)
            let rhs = self.parse_binary(level + 1)?;
            let span = lhs.span.to(rhs.span);
            lhs = Expr {
                kind: ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)),
                span,
            };
            if level == 2 {
                if let Some(next) = self.binop_at_level(2) {
                    let span = self.span();
                    self.report(
                        Diagnostic::error("E0031", "comparisons do not chain", span)
                            .with_label(format!("second comparison `{}`", next.as_str()))
                            .with_note("write `a < b and b < c`"),
                    );
                    return Err(Abort);
                }
                break;
            }
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> PResult<Expr> {
        self.descend()?;
        let r = self.parse_unary_inner();
        self.ascend();
        r
    }

    fn parse_unary_inner(&mut self) -> PResult<Expr> {
        let start = self.span();
        let op = match self.kind() {
            TokenKind::Kw(Kw::Not) => Some(UnOp::Not),
            TokenKind::Minus => Some(UnOp::Neg),
            TokenKind::Tilde => Some(UnOp::BitNot),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let e = self.parse_unary()?;
            let span = start.to(e.span);
            return Ok(Expr {
                kind: ExprKind::Unary(op, Box::new(e)),
                span,
            });
        }
        match self.kind() {
            TokenKind::Kw(Kw::Addr) => self.parse_addr_expr(),
            TokenKind::Kw(Kw::Ref) => {
                self.bump();
                let e = self.parse_unary()?;
                let span = start.to(e.span);
                Ok(Expr {
                    kind: ExprKind::RefOf {
                        rw: false,
                        expr: Box::new(e),
                    },
                    span,
                })
            }
            TokenKind::Kw(Kw::Rw) => {
                self.bump();
                self.expect_kw(Kw::Ref)?;
                let e = self.parse_unary()?;
                let span = start.to(e.span);
                Ok(Expr {
                    kind: ExprKind::RefOf {
                        rw: true,
                        expr: Box::new(e),
                    },
                    span,
                })
            }
            _ => self.parse_postfix(),
        }
    }

    /// After `addr`: a conversion `addr T (e)` / `addr (e)` or an address-of `addr place`.
    fn parse_addr_expr(&mut self) -> PResult<Expr> {
        let start = self.bump().span;
        let target = match self.kind() {
            TokenKind::Prim(_) => {
                let elem = self.parse_simple_type()?;
                let span = self.span_from(start);
                Some(Type {
                    kind: TypeKind::Addr(Some(Box::new(elem))),
                    span,
                })
            }
            TokenKind::LParen => Some(Type {
                kind: TypeKind::Addr(None),
                span: start,
            }),
            TokenKind::Ident(_) if matches!(self.nth(1).kind, TokenKind::LParen) => {
                let id = self.expect_ident("type name")?;
                let span = start.to(id.span);
                let named = Type {
                    kind: TypeKind::Named(Path {
                        segments: vec![id],
                        span,
                    }),
                    span,
                };
                Some(Type {
                    kind: TypeKind::Addr(Some(Box::new(named))),
                    span,
                })
            }
            _ => None,
        };
        match target {
            Some(ty) => self.parse_conversion_call(ty, None, start),
            None => {
                let e = self.parse_unary()?;
                let span = start.to(e.span);
                Ok(Expr {
                    kind: ExprKind::AddrOf(Box::new(e)),
                    span,
                })
            }
        }
    }

    /// `T ( expr )` with the type already parsed.
    fn parse_conversion_call(
        &mut self,
        ty: Type,
        mode: Option<Mode>,
        start: Span,
    ) -> PResult<Expr> {
        let what = format!("`(` after conversion type `{}`", oli_ast::type_text(&ty));
        self.expect(&TokenKind::LParen, &what)?;
        let e = self.parse_expr()?;
        self.expect(&TokenKind::RParen, "`)` closing the conversion")?;
        let span = self.span_from(start);
        Ok(Expr {
            kind: ExprKind::Convert {
                ty,
                mode,
                expr: Box::new(e),
            },
            span,
        })
    }

    fn is_path_like(e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Name(_) => true,
            ExprKind::Field(base, _) => Self::is_path_like(base),
            _ => false,
        }
    }

    fn parse_postfix(&mut self) -> PResult<Expr> {
        let mut e = self.parse_primary()?;
        loop {
            match self.kind() {
                TokenKind::Dot => {
                    self.bump();
                    self.skip_newlines();
                    let name = self.expect_member("a field or method name after `.`")?;
                    let span = e.span.to(name.span);
                    e = Expr {
                        kind: ExprKind::Field(Box::new(e), name),
                        span,
                    };
                }
                TokenKind::LParen => {
                    let args = self.parse_args()?;
                    let span = self.span_from(e.span);
                    e = Expr {
                        kind: ExprKind::Call {
                            callee: Box::new(e),
                            args,
                        },
                        span,
                    };
                }
                TokenKind::LBracket => {
                    self.bump();
                    let idx = self.parse_range_expr()?;
                    self.expect(&TokenKind::RBracket, "`]`")?;
                    let span = self.span_from(e.span);
                    e = Expr {
                        kind: ExprKind::Index(Box::new(e), Box::new(idx)),
                        span,
                    };
                }
                TokenKind::LBrace if Self::is_path_like(&e) => {
                    let fields = self.parse_struct_fields()?;
                    let span = self.span_from(e.span);
                    e = Expr {
                        kind: ExprKind::StructLit {
                            path: Box::new(e),
                            fields,
                        },
                        span,
                    };
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn parse_primary(&mut self) -> PResult<Expr> {
        let span = self.span();
        let kind = match self.kind().clone() {
            TokenKind::Int(v) => {
                self.bump();
                ExprKind::Int(v)
            }
            TokenKind::Char(c) => {
                self.bump();
                ExprKind::Char(c)
            }
            TokenKind::Str(s) => {
                self.bump();
                ExprKind::Str(s)
            }
            TokenKind::Kw(Kw::True) => {
                self.bump();
                ExprKind::Bool(true)
            }
            TokenKind::Kw(Kw::False) => {
                self.bump();
                ExprKind::Bool(false)
            }
            TokenKind::Kw(Kw::None) => {
                self.bump();
                ExprKind::None
            }
            TokenKind::Ident(name) => {
                self.bump();
                ExprKind::Name(Ident { name, span })
            }
            TokenKind::Prim(p) => return self.parse_prim_primary(p),
            TokenKind::Kw(Kw::Port) => {
                self.bump();
                let prim = self.expect_prim("a primitive type after `port`")?;
                let ty = Type {
                    kind: TypeKind::Port(prim),
                    span: self.span_from(span),
                };
                return self.parse_conversion_call(ty, None, span);
            }
            TokenKind::Kw(k @ (Kw::Wrap | Kw::Sat | Kw::Checked)) => {
                self.bump();
                let mode = match k {
                    Kw::Wrap => Mode::Wrap,
                    Kw::Sat => Mode::Sat,
                    _ => Mode::Checked,
                };
                self.expect(&TokenKind::LParen, &format!("`(` after `{}`", k.as_str()))?;
                let e = self.parse_expr()?;
                self.expect(&TokenKind::RParen, "`)`")?;
                ExprKind::ModeExpr(mode, Box::new(e))
            }
            TokenKind::LParen => {
                self.bump();
                let mut e = self.parse_expr()?;
                self.expect(&TokenKind::RParen, "`)`")?;
                e.span = self.span_from(span);
                return Ok(e);
            }
            TokenKind::LBracket => {
                self.bump();
                let e = self.parse_expr()?;
                self.expect(&TokenKind::RBracket, "`]` closing the raw load")?;
                ExprKind::RawLoad(Box::new(e))
            }
            TokenKind::LBrace => ExprKind::ArrayLit(self.parse_array_items()?),
            TokenKind::Kw(Kw::Addr | Kw::Ref | Kw::Rw | Kw::Not)
            | TokenKind::Minus
            | TokenKind::Tilde => {
                return self.parse_unary();
            }
            _ => {
                let mut d = Diagnostic::error("E0012", "expected expression", self.here())
                    .with_label("expression expected here");
                if let TokenKind::Kw(k) = self.kind() {
                    d = d.with_note(format!(
                        "`{}` is a keyword and cannot start an expression",
                        k.as_str()
                    ));
                }
                self.report(d);
                return Err(Abort);
            }
        };
        let span = self.span_from(span);
        Ok(Expr { kind, span })
    }

    /// A primitive type in value position: `u8(x)`, `u8.wrap(x)`, or a bare `u8`.
    fn parse_prim_primary(&mut self, p: Prim) -> PResult<Expr> {
        let span = self.bump().span;
        let ty = Type {
            kind: TypeKind::Prim(p),
            span,
        };
        if self.at(&TokenKind::LParen) {
            return self.parse_conversion_call(ty, None, span);
        }
        if self.at(&TokenKind::Dot) {
            let followed_by_paren = matches!(self.nth(2).kind, TokenKind::LParen);
            let mode = match &self.nth(1).kind {
                TokenKind::Kw(Kw::Wrap) if followed_by_paren => Some(Mode::Wrap),
                TokenKind::Kw(Kw::Sat) if followed_by_paren => Some(Mode::Sat),
                TokenKind::Kw(Kw::Checked) if followed_by_paren => Some(Mode::Checked),
                TokenKind::Ident(m) if followed_by_paren && m == "bits" => Some(Mode::Bits),
                _ => None,
            };
            if let Some(mode) = mode {
                self.bump();
                self.bump();
                return self.parse_conversion_call(ty, Some(mode), span);
            }
        }
        Ok(Expr {
            kind: ExprKind::TypeRef(ty),
            span,
        })
    }

    /// `( [arg {, arg}] [,] )` — all positional or all named.
    fn parse_args(&mut self) -> PResult<Vec<Arg>> {
        self.expect(&TokenKind::LParen, "`(`")?;
        let mut args: Vec<Arg> = Vec::new();
        let mut mixed_reported = false;
        while !self.at(&TokenKind::RParen) {
            if self.at_eof() {
                return Err(self.expected("E0011", "`)` closing the argument list"));
            }
            let name = match (self.kind(), &self.nth(1).kind) {
                (TokenKind::Ident(_), TokenKind::Colon) => {
                    let id = self.expect_ident("argument name")?;
                    self.bump(); // `:`
                    Some(id)
                }
                _ => None,
            };
            let value = self.parse_expr()?;
            if let Some(first) = args.first() {
                if first.name.is_some() != name.is_some() && !mixed_reported {
                    mixed_reported = true;
                    self.report(
                        Diagnostic::error(
                            "E0022",
                            "cannot mix named and positional arguments",
                            value.span,
                        )
                        .with_note("use `f(a, b)` or `f(x: a, y: b)`, not both"),
                    );
                }
            }
            args.push(Arg { name, value });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RParen, "`,` or `)` in the argument list")?;
        Ok(args)
    }

    /// `{ name: expr, ... }`
    fn parse_struct_fields(&mut self) -> PResult<Vec<FieldInit>> {
        let brace = self.span();
        let brace_ends_line = matches!(self.nth_raw_after_current(), TokenKind::Newline);
        let result = self.parse_struct_fields_inner();
        if result.is_err() && brace_ends_line {
            self.report(
                Diagnostic::error("E0032", "`{` at the end of a line does not open a block", brace)
                    .with_note("Oli-- blocks are opened by a keyword and closed by `end`; `{ }` only builds literals"),
            );
        }
        result
    }

    fn parse_struct_fields_inner(&mut self) -> PResult<Vec<FieldInit>> {
        self.expect(&TokenKind::LBrace, "`{`")?;
        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            if self.at_eof() {
                return Err(self.expected("E0011", "`}` closing the literal"));
            }
            let name = self.expect_ident("a field name")?;
            self.expect(&TokenKind::Colon, "`:` after the field name")?;
            let value = self.parse_expr()?;
            fields.push(FieldInit { name, value });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace, "`,` or `}` in the literal")?;
        Ok(fields)
    }

    /// `{ expr, ... }`
    fn parse_array_items(&mut self) -> PResult<Vec<Expr>> {
        self.expect(&TokenKind::LBrace, "`{`")?;
        let mut items = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            if self.at_eof() {
                return Err(self.expected("E0011", "`}` closing the array literal"));
            }
            items.push(self.parse_expr()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace, "`,` or `}` in the array literal")?;
        Ok(items)
    }
}
