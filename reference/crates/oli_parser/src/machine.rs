use crate::parser::{Abort, PResult, Parser};
use oli_ast::{
    Clobber, ExprKind, Ident, MachineBlock, MachineLine, MachineOperand, MemTerm, MemTermKind,
};
use oli_diag::{Diagnostic, Span};
use oli_lexer::{Kw, TokenKind};

impl Parser<'_> {
    /// `machine ARCH NL { line NL } end NL`
    pub(crate) fn parse_machine(&mut self) -> PResult<MachineBlock> {
        let start = self.bump().span;
        let arch = self.expect_ident("an architecture name after `machine` (e.g. `x64`)")?;
        self.expect_nl();
        let mut lines = Vec::new();
        loop {
            self.start_line();
            match self.kind() {
                TokenKind::Kw(Kw::End) | TokenKind::Eof => break,
                _ => match self.parse_machine_line() {
                    Ok(line) => {
                        lines.push(line);
                        self.expect_nl();
                    }
                    Err(Abort) => self.sync_line(),
                },
            }
        }
        self.expect_end("`machine`", start);
        let span = self.span_from(start);
        Ok(MachineBlock { arch, lines, span })
    }

    fn parse_machine_line(&mut self) -> PResult<MachineLine> {
        match self.kind().clone() {
            // `in REG <- expr` directive, or the x86 `in` instruction (`in al, dx`).
            TokenKind::Kw(Kw::In) if matches!(self.nth(2).kind, TokenKind::Store) => {
                self.bump();
                let reg = self.expect_ident("a register name after `in`")?;
                self.expect(&TokenKind::Store, "`<-`")?;
                let value = self.parse_expr()?;
                Ok(MachineLine::In { reg, value })
            }
            TokenKind::Ident(s) if s == "out" && matches!(self.nth(2).kind, TokenKind::Arrow) => {
                self.bump();
                let reg = self.expect_ident("a register name after `out`")?;
                self.expect(&TokenKind::Arrow, "`->`")?;
                let target = self.parse_expr()?;
                if !matches!(
                    target.kind,
                    ExprKind::Name(_) | ExprKind::Field(..) | ExprKind::Index(..)
                ) {
                    self.report(
                        Diagnostic::error("E0017", "`out` must store into a place", target.span)
                            .with_note(
                            "write `out REG -> name`, `out REG -> x.field` or `out REG -> v[i]`",
                        ),
                    );
                    return Err(Abort);
                }
                Ok(MachineLine::Out { reg, target })
            }
            TokenKind::Kw(Kw::Clobber) => {
                self.bump();
                let mut list = Vec::new();
                loop {
                    match self.kind() {
                        TokenKind::Ident(s) if s == "memory" => {
                            let span = self.bump().span;
                            list.push(Clobber::Memory(span));
                        }
                        TokenKind::Ident(_) => {
                            list.push(Clobber::Reg(self.expect_ident("a register")?))
                        }
                        _ => {
                            return Err(self
                                .expected("E0019", "a register name or `memory` after `clobber`"))
                        }
                    }
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                Ok(MachineLine::Clobber(list))
            }
            TokenKind::Dot => {
                self.bump();
                let name = self.expect_ident("a label name after `.`")?;
                self.expect(&TokenKind::Colon, "`:` after the label name")?;
                Ok(MachineLine::Label(name))
            }
            TokenKind::Ident(_) | TokenKind::Kw(_) | TokenKind::Prim(_) => {
                let tok = self.bump();
                let text = match &tok.kind {
                    TokenKind::Ident(s) => s.clone(),
                    TokenKind::Kw(k) => k.as_str().to_string(),
                    TokenKind::Prim(p) => p.as_str().to_string(),
                    _ => String::new(),
                };
                let mnemonic = Ident {
                    name: text,
                    span: tok.span,
                };
                let mut operands = Vec::new();
                if !self.at_line_end() {
                    loop {
                        operands.push(self.parse_operand()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                let span = self.span_from(tok.span);
                Ok(MachineLine::Instr {
                    mnemonic,
                    operands,
                    span,
                })
            }
            _ => Err(self.expected(
                "E0019",
                "an instruction, `in`, `out`, `clobber` or a `.label:`",
            )),
        }
    }

    fn parse_operand(&mut self) -> PResult<MachineOperand> {
        match self.kind().clone() {
            TokenKind::Int(v) => {
                let span = self.bump().span;
                Ok(MachineOperand::Imm(self.imm(v, false, span)?))
            }
            TokenKind::Minus => {
                let start = self.bump().span;
                match self.kind().clone() {
                    TokenKind::Int(v) => {
                        self.bump();
                        let span = self.span_from(start);
                        Ok(MachineOperand::Imm(self.imm(v, true, span)?))
                    }
                    _ => Err(self.expected("E0019", "an integer after `-`")),
                }
            }
            TokenKind::Dot => {
                self.bump();
                Ok(MachineOperand::Label(
                    self.expect_ident("a label name after `.`")?,
                ))
            }
            TokenKind::Ident(s)
                if matches!(s.as_str(), "dword" | "qword")
                    && matches!(self.nth(1).kind, TokenKind::LBracket) =>
            {
                let size = self.expect_ident("size")?;
                self.parse_mem(Some(size))
            }
            TokenKind::Prim(p) if matches!(self.nth(1).kind, TokenKind::LBracket) => {
                let span = self.bump().span;
                let size = Ident {
                    name: p.as_str().to_string(),
                    span,
                };
                self.parse_mem(Some(size))
            }
            TokenKind::Ident(_) => Ok(MachineOperand::Name(self.parse_path("an operand")?)),
            TokenKind::LBracket => self.parse_mem(None),
            _ => Err(self.expected(
                "E0019",
                "an operand (register, symbol, immediate, `.label` or `[memory]`)",
            )),
        }
    }

    fn imm(&mut self, v: u128, negative: bool, span: Span) -> PResult<i128> {
        let Ok(v) = i128::try_from(v) else {
            return Err(self.error("E0019", "immediate operand is too large", span));
        };
        Ok(if negative { -v } else { v })
    }

    /// `[ term { ("+"|"-") term } ]` where `term := name [* n] | n | .label`.
    fn parse_mem(&mut self, size: Option<Ident>) -> PResult<MachineOperand> {
        let start = self.expect(&TokenKind::LBracket, "`[`")?;
        let mut terms = Vec::new();
        let mut negative = self.eat(&TokenKind::Minus);
        loop {
            let kind = match self.kind().clone() {
                TokenKind::Int(v) => {
                    self.bump();
                    MemTermKind::Int(v)
                }
                TokenKind::Dot => {
                    self.bump();
                    MemTermKind::Label(self.expect_ident("a label name after `.`")?)
                }
                TokenKind::Ident(_) => {
                    let path = self.parse_path("a register or symbol")?;
                    if self.eat(&TokenKind::Star) {
                        let scale = match self.kind().clone() {
                            TokenKind::Int(v) => {
                                self.bump();
                                v
                            }
                            _ => return Err(self.expected("E0019", "a scale factor after `*`")),
                        };
                        let path_span = path.span;
                        let mut segs = path.segments.into_iter();
                        match (segs.next(), segs.next()) {
                            (Some(reg), None) => MemTermKind::Scaled(reg, scale),
                            _ => {
                                return Err(self.error(
                                    "E0019",
                                    "only a register can be scaled",
                                    path_span,
                                ))
                            }
                        }
                    } else {
                        MemTermKind::Name(path)
                    }
                }
                _ => return Err(self.expected("E0019", "a memory operand term")),
            };
            terms.push(MemTerm { negative, kind });
            if self.eat(&TokenKind::Plus) {
                negative = false;
            } else if self.eat(&TokenKind::Minus) {
                negative = true;
            } else {
                break;
            }
        }
        self.expect(&TokenKind::RBracket, "`]` closing the memory operand")?;
        let span = self.span_from(start);
        Ok(MachineOperand::Mem { size, terms, span })
    }
}
