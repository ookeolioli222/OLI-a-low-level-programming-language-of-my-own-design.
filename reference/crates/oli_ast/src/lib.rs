//! Abstract syntax tree of Oli-- V0 (`spec/OLI_SYNTAX_V0.md` §3–§6).
//!
//! The tree is purely syntactic: names are not resolved, types are not
//! checked, and `Name.at(v)` is an ordinary call. Every node carries the
//! span of its source text for diagnostics in later stages.

mod printer;

pub use printer::{expr_text, print_module, type_text};

use oli_diag::Span;
use oli_lexer::Prim;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

/// A dotted name as written in `module` and `import` lines: `std.os`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    pub segments: Vec<Ident>,
    pub span: Span,
}

impl Path {
    pub fn dotted(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    /// The `module` line, if present. Otherwise the file stem names the module.
    pub name: Option<Path>,
    pub imports: Vec<Import>,
    pub decls: Vec<Decl>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub path: Path,
    pub alias: Option<Ident>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decl {
    pub is_pub: bool,
    /// `---` comment lines immediately before the declaration.
    pub doc: Vec<String>,
    pub kind: DeclKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeclKind {
    /// `NAME [: T] := expr` — a compile-time constant. Aggregate constants live
    /// in `.rodata` and may carry `section`/`align` clauses.
    Const {
        name: Ident,
        ty: Option<Type>,
        value: Expr,
        clauses: Vec<Clause>,
    },
    /// `NAME : T [<- expr]` plus `section`/`align` clauses — a static place.
    Static {
        name: Ident,
        ty: Type,
        init: Option<Expr>,
        clauses: Vec<Clause>,
    },
    Proc(ProcDecl),
    Layout(LayoutDecl),
    Choice(ChoiceDecl),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    pub result: Option<Type>,
    pub clauses: Vec<Clause>,
    pub body: Block,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: Ident,
    pub ty: Type,
}

/// Header clauses of procedures and static places (`spec` §3, `proc_clause`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clause {
    pub kind: ClauseKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClauseKind {
    Permit(Vec<Path>),
    Calls(CallConv),
    Section(Vec<u8>),
    Align(u128),
    Entry,
    Export(Option<Vec<u8>>),
    Traps,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CallConv {
    Sysv,
    C,
    None,
    Interrupt,
}

impl CallConv {
    pub fn as_str(self) -> &'static str {
        match self {
            CallConv::Sysv => "sysv",
            CallConv::C => "c",
            CallConv::None => "none",
            CallConv::Interrupt => "interrupt",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutDecl {
    pub name: Ident,
    pub packed: bool,
    pub align: Option<u128>,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: Ident,
    pub ty: Type,
    pub align: Option<u128>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceDecl {
    pub name: Ident,
    pub variants: Vec<Variant>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    pub name: Ident,
    pub fields: Vec<Field>,
    pub span: Span,
}

// ---------------------------------------------------------------- types

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeKind {
    Prim(Prim),
    /// A layout or choice name, possibly module-qualified (`os.Error`).
    Named(Path),
    /// `[N]T` — N is an integer literal or a constant name.
    Array {
        len: Box<Expr>,
        elem: Box<Type>,
    },
    View {
        mmio: bool,
        rw: bool,
        elem: Box<Type>,
    },
    Ref {
        mmio: bool,
        rw: bool,
        elem: Box<Type>,
    },
    /// `addr T`; bare `addr` means `addr u8`.
    Addr(Option<Box<Type>>),
    Own(Box<Type>),
    Port(Prim),
    /// `be T` / `le T`.
    Endian {
        big: bool,
        prim: Prim,
    },
    Zone,
    /// `T or E`.
    Fallible {
        ok: Box<Type>,
        err: Box<Type>,
    },
    /// `none` — the empty type; `T or none` is an optional `T`.
    None,
    Never,
}

// ----------------------------------------------------------- statements

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StmtKind {
    /// `name [: T] := expr`
    Bind {
        name: Ident,
        ty: Option<Type>,
        value: Expr,
    },
    /// `name : T [<- expr]`
    Place {
        name: Ident,
        ty: Type,
        init: Option<Expr>,
    },
    /// `lvalue <- expr`
    Store {
        target: Expr,
        value: Expr,
    },
    /// `lvalue <~ expr`
    Move {
        target: Expr,
        value: Expr,
    },
    Expr(Expr),
    If {
        cond: Expr,
        then: Block,
        elifs: Vec<(Expr, Block)>,
        els: Option<Block>,
    },
    /// One-line `if cond then stmt`.
    IfThen {
        cond: Expr,
        stmt: Box<Stmt>,
    },
    While {
        cond: Expr,
        body: Block,
    },
    /// `each x in expr` — `expr` is a view or a `Range`.
    Each {
        var: Ident,
        iter: Expr,
        body: Block,
    },
    Loop {
        body: Block,
    },
    Break,
    Continue,
    Ret(Option<Expr>),
    Fail(Option<Expr>),
    Zone {
        name: Ident,
        size: Expr,
        source: Option<ZoneSource>,
        body: Block,
    },
    Case {
        scrutinee: Expr,
        arms: Vec<Arm>,
        els: Option<Block>,
    },
    Machine(MachineBlock),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZoneSource {
    At(Expr),
    From(Expr),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arm {
    pub pattern: Pattern,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternKind {
    /// `ok [name]`
    Ok(Option<Ident>),
    /// `fail [pattern]`
    Fail(Option<Box<Pattern>>),
    /// `variant` or `variant { a, b }` — also matches a plain binding name.
    Variant {
        name: Ident,
        fields: Vec<Ident>,
    },
    Int {
        negative: bool,
        value: u128,
    },
    Char(u8),
    Bool(bool),
    None,
    /// Placeholder produced during error recovery.
    Error,
}

// ------------------------------------------------------- machine blocks

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineBlock {
    pub arch: Ident,
    pub lines: Vec<MachineLine>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MachineLine {
    /// `in REG <- expr`
    In {
        reg: Ident,
        value: Expr,
    },
    /// `out REG -> lvalue`
    Out {
        reg: Ident,
        target: Expr,
    },
    Clobber(Vec<Clobber>),
    /// `.name:`
    Label(Ident),
    Instr {
        mnemonic: Ident,
        operands: Vec<MachineOperand>,
        span: Span,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Clobber {
    Reg(Ident),
    Memory(Span),
}

/// Operands are kept syntactic; the encoder decides what `rax` or `foo` is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MachineOperand {
    /// A register or a symbol (`rax`, `kernel.main`).
    Name(Path),
    Imm(i128),
    /// `.name`
    Label(Ident),
    /// `[size] [terms]` — e.g. `qword [rbp - 8]`, `[boot_stack + 16K]`, `[rax + rcx*8]`.
    Mem {
        size: Option<Ident>,
        terms: Vec<MemTerm>,
        span: Span,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemTerm {
    pub negative: bool,
    pub kind: MemTermKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemTermKind {
    Name(Path),
    Int(u128),
    Label(Ident),
    /// `reg * scale`
    Scaled(Ident, u128),
}

// ---------------------------------------------------------- expressions

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UnOp {
    Not,
    Neg,
    BitNot,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    And,
    Or,
}

impl BinOp {
    pub fn as_str(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::And => "and",
            BinOp::Or => "or",
        }
    }
}

/// Arithmetic / conversion mode words.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Wrap,
    Sat,
    Checked,
    /// Same-width reinterpretation (`uword.bits(x)`); conversions only.
    Bits,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Wrap => "wrap",
            Mode::Sat => "sat",
            Mode::Checked => "checked",
            Mode::Bits => "bits",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Handler {
    /// `else fail`
    Fail,
    /// `else ret [expr]`
    Ret(Option<Box<Expr>>),
    /// `else expr`
    Default(Box<Expr>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arg {
    pub name: Option<Ident>,
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldInit {
    pub name: Ident,
    pub value: Expr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprKind {
    Int(u128),
    Char(u8),
    Str(Vec<u8>),
    Bool(bool),
    None,
    Name(Ident),
    /// A type in value position: `u8` in `z.make(u8)`, or the callee of a conversion.
    TypeRef(Type),
    Field(Box<Expr>, Ident),
    Index(Box<Expr>, Box<Expr>),
    /// `a..b` / `a..` — only inside `[ ]` and after `each ... in`.
    Range {
        start: Box<Expr>,
        end: Option<Box<Expr>>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
    },
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `addr x`
    AddrOf(Box<Expr>),
    /// `ref x` / `rw ref x`
    RefOf {
        rw: bool,
        expr: Box<Expr>,
    },
    /// `[a]`
    RawLoad(Box<Expr>),
    /// `wrap(e)`, `sat(e)`, `checked(e)`
    ModeExpr(Mode, Box<Expr>),
    /// `T(e)`, `T.wrap(e)`, `T.bits(e)`
    Convert {
        ty: Type,
        mode: Option<Mode>,
        expr: Box<Expr>,
    },
    /// `expr else handler`
    Fallback {
        expr: Box<Expr>,
        handler: Handler,
    },
    /// `Name { f: e, ... }` — a layout literal or a choice variant with payload.
    StructLit {
        path: Box<Expr>,
        fields: Vec<FieldInit>,
    },
    ArrayLit(Vec<Expr>),
    /// Placeholder produced during error recovery; never present when no error was reported.
    Error,
}
