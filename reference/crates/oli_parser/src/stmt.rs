use crate::parser::{Abort, PResult, Parser};
use oli_ast::{Arm, Block, Expr, ExprKind, Pattern, PatternKind, Stmt, StmtKind, ZoneSource};
use oli_diag::{Diagnostic, Span};
use oli_lexer::{Kw, TokenKind};

/// Which keywords may legally end the block being parsed.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum BlockKind {
    Proc,
    If,
    Loop,
    Zone,
    CaseArm,
}

fn is_decl_keyword(k: &TokenKind) -> bool {
    matches!(
        k,
        TokenKind::Kw(Kw::Proc | Kw::Layout | Kw::Choice | Kw::Import | Kw::Module | Kw::Pub)
    )
}

fn is_block_stmt_keyword(k: &TokenKind) -> bool {
    matches!(
        k,
        TokenKind::Kw(Kw::If | Kw::While | Kw::Each | Kw::Loop | Kw::Zone | Kw::Case | Kw::Machine)
    )
}

impl Parser<'_> {
    /// Parses statements until a token that closes this block (`end`, or
    /// `else`/`elif`/`when` where the block kind allows). The closer is left
    /// for the caller. Never fails: bad statements are dropped line by line.
    pub(crate) fn parse_block(&mut self, kind: BlockKind) -> Block {
        let start = self.span();
        let mut stmts = Vec::new();
        if self.descend().is_err() {
            self.recover_to_line_end();
            return Block { stmts, span: start };
        }
        loop {
            self.start_line();
            match self.kind() {
                TokenKind::Kw(Kw::End) | TokenKind::Eof => break,
                TokenKind::Kw(k @ (Kw::Else | Kw::Elif)) => {
                    if kind == BlockKind::If || (kind == BlockKind::CaseArm && *k == Kw::Else) {
                        break;
                    }
                    let span = self.span();
                    let msg = format!("`{}` without a matching `if`", k.as_str());
                    self.error("E0011", msg, span);
                    self.sync_line();
                }
                TokenKind::Kw(Kw::When) => {
                    if kind == BlockKind::CaseArm {
                        break;
                    }
                    let span = self.span();
                    self.error("E0011", "`when` outside of a `case`", span);
                    self.sync_line();
                }
                k if is_decl_keyword(k) => {
                    let span = self.span();
                    let what = k.describe();
                    self.report(
                        Diagnostic::error(
                            "E0014",
                            format!("{what} is not allowed inside a procedure body"),
                            span,
                        )
                        .with_note("a declaration here usually means a missing `end` above"),
                    );
                    self.abort_to_decl = true;
                    break;
                }
                _ => match self.parse_stmt() {
                    Ok(s) => stmts.push(s),
                    Err(Abort) => self.sync_line(),
                },
            }
            if self.abort_to_decl {
                break;
            }
        }
        self.ascend();
        let span = Span::new(start.start, self.last_end_offset().max(start.start));
        Block { stmts, span }
    }

    /// Consumes the `end` closing a block, or reports `E0014` when the file
    /// ended first. Also consumes the newline after `end`.
    pub(crate) fn expect_end(&mut self, what: &str, opened: Span) {
        if self.abort_to_decl {
            return;
        }
        if self.at_kw(Kw::End) {
            self.bump();
            self.expect_nl();
        } else {
            let span = self.here();
            self.report(
                Diagnostic::error("E0014", format!("missing `end` for {what}"), span)
                    .with_label("`end` expected before the file ends")
                    .with_secondary(opened, format!("{what} opened here")),
            );
        }
    }

    pub(crate) fn parse_stmt(&mut self) -> PResult<Stmt> {
        let start = self.span();
        let kind = match self.kind().clone() {
            TokenKind::Kw(Kw::If) => return self.parse_if(),
            TokenKind::Kw(Kw::While) => {
                self.bump();
                let cond = self.parse_expr_or_recover();
                self.expect_nl();
                let body = self.parse_block(BlockKind::Loop);
                self.expect_end("`while`", start);
                StmtKind::While { cond, body }
            }
            TokenKind::Kw(Kw::Each) => {
                self.bump();
                let var = self.expect_name("a loop variable name")?;
                self.expect_kw(Kw::In)?;
                let iter = match self.parse_range_expr() {
                    Ok(e) => e,
                    Err(Abort) => {
                        let span = self.here();
                        self.recover_to_line_end();
                        Expr {
                            kind: ExprKind::Error,
                            span,
                        }
                    }
                };
                self.expect_nl();
                let body = self.parse_block(BlockKind::Loop);
                self.expect_end("`each`", start);
                StmtKind::Each { var, iter, body }
            }
            TokenKind::Kw(Kw::Loop) => {
                self.bump();
                self.expect_nl();
                let body = self.parse_block(BlockKind::Loop);
                self.expect_end("`loop`", start);
                StmtKind::Loop { body }
            }
            TokenKind::Kw(Kw::Zone) => return self.parse_zone(),
            TokenKind::Kw(Kw::Case) => return self.parse_case(),
            TokenKind::Kw(Kw::Machine) => StmtKind::Machine(self.parse_machine()?),
            TokenKind::Kw(Kw::Break) => {
                self.bump();
                self.expect_nl();
                StmtKind::Break
            }
            TokenKind::Kw(Kw::Continue) => {
                self.bump();
                self.expect_nl();
                StmtKind::Continue
            }
            TokenKind::Kw(k @ (Kw::Ret | Kw::Fail)) => {
                self.bump();
                let value = if self.at_line_end() {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.expect_nl();
                if k == Kw::Ret {
                    StmtKind::Ret(value)
                } else {
                    StmtKind::Fail(value)
                }
            }
            TokenKind::Kw(Kw::Then) => {
                let span = self.span();
                return Err(self.error("E0011", "`then` without a preceding `if` condition", span));
            }
            TokenKind::Ident(_)
                if matches!(self.nth(1).kind, TokenKind::Bind | TokenKind::Colon) =>
            {
                return self.parse_declaration_stmt();
            }
            _ => return self.parse_expr_stmt(),
        };
        let span = self.span_from(start);
        Ok(Stmt { kind, span })
    }

    /// `name := e`, `name : T := e`, `name : T`, `name : T <- e`
    fn parse_declaration_stmt(&mut self) -> PResult<Stmt> {
        let start = self.span();
        let name = self.expect_name("a name")?;
        let kind = if self.eat(&TokenKind::Bind) {
            self.skip_newlines();
            let value = self.parse_expr()?;
            StmtKind::Bind {
                name,
                ty: None,
                value,
            }
        } else {
            self.expect(&TokenKind::Colon, "`:`")?;
            let ty = self.parse_type()?;
            if self.eat(&TokenKind::Bind) {
                self.skip_newlines();
                let value = self.parse_expr()?;
                StmtKind::Bind {
                    name,
                    ty: Some(ty),
                    value,
                }
            } else if self.eat(&TokenKind::Store) {
                self.skip_newlines();
                let init = self.parse_expr()?;
                StmtKind::Place {
                    name,
                    ty,
                    init: Some(init),
                }
            } else {
                StmtKind::Place {
                    name,
                    ty,
                    init: None,
                }
            }
        };
        self.expect_nl();
        let span = self.span_from(start);
        Ok(Stmt { kind, span })
    }

    fn is_lvalue(e: &Expr) -> bool {
        matches!(
            e.kind,
            ExprKind::Name(_) | ExprKind::Field(..) | ExprKind::Index(..) | ExprKind::RawLoad(_)
        )
    }

    /// An expression statement, a store `lvalue <- e` or a move `lvalue <~ e`.
    fn parse_expr_stmt(&mut self) -> PResult<Stmt> {
        let start = self.span();
        let e = self.parse_expr()?;
        let kind = match self.kind() {
            TokenKind::Store | TokenKind::Move => {
                let is_move = self.at(&TokenKind::Move);
                if !Self::is_lvalue(&e) {
                    self.report(
                        Diagnostic::error("E0017", "cannot store into this expression", e.span)
                            .with_label("not a place, field, element or raw address")
                            .with_note("stores target `name`, `x.field`, `v[i]` or `[addr]`"),
                    );
                    return Err(Abort);
                }
                self.bump();
                self.skip_newlines();
                let value = self.parse_expr()?;
                if is_move {
                    StmtKind::Move { target: e, value }
                } else {
                    StmtKind::Store { target: e, value }
                }
            }
            TokenKind::Bind => {
                self.report(
                    Diagnostic::error("E0011", "`:=` needs a plain name on its left", e.span)
                        .with_note("a binding is `name := expr`; to write into memory use `<-`"),
                );
                return Err(Abort);
            }
            _ => StmtKind::Expr(e),
        };
        self.expect_nl();
        let span = self.span_from(start);
        Ok(Stmt { kind, span })
    }

    fn parse_if(&mut self) -> PResult<Stmt> {
        let start = self.bump().span;
        let cond = self.parse_expr_or_recover();
        if self.eat_kw(Kw::Then) {
            if self.at_line_end() {
                return Err(self.expected("E0012", "a statement after `then`"));
            }
            let stmt = if is_block_stmt_keyword(self.kind()) {
                let span = self.span();
                self.report(
                    Diagnostic::error("E0021", "expected a simple statement after `then`", span)
                        .with_note("use the block form `if cond` / ... / `end` for nested blocks"),
                );
                // Parse the block statement anyway so that its `end` is consumed.
                self.parse_stmt()?
            } else {
                self.in_then = true;
                let stmt = self.parse_stmt();
                self.in_then = false;
                stmt?
            };
            let span = self.span_from(start);
            return Ok(Stmt {
                kind: StmtKind::IfThen {
                    cond,
                    stmt: Box::new(stmt),
                },
                span,
            });
        }
        self.expect_nl();
        let then = self.parse_block(BlockKind::If);
        let mut elifs = Vec::new();
        let mut els = None;
        loop {
            if self.abort_to_decl {
                break;
            }
            if self.eat_kw(Kw::Elif) {
                let c = self.parse_expr_or_recover();
                self.expect_nl();
                let b = self.parse_block(BlockKind::If);
                elifs.push((c, b));
            } else if self.eat_kw(Kw::Else) {
                self.expect_nl();
                els = Some(self.parse_block(BlockKind::Loop)); // no further else/elif allowed
                break;
            } else {
                break;
            }
        }
        self.expect_end("`if`", start);
        let span = self.span_from(start);
        Ok(Stmt {
            kind: StmtKind::If {
                cond,
                then,
                elifs,
                els,
            },
            span,
        })
    }

    fn parse_zone(&mut self) -> PResult<Stmt> {
        let start = self.bump().span;
        let name = self.expect_name("a zone name")?;
        let size = self.parse_expr_or_recover();
        let source = if self.eat_kw(Kw::At) {
            Some(ZoneSource::At(self.parse_expr_or_recover()))
        } else if self.eat_kw(Kw::From) {
            Some(ZoneSource::From(self.parse_expr_or_recover()))
        } else {
            None
        };
        self.expect_nl();
        let body = self.parse_block(BlockKind::Zone);
        self.expect_end("`zone`", start);
        let span = self.span_from(start);
        Ok(Stmt {
            kind: StmtKind::Zone {
                name,
                size,
                source,
                body,
            },
            span,
        })
    }

    fn parse_case(&mut self) -> PResult<Stmt> {
        let start = self.bump().span;
        let scrutinee = self.parse_expr_or_recover();
        self.expect_nl();
        let mut arms = Vec::new();
        let mut els = None;
        loop {
            self.start_line();
            if self.abort_to_decl {
                break;
            }
            match self.kind() {
                TokenKind::Kw(Kw::When) => {
                    let arm_start = self.bump().span;
                    let pattern = match self.parse_pattern() {
                        Ok(p) => p,
                        Err(Abort) => {
                            let span = self.here();
                            self.recover_to_line_end();
                            Pattern {
                                kind: PatternKind::Error,
                                span,
                            }
                        }
                    };
                    self.expect_nl();
                    let body = self.parse_block(BlockKind::CaseArm);
                    let span = self.span_from(arm_start);
                    arms.push(Arm {
                        pattern,
                        body,
                        span,
                    });
                }
                TokenKind::Kw(Kw::Else) => {
                    self.bump();
                    self.expect_nl();
                    els = Some(self.parse_block(BlockKind::Loop));
                    break;
                }
                TokenKind::Kw(Kw::End) | TokenKind::Eof => break,
                _ => {
                    let span = self.span();
                    self.report(
                        Diagnostic::error(
                            "E0011",
                            "expected `when`, `else` or `end` in a `case`",
                            span,
                        )
                        .with_note("every arm of a `case` starts with `when pattern`"),
                    );
                    self.sync_line();
                }
            }
        }
        self.expect_end("`case`", start);
        let span = self.span_from(start);
        Ok(Stmt {
            kind: StmtKind::Case {
                scrutinee,
                arms,
                els,
            },
            span,
        })
    }

    /// `ok [name]` | `fail [sub]` | sub
    fn parse_pattern(&mut self) -> PResult<Pattern> {
        let start = self.span();
        let kind = match self.kind() {
            TokenKind::Ident(s) if s == "ok" => {
                self.bump();
                let name = if let TokenKind::Ident(_) = self.kind() {
                    Some(self.expect_name("a name")?)
                } else {
                    None
                };
                PatternKind::Ok(name)
            }
            TokenKind::Kw(Kw::Fail) => {
                self.bump();
                if self.at_line_end() {
                    PatternKind::Fail(None)
                } else {
                    PatternKind::Fail(Some(Box::new(self.parse_sub_pattern()?)))
                }
            }
            _ => return self.parse_sub_pattern(),
        };
        let span = self.span_from(start);
        Ok(Pattern { kind, span })
    }

    fn parse_sub_pattern(&mut self) -> PResult<Pattern> {
        let start = self.span();
        let kind = match self.kind().clone() {
            TokenKind::Ident(_) => {
                let name = self.expect_ident("a pattern")?;
                let mut fields = Vec::new();
                if self.eat(&TokenKind::LBrace) {
                    while !self.at(&TokenKind::RBrace) {
                        if self.at_eof() {
                            return Err(self.expected("E0011", "`}` closing the pattern"));
                        }
                        fields.push(self.expect_name("a field name")?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(&TokenKind::RBrace, "`,` or `}` in the pattern")?;
                }
                PatternKind::Variant { name, fields }
            }
            TokenKind::Int(v) => {
                self.bump();
                PatternKind::Int {
                    negative: false,
                    value: v,
                }
            }
            TokenKind::Minus => {
                self.bump();
                match self.kind().clone() {
                    TokenKind::Int(v) => {
                        self.bump();
                        PatternKind::Int {
                            negative: true,
                            value: v,
                        }
                    }
                    _ => return Err(self.expected("E0011", "an integer after `-` in a pattern")),
                }
            }
            TokenKind::Char(c) => {
                self.bump();
                PatternKind::Char(c)
            }
            TokenKind::Kw(Kw::True) => {
                self.bump();
                PatternKind::Bool(true)
            }
            TokenKind::Kw(Kw::False) => {
                self.bump();
                PatternKind::Bool(false)
            }
            TokenKind::Kw(Kw::None) => {
                self.bump();
                PatternKind::None
            }
            _ => return Err(self.expected("E0011", "a pattern")),
        };
        let span = self.span_from(start);
        Ok(Pattern { kind, span })
    }
}
