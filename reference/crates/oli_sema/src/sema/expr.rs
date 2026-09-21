//! Expression checking: bidirectional integer typing, implicit conversions,
//! places versus values, calls and intrinsics, conversions, fallbacks.

use super::body::Ctx;
use super::{Namespace, Sema, Symbol};
use crate::hir::*;
use crate::regions::Regions;
use crate::ty::{IntTy, Ty};
use oli_diag::{Diagnostic, Span};

/// A place expression may fail (error reported) or simply not be a place.
type PlaceResult = Result<Option<Place>, ()>;

pub(crate) fn is_aggregate(t: &Ty) -> bool {
    matches!(
        t,
        Ty::Array(..) | Ty::Layout(_) | Ty::Choice(_) | Ty::Fallible(..)
    )
}

/// The value type read from memory of type `t` (`be u16` reads as `u16`).
pub(crate) fn value_ty(t: &Ty) -> Ty {
    match t {
        Ty::Endian { int, .. } => Ty::Int(*int),
        other => other.clone(),
    }
}

impl Sema<'_> {
    /// Checks `e`; when `expected` is given the result is coerced to it.
    pub(crate) fn check_expr(
        &mut self,
        ctx: &mut Ctx,
        e: &oli_ast::Expr,
        expected: Option<&Ty>,
    ) -> Expr {
        let v = self.check_expr_inner(ctx, e, expected);
        match expected {
            Some(t) => self.coerce(ctx, v, t),
            None => v,
        }
    }

    /// `E0207` for a call without a value.
    pub(crate) fn require_value(&mut self, ctx: &Ctx, e: Expr) -> Expr {
        if matches!(e.ty, Ty::Unit) {
            self.error(ctx.module, "E0207", "this call produces no value", e.span);
            return Expr::error(e.span);
        }
        e
    }

    /// `E0201` when an integer literal never received a type.
    pub(crate) fn settle_untyped_error(&mut self, ctx: &Ctx, ty: Ty, span: Span) -> Ty {
        if matches!(ty, Ty::UntypedInt) {
            self.report(
                ctx.module,
                Diagnostic::error("E0201", "integer literal needs a type", span).with_note(
                    "write `x : u32 := 10` or `u32(10)`; Oli-- never guesses an integer width",
                ),
            );
            return Ty::Error;
        }
        ty
    }

    /// Gives an untyped integer expression the concrete type `to` (range-checked).
    fn settle(&mut self, ctx: &Ctx, e: Expr, to: IntTy) -> Expr {
        match e.kind {
            ExprKind::Int(v) => {
                if !to.fits(v) {
                    let msg = format!("`{v}` does not fit in `{}`", to.as_str());
                    self.report(
                        ctx.module,
                        Diagnostic::error("E0202", msg, e.span).with_note(format!(
                            "`{}` holds {} .. {}",
                            to.as_str(),
                            to.min(),
                            to.max()
                        )),
                    );
                }
                Expr::int(v, to, e.span)
            }
            _ => Expr {
                ty: Ty::Int(to),
                ..e
            },
        }
    }

    /// Implicit conversions (`spec` §2). Reports `E0200`/`E0202` on failure.
    pub(crate) fn coerce(&mut self, ctx: &mut Ctx, e: Expr, to: &Ty) -> Expr {
        if e.ty.is_error() || to.is_error() || matches!(e.ty, Ty::Never) {
            return e;
        }
        let span = e.span;
        match (&e.ty, to) {
            (Ty::UntypedInt, Ty::Int(i)) => self.settle(ctx, e, *i),
            (Ty::UntypedInt, Ty::Physaddr) => {
                let v = self.settle(ctx, e, IntTy::Uword);
                Expr {
                    kind: ExprKind::Convert {
                        kind: ConvKind::IntToPhys,
                        expr: Box::new(v),
                    },
                    ty: Ty::Physaddr,
                    regions: Regions::stat(),
                    span,
                }
            }
            (Ty::UntypedInt, Ty::Addr(_)) => {
                self.require_permit(
                    ctx,
                    Capability::MemoryRaw,
                    "using an integer as a raw address",
                    span,
                );
                let v = self.settle(ctx, e, IntTy::Uword);
                Expr {
                    kind: ExprKind::Convert {
                        kind: ConvKind::IntToAddr,
                        expr: Box::new(v),
                    },
                    ty: to.clone(),
                    regions: Regions::stat(),
                    span,
                }
            }
            (Ty::Int(a), Ty::Int(b)) => {
                if a.same_repr(*b) {
                    Expr {
                        ty: to.clone(),
                        ..e
                    }
                } else if a.widens_to(*b) {
                    Expr {
                        kind: ExprKind::Convert {
                            kind: ConvKind::Widen,
                            expr: Box::new(e),
                        },
                        ty: to.clone(),
                        regions: Regions::stat(),
                        span,
                    }
                } else {
                    let msg = format!(
                        "`{}` does not convert to `{}` without loss",
                        a.as_str(),
                        b.as_str()
                    );
                    self.report(
                        ctx.module,
                        Diagnostic::error("E0202", msg, span)
                            .with_note(format!("say what should happen: `{b}.wrap(x)`, `{b}.sat(x)` or `{b}.checked(x)`")),
                    );
                    Expr::error(span)
                }
            }
            (
                Ty::View {
                    mmio: m1,
                    rw: true,
                    elem: e1,
                },
                Ty::View {
                    mmio: m2,
                    rw: false,
                    elem: e2,
                },
            )
            | (
                Ty::Ref {
                    mmio: m1,
                    rw: true,
                    elem: e1,
                },
                Ty::Ref {
                    mmio: m2,
                    rw: false,
                    elem: e2,
                },
            ) if m1 == m2 && e1.same(e2) => Expr {
                ty: to.clone(),
                ..e
            },
            (Ty::Ref { elem, .. }, t) if !matches!(t, Ty::Ref { .. }) && value_ty(elem).same(t) => {
                let elem = (**elem).clone();
                let regions = e.regions.clone();
                let rw = matches!(e.ty, Ty::Ref { rw: true, .. });
                let place = Place {
                    kind: PlaceKind::Deref(Box::new(e)),
                    ty: elem,
                    rw,
                    regions,
                    span,
                };
                self.load_place(ctx, place)
            }
            _ if e.ty.same(to) => e,
            _ => {
                let (have, want) = (self.type_text(&e.ty), self.type_text(to));
                self.report(
                    ctx.module,
                    Diagnostic::error("E0200", format!("expected `{want}`, found `{have}`"), span),
                );
                Expr::error(span)
            }
        }
    }

    /// Loads a place: definite-assignment check for locals, array → view.
    pub(crate) fn load_place(&mut self, ctx: &mut Ctx, place: Place) -> Expr {
        if let Some(root) = Self::root_local(&place) {
            if !ctx.flow.is_assigned(root) {
                let name = ctx.locals[root].name.clone();
                self.report(
                    ctx.module,
                    Diagnostic::error(
                        "E0220",
                        format!("`{name}` is read before it is stored"),
                        place.span,
                    )
                    .with_note("frame places hold no defined value until the first `<-`"),
                );
                ctx.flow.set_assigned(root); // report once
            }
        }
        let span = place.span;
        match &place.ty {
            Ty::Array(_, elem) => {
                let ty = Ty::View {
                    mmio: false,
                    rw: place.rw,
                    elem: elem.clone(),
                };
                let regions = place.regions.clone();
                Expr {
                    kind: ExprKind::ViewOfArray(place),
                    ty,
                    regions,
                    span,
                }
            }
            t => {
                let ty = value_ty(t);
                let regions = if self.type_carries_region(&ty) {
                    place.regions.clone()
                } else {
                    Regions::stat()
                };
                Expr {
                    kind: ExprKind::Load(place),
                    ty,
                    regions,
                    span,
                }
            }
        }
    }

    fn root_local(place: &Place) -> Option<LocalId> {
        match &place.kind {
            PlaceKind::Local(id) => Some(*id),
            PlaceKind::Field(p, _) | PlaceKind::ArrayElem(p, _) => Self::root_local(p),
            _ => None,
        }
    }

    /// Is `e` an integer expression without a type of its own (literals,
    /// unannotated constants and arithmetic over them)?
    fn is_untyped(&self, ctx: &Ctx, e: &oli_ast::Expr) -> bool {
        match &e.kind {
            oli_ast::ExprKind::Int(_) => true,
            oli_ast::ExprKind::Unary(oli_ast::UnOp::Neg | oli_ast::UnOp::BitNot, x) => {
                self.is_untyped(ctx, x)
            }
            oli_ast::ExprKind::Binary(op, l, r) => {
                !matches!(
                    op,
                    oli_ast::BinOp::Eq
                        | oli_ast::BinOp::Ne
                        | oli_ast::BinOp::Lt
                        | oli_ast::BinOp::Le
                        | oli_ast::BinOp::Gt
                        | oli_ast::BinOp::Ge
                        | oli_ast::BinOp::And
                        | oli_ast::BinOp::Or
                ) && self.is_untyped(ctx, l)
                    && self.is_untyped(ctx, r)
            }
            oli_ast::ExprKind::Name(id) => {
                ctx.lookup_local(&id.name).is_none()
                    && match self.lookup_module_symbol(ctx.module, &id.name) {
                        Some(Symbol::Const(c)) => {
                            matches!(self.prog.consts[c].ty, Ty::UntypedInt | Ty::Error)
                        }
                        _ => false,
                    }
            }
            oli_ast::ExprKind::ModeExpr(oli_ast::Mode::Wrap | oli_ast::Mode::Sat, x) => {
                self.is_untyped(ctx, x)
            }
            _ => false,
        }
    }

    fn check_expr_inner(
        &mut self,
        ctx: &mut Ctx,
        e: &oli_ast::Expr,
        expected: Option<&Ty>,
    ) -> Expr {
        let span = e.span;
        match &e.kind {
            oli_ast::ExprKind::Int(v) => match i128::try_from(*v) {
                Ok(v) => Expr {
                    kind: ExprKind::Int(v),
                    ty: Ty::UntypedInt,
                    regions: Regions::stat(),
                    span,
                },
                Err(_) => {
                    self.error(ctx.module, "E0003", "integer literal is too large", span);
                    Expr::error(span)
                }
            },
            oli_ast::ExprKind::Char(c) => Expr::int(i128::from(*c), IntTy::U8, span),
            oli_ast::ExprKind::Str(s) => Expr {
                kind: ExprKind::Str(s.clone()),
                ty: Ty::view_u8(false),
                regions: Regions::stat(),
                span,
            },
            oli_ast::ExprKind::Bool(b) => Expr {
                kind: ExprKind::Bool(*b),
                ty: Ty::Bool,
                regions: Regions::stat(),
                span,
            },
            oli_ast::ExprKind::None => Expr {
                kind: ExprKind::NoneVal,
                ty: Ty::None,
                regions: Regions::stat(),
                span,
            },
            oli_ast::ExprKind::Name(id)
                if ctx
                    .lookup_local(&id.name)
                    .is_some_and(|l| ctx.locals[l].kind != LocalKind::Place) =>
            {
                self.check_name_value(ctx, id, expected, span)
            }
            oli_ast::ExprKind::Name(_)
            | oli_ast::ExprKind::Field(..)
            | oli_ast::ExprKind::Index(..)
            | oli_ast::ExprKind::RawLoad(_) => match self.check_place_opt(ctx, e) {
                Err(()) => Expr::error(span),
                Ok(Some(place)) => self.load_place(ctx, place),
                Ok(None) => self.check_value_path(ctx, e, expected),
            },
            oli_ast::ExprKind::TypeRef(t) => {
                let ty = self.resolve_type(ctx.module, t);
                let text = self.type_text(&ty);
                self.error(
                    ctx.module,
                    "E0200",
                    format!("`{text}` is a type, not a value"),
                    span,
                );
                Expr::error(span)
            }
            oli_ast::ExprKind::Range { .. } => {
                self.error(
                    ctx.module,
                    "E0200",
                    "a range is only valid inside `[ ]` or after `each ... in`",
                    span,
                );
                Expr::error(span)
            }
            oli_ast::ExprKind::Call { callee, args } => self.check_call(ctx, callee, args, span),
            oli_ast::ExprKind::Unary(op, x) => self.check_unary(ctx, *op, x, expected, span),
            oli_ast::ExprKind::Binary(op, l, r) => {
                self.check_binary(ctx, *op, l, r, expected, span)
            }
            oli_ast::ExprKind::AddrOf(x) => self.check_addr_of(ctx, x, span),
            oli_ast::ExprKind::RefOf { rw, expr } => self.check_ref_of(ctx, *rw, expr, span),
            oli_ast::ExprKind::ModeExpr(mode, x) => {
                self.check_mode_expr(ctx, *mode, x, expected, span)
            }
            oli_ast::ExprKind::Convert { ty, mode, expr } => {
                self.check_convert(ctx, ty, *mode, expr, span)
            }
            oli_ast::ExprKind::Fallback { expr, handler } => {
                self.check_fallback(ctx, expr, handler, span)
            }
            oli_ast::ExprKind::StructLit { path, fields } => {
                self.check_struct_lit(ctx, path, fields, expected, span)
            }
            oli_ast::ExprKind::ArrayLit(items) => self.check_array_lit(ctx, items, expected, span),
            oli_ast::ExprKind::Error => Expr::error(span),
        }
    }

    // ------------------------------------------------------------ places

    /// Recognizes place syntax: a local place or static, a `ref` binding
    /// (dereferenced), a field of a layout place, an element of an array
    /// place or view, or `[a]`. `Ok(None)` means "not a place".
    pub(crate) fn check_place_opt(&mut self, ctx: &mut Ctx, e: &oli_ast::Expr) -> PlaceResult {
        let span = e.span;
        match &e.kind {
            oli_ast::ExprKind::Name(id) => self.name_place(ctx, id, span),
            oli_ast::ExprKind::Field(base, name) => {
                let Some(base_place) = self.check_place_opt(ctx, base)? else {
                    return Ok(None);
                };
                match &base_place.ty {
                    Ty::Layout(id) => {
                        let id = *id;
                        self.layout_ready(id);
                        match self.prog.layouts[id]
                            .fields
                            .iter()
                            .position(|f| f.name == name.name)
                        {
                            Some(idx) => {
                                let ty = self.prog.layouts[id].fields[idx].ty.clone();
                                let (rw, regions) = (base_place.rw, base_place.regions.clone());
                                let kind = PlaceKind::Field(Box::new(base_place), idx);
                                Ok(Some(Place {
                                    kind,
                                    ty,
                                    rw,
                                    regions,
                                    span,
                                }))
                            }
                            None => {
                                let lname = self.prog.layouts[id].name.clone();
                                let msg = format!("layout `{lname}` has no field `{}`", name.name);
                                self.error(ctx.module, "E0105", msg, name.span);
                                Err(())
                            }
                        }
                    }
                    Ty::Choice(id) => {
                        let cname = self.prog.choices[*id].name.clone();
                        let msg =
                            format!("`{cname}` is a choice; its payload is reached with `case`");
                        self.error(ctx.module, "E0105", msg, name.span);
                        Err(())
                    }
                    Ty::Error => Err(()),
                    _ => Ok(None),
                }
            }
            oli_ast::ExprKind::Index(base, idx)
                if !matches!(idx.kind, oli_ast::ExprKind::Range { .. }) =>
            {
                self.index_place(ctx, base, idx, span)
            }
            oli_ast::ExprKind::RawLoad(a) => {
                let a = self.check_expr(ctx, a, None);
                let a = self.require_value(ctx, a);
                self.require_permit(
                    ctx,
                    Capability::MemoryRaw,
                    "dereferencing a raw address",
                    span,
                );
                match a.ty.clone() {
                    Ty::Addr(t) => {
                        let kind = PlaceKind::Raw(Box::new(a));
                        Ok(Some(Place {
                            kind,
                            ty: *t,
                            rw: true,
                            regions: Regions::stat(),
                            span,
                        }))
                    }
                    Ty::Error => Err(()),
                    other => {
                        let t = self.type_text(&other);
                        self.error(
                            ctx.module,
                            "E0200",
                            format!("`[ ]` needs an `addr T`, found `{t}`"),
                            a.span,
                        );
                        Err(())
                    }
                }
            }
            _ => Ok(None),
        }
    }

    fn name_place(&mut self, ctx: &mut Ctx, id: &oli_ast::Ident, span: Span) -> PlaceResult {
        if let Some(l) = ctx.lookup_local(&id.name) {
            ctx.used[l] = true;
            let def = ctx.locals[l].clone();
            return Ok(match def.kind {
                LocalKind::Place => {
                    let regions = Regions::one(ctx.scope_region(l));
                    Some(Place {
                        kind: PlaceKind::Local(l),
                        ty: def.ty,
                        rw: true,
                        regions,
                        span,
                    })
                }
                _ => match &def.ty {
                    Ty::Ref { rw, elem, .. } => {
                        let regions = ctx.local_regions[l].clone();
                        let base = Expr {
                            kind: ExprKind::Local(l),
                            ty: def.ty.clone(),
                            regions: regions.clone(),
                            span,
                        };
                        let kind = PlaceKind::Deref(Box::new(base));
                        Some(Place {
                            kind,
                            ty: (**elem).clone(),
                            rw: *rw,
                            regions,
                            span,
                        })
                    }
                    _ => None,
                },
            });
        }
        match self.lookup_module_symbol(ctx.module, &id.name) {
            Some(Symbol::Static(s)) => {
                self.static_ready(s);
                let def = &self.prog.statics[s];
                let (ty, rw) = (def.ty.clone(), !def.readonly);
                Ok(Some(Place {
                    kind: PlaceKind::Static(s),
                    ty,
                    rw,
                    regions: Regions::stat(),
                    span,
                }))
            }
            Some(Symbol::Const(c)) => {
                self.const_ready(c);
                match self.const_backing.get(c).copied().flatten() {
                    Some(s) => {
                        let ty = self.prog.statics[s].ty.clone();
                        Ok(Some(Place {
                            kind: PlaceKind::Static(s),
                            ty,
                            rw: false,
                            regions: Regions::stat(),
                            span,
                        }))
                    }
                    None => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    fn index_place(
        &mut self,
        ctx: &mut Ctx,
        base: &oli_ast::Expr,
        idx: &oli_ast::Expr,
        span: Span,
    ) -> PlaceResult {
        let base_place = self.check_place_opt(ctx, base)?;
        let (container, elem, rw, regions) = match base_place {
            Some(p) => match &p.ty {
                Ty::Array(_, elem) => {
                    let elem = (**elem).clone();
                    let (rw, regions) = (p.rw, p.regions.clone());
                    let index = self.check_index_value(ctx, idx);
                    let kind = PlaceKind::ArrayElem(Box::new(p), Box::new(index));
                    return Ok(Some(Place {
                        kind,
                        ty: elem,
                        rw,
                        regions,
                        span,
                    }));
                }
                Ty::View { .. } => {
                    let v = self.load_place(ctx, p);
                    let Ty::View { rw, elem, .. } = v.ty.clone() else {
                        return Err(());
                    };
                    let regions = v.regions.clone();
                    (v, *elem, rw, regions)
                }
                Ty::Error => return Err(()),
                other => {
                    let t = self.type_text(other);
                    self.error(
                        ctx.module,
                        "E0200",
                        format!("cannot index a value of type `{t}`"),
                        base.span,
                    );
                    return Err(());
                }
            },
            None => {
                let v = self.check_expr(ctx, base, None);
                let v = self.require_value(ctx, v);
                match v.ty.clone() {
                    Ty::View { rw, elem, .. } => {
                        let regions = v.regions.clone();
                        (v, *elem, rw, regions)
                    }
                    Ty::Error => return Err(()),
                    other => {
                        let t = self.type_text(&other);
                        self.error(
                            ctx.module,
                            "E0200",
                            format!("cannot index a value of type `{t}`"),
                            base.span,
                        );
                        return Err(());
                    }
                }
            }
        };
        let index = self.check_index_value(ctx, idx);
        let kind = PlaceKind::ViewElem(Box::new(container), Box::new(index));
        Ok(Some(Place {
            kind,
            ty: elem,
            rw,
            regions,
            span,
        }))
    }

    fn check_index_value(&mut self, ctx: &mut Ctx, idx: &oli_ast::Expr) -> Expr {
        let i = self.check_expr(ctx, idx, None);
        let i = self.require_value(ctx, i);
        match &i.ty {
            Ty::UntypedInt => self.coerce(ctx, i, &Ty::uword()),
            Ty::Int(t) if !t.signed() => self.coerce(ctx, i, &Ty::uword()),
            Ty::Error => i,
            other => {
                let t = self.type_text(other);
                self.report(
                    ctx.module,
                    Diagnostic::error("E0200", format!("an index must be an unsigned integer, found `{t}`"), i.span)
                        .with_note("convert explicitly with `uword.bits(i)` if the value is known to be non-negative"),
                );
                Expr::error(i.span)
            }
        }
    }

    /// A store target: a writable place (`E0110` for bindings, `E0111` for read-only memory).
    pub(crate) fn check_place(
        &mut self,
        ctx: &mut Ctx,
        e: &oli_ast::Expr,
        need_rw: bool,
    ) -> Option<Place> {
        match self.check_place_opt(ctx, e) {
            Err(()) => None,
            Ok(Some(p)) => {
                if need_rw && !p.rw {
                    let what = match &p.kind {
                        PlaceKind::Static(_) => "a constant is read-only",
                        PlaceKind::Deref(_) => "this `ref` is not `rw`",
                        PlaceKind::ViewElem(..) => "this view is not `rw`",
                        _ => "this memory is read-only",
                    };
                    self.report(
                        ctx.module,
                        Diagnostic::error("E0111", "cannot store through read-only memory", e.span)
                            .with_label(what),
                    );
                    return None;
                }
                Some(p)
            }
            Ok(None) => {
                if let oli_ast::ExprKind::Name(id) = &e.kind {
                    if ctx.lookup_local(&id.name).is_some() {
                        self.report(
                            ctx.module,
                            Diagnostic::error("E0110", format!("cannot store into the binding `{}`", id.name), e.span)
                                .with_note(format!("a binding is immutable; declare a place with `{} : T` to store into it", id.name)),
                        );
                        return None;
                    }
                }
                self.report(
                    ctx.module,
                    Diagnostic::error("E0017", "this expression is not a place", e.span)
                        .with_note("stores target `name`, `x.field`, `v[i]` or `[addr]`"),
                );
                None
            }
        }
    }

    // ------------------------------------------------------------ values

    /// Names, fields and indexes that are not places: bindings, constants,
    /// view members, type members, module members, slices.
    fn check_value_path(
        &mut self,
        ctx: &mut Ctx,
        e: &oli_ast::Expr,
        expected: Option<&Ty>,
    ) -> Expr {
        let span = e.span;
        match &e.kind {
            oli_ast::ExprKind::Name(id) => self.check_name_value(ctx, id, expected, span),
            oli_ast::ExprKind::Field(base, name) => {
                self.check_field_value(ctx, base, name, expected, span)
            }
            oli_ast::ExprKind::Index(base, idx) => {
                let oli_ast::ExprKind::Range { start, end } = &idx.kind else {
                    return Expr::error(span);
                };
                let v = self.check_expr(ctx, base, None);
                let v = self.require_value(ctx, v);
                let Ty::View { .. } = &v.ty else {
                    if !v.ty.is_error() {
                        let t = self.type_text(&v.ty);
                        self.error(
                            ctx.module,
                            "E0200",
                            format!("cannot slice a value of type `{t}`"),
                            base.span,
                        );
                    }
                    return Expr::error(span);
                };
                let start_e = self.check_index_value(ctx, start);
                let end_e = end
                    .as_ref()
                    .map(|x| Box::new(self.check_index_value(ctx, x)));
                let (ty, regions) = (v.ty.clone(), v.regions.clone());
                Expr {
                    kind: ExprKind::Slice {
                        view: Box::new(v),
                        start: Box::new(start_e),
                        end: end_e,
                    },
                    ty,
                    regions,
                    span,
                }
            }
            _ => Expr::error(span),
        }
    }

    fn check_name_value(
        &mut self,
        ctx: &mut Ctx,
        id: &oli_ast::Ident,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        // A bare variant name where a choice is expected (`fail too_short`, `ret dot`).
        let expected_choice = match expected {
            Some(Ty::Choice(c)) => Some(*c),
            Some(Ty::Fallible(_, e)) => match &**e {
                Ty::Choice(c) => Some(*c),
                _ => None,
            },
            _ => None,
        };
        if let Some(c) = expected_choice {
            if ctx.lookup_local(&id.name).is_none()
                && self.lookup_module_symbol(ctx.module, &id.name).is_none()
            {
                self.choice_ready(c);
                if self.prog.choices[c]
                    .variants
                    .iter()
                    .any(|v| v.name == id.name)
                {
                    return self.choice_member(ctx, c, id, expected, span);
                }
            }
        }
        if let Some(l) = ctx.lookup_local(&id.name) {
            ctx.used[l] = true;
            let ty = ctx.locals[l].ty.clone();
            let regions = ctx.local_regions[l].clone();
            return Expr {
                kind: ExprKind::Local(l),
                ty,
                regions,
                span,
            };
        }
        match self.lookup_module_symbol(ctx.module, &id.name) {
            Some(Symbol::Const(c)) => self.const_value_expr(ctx, c, span),
            Some(Symbol::Proc(_)) => {
                self.report(
                    ctx.module,
                    Diagnostic::error("E0900", "feature not implemented: procedure values", span)
                        .with_note("call the procedure, or take its code address with `addr name` under `permit memory.raw`"),
                );
                Expr::error(span)
            }
            Some(Symbol::Layout(_) | Symbol::Choice(_)) => {
                self.error(
                    ctx.module,
                    "E0200",
                    format!("`{}` is a type, not a value", id.name),
                    span,
                );
                Expr::error(span)
            }
            Some(Symbol::Module(_)) => {
                self.error(
                    ctx.module,
                    "E0200",
                    format!("`{}` is a module, not a value", id.name),
                    span,
                );
                Expr::error(span)
            }
            Some(Symbol::Static(_)) => Expr::error(span), // handled as a place
            None => {
                if Namespace::from_name(&id.name).is_some() {
                    let msg = format!("`{}` is an intrinsic namespace, not a value", id.name);
                    self.error(ctx.module, "E0200", msg, span);
                } else {
                    self.unknown_name(ctx.module, &id.name, span);
                }
                Expr::error(span)
            }
        }
    }

    /// The value of a scalar constant (aggregates are places, see `name_place`).
    fn const_value_expr(&mut self, ctx: &Ctx, c: ConstId, span: Span) -> Expr {
        self.const_ready(c);
        let def = &self.prog.consts[c];
        match (&def.ty, &def.value) {
            (Ty::UntypedInt, ConstValue::Int(v)) => Expr {
                kind: ExprKind::Int(*v),
                ty: Ty::UntypedInt,
                regions: Regions::stat(),
                span,
            },
            (Ty::Int(i), ConstValue::Int(v)) => Expr::int(*v, *i, span),
            (Ty::Physaddr, ConstValue::Int(v)) => {
                let inner = Expr::int(*v, IntTy::Uword, span);
                let kind = ExprKind::Convert {
                    kind: ConvKind::IntToPhys,
                    expr: Box::new(inner),
                };
                Expr {
                    kind,
                    ty: Ty::Physaddr,
                    regions: Regions::stat(),
                    span,
                }
            }
            (Ty::Bool, ConstValue::Bool(b)) => Expr {
                kind: ExprKind::Bool(*b),
                ty: Ty::Bool,
                regions: Regions::stat(),
                span,
            },
            (Ty::View { .. }, ConstValue::Str(s)) => Expr {
                kind: ExprKind::Str(s.clone()),
                ty: Ty::view_u8(false),
                regions: Regions::stat(),
                span,
            },
            (Ty::Error, _) => Expr::error(span),
            _ => {
                let name = def.name.clone();
                self.error(
                    ctx.module,
                    "E0200",
                    format!("constant `{name}` cannot be used here"),
                    span,
                );
                Expr::error(span)
            }
        }
    }

    /// `a.b.c` written as an expression, if every segment is a plain name.
    fn expr_path(e: &oli_ast::Expr) -> Option<Vec<oli_ast::Ident>> {
        match &e.kind {
            oli_ast::ExprKind::Name(id) => Some(vec![id.clone()]),
            oli_ast::ExprKind::Field(base, name) => {
                let mut v = Self::expr_path(base)?;
                v.push(name.clone());
                Some(v)
            }
            _ => None,
        }
    }

    /// Resolves a dotted expression to a module-level symbol when its head is
    /// not a local: `os.WRITE`, `Header`, `core.TrapKind`.
    pub(crate) fn expr_symbol(&mut self, ctx: &Ctx, e: &oli_ast::Expr) -> Option<Symbol> {
        let segs = Self::expr_path(e)?;
        let first = segs.first()?;
        if ctx.lookup_local(&first.name).is_some() {
            return None;
        }
        let mut sym = match self.lookup_module_symbol(ctx.module, &first.name) {
            Some(s) => s,
            None => {
                // An intrinsic namespace with an imported module of the same name.
                let ns = Namespace::from_name(&first.name)?;
                let target = self.alias_target(ctx.module, ns.as_str())?;
                Symbol::Module(target)
            }
        };
        for seg in segs.iter().skip(1) {
            match sym {
                Symbol::Module(target) => {
                    sym = self.module_member(target, &seg.name, ctx.module, seg.span)?
                }
                _ => return None,
            }
        }
        Some(sym)
    }

    fn check_field_value(
        &mut self,
        ctx: &mut Ctx,
        base: &oli_ast::Expr,
        name: &oli_ast::Ident,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        // Type members: `T.size`, `T.align`, `Choice.variant`.
        if let oli_ast::ExprKind::TypeRef(t) = &base.kind {
            let ty = self.resolve_type(ctx.module, t);
            return self.type_member(ctx, &ty, name, span);
        }
        let head_is_local = matches!(&base.kind, oli_ast::ExprKind::Name(id) if ctx.lookup_local(&id.name).is_some());
        if !head_is_local {
            if let Some(path) = Self::expr_path(base) {
                let head = path.first().map(|i| i.name.clone()).unwrap_or_default();
                let known = self.lookup_module_symbol(ctx.module, &head).is_some()
                    || Namespace::from_name(&head).is_some();
                if known {
                    let Some(sym) = self.expr_symbol(ctx, base) else {
                        if let Some(ns) = Namespace::from_name(&head) {
                            if path.len() == 1 {
                                let msg =
                                    format!("`{}` has no member `{}`", ns.as_str(), name.name);
                                let mut d = Diagnostic::error("E0105", msg, name.span);
                                if self.alias_target(ctx.module, ns.as_str()).is_none() {
                                    d = d.with_note(format!("intrinsics are called (`{}.name(...)`); constants need an `import` of the matching module", ns.as_str()));
                                }
                                self.report(ctx.module, d);
                            }
                        }
                        return Expr::error(span);
                    };
                    match sym {
                        Symbol::Module(m) => {
                            return match self.module_member(m, &name.name, ctx.module, name.span) {
                                Some(member) => self.symbol_value(ctx, member, span),
                                None => Expr::error(span),
                            }
                        }
                        Symbol::Layout(id) => {
                            return self.type_member(ctx, &Ty::Layout(id), name, span)
                        }
                        Symbol::Choice(id) => {
                            return self.choice_member(ctx, id, name, expected, span)
                        }
                        Symbol::Const(_) | Symbol::Static(_) | Symbol::Proc(_) => {
                            // A value reached by name: fall through to value members below.
                        }
                    }
                }
            }
        }
        // Members of values.
        let v = self.check_expr(ctx, base, None);
        let v = self.require_value(ctx, v);
        match v.ty.clone() {
            Ty::View { elem, .. } => match name.name.as_str() {
                "len" => Expr {
                    kind: ExprKind::ViewLen(Box::new(v)),
                    ty: Ty::uword(),
                    regions: Regions::stat(),
                    span,
                },
                "addr" => Expr {
                    kind: ExprKind::ViewAddr(Box::new(v)),
                    ty: Ty::Addr(elem),
                    regions: Regions::stat(),
                    span,
                },
                other => {
                    self.error(
                        ctx.module,
                        "E0105",
                        format!("views have `len` and `addr`, not `{other}`"),
                        name.span,
                    );
                    Expr::error(span)
                }
            },
            Ty::Layout(id) => {
                self.layout_ready(id);
                match self.prog.layouts[id]
                    .fields
                    .iter()
                    .position(|f| f.name == name.name)
                {
                    Some(idx) => {
                        let ty = value_ty(&self.prog.layouts[id].fields[idx].ty);
                        let regions = if self.type_carries_region(&ty) {
                            v.regions.clone()
                        } else {
                            Regions::stat()
                        };
                        Expr {
                            kind: ExprKind::ValueField(Box::new(v), idx),
                            ty,
                            regions,
                            span,
                        }
                    }
                    None => {
                        let lname = self.prog.layouts[id].name.clone();
                        self.error(
                            ctx.module,
                            "E0105",
                            format!("layout `{lname}` has no field `{}`", name.name),
                            name.span,
                        );
                        Expr::error(span)
                    }
                }
            }
            Ty::Zone => {
                let msg = format!("zone handles are used through calls: `{}.bytes(n)`, `.try_bytes(n)`, `.make(T)`", "z");
                self.error(ctx.module, "E0105", msg, name.span);
                Expr::error(span)
            }
            Ty::Port(_) => {
                self.not_implemented(ctx.module, "port I/O (`p.in()` / `p.out(v)`)", span);
                Expr::error(span)
            }
            Ty::Error => Expr::error(span),
            other => {
                let t = self.type_text(&other);
                self.error(
                    ctx.module,
                    "E0105",
                    format!("`{t}` has no member `{}`", name.name),
                    name.span,
                );
                Expr::error(span)
            }
        }
    }

    fn symbol_value(&mut self, ctx: &mut Ctx, sym: Symbol, span: Span) -> Expr {
        match sym {
            Symbol::Const(c) => {
                self.const_ready(c);
                match self.const_backing.get(c).copied().flatten() {
                    Some(s) => {
                        let ty = self.prog.statics[s].ty.clone();
                        let place = Place {
                            kind: PlaceKind::Static(s),
                            ty,
                            rw: false,
                            regions: Regions::stat(),
                            span,
                        };
                        self.load_place(ctx, place)
                    }
                    None => self.const_value_expr(ctx, c, span),
                }
            }
            Symbol::Static(s) => {
                self.static_ready(s);
                let def = &self.prog.statics[s];
                let (ty, rw) = (def.ty.clone(), !def.readonly);
                let place = Place {
                    kind: PlaceKind::Static(s),
                    ty,
                    rw,
                    regions: Regions::stat(),
                    span,
                };
                self.load_place(ctx, place)
            }
            Symbol::Proc(_) => {
                self.report(
                    ctx.module,
                    Diagnostic::error("E0900", "feature not implemented: procedure values", span)
                        .with_note("call the procedure, or take its code address with `addr name` under `permit memory.raw`"),
                );
                Expr::error(span)
            }
            Symbol::Layout(_) | Symbol::Choice(_) => {
                self.error(ctx.module, "E0200", "a type is not a value", span);
                Expr::error(span)
            }
            Symbol::Module(_) => {
                self.error(ctx.module, "E0200", "a module is not a value", span);
                Expr::error(span)
            }
        }
    }

    fn type_member(&mut self, ctx: &Ctx, ty: &Ty, name: &oli_ast::Ident, span: Span) -> Expr {
        match name.name.as_str() {
            "size" => {
                let (size, _) = self.size_align(ty);
                Expr::int(i128::from(size), IntTy::Uword, span)
            }
            "align" => {
                let (_, align) = self.size_align(ty);
                Expr::int(i128::from(align), IntTy::Uword, span)
            }
            other => {
                let t = self.type_text(ty);
                let mut d = Diagnostic::error(
                    "E0105",
                    format!("type `{t}` has no member `{other}`"),
                    name.span,
                );
                if other == "at" {
                    d = d.with_note(format!("`at` is called: `{t}.at(view)`"));
                }
                self.report(ctx.module, d);
                Expr::error(span)
            }
        }
    }

    fn choice_member(
        &mut self,
        ctx: &Ctx,
        id: crate::ty::ChoiceId,
        name: &oli_ast::Ident,
        _expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        if matches!(name.name.as_str(), "size" | "align") {
            return self.type_member(ctx, &Ty::Choice(id), name, span);
        }
        self.choice_ready(id);
        match self.prog.choices[id]
            .variants
            .iter()
            .position(|v| v.name == name.name)
        {
            Some(index) => {
                if !self.prog.choices[id].variants[index].fields.is_empty() {
                    let cname = self.prog.choices[id].name.clone();
                    let msg = format!(
                        "variant `{}` carries a payload; write `{cname}.{} {{ ... }}`",
                        name.name, name.name
                    );
                    self.error(ctx.module, "E0208", msg, span);
                    return Expr::error(span);
                }
                Expr {
                    kind: ExprKind::VariantLit {
                        choice: id,
                        index,
                        fields: Vec::new(),
                    },
                    ty: Ty::Choice(id),
                    regions: Regions::stat(),
                    span,
                }
            }
            None => {
                let cname = self.prog.choices[id].name.clone();
                self.error(
                    ctx.module,
                    "E0105",
                    format!("`{cname}` has no variant `{}`", name.name),
                    name.span,
                );
                Expr::error(span)
            }
        }
    }

    // -------------------------------------------------------- operators

    fn check_unary(
        &mut self,
        ctx: &mut Ctx,
        op: oli_ast::UnOp,
        x: &oli_ast::Expr,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        match op {
            oli_ast::UnOp::Not => {
                let v = self.check_expr(ctx, x, Some(&Ty::Bool));
                Expr {
                    kind: ExprKind::Unary(UnOp::Not, Box::new(v)),
                    ty: Ty::Bool,
                    regions: Regions::stat(),
                    span,
                }
            }
            oli_ast::UnOp::Neg | oli_ast::UnOp::BitNot => {
                // An untyped operand is folded first (`-128` must not settle `128` into `s8`).
                let hint = if self.is_untyped(ctx, x) {
                    None
                } else {
                    expected.filter(|t| t.is_int())
                };
                let v = self.check_expr(ctx, x, hint);
                let v = self.require_value(ctx, v);
                match (&v.ty, &v.kind) {
                    (Ty::UntypedInt, ExprKind::Int(n)) => {
                        if op == oli_ast::UnOp::Neg {
                            return Expr {
                                kind: ExprKind::Int(-n),
                                ty: Ty::UntypedInt,
                                regions: Regions::stat(),
                                span,
                            };
                        }
                        self.report(
                            ctx.module,
                            Diagnostic::error(
                                "E0201",
                                "`~` needs an operand with a known width",
                                span,
                            )
                            .with_note("write `~u32(x)` or give the surrounding expression a type"),
                        );
                        Expr::error(span)
                    }
                    (Ty::Int(i), _) => {
                        if op == oli_ast::UnOp::Neg && !i.signed() {
                            let msg =
                                format!("cannot negate a value of unsigned type `{}`", i.as_str());
                            self.report(
                                ctx.module,
                                Diagnostic::error("E0200", msg, span)
                                    .with_note("use a signed type, or `0 - x` in `wrap(...)`"),
                            );
                            return Expr::error(span);
                        }
                        let ty = v.ty.clone();
                        let kind = ExprKind::Unary(
                            if op == oli_ast::UnOp::Neg {
                                UnOp::Neg
                            } else {
                                UnOp::BitNot
                            },
                            Box::new(v),
                        );
                        Expr {
                            kind,
                            ty,
                            regions: Regions::stat(),
                            span,
                        }
                    }
                    (Ty::Error, _) => Expr::error(span),
                    (other, _) => {
                        let t = self.type_text(other);
                        self.error(
                            ctx.module,
                            "E0200",
                            format!("unary operator on a value of type `{t}`"),
                            span,
                        );
                        Expr::error(span)
                    }
                }
            }
        }
    }

    fn check_mode_expr(
        &mut self,
        ctx: &mut Ctx,
        mode: oli_ast::Mode,
        x: &oli_ast::Expr,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        let saved = ctx.mode;
        ctx.mode = match mode {
            oli_ast::Mode::Wrap => ArithMode::Wrap,
            oli_ast::Mode::Sat => ArithMode::Sat,
            oli_ast::Mode::Checked => ArithMode::Checked,
            oli_ast::Mode::Bits => ArithMode::Trap,
        };
        let hint = match (mode, expected) {
            (oli_ast::Mode::Checked, Some(Ty::Fallible(ok, _))) => Some((**ok).clone()),
            (_, Some(t)) if t.is_int() => Some(t.clone()),
            _ => None,
        };
        let v = self.check_expr(ctx, x, hint.as_ref());
        ctx.mode = saved;
        let v = self.require_value(ctx, v);
        if matches!(v.ty, Ty::UntypedInt) && mode != oli_ast::Mode::Checked {
            self.report(
                ctx.module,
                Diagnostic::error(
                    "E0201",
                    format!("`{}(...)` needs a typed operand", mode.as_str()),
                    span,
                )
                .with_note(
                    "wrapping and saturation depend on the width; give the expression a type",
                ),
            );
            return Expr::error(span);
        }
        if !v.ty.is_int() && !v.ty.is_error() {
            let t = self.type_text(&v.ty);
            self.error(
                ctx.module,
                "E0200",
                format!(
                    "`{}(...)` needs an integer expression, found `{t}`",
                    mode.as_str()
                ),
                span,
            );
            return Expr::error(span);
        }
        match mode {
            oli_ast::Mode::Checked => {
                let ty = Ty::Fallible(Box::new(v.ty.clone()), Box::new(Ty::Overflow));
                Expr {
                    kind: ExprKind::Checked(Box::new(v)),
                    ty,
                    regions: Regions::stat(),
                    span,
                }
            }
            _ => v,
        }
    }

    /// Unifies two integer operands by lossless widening.
    fn unify_ints(
        &mut self,
        ctx: &mut Ctx,
        l: Expr,
        r: Expr,
        strict: bool,
        span: Span,
    ) -> Option<(Expr, Expr, IntTy)> {
        let (Some(a), Some(b)) = (l.ty.as_int(), r.ty.as_int()) else {
            return None;
        };
        if a.same_repr(b) {
            return Some((l, r, a));
        }
        if !strict {
            if a.widens_to(b) {
                let l = self.coerce(ctx, l, &Ty::Int(b));
                return Some((l, r, b));
            }
            if b.widens_to(a) {
                let r = self.coerce(ctx, r, &Ty::Int(a));
                return Some((l, r, a));
            }
        }
        let msg = if strict {
            format!(
                "bit operators need operands of the same type, found `{}` and `{}`",
                a.as_str(),
                b.as_str()
            )
        } else {
            format!(
                "operands have different types `{}` and `{}` and neither widens to the other",
                a.as_str(),
                b.as_str()
            )
        };
        self.report(
            ctx.module,
            Diagnostic::error("E0200", msg, span).with_note("convert one side explicitly"),
        );
        None
    }

    fn map_binop(op: oli_ast::BinOp) -> BinOp {
        match op {
            oli_ast::BinOp::Add => BinOp::Add,
            oli_ast::BinOp::Sub => BinOp::Sub,
            oli_ast::BinOp::Mul => BinOp::Mul,
            oli_ast::BinOp::Div => BinOp::Div,
            oli_ast::BinOp::Rem => BinOp::Rem,
            oli_ast::BinOp::Eq => BinOp::Eq,
            oli_ast::BinOp::Ne => BinOp::Ne,
            oli_ast::BinOp::Lt => BinOp::Lt,
            oli_ast::BinOp::Le => BinOp::Le,
            oli_ast::BinOp::Gt => BinOp::Gt,
            oli_ast::BinOp::Ge => BinOp::Ge,
            oli_ast::BinOp::BitAnd => BinOp::BitAnd,
            oli_ast::BinOp::BitOr => BinOp::BitOr,
            oli_ast::BinOp::BitXor => BinOp::BitXor,
            oli_ast::BinOp::Shl => BinOp::Shl,
            oli_ast::BinOp::Shr => BinOp::Shr,
            oli_ast::BinOp::And => BinOp::And,
            oli_ast::BinOp::Or => BinOp::Or,
        }
    }

    /// Compile-time arithmetic on two untyped integers (trap semantics: an
    /// overflow of the unbounded value is an error, never a silent wrap).
    fn fold_untyped(&mut self, ctx: &Ctx, op: BinOp, a: i128, b: i128, span: Span) -> Expr {
        let int = |v: Option<i128>| v.map(|v| (ExprKind::Int(v), Ty::UntypedInt));
        let boolean = |v: bool| Some((ExprKind::Bool(v), Ty::Bool));
        let result = match op {
            BinOp::Add => int(a.checked_add(b)),
            BinOp::Sub => int(a.checked_sub(b)),
            BinOp::Mul => int(a.checked_mul(b)),
            BinOp::Div | BinOp::Rem if b == 0 => {
                self.error(
                    ctx.module,
                    "E0213",
                    "division by zero in a constant expression",
                    span,
                );
                return Expr::error(span);
            }
            BinOp::Div => int(a.checked_div(b)),
            BinOp::Rem => int(a.checked_rem(b)),
            BinOp::BitAnd => int(Some(a & b)),
            BinOp::BitOr => int(Some(a | b)),
            BinOp::BitXor => int(Some(a ^ b)),
            BinOp::Shl | BinOp::Shr => {
                if !(0..128).contains(&b) {
                    self.error(
                        ctx.module,
                        "E0212",
                        "shift count of a constant expression is out of range",
                        span,
                    );
                    return Expr::error(span);
                }
                let shift = b as u32;
                if op == BinOp::Shl {
                    int(a.checked_shl(shift).filter(|v| v >> shift == a))
                } else {
                    int(Some(a >> shift))
                }
            }
            BinOp::Eq => boolean(a == b),
            BinOp::Ne => boolean(a != b),
            BinOp::Lt => boolean(a < b),
            BinOp::Le => boolean(a <= b),
            BinOp::Gt => boolean(a > b),
            BinOp::Ge => boolean(a >= b),
            BinOp::And | BinOp::Or => None,
        };
        match result {
            Some((kind, ty)) => Expr {
                kind,
                ty,
                regions: Regions::stat(),
                span,
            },
            None => {
                self.report(
                    ctx.module,
                    Diagnostic::error("E0212", "constant expression overflows", span).with_note(
                        "give the expression a type and use `wrap(...)` if wrapping is intended",
                    ),
                );
                Expr::error(span)
            }
        }
    }

    fn check_binary(
        &mut self,
        ctx: &mut Ctx,
        op: oli_ast::BinOp,
        l: &oli_ast::Expr,
        r: &oli_ast::Expr,
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        let bop = Self::map_binop(op);
        if matches!(bop, BinOp::And | BinOp::Or) {
            let lv = self.check_expr(ctx, l, Some(&Ty::Bool));
            let rv = self.check_expr(ctx, r, Some(&Ty::Bool));
            let kind = ExprKind::Binary {
                op: bop,
                mode: ArithMode::Trap,
                lhs: Box::new(lv),
                rhs: Box::new(rv),
            };
            return Expr {
                kind,
                ty: Ty::Bool,
                regions: Regions::stat(),
                span,
            };
        }
        let is_cmp = matches!(
            bop,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
        );
        let is_shift = matches!(bop, BinOp::Shl | BinOp::Shr);
        let is_bit = matches!(bop, BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor);
        let (lu, ru) = (self.is_untyped(ctx, l), self.is_untyped(ctx, r));
        let hint: Option<Ty> = if is_cmp {
            None
        } else {
            expected.filter(|t| t.is_int()).cloned()
        };
        if lu && ru && hint.is_none() {
            let lv = self.check_expr(ctx, l, None);
            let rv = self.check_expr(ctx, r, None);
            return match (&lv.kind, &rv.kind) {
                (ExprKind::Int(a), ExprKind::Int(b)) => self.fold_untyped(ctx, bop, *a, *b, span),
                _ => Expr::error(span),
            };
        }
        let operand_hint = |t: &Ty| -> Option<Ty> {
            match t {
                Ty::Int(_) => Some(t.clone()),
                Ty::Physaddr | Ty::Addr(_) => Some(Ty::uword()),
                _ => None,
            }
        };
        let uword = Ty::uword();
        let (lv, rv) = if lu && ru {
            let lv = self.check_expr(ctx, l, hint.as_ref());
            let rv = self.check_expr(
                ctx,
                r,
                if is_shift {
                    Some(&uword)
                } else {
                    hint.as_ref()
                },
            );
            (lv, rv)
        } else if lu {
            let rv = self.check_expr(ctx, r, None);
            let h = if is_shift {
                hint.clone()
            } else {
                operand_hint(&rv.ty)
            };
            let lv = self.check_expr(ctx, l, h.as_ref());
            (lv, rv)
        } else {
            let lv = self.check_expr(ctx, l, None);
            let h = if !ru {
                None
            } else if is_shift {
                Some(Ty::uword())
            } else {
                operand_hint(&lv.ty)
            };
            let rv = self.check_expr(ctx, r, h.as_ref());
            (lv, rv)
        };
        let lv = self.require_value(ctx, lv);
        let rv = self.require_value(ctx, rv);
        if lv.ty.is_error() || rv.ty.is_error() {
            return Expr::error(span);
        }
        if matches!(lv.ty, Ty::UntypedInt) || matches!(rv.ty, Ty::UntypedInt) {
            // A literal met a non-integer operand; report once.
            let other = if matches!(lv.ty, Ty::UntypedInt) {
                &rv.ty
            } else {
                &lv.ty
            };
            let t = self.type_text(other);
            self.error(
                ctx.module,
                "E0200",
                format!("integer literal combined with a value of type `{t}`"),
                span,
            );
            return Expr::error(span);
        }
        let mode = ctx.mode;
        let make = |lhs: Expr, rhs: Expr, ty: Ty| Expr {
            kind: ExprKind::Binary {
                op: bop,
                mode,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty,
            regions: Regions::stat(),
            span,
        };
        if is_cmp {
            return match (&lv.ty, &rv.ty) {
                (Ty::Int(_), Ty::Int(_)) => match self.unify_ints(ctx, lv, rv, false, span) {
                    Some((a, b, _)) => make(a, b, Ty::Bool),
                    None => Expr::error(span),
                },
                (Ty::Physaddr, Ty::Physaddr) => make(lv, rv, Ty::Bool),
                (Ty::Addr(a), Ty::Addr(b)) if a.same(b) => make(lv, rv, Ty::Bool),
                (Ty::Bool, Ty::Bool) if matches!(bop, BinOp::Eq | BinOp::Ne) => {
                    make(lv, rv, Ty::Bool)
                }
                (a, b) => {
                    let (a, b) = (self.type_text(a), self.type_text(b));
                    self.error(
                        ctx.module,
                        "E0200",
                        format!("cannot compare `{a}` with `{b}`"),
                        span,
                    );
                    Expr::error(span)
                }
            };
        }
        if is_shift {
            return match (&lv.ty, &rv.ty) {
                (Ty::Int(i), Ty::Int(c)) if !c.signed() => {
                    let ty = Ty::Int(*i);
                    let rv = self.coerce(ctx, rv, &Ty::uword());
                    make(lv, rv, ty)
                }
                (Ty::Int(_), other) => {
                    let t = self.type_text(other);
                    self.error(
                        ctx.module,
                        "E0200",
                        format!("a shift count must be unsigned, found `{t}`"),
                        rv.span,
                    );
                    Expr::error(span)
                }
                (other, _) => {
                    let t = self.type_text(other);
                    self.error(
                        ctx.module,
                        "E0200",
                        format!("cannot shift a value of type `{t}`"),
                        lv.span,
                    );
                    Expr::error(span)
                }
            };
        }
        if is_bit {
            return match self.unify_ints(ctx, lv, rv, true, span) {
                Some((a, b, t)) => make(a, b, Ty::Int(t)),
                None => Expr::error(span),
            };
        }
        // Arithmetic.
        match (&lv.ty, &rv.ty) {
            (Ty::Int(_), Ty::Int(_)) => match self.unify_ints(ctx, lv, rv, false, span) {
                Some((a, b, t)) => make(a, b, Ty::Int(t)),
                None => Expr::error(span),
            },
            (Ty::Physaddr, Ty::Int(_)) | (Ty::Addr(_), Ty::Int(_))
                if matches!(bop, BinOp::Add | BinOp::Sub) =>
            {
                let ty = lv.ty.clone();
                let rv = self.coerce(ctx, rv, &Ty::uword());
                make(lv, rv, ty)
            }
            (Ty::Int(_), Ty::Physaddr) | (Ty::Int(_), Ty::Addr(_)) if bop == BinOp::Add => {
                let ty = rv.ty.clone();
                let lv = self.coerce(ctx, lv, &Ty::uword());
                make(lv, rv, ty)
            }
            (Ty::Physaddr, Ty::Addr(_)) | (Ty::Addr(_), Ty::Physaddr) => {
                self.report(
                    ctx.module,
                    Diagnostic::error(
                        "E0203",
                        "mixed address spaces: a physical and a virtual address",
                        span,
                    )
                    .with_note("convert explicitly; `physaddr` and `addr T` never mix by accident"),
                );
                Expr::error(span)
            }
            (a, b) => {
                let (a, b) = (self.type_text(a), self.type_text(b));
                let msg = format!("operator `{}` on `{a}` and `{b}`", op.as_str());
                self.error(ctx.module, "E0200", msg, span);
                Expr::error(span)
            }
        }
    }

    // ------------------------------------------------- addresses and refs

    fn check_addr_of(&mut self, ctx: &mut Ctx, x: &oli_ast::Expr, span: Span) -> Expr {
        // `addr proc` — the code address.
        if let oli_ast::ExprKind::Name(id) = &x.kind {
            if ctx.lookup_local(&id.name).is_none() {
                if let Some(Symbol::Proc(p)) = self.lookup_module_symbol(ctx.module, &id.name) {
                    self.require_permit(
                        ctx,
                        Capability::MemoryRaw,
                        "taking the code address of a procedure",
                        span,
                    );
                    return Expr {
                        kind: ExprKind::ProcAddr(p),
                        ty: Ty::Addr(Box::new(Ty::u8())),
                        regions: Regions::stat(),
                        span,
                    };
                }
            }
        }
        match self.check_place_opt(ctx, x) {
            Err(()) => Expr::error(span),
            Ok(Some(place)) => {
                let elem = match &place.ty {
                    Ty::Array(_, e) => (**e).clone(),
                    t => value_ty(t),
                };
                Expr {
                    kind: ExprKind::AddrOf(place),
                    ty: Ty::Addr(Box::new(elem)),
                    regions: Regions::stat(),
                    span,
                }
            }
            Ok(None) => {
                let v = self.check_expr(ctx, x, None);
                let v = self.require_value(ctx, v);
                match v.ty.clone() {
                    Ty::View { elem, .. } => Expr {
                        kind: ExprKind::ViewAddr(Box::new(v)),
                        ty: Ty::Addr(elem),
                        regions: Regions::stat(),
                        span,
                    },
                    Ty::Error => Expr::error(span),
                    other => {
                        let t = self.type_text(&other);
                        self.report(
                            ctx.module,
                            Diagnostic::error("E0200", format!("cannot take the address of a value of type `{t}`"), span)
                                .with_note("`addr` applies to places (declared with `name : T`), views and procedures; bindings live in registers"),
                        );
                        Expr::error(span)
                    }
                }
            }
        }
    }

    fn check_ref_of(&mut self, ctx: &mut Ctx, rw: bool, x: &oli_ast::Expr, span: Span) -> Expr {
        match self.check_place_opt(ctx, x) {
            Err(()) => Expr::error(span),
            Ok(Some(place)) => {
                if rw && !place.rw {
                    self.error(ctx.module, "E0111", "`rw ref` needs writable memory", span);
                    return Expr::error(span);
                }
                let ty = Ty::Ref {
                    mmio: false,
                    rw,
                    elem: Box::new(place.ty.clone()),
                };
                let regions = place.regions.clone();
                Expr {
                    kind: ExprKind::RefOf(place),
                    ty,
                    regions,
                    span,
                }
            }
            Ok(None) => {
                self.report(
                    ctx.module,
                    Diagnostic::error("E0200", "`ref` needs a place", span)
                        .with_note("bindings (`:=`) have no address; declare a place with `name : T` to reference it"),
                );
                Expr::error(span)
            }
        }
    }

    // ------------------------------------------------------- conversions

    fn check_convert(
        &mut self,
        ctx: &mut Ctx,
        ty: &oli_ast::Type,
        mode: Option<oli_ast::Mode>,
        x: &oli_ast::Expr,
        span: Span,
    ) -> Expr {
        let to = self.resolve_type(ctx.module, ty);
        if to.is_error() {
            let _ = self.check_expr(ctx, x, None);
            return Expr::error(span);
        }
        if let Ty::Port(_) = to {
            let _ = self.check_expr(ctx, x, None);
            self.not_implemented(ctx.module, "port values", span);
            return Expr::error(span);
        }
        let v = self.check_expr(ctx, x, None);
        let v = self.require_value(ctx, v);
        if v.ty.is_error() {
            return Expr::error(span);
        }
        let conv = |kind: ConvKind, expr: Expr, ty: Ty| Expr {
            kind: ExprKind::Convert {
                kind,
                expr: Box::new(expr),
            },
            ty,
            regions: Regions::stat(),
            span,
        };
        match (mode, &to, &v.ty) {
            (None, _, _) => self.check_plain_conversion(ctx, v, to, span),
            (Some(oli_ast::Mode::Wrap), Ty::Int(t), Ty::UntypedInt) => {
                let ExprKind::Int(n) = v.kind else {
                    return Expr::error(span);
                };
                Expr::int(t.wrap(n), *t, span)
            }
            (Some(oli_ast::Mode::Sat), Ty::Int(t), Ty::UntypedInt) => {
                let ExprKind::Int(n) = v.kind else {
                    return Expr::error(span);
                };
                Expr::int(t.saturate(n), *t, span)
            }
            (Some(oli_ast::Mode::Wrap), Ty::Int(_), Ty::Int(_)) => {
                conv(ConvKind::Wrap, v, to.clone())
            }
            (Some(oli_ast::Mode::Sat), Ty::Int(_), Ty::Int(_)) => {
                conv(ConvKind::Sat, v, to.clone())
            }
            (Some(oli_ast::Mode::Checked), Ty::Int(t), Ty::UntypedInt) => {
                let v = self.settle(ctx, v, *t);
                conv(
                    ConvKind::Checked,
                    v,
                    Ty::Fallible(Box::new(to.clone()), Box::new(Ty::Overflow)),
                )
            }
            (Some(oli_ast::Mode::Checked), Ty::Int(_), Ty::Int(_)) => conv(
                ConvKind::Checked,
                v,
                Ty::Fallible(Box::new(to.clone()), Box::new(Ty::Overflow)),
            ),
            (Some(oli_ast::Mode::Bits), Ty::Int(t), Ty::Int(f)) if t.bits() == f.bits() => {
                conv(ConvKind::Bits, v, to.clone())
            }
            (Some(oli_ast::Mode::Bits), Ty::Int(t), Ty::Int(f)) => {
                let msg = format!(
                    "`bits` reinterprets equal widths only: `{}` is {} bits, `{}` is {}",
                    f.as_str(),
                    f.bits(),
                    t.as_str(),
                    t.bits()
                );
                self.error(ctx.module, "E0202", msg, span);
                Expr::error(span)
            }
            (Some(m), _, from) => {
                let (f, t) = (self.type_text(from), self.type_text(&to));
                self.error(
                    ctx.module,
                    "E0200",
                    format!("`{t}.{}` cannot convert from `{f}`", m.as_str()),
                    span,
                );
                Expr::error(span)
            }
        }
    }

    /// `T(x)`: lossless conversions only (`spec` §2, explicit conversions).
    fn check_plain_conversion(&mut self, ctx: &mut Ctx, v: Expr, to: Ty, span: Span) -> Expr {
        let conv = |kind: ConvKind, expr: Expr, ty: Ty| Expr {
            kind: ExprKind::Convert {
                kind,
                expr: Box::new(expr),
            },
            ty,
            regions: Regions::stat(),
            span,
        };
        match (&to, &v.ty) {
            (Ty::Int(t), Ty::UntypedInt) => {
                let mut r = self.settle(ctx, v, *t);
                r.span = span;
                r
            }
            (Ty::Int(t), Ty::Int(f)) => {
                let lossless =
                    f.widens_to(*t) || (!f.signed() && t.signed() && t.bits() > f.bits());
                if f.same_repr(*t) {
                    Expr {
                        ty: to.clone(),
                        ..v
                    }
                } else if lossless {
                    conv(ConvKind::Widen, v, to.clone())
                } else {
                    let msg = format!("`{}` to `{}` may lose information", f.as_str(), t.as_str());
                    self.report(
                        ctx.module,
                        Diagnostic::error("E0202", msg, span)
                            .with_note(format!("say what should happen: `{t}.wrap(x)`, `{t}.sat(x)` or `{t}.checked(x)`")),
                    );
                    Expr::error(span)
                }
            }
            (Ty::Int(IntTy::Uword | IntTy::U64), Ty::Addr(_)) => {
                self.require_permit(
                    ctx,
                    Capability::MemoryRaw,
                    "converting a raw address to an integer",
                    span,
                );
                conv(ConvKind::AddrToInt, v, to.clone())
            }
            (Ty::Int(IntTy::Uword | IntTy::U64), Ty::Physaddr) => {
                conv(ConvKind::PhysToInt, v, to.clone())
            }
            (Ty::Physaddr, Ty::UntypedInt) => {
                let v = self.settle(ctx, v, IntTy::Uword);
                conv(ConvKind::IntToPhys, v, Ty::Physaddr)
            }
            (Ty::Physaddr, Ty::Int(f)) if !f.signed() => {
                let v = self.coerce(ctx, v, &Ty::uword());
                conv(ConvKind::IntToPhys, v, Ty::Physaddr)
            }
            (Ty::Addr(_), Ty::UntypedInt) => {
                self.require_permit(
                    ctx,
                    Capability::MemoryRaw,
                    "converting an integer to a raw address",
                    span,
                );
                let v = self.settle(ctx, v, IntTy::Uword);
                conv(ConvKind::IntToAddr, v, to.clone())
            }
            (Ty::Addr(_), Ty::Int(f)) if !f.signed() => {
                self.require_permit(
                    ctx,
                    Capability::MemoryRaw,
                    "converting an integer to a raw address",
                    span,
                );
                let v = self.coerce(ctx, v, &Ty::uword());
                conv(ConvKind::IntToAddr, v, to.clone())
            }
            (Ty::Addr(_), Ty::Addr(_)) => conv(ConvKind::Reinterpret, v, to.clone()),
            (Ty::Physaddr, Ty::Physaddr) => v,
            (t, f) => {
                let (f, t) = (self.type_text(f), self.type_text(t));
                let mut d =
                    Diagnostic::error("E0200", format!("cannot convert `{f}` to `{t}`"), span);
                if matches!(
                    (&to, &v.ty),
                    (Ty::Physaddr, Ty::Addr(_)) | (Ty::Addr(_), Ty::Physaddr)
                ) {
                    d = d.with_note("physical and virtual addresses convert only through an explicit procedure (see docs/KERNEL_PROGRAMMING.md)");
                }
                self.report(ctx.module, d);
                Expr::error(span)
            }
        }
    }

    // ---------------------------------------------------------- fallback

    fn check_fallback(
        &mut self,
        ctx: &mut Ctx,
        x: &oli_ast::Expr,
        handler: &oli_ast::Handler,
        span: Span,
    ) -> Expr {
        let v = self.check_expr(ctx, x, None);
        let v = self.require_value(ctx, v);
        let (ok, err) = match &v.ty {
            Ty::Fallible(ok, err) => ((**ok).clone(), (**err).clone()),
            Ty::Error => return Expr::error(span),
            other => {
                let t = self.type_text(other);
                self.report(
                    ctx.module,
                    Diagnostic::error(
                        "E0200",
                        format!("`else` needs a fallible value, found `{t}`"),
                        v.span,
                    )
                    .with_note("only a `T or E` value has a failure to handle"),
                );
                return Expr::error(span);
            }
        };
        let mut regions = v.regions.clone();
        let h = match handler {
            oli_ast::Handler::Fail => {
                match ctx.fail_type() {
                    Some(mine) if mine.same(&err) => {}
                    Some(mine) => {
                        let (have, want) = (self.type_text(&err), self.type_text(&mine));
                        let mut d = Diagnostic::error("E0200", format!("`else fail` would propagate `{have}` from a procedure that fails with `{want}`"), span);
                        if matches!(err, Ty::Overflow) {
                            d = d.with_note("the failure of `checked(...)` cannot be named in V0; handle it with `else default`, `else ret ...` or `case`");
                        }
                        self.report(ctx.module, d);
                    }
                    None => {
                        self.report(
                            ctx.module,
                            Diagnostic::error(
                                "E0200",
                                "`else fail` in a procedure that cannot fail",
                                span,
                            )
                            .with_note("declare the result as `-> T or E`"),
                        );
                    }
                }
                Fallback::Fail
            }
            oli_ast::Handler::Ret(value) => {
                let expected = ctx.ok_result();
                let out = match (value, expected) {
                    (None, None) => None,
                    (None, Some(t)) => {
                        let t = self.type_text(&t);
                        self.error(
                            ctx.module,
                            "E0200",
                            format!("`else ret` needs a value of type `{t}`"),
                            span,
                        );
                        None
                    }
                    (Some(e), None) => {
                        let e = self.check_expr(ctx, e, None);
                        self.error(
                            ctx.module,
                            "E0200",
                            "this procedure has no result; `else ret` takes no value",
                            e.span,
                        );
                        None
                    }
                    (Some(e), Some(t)) => {
                        let e = self.check_expr(ctx, e, Some(&t));
                        let e = self.require_value(ctx, e);
                        self.check_escape_on_return(ctx, &e);
                        Some(Box::new(e))
                    }
                };
                Fallback::Ret(out)
            }
            oli_ast::Handler::Default(d) => {
                let d = self.check_expr(ctx, d, Some(&ok));
                let d = self.require_value(ctx, d);
                regions = regions.join(&d.regions);
                Fallback::Default(Box::new(d))
            }
        };
        let regions = if self.type_carries_region(&ok) {
            regions
        } else {
            Regions::stat()
        };
        Expr {
            kind: ExprKind::Fallback {
                expr: Box::new(v),
                handler: h,
            },
            ty: ok,
            regions,
            span,
        }
    }

    // ---------------------------------------------------------- literals

    fn check_struct_lit(
        &mut self,
        ctx: &mut Ctx,
        path: &oli_ast::Expr,
        fields: &[oli_ast::FieldInit],
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        // Which aggregate is being built?
        let target = match self.expr_symbol(ctx, path) {
            Some(Symbol::Layout(id)) => Some((Ty::Layout(id), None)),
            Some(Symbol::Choice(_)) => {
                self.error(
                    ctx.module,
                    "E0200",
                    "name a variant of the choice, e.g. `Choice.variant { ... }`",
                    path.span,
                );
                return Expr::error(span);
            }
            Some(_) => {
                self.error(
                    ctx.module,
                    "E0200",
                    "only layouts and choice variants take `{ ... }`",
                    path.span,
                );
                return Expr::error(span);
            }
            None => {
                // `Choice.variant { }` or a bare `variant { }` against the expected choice.
                let segs = Self::expr_path(path).unwrap_or_default();
                let (choice, vname) = match segs.as_slice() {
                    [v] => match expected {
                        Some(Ty::Choice(id)) => (Some(*id), v.clone()),
                        Some(Ty::Fallible(_, e)) => match &**e {
                            Ty::Choice(id) => (Some(*id), v.clone()),
                            _ => (None, v.clone()),
                        },
                        _ => (None, v.clone()),
                    },
                    [.., v] => {
                        match Self::path_prefix(path).and_then(|e| self.expr_symbol(ctx, &e)) {
                            Some(Symbol::Choice(id)) => (Some(id), v.clone()),
                            _ => (None, v.clone()),
                        }
                    }
                    [] => return Expr::error(span),
                };
                match choice {
                    Some(id) => {
                        self.choice_ready(id);
                        match self.prog.choices[id]
                            .variants
                            .iter()
                            .position(|x| x.name == vname.name)
                        {
                            Some(index) => Some((Ty::Choice(id), Some(index))),
                            None => {
                                let cname = self.prog.choices[id].name.clone();
                                self.error(
                                    ctx.module,
                                    "E0100",
                                    format!("`{cname}` has no variant `{}`", vname.name),
                                    vname.span,
                                );
                                None
                            }
                        }
                    }
                    None => {
                        self.report(
                            ctx.module,
                            Diagnostic::error("E0100", format!("unknown layout or variant `{}`", vname.name), vname.span)
                                .with_note("a bare variant name needs an expected choice type (e.g. in `fail` or `ret`); otherwise write `Choice.variant { ... }`"),
                        );
                        None
                    }
                }
            }
        };
        let Some((ty, variant)) = target else {
            for f in fields {
                let _ = self.check_expr(ctx, &f.value, None);
            }
            return Expr::error(span);
        };
        let defs: Vec<FieldDef> = match (&ty, variant) {
            (Ty::Layout(id), _) => {
                self.layout_ready(*id);
                self.prog.layouts[*id].fields.clone()
            }
            (Ty::Choice(id), Some(vi)) => self.prog.choices[*id].variants[vi].fields.clone(),
            _ => Vec::new(),
        };
        let mut values: Vec<Option<Expr>> = (0..defs.len()).map(|_| None).collect();
        let mut regions = Regions::stat();
        for f in fields {
            match defs.iter().position(|d| d.name == f.name.name) {
                Some(i) => {
                    let want = value_ty(&defs[i].ty);
                    let v = self.check_expr(ctx, &f.value, Some(&want));
                    let v = self.require_value(ctx, v);
                    regions = regions.join(&v.regions);
                    if values[i].is_some() {
                        self.error(
                            ctx.module,
                            "E0208",
                            format!("field `{}` is given twice", f.name.name),
                            f.name.span,
                        );
                    }
                    values[i] = Some(v);
                }
                None => {
                    let tname = self.type_text(&ty);
                    self.error(
                        ctx.module,
                        "E0208",
                        format!("`{tname}` has no field `{}`", f.name.name),
                        f.name.span,
                    );
                    let _ = self.check_expr(ctx, &f.value, None);
                }
            }
        }
        let missing: Vec<&str> = defs
            .iter()
            .zip(&values)
            .filter(|(_, v)| v.is_none())
            .map(|(d, _)| d.name.as_str())
            .collect();
        if !missing.is_empty() {
            let tname = self.type_text(&ty);
            let list = missing
                .iter()
                .map(|m| format!("`{m}`"))
                .collect::<Vec<_>>()
                .join(", ");
            self.report(
                ctx.module,
                Diagnostic::error(
                    "E0208",
                    format!("missing field(s) {list} in the `{tname}` literal"),
                    span,
                )
                .with_note("every field must be given; Oli-- has no implicit defaults"),
            );
            return Expr::error(span);
        }
        let values: Vec<Expr> = values.into_iter().flatten().collect();
        let kind = match (&ty, variant) {
            (Ty::Layout(id), _) => ExprKind::LayoutLit {
                layout: *id,
                fields: values,
            },
            (Ty::Choice(id), Some(index)) => ExprKind::VariantLit {
                choice: *id,
                index,
                fields: values,
            },
            _ => ExprKind::Error,
        };
        Expr {
            kind,
            ty,
            regions,
            span,
        }
    }

    /// `a.b.c` without its last segment, as an expression.
    fn path_prefix(e: &oli_ast::Expr) -> Option<oli_ast::Expr> {
        match &e.kind {
            oli_ast::ExprKind::Field(base, _) => Some((**base).clone()),
            _ => None,
        }
    }

    fn check_array_lit(
        &mut self,
        ctx: &mut Ctx,
        items: &[oli_ast::Expr],
        expected: Option<&Ty>,
        span: Span,
    ) -> Expr {
        let Some(Ty::Array(n, elem)) = expected else {
            for i in items {
                let _ = self.check_expr(ctx, i, None);
            }
            self.report(
                ctx.module,
                Diagnostic::error("E0200", "an array literal needs a known array type", span)
                    .with_note("use it to initialize a place or static of type `[N]T`"),
            );
            return Expr::error(span);
        };
        if items.len() as u64 != *n {
            let msg = format!(
                "array literal has {} element(s) but the type `[{n}]{}` needs {n}",
                items.len(),
                self.type_text(elem)
            );
            self.error(ctx.module, "E0209", msg, span);
        }
        let want = value_ty(elem);
        let mut regions = Regions::stat();
        let values: Vec<Expr> = items
            .iter()
            .map(|i| {
                let v = self.check_expr(ctx, i, Some(&want));
                let v = self.require_value(ctx, v);
                regions = regions.join(&v.regions);
                v
            })
            .collect();
        let ty = Ty::Array(*n, elem.clone());
        Expr {
            kind: ExprKind::ArrayLit(values),
            ty,
            regions,
            span,
        }
    }

    /// Two integer expressions that must agree (range bounds): a typed side
    /// types an untyped one; two untyped sides take `default`.
    pub(crate) fn check_pair(
        &mut self,
        ctx: &mut Ctx,
        a: &oli_ast::Expr,
        b: &oli_ast::Expr,
        default: Option<&Ty>,
    ) -> (Expr, Expr) {
        let (au, bu) = (self.is_untyped(ctx, a), self.is_untyped(ctx, b));
        let (av, bv) = if au && bu {
            (
                self.check_expr(ctx, a, default),
                self.check_expr(ctx, b, default),
            )
        } else if au {
            let bv = self.check_expr(ctx, b, None);
            let hint = bv.ty.clone();
            (self.check_expr(ctx, a, Some(&hint)), bv)
        } else if bu {
            let av = self.check_expr(ctx, a, None);
            let hint = av.ty.clone();
            let bv = self.check_expr(ctx, b, Some(&hint));
            (av, bv)
        } else {
            (self.check_expr(ctx, a, None), self.check_expr(ctx, b, None))
        };
        let av = self.require_value(ctx, av);
        let bv = self.require_value(ctx, bv);
        match (&av.ty, &bv.ty) {
            (Ty::Int(_), Ty::Int(_)) => {
                let span = av.span.to(bv.span);
                match self.unify_ints(ctx, av, bv, false, span) {
                    Some((x, y, _)) => (x, y),
                    None => (Expr::error(span), Expr::error(span)),
                }
            }
            (Ty::Error, _) | (_, Ty::Error) => (av, bv),
            (x, y) => {
                let (x, y) = (self.type_text(x), self.type_text(y));
                let span = av.span.to(bv.span);
                self.error(
                    ctx.module,
                    "E0200",
                    format!("range bounds must be integers, found `{x}` and `{y}`"),
                    span,
                );
                (Expr::error(av.span), Expr::error(bv.span))
            }
        }
    }

    // ------------------------------------------------------------- calls

    fn check_call(
        &mut self,
        ctx: &mut Ctx,
        callee: &oli_ast::Expr,
        args: &[oli_ast::Arg],
        span: Span,
    ) -> Expr {
        // `wrap(...)` and friends do not reach into nested calls (§4).
        let saved = ctx.mode;
        ctx.mode = ArithMode::Trap;
        let r = self.check_call_inner(ctx, callee, args, span);
        ctx.mode = saved;
        r
    }

    fn not_callable(&mut self, ctx: &Ctx, what: &str, span: Span) -> Expr {
        self.error(ctx.module, "E0210", format!("{what} is not callable"), span);
        Expr::error(span)
    }

    fn check_call_inner(
        &mut self,
        ctx: &mut Ctx,
        callee: &oli_ast::Expr,
        args: &[oli_ast::Arg],
        span: Span,
    ) -> Expr {
        match &callee.kind {
            oli_ast::ExprKind::Name(id) => {
                if ctx.lookup_local(&id.name).is_some() {
                    return self.not_callable(
                        ctx,
                        &format!("the local `{}`", id.name),
                        callee.span,
                    );
                }
                match self.lookup_module_symbol(ctx.module, &id.name) {
                    Some(Symbol::Proc(p)) => self.check_proc_call(ctx, p, args, span),
                    Some(Symbol::Layout(_) | Symbol::Choice(_)) => {
                        self.report(
                            ctx.module,
                            Diagnostic::error("E0210", format!("`{}` is a type", id.name), callee.span)
                                .with_note("reinterpret memory with `T.at(view)`; build a value with `T { field: value }`"),
                        );
                        Expr::error(span)
                    }
                    Some(_) => self.not_callable(ctx, &format!("`{}`", id.name), callee.span),
                    None => {
                        if Namespace::from_name(&id.name).is_some() {
                            self.not_callable(
                                ctx,
                                &format!("the namespace `{}`", id.name),
                                callee.span,
                            )
                        } else {
                            self.unknown_name(ctx.module, &id.name, id.span);
                            Expr::error(span)
                        }
                    }
                }
            }
            oli_ast::ExprKind::Field(base, name) => {
                self.check_method_call(ctx, base, name, args, span)
            }
            _ => self.not_callable(ctx, "this expression", callee.span),
        }
    }

    fn check_method_call(
        &mut self,
        ctx: &mut Ctx,
        base: &oli_ast::Expr,
        name: &oli_ast::Ident,
        args: &[oli_ast::Arg],
        span: Span,
    ) -> Expr {
        let head_is_local = matches!(&base.kind, oli_ast::ExprKind::Name(id) if ctx.lookup_local(&id.name).is_some());
        if !head_is_local {
            if let Some(path) = Self::expr_path(base) {
                let head = path.first().map(|i| i.name.clone()).unwrap_or_default();
                // Intrinsic namespaces (possibly merged with an imported module).
                if path.len() == 1 {
                    if let Some(ns) = Namespace::from_name(&head) {
                        if let Some(sig) = intrinsic_sig(ns, &name.name) {
                            return self.check_intrinsic_call(ctx, sig, args, span);
                        }
                        if matches!((ns, name.name.as_str()), (Namespace::Mem, "mmio")) {
                            self.not_implemented(
                                ctx.module,
                                "`mem.mmio` (creating device views)",
                                span,
                            );
                            return Expr::error(span);
                        }
                        if ns == Namespace::Cpu {
                            self.not_implemented(
                                ctx.module,
                                &format!("the `cpu.{}` intrinsic (V1)", name.name),
                                span,
                            );
                            return Expr::error(span);
                        }
                        if let Some(m) = self.alias_target(ctx.module, ns.as_str()) {
                            return match self.module_member(m, &name.name, ctx.module, name.span) {
                                Some(Symbol::Proc(p)) => self.check_proc_call(ctx, p, args, span),
                                Some(_) => self.not_callable(
                                    ctx,
                                    &format!("`{}.{}`", head, name.name),
                                    span,
                                ),
                                None => Expr::error(span),
                            };
                        }
                        let msg = format!("`{}` has no intrinsic `{}`", ns.as_str(), name.name);
                        self.error(ctx.module, "E0105", msg, name.span);
                        return Expr::error(span);
                    }
                }
                if self.lookup_module_symbol(ctx.module, &head).is_some() || head == "core" {
                    return match self.expr_symbol(ctx, base) {
                        Some(Symbol::Module(m)) => match self
                            .module_member(m, &name.name, ctx.module, name.span)
                        {
                            Some(Symbol::Proc(p)) => self.check_proc_call(ctx, p, args, span),
                            Some(_) => self.not_callable(ctx, &format!("`{}`", name.name), span),
                            None => Expr::error(span),
                        },
                        Some(Symbol::Layout(id)) if name.name == "at" => {
                            self.check_layout_at(ctx, id, args, span)
                        }
                        Some(Symbol::Layout(_) | Symbol::Choice(_)) => {
                            let msg = format!("types have no method `{}`", name.name);
                            self.report(
                                ctx.module,
                                Diagnostic::error("E0105", msg, name.span).with_note(
                                    "layouts offer `T.at(view)`, `T.size` and `T.align`",
                                ),
                            );
                            Expr::error(span)
                        }
                        Some(_) => self.not_callable(ctx, &format!("`{}`", name.name), span),
                        None => Expr::error(span),
                    };
                }
            }
        }
        // Methods of values: zone handles (and, later, ports).
        let v = self.check_expr(ctx, base, None);
        let v = self.require_value(ctx, v);
        match v.ty.clone() {
            Ty::Zone => self.check_zone_method(ctx, v, name, args, span),
            Ty::Port(_) => {
                self.not_implemented(ctx.module, "port I/O (`p.in()` / `p.out(v)`)", span);
                Expr::error(span)
            }
            Ty::Error => Expr::error(span),
            other => {
                let t = self.type_text(&other);
                self.error(
                    ctx.module,
                    "E0105",
                    format!("`{t}` has no method `{}`", name.name),
                    name.span,
                );
                Expr::error(span)
            }
        }
    }

    /// Matches positional or named arguments to `params`, checking each
    /// against its parameter type. `None` when the shape is wrong.
    fn check_args(
        &mut self,
        ctx: &mut Ctx,
        args: &[oli_ast::Arg],
        params: &[(String, Ty)],
        what: &str,
        span: Span,
    ) -> Option<Vec<Expr>> {
        let named = args.iter().any(|a| a.name.is_some());
        if !named {
            if args.len() != params.len() {
                let msg = format!(
                    "{what} takes {} argument(s), {} given",
                    params.len(),
                    args.len()
                );
                let list = params
                    .iter()
                    .map(|(n, t)| format!("{n} : {}", self.type_text(t)))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.report(
                    ctx.module,
                    Diagnostic::error("E0205", msg, span)
                        .with_note(format!("parameters: ({list})")),
                );
                for a in args {
                    let _ = self.check_expr(ctx, &a.value, None);
                }
                return None;
            }
            let out = args
                .iter()
                .zip(params)
                .map(|(a, (_, t))| {
                    let v = self.check_expr(ctx, &a.value, Some(t));
                    self.require_value(ctx, v)
                })
                .collect();
            return Some(out);
        }
        let mut slots: Vec<Option<Expr>> = (0..params.len()).map(|_| None).collect();
        let mut ok = true;
        for a in args {
            let Some(n) = &a.name else { continue };
            match params.iter().position(|(p, _)| *p == n.name) {
                Some(i) => {
                    let v = self.check_expr(ctx, &a.value, Some(&params[i].1));
                    let v = self.require_value(ctx, v);
                    if slots[i].is_some() {
                        self.error(
                            ctx.module,
                            "E0206",
                            format!("argument `{}` is given twice", n.name),
                            n.span,
                        );
                        ok = false;
                    }
                    slots[i] = Some(v);
                }
                None => {
                    self.error(
                        ctx.module,
                        "E0206",
                        format!("{what} has no parameter `{}`", n.name),
                        n.span,
                    );
                    let _ = self.check_expr(ctx, &a.value, None);
                    ok = false;
                }
            }
        }
        let missing: Vec<String> = params
            .iter()
            .zip(&slots)
            .filter(|(_, s)| s.is_none())
            .map(|(p, _)| format!("`{}`", p.0))
            .collect();
        if !missing.is_empty() {
            self.error(
                ctx.module,
                "E0206",
                format!("missing argument(s) {}", missing.join(", ")),
                span,
            );
            ok = false;
        }
        if !ok {
            return None;
        }
        Some(slots.into_iter().flatten().collect())
    }

    fn check_proc_call(
        &mut self,
        ctx: &mut Ctx,
        p: ProcId,
        args: &[oli_ast::Arg],
        span: Span,
    ) -> Expr {
        let def = &self.prog.procs[p];
        let name = def.name.clone();
        let params: Vec<(String, Ty)> = def
            .params
            .iter()
            .map(|&i| (def.locals[i].name.clone(), def.locals[i].ty.clone()))
            .collect();
        let result = def.result.clone().unwrap_or(Ty::Unit);
        if def.conv == CallConv::None {
            self.report(
                ctx.module,
                Diagnostic::error(
                    "E0603",
                    format!(
                        "`{name}` is `calls none` and has no calling convention to call it with"
                    ),
                    span,
                )
                .with_note("naked procedures are entry points or targets of `machine` code only"),
            );
        }
        let Some(values) = self.check_args(ctx, args, &params, &format!("`{name}`"), span) else {
            return Expr::error(span);
        };
        // Rule 3: the result may point into any region an argument points into.
        let mut regions = Regions::stat();
        for v in &values {
            regions = regions.join(&v.regions);
        }
        let carries = self.type_carries_region(&result);
        let regions = if carries { regions } else { Regions::stat() };
        Expr {
            kind: ExprKind::Call {
                proc: p,
                args: values,
            },
            ty: result,
            regions,
            span,
        }
    }

    fn check_layout_at(
        &mut self,
        ctx: &mut Ctx,
        id: crate::ty::LayoutId,
        args: &[oli_ast::Arg],
        span: Span,
    ) -> Expr {
        let params = vec![("view".to_string(), Ty::view_u8(false))];
        // Accept `rw view u8` too: check the argument untyped, then classify.
        if args.len() != 1 || args[0].name.is_some() {
            let _ = self.check_args(ctx, args, &params, "`T.at`", span);
            return Expr::error(span);
        }
        let v = self.check_expr(ctx, &args[0].value, None);
        let v = self.require_value(ctx, v);
        let rw = match &v.ty {
            Ty::View {
                rw,
                elem,
                mmio: false,
            } if matches!(**elem, Ty::Int(IntTy::U8)) => *rw,
            Ty::Error => return Expr::error(span),
            other => {
                let t = self.type_text(other);
                self.error(
                    ctx.module,
                    "E0200",
                    format!("`T.at` needs a `view u8`, found `{t}`"),
                    v.span,
                );
                return Expr::error(span);
            }
        };
        let regions = v.regions.clone();
        let ty = Ty::Ref {
            mmio: false,
            rw,
            elem: Box::new(Ty::Layout(id)),
        };
        Expr {
            kind: ExprKind::LayoutAt {
                layout: id,
                view: Box::new(v),
            },
            ty,
            regions,
            span,
        }
    }

    fn check_zone_method(
        &mut self,
        ctx: &mut Ctx,
        zone: Expr,
        name: &oli_ast::Ident,
        args: &[oli_ast::Arg],
        span: Span,
    ) -> Expr {
        let regions = zone.regions.clone();
        match name.name.as_str() {
            "bytes" | "try_bytes" => {
                let params = vec![("size".to_string(), Ty::uword())];
                let what = format!("`zone.{}`", name.name);
                let Some(mut values) = self.check_args(ctx, args, &params, &what, span) else {
                    return Expr::error(span);
                };
                let size = values.pop().unwrap_or_else(|| Expr::error(span));
                let fallible = name.name == "try_bytes";
                let view = Ty::view_u8(true);
                let ty = if fallible {
                    Ty::Fallible(Box::new(view), Box::new(Ty::None))
                } else {
                    view
                };
                let kind = ExprKind::ZoneBytes {
                    zone: Box::new(zone),
                    size: Box::new(size),
                    fallible,
                };
                Expr {
                    kind,
                    ty,
                    regions,
                    span,
                }
            }
            "make" => {
                let [arg] = args else {
                    self.error(
                        ctx.module,
                        "E0205",
                        "`zone.make` takes exactly one type argument",
                        span,
                    );
                    return Expr::error(span);
                };
                let ty = match &arg.value.kind {
                    oli_ast::ExprKind::TypeRef(t) => self.resolve_type(ctx.module, t),
                    _ => match self.expr_symbol(ctx, &arg.value) {
                        Some(Symbol::Layout(id)) => Ty::Layout(id),
                        Some(Symbol::Choice(id)) => Ty::Choice(id),
                        _ => {
                            let msg = "`zone.make` needs a type, e.g. `z.make(Header)`";
                            self.error(ctx.module, "E0200", msg, arg.value.span);
                            return Expr::error(span);
                        }
                    },
                };
                if ty.is_error() {
                    return Expr::error(span);
                }
                let result = Ty::Ref {
                    mmio: false,
                    rw: true,
                    elem: Box::new(ty.clone()),
                };
                Expr {
                    kind: ExprKind::ZoneMake {
                        zone: Box::new(zone),
                        ty,
                    },
                    ty: result,
                    regions,
                    span,
                }
            }
            other => {
                let msg =
                    format!("zone handles have `bytes`, `try_bytes` and `make`, not `{other}`");
                self.error(ctx.module, "E0105", msg, name.span);
                Expr::error(span)
            }
        }
    }

    fn check_intrinsic_call(
        &mut self,
        ctx: &mut Ctx,
        sig: IntrinsicSig,
        args: &[oli_ast::Arg],
        span: Span,
    ) -> Expr {
        if let Some(cap) = sig.permit {
            self.require_permit(ctx, cap, &format!("calling `{}`", sig.name), span);
        }
        let values = if sig.which == Intrinsic::OsSyscall {
            if args.is_empty() || args.len() > 7 || args.iter().any(|a| a.name.is_some()) {
                let msg =
                    "`os.syscall` takes the syscall number and up to six positional arguments";
                self.error(ctx.module, "E0205", msg, span);
                for a in args {
                    let _ = self.check_expr(ctx, &a.value, None);
                }
                return Expr::error(span);
            }
            let mut out = Vec::new();
            for a in args {
                let v = self.check_expr(ctx, &a.value, None);
                let v = self.require_value(ctx, v);
                out.push(self.syscall_arg(ctx, v));
            }
            out
        } else {
            let params: Vec<(String, Ty)> = sig
                .params
                .iter()
                .enumerate()
                .map(|(i, t)| (format!("arg{i}"), t.clone()))
                .collect();
            match self.check_args(ctx, args, &params, &format!("`{}`", sig.name), span) {
                Some(v) => v,
                None => return Expr::error(span),
            }
        };
        let kind = ExprKind::Intrinsic {
            which: sig.which,
            args: values,
        };
        Expr {
            kind,
            ty: sig.result,
            regions: Regions::stat(),
            span,
        }
    }

    /// Syscall arguments are machine words: any integer widens, addresses pass as-is.
    fn syscall_arg(&mut self, ctx: &mut Ctx, v: Expr) -> Expr {
        let span = v.span;
        match &v.ty {
            Ty::UntypedInt => self.coerce(ctx, v, &Ty::word()),
            Ty::Int(i) if i.signed() => self.coerce(ctx, v, &Ty::word()),
            Ty::Int(_) => {
                let u = self.coerce(ctx, v, &Ty::uword());
                let kind = ExprKind::Convert {
                    kind: ConvKind::Bits,
                    expr: Box::new(u),
                };
                Expr {
                    kind,
                    ty: Ty::word(),
                    regions: Regions::stat(),
                    span,
                }
            }
            Ty::Addr(_) | Ty::Error => v,
            other => {
                let t = self.type_text(other);
                let msg = format!("syscall arguments are integers or addresses, found `{t}`");
                self.error(ctx.module, "E0200", msg, span);
                Expr::error(span)
            }
        }
    }
}

pub(crate) struct IntrinsicSig {
    pub name: &'static str,
    pub which: Intrinsic,
    pub params: Vec<Ty>,
    pub result: Ty,
    pub permit: Option<Capability>,
}

#[rustfmt::skip]
const MEM_ACCESSORS: &[(&str, &str, Option<bool>, u32)] = &[
    ("get_u16", "mem.get_u16", None, 16), ("get_u32", "mem.get_u32", None, 32), ("get_u64", "mem.get_u64", None, 64),
    ("get_be16", "mem.get_be16", Some(true), 16), ("get_be32", "mem.get_be32", Some(true), 32), ("get_be64", "mem.get_be64", Some(true), 64),
    ("get_le16", "mem.get_le16", Some(false), 16), ("get_le32", "mem.get_le32", Some(false), 32), ("get_le64", "mem.get_le64", Some(false), 64),
    ("put_u16", "mem.put_u16", None, 16), ("put_u32", "mem.put_u32", None, 32), ("put_u64", "mem.put_u64", None, 64),
    ("put_be16", "mem.put_be16", Some(true), 16), ("put_be32", "mem.put_be32", Some(true), 32), ("put_be64", "mem.put_be64", Some(true), 64),
    ("put_le16", "mem.put_le16", Some(false), 16), ("put_le32", "mem.put_le32", Some(false), 32), ("put_le64", "mem.put_le64", Some(false), 64),
];

/// The V0 intrinsic table (`spec/OLI_SEMANTICS_V0.md` §9).
fn intrinsic_sig(ns: Namespace, name: &str) -> Option<IntrinsicSig> {
    let sig = |name: &'static str,
               which: Intrinsic,
               params: Vec<Ty>,
               result: Ty,
               permit: Option<Capability>| {
        Some(IntrinsicSig {
            name,
            which,
            params,
            result,
            permit,
        })
    };
    let rw_bytes = Ty::view_u8(true);
    let bytes = Ty::view_u8(false);
    match (ns, name) {
        (Namespace::Os, "syscall") => sig(
            "os.syscall",
            Intrinsic::OsSyscall,
            Vec::new(),
            Ty::word(),
            Some(Capability::OsSyscall),
        ),
        (Namespace::Cpu, "halt") => sig(
            "cpu.halt",
            Intrinsic::CpuHalt,
            Vec::new(),
            Ty::Unit,
            Some(Capability::CpuHalt),
        ),
        (Namespace::Cpu, "pause") => {
            sig("cpu.pause", Intrinsic::CpuPause, Vec::new(), Ty::Unit, None)
        }
        (Namespace::Mem, "copy") => sig(
            "mem.copy",
            Intrinsic::MemCopy,
            vec![rw_bytes, bytes, Ty::uword()],
            Ty::Unit,
            None,
        ),
        (Namespace::Mem, "set") => sig(
            "mem.set",
            Intrinsic::MemSet,
            vec![rw_bytes, Ty::u8()],
            Ty::Unit,
            None,
        ),
        (Namespace::Mem, "zero") => sig(
            "mem.zero",
            Intrinsic::MemZero,
            vec![rw_bytes],
            Ty::Unit,
            None,
        ),
        (Namespace::Mem, "secure_zero") => sig(
            "mem.secure_zero",
            Intrinsic::MemSecureZero,
            vec![rw_bytes],
            Ty::Unit,
            None,
        ),
        (Namespace::Mem, n) => {
            let (_, full, order, bits) = MEM_ACCESSORS.iter().find(|(short, ..)| *short == n)?;
            let int = match bits {
                16 => Ty::Int(IntTy::U16),
                32 => Ty::Int(IntTy::U32),
                _ => Ty::Int(IntTy::U64),
            };
            if n.starts_with("get_") {
                sig(
                    full,
                    Intrinsic::MemGet(*order, *bits),
                    vec![bytes],
                    int,
                    None,
                )
            } else {
                sig(
                    full,
                    Intrinsic::MemPut(*order, *bits),
                    vec![rw_bytes, int],
                    Ty::Unit,
                    None,
                )
            }
        }
        _ => None,
    }
}
