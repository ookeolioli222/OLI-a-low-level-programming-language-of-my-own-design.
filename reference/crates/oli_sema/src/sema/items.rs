//! Declaration collection, type resolution, layout/choice sizes, signatures.

use super::{ModuleScope, Sema, State, Symbol};
use crate::hir::*;
use crate::loader::LoadedModule;
use crate::ty::{ChoiceId, IntTy, LayoutId, Ty};
use oli_diag::{Diagnostic, Span};
use oli_lexer::Prim;
use std::collections::HashMap;

const RESERVED_TOP_LEVEL: &[&str] = &["cpu", "mem", "os", "core"];

impl Sema<'_> {
    pub(crate) fn collect_all(&mut self, modules: Vec<LoadedModule>) {
        for (id, m) in modules.into_iter().enumerate() {
            if m.name == "core" {
                self.core_module = Some(id);
            }
            self.prog.modules.push(ModuleDef {
                name: m.name.clone(),
                file: m.file,
            });
            let mut scope = ModuleScope {
                name: m.name.clone(),
                file: m.file,
                symbols: HashMap::new(),
                aliases: HashMap::new(),
            };
            for (alias, target, span) in &m.imports {
                if scope.aliases.contains_key(alias) {
                    let d = Diagnostic::error(
                        "E0102",
                        format!("duplicate import alias `{alias}`"),
                        *span,
                    );
                    self.diags.push(d.in_file(m.file));
                }
                scope.aliases.insert(alias.clone(), *target);
            }
            self.scopes.push(scope);
            if let Some(declared) = &m.ast.name {
                if id != 0 && declared.dotted() != m.name {
                    let msg = format!(
                        "module declares itself `{}` but is loaded as `{}`",
                        declared.dotted(),
                        m.name
                    );
                    self.diags.push(
                        Diagnostic::error("E0109", msg, declared.span)
                            .with_note("a module's name must match its import path (its file path under the library root)")
                            .in_file(m.file),
                    );
                }
            }
            for decl in m.ast.decls {
                self.collect_decl(id, decl);
            }
        }
    }

    fn declare(&mut self, module: ModuleId, name: &oli_ast::Ident, sym: Symbol) {
        if RESERVED_TOP_LEVEL.contains(&name.name.as_str()) {
            let msg = format!(
                "`{}` is a reserved namespace and cannot be declared",
                name.name
            );
            self.report(
                module,
                Diagnostic::error("E0103", msg, name.span).with_note(
                    "`cpu`, `mem`, `os` and `core` name intrinsics and the core library",
                ),
            );
        }
        if let Some((_, first)) = self.scopes[module].symbols.get(&name.name).copied() {
            let msg = format!("`{}` is declared twice in this module", name.name);
            self.report(
                module,
                Diagnostic::error("E0102", msg, name.span)
                    .with_secondary(first, "first declaration"),
            );
            return;
        }
        if self.scopes[module].aliases.contains_key(&name.name) {
            let msg = format!("`{}` collides with an import alias", name.name);
            self.error(module, "E0102", msg, name.span);
        }
        self.scopes[module]
            .symbols
            .insert(name.name.clone(), (sym, name.span));
    }

    fn collect_decl(&mut self, module: ModuleId, decl: oli_ast::Decl) {
        let is_pub = decl.is_pub;
        let span = decl.span;
        match decl.kind {
            oli_ast::DeclKind::Layout(l) => {
                let id = self.prog.layouts.len();
                self.declare(module, &l.name, Symbol::Layout(id));
                let name = l.name.name.clone();
                let packed = l.packed;
                self.prog.layouts.push(LayoutDef {
                    module,
                    name,
                    is_pub,
                    packed,
                    fields: Vec::new(),
                    size: 0,
                    align: 1,
                    span,
                });
                self.layout_asts.push((module, l));
                self.layout_state.push(State::Pending);
            }
            oli_ast::DeclKind::Choice(c) => {
                let id = self.prog.choices.len();
                self.declare(module, &c.name, Symbol::Choice(id));
                let name = c.name.name.clone();
                self.prog.choices.push(ChoiceDef {
                    module,
                    name,
                    is_pub,
                    variants: Vec::new(),
                    tag_bytes: 1,
                    payload_offset: 1,
                    size: 1,
                    align: 1,
                    span,
                });
                self.choice_asts.push((module, c));
                self.choice_state.push(State::Pending);
            }
            oli_ast::DeclKind::Proc(p) => {
                let id = self.prog.procs.len();
                self.declare(module, &p.name, Symbol::Proc(id));
                let symbol = format!("{}.{}", self.scopes[module].name, p.name.name);
                self.prog.procs.push(ProcDef {
                    module,
                    name: p.name.name.clone(),
                    symbol,
                    is_pub,
                    params: Vec::new(),
                    result: None,
                    permits: Vec::new(),
                    conv: CallConv::Sysv,
                    section: None,
                    align: None,
                    is_entry: false,
                    export: None,
                    is_traps: false,
                    locals: Vec::new(),
                    body: Block::default(),
                    span,
                });
                self.proc_asts.push((module, p));
            }
            oli_ast::DeclKind::Const { ref name, .. } => {
                let id = self.prog.consts.len();
                self.declare(module, name, Symbol::Const(id));
                let name = name.name.clone();
                self.prog.consts.push(ConstDef {
                    module,
                    name,
                    is_pub,
                    ty: Ty::Error,
                    value: ConstValue::None,
                    span,
                });
                self.const_asts.push((module, decl));
                self.const_state.push(State::Pending);
                self.const_backing.push(None);
            }
            oli_ast::DeclKind::Static { ref name, .. } => {
                let id = self.prog.statics.len();
                self.declare(module, name, Symbol::Static(id));
                let name = name.name.clone();
                self.prog.statics.push(StaticDef {
                    module,
                    name,
                    is_pub,
                    ty: Ty::Error,
                    init: None,
                    readonly: false,
                    section: None,
                    align: None,
                    span,
                });
                self.static_asts.push(Some((module, decl)));
                self.static_state.push(State::Pending);
            }
        }
    }

    // ----------------------------------------------------------- types

    pub(crate) fn prim_ty(&mut self, module: ModuleId, p: Prim, span: Span) -> Ty {
        match p {
            Prim::U8 | Prim::Byte => Ty::Int(IntTy::U8),
            Prim::U16 => Ty::Int(IntTy::U16),
            Prim::U32 => Ty::Int(IntTy::U32),
            Prim::U64 => Ty::Int(IntTy::U64),
            Prim::S8 => Ty::Int(IntTy::S8),
            Prim::S16 => Ty::Int(IntTy::S16),
            Prim::S32 => Ty::Int(IntTy::S32),
            Prim::S64 => Ty::Int(IntTy::S64),
            Prim::Word => Ty::Int(IntTy::Word),
            Prim::Uword => Ty::Int(IntTy::Uword),
            Prim::Bool => Ty::Bool,
            Prim::Physaddr => Ty::Physaddr,
            Prim::F32 | Prim::F64 => {
                self.not_implemented(module, "floating-point types", span);
                Ty::Error
            }
        }
    }

    pub(crate) fn not_implemented(&mut self, module: ModuleId, what: &str, span: Span) {
        self.report(
            module,
            Diagnostic::error("E0900", format!("feature not implemented: {what}"), span)
                .with_note("see spec/OLI_SEMANTICS_V0.md §13 for what V0 leaves out"),
        );
    }

    /// Resolves a syntactic type in the scope of `module`.
    pub(crate) fn resolve_type(&mut self, module: ModuleId, t: &oli_ast::Type) -> Ty {
        match &t.kind {
            oli_ast::TypeKind::Prim(p) => self.prim_ty(module, *p, t.span),
            oli_ast::TypeKind::Named(path) => self.resolve_named_type(module, path),
            oli_ast::TypeKind::Array { len, elem } => {
                let n = self.array_len(module, len);
                let elem = self.resolve_type(module, elem);
                match n {
                    Some(n) => Ty::Array(n, Box::new(elem)),
                    None => Ty::Error,
                }
            }
            oli_ast::TypeKind::View { mmio, rw, elem } => Ty::View {
                mmio: *mmio,
                rw: *rw,
                elem: Box::new(self.resolve_type(module, elem)),
            },
            oli_ast::TypeKind::Ref { mmio, rw, elem } => Ty::Ref {
                mmio: *mmio,
                rw: *rw,
                elem: Box::new(self.resolve_type(module, elem)),
            },
            oli_ast::TypeKind::Addr(None) => Ty::Addr(Box::new(Ty::u8())),
            oli_ast::TypeKind::Addr(Some(e)) => Ty::Addr(Box::new(self.resolve_type(module, e))),
            oli_ast::TypeKind::Own(_) => {
                self.not_implemented(module, "`own` values", t.span);
                Ty::Error
            }
            oli_ast::TypeKind::Port(p) => match self.prim_ty(module, *p, t.span) {
                Ty::Int(i) => Ty::Port(i),
                _ => Ty::Error,
            },
            oli_ast::TypeKind::Endian { big, prim } => match self.prim_ty(module, *prim, t.span) {
                Ty::Int(i) => Ty::Endian { big: *big, int: i },
                _ => Ty::Error,
            },
            oli_ast::TypeKind::Zone => Ty::Zone,
            oli_ast::TypeKind::Fallible { ok, err } => {
                let ok = self.resolve_type(module, ok);
                let err = self.resolve_type(module, err);
                Ty::Fallible(Box::new(ok), Box::new(err))
            }
            oli_ast::TypeKind::None => Ty::None,
            oli_ast::TypeKind::Never => Ty::Never,
        }
    }

    fn resolve_named_type(&mut self, module: ModuleId, path: &oli_ast::Path) -> Ty {
        match self.resolve_path_symbol(module, path) {
            Some(Symbol::Layout(id)) => Ty::Layout(id),
            Some(Symbol::Choice(id)) => Ty::Choice(id),
            Some(_) => {
                let msg = format!("`{}` is not a type", path.dotted());
                self.error(module, "E0013", msg, path.span);
                Ty::Error
            }
            None => Ty::Error,
        }
    }

    /// Resolves `a`, `alias.name` or `core.name` to a symbol, reporting failures.
    pub(crate) fn resolve_path_symbol(
        &mut self,
        module: ModuleId,
        path: &oli_ast::Path,
    ) -> Option<Symbol> {
        let mut segs = path.segments.iter();
        let first = segs.next()?;
        let mut sym = match self.lookup_module_symbol(module, &first.name) {
            Some(s) => s,
            None => {
                self.unknown_name(module, &first.name, first.span);
                return None;
            }
        };
        for seg in segs {
            match sym {
                Symbol::Module(target) => {
                    sym = self.module_member(target, &seg.name, module, seg.span)?;
                }
                _ => {
                    let msg = format!("`{}` has no member `{}`", first.name, seg.name);
                    self.error(module, "E0105", msg, seg.span);
                    return None;
                }
            }
        }
        Some(sym)
    }

    pub(crate) fn unknown_name(&mut self, module: ModuleId, name: &str, span: Span) {
        let mut d = Diagnostic::error("E0100", format!("unknown name `{name}`"), span);
        if self.scopes[module]
            .aliases
            .get(name)
            .is_some_and(Option::is_none)
        {
            d = d.with_note("the module this alias refers to could not be loaded");
        }
        self.report(module, d);
    }

    fn array_len(&mut self, module: ModuleId, len: &oli_ast::Expr) -> Option<u64> {
        let value = match &len.kind {
            oli_ast::ExprKind::Int(v) => i128::try_from(*v).ok(),
            oli_ast::ExprKind::Name(id) => match self.lookup_module_symbol(module, &id.name) {
                Some(Symbol::Const(c)) => {
                    self.const_ready(c);
                    match (&self.prog.consts[c].ty, &self.prog.consts[c].value) {
                        (Ty::UntypedInt | Ty::Int(_), ConstValue::Int(v)) => Some(*v),
                        _ => None,
                    }
                }
                Some(_) => None,
                None => {
                    self.unknown_name(module, &id.name, id.span);
                    return None;
                }
            },
            _ => None,
        };
        match value {
            Some(v) if (0..=i128::from(u32::MAX)).contains(&v) => Some(v as u64),
            Some(_) => {
                self.error(
                    module,
                    "E0013",
                    "array length is out of range (0 .. 2^32)",
                    len.span,
                );
                None
            }
            None => {
                self.error(
                    module,
                    "E0013",
                    "array length must be an integer literal or an integer constant",
                    len.span,
                );
                None
            }
        }
    }

    // -------------------------------------------------- sizes and layouts

    fn align_up(x: u64, a: u64) -> u64 {
        if a <= 1 {
            x
        } else {
            x.div_ceil(a) * a
        }
    }

    /// `(size, align)` of a type. Layouts and choices are resolved on demand.
    pub(crate) fn size_align(&mut self, t: &Ty) -> (u64, u64) {
        match t {
            Ty::Int(i) => (i.bytes(), i.bytes()),
            Ty::Bool => (1, 1),
            Ty::Physaddr | Ty::Addr(_) | Ty::Ref { .. } | Ty::Zone => (8, 8),
            Ty::View { .. } => (16, 8),
            Ty::Port(_) => (2, 2),
            Ty::Endian { int, .. } => (int.bytes(), int.bytes()),
            Ty::Array(n, e) => {
                let (s, a) = self.size_align(e);
                (s.saturating_mul(*n), a)
            }
            Ty::Layout(id) => {
                self.layout_ready(*id);
                (self.prog.layouts[*id].size, self.prog.layouts[*id].align)
            }
            Ty::Choice(id) => {
                self.choice_ready(*id);
                (self.prog.choices[*id].size, self.prog.choices[*id].align)
            }
            Ty::Fallible(ok, err) => {
                let (s1, a1) = self.size_align(ok);
                let (s2, a2) = self.size_align(err);
                let align = a1.max(a2).max(1);
                let payload = Self::align_up(1, align);
                (Self::align_up(payload + s1.max(s2), align), align)
            }
            _ => (0, 1),
        }
    }

    /// Lays fields out in declaration order (`docs/ABI.md` §3).
    fn lay_out(
        &mut self,
        module: ModuleId,
        fields: &[oli_ast::Field],
        packed: bool,
        base: u64,
    ) -> (Vec<FieldDef>, u64, u64) {
        let mut out = Vec::new();
        let mut offset = base;
        let mut max_align = 1u64;
        for f in fields {
            let ty = self.resolve_type(module, &f.ty);
            let (size, mut align) = self.size_align(&ty);
            if packed {
                align = 1;
            }
            if let Some(a) = f.align {
                align = align.max(u64::try_from(a).unwrap_or(u64::MAX));
            }
            offset = Self::align_up(offset, align);
            max_align = max_align.max(align);
            out.push(FieldDef {
                name: f.name.name.clone(),
                ty,
                offset,
                span: f.span,
            });
            offset = offset.saturating_add(size);
        }
        (out, offset, max_align)
    }

    pub(crate) fn layout_ready(&mut self, id: LayoutId) {
        match self.layout_state[id] {
            State::Done => return,
            State::InProgress => {
                let (module, span, name) = (
                    self.prog.layouts[id].module,
                    self.prog.layouts[id].span,
                    self.prog.layouts[id].name.clone(),
                );
                self.report(
                    module,
                    Diagnostic::error("E0204", format!("layout `{name}` contains itself"), span)
                        .with_note("a layout cannot hold a field of its own type by value; use `ref` or `addr`"),
                );
                self.layout_state[id] = State::Done;
                return;
            }
            State::Pending => {}
        }
        self.layout_state[id] = State::InProgress;
        let (module, ast) = self.layout_asts[id].clone();
        // Duplicate field names.
        let mut seen: HashMap<String, Span> = HashMap::new();
        for f in &ast.fields {
            if let Some(first) = seen.insert(f.name.name.clone(), f.name.span) {
                let msg = format!("field `{}` is declared twice", f.name.name);
                self.report(
                    module,
                    Diagnostic::error("E0102", msg, f.name.span)
                        .with_secondary(first, "first declaration"),
                );
            }
        }
        let (fields, end, mut align) = self.lay_out(module, &ast.fields, ast.packed, 0);
        if let Some(a) = ast.align {
            align = align.max(u64::try_from(a).unwrap_or(u64::MAX));
        }
        let size = Self::align_up(end, align);
        let def = &mut self.prog.layouts[id];
        def.fields = fields;
        def.size = size;
        def.align = align;
        self.layout_state[id] = State::Done;
    }

    pub(crate) fn choice_ready(&mut self, id: ChoiceId) {
        match self.choice_state[id] {
            State::Done => return,
            State::InProgress => {
                let (module, span, name) = (
                    self.prog.choices[id].module,
                    self.prog.choices[id].span,
                    self.prog.choices[id].name.clone(),
                );
                self.error(
                    module,
                    "E0204",
                    format!("choice `{name}` contains itself"),
                    span,
                );
                self.choice_state[id] = State::Done;
                return;
            }
            State::Pending => {}
        }
        self.choice_state[id] = State::InProgress;
        let (module, ast) = self.choice_asts[id].clone();
        let count = ast.variants.len() as u64;
        let tag_bytes: u64 = if count <= 256 {
            1
        } else if count <= 65536 {
            2
        } else {
            4
        };
        // First pass: payload alignment.
        let mut payload_align = 1u64;
        let mut seen: HashMap<String, Span> = HashMap::new();
        for v in &ast.variants {
            if let Some(first) = seen.insert(v.name.name.clone(), v.name.span) {
                let msg = format!("variant `{}` is declared twice", v.name.name);
                self.report(
                    module,
                    Diagnostic::error("E0102", msg, v.name.span)
                        .with_secondary(first, "first declaration"),
                );
            }
            for f in &v.fields {
                let ty = self.resolve_type(module, &f.ty);
                let (_, a) = self.size_align(&ty);
                payload_align = payload_align.max(a);
            }
        }
        let payload_offset = Self::align_up(tag_bytes, payload_align);
        let mut variants = Vec::new();
        let mut end = payload_offset;
        for v in &ast.variants {
            let (fields, vend, _) = self.lay_out(module, &v.fields, false, payload_offset);
            end = end.max(vend);
            variants.push(VariantDef {
                name: v.name.name.clone(),
                fields,
                span: v.span,
            });
        }
        let align = payload_align.max(tag_bytes);
        let size = Self::align_up(end, align);
        let def = &mut self.prog.choices[id];
        def.variants = variants;
        def.tag_bytes = tag_bytes;
        def.payload_offset = payload_offset;
        def.size = size;
        def.align = align;
        self.choice_state[id] = State::Done;
    }

    // ------------------------------------------------------- signatures

    pub(crate) fn resolve_signatures(&mut self) {
        for id in 0..self.prog.procs.len() {
            self.resolve_proc_signature(id);
        }
    }

    fn resolve_proc_signature(&mut self, id: ProcId) {
        let (module, ast) = self.proc_asts[id].clone();
        let mut params = Vec::new();
        let mut locals = Vec::new();
        let mut seen: HashMap<String, Span> = HashMap::new();
        for (i, p) in ast.params.iter().enumerate() {
            if let Some(first) = seen.insert(p.name.name.clone(), p.name.span) {
                let msg = format!("parameter `{}` is declared twice", p.name.name);
                self.report(
                    module,
                    Diagnostic::error("E0102", msg, p.name.span)
                        .with_secondary(first, "first declaration"),
                );
            }
            let ty = self.resolve_type(module, &p.ty);
            if let Ty::Endian { .. } = ty {
                self.error(
                    module,
                    "E0200",
                    "`be`/`le` integers are layout field types only",
                    p.ty.span,
                );
            }
            if let Ty::Array(..) = ty {
                self.report(
                    module,
                    Diagnostic::error("E0200", "arrays are not value types", p.ty.span)
                        .with_note("pass a `view T` (the array converts to one at the call site)"),
                );
            }
            locals.push(LocalDef {
                name: p.name.name.clone(),
                ty,
                kind: LocalKind::Param(i),
                scope_zone: None,
                parent_zone: None,
                span: p.name.span,
            });
            params.push(i);
        }
        let result = ast.result.as_ref().map(|t| self.resolve_type(module, t));
        let mut permits = Vec::new();
        let mut conv = CallConv::Sysv;
        let mut section = None;
        let mut align = None;
        let mut is_entry = false;
        let mut export = None;
        let mut is_traps = false;
        for c in &ast.clauses {
            match &c.kind {
                oli_ast::ClauseKind::Permit(caps) => {
                    for cap in caps {
                        let name = cap.dotted();
                        match Capability::from_path(&name) {
                            Some(k) => {
                                if !permits.contains(&k) {
                                    permits.push(k);
                                }
                                self.check_permit_on_target(module, k, cap.span);
                            }
                            None => {
                                self.report(
                                    module,
                                    Diagnostic::error("E0402", format!("unknown capability `{name}`"), cap.span).with_note(
                                        "capabilities: memory.raw, memory.mmio, io.port, cpu.asm, cpu.halt, cpu.interrupt, cpu.msr, cpu.control, os.syscall",
                                    ),
                                );
                            }
                        }
                    }
                }
                oli_ast::ClauseKind::Calls(cc) => match cc {
                    oli_ast::CallConv::Sysv | oli_ast::CallConv::C => conv = CallConv::Sysv,
                    oli_ast::CallConv::None => conv = CallConv::None,
                    oli_ast::CallConv::Interrupt => {
                        self.not_implemented(module, "`calls interrupt`", c.span)
                    }
                },
                oli_ast::ClauseKind::Section(s) => section = Some(s.clone()),
                oli_ast::ClauseKind::Align(a) => align = u64::try_from(*a).ok(),
                oli_ast::ClauseKind::Entry => is_entry = true,
                oli_ast::ClauseKind::Export(name) => {
                    export = Some(match name {
                        Some(s) => String::from_utf8_lossy(s).into_owned(),
                        None => ast.name.name.clone(),
                    });
                }
                oli_ast::ClauseKind::Traps => is_traps = true,
            }
        }
        if conv == CallConv::None {
            if !ast.params.is_empty() {
                self.error(
                    module,
                    "E0603",
                    "a `calls none` procedure cannot have parameters",
                    ast.name.span,
                );
            }
            if !matches!(result, None | Some(Ty::Never)) {
                self.error(
                    module,
                    "E0603",
                    "a `calls none` procedure returns nothing or `never`",
                    ast.name.span,
                );
            }
        }
        let def = &mut self.prog.procs[id];
        def.params = params;
        def.locals = locals;
        def.result = result;
        def.permits = permits;
        def.conv = conv;
        def.section = section;
        def.align = align;
        def.is_entry = is_entry;
        if let Some(e) = &export {
            def.symbol = e.clone();
        }
        def.export = export;
        def.is_traps = is_traps;
    }

    fn check_permit_on_target(&mut self, module: ModuleId, cap: Capability, span: Span) {
        match self.target {
            Target::Hosted if cap.is_privileged() => {
                let msg = format!("`{}` faults in user mode on a hosted target", cap.as_str());
                self.warn(module, "W0100", msg, span);
            }
            Target::Freestanding if cap == Capability::OsSyscall => {
                self.report(
                    module,
                    Diagnostic::error(
                        "E0400",
                        "`os.syscall` is not available on a freestanding target",
                        span,
                    )
                    .with_note("there is no operating system to call; see docs/FREESTANDING.md"),
                );
            }
            _ => {}
        }
    }
}
