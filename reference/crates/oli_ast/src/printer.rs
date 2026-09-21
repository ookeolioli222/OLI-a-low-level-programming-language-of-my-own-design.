//! S-expression printer for `--show-ast` and snapshot tests.
//!
//! Types print as compact text (`rw view u8`), expressions as one-line
//! S-expressions, statements one per line with indentation.

use crate::*;
use std::fmt::Write;

pub fn print_module(m: &Module) -> String {
    let mut p = Printer {
        out: String::new(),
        depth: 0,
    };
    p.module(m);
    p.out
}

struct Printer {
    out: String,
    depth: usize,
}

fn quote_bytes(b: &[u8]) -> String {
    let mut s = String::from('"');
    for &c in b {
        match c {
            b'"' => s.push_str(r#"\""#),
            b'\\' => s.push_str(r"\\"),
            b'\n' => s.push_str(r"\n"),
            b'\t' => s.push_str(r"\t"),
            b'\r' => s.push_str(r"\r"),
            0x20..=0x7E => s.push(c as char),
            _ => {
                let _ = write!(s, r"\x{c:02x}");
            }
        }
    }
    s.push('"');
    s
}

pub fn type_text(t: &Type) -> String {
    match &t.kind {
        TypeKind::Prim(p) => p.as_str().to_string(),
        TypeKind::Named(p) => p.dotted(),
        TypeKind::Array { len, elem } => {
            let n = match &len.kind {
                ExprKind::Int(v) => v.to_string(),
                ExprKind::Name(id) => id.name.clone(),
                _ => expr_text(len),
            };
            format!("[{n}]{}", type_text(elem))
        }
        TypeKind::View { mmio, rw, elem } => {
            format!(
                "{}{}view {}",
                if *mmio { "mmio " } else { "" },
                if *rw { "rw " } else { "" },
                type_text(elem)
            )
        }
        TypeKind::Ref { mmio, rw, elem } => {
            format!(
                "{}{}ref {}",
                if *mmio { "mmio " } else { "" },
                if *rw { "rw " } else { "" },
                type_text(elem)
            )
        }
        TypeKind::Addr(None) => "addr".to_string(),
        TypeKind::Addr(Some(t)) => format!("addr {}", type_text(t)),
        TypeKind::Own(t) => format!("own {}", type_text(t)),
        TypeKind::Port(p) => format!("port {}", p.as_str()),
        TypeKind::Endian { big, prim } => {
            format!("{} {}", if *big { "be" } else { "le" }, prim.as_str())
        }
        TypeKind::Zone => "zone".to_string(),
        TypeKind::Fallible { ok, err } => format!("{} or {}", type_text(ok), type_text(err)),
        TypeKind::None => "none".to_string(),
        TypeKind::Never => "never".to_string(),
    }
}

pub fn expr_text(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Int(v) => format!("(int {v})"),
        ExprKind::Char(c) => format!("(char {})", quote_bytes(&[*c])),
        ExprKind::Str(s) => format!("(str {})", quote_bytes(s)),
        ExprKind::Bool(b) => format!("(bool {b})"),
        ExprKind::None => "(none)".to_string(),
        ExprKind::Name(id) => format!("(name {})", id.name),
        ExprKind::TypeRef(t) => format!("(type {})", type_text(t)),
        ExprKind::Field(base, f) => format!("(field {} {})", expr_text(base), f.name),
        ExprKind::Index(base, i) => format!("(index {} {})", expr_text(base), expr_text(i)),
        ExprKind::Range { start, end } => match end {
            Some(end) => format!("(range {} {})", expr_text(start), expr_text(end)),
            None => format!("(range {} end)", expr_text(start)),
        },
        ExprKind::Call { callee, args } => {
            let mut s = format!("(call {}", expr_text(callee));
            for a in args {
                match &a.name {
                    Some(n) => {
                        let _ = write!(s, " ({}: {})", n.name, expr_text(&a.value));
                    }
                    None => {
                        let _ = write!(s, " {}", expr_text(&a.value));
                    }
                }
            }
            s.push(')');
            s
        }
        ExprKind::Unary(op, x) => {
            let o = match op {
                UnOp::Not => "not",
                UnOp::Neg => "neg",
                UnOp::BitNot => "bitnot",
            };
            format!("({o} {})", expr_text(x))
        }
        ExprKind::Binary(op, l, r) => {
            format!("({} {} {})", op.as_str(), expr_text(l), expr_text(r))
        }
        ExprKind::AddrOf(x) => format!("(addr-of {})", expr_text(x)),
        ExprKind::RefOf { rw, expr } => format!(
            "({} {})",
            if *rw { "rw-ref-of" } else { "ref-of" },
            expr_text(expr)
        ),
        ExprKind::RawLoad(x) => format!("(raw-load {})", expr_text(x)),
        ExprKind::ModeExpr(m, x) => format!("({} {})", m.as_str(), expr_text(x)),
        ExprKind::Convert { ty, mode, expr } => match mode {
            Some(m) => format!(
                "(convert {} {} {})",
                type_text(ty),
                m.as_str(),
                expr_text(expr)
            ),
            None => format!("(convert {} {})", type_text(ty), expr_text(expr)),
        },
        ExprKind::Fallback { expr, handler } => {
            let h = match handler {
                Handler::Fail => "(fail)".to_string(),
                Handler::Ret(None) => "(ret)".to_string(),
                Handler::Ret(Some(v)) => format!("(ret {})", expr_text(v)),
                Handler::Default(v) => format!("(default {})", expr_text(v)),
            };
            format!("(else {} {})", expr_text(expr), h)
        }
        ExprKind::StructLit { path, fields } => {
            let mut s = format!("(lit {}", expr_text(path));
            for f in fields {
                let _ = write!(s, " ({}: {})", f.name.name, expr_text(&f.value));
            }
            s.push(')');
            s
        }
        ExprKind::ArrayLit(items) => {
            let mut s = String::from("(array");
            for i in items {
                let _ = write!(s, " {}", expr_text(i));
            }
            s.push(')');
            s
        }
        ExprKind::Error => "(error)".to_string(),
    }
}

impl Printer {
    fn line(&mut self, text: &str) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// Removes the trailing newline so a closing paren can be appended.
    fn close(&mut self, parens: usize) {
        if self.out.ends_with('\n') {
            self.out.pop();
        }
        for _ in 0..parens {
            self.out.push(')');
        }
        self.out.push('\n');
    }

    fn module(&mut self, m: &Module) {
        let name = m.name.as_ref().map_or("-".to_string(), Path::dotted);
        self.line(&format!("(module {name}"));
        self.depth += 1;
        for i in &m.imports {
            match &i.alias {
                Some(a) => self.line(&format!("(import {} as {})", i.path.dotted(), a.name)),
                None => self.line(&format!("(import {})", i.path.dotted())),
            }
        }
        for d in &m.decls {
            self.decl(d);
        }
        self.depth -= 1;
        self.close(1);
    }

    fn clauses(&mut self, clauses: &[Clause]) {
        for c in clauses {
            let text = match &c.kind {
                ClauseKind::Permit(caps) => {
                    format!(
                        "(permit {})",
                        caps.iter().map(Path::dotted).collect::<Vec<_>>().join(" ")
                    )
                }
                ClauseKind::Calls(cc) => format!("(calls {})", cc.as_str()),
                ClauseKind::Section(s) => format!("(section {})", quote_bytes(s)),
                ClauseKind::Align(n) => format!("(align {n})"),
                ClauseKind::Entry => "(entry)".to_string(),
                ClauseKind::Export(None) => "(export)".to_string(),
                ClauseKind::Export(Some(s)) => format!("(export {})", quote_bytes(s)),
                ClauseKind::Traps => "(traps)".to_string(),
            };
            self.line(&text);
        }
    }

    fn fields(&mut self, fields: &[Field]) {
        for f in fields {
            match f.align {
                Some(a) => self.line(&format!(
                    "(field {} {} align {a})",
                    f.name.name,
                    type_text(&f.ty)
                )),
                None => self.line(&format!("(field {} {})", f.name.name, type_text(&f.ty))),
            }
        }
    }

    fn decl(&mut self, d: &Decl) {
        let pub_ = if d.is_pub { "pub " } else { "" };
        match &d.kind {
            DeclKind::Const {
                name,
                ty,
                value,
                clauses,
            } => {
                let t = ty
                    .as_ref()
                    .map_or(String::new(), |t| format!(" : {}", type_text(t)));
                if clauses.is_empty() && d.doc.is_empty() {
                    self.line(&format!(
                        "({pub_}const {}{t} {})",
                        name.name,
                        expr_text(value)
                    ));
                } else {
                    self.line(&format!(
                        "({pub_}const {}{t} {}",
                        name.name,
                        expr_text(value)
                    ));
                    self.depth += 1;
                    self.docs(d);
                    self.clauses(clauses);
                    self.depth -= 1;
                    self.close(1);
                }
            }
            DeclKind::Static {
                name,
                ty,
                init,
                clauses,
            } => {
                let i = init
                    .as_ref()
                    .map_or(String::new(), |e| format!(" {}", expr_text(e)));
                self.line(&format!(
                    "({pub_}static {} : {}{i}",
                    name.name,
                    type_text(ty)
                ));
                self.depth += 1;
                self.docs(d);
                self.clauses(clauses);
                self.depth -= 1;
                self.close(1);
            }
            DeclKind::Proc(p) => {
                let params = p
                    .params
                    .iter()
                    .map(|q| format!("({} {})", q.name.name, type_text(&q.ty)))
                    .collect::<Vec<_>>()
                    .join(" ");
                let result = p
                    .result
                    .as_ref()
                    .map_or(String::new(), |t| format!(" -> {}", type_text(t)));
                self.line(&format!("({pub_}proc {} ({params}){result}", p.name.name));
                self.depth += 1;
                self.docs(d);
                self.clauses(&p.clauses);
                self.block("body", &p.body);
                self.depth -= 1;
                self.close(1);
            }
            DeclKind::Layout(l) => {
                let mut head = format!("({pub_}layout {}", l.name.name);
                if l.packed {
                    head.push_str(" packed");
                }
                if let Some(a) = l.align {
                    let _ = write!(head, " align {a}");
                }
                self.line(&head);
                self.depth += 1;
                self.docs(d);
                self.fields(&l.fields);
                self.depth -= 1;
                self.close(1);
            }
            DeclKind::Choice(c) => {
                self.line(&format!("({pub_}choice {}", c.name.name));
                self.depth += 1;
                self.docs(d);
                for v in &c.variants {
                    if v.fields.is_empty() {
                        self.line(&format!("(variant {})", v.name.name));
                    } else {
                        self.line(&format!("(variant {}", v.name.name));
                        self.depth += 1;
                        self.fields(&v.fields);
                        self.depth -= 1;
                        self.close(1);
                    }
                }
                self.depth -= 1;
                self.close(1);
            }
        }
    }

    fn docs(&mut self, d: &Decl) {
        for line in &d.doc {
            self.line(&format!("(doc {})", quote_bytes(line.as_bytes())));
        }
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
        self.close(1);
    }

    fn stmt(&mut self, s: &Stmt) {
        match &s.kind {
            StmtKind::Bind { name, ty, value } => {
                let t = ty
                    .as_ref()
                    .map_or(String::new(), |t| format!(" : {}", type_text(t)));
                self.line(&format!("(bind {}{t} {})", name.name, expr_text(value)));
            }
            StmtKind::Place { name, ty, init } => {
                let i = init
                    .as_ref()
                    .map_or(String::new(), |e| format!(" {}", expr_text(e)));
                self.line(&format!("(place {} : {}{i})", name.name, type_text(ty)));
            }
            StmtKind::Store { target, value } => {
                self.line(&format!(
                    "(store {} {})",
                    expr_text(target),
                    expr_text(value)
                ));
            }
            StmtKind::Move { target, value } => {
                self.line(&format!(
                    "(move {} {})",
                    expr_text(target),
                    expr_text(value)
                ));
            }
            StmtKind::Expr(e) => self.line(&format!("(expr {})", expr_text(e))),
            StmtKind::If {
                cond,
                then,
                elifs,
                els,
            } => {
                self.line(&format!("(if {}", expr_text(cond)));
                self.depth += 1;
                self.block("then", then);
                for (c, b) in elifs {
                    self.block(&format!("elif {}", expr_text(c)), b);
                }
                if let Some(b) = els {
                    self.block("else", b);
                }
                self.depth -= 1;
                self.close(1);
            }
            StmtKind::IfThen { cond, stmt } => {
                self.line(&format!("(if-then {}", expr_text(cond)));
                self.depth += 1;
                self.stmt(stmt);
                self.depth -= 1;
                self.close(1);
            }
            StmtKind::While { cond, body } => {
                self.line(&format!("(while {}", expr_text(cond)));
                self.depth += 1;
                self.block("body", body);
                self.depth -= 1;
                self.close(1);
            }
            StmtKind::Each { var, iter, body } => {
                self.line(&format!("(each {} {}", var.name, expr_text(iter)));
                self.depth += 1;
                self.block("body", body);
                self.depth -= 1;
                self.close(1);
            }
            StmtKind::Loop { body } => {
                self.line("(loop");
                self.depth += 1;
                self.block("body", body);
                self.depth -= 1;
                self.close(1);
            }
            StmtKind::Break => self.line("(break)"),
            StmtKind::Continue => self.line("(continue)"),
            StmtKind::Ret(None) => self.line("(ret)"),
            StmtKind::Ret(Some(e)) => self.line(&format!("(ret {})", expr_text(e))),
            StmtKind::Fail(None) => self.line("(fail)"),
            StmtKind::Fail(Some(e)) => self.line(&format!("(fail {})", expr_text(e))),
            StmtKind::Zone {
                name,
                size,
                source,
                body,
            } => {
                let src = match source {
                    Some(ZoneSource::At(e)) => format!(" at {}", expr_text(e)),
                    Some(ZoneSource::From(e)) => format!(" from {}", expr_text(e)),
                    None => String::new(),
                };
                self.line(&format!("(zone {} {}{src}", name.name, expr_text(size)));
                self.depth += 1;
                self.block("body", body);
                self.depth -= 1;
                self.close(1);
            }
            StmtKind::Case {
                scrutinee,
                arms,
                els,
            } => {
                self.line(&format!("(case {}", expr_text(scrutinee)));
                self.depth += 1;
                for a in arms {
                    self.block(&format!("when {}", pattern_text(&a.pattern)), &a.body);
                }
                if let Some(b) = els {
                    self.block("else", b);
                }
                self.depth -= 1;
                self.close(1);
            }
            StmtKind::Machine(m) => self.machine(m),
        }
    }

    fn machine(&mut self, m: &MachineBlock) {
        self.line(&format!("(machine {}", m.arch.name));
        self.depth += 1;
        for l in &m.lines {
            let text = match l {
                MachineLine::In { reg, value } => format!("(in {} {})", reg.name, expr_text(value)),
                MachineLine::Out { reg, target } => {
                    format!("(out {} {})", reg.name, expr_text(target))
                }
                MachineLine::Clobber(list) => {
                    let items = list
                        .iter()
                        .map(|c| match c {
                            Clobber::Reg(r) => r.name.clone(),
                            Clobber::Memory(_) => "memory".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!("(clobber {items})")
                }
                MachineLine::Label(l) => format!("(label {})", l.name),
                MachineLine::Instr {
                    mnemonic, operands, ..
                } => {
                    let mut s = format!("(instr {}", mnemonic.name);
                    for o in operands {
                        let _ = write!(s, " {}", operand_text(o));
                    }
                    s.push(')');
                    s
                }
            };
            self.line(&text);
        }
        self.depth -= 1;
        self.close(1);
    }
}

fn pattern_text(p: &Pattern) -> String {
    match &p.kind {
        PatternKind::Ok(None) => "ok".to_string(),
        PatternKind::Ok(Some(id)) => format!("ok {}", id.name),
        PatternKind::Fail(None) => "fail".to_string(),
        PatternKind::Fail(Some(sub)) => format!("fail {}", pattern_text(sub)),
        PatternKind::Variant { name, fields } => {
            if fields.is_empty() {
                name.name.clone()
            } else {
                let f = fields
                    .iter()
                    .map(|f| f.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("{} {{{f}}}", name.name)
            }
        }
        PatternKind::Int {
            negative: false,
            value,
        } => format!("(int {value})"),
        PatternKind::Int {
            negative: true,
            value,
        } => format!("(int -{value})"),
        PatternKind::Error => "(error)".to_string(),
        PatternKind::Char(c) => format!("(char {})", quote_bytes(&[*c])),
        PatternKind::Bool(b) => format!("(bool {b})"),
        PatternKind::None => "none".to_string(),
    }
}

fn operand_text(o: &MachineOperand) -> String {
    match o {
        MachineOperand::Name(p) => p.dotted(),
        MachineOperand::Imm(v) => v.to_string(),
        MachineOperand::Label(l) => format!(".{}", l.name),
        MachineOperand::Mem { size, terms, .. } => {
            let mut s = String::from("(mem");
            if let Some(sz) = size {
                let _ = write!(s, " {}", sz.name);
            }
            for t in terms {
                let sign = if t.negative { "-" } else { "+" };
                let body = match &t.kind {
                    MemTermKind::Name(p) => p.dotted(),
                    MemTermKind::Int(v) => v.to_string(),
                    MemTermKind::Label(l) => format!(".{}", l.name),
                    MemTermKind::Scaled(r, k) => format!("{}*{k}", r.name),
                };
                let _ = write!(s, " {sign}{body}");
            }
            s.push(')');
            s
        }
    }
}
