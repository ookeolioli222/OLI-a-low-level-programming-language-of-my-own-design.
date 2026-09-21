//! The checker. `check_program` runs the passes in order:
//! collect declarations → resolve layouts/choices → resolve signatures →
//! evaluate constants and static initializers → check bodies → program rules.

mod body;
mod consts;
mod expr;
mod items;
mod machine;
mod program;

use crate::hir::*;
use crate::loader::LoadedModule;
use crate::ty::{ChoiceId, LayoutId, Ty, TyPrinter};
use crate::Options;
use oli_diag::{Diagnostic, Diagnostics, Span};
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Symbol {
    Proc(ProcId),
    Const(ConstId),
    Static(StaticId),
    Layout(LayoutId),
    Choice(ChoiceId),
    /// An import alias.
    Module(ModuleId),
}

/// The intrinsic namespaces that are always in scope.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Namespace {
    Os,
    Cpu,
    Mem,
}

impl Namespace {
    pub(crate) fn from_name(s: &str) -> Option<Namespace> {
        match s {
            "os" => Some(Namespace::Os),
            "cpu" => Some(Namespace::Cpu),
            "mem" => Some(Namespace::Mem),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Namespace::Os => "os",
            Namespace::Cpu => "cpu",
            Namespace::Mem => "mem",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Pending,
    InProgress,
    Done,
}

pub(crate) struct ModuleScope {
    pub name: String,
    pub file: u32,
    pub symbols: HashMap<String, (Symbol, Span)>,
    /// import alias → module (None when the module failed to load)
    pub aliases: HashMap<String, Option<ModuleId>>,
}

pub(crate) struct Sema<'a> {
    pub diags: &'a mut Diagnostics,
    pub target: Target,
    pub prog: Program,
    pub scopes: Vec<ModuleScope>,
    /// Index of the `core` module when it was loaded.
    pub core_module: Option<ModuleId>,
    pub layout_asts: Vec<(ModuleId, oli_ast::LayoutDecl)>,
    pub choice_asts: Vec<(ModuleId, oli_ast::ChoiceDecl)>,
    pub const_asts: Vec<(ModuleId, oli_ast::Decl)>,
    /// `None` for the backing statics of aggregate constants.
    pub static_asts: Vec<Option<(ModuleId, oli_ast::Decl)>>,
    pub proc_asts: Vec<(ModuleId, oli_ast::ProcDecl)>,
    pub layout_state: Vec<State>,
    pub choice_state: Vec<State>,
    pub const_state: Vec<State>,
    pub static_state: Vec<State>,
    /// Aggregate constants live in `.rodata`: the read-only static holding each.
    pub const_backing: Vec<Option<StaticId>>,
}

pub(crate) fn check_program(
    modules: Vec<LoadedModule>,
    opts: &Options,
    diags: &mut Diagnostics,
) -> Program {
    let mut sema = Sema {
        diags,
        target: opts.target,
        prog: Program::default(),
        scopes: Vec::new(),
        core_module: None,
        layout_asts: Vec::new(),
        choice_asts: Vec::new(),
        const_asts: Vec::new(),
        static_asts: Vec::new(),
        proc_asts: Vec::new(),
        layout_state: Vec::new(),
        choice_state: Vec::new(),
        const_state: Vec::new(),
        static_state: Vec::new(),
        const_backing: Vec::new(),
    };
    sema.collect_all(modules);
    for id in 0..sema.prog.layouts.len() {
        sema.layout_ready(id);
    }
    for id in 0..sema.prog.choices.len() {
        sema.choice_ready(id);
    }
    sema.resolve_signatures();
    for id in 0..sema.prog.consts.len() {
        sema.const_ready(id);
    }
    for id in 0..sema.prog.statics.len() {
        sema.static_ready(id);
    }
    for id in 0..sema.prog.procs.len() {
        sema.check_proc(id);
    }
    sema.check_program_rules();
    sema.prog
}

impl Sema<'_> {
    // ------------------------------------------------------- diagnostics

    pub(crate) fn file_of(&self, module: ModuleId) -> u32 {
        self.scopes[module].file
    }

    pub(crate) fn error(
        &mut self,
        module: ModuleId,
        code: &'static str,
        msg: impl Into<String>,
        span: Span,
    ) {
        let file = self.file_of(module);
        self.diags
            .push(Diagnostic::error(code, msg, span).in_file(file));
    }

    pub(crate) fn report(&mut self, module: ModuleId, d: Diagnostic) {
        let file = self.file_of(module);
        self.diags.push(d.in_file(file));
    }

    pub(crate) fn warn(
        &mut self,
        module: ModuleId,
        code: &'static str,
        msg: impl Into<String>,
        span: Span,
    ) {
        let file = self.file_of(module);
        self.diags
            .push(Diagnostic::warning(code, msg, span).in_file(file));
    }

    pub(crate) fn type_text(&self, t: &Ty) -> String {
        let layouts = &self.prog.layouts;
        let choices = &self.prog.choices;
        let ln = |id: LayoutId| layouts.get(id).map_or("?".to_string(), |l| l.name.clone());
        let cn = |id: ChoiceId| choices.get(id).map_or("?".to_string(), |c| c.name.clone());
        TyPrinter {
            layout_name: &ln,
            choice_name: &cn,
        }
        .text(t)
    }

    // ------------------------------------------------------------ lookup

    /// Looks a name up at module level: declarations, then import aliases.
    pub(crate) fn lookup_module_symbol(&self, module: ModuleId, name: &str) -> Option<Symbol> {
        let scope = &self.scopes[module];
        if let Some((sym, _)) = scope.symbols.get(name) {
            return Some(*sym);
        }
        if let Some(target) = scope.aliases.get(name) {
            return target.map(Symbol::Module);
        }
        if name == "core" {
            return self.core_module.map(Symbol::Module);
        }
        // A module may name itself (`call kernel.main` inside module `kernel`).
        if scope.name == name {
            return Some(Symbol::Module(module));
        }
        None
    }

    /// A public member of another module; `from` is the requesting module.
    pub(crate) fn module_member(
        &mut self,
        target: ModuleId,
        name: &str,
        from: ModuleId,
        span: Span,
    ) -> Option<Symbol> {
        let Some((sym, decl_span)) = self.scopes[target].symbols.get(name).copied() else {
            let msg = format!(
                "module `{}` has no member `{name}`",
                self.scopes[target].name
            );
            self.error(from, "E0105", msg, span);
            return None;
        };
        let is_pub = match sym {
            Symbol::Proc(id) => self.prog.procs[id].is_pub,
            Symbol::Const(id) => self.prog.consts[id].is_pub,
            Symbol::Static(id) => self.prog.statics[id].is_pub,
            Symbol::Layout(id) => self.prog.layouts[id].is_pub,
            Symbol::Choice(id) => self.prog.choices[id].is_pub,
            Symbol::Module(_) => false,
        };
        if !is_pub && target != from {
            let modname = self.scopes[target].name.clone();
            let file = self.file_of(target);
            self.report(
                from,
                Diagnostic::error(
                    "E0105",
                    format!("`{name}` in module `{modname}` is not public"),
                    span,
                )
                .with_note(format!(
                    "declare it with `pub` in {}",
                    self.scopes[target].name
                ))
                .with_secondary(decl_span, "declared here (in another file)"),
            );
            let _ = file;
            return None;
        }
        Some(sym)
    }

    pub(crate) fn alias_target(&self, module: ModuleId, alias: &str) -> Option<ModuleId> {
        self.scopes[module].aliases.get(alias).copied().flatten()
    }
}
