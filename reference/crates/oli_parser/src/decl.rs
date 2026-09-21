use crate::parser::{Abort, PResult, Parser};
use crate::stmt::BlockKind;
use oli_ast::{
    CallConv, ChoiceDecl, Clause, ClauseKind, Decl, DeclKind, Field, Import, LayoutDecl, Module,
    Param, ProcDecl, Variant,
};
use oli_diag::Diagnostic;
use oli_lexer::{Kw, TokenKind};

impl Parser<'_> {
    /// `module := [ "module" path NL ] { import } { decl }`
    pub(crate) fn parse_module(&mut self) -> Module {
        let mut m = Module {
            name: None,
            imports: Vec::new(),
            decls: Vec::new(),
        };
        self.skip_newlines();
        if self.at_kw(Kw::Module) {
            self.bump();
            match self.parse_path("a module name") {
                Ok(p) => m.name = Some(p),
                Err(Abort) => self.recover_to_line_end(),
            }
            self.expect_nl();
        }
        loop {
            self.skip_newlines();
            self.nl_since_last = false;
            self.abort_to_decl = false;
            match self.kind() {
                TokenKind::Eof => {
                    self.warn_dangling_docs();
                    break;
                }
                TokenKind::Kw(Kw::Import) => {
                    self.warn_dangling_docs();
                    if !m.decls.is_empty() {
                        let span = self.span();
                        self.report(
                            Diagnostic::error(
                                "E0023",
                                "imports must come before declarations",
                                span,
                            )
                            .with_note("move every `import` line to the top of the file"),
                        );
                    }
                    match self.parse_import() {
                        Ok(i) => {
                            m.imports.push(i);
                            self.expect_nl();
                        }
                        Err(Abort) => self.sync_line(),
                    }
                }
                TokenKind::Kw(Kw::Module) => {
                    let span = self.span();
                    self.report(
                        Diagnostic::error(
                            "E0024",
                            "`module` must be the first line of the file",
                            span,
                        )
                        .with_note("only one `module` line is allowed"),
                    );
                    self.sync_line();
                }
                TokenKind::Kw(Kw::End) => {
                    let span = self.span();
                    self.report(
                        Diagnostic::error("E0015", "unexpected `end` at module level", span)
                            .with_note(
                                "this `end` closes nothing; remove it or check the block above",
                            ),
                    );
                    self.sync_line();
                }
                TokenKind::Kw(Kw::Pub | Kw::Proc | Kw::Layout | Kw::Choice)
                | TokenKind::Ident(_) => match self.parse_decl() {
                    Ok(d) => m.decls.push(d),
                    Err(Abort) => self.sync_line(),
                },
                _ => {
                    let found = self.kind().describe();
                    let span = self.span();
                    self.report(
                        Diagnostic::error("E0015", format!("expected a declaration, found {found}"), span)
                            .with_note("a module contains `proc`, `layout`, `choice`, constants and static places"),
                    );
                    self.sync_line();
                }
            }
        }
        m
    }

    fn warn_dangling_docs(&mut self) {
        if let Some((_, span)) = self.pending_docs.first() {
            let span = *span;
            self.report(
                Diagnostic::warning(
                    "W0001",
                    "doc comment is not attached to a declaration",
                    span,
                )
                .with_note("`---` comments document the declaration that follows them"),
            );
            self.drop_docs();
        }
    }

    fn parse_import(&mut self) -> PResult<Import> {
        let start = self.bump().span;
        let path = self.parse_path("a module path after `import`")?;
        let alias = if self.kind().is_ident("as") {
            self.bump();
            Some(self.expect_name("an alias name after `as`")?)
        } else {
            None
        };
        let span = self.span_from(start);
        Ok(Import { path, alias, span })
    }

    fn parse_decl(&mut self) -> PResult<Decl> {
        let start = self.span();
        let doc = self.take_docs();
        let is_pub = self.eat_kw(Kw::Pub);
        let kind = match self.kind() {
            TokenKind::Kw(Kw::Proc) => DeclKind::Proc(self.parse_proc()?),
            TokenKind::Kw(Kw::Layout) => DeclKind::Layout(self.parse_layout()?),
            TokenKind::Kw(Kw::Choice) => DeclKind::Choice(self.parse_choice()?),
            TokenKind::Ident(_) => self.parse_const_or_static()?,
            _ => {
                return Err(
                    self.expected("E0015", "`proc`, `layout`, `choice` or a name after `pub`")
                )
            }
        };
        let span = self.span_from(start);
        Ok(Decl {
            is_pub,
            doc,
            kind,
            span,
        })
    }

    /// `NAME [: T] := expr` (constant) or `NAME : T [<- expr] NL { clause NL }` (static place).
    fn parse_const_or_static(&mut self) -> PResult<DeclKind> {
        let name = self.expect_name("a name")?;
        if self.eat(&TokenKind::Bind) {
            self.skip_newlines();
            let value = self.parse_expr()?;
            self.expect_nl();
            let clauses = self.parse_clauses(false);
            return Ok(DeclKind::Const {
                name,
                ty: None,
                value,
                clauses,
            });
        }
        if !self.at(&TokenKind::Colon) {
            let d = Diagnostic::error(
                "E0015",
                "expected `:=` or `: type` after the name",
                self.here(),
            )
            .with_note(
                "module-level items are `NAME := value` (constant) or `NAME : type` (static place)",
            );
            self.report(d);
            return Err(Abort);
        }
        self.bump();
        let ty = self.parse_type()?;
        if self.eat(&TokenKind::Bind) {
            self.skip_newlines();
            let value = self.parse_expr()?;
            self.expect_nl();
            let clauses = self.parse_clauses(false);
            return Ok(DeclKind::Const {
                name,
                ty: Some(ty),
                value,
                clauses,
            });
        }
        let init = if self.eat(&TokenKind::Store) {
            self.skip_newlines();
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect_nl();
        let clauses = self.parse_clauses(false);
        Ok(DeclKind::Static {
            name,
            ty,
            init,
            clauses,
        })
    }

    /// `proc NAME [ "(" params ")" ] [ "->" type ] NL { clause NL } block "end" NL`
    fn parse_proc(&mut self) -> PResult<ProcDecl> {
        let start = self.bump().span;
        let name = match self.expect_name("a procedure name after `proc`") {
            Ok(n) => n,
            Err(Abort) => {
                self.recover_to_line_end();
                oli_ast::Ident {
                    name: "<error>".to_string(),
                    span: self.here(),
                }
            }
        };
        let mut params = Vec::new();
        if self.at(&TokenKind::LParen) {
            match self.parse_params() {
                Ok(p) => params = p,
                Err(Abort) => self.recover_to_line_end(),
            }
        }
        let mut result = None;
        if self.eat(&TokenKind::Arrow) {
            match self.parse_type() {
                Ok(t) => result = Some(t),
                Err(Abort) => self.recover_to_line_end(),
            }
        }
        self.expect_nl();
        let clauses = self.parse_clauses(true);
        let body = self.parse_block(BlockKind::Proc);
        self.expect_end(&format!("`proc {}`", name.name), start);
        Ok(ProcDecl {
            name,
            params,
            result,
            clauses,
            body,
        })
    }

    fn parse_params(&mut self) -> PResult<Vec<Param>> {
        self.expect(&TokenKind::LParen, "`(`")?;
        let mut params = Vec::new();
        while !self.at(&TokenKind::RParen) {
            if self.at_eof() {
                return Err(self.expected("E0011", "`)` closing the parameter list"));
            }
            let name = self.expect_name("a parameter name")?;
            self.expect(&TokenKind::Colon, "`:` after the parameter name")?;
            let ty = self.parse_type()?;
            params.push(Param { name, ty });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RParen, "`,` or `)` in the parameter list")?;
        Ok(params)
    }

    /// Clause lines after a procedure header (`is_proc`) or a static place.
    fn parse_clauses(&mut self, is_proc: bool) -> Vec<Clause> {
        let mut clauses: Vec<Clause> = Vec::new();
        loop {
            self.skip_newlines();
            self.nl_since_last = false;
            let kw = match self.kind() {
                TokenKind::Kw(
                    k @ (Kw::Permit
                    | Kw::Calls
                    | Kw::Section
                    | Kw::Align
                    | Kw::Entry
                    | Kw::Export
                    | Kw::Traps),
                ) => *k,
                _ => break,
            };
            let start = self.bump().span;
            let kind = match self.parse_clause_body(kw) {
                Ok(k) => k,
                Err(Abort) => {
                    self.sync_line();
                    continue;
                }
            };
            let span = self.span_from(start);
            if !is_proc && !matches!(kind, ClauseKind::Section(_) | ClauseKind::Align(_)) {
                self.report(
                    Diagnostic::error(
                        "E0018",
                        format!("clause `{}` is not allowed on a static place", kw.as_str()),
                        span,
                    )
                    .with_note("static places accept only `section` and `align`"),
                );
            } else if !matches!(kind, ClauseKind::Permit(_))
                && clauses
                    .iter()
                    .any(|c| std::mem::discriminant(&c.kind) == std::mem::discriminant(&kind))
            {
                self.report(
                    Diagnostic::error("E0018", format!("duplicate clause `{}`", kw.as_str()), span)
                        .with_note("each clause may appear once; `permit` lines may be repeated"),
                );
            } else {
                clauses.push(Clause { kind, span });
            }
            self.expect_nl();
        }
        clauses
    }

    fn parse_clause_body(&mut self, kw: Kw) -> PResult<ClauseKind> {
        Ok(match kw {
            Kw::Permit => {
                let mut caps = vec![self.parse_path("a capability such as `memory.raw`")?];
                while self.eat(&TokenKind::Comma) {
                    caps.push(self.parse_path("a capability")?);
                }
                ClauseKind::Permit(caps)
            }
            Kw::Calls => {
                let name = match self.kind() {
                    TokenKind::Ident(s) => s.clone(),
                    TokenKind::Kw(Kw::None) => "none".to_string(),
                    _ => {
                        return Err(self.expected(
                            "E0018",
                            "a calling convention: `sysv`, `c`, `none` or `interrupt`",
                        ))
                    }
                };
                let span = self.bump().span;
                let cc = match name.as_str() {
                    "sysv" => CallConv::Sysv,
                    "c" => CallConv::C,
                    "none" => CallConv::None,
                    "interrupt" => CallConv::Interrupt,
                    other => {
                        let msg = format!("unknown calling convention `{other}`");
                        return Err(self.error("E0018", msg, span));
                    }
                };
                ClauseKind::Calls(cc)
            }
            Kw::Section => ClauseKind::Section(self.expect_string("a section name in quotes")?),
            Kw::Align => match self.kind().clone() {
                TokenKind::Int(v) => {
                    let span = self.bump().span;
                    if v == 0 || !v.is_power_of_two() {
                        self.report(
                            Diagnostic::error("E0018", "alignment must be a power of two", span)
                                .with_label("not a power of two"),
                        );
                    }
                    ClauseKind::Align(v)
                }
                _ => return Err(self.expected("E0018", "an integer alignment")),
            },
            Kw::Entry => ClauseKind::Entry,
            Kw::Export => {
                if let TokenKind::Str(_) = self.kind() {
                    ClauseKind::Export(Some(self.expect_string("a symbol name")?))
                } else {
                    ClauseKind::Export(None)
                }
            }
            Kw::Traps => ClauseKind::Traps,
            _ => return Err(self.expected("E0018", "a clause")),
        })
    }

    pub(crate) fn expect_string(&mut self, what: &str) -> PResult<Vec<u8>> {
        match self.kind().clone() {
            TokenKind::Str(s) => {
                self.bump();
                Ok(s)
            }
            _ => Err(self.expected("E0011", what)),
        }
    }

    /// `layout NAME [packed] [align N] NL { field NL } end NL`
    fn parse_layout(&mut self) -> PResult<LayoutDecl> {
        let start = self.bump().span;
        let name = self.expect_name("a layout name after `layout`")?;
        let packed = self.eat_kw(Kw::Packed);
        let align = self.parse_optional_align()?;
        self.expect_nl();
        let mut fields = Vec::new();
        loop {
            self.start_line();
            match self.kind() {
                TokenKind::Kw(Kw::End) | TokenKind::Eof => break,
                TokenKind::Ident(_) => match self.parse_field() {
                    Ok(f) => {
                        fields.push(f);
                        self.expect_nl();
                    }
                    Err(Abort) => self.sync_line(),
                },
                TokenKind::Kw(Kw::Proc | Kw::Layout | Kw::Choice | Kw::Pub) => {
                    let span = self.span();
                    self.report(
                        Diagnostic::error(
                            "E0014",
                            format!("missing `end` for `layout {}`", name.name),
                            span,
                        )
                        .with_secondary(start, "layout opened here"),
                    );
                    return Ok(LayoutDecl {
                        name,
                        packed,
                        align,
                        fields,
                    });
                }
                _ => {
                    self.expected("E0011", "a field `name : type`");
                    self.sync_line();
                }
            }
        }
        self.expect_end(&format!("`layout {}`", name.name), start);
        Ok(LayoutDecl {
            name,
            packed,
            align,
            fields,
        })
    }

    fn parse_optional_align(&mut self) -> PResult<Option<u128>> {
        if !self.eat_kw(Kw::Align) {
            return Ok(None);
        }
        match self.kind().clone() {
            TokenKind::Int(v) => {
                let span = self.bump().span;
                if v == 0 || !v.is_power_of_two() {
                    self.report(Diagnostic::error(
                        "E0018",
                        "alignment must be a power of two",
                        span,
                    ));
                }
                Ok(Some(v))
            }
            _ => Err(self.expected("E0018", "an integer after `align`")),
        }
    }

    /// `name : type [align N]`
    fn parse_field(&mut self) -> PResult<Field> {
        let start = self.span();
        let name = self.expect_name("a field name")?;
        self.expect(&TokenKind::Colon, "`:` after the field name")?;
        let ty = self.parse_type()?;
        let align = self.parse_optional_align()?;
        let span = self.span_from(start);
        Ok(Field {
            name,
            ty,
            align,
            span,
        })
    }

    /// `choice NAME NL { variant NL } end NL`
    fn parse_choice(&mut self) -> PResult<ChoiceDecl> {
        let start = self.bump().span;
        let name = self.expect_name("a choice name after `choice`")?;
        self.expect_nl();
        let mut variants = Vec::new();
        loop {
            self.start_line();
            match self.kind() {
                TokenKind::Kw(Kw::End) | TokenKind::Eof => break,
                TokenKind::Ident(_) => match self.parse_variant() {
                    Ok(v) => {
                        variants.push(v);
                        self.expect_nl();
                    }
                    Err(Abort) => self.sync_line(),
                },
                TokenKind::Kw(Kw::Proc | Kw::Layout | Kw::Choice | Kw::Pub) => {
                    let span = self.span();
                    self.report(
                        Diagnostic::error(
                            "E0014",
                            format!("missing `end` for `choice {}`", name.name),
                            span,
                        )
                        .with_secondary(start, "choice opened here"),
                    );
                    return Ok(ChoiceDecl { name, variants });
                }
                _ => {
                    self.expected("E0011", "a variant name");
                    self.sync_line();
                }
            }
        }
        self.expect_end(&format!("`choice {}`", name.name), start);
        Ok(ChoiceDecl { name, variants })
    }

    /// `name [ "{" field { "," field } "}" ]`
    fn parse_variant(&mut self) -> PResult<Variant> {
        let start = self.span();
        let name = self.expect_name("a variant name")?;
        let mut fields = Vec::new();
        if self.eat(&TokenKind::LBrace) {
            while !self.at(&TokenKind::RBrace) {
                if self.at_eof() {
                    return Err(self.expected("E0011", "`}` closing the variant payload"));
                }
                fields.push(self.parse_field()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::RBrace, "`,` or `}` in the variant payload")?;
        }
        let span = self.span_from(start);
        Ok(Variant { name, fields, span })
    }
}
