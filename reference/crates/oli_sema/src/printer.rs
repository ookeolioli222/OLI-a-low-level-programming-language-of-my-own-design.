//! Text form of the semantic graph for `--show-sema` and snapshot tests.
//! Every expression prints as `(kind ...):type`, with `@region` when the
//! value may point into non-static memory.

use crate::hir::*;
use crate::regions::{Region, Regions};
use crate::ty::{Ty, TyPrinter};
use std::fmt::Write;

pub fn print_program(p: &Program) -> String {
    let mut pr = Printer {
        p,
        out: String::new(),
        depth: 0,
        locals: &[],
    };
    pr.program();
    pr.out
}

struct Printer<'a> {
    p: &'a Program,
    out: String,
    depth: usize,
    locals: &'a [LocalDef],
}

fn quote(b: &[u8]) -> String {
    let mut s = String::from('"');
    for &c in b {
        match c {
            b'"' => s.push_str(r#"\""#),
            0x5C => s.push_str(r"\\"),
            b'\n' => s.push_str(r"\n"),
            b'\t' => s.push_str(r"\t"),
            0x20..=0x7E => s.push(c as char),
            _ => {
                let _ = write!(s, r"\x{c:02x}");
            }
        }
    }
    s.push('"');
    s
}

impl Printer<'_> {
    fn ty(&self, t: &Ty) -> String {
        let ln = |id: usize| {
            self.p
                .layouts
                .get(id)
                .map_or("?".into(), |l| l.name.clone())
        };
        let cn = |id: usize| {
            self.p
                .choices
                .get(id)
                .map_or("?".into(), |c| c.name.clone())
        };
        TyPrinter {
            layout_name: &ln,
            choice_name: &cn,
        }
        .text(t)
    }

    fn regions(&self, r: &Regions) -> String {
        if r.atoms() == [Region::Static] {
            return String::new();
        }
        let name = |id: LocalId| {
            self.locals
                .get(id)
                .map_or("?".to_string(), |l| l.name.clone())
        };
        format!(" @{}", r.describe(&name))
    }

    fn line(&mut self, text: &str) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn close(&mut self) {
        if self.out.ends_with('\n') {
            self.out.pop();
        }
        self.out.push_str(")\n");
    }

    fn program(&mut self) {
        let entry = self
            .p
            .entry
            .map_or("-".to_string(), |e| self.p.procs[e].symbol.clone());
        self.line(&format!("(program entry={entry}"));
        self.depth += 1;
        for (i, l) in self.p.layouts.iter().enumerate() {
            let fields: Vec<String> = l
                .fields
                .iter()
                .map(|f| format!("({} {} @{})", f.name, self.ty(&f.ty), f.offset))
                .collect();
            let packed = if l.packed { " packed" } else { "" };
            self.line(&format!(
                "(layout {}#{i} size={} align={}{packed} {})",
                l.name,
                l.size,
                l.align,
                fields.join(" ")
            ));
        }
        for (i, c) in self.p.choices.iter().enumerate() {
            let variants: Vec<String> = c
                .variants
                .iter()
                .map(|v| {
                    let fields: Vec<String> = v
                        .fields
                        .iter()
                        .map(|f| format!("({} {} @{})", f.name, self.ty(&f.ty), f.offset))
                        .collect();
                    if fields.is_empty() {
                        format!("({})", v.name)
                    } else {
                        format!("({} {})", v.name, fields.join(" "))
                    }
                })
                .collect();
            self.line(&format!(
                "(choice {}#{i} size={} align={} tag={} payload@{} {})",
                c.name,
                c.size,
                c.align,
                c.tag_bytes,
                c.payload_offset,
                variants.join(" ")
            ));
        }
        for c in &self.p.consts {
            self.line(&format!(
                "(const {} : {} = {})",
                c.name,
                self.ty(&c.ty),
                self.const_value(&c.value)
            ));
        }
        for (i, s) in self.p.statics.iter().enumerate() {
            let init = s
                .init
                .as_ref()
                .map_or("zero".to_string(), |v| self.const_value(v));
            let ro = if s.readonly { " readonly" } else { "" };
            let sec = s
                .section
                .as_ref()
                .map_or(String::new(), |x| format!(" section={}", quote(x)));
            let al = s.align.map_or(String::new(), |a| format!(" align={a}"));
            self.line(&format!(
                "(static {}#{i} : {} = {init}{ro}{sec}{al})",
                s.name,
                self.ty(&s.ty)
            ));
        }
        for i in 0..self.p.procs.len() {
            self.proc(i);
        }
        self.depth -= 1;
        self.close();
    }

    fn const_value(&self, v: &ConstValue) -> String {
        match v {
            ConstValue::Int(i) => i.to_string(),
            ConstValue::Bool(b) => b.to_string(),
            ConstValue::Str(s) => quote(s),
            ConstValue::Layout(id, fields) => {
                let items: Vec<String> = fields.iter().map(|f| self.const_value(f)).collect();
                format!("{} {{{}}}", self.p.layouts[*id].name, items.join(", "))
            }
            ConstValue::Variant(id, idx, fields) => {
                let items: Vec<String> = fields.iter().map(|f| self.const_value(f)).collect();
                let name = &self.p.choices[*id].variants[*idx].name;
                if items.is_empty() {
                    name.clone()
                } else {
                    format!("{name} {{{}}}", items.join(", "))
                }
            }
            ConstValue::Array(items) => {
                let items: Vec<String> = items.iter().map(|f| self.const_value(f)).collect();
                format!("{{{}}}", items.join(", "))
            }
            ConstValue::None => "none".to_string(),
        }
    }

    fn proc(&mut self, id: ProcId) {
        let p = &self.p.procs[id];
        self.locals = &p.locals;
        let result = p
            .result
            .as_ref()
            .map_or(String::new(), |t| format!(" -> {}", self.ty(t)));
        let mut head = format!("(proc {}{result}", p.symbol);
        if !p.permits.is_empty() {
            let caps: Vec<&str> = p.permits.iter().map(|c| c.as_str()).collect();
            let _ = write!(head, " permits=[{}]", caps.join(","));
        }
        if p.conv == CallConv::None {
            head.push_str(" calls=none");
        }
        if p.is_entry {
            head.push_str(" entry");
        }
        if p.is_traps {
            head.push_str(" traps");
        }
        if let Some(s) = &p.section {
            let _ = write!(head, " section={}", quote(s));
        }
        self.line(&head);
        self.depth += 1;
        for (i, l) in p.locals.iter().enumerate() {
            let kind = match l.kind {
                LocalKind::Param(n) => format!("param{n}"),
                LocalKind::Binding => "bind".into(),
                LocalKind::Place => "place".into(),
                LocalKind::ZoneHandle => "zone".into(),
                LocalKind::PatternBinding => "pattern".into(),
            };
            let scope = l
                .scope_zone
                .map_or(String::new(), |z| format!(" in-zone={}", p.locals[z].name));
            self.line(&format!(
                "(local {i} {} {kind} {}{scope})",
                l.name,
                self.ty(&l.ty)
            ));
        }
        self.block("body", &p.body);
        self.depth -= 1;
        self.close();
    }

    fn block(&mut self, head: &str, b: &Block) {
        if b.stmts.is_empty() {
            self.line(&format!("({head})"));
            return;
        }
        self.line(&format!("({head}"));
        self.depth += 1;
        for s in &b.stmts {
            self.stmt(s);
        }
        self.depth -= 1;
        self.close();
    }

    fn local_name(&self, id: LocalId) -> String {
        self.locals.get(id).map_or("?".into(), |l| l.name.clone())
    }

    fn stmt(&mut self, s: &Stmt) {
        match &s.kind {
            StmtKind::Bind(id, e) => {
                self.line(&format!("(bind {} {})", self.local_name(*id), self.expr(e)))
            }
            StmtKind::Place(id, init) => {
                let i = init
                    .as_ref()
                    .map_or(String::new(), |e| format!(" {}", self.expr(e)));
                self.line(&format!("(place {}{i})", self.local_name(*id)));
            }
            StmtKind::Store(p, e) => {
                self.line(&format!("(store {} {})", self.place(p), self.expr(e)))
            }
            StmtKind::Expr(e) => self.line(&format!("(expr {})", self.expr(e))),
            StmtKind::If {
                cond,
                then,
                elifs,
                els,
            } => {
                self.line(&format!("(if {}", self.expr(cond)));
                self.depth += 1;
                self.block("then", then);
                for (c, b) in elifs {
                    self.block(&format!("elif {}", self.expr(c)), b);
                }
                if let Some(b) = els {
                    self.block("else", b);
                }
                self.depth -= 1;
                self.close();
            }
            StmtKind::While { cond, body } => {
                self.line(&format!("(while {}", self.expr(cond)));
                self.depth += 1;
                self.block("body", body);
                self.depth -= 1;
                self.close();
            }
            StmtKind::Each { var, iter, body } => {
                let it = match iter {
                    Iter::View(v) => self.expr(v),
                    Iter::Range(a, b) => format!("(range {} {})", self.expr(a), self.expr(b)),
                };
                self.line(&format!("(each {} {it}", self.local_name(*var)));
                self.depth += 1;
                self.block("body", body);
                self.depth -= 1;
                self.close();
            }
            StmtKind::Loop { body } => {
                self.line("(loop");
                self.depth += 1;
                self.block("body", body);
                self.depth -= 1;
                self.close();
            }
            StmtKind::Break => self.line("(break)"),
            StmtKind::Continue => self.line("(continue)"),
            StmtKind::Ret(e) => self.line(&format!(
                "(ret{})",
                e.as_ref()
                    .map_or(String::new(), |e| format!(" {}", self.expr(e)))
            )),
            StmtKind::Fail(e) => self.line(&format!(
                "(fail{})",
                e.as_ref()
                    .map_or(String::new(), |e| format!(" {}", self.expr(e)))
            )),
            StmtKind::Zone {
                handle,
                size,
                source,
                body,
            } => {
                let src = match source {
                    ZoneSource::Parent(z) => format!("parent={}", self.local_name(*z)),
                    ZoneSource::Os => "os".to_string(),
                    ZoneSource::At(a) => format!("at={}", self.expr(a)),
                    ZoneSource::FromZone(z) => format!("from-zone={}", self.expr(z)),
                    ZoneSource::FromBuffer(b) => format!("from-buffer={}", self.expr(b)),
                };
                self.line(&format!(
                    "(zone {} {} {src}",
                    self.local_name(*handle),
                    self.expr(size)
                ));
                self.depth += 1;
                self.block("body", body);
                self.depth -= 1;
                self.close();
            }
            StmtKind::Case {
                scrutinee,
                arms,
                els,
            } => {
                self.line(&format!("(case {}", self.expr(scrutinee)));
                self.depth += 1;
                for a in arms {
                    self.block(&format!("when {}", self.pattern(&a.pattern)), &a.body);
                }
                if let Some(b) = els {
                    self.block("else", b);
                }
                self.depth -= 1;
                self.close();
            }
            StmtKind::Machine(m) => self.machine(m),
        }
    }

    fn pattern(&self, p: &Pattern) -> String {
        match p {
            Pattern::Ok(None) => "ok".into(),
            Pattern::Ok(Some(l)) => format!("ok {}", self.local_name(*l)),
            Pattern::Fail(None) => "fail".into(),
            Pattern::Fail(Some(sub)) => format!("fail {}", self.pattern(sub)),
            Pattern::Variant {
                choice,
                index,
                bindings,
            } => {
                let name = &self.p.choices[*choice].variants[*index].name;
                if bindings.is_empty() {
                    name.clone()
                } else {
                    let b: Vec<String> = bindings
                        .iter()
                        .map(|(f, l)| format!("{}={}", f, self.local_name(*l)))
                        .collect();
                    format!("{name} {{{}}}", b.join(" "))
                }
            }
            Pattern::Bind(l) => format!("bind {}", self.local_name(*l)),
            Pattern::Int(v) => format!("(int {v})"),
            Pattern::Bool(b) => format!("(bool {b})"),
            Pattern::None => "none".into(),
        }
    }

    fn machine(&mut self, m: &Machine) {
        self.line(&format!("(machine {}", m.arch));
        self.depth += 1;
        for l in &m.lines {
            let text = match l {
                MachineLine::In { reg, value } => format!("(in {reg} {})", self.expr(value)),
                MachineLine::Out { reg, target } => format!("(out {reg} {})", self.place(target)),
                MachineLine::Clobber { regs, memory } => {
                    let mut items = regs.clone();
                    if *memory {
                        items.push("memory".into());
                    }
                    format!("(clobber {})", items.join(" "))
                }
                MachineLine::Label(l) => format!("(label {l})"),
                MachineLine::Instr {
                    mnemonic, operands, ..
                } => {
                    let ops: Vec<String> = operands.iter().map(|o| self.operand(o)).collect();
                    if ops.is_empty() {
                        format!("(instr {mnemonic})")
                    } else {
                        format!("(instr {mnemonic} {})", ops.join(" "))
                    }
                }
            };
            self.line(&text);
        }
        self.depth -= 1;
        self.close();
    }

    fn sym(&self, s: &SymRef) -> String {
        match s {
            SymRef::Static(id) => format!("static:{}", self.p.statics[*id].name),
            SymRef::Proc(id) => format!("proc:{}", self.p.procs[*id].symbol),
        }
    }

    fn operand(&self, o: &Operand) -> String {
        match o {
            Operand::Reg(r) => r.clone(),
            Operand::Sym(s) => self.sym(s),
            Operand::Imm(v) => v.to_string(),
            Operand::Label(l) => format!(".{l}"),
            Operand::Mem { size, terms } => {
                let mut s = String::from("(mem");
                if let Some(sz) = size {
                    let _ = write!(s, " {sz}");
                }
                for t in terms {
                    let sign = if t.negative { "-" } else { "+" };
                    let body = match &t.kind {
                        MemTermKind::Reg(r) => r.clone(),
                        MemTermKind::Sym(x) => self.sym(x),
                        MemTermKind::Int(v) => v.to_string(),
                        MemTermKind::Label(l) => format!(".{l}"),
                        MemTermKind::Scaled(r, k) => format!("{r}*{k}"),
                    };
                    let _ = write!(s, " {sign}{body}");
                }
                s.push(')');
                s
            }
        }
    }

    fn place(&self, p: &Place) -> String {
        let inner = match &p.kind {
            PlaceKind::Local(id) => format!("local {}", self.local_name(*id)),
            PlaceKind::Static(id) => format!("static {}", self.p.statics[*id].name),
            PlaceKind::Field(base, idx) => {
                let fname = match &base.ty {
                    Ty::Layout(l) => self.p.layouts[*l]
                        .fields
                        .get(*idx)
                        .map_or("?".into(), |f| f.name.clone()),
                    _ => idx.to_string(),
                };
                format!("field {} {fname}", self.place(base))
            }
            PlaceKind::ArrayElem(base, i) => format!("elem {} {}", self.place(base), self.expr(i)),
            PlaceKind::ViewElem(v, i) => format!("elem {} {}", self.expr(v), self.expr(i)),
            PlaceKind::Deref(r) => format!("deref {}", self.expr(r)),
            PlaceKind::Raw(a) => format!("raw {}", self.expr(a)),
        };
        let rw = if p.rw { "rw " } else { "" };
        format!("[{rw}{inner}]")
    }

    fn expr(&self, e: &Expr) -> String {
        let body = match &e.kind {
            ExprKind::Int(v) => format!("(int {v})"),
            ExprKind::Bool(b) => format!("(bool {b})"),
            ExprKind::Str(s) => format!("(str {})", quote(s)),
            ExprKind::NoneVal => "(none)".into(),
            ExprKind::Local(id) => format!("(local {})", self.local_name(*id)),
            ExprKind::Load(p) => format!("(load {})", self.place(p)),
            ExprKind::AddrOf(p) => format!("(addr-of {})", self.place(p)),
            ExprKind::RefOf(p) => format!("(ref-of {})", self.place(p)),
            ExprKind::ProcAddr(p) => format!("(proc-addr {})", self.p.procs[*p].symbol),
            ExprKind::ViewOfArray(p) => format!("(view-of {})", self.place(p)),
            ExprKind::ViewLen(v) => format!("(len {})", self.expr(v)),
            ExprKind::ViewAddr(v) => format!("(addr {})", self.expr(v)),
            ExprKind::Slice { view, start, end } => match end {
                Some(end) => format!(
                    "(slice {} {} {})",
                    self.expr(view),
                    self.expr(start),
                    self.expr(end)
                ),
                None => format!("(slice {} {} end)", self.expr(view), self.expr(start)),
            },
            ExprKind::ValueField(base, idx) => format!("(field {} {idx})", self.expr(base)),
            ExprKind::Call { proc, args } => {
                let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
                format!("(call {} {})", self.p.procs[*proc].symbol, a.join(" ")).replace(" )", ")")
            }
            ExprKind::Intrinsic { which, args } => {
                let a: Vec<String> = args.iter().map(|x| self.expr(x)).collect();
                format!("(intrinsic {which:?} {})", a.join(" ")).replace(" )", ")")
            }
            ExprKind::Unary(op, x) => {
                let o = match op {
                    UnOp::Not => "not",
                    UnOp::Neg => "neg",
                    UnOp::BitNot => "bitnot",
                };
                format!("({o} {})", self.expr(x))
            }
            ExprKind::Binary { op, mode, lhs, rhs } => {
                let m = match mode {
                    ArithMode::Trap => "",
                    ArithMode::Wrap => "/wrap",
                    ArithMode::Sat => "/sat",
                    ArithMode::Checked => "/checked",
                };
                format!(
                    "({}{m} {} {})",
                    format!("{op:?}").to_lowercase(),
                    self.expr(lhs),
                    self.expr(rhs)
                )
            }
            ExprKind::Convert { kind, expr } => format!(
                "({} {})",
                format!("{kind:?}").to_lowercase(),
                self.expr(expr)
            ),
            ExprKind::Fallback { expr, handler } => {
                let h = match handler {
                    Fallback::Fail => "(fail)".to_string(),
                    Fallback::Ret(None) => "(ret)".to_string(),
                    Fallback::Ret(Some(v)) => format!("(ret {})", self.expr(v)),
                    Fallback::Default(v) => format!("(default {})", self.expr(v)),
                };
                format!("(else {} {h})", self.expr(expr))
            }
            ExprKind::LayoutLit { layout, fields } => {
                let f: Vec<String> = fields.iter().map(|x| self.expr(x)).collect();
                format!("(layout {} {})", self.p.layouts[*layout].name, f.join(" "))
                    .replace(" )", ")")
            }
            ExprKind::VariantLit {
                choice,
                index,
                fields,
            } => {
                let f: Vec<String> = fields.iter().map(|x| self.expr(x)).collect();
                let name = &self.p.choices[*choice].variants[*index].name;
                format!("(variant {name} {})", f.join(" ")).replace(" )", ")")
            }
            ExprKind::ArrayLit(items) => {
                let f: Vec<String> = items.iter().map(|x| self.expr(x)).collect();
                format!("(array {})", f.join(" ")).replace("(array )", "(array)")
            }
            ExprKind::Checked(x) => format!("(checked {})", self.expr(x)),
            ExprKind::LayoutAt { layout, view } => {
                format!("(at {} {})", self.p.layouts[*layout].name, self.expr(view))
            }
            ExprKind::ZoneBytes {
                zone,
                size,
                fallible,
            } => {
                format!(
                    "({} {} {})",
                    if *fallible { "try-bytes" } else { "bytes" },
                    self.expr(zone),
                    self.expr(size)
                )
            }
            ExprKind::ZoneMake { zone, ty } => {
                format!("(make {} {})", self.expr(zone), self.ty(ty))
            }
            ExprKind::Error => "(error)".into(),
        };
        format!("{body}:{}{}", self.ty(&e.ty), self.regions(&e.regions))
    }
}
