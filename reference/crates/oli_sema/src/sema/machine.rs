//! `machine x64 ... end` blocks (`spec/OLI_SEMANTICS_V0.md` §8): interface
//! directives are typed, symbols are resolved, the rest is left to the encoder.

use super::body::Ctx;
use super::{Sema, Symbol};
use crate::hir::*;
use crate::ty::{IntTy, Ty};
use oli_diag::Diagnostic;

#[rustfmt::skip]
const REGS64: &[&str] = &["rax","rbx","rcx","rdx","rsi","rdi","rbp","rsp","r8","r9","r10","r11","r12","r13","r14","r15","rip","cr0","cr2","cr3","cr4","cr8"];
#[rustfmt::skip]
const REGS32: &[&str] = &["eax","ebx","ecx","edx","esi","edi","ebp","esp","r8d","r9d","r10d","r11d","r12d","r13d","r14d","r15d"];
#[rustfmt::skip]
const REGS16: &[&str] = &["ax","bx","cx","dx","si","di","bp","sp","r8w","r9w","r10w","r11w","r12w","r13w","r14w","r15w","cs","ds","es","fs","gs","ss"];
#[rustfmt::skip]
const REGS8: &[&str] = &["al","bl","cl","dl","sil","dil","bpl","spl","r8b","r9b","r10b","r11b","r12b","r13b","r14b","r15b","ah","bh","ch","dh"];

/// Width of a general register name, if it is one.
pub(crate) fn register_width(name: &str) -> Option<IntTy> {
    if REGS64.contains(&name) {
        Some(IntTy::U64)
    } else if REGS32.contains(&name) {
        Some(IntTy::U32)
    } else if REGS16.contains(&name) {
        Some(IntTy::U16)
    } else if REGS8.contains(&name) {
        Some(IntTy::U8)
    } else {
        None
    }
}

/// Mnemonics that own the frame: only `calls none` procedures may use them.
const FRAME_OWNERS: &[&str] = &[
    "ret", "retq", "retf", "retfq", "iret", "iretq", "syscall", "sysret", "sysretq", "call",
];

impl Sema<'_> {
    pub(crate) fn check_machine(&mut self, ctx: &mut Ctx, m: &oli_ast::MachineBlock) -> Machine {
        self.require_permit(ctx, Capability::CpuAsm, "a `machine` block", m.span);
        if m.arch.name != "x64" {
            let msg = format!(
                "unknown machine architecture `{}`; V0 supports `x64`",
                m.arch.name
            );
            self.error(ctx.module, "E0500", msg, m.arch.span);
        }
        let mut lines = Vec::new();
        for line in &m.lines {
            match line {
                oli_ast::MachineLine::In { reg, value } => {
                    let width = self.expect_register(ctx, reg);
                    let expected = width.map(Ty::Int);
                    let v = self.check_expr(ctx, value, expected.as_ref());
                    let v = self.require_value(ctx, v);
                    lines.push(MachineLine::In {
                        reg: reg.name.clone(),
                        value: v,
                    });
                }
                oli_ast::MachineLine::Out { reg, target } => {
                    let width = self.expect_register(ctx, reg);
                    let Some(place) = self.check_place(ctx, target, true) else {
                        continue;
                    };
                    self.check_out_width(ctx, reg, width, &place, target.span);
                    if let PlaceKind::Local(id) = place.kind {
                        ctx.flow.set_assigned(id);
                    }
                    lines.push(MachineLine::Out {
                        reg: reg.name.clone(),
                        target: place,
                    });
                }
                oli_ast::MachineLine::Clobber(list) => {
                    let mut regs = Vec::new();
                    let mut memory = false;
                    for c in list {
                        match c {
                            oli_ast::Clobber::Memory(_) => memory = true,
                            oli_ast::Clobber::Reg(r) => {
                                self.expect_register(ctx, r);
                                regs.push(r.name.clone());
                            }
                        }
                    }
                    lines.push(MachineLine::Clobber { regs, memory });
                }
                oli_ast::MachineLine::Label(l) => lines.push(MachineLine::Label(l.name.clone())),
                oli_ast::MachineLine::Instr {
                    mnemonic,
                    operands,
                    span,
                } => {
                    if ctx.conv != CallConv::None && FRAME_OWNERS.contains(&mnemonic.name.as_str())
                    {
                        let msg = format!(
                            "`{}` is only allowed in a `calls none` procedure",
                            mnemonic.name
                        );
                        self.report(
                            ctx.module,
                            Diagnostic::error("E0500", msg, mnemonic.span)
                                .with_note("inside an ordinary procedure the compiler owns the stack frame and the return"),
                        );
                    }
                    let ops: Vec<Operand> = operands
                        .iter()
                        .map(|o| self.check_operand(ctx, o))
                        .collect();
                    lines.push(MachineLine::Instr {
                        mnemonic: mnemonic.name.clone(),
                        operands: ops,
                        span: *span,
                    });
                }
            }
        }
        Machine {
            arch: m.arch.name.clone(),
            lines,
            span: m.span,
        }
    }

    fn check_out_width(
        &mut self,
        ctx: &Ctx,
        reg: &oli_ast::Ident,
        width: Option<IntTy>,
        place: &Place,
        span: oli_diag::Span,
    ) {
        match (width, super::expr::value_ty(&place.ty).as_int()) {
            (Some(w), Some(t)) if w.bits() != t.bits() => {
                let msg = format!(
                    "register `{}` is {} bits but `{}` needs {}",
                    reg.name,
                    w.bits(),
                    t.as_str(),
                    t.bits()
                );
                self.error(ctx.module, "E0200", msg, span);
            }
            (Some(_), None) if !place.ty.is_error() => {
                let t = self.type_text(&place.ty);
                let msg =
                    format!("`out` stores an integer register; `{t}` is not an integer place");
                self.error(ctx.module, "E0200", msg, span);
            }
            _ => {}
        }
    }

    fn expect_register(&mut self, ctx: &Ctx, reg: &oli_ast::Ident) -> Option<IntTy> {
        let w = register_width(&reg.name);
        if w.is_none() {
            self.error(
                ctx.module,
                "E0500",
                format!("`{}` is not a register", reg.name),
                reg.span,
            );
        }
        w
    }

    /// A symbol named in machine code: a static, a procedure, or an integer constant.
    fn machine_symbol(&mut self, ctx: &Ctx, path: &oli_ast::Path) -> Option<MemTermKind> {
        if let [only] = path.segments.as_slice() {
            if ctx.lookup_local(&only.name).is_some() {
                let msg = format!(
                    "`{}` is a local; machine code cannot name locals",
                    only.name
                );
                self.report(
                    ctx.module,
                    Diagnostic::error("E0500", msg, only.span).with_note(
                        "move values in and out with `in REG <- expr` and `out REG -> place`",
                    ),
                );
                return None;
            }
        }
        match self.resolve_path_symbol(ctx.module, path)? {
            Symbol::Static(s) => {
                self.static_ready(s);
                Some(MemTermKind::Sym(SymRef::Static(s)))
            }
            Symbol::Proc(p) => Some(MemTermKind::Sym(SymRef::Proc(p))),
            Symbol::Const(c) => {
                self.const_ready(c);
                match (&self.prog.consts[c].value, self.const_backing[c]) {
                    (_, Some(s)) => Some(MemTermKind::Sym(SymRef::Static(s))),
                    (ConstValue::Int(v), None) => u128::try_from(*v).ok().map(MemTermKind::Int),
                    _ => {
                        let msg =
                            "only integer and aggregate constants can be named in machine code";
                        self.error(ctx.module, "E0500", msg, path.span);
                        None
                    }
                }
            }
            _ => {
                let msg = format!("`{}` cannot be used as a machine operand", path.dotted());
                self.error(ctx.module, "E0500", msg, path.span);
                None
            }
        }
    }

    fn check_operand(&mut self, ctx: &Ctx, o: &oli_ast::MachineOperand) -> Operand {
        match o {
            oli_ast::MachineOperand::Name(path) => {
                if let [only] = path.segments.as_slice() {
                    if register_width(&only.name).is_some() {
                        return Operand::Reg(only.name.clone());
                    }
                }
                match self.machine_symbol(ctx, path) {
                    Some(MemTermKind::Sym(s)) => Operand::Sym(s),
                    Some(MemTermKind::Int(v)) => {
                        Operand::Imm(i128::try_from(v).unwrap_or(i128::MAX))
                    }
                    _ => Operand::Imm(0),
                }
            }
            oli_ast::MachineOperand::Imm(v) => Operand::Imm(*v),
            oli_ast::MachineOperand::Label(l) => Operand::Label(l.name.clone()),
            oli_ast::MachineOperand::Mem { size, terms, .. } => {
                let terms = terms.iter().map(|t| self.check_mem_term(ctx, t)).collect();
                Operand::Mem {
                    size: size.as_ref().map(|s| s.name.clone()),
                    terms,
                }
            }
        }
    }

    fn check_mem_term(&mut self, ctx: &Ctx, t: &oli_ast::MemTerm) -> MemTerm {
        let kind = match &t.kind {
            oli_ast::MemTermKind::Name(path) => {
                if let [only] = path.segments.as_slice() {
                    if register_width(&only.name).is_some() {
                        return MemTerm {
                            negative: t.negative,
                            kind: MemTermKind::Reg(only.name.clone()),
                        };
                    }
                }
                self.machine_symbol(ctx, path)
                    .unwrap_or(MemTermKind::Int(0))
            }
            oli_ast::MemTermKind::Int(v) => MemTermKind::Int(*v),
            oli_ast::MemTermKind::Label(l) => MemTermKind::Label(l.name.clone()),
            oli_ast::MemTermKind::Scaled(r, k) => {
                if register_width(&r.name).is_none() {
                    self.error(
                        ctx.module,
                        "E0500",
                        format!("`{}` is not a register", r.name),
                        r.span,
                    );
                }
                MemTermKind::Scaled(r.name.clone(), *k)
            }
        };
        MemTerm {
            negative: t.negative,
            kind,
        }
    }
}
