//! The semantic graph: the typed, resolved form of a program that OIR
//! lowering consumes. Every expression carries its type and regions;
//! places (memory) and values are distinct.

use crate::regions::Regions;
use crate::ty::{ChoiceId, IntTy, LayoutId, Ty};
use oli_diag::Span;

pub type ModuleId = usize;
pub type ProcId = usize;
pub type StaticId = usize;
pub type ConstId = usize;
pub type LocalId = usize;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Hosted,
    Freestanding,
}

#[derive(Debug, Clone, Default)]
pub struct Program {
    pub modules: Vec<ModuleDef>,
    pub layouts: Vec<LayoutDef>,
    pub choices: Vec<ChoiceDef>,
    pub statics: Vec<StaticDef>,
    pub consts: Vec<ConstDef>,
    pub procs: Vec<ProcDef>,
    pub entry: Option<ProcId>,
    pub traps: Option<ProcId>,
}

#[derive(Debug, Clone)]
pub struct ModuleDef {
    pub name: String,
    pub file: u32,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub ty: Ty,
    pub offset: u64,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct LayoutDef {
    pub module: ModuleId,
    pub name: String,
    pub is_pub: bool,
    pub packed: bool,
    pub fields: Vec<FieldDef>,
    pub size: u64,
    pub align: u64,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct VariantDef {
    pub name: String,
    /// Payload fields laid out from the payload offset of the choice.
    pub fields: Vec<FieldDef>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ChoiceDef {
    pub module: ModuleId,
    pub name: String,
    pub is_pub: bool,
    pub variants: Vec<VariantDef>,
    pub tag_bytes: u64,
    pub payload_offset: u64,
    pub size: u64,
    pub align: u64,
    pub span: Span,
}

/// A compile-time value (`spec` §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstValue {
    Int(i128),
    Bool(bool),
    /// A string literal: a `view u8` into `.rodata`.
    Str(Vec<u8>),
    Layout(LayoutId, Vec<ConstValue>),
    Variant(ChoiceId, usize, Vec<ConstValue>),
    Array(Vec<ConstValue>),
    None,
}

#[derive(Debug, Clone)]
pub struct ConstDef {
    pub module: ModuleId,
    pub name: String,
    pub is_pub: bool,
    /// `UntypedInt` for an unannotated integer constant.
    pub ty: Ty,
    pub value: ConstValue,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StaticDef {
    pub module: ModuleId,
    pub name: String,
    pub is_pub: bool,
    pub ty: Ty,
    /// `None` → `.bss` (zeroed); `Some` → `.data`, or `.rodata` when `readonly`.
    pub init: Option<ConstValue>,
    /// Aggregate constants (`X : [3]u64 := {...}`) are read-only static places.
    pub readonly: bool,
    pub section: Option<Vec<u8>>,
    pub align: Option<u64>,
    pub span: Span,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Capability {
    MemoryRaw,
    MemoryMmio,
    IoPort,
    CpuAsm,
    CpuHalt,
    CpuInterrupt,
    CpuMsr,
    CpuControl,
    OsSyscall,
}

impl Capability {
    pub fn from_path(s: &str) -> Option<Capability> {
        Some(match s {
            "memory.raw" => Capability::MemoryRaw,
            "memory.mmio" => Capability::MemoryMmio,
            "io.port" => Capability::IoPort,
            "cpu.asm" => Capability::CpuAsm,
            "cpu.halt" => Capability::CpuHalt,
            "cpu.interrupt" => Capability::CpuInterrupt,
            "cpu.msr" => Capability::CpuMsr,
            "cpu.control" => Capability::CpuControl,
            "os.syscall" => Capability::OsSyscall,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Capability::MemoryRaw => "memory.raw",
            Capability::MemoryMmio => "memory.mmio",
            Capability::IoPort => "io.port",
            Capability::CpuAsm => "cpu.asm",
            Capability::CpuHalt => "cpu.halt",
            Capability::CpuInterrupt => "cpu.interrupt",
            Capability::CpuMsr => "cpu.msr",
            Capability::CpuControl => "cpu.control",
            Capability::OsSyscall => "os.syscall",
        }
    }

    /// Faults in user mode on a hosted target.
    pub fn is_privileged(self) -> bool {
        !matches!(
            self,
            Capability::MemoryRaw
                | Capability::MemoryMmio
                | Capability::CpuAsm
                | Capability::OsSyscall
        )
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CallConv {
    Sysv,
    None,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LocalKind {
    Param(usize),
    Binding,
    Place,
    /// The handle introduced by `zone NAME ...`.
    ZoneHandle,
    /// A name bound by a `case` pattern.
    PatternBinding,
}

#[derive(Debug, Clone)]
pub struct LocalDef {
    pub name: String,
    pub ty: Ty,
    pub kind: LocalKind,
    /// For places: the innermost zone block enclosing the declaration, if any.
    pub scope_zone: Option<LocalId>,
    /// For zone handles: the enclosing zone (source `Parent`).
    pub parent_zone: Option<LocalId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ProcDef {
    pub module: ModuleId,
    pub name: String,
    /// ELF symbol: `module.name`, or the `export` name.
    pub symbol: String,
    pub is_pub: bool,
    pub params: Vec<LocalId>,
    /// `None` for procedures without a result.
    pub result: Option<Ty>,
    pub permits: Vec<Capability>,
    pub conv: CallConv,
    pub section: Option<Vec<u8>>,
    pub align: Option<u64>,
    pub is_entry: bool,
    pub export: Option<String>,
    pub is_traps: bool,
    pub locals: Vec<LocalDef>,
    pub body: Block,
    pub span: Span,
}

// ------------------------------------------------------------------ bodies

#[derive(Debug, Clone, Default)]
pub struct Block {
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    Bind(LocalId, Expr),
    Place(LocalId, Option<Expr>),
    Store(Place, Expr),
    Expr(Expr),
    If {
        cond: Expr,
        then: Block,
        elifs: Vec<(Expr, Block)>,
        els: Option<Block>,
    },
    While {
        cond: Expr,
        body: Block,
    },
    Each {
        var: LocalId,
        iter: Iter,
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
        handle: LocalId,
        size: Expr,
        source: ZoneSource,
        body: Block,
    },
    Case {
        scrutinee: Expr,
        arms: Vec<Arm>,
        els: Option<Block>,
    },
    Machine(Machine),
}

#[derive(Debug, Clone)]
pub enum Iter {
    View(Expr),
    Range(Expr, Expr),
}

#[derive(Debug, Clone)]
pub enum ZoneSource {
    /// Carved from the enclosing zone of the same procedure.
    Parent(LocalId),
    /// `mmap` from the operating system (hosted only).
    Os,
    /// `at ADDR`
    At(Expr),
    /// `from HANDLE` (a zone) or `from BUFFER` (a `rw view u8`).
    FromZone(Expr),
    FromBuffer(Expr),
}

#[derive(Debug, Clone)]
pub struct Arm {
    pub pattern: Pattern,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Ok(Option<LocalId>),
    Fail(Option<Box<Pattern>>),
    Variant {
        choice: ChoiceId,
        index: usize,
        bindings: Vec<(usize, LocalId)>,
    },
    /// Binds the whole value.
    Bind(LocalId),
    Int(i128),
    Bool(bool),
    None,
}

#[derive(Debug, Clone)]
pub struct Machine {
    pub arch: String,
    pub lines: Vec<MachineLine>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum MachineLine {
    In {
        reg: String,
        value: Expr,
    },
    Out {
        reg: String,
        target: Place,
    },
    Clobber {
        regs: Vec<String>,
        memory: bool,
    },
    Label(String),
    Instr {
        mnemonic: String,
        operands: Vec<Operand>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum SymRef {
    Static(StaticId),
    Proc(ProcId),
}

#[derive(Debug, Clone)]
pub enum Operand {
    /// A register name (validated by the encoder).
    Reg(String),
    Sym(SymRef),
    Imm(i128),
    Label(String),
    Mem {
        size: Option<String>,
        terms: Vec<MemTerm>,
    },
}

#[derive(Debug, Clone)]
pub struct MemTerm {
    pub negative: bool,
    pub kind: MemTermKind,
}

#[derive(Debug, Clone)]
pub enum MemTermKind {
    Reg(String),
    Sym(SymRef),
    Int(u128),
    Label(String),
    Scaled(String, u128),
}

// ------------------------------------------------------------------ places

/// A typed memory location.
#[derive(Debug, Clone)]
pub struct Place {
    pub kind: PlaceKind,
    pub ty: Ty,
    /// Writable through this path (`rw` view/ref, non-readonly static, any local place).
    pub rw: bool,
    /// Where the memory lives; the region a loaded region-carrying value inherits.
    pub regions: Regions,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum PlaceKind {
    Local(LocalId),
    Static(StaticId),
    Field(Box<Place>, usize),
    /// Element of an array place; bounds against the constant length.
    ArrayElem(Box<Place>, Box<Expr>),
    /// Element of a view; bounds against the view's length (`CHECK`).
    ViewElem(Box<Expr>, Box<Expr>),
    /// The referent of a `ref`.
    Deref(Box<Expr>),
    /// `[a]` — raw memory, no checks (`memory.raw`).
    Raw(Box<Expr>),
}

// ------------------------------------------------------------- expressions

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Ty,
    pub regions: Regions,
    pub span: Span,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArithMode {
    Trap,
    Wrap,
    Sat,
    Checked,
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UnOp {
    Not,
    Neg,
    BitNot,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ConvKind {
    /// Lossless integer widening (or same representation).
    Widen,
    /// `T.wrap(x)`: truncate or extend, wrapping.
    Wrap,
    /// `T.sat(x)`: saturate.
    Sat,
    /// `T.checked(x)`: `T or Overflow`.
    Checked,
    /// `T.bits(x)`: same-width reinterpretation.
    Bits,
    /// integer → `addr T` (`memory.raw`)
    IntToAddr,
    /// `addr T` → `uword` (`memory.raw`)
    AddrToInt,
    /// integer → `physaddr`
    IntToPhys,
    /// `physaddr` → `uword`
    PhysToInt,
    /// `rw` → read-only, or `addr T` reinterpretation of element type.
    Reinterpret,
}

#[derive(Debug, Clone)]
pub enum Fallback {
    Fail,
    Ret(Option<Box<Expr>>),
    Default(Box<Expr>),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Intrinsic {
    OsSyscall,
    CpuHalt,
    CpuPause,
    MemCopy,
    MemSet,
    MemZero,
    MemSecureZero,
    /// `mem.get_*`: (big-endian?, width bits); native order is `None`.
    MemGet(Option<bool>, u32),
    MemPut(Option<bool>, u32),
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Int(i128),
    Bool(bool),
    /// A string literal: `view u8` into `.rodata`.
    Str(Vec<u8>),
    NoneVal,
    /// The value of a binding or parameter.
    Local(LocalId),
    Load(Place),
    /// `addr place` → `addr T`
    AddrOf(Place),
    /// `ref place` / `rw ref place`
    RefOf(Place),
    /// `addr proc`
    ProcAddr(ProcId),
    /// An array place used as a view.
    ViewOfArray(Place),
    ViewLen(Box<Expr>),
    ViewAddr(Box<Expr>),
    /// `v[a..b]` / `v[a..]`
    Slice {
        view: Box<Expr>,
        start: Box<Expr>,
        end: Option<Box<Expr>>,
    },
    /// Field of a by-value aggregate (a call result); places use `Place::Field`.
    ValueField(Box<Expr>, usize),
    Call {
        proc: ProcId,
        args: Vec<Expr>,
    },
    Intrinsic {
        which: Intrinsic,
        args: Vec<Expr>,
    },
    Unary(UnOp, Box<Expr>),
    Binary {
        op: BinOp,
        mode: ArithMode,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Convert {
        kind: ConvKind,
        expr: Box<Expr>,
    },
    Fallback {
        expr: Box<Expr>,
        handler: Fallback,
    },
    LayoutLit {
        layout: LayoutId,
        fields: Vec<Expr>,
    },
    VariantLit {
        choice: ChoiceId,
        index: usize,
        fields: Vec<Expr>,
    },
    ArrayLit(Vec<Expr>),
    /// `checked(e)`: the arithmetic inside runs in checked mode; the result is `T or Overflow`.
    Checked(Box<Expr>),
    /// `Name.at(v)`
    LayoutAt {
        layout: LayoutId,
        view: Box<Expr>,
    },
    ZoneBytes {
        zone: Box<Expr>,
        size: Box<Expr>,
        fallible: bool,
    },
    ZoneMake {
        zone: Box<Expr>,
        ty: Ty,
    },
    Error,
}

impl Expr {
    pub fn error(span: Span) -> Expr {
        Expr {
            kind: ExprKind::Error,
            ty: Ty::Error,
            regions: Regions::stat(),
            span,
        }
    }

    pub fn int(v: i128, ty: IntTy, span: Span) -> Expr {
        Expr {
            kind: ExprKind::Int(v),
            ty: Ty::Int(ty),
            regions: Regions::stat(),
            span,
        }
    }
}
