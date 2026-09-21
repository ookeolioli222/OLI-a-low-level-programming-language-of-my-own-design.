use crate::parser::{PResult, Parser};
use oli_ast::{Expr, ExprKind, Ident, Path, Type, TypeKind};
use oli_diag::Span;
use oli_lexer::{Kw, TokenKind};

impl Parser<'_> {
    /// Can the current token begin a type? Used after `addr`, which may stand alone.
    pub(crate) fn at_type_start(&self) -> bool {
        match self.kind() {
            TokenKind::Prim(_) | TokenKind::Ident(_) | TokenKind::LBracket => true,
            TokenKind::Kw(k) => matches!(
                k,
                Kw::Mmio
                    | Kw::Rw
                    | Kw::View
                    | Kw::Ref
                    | Kw::Addr
                    | Kw::Own
                    | Kw::Port
                    | Kw::Be
                    | Kw::Le
                    | Kw::Zone
                    | Kw::Never
                    | Kw::None
            ),
            _ => false,
        }
    }

    /// `type := simple_type [ "or" simple_type ]` — `or` binds loosest and does not nest.
    pub(crate) fn parse_type(&mut self) -> PResult<Type> {
        let ok = self.parse_simple_type()?;
        if !self.at_kw(Kw::Or) {
            return Ok(ok);
        }
        self.bump();
        let err = self.parse_simple_type()?;
        if self.at_kw(Kw::Or) {
            let span = self.span();
            self.error(
                "E0013",
                "a fallible type `T or E` cannot itself be fallible",
                span,
            );
            // Keep going after reporting: skip the extra `or T`.
            self.bump();
            let _ = self.parse_simple_type();
        }
        let span = ok.span.to(err.span);
        Ok(Type {
            kind: TypeKind::Fallible {
                ok: Box::new(ok),
                err: Box::new(err),
            },
            span,
        })
    }

    /// Parses a dotted name: `ident { "." ident }`.
    pub(crate) fn parse_path(&mut self, what: &str) -> PResult<Path> {
        let first = self.expect_ident(what)?;
        let mut span = first.span;
        let mut segments = vec![first];
        while self.at(&TokenKind::Dot) {
            self.bump();
            let seg = self.expect_member("a name after `.`")?;
            span = span.to(seg.span);
            segments.push(seg);
        }
        Ok(Path { segments, span })
    }

    pub(crate) fn parse_simple_type(&mut self) -> PResult<Type> {
        let start = self.span();
        let kind = match self.kind().clone() {
            TokenKind::Prim(p) => {
                self.bump();
                TypeKind::Prim(p)
            }
            TokenKind::Ident(_) => TypeKind::Named(self.parse_path("type name")?),
            TokenKind::LBracket => {
                self.bump();
                let len = self.parse_array_len()?;
                self.expect(&TokenKind::RBracket, "`]` after array length")?;
                let elem = self.parse_simple_type()?;
                TypeKind::Array {
                    len: Box::new(len),
                    elem: Box::new(elem),
                }
            }
            TokenKind::Kw(Kw::Mmio | Kw::Rw | Kw::View | Kw::Ref) => self.parse_view_or_ref()?,
            TokenKind::Kw(Kw::Addr) => {
                self.bump();
                if self.at_type_start() {
                    TypeKind::Addr(Some(Box::new(self.parse_simple_type()?)))
                } else {
                    TypeKind::Addr(None)
                }
            }
            TokenKind::Kw(Kw::Own) => {
                self.bump();
                TypeKind::Own(Box::new(self.parse_simple_type()?))
            }
            TokenKind::Kw(Kw::Port) => {
                self.bump();
                TypeKind::Port(self.expect_prim("a primitive type after `port`")?)
            }
            TokenKind::Kw(k @ (Kw::Be | Kw::Le)) => {
                self.bump();
                let prim =
                    self.expect_prim(&format!("a primitive integer type after `{}`", k.as_str()))?;
                TypeKind::Endian {
                    big: k == Kw::Be,
                    prim,
                }
            }
            TokenKind::Kw(Kw::Zone) => {
                self.bump();
                TypeKind::Zone
            }
            TokenKind::Kw(Kw::Never) => {
                self.bump();
                TypeKind::Never
            }
            TokenKind::Kw(Kw::None) => {
                self.bump();
                TypeKind::None
            }
            _ => return Err(self.expected("E0013", "a type")),
        };
        let span = Span::new(start.start, self.last_end_offset());
        Ok(Type { kind, span })
    }

    fn parse_view_or_ref(&mut self) -> PResult<TypeKind> {
        let mmio = self.eat_kw(Kw::Mmio);
        let rw = self.eat_kw(Kw::Rw);
        let is_view = match self.kind() {
            TokenKind::Kw(Kw::View) => true,
            TokenKind::Kw(Kw::Ref) => false,
            _ => return Err(self.expected("E0013", "`view` or `ref`")),
        };
        self.bump();
        let elem = Box::new(self.parse_simple_type()?);
        Ok(if is_view {
            TypeKind::View { mmio, rw, elem }
        } else {
            TypeKind::Ref { mmio, rw, elem }
        })
    }

    fn parse_array_len(&mut self) -> PResult<Expr> {
        let span = self.span();
        match self.kind().clone() {
            TokenKind::Int(v) => {
                self.bump();
                Ok(Expr {
                    kind: ExprKind::Int(v),
                    span,
                })
            }
            TokenKind::Ident(name) => {
                self.bump();
                Ok(Expr {
                    kind: ExprKind::Name(Ident { name, span }),
                    span,
                })
            }
            _ => Err(self.expected("E0013", "an array length (integer or constant name)")),
        }
    }

    pub(crate) fn expect_prim(&mut self, what: &str) -> PResult<oli_lexer::Prim> {
        match self.kind() {
            TokenKind::Prim(p) => {
                let p = *p;
                self.bump();
                Ok(p)
            }
            _ => Err(self.expected("E0013", what)),
        }
    }
}
