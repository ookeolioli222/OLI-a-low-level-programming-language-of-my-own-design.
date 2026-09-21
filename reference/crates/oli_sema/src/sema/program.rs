//! Whole-program rules: entry points, `traps`, symbol uniqueness.

use super::Sema;
use crate::hir::*;
use crate::ty::Ty;
use oli_diag::Diagnostic;
use std::collections::HashMap;

impl Sema<'_> {
    pub(crate) fn check_program_rules(&mut self) {
        self.check_symbols_unique();
        match self.target {
            Target::Hosted => self.check_hosted(),
            Target::Freestanding => self.check_freestanding(),
        }
        self.check_traps();
    }

    fn check_symbols_unique(&mut self) {
        let mut seen: HashMap<String, ProcId> = HashMap::new();
        for id in 0..self.prog.procs.len() {
            let sym = self.prog.procs[id].symbol.clone();
            if let Some(first) = seen.insert(sym.clone(), id) {
                if self.prog.procs[first].module == self.prog.procs[id].module {
                    continue; // already reported as a duplicate declaration
                }
                let (module, span) = (self.prog.procs[id].module, self.prog.procs[id].span);
                let first_span = self.prog.procs[first].span;
                self.report(
                    module,
                    Diagnostic::error(
                        "E0102",
                        format!("two procedures export the symbol `{sym}`"),
                        span,
                    )
                    .with_secondary(first_span, "first definition (possibly in another file)"),
                );
            }
        }
    }

    /// Design 0016: the procedure carrying `entry` starts the program on every
    /// target. Hosted: `-> s32` (the exit status), no parameters. `main` has no meaning.
    fn check_hosted(&mut self) {
        let root = 0;
        let entries: Vec<ProcId> = (0..self.prog.procs.len())
            .filter(|&i| self.prog.procs[i].is_entry)
            .collect();
        match entries.as_slice() {
            [] => {
                let span = oli_diag::Span::point(0);
                let mut d = Diagnostic::error(
                    "E0600",
                    "a hosted program needs a procedure with the `entry` clause",
                    span,
                )
                .with_note("write `proc start -> s32` with `entry` on the next line; its result is the exit status");
                if self
                    .prog
                    .procs
                    .iter()
                    .any(|p| p.module == root && p.name == "main")
                {
                    d = d.with_note(
                        "`main` has no special meaning in Oli--; add the `entry` clause to it",
                    );
                }
                self.report(root, d);
            }
            [id] => {
                let ok = self.prog.procs[*id].params.is_empty()
                    && matches!(
                        self.prog.procs[*id].result,
                        Some(Ty::Int(crate::ty::IntTy::S32))
                    );
                if !ok {
                    let (module, span) = (self.prog.procs[*id].module, self.prog.procs[*id].span);
                    self.report(
                        module,
                        Diagnostic::error(
                            "E0600",
                            "a hosted `entry` procedure is `-> s32` (the exit status) with no parameters",
                            span,
                        ),
                    );
                }
                self.prog.entry = Some(*id);
            }
            [first, rest @ ..] => {
                let first_span = self.prog.procs[*first].span;
                for id in rest {
                    let (module, span) = (self.prog.procs[*id].module, self.prog.procs[*id].span);
                    self.report(
                        module,
                        Diagnostic::error("E0602", "more than one `entry` procedure", span)
                            .with_secondary(first_span, "first `entry` here"),
                    );
                }
                self.prog.entry = Some(*first);
            }
        }
        for id in 0..self.prog.procs.len() {
            if self.prog.procs[id].is_traps {
                let (module, span) = (self.prog.procs[id].module, self.prog.procs[id].span);
                self.report(
                    module,
                    Diagnostic::error(
                        "E0601",
                        "`traps` is only allowed on a freestanding target",
                        span,
                    )
                    .with_note("hosted programs trap through the OS; see docs/FREESTANDING.md"),
                );
            }
        }
    }

    fn check_freestanding(&mut self) {
        let entries: Vec<ProcId> = (0..self.prog.procs.len())
            .filter(|&i| self.prog.procs[i].is_entry)
            .collect();
        match entries.as_slice() {
            [] => {
                let span = oli_diag::Span::point(0);
                self.report(
                    0,
                    Diagnostic::error("E0602", "a freestanding program needs exactly one procedure with the `entry` clause", span),
                );
            }
            [id] => {
                let p = &self.prog.procs[*id];
                if !p.params.is_empty() || !matches!(p.result, None | Some(Ty::Never)) {
                    let (module, span) = (p.module, p.span);
                    self.report(
                        module,
                        Diagnostic::error("E0603", "the `entry` procedure takes no parameters and returns nothing or `never`", span),
                    );
                }
                self.prog.entry = Some(*id);
            }
            [first, rest @ ..] => {
                let first_span = self.prog.procs[*first].span;
                for id in rest {
                    let (module, span) = (self.prog.procs[*id].module, self.prog.procs[*id].span);
                    self.report(
                        module,
                        Diagnostic::error("E0602", "more than one `entry` procedure", span)
                            .with_secondary(first_span, "first `entry` here"),
                    );
                }
                self.prog.entry = Some(*first);
            }
        }
    }

    fn check_traps(&mut self) {
        let traps: Vec<ProcId> = (0..self.prog.procs.len())
            .filter(|&i| self.prog.procs[i].is_traps)
            .collect();
        if traps.len() > 1 {
            let first_span = self.prog.procs[traps[0]].span;
            for id in traps.iter().skip(1) {
                let (module, span) = (self.prog.procs[*id].module, self.prog.procs[*id].span);
                self.report(
                    module,
                    Diagnostic::error("E0602", "more than one `traps` procedure", span)
                        .with_secondary(first_span, "first `traps` here"),
                );
            }
        }
        if let Some(&id) = traps.first() {
            if self.target == Target::Freestanding {
                self.check_traps_signature(id);
                self.prog.traps = Some(id);
            }
        }
    }

    fn check_traps_signature(&mut self, id: ProcId) {
        let p = &self.prog.procs[id];
        let (module, span) = (p.module, p.span);
        let kinds: Vec<Ty> = p.params.iter().map(|&i| p.locals[i].ty.clone()).collect();
        let core = self.core_module;
        let is_core_item = |name: &str, ty: &Ty, sema: &Sema| match ty {
            Ty::Choice(c) => {
                Some(sema.prog.choices[*c].module) == core && sema.prog.choices[*c].name == name
            }
            Ty::Layout(l) => {
                Some(sema.prog.layouts[*l].module) == core && sema.prog.layouts[*l].name == name
            }
            _ => false,
        };
        let ok = matches!(kinds.as_slice(), [k, s] if is_core_item("TrapKind", k, self) && is_core_item("Site", s, self))
            && matches!(p.result, Some(Ty::Never));
        if !ok {
            self.report(
                module,
                Diagnostic::error("E0603", "a `traps` procedure has the signature `(kind : core.TrapKind, site : core.Site) -> never`", span),
            );
        }
    }
}
