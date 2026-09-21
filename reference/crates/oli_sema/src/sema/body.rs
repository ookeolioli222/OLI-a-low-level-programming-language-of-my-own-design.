//! Procedure bodies: scopes, statements, definite assignment, reachability.

use super::{Sema, Symbol};
use crate::hir::*;
use crate::regions::{Region, Regions};
use crate::ty::Ty;
use oli_diag::{Diagnostic, Span};
use std::collections::HashMap;

/// Flow-sensitive state: is this point reachable, which places are assigned.
#[derive(Clone, Debug)]
pub(crate) struct Flow {
    pub reachable: bool,
    pub assigned: Vec<bool>,
}

impl Flow {
    fn new() -> Flow {
        Flow {
            reachable: true,
            assigned: Vec::new(),
        }
    }

    /// The state after control could arrive from either `a` or `b`.
    pub(crate) fn merge(a: &Flow, b: &Flow) -> Flow {
        match (a.reachable, b.reachable) {
            (false, false) => Flow {
                reachable: false,
                assigned: a.assigned.clone(),
            },
            (true, false) => a.clone(),
            (false, true) => b.clone(),
            (true, true) => {
                let n = a.assigned.len().max(b.assigned.len());
                let assigned = (0..n)
                    .map(|i| {
                        a.assigned.get(i).copied().unwrap_or(true)
                            && b.assigned.get(i).copied().unwrap_or(true)
                    })
                    .collect();
                Flow {
                    reachable: true,
                    assigned,
                }
            }
        }
    }

    pub(crate) fn diverge(&mut self) {
        self.reachable = false;
    }

    /// Marks a local as definitely assigned (the vector may be shorter than
    /// the local table after a merge with an older snapshot).
    pub(crate) fn set_assigned(&mut self, id: LocalId) {
        if self.assigned.len() <= id {
            self.assigned.resize(id + 1, true);
        }
        self.assigned[id] = true;
    }

    pub(crate) fn is_assigned(&self, id: LocalId) -> bool {
        self.assigned.get(id).copied().unwrap_or(true)
    }
}

/// Per-procedure (or per-constant) checking context.
pub(crate) struct Ctx {
    pub module: ModuleId,
    pub locals: Vec<LocalDef>,
    pub local_regions: Vec<Regions>,
    pub used: Vec<bool>,
    pub scopes: Vec<HashMap<String, LocalId>>,
    pub zone_stack: Vec<LocalId>,
    /// The declared result type (possibly fallible); `None` = no result.
    pub result: Option<Ty>,
    pub permits: Vec<Capability>,
    pub conv: CallConv,
    pub mode: ArithMode,
    pub flow: Flow,
    /// Break states of the enclosing loops, innermost last.
    pub loops: Vec<Vec<Flow>>,
    pub unreachable_reported: bool,
}

impl Ctx {
    pub(crate) fn new(module: ModuleId) -> Ctx {
        Ctx {
            module,
            locals: Vec::new(),
            local_regions: Vec::new(),
            used: Vec::new(),
            scopes: vec![HashMap::new()],
            zone_stack: Vec::new(),
            result: None,
            permits: Vec::new(),
            conv: CallConv::Sysv,
            mode: ArithMode::Trap,
            flow: Flow::new(),
            loops: Vec::new(),
            unreachable_reported: false,
        }
    }

    pub(crate) fn has_permit(&self, cap: Capability) -> bool {
        self.permits.contains(&cap)
    }

    pub(crate) fn lookup_local(&self, name: &str) -> Option<LocalId> {
        self.scopes.iter().rev().find_map(|s| s.get(name).copied())
    }

    /// The ok payload type of the result (`T` of `T or E`, or `T`).
    pub(crate) fn ok_result(&self) -> Option<Ty> {
        match &self.result {
            Some(Ty::Fallible(ok, _)) => Some((**ok).clone()),
            other => other.clone(),
        }
    }

    pub(crate) fn fail_type(&self) -> Option<Ty> {
        match &self.result {
            Some(Ty::Fallible(_, err)) => Some((**err).clone()),
            _ => None,
        }
    }

    /// Zone nesting: does the block of zone `outer` enclose the block of `inner`?
    pub(crate) fn zone_encloses(&self, outer: LocalId, inner: LocalId) -> bool {
        let mut cur = self.locals.get(inner).and_then(|l| l.parent_zone);
        while let Some(z) = cur {
            if z == outer {
                return true;
            }
            cur = self.locals.get(z).and_then(|l| l.parent_zone);
        }
        false
    }

    /// Region of memory owned by a local place (its enclosing zone block, or the frame).
    pub(crate) fn scope_region(&self, local: LocalId) -> Region {
        match self.locals.get(local).and_then(|l| l.scope_zone) {
            Some(z) => Region::Zone(z),
            None => Region::Frame,
        }
    }
}

impl Sema<'_> {
    pub(crate) fn check_proc(&mut self, id: ProcId) {
        let (module, ast) = self.proc_asts[id].clone();
        let def = &self.prog.procs[id];
        let mut ctx = Ctx::new(module);
        ctx.result = def.result.clone();
        ctx.permits = def.permits.clone();
        ctx.conv = def.conv;
        ctx.locals = def.locals.clone();
        let param_tys: Vec<Ty> = ctx.locals.iter().map(|l| l.ty.clone()).collect();
        for (i, ty) in param_tys.iter().enumerate() {
            let carries = self.type_carries_region(ty);
            ctx.local_regions.push(if carries {
                Regions::one(Region::Param(i))
            } else {
                Regions::stat()
            });
            ctx.used.push(false);
            ctx.flow.assigned.push(true);
            let name = ctx.locals[i].name.clone();
            if let Some(scope) = ctx.scopes.last_mut() {
                scope.insert(name, i);
            }
        }
        let names: Vec<(String, Span)> = ctx
            .locals
            .iter()
            .map(|l| (l.name.clone(), l.span))
            .collect();
        for (name, span) in names {
            self.check_no_module_shadow(&ctx, &name, span);
        }
        let body = if ctx.conv == CallConv::None {
            self.check_naked_body(&mut ctx, &ast.body)
        } else {
            self.check_block(&mut ctx, &ast.body)
        };
        if ctx.flow.reachable {
            match &ctx.result {
                None => {}
                Some(Ty::Never) => {
                    let msg = format!(
                        "procedure `{}` is declared `-> never` but its end is reachable",
                        ast.name.name
                    );
                    self.report(
                        module,
                        Diagnostic::error("E0230", msg, ast.name.span).with_note(
                            "end with `loop ... end`, a call to a `never` procedure, or `fail`",
                        ),
                    );
                }
                Some(_) => {
                    let msg = format!("missing `ret` at the end of procedure `{}`", ast.name.name);
                    self.report(
                        module,
                        Diagnostic::error("E0230", msg, ast.name.span)
                            .with_note("every path must end in `ret`, `fail` or a `never` call"),
                    );
                }
            }
        }
        self.report_unused(&ctx);
        let def = &mut self.prog.procs[id];
        def.locals = ctx.locals;
        def.body = body;
    }

    fn check_naked_body(&mut self, ctx: &mut Ctx, body: &oli_ast::Block) -> Block {
        let only_machine = body.stmts.len() == 1
            && matches!(
                body.stmts.first().map(|s| &s.kind),
                Some(oli_ast::StmtKind::Machine(_))
            );
        if !only_machine {
            let span = body.stmts.first().map_or(body.span, |s| s.span);
            self.report(
                ctx.module,
                Diagnostic::error("E0603", "a `calls none` procedure body must be exactly one `machine` block", span)
                    .with_note("without a prologue the compiler cannot promise a valid stack, so no Oli-- statements are allowed"),
            );
        }
        let block = self.check_block(ctx, body);
        ctx.flow.diverge(); // the machine code owns control flow entirely
        block
    }

    pub(crate) fn type_carries_region(&mut self, t: &Ty) -> bool {
        match t {
            Ty::Ref { .. } | Ty::View { .. } | Ty::Zone => true,
            Ty::Array(_, e) => self.type_carries_region(e),
            Ty::Fallible(a, b) => self.type_carries_region(a) || self.type_carries_region(b),
            Ty::Layout(id) => {
                self.layout_ready(*id);
                let tys: Vec<Ty> = self.prog.layouts[*id]
                    .fields
                    .iter()
                    .map(|f| f.ty.clone())
                    .collect();
                tys.iter().any(|t| self.type_carries_region(t))
            }
            Ty::Choice(id) => {
                self.choice_ready(*id);
                let tys: Vec<Ty> = self.prog.choices[*id]
                    .variants
                    .iter()
                    .flat_map(|v| v.fields.iter().map(|f| f.ty.clone()))
                    .collect();
                tys.iter().any(|t| self.type_carries_region(t))
            }
            _ => false,
        }
    }

    fn report_unused(&mut self, ctx: &Ctx) {
        for (i, l) in ctx.locals.iter().enumerate() {
            if ctx.used.get(i).copied().unwrap_or(true) || l.name.starts_with('_') {
                continue;
            }
            let what = match l.kind {
                LocalKind::Param(_) => "parameter",
                LocalKind::Binding => "binding",
                LocalKind::Place => "place",
                LocalKind::ZoneHandle | LocalKind::PatternBinding => continue,
            };
            let msg = format!("{what} `{}` is never read", l.name);
            let note = format!("prefix it with `_` (`_{}`) if this is intended", l.name);
            self.report(
                ctx.module,
                Diagnostic::warning("W0002", msg, l.span).with_note(note),
            );
        }
    }

    // ------------------------------------------------------------ scopes

    /// Reports `E0101` when `name` shadows a module-level name or an alias.
    fn check_no_module_shadow(&mut self, ctx: &Ctx, name: &str, span: Span) {
        let clash = self.lookup_module_symbol(ctx.module, name).is_some()
            || super::Namespace::from_name(name).is_some()
            || name == "core";
        if clash {
            let msg = format!("`{name}` shadows a module-level name");
            self.report(
                ctx.module,
                Diagnostic::error("E0101", msg, span).with_note(
                    "Oli-- never lets a local silently rebind a visible name; pick another name",
                ),
            );
        }
    }

    /// Declares a local in the innermost scope; `E0101` if it shadows anything visible.
    pub(crate) fn declare_local(
        &mut self,
        ctx: &mut Ctx,
        name: &oli_ast::Ident,
        ty: Ty,
        kind: LocalKind,
        regions: Regions,
    ) -> LocalId {
        if let Some(prev) = ctx.lookup_local(&name.name) {
            let prev_span = ctx.locals[prev].span;
            let msg = format!("`{}` shadows a name visible here", name.name);
            self.report(
                ctx.module,
                Diagnostic::error("E0101", msg, name.span)
                    .with_secondary(prev_span, "earlier declaration")
                    .with_note("Oli-- never lets a local silently rebind a visible name; pick another name"),
            );
        } else {
            self.check_no_module_shadow(ctx, &name.name, name.span);
        }
        if let Ty::Endian { .. } = ty {
            self.report(
                ctx.module,
                Diagnostic::error("E0200", "`be`/`le` integers are layout field types only", name.span)
                    .with_note("declare the local with the native integer type; byte order applies where the value is stored in a layout"),
            );
        }
        let id = ctx.locals.len();
        let scope_zone = if kind == LocalKind::Place {
            ctx.zone_stack.last().copied()
        } else {
            None
        };
        let parent_zone = if kind == LocalKind::ZoneHandle {
            ctx.zone_stack.last().copied()
        } else {
            None
        };
        ctx.locals.push(LocalDef {
            name: name.name.clone(),
            ty,
            kind,
            scope_zone,
            parent_zone,
            span: name.span,
        });
        ctx.local_regions.push(regions);
        ctx.used.push(false);
        // Aggregate places are zero-initialized at declaration (spec/OLI_MEMORY_V0.md §2);
        // scalar places are tracked for definite assignment.
        let assigned = kind != LocalKind::Place || super::expr::is_aggregate(&ctx.locals[id].ty);
        ctx.flow.assigned.resize(id, true);
        ctx.flow.assigned.push(assigned);
        if let Some(scope) = ctx.scopes.last_mut() {
            scope.insert(name.name.clone(), id);
        }
        id
    }

    pub(crate) fn push_scope(ctx: &mut Ctx) {
        ctx.scopes.push(HashMap::new());
    }

    pub(crate) fn pop_scope(ctx: &mut Ctx) {
        ctx.scopes.pop();
    }

    // -------------------------------------------------------- statements

    /// Checks a block in a fresh scope. Unreachable statements are reported
    /// once per block and still checked.
    pub(crate) fn check_block(&mut self, ctx: &mut Ctx, block: &oli_ast::Block) -> Block {
        Self::push_scope(ctx);
        let saved_flag = ctx.unreachable_reported;
        ctx.unreachable_reported = false;
        let mut stmts = Vec::new();
        for s in &block.stmts {
            if !ctx.flow.reachable && !ctx.unreachable_reported {
                ctx.unreachable_reported = true;
                self.report(
                    ctx.module,
                    Diagnostic::error("E0231", "unreachable statement", s.span).with_note(
                        "control never reaches this point after the previous `ret`, `fail`, `break`, `continue` or `never` call",
                    ),
                );
            }
            if let Some(t) = self.check_stmt(ctx, s) {
                stmts.push(t);
            }
        }
        ctx.unreachable_reported = saved_flag;
        Self::pop_scope(ctx);
        Block { stmts }
    }

    fn check_stmt(&mut self, ctx: &mut Ctx, s: &oli_ast::Stmt) -> Option<Stmt> {
        let span = s.span;
        let kind = match &s.kind {
            oli_ast::StmtKind::Bind { name, ty, value } => {
                let declared = ty.as_ref().map(|t| self.local_type(ctx, t));
                let value = self.check_expr(ctx, value, declared.as_ref());
                let value = self.require_value(ctx, value);
                let ty = declared.unwrap_or_else(|| value.ty.clone());
                let ty = self.settle_untyped_error(ctx, ty, value.span);
                let regions = value.regions.clone();
                let id = self.declare_local(ctx, name, ty, LocalKind::Binding, regions);
                StmtKind::Bind(id, value)
            }
            oli_ast::StmtKind::Place { name, ty, init } => {
                let ty = self.local_type(ctx, ty);
                let init = init.as_ref().map(|e| {
                    let v = self.check_expr(ctx, e, Some(&ty));
                    self.require_value(ctx, v)
                });
                let id = self.declare_local(ctx, name, ty, LocalKind::Place, Regions::stat());
                if let Some(v) = &init {
                    ctx.flow.set_assigned(id);
                    let scope = ctx.scope_region(id);
                    self.check_store_regions(ctx, &v.regions, &Regions::one(scope), v.span);
                }
                StmtKind::Place(id, init)
            }
            oli_ast::StmtKind::Store { target, value } => {
                let place = self.check_place(ctx, target, true)?;
                let want = super::expr::value_ty(&place.ty);
                let value = self.check_expr(ctx, value, Some(&want));
                let value = self.require_value(ctx, value);
                let regions = place.regions.clone();
                self.check_store_regions(ctx, &value.regions, &regions, value.span);
                if let PlaceKind::Local(id) = place.kind {
                    ctx.flow.set_assigned(id);
                }
                StmtKind::Store(place, value)
            }
            oli_ast::StmtKind::Move { .. } => {
                self.not_implemented(ctx.module, "`<~` moves of `own` values", span);
                return None;
            }
            oli_ast::StmtKind::Expr(e) => {
                let value = self.check_expr(ctx, e, None);
                if value.ty.is_fallible() {
                    let t = self.type_text(&value.ty);
                    self.report(
                        ctx.module,
                        Diagnostic::error("E0310", format!("unhandled failure: this value has type `{t}`"), value.span)
                            .with_note("resolve it with `else fail`, `else ret ...`, `else default` or a `case`"),
                    );
                }
                if matches!(value.ty, Ty::Never) {
                    ctx.flow.diverge();
                }
                StmtKind::Expr(value)
            }
            oli_ast::StmtKind::If {
                cond,
                then,
                elifs,
                els,
            } => self.check_if(ctx, cond, then, elifs, els.as_ref()),
            oli_ast::StmtKind::IfThen { cond, stmt } => {
                let then = oli_ast::Block {
                    stmts: vec![(**stmt).clone()],
                    span: stmt.span,
                };
                self.check_if(ctx, cond, &then, &[], None)
            }
            oli_ast::StmtKind::While { cond, body } => {
                let cond = self.check_cond(ctx, cond);
                let before = ctx.flow.clone();
                ctx.loops.push(Vec::new());
                let body = self.check_block(ctx, body);
                ctx.loops.pop();
                ctx.flow = before; // the body may not run at all
                StmtKind::While { cond, body }
            }
            oli_ast::StmtKind::Each { var, iter, body } => self.check_each(ctx, var, iter, body),
            oli_ast::StmtKind::Loop { body } => {
                ctx.loops.push(Vec::new());
                let body = self.check_block(ctx, body);
                let breaks = ctx.loops.pop().unwrap_or_default();
                let mut after = Flow {
                    reachable: false,
                    assigned: ctx.flow.assigned.clone(),
                };
                for b in &breaks {
                    after = Flow::merge(&after, b);
                }
                ctx.flow = after;
                StmtKind::Loop { body }
            }
            oli_ast::StmtKind::Break | oli_ast::StmtKind::Continue => {
                let is_break = matches!(s.kind, oli_ast::StmtKind::Break);
                if ctx.loops.is_empty() {
                    let word = if is_break { "break" } else { "continue" };
                    self.error(
                        ctx.module,
                        "E0232",
                        format!("`{word}` outside of a loop"),
                        span,
                    );
                } else if is_break {
                    let state = ctx.flow.clone();
                    if let Some(l) = ctx.loops.last_mut() {
                        l.push(state);
                    }
                }
                ctx.flow.diverge();
                if is_break {
                    StmtKind::Break
                } else {
                    StmtKind::Continue
                }
            }
            oli_ast::StmtKind::Ret(value) => self.check_ret(ctx, value.as_ref(), span),
            oli_ast::StmtKind::Fail(value) => self.check_fail(ctx, value.as_ref(), span),
            oli_ast::StmtKind::Zone {
                name,
                size,
                source,
                body,
            } => self.check_zone(ctx, name, size, source.as_ref(), body),
            oli_ast::StmtKind::Case {
                scrutinee,
                arms,
                els,
            } => self.check_case(ctx, scrutinee, arms, els.as_ref()),
            oli_ast::StmtKind::Machine(m) => StmtKind::Machine(self.check_machine(ctx, m)),
        };
        Some(Stmt { kind, span })
    }

    /// A local's declared type; `be`/`le` are field types only.
    fn local_type(&mut self, ctx: &Ctx, t: &oli_ast::Type) -> Ty {
        let ty = self.resolve_type(ctx.module, t);
        if let Ty::Endian { .. } = ty {
            self.report(
                ctx.module,
                Diagnostic::error("E0200", "`be`/`le` integers are layout field types only", t.span)
                    .with_note("declare the local with the native integer type; byte order applies where the value is stored in a layout"),
            );
            return Ty::Error;
        }
        ty
    }

    /// `E0300` when a value could point into memory that dies before the target.
    pub(crate) fn check_store_regions(
        &mut self,
        ctx: &Ctx,
        value: &Regions,
        target: &Regions,
        span: Span,
    ) {
        let encloses = |a: LocalId, b: LocalId| ctx.zone_encloses(a, b);
        for t in target.atoms() {
            if !value.outlives(*t, &encloses) {
                let names = |id: LocalId| {
                    ctx.locals
                        .get(id)
                        .map_or("?".to_string(), |l| l.name.clone())
                };
                let v = value.describe(&names);
                let tdesc = Regions::one(*t).describe(&names);
                self.report(
                    ctx.module,
                    Diagnostic::error("E0300", "value escapes the memory it points into", span)
                        .with_label(format!("points into {v}, but is stored in memory that lives longer ({tdesc})"))
                        .with_note("a view, ref or zone handle may only be stored where the memory it points into is still alive"),
                );
                return;
            }
        }
    }

    fn check_cond(&mut self, ctx: &mut Ctx, cond: &oli_ast::Expr) -> Expr {
        let c = self.check_expr(ctx, cond, Some(&Ty::Bool));
        self.require_value(ctx, c)
    }

    fn check_if(
        &mut self,
        ctx: &mut Ctx,
        cond: &oli_ast::Expr,
        then: &oli_ast::Block,
        elifs: &[(oli_ast::Expr, oli_ast::Block)],
        els: Option<&oli_ast::Block>,
    ) -> StmtKind {
        let cond = self.check_cond(ctx, cond);
        let entry = ctx.flow.clone();
        let then_b = self.check_block(ctx, then);
        let mut merged = ctx.flow.clone();
        let mut elif_out = Vec::new();
        let mut fallthrough = entry.clone();
        for (c, b) in elifs {
            ctx.flow = fallthrough.clone();
            let c = self.check_cond(ctx, c);
            fallthrough = ctx.flow.clone();
            let b = self.check_block(ctx, b);
            merged = Flow::merge(&merged, &ctx.flow);
            elif_out.push((c, b));
        }
        let els_out = match els {
            Some(b) => {
                ctx.flow = fallthrough.clone();
                let b = self.check_block(ctx, b);
                merged = Flow::merge(&merged, &ctx.flow);
                Some(b)
            }
            None => {
                merged = Flow::merge(&merged, &fallthrough);
                None
            }
        };
        ctx.flow = merged;
        StmtKind::If {
            cond,
            then: then_b,
            elifs: elif_out,
            els: els_out,
        }
    }

    fn check_each(
        &mut self,
        ctx: &mut Ctx,
        var: &oli_ast::Ident,
        iter: &oli_ast::Expr,
        body: &oli_ast::Block,
    ) -> StmtKind {
        let (iter, elem_ty, regions) = match &iter.kind {
            oli_ast::ExprKind::Range { start, end } => {
                let (a, b) = match end {
                    Some(b) => self.check_pair(ctx, start, b, Some(&Ty::uword())),
                    None => {
                        self.error(
                            ctx.module,
                            "E0200",
                            "`each` over a range needs both bounds: `a..b`",
                            iter.span,
                        );
                        (Expr::error(iter.span), Expr::error(iter.span))
                    }
                };
                let ty = a.ty.clone();
                (Iter::Range(a, b), ty, Regions::stat())
            }
            _ => {
                let v = self.check_expr(ctx, iter, None);
                let v = self.require_value(ctx, v);
                let elem = match &v.ty {
                    Ty::View { elem, .. } => (**elem).clone(),
                    Ty::Error => Ty::Error,
                    other => {
                        let t = self.type_text(other);
                        self.report(
                            ctx.module,
                            Diagnostic::error("E0200", format!("`each` needs a view or a range, found `{t}`"), v.span)
                                .with_note("iterate a view with `each x in v` or integers with `each i in a..b`"),
                        );
                        Ty::Error
                    }
                };
                let regions = v.regions.clone();
                (Iter::View(v), elem, regions)
            }
        };
        let before = ctx.flow.clone();
        Self::push_scope(ctx);
        let carries = self.type_carries_region(&elem_ty);
        let regions = if carries { regions } else { Regions::stat() };
        let id = self.declare_local(ctx, var, elem_ty, LocalKind::Binding, regions);
        ctx.loops.push(Vec::new());
        let body = self.check_block(ctx, body);
        ctx.loops.pop();
        Self::pop_scope(ctx);
        ctx.flow = before;
        StmtKind::Each {
            var: id,
            iter,
            body,
        }
    }

    fn check_ret(&mut self, ctx: &mut Ctx, value: Option<&oli_ast::Expr>, span: Span) -> StmtKind {
        let expected = ctx.ok_result();
        let out = match (value, &expected) {
            (None, None) => None,
            (_, Some(Ty::Never)) => {
                self.error(
                    ctx.module,
                    "E0200",
                    "a `-> never` procedure cannot `ret`",
                    span,
                );
                None
            }
            (None, Some(t)) => {
                let t = self.type_text(t);
                self.error(
                    ctx.module,
                    "E0200",
                    format!("`ret` needs a value of type `{t}`"),
                    span,
                );
                None
            }
            (Some(e), None) => {
                let v = self.check_expr(ctx, e, None);
                self.error(
                    ctx.module,
                    "E0200",
                    "this procedure has no result; `ret` takes no value",
                    v.span,
                );
                Some(v)
            }
            (Some(e), Some(t)) => {
                let t = t.clone();
                let v = self.check_expr(ctx, e, Some(&t));
                let v = self.require_value(ctx, v);
                self.check_escape_on_return(ctx, &v);
                Some(v)
            }
        };
        ctx.flow.diverge();
        StmtKind::Ret(out)
    }

    fn check_fail(&mut self, ctx: &mut Ctx, value: Option<&oli_ast::Expr>, span: Span) -> StmtKind {
        let out = match (value, ctx.fail_type()) {
            (_, None) => {
                self.report(
                    ctx.module,
                    Diagnostic::error("E0200", "`fail` in a procedure that cannot fail", span)
                        .with_note("declare the result as `-> T or E`"),
                );
                value.map(|e| self.check_expr(ctx, e, None))
            }
            (None, Some(Ty::None)) => None,
            (None, Some(t)) => {
                let t = self.type_text(&t);
                self.error(
                    ctx.module,
                    "E0200",
                    format!("`fail` needs a value of type `{t}`"),
                    span,
                );
                None
            }
            (Some(e), Some(t)) => {
                let v = self.check_expr(ctx, e, Some(&t));
                let v = self.require_value(ctx, v);
                self.check_escape_on_return(ctx, &v);
                Some(v)
            }
        };
        ctx.flow.diverge();
        StmtKind::Fail(out)
    }

    /// Rule 2: nothing pointing into the frame or a local zone may be returned.
    pub(crate) fn check_escape_on_return(&mut self, ctx: &Ctx, v: &Expr) {
        let local = v.regions.local_atoms();
        if let Some(r) = local.first() {
            let names = |id: LocalId| {
                ctx.locals
                    .get(id)
                    .map_or("?".to_string(), |l| l.name.clone())
            };
            let what = Regions::one(*r).describe(&names);
            self.report(
                ctx.module,
                Diagnostic::error("E0300", "view escapes destroyed memory", v.span)
                    .with_label(format!("points into {what}, which is released when this procedure returns"))
                    .with_note("return data that lives in a parameter's memory, a zone passed in by the caller, or static memory"),
            );
        }
    }

    fn check_zone(
        &mut self,
        ctx: &mut Ctx,
        name: &oli_ast::Ident,
        size: &oli_ast::Expr,
        source: Option<&oli_ast::ZoneSource>,
        body: &oli_ast::Block,
    ) -> StmtKind {
        let size = self.check_expr(ctx, size, Some(&Ty::uword()));
        let size = self.require_value(ctx, size);
        let source = match source {
            None => match ctx.zone_stack.last() {
                Some(parent) => ZoneSource::Parent(*parent),
                None => match self.target {
                    Target::Hosted => {
                        if !ctx.has_permit(Capability::OsSyscall) {
                            self.report(
                                ctx.module,
                                Diagnostic::error("E0401", "a top-level zone takes its memory from the OS and requires `permit os.syscall`", name.span)
                                    .with_note("or give the zone explicit memory: `zone NAME SIZE from HANDLE`"),
                            );
                        }
                        ZoneSource::Os
                    }
                    Target::Freestanding => {
                        self.report(
                            ctx.module,
                            Diagnostic::error("E0330", "zone has no memory source on a freestanding target", name.span)
                                .with_note("write `zone NAME SIZE at ADDR` or `zone NAME SIZE from HANDLE`; there is no OS to ask"),
                        );
                        ZoneSource::Os
                    }
                },
            },
            Some(oli_ast::ZoneSource::At(e)) => {
                let a = self.check_expr(ctx, e, Some(&Ty::Addr(Box::new(Ty::u8()))));
                let a = self.require_value(ctx, a);
                self.require_permit(
                    ctx,
                    Capability::MemoryRaw,
                    "placing a zone at a raw address",
                    a.span,
                );
                ZoneSource::At(a)
            }
            Some(oli_ast::ZoneSource::From(e)) => {
                let s = self.check_expr(ctx, e, None);
                let s = self.require_value(ctx, s);
                match &s.ty {
                    Ty::Zone => ZoneSource::FromZone(s),
                    Ty::View { rw: true, elem, .. }
                        if matches!(**elem, Ty::Int(crate::ty::IntTy::U8)) =>
                    {
                        ZoneSource::FromBuffer(s)
                    }
                    Ty::Error => ZoneSource::FromBuffer(s),
                    other => {
                        let t = self.type_text(other);
                        self.report(
                            ctx.module,
                            Diagnostic::error(
                                "E0200",
                                format!(
                                    "`from` needs a zone handle or a `rw view u8`, found `{t}`"
                                ),
                                s.span,
                            ),
                        );
                        ZoneSource::FromBuffer(s)
                    }
                }
            }
        };
        Self::push_scope(ctx);
        let handle =
            self.declare_local(ctx, name, Ty::Zone, LocalKind::ZoneHandle, Regions::stat());
        ctx.local_regions[handle] = Regions::one(Region::Zone(handle));
        ctx.zone_stack.push(handle);
        let body = self.check_block(ctx, body);
        ctx.zone_stack.pop();
        Self::pop_scope(ctx);
        StmtKind::Zone {
            handle,
            size,
            source,
            body,
        }
    }

    pub(crate) fn require_permit(&mut self, ctx: &Ctx, cap: Capability, what: &str, span: Span) {
        if !ctx.has_permit(cap) {
            let msg = format!("{what} requires `permit {}`", cap.as_str());
            self.report(
                ctx.module,
                Diagnostic::error("E0401", msg, span).with_note(format!(
                    "add the line `permit {}` under the procedure header",
                    cap.as_str()
                )),
            );
        }
    }

    fn check_case(
        &mut self,
        ctx: &mut Ctx,
        scrutinee: &oli_ast::Expr,
        arms: &[oli_ast::Arm],
        els: Option<&oli_ast::Block>,
    ) -> StmtKind {
        let scrut = self.check_expr(ctx, scrutinee, None);
        let scrut = self.require_value(ctx, scrut);
        // `case r` on a `ref T` inspects the referent.
        let scrut = match scrut.ty.clone() {
            Ty::Ref { elem, .. } => {
                let elem = (*elem).clone();
                self.coerce(ctx, scrut, &elem)
            }
            _ => scrut,
        };
        let ty = self.settle_untyped_error(ctx, scrut.ty.clone(), scrut.span);
        let regions = scrut.regions.clone();
        let entry = ctx.flow.clone();
        let mut merged: Option<Flow> = None;
        let mut cover = Cover::default();
        let mut out_arms = Vec::new();
        for arm in arms {
            ctx.flow = entry.clone();
            Self::push_scope(ctx);
            let pattern = self.check_pattern(ctx, &arm.pattern, &ty, &regions, &mut cover);
            let body = self.check_block(ctx, &arm.body);
            Self::pop_scope(ctx);
            merged = Some(match merged {
                Some(m) => Flow::merge(&m, &ctx.flow),
                None => ctx.flow.clone(),
            });
            out_arms.push(Arm {
                pattern,
                body,
                span: arm.span,
            });
        }
        let els_out = match els {
            Some(b) => {
                ctx.flow = entry.clone();
                let b = self.check_block(ctx, b);
                merged = Some(match merged {
                    Some(m) => Flow::merge(&m, &ctx.flow),
                    None => ctx.flow.clone(),
                });
                Some(b)
            }
            None => {
                if let Some(missing) = self.missing_coverage(&ty, &cover) {
                    self.report(
                        ctx.module,
                        Diagnostic::error("E0311", "`case` is not exhaustive", scrutinee.span)
                            .with_label(format!("not covered: {missing}"))
                            .with_note("add the missing arms or an `else` arm"),
                    );
                }
                None
            }
        };
        ctx.flow = merged.unwrap_or(entry);
        StmtKind::Case {
            scrutinee: scrut,
            arms: out_arms,
            els: els_out,
        }
    }

    /// What a `case` without `else` still needs to cover, if anything.
    fn missing_coverage(&mut self, ty: &Ty, cover: &Cover) -> Option<String> {
        if cover.all {
            return None;
        }
        match ty {
            Ty::Fallible(_, err) => {
                let mut missing = Vec::new();
                if !cover.ok {
                    missing.push("`ok`".to_string());
                }
                let fail_done = cover.fail_all
                    || match &**err {
                        Ty::Choice(id) => {
                            self.choice_ready(*id);
                            let n = self.prog.choices[*id].variants.len();
                            (0..n).all(|i| cover.variants.get(i).copied().unwrap_or(false))
                        }
                        _ => false,
                    };
                if !fail_done {
                    missing.push("`fail`".to_string());
                }
                if missing.is_empty() {
                    None
                } else {
                    Some(missing.join(", "))
                }
            }
            Ty::Choice(id) => {
                self.choice_ready(*id);
                let names: Vec<String> = self.prog.choices[*id]
                    .variants
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !cover.variants.get(*i).copied().unwrap_or(false))
                    .map(|(_, v)| format!("`{}`", v.name))
                    .collect();
                if names.is_empty() {
                    None
                } else {
                    Some(names.join(", "))
                }
            }
            Ty::Bool => {
                let mut m = Vec::new();
                if !cover.t {
                    m.push("`true`");
                }
                if !cover.f {
                    m.push("`false`");
                }
                if m.is_empty() {
                    None
                } else {
                    Some(m.join(", "))
                }
            }
            Ty::None | Ty::Error => None,
            _ => Some("the remaining values (integers need an `else` arm)".to_string()),
        }
    }

    fn check_pattern(
        &mut self,
        ctx: &mut Ctx,
        p: &oli_ast::Pattern,
        ty: &Ty,
        regions: &Regions,
        cover: &mut Cover,
    ) -> Pattern {
        match &p.kind {
            oli_ast::PatternKind::Ok(name) => {
                let Ty::Fallible(ok, _) = ty else {
                    if !ty.is_error() {
                        self.error(
                            ctx.module,
                            "E0200",
                            "`ok` pattern on a value that cannot fail",
                            p.span,
                        );
                    }
                    return Pattern::None;
                };
                if cover.ok {
                    self.warn(
                        ctx.module,
                        "W0003",
                        "this arm is unreachable: `ok` is already covered",
                        p.span,
                    );
                }
                cover.ok = true;
                let ok = (**ok).clone();
                let binding = name.as_ref().map(|n| {
                    let carries = self.type_carries_region(&ok);
                    let r = if carries {
                        regions.clone()
                    } else {
                        Regions::stat()
                    };
                    self.declare_local(ctx, n, ok.clone(), LocalKind::PatternBinding, r)
                });
                Pattern::Ok(binding)
            }
            oli_ast::PatternKind::Fail(sub) => {
                let Ty::Fallible(_, err) = ty else {
                    if !ty.is_error() {
                        self.error(
                            ctx.module,
                            "E0200",
                            "`fail` pattern on a value that cannot fail",
                            p.span,
                        );
                    }
                    return Pattern::None;
                };
                let err = (**err).clone();
                match sub {
                    None => {
                        if cover.fail_all {
                            self.warn(
                                ctx.module,
                                "W0003",
                                "this arm is unreachable: `fail` is already covered",
                                p.span,
                            );
                        }
                        cover.fail_all = true;
                        Pattern::Fail(None)
                    }
                    Some(sp) => {
                        if cover.fail_all {
                            self.warn(
                                ctx.module,
                                "W0003",
                                "this arm is unreachable: `fail` is already covered",
                                p.span,
                            );
                        }
                        let inner = self.check_sub_pattern(ctx, sp, &err, regions, cover);
                        Pattern::Fail(Some(Box::new(inner)))
                    }
                }
            }
            _ => self.check_sub_pattern(ctx, p, ty, regions, cover),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn variant_pattern(
        &mut self,
        ctx: &mut Ctx,
        id: crate::ty::ChoiceId,
        index: usize,
        name: &oli_ast::Ident,
        fields: &[oli_ast::Ident],
        regions: &Regions,
        cover: &mut Cover,
        span: Span,
    ) -> Pattern {
        if cover.variants.len() <= index {
            cover.variants.resize(index + 1, false);
        }
        if cover.variants[index] {
            let msg = format!(
                "this arm is unreachable: `{}` is already covered",
                name.name
            );
            self.warn(ctx.module, "W0003", msg, span);
        }
        cover.variants[index] = true;
        let mut bindings = Vec::new();
        for f in fields {
            let fdef = self.prog.choices[id].variants[index]
                .fields
                .iter()
                .position(|x| x.name == f.name);
            match fdef {
                Some(fi) => {
                    let fty = self.prog.choices[id].variants[index].fields[fi].ty.clone();
                    let carries = self.type_carries_region(&fty);
                    let r = if carries {
                        regions.clone()
                    } else {
                        Regions::stat()
                    };
                    let local = self.declare_local(ctx, f, fty, LocalKind::PatternBinding, r);
                    bindings.push((fi, local));
                }
                None => {
                    let msg = format!("variant `{}` has no field `{}`", name.name, f.name);
                    self.error(ctx.module, "E0208", msg, f.span);
                }
            }
        }
        Pattern::Variant {
            choice: id,
            index,
            bindings,
        }
    }

    fn check_sub_pattern(
        &mut self,
        ctx: &mut Ctx,
        p: &oli_ast::Pattern,
        ty: &Ty,
        regions: &Regions,
        cover: &mut Cover,
    ) -> Pattern {
        match &p.kind {
            oli_ast::PatternKind::Variant { name, fields } => {
                if let Ty::Choice(id) = ty {
                    self.choice_ready(*id);
                    let found = self.prog.choices[*id]
                        .variants
                        .iter()
                        .position(|v| v.name == name.name);
                    if let Some(index) = found {
                        if cover.all {
                            self.warn(
                                ctx.module,
                                "W0003",
                                "this arm is unreachable: everything is already covered",
                                p.span,
                            );
                        }
                        return self.variant_pattern(
                            ctx, *id, index, name, fields, regions, cover, p.span,
                        );
                    }
                    if !fields.is_empty() {
                        let cname = self.prog.choices[*id].name.clone();
                        let msg = format!("`{}` has no variant `{}`", cname, name.name);
                        self.error(ctx.module, "E0100", msg, name.span);
                        return Pattern::None;
                    }
                }
                if !fields.is_empty() {
                    if !ty.is_error() {
                        let t = self.type_text(ty);
                        let msg =
                            format!("a pattern with fields needs a choice value, found `{t}`");
                        self.error(ctx.module, "E0200", msg, p.span);
                    }
                    return Pattern::None;
                }
                // A bare name binds the whole value.
                if cover.all {
                    self.warn(
                        ctx.module,
                        "W0003",
                        "this arm is unreachable: everything is already covered",
                        p.span,
                    );
                }
                cover.all = true;
                let carries = self.type_carries_region(ty);
                let r = if carries {
                    regions.clone()
                } else {
                    Regions::stat()
                };
                let local = self.declare_local(ctx, name, ty.clone(), LocalKind::PatternBinding, r);
                Pattern::Bind(local)
            }
            oli_ast::PatternKind::Int { negative, value } => {
                let v =
                    i128::try_from(*value).map_or(i128::MAX, |v| if *negative { -v } else { v });
                match ty {
                    Ty::Int(i) => {
                        if !i.fits(v) {
                            let msg = format!("`{v}` does not fit in `{}`", i.as_str());
                            self.error(ctx.module, "E0202", msg, p.span);
                        }
                    }
                    Ty::Error => {}
                    other => {
                        let t = self.type_text(other);
                        self.error(
                            ctx.module,
                            "E0200",
                            format!("integer pattern on a value of type `{t}`"),
                            p.span,
                        );
                    }
                }
                Pattern::Int(v)
            }
            oli_ast::PatternKind::Char(c) => {
                if !matches!(ty, Ty::Int(crate::ty::IntTy::U8) | Ty::Error) {
                    let t = self.type_text(ty);
                    self.error(
                        ctx.module,
                        "E0200",
                        format!("character pattern on a value of type `{t}`"),
                        p.span,
                    );
                }
                Pattern::Int(i128::from(*c))
            }
            oli_ast::PatternKind::Bool(b) => {
                if !matches!(ty, Ty::Bool | Ty::Error) {
                    let t = self.type_text(ty);
                    self.error(
                        ctx.module,
                        "E0200",
                        format!("boolean pattern on a value of type `{t}`"),
                        p.span,
                    );
                }
                if *b {
                    cover.t = true;
                } else {
                    cover.f = true;
                }
                Pattern::Bool(*b)
            }
            oli_ast::PatternKind::None => {
                match ty {
                    Ty::None => cover.all = true,
                    Ty::Fallible(_, err) if matches!(**err, Ty::None) => cover.fail_all = true,
                    Ty::Error => {}
                    other => {
                        let t = self.type_text(other);
                        self.error(
                            ctx.module,
                            "E0200",
                            format!("`none` pattern on a value of type `{t}`"),
                            p.span,
                        );
                    }
                }
                Pattern::None
            }
            oli_ast::PatternKind::Ok(_) | oli_ast::PatternKind::Fail(_) => {
                self.error(
                    ctx.module,
                    "E0200",
                    "`ok`/`fail` patterns cannot nest",
                    p.span,
                );
                Pattern::None
            }
            oli_ast::PatternKind::Error => Pattern::None,
        }
    }
}

/// Which values the arms of a `case` cover so far.
#[derive(Default)]
pub(crate) struct Cover {
    pub ok: bool,
    pub fail_all: bool,
    pub all: bool,
    pub variants: Vec<bool>,
    pub t: bool,
    pub f: bool,
}

#[allow(dead_code)]
fn _symbol_used(_: Symbol) {}
