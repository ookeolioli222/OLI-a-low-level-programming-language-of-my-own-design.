//! Compile-time evaluation of module constants and static initializers (`spec` §10).

use super::body::Ctx;
use super::{Sema, State};
use crate::hir::*;
use crate::ty::{IntTy, Ty};
use oli_diag::{Diagnostic, Span};

impl Sema<'_> {
    pub(crate) fn const_ready(&mut self, id: ConstId) {
        match self.const_state[id] {
            State::Done => return,
            State::InProgress => {
                let (module, span, name) = (
                    self.prog.consts[id].module,
                    self.prog.consts[id].span,
                    self.prog.consts[id].name.clone(),
                );
                self.report(
                    module,
                    Diagnostic::error(
                        "E0106",
                        format!("constant `{name}` depends on itself"),
                        span,
                    )
                    .with_note("constants are evaluated at compile time and cannot be cyclic"),
                );
                self.const_state[id] = State::Done;
                return;
            }
            State::Pending => {}
        }
        self.const_state[id] = State::InProgress;
        let (module, decl) = self.const_asts[id].clone();
        let oli_ast::DeclKind::Const {
            ty, value, clauses, ..
        } = decl.kind
        else {
            self.const_state[id] = State::Done;
            return;
        };
        let declared = ty.as_ref().map(|t| self.resolve_type(module, t));
        let mut ctx = Ctx::new(module);
        ctx.permits = vec![Capability::MemoryRaw]; // a constant address is a number, not an access
        let v = self.check_expr(&mut ctx, &value, declared.as_ref());
        let ty = declared.unwrap_or_else(|| v.ty.clone());
        let ty = match ty {
            Ty::Unit => {
                self.error(module, "E0207", "this call produces no value", v.span);
                Ty::Error
            }
            t => t,
        };
        let cv = if ty.is_error() {
            None
        } else {
            self.eval_const(&ctx, &v)
        };
        let cv = match cv {
            Some(c) => c,
            None => {
                if !ty.is_error() {
                    self.report(
                        module,
                        Diagnostic::error("E0107", "this is not a compile-time constant", v.span)
                            .with_note("constants may use literals, other constants, arithmetic, conversions, `T.size`/`T.align` and aggregate literals"),
                    );
                }
                ConstValue::None
            }
        };
        let aggregate = matches!(ty, Ty::Array(..) | Ty::Layout(_) | Ty::Choice(_));
        if aggregate {
            let (section, align) = self.place_clauses(module, &clauses);
            let sid = self.prog.statics.len();
            let (name, is_pub) = (
                self.prog.consts[id].name.clone(),
                self.prog.consts[id].is_pub,
            );
            self.prog.statics.push(StaticDef {
                module,
                name,
                is_pub,
                ty: ty.clone(),
                init: Some(cv.clone()),
                readonly: true,
                section,
                align,
                span: decl.span,
            });
            self.static_asts.push(None);
            self.static_state.push(State::Done);
            self.const_backing[id] = Some(sid);
        } else if let Some(c) = clauses.first() {
            self.report(
                module,
                Diagnostic::error(
                    "E0018",
                    "`section`/`align` apply to aggregate constants only",
                    c.span,
                )
                .with_note("integer, boolean and string constants occupy no memory of their own"),
            );
        }
        let def = &mut self.prog.consts[id];
        def.ty = ty;
        def.value = cv;
        self.const_state[id] = State::Done;
    }

    fn place_clauses(
        &mut self,
        module: ModuleId,
        clauses: &[oli_ast::Clause],
    ) -> (Option<Vec<u8>>, Option<u64>) {
        let mut section = None;
        let mut align = None;
        for c in clauses {
            match &c.kind {
                oli_ast::ClauseKind::Section(s) => section = Some(s.clone()),
                oli_ast::ClauseKind::Align(a) => align = u64::try_from(*a).ok(),
                _ => self.error(
                    module,
                    "E0018",
                    "only `section` and `align` apply here",
                    c.span,
                ),
            }
        }
        (section, align)
    }

    pub(crate) fn static_ready(&mut self, id: StaticId) {
        if self.static_state[id] != State::Pending {
            return;
        }
        self.static_state[id] = State::InProgress;
        let Some((module, decl)) = self.static_asts[id].clone() else {
            self.static_state[id] = State::Done;
            return;
        };
        let oli_ast::DeclKind::Static {
            ty, init, clauses, ..
        } = decl.kind
        else {
            self.static_state[id] = State::Done;
            return;
        };
        let ty = self.resolve_type(module, &ty);
        let (section, align) = self.place_clauses(module, &clauses);
        let init = init.and_then(|e| {
            let mut ctx = Ctx::new(module);
            ctx.permits = vec![Capability::MemoryRaw];
            let v = self.check_expr(&mut ctx, &e, Some(&ty));
            let v = self.require_value(&ctx, v);
            if v.ty.is_error() {
                return None;
            }
            let cv = self.eval_const(&ctx, &v);
            if cv.is_none() {
                self.report(
                    module,
                    Diagnostic::error("E0107", "a static initializer must be a compile-time constant", v.span)
                        .with_note("store at run time instead: leave the static uninitialized (zeroed) and assign in a procedure"),
                );
            }
            cv
        });
        let def = &mut self.prog.statics[id];
        def.ty = ty;
        def.init = init;
        def.section = section;
        def.align = align;
        self.static_state[id] = State::Done;
    }

    /// Evaluates a checked expression at compile time, or returns `None`
    /// when it is not constant. Overflow and division by zero are errors.
    pub(crate) fn eval_const(&mut self, ctx: &Ctx, e: &Expr) -> Option<ConstValue> {
        match &e.kind {
            ExprKind::Int(v) => Some(ConstValue::Int(*v)),
            ExprKind::Bool(b) => Some(ConstValue::Bool(*b)),
            ExprKind::Str(s) => Some(ConstValue::Str(s.clone())),
            ExprKind::NoneVal => Some(ConstValue::None),
            ExprKind::Load(place) | ExprKind::ViewOfArray(place) => match &place.kind {
                PlaceKind::Static(s) if self.prog.statics[*s].readonly => {
                    self.prog.statics[*s].init.clone()
                }
                _ => None,
            },
            ExprKind::ValueField(base, idx) => match self.eval_const(ctx, base)? {
                ConstValue::Layout(_, fields) => fields.get(*idx).cloned(),
                _ => None,
            },
            ExprKind::Convert { kind, expr } => {
                let inner = self.eval_const(ctx, expr)?;
                let ConstValue::Int(v) = inner else {
                    return Some(inner);
                };
                let target = e.ty.as_int();
                Some(ConstValue::Int(match (kind, target) {
                    (ConvKind::Wrap, Some(t)) => t.wrap(v),
                    (ConvKind::Sat, Some(t)) => t.saturate(v),
                    (ConvKind::Bits, Some(t)) => t.wrap(v),
                    (ConvKind::Checked, _) => return None,
                    _ => v,
                }))
            }
            ExprKind::Unary(op, x) => {
                let v = self.eval_const(ctx, x)?;
                match (op, v) {
                    (UnOp::Not, ConstValue::Bool(b)) => Some(ConstValue::Bool(!b)),
                    (UnOp::Neg, ConstValue::Int(n)) => self.fit_const(ctx, e, n.checked_neg()),
                    (UnOp::BitNot, ConstValue::Int(n)) => match e.ty.as_int() {
                        Some(t) => Some(ConstValue::Int(t.wrap(!n))),
                        None => Some(ConstValue::Int(!n)),
                    },
                    _ => None,
                }
            }
            ExprKind::Binary { op, mode, lhs, rhs } => {
                let a = self.eval_const(ctx, lhs)?;
                let b = self.eval_const(ctx, rhs)?;
                match (a, b) {
                    (ConstValue::Bool(x), ConstValue::Bool(y)) => {
                        Some(ConstValue::Bool(match op {
                            BinOp::And => x && y,
                            BinOp::Or => x || y,
                            BinOp::Eq => x == y,
                            BinOp::Ne => x != y,
                            _ => return None,
                        }))
                    }
                    (ConstValue::Int(x), ConstValue::Int(y)) => {
                        self.eval_int_binop(ctx, e, *op, *mode, x, y)
                    }
                    _ => None,
                }
            }
            ExprKind::LayoutLit { layout, fields } => {
                let vals: Option<Vec<ConstValue>> =
                    fields.iter().map(|f| self.eval_const(ctx, f)).collect();
                Some(ConstValue::Layout(*layout, vals?))
            }
            ExprKind::VariantLit {
                choice,
                index,
                fields,
            } => {
                let vals: Option<Vec<ConstValue>> =
                    fields.iter().map(|f| self.eval_const(ctx, f)).collect();
                Some(ConstValue::Variant(*choice, *index, vals?))
            }
            ExprKind::ArrayLit(items) => {
                let vals: Option<Vec<ConstValue>> =
                    items.iter().map(|f| self.eval_const(ctx, f)).collect();
                Some(ConstValue::Array(vals?))
            }
            _ => None,
        }
    }

    fn fit_const(&mut self, ctx: &Ctx, e: &Expr, v: Option<i128>) -> Option<ConstValue> {
        let fits = v.filter(|v| e.ty.as_int().map_or(true, |t| t.fits(*v)));
        match fits {
            Some(v) => Some(ConstValue::Int(v)),
            None => {
                self.report(
                    ctx.module,
                    Diagnostic::error("E0212", "constant expression overflows its type", e.span)
                        .with_note("plain arithmetic traps on overflow; use `wrap(...)` or `sat(...)` if that is intended"),
                );
                Some(ConstValue::Int(0))
            }
        }
    }

    fn eval_int_binop(
        &mut self,
        ctx: &Ctx,
        e: &Expr,
        op: BinOp,
        mode: ArithMode,
        x: i128,
        y: i128,
    ) -> Option<ConstValue> {
        let span: Span = e.span;
        let ty = e.ty.as_int();
        let width = ty.map_or(128, IntTy::bits);
        let raw = match op {
            BinOp::Add => x.checked_add(y),
            BinOp::Sub => x.checked_sub(y),
            BinOp::Mul => x.checked_mul(y),
            BinOp::Div | BinOp::Rem => {
                if y == 0 {
                    self.error(
                        ctx.module,
                        "E0213",
                        "division by zero in a constant expression",
                        span,
                    );
                    return Some(ConstValue::Int(0));
                }
                if op == BinOp::Div {
                    x.checked_div(y)
                } else {
                    x.checked_rem(y)
                }
            }
            BinOp::BitAnd => Some(x & y),
            BinOp::BitOr => Some(x | y),
            BinOp::BitXor => Some(x ^ y),
            BinOp::Shl => Some(x.wrapping_shl((y.rem_euclid(i128::from(width))) as u32)),
            BinOp::Shr => Some(x >> ((y.rem_euclid(i128::from(width))) as u32)),
            BinOp::Eq => return Some(ConstValue::Bool(x == y)),
            BinOp::Ne => return Some(ConstValue::Bool(x != y)),
            BinOp::Lt => return Some(ConstValue::Bool(x < y)),
            BinOp::Le => return Some(ConstValue::Bool(x <= y)),
            BinOp::Gt => return Some(ConstValue::Bool(x > y)),
            BinOp::Ge => return Some(ConstValue::Bool(x >= y)),
            BinOp::And | BinOp::Or => return None,
        };
        match (mode, ty) {
            (ArithMode::Wrap, Some(t)) => Some(ConstValue::Int(t.wrap(raw.unwrap_or(0)))),
            (ArithMode::Sat, Some(t)) => Some(ConstValue::Int(
                raw.map_or(if x < 0 { t.min() } else { t.max() }, |v| t.saturate(v)),
            )),
            (ArithMode::Checked, _) => None,
            _ => self.fit_const(ctx, e, raw),
        }
    }
}
