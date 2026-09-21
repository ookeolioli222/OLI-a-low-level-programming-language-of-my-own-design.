//! Semantic types of Oli-- V0 (`spec/OLI_SEMANTICS_V0.md` §2).

use std::fmt;

pub type LayoutId = usize;
pub type ChoiceId = usize;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum IntTy {
    U8,
    U16,
    U32,
    U64,
    S8,
    S16,
    S32,
    S64,
    /// `word`: same representation as `s64`, mutually convertible.
    Word,
    /// `uword`: same representation as `u64`, mutually convertible.
    Uword,
}

impl IntTy {
    pub fn bits(self) -> u32 {
        match self {
            IntTy::U8 | IntTy::S8 => 8,
            IntTy::U16 | IntTy::S16 => 16,
            IntTy::U32 | IntTy::S32 => 32,
            IntTy::U64 | IntTy::S64 | IntTy::Word | IntTy::Uword => 64,
        }
    }

    pub fn bytes(self) -> u64 {
        u64::from(self.bits() / 8)
    }

    pub fn signed(self) -> bool {
        matches!(
            self,
            IntTy::S8 | IntTy::S16 | IntTy::S32 | IntTy::S64 | IntTy::Word
        )
    }

    pub fn min(self) -> i128 {
        if self.signed() {
            -(1i128 << (self.bits() - 1))
        } else {
            0
        }
    }

    pub fn max(self) -> i128 {
        if self.signed() {
            (1i128 << (self.bits() - 1)) - 1
        } else {
            (1i128 << self.bits()) - 1
        }
    }

    pub fn fits(self, v: i128) -> bool {
        v >= self.min() && v <= self.max()
    }

    /// Wraps `v` into this type's range (two's complement).
    pub fn wrap(self, v: i128) -> i128 {
        let m = 1i128 << self.bits();
        let u = v.rem_euclid(m);
        if self.signed() && u > self.max() {
            u - m
        } else {
            u
        }
    }

    pub fn saturate(self, v: i128) -> i128 {
        v.clamp(self.min(), self.max())
    }

    /// Same machine representation (`word`/`s64`, `uword`/`u64`, or identical).
    pub fn same_repr(self, other: IntTy) -> bool {
        self == other
            || (self.bits() == 64 && other.bits() == 64 && self.signed() == other.signed())
    }

    /// Can a value of `self` be implicitly converted to `to` without loss?
    pub fn widens_to(self, to: IntTy) -> bool {
        self.same_repr(to) || (self.signed() == to.signed() && self.bits() < to.bits())
    }

    pub fn as_str(self) -> &'static str {
        match self {
            IntTy::U8 => "u8",
            IntTy::U16 => "u16",
            IntTy::U32 => "u32",
            IntTy::U64 => "u64",
            IntTy::S8 => "s8",
            IntTy::S16 => "s16",
            IntTy::S32 => "s32",
            IntTy::S64 => "s64",
            IntTy::Word => "word",
            IntTy::Uword => "uword",
        }
    }
}

impl fmt::Display for IntTy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    Int(IntTy),
    Bool,
    Physaddr,
    /// `addr T` (bare `addr` is `addr u8`).
    Addr(Box<Ty>),
    Ref {
        mmio: bool,
        rw: bool,
        elem: Box<Ty>,
    },
    View {
        mmio: bool,
        rw: bool,
        elem: Box<Ty>,
    },
    Port(IntTy),
    /// `be T` / `le T` — valid only as a layout field type.
    Endian {
        big: bool,
        int: IntTy,
    },
    Array(u64, Box<Ty>),
    Layout(LayoutId),
    Choice(ChoiceId),
    /// `T or E`
    Fallible(Box<Ty>, Box<Ty>),
    Zone,
    /// `none`: the empty type.
    None,
    /// The failure type of `checked(...)`; cannot be named in V0.
    Overflow,
    Never,
    /// A procedure call that produces no value.
    Unit,
    /// An integer literal or unannotated integer constant before context types it.
    UntypedInt,
    /// Poison after an error: unifies with everything, reports nothing.
    Error,
}

impl Ty {
    pub fn u8() -> Ty {
        Ty::Int(IntTy::U8)
    }

    pub fn uword() -> Ty {
        Ty::Int(IntTy::Uword)
    }

    pub fn word() -> Ty {
        Ty::Int(IntTy::Word)
    }

    pub fn view_u8(rw: bool) -> Ty {
        Ty::View {
            mmio: false,
            rw,
            elem: Box::new(Ty::u8()),
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Ty::Error)
    }

    pub fn is_int(&self) -> bool {
        matches!(self, Ty::Int(_))
    }

    pub fn as_int(&self) -> Option<IntTy> {
        match self {
            Ty::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn is_fallible(&self) -> bool {
        matches!(self, Ty::Fallible(..))
    }

    /// Structural equality with nominal layouts/choices; `word`≡`s64`, `uword`≡`u64`;
    /// `Error` equals everything so one mistake reports once.
    pub fn same(&self, other: &Ty) -> bool {
        match (self, other) {
            (Ty::Error, _) | (_, Ty::Error) => true,
            (Ty::Int(a), Ty::Int(b)) => a.same_repr(*b),
            (Ty::Addr(a), Ty::Addr(b)) => a.same(b),
            (
                Ty::Ref {
                    mmio: m1,
                    rw: r1,
                    elem: e1,
                },
                Ty::Ref {
                    mmio: m2,
                    rw: r2,
                    elem: e2,
                },
            )
            | (
                Ty::View {
                    mmio: m1,
                    rw: r1,
                    elem: e1,
                },
                Ty::View {
                    mmio: m2,
                    rw: r2,
                    elem: e2,
                },
            ) => m1 == m2 && r1 == r2 && e1.same(e2),
            (Ty::Port(a), Ty::Port(b)) => a == b,
            (Ty::Endian { big: b1, int: i1 }, Ty::Endian { big: b2, int: i2 }) => {
                b1 == b2 && i1 == i2
            }
            (Ty::Array(n, a), Ty::Array(m, b)) => n == m && a.same(b),
            (Ty::Fallible(a1, b1), Ty::Fallible(a2, b2)) => a1.same(a2) && b1.same(b2),
            _ => self == other,
        }
    }
}

/// Printing needs the names of layouts and choices; the checker supplies them.
pub struct TyPrinter<'a> {
    pub layout_name: &'a dyn Fn(LayoutId) -> String,
    pub choice_name: &'a dyn Fn(ChoiceId) -> String,
}

impl fmt::Debug for TyPrinter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TyPrinter")
    }
}

impl TyPrinter<'_> {
    pub fn text(&self, t: &Ty) -> String {
        let flags = |mmio: bool, rw: bool| {
            format!(
                "{}{}",
                if mmio { "mmio " } else { "" },
                if rw { "rw " } else { "" }
            )
        };
        match t {
            Ty::Int(i) => i.as_str().to_string(),
            Ty::Bool => "bool".into(),
            Ty::Physaddr => "physaddr".into(),
            Ty::Addr(e) => format!("addr {}", self.text(e)),
            Ty::Ref { mmio, rw, elem } => format!("{}ref {}", flags(*mmio, *rw), self.text(elem)),
            Ty::View { mmio, rw, elem } => format!("{}view {}", flags(*mmio, *rw), self.text(elem)),
            Ty::Port(i) => format!("port {}", i.as_str()),
            Ty::Endian { big, int } => {
                format!("{} {}", if *big { "be" } else { "le" }, int.as_str())
            }
            Ty::Array(n, e) => format!("[{n}]{}", self.text(e)),
            Ty::Layout(id) => (self.layout_name)(*id),
            Ty::Choice(id) => (self.choice_name)(*id),
            Ty::Fallible(a, b) => format!("{} or {}", self.text(a), self.text(b)),
            Ty::Zone => "zone".into(),
            Ty::None => "none".into(),
            Ty::Overflow => "Overflow".into(),
            Ty::Never => "never".into(),
            Ty::Unit => "(no value)".into(),
            Ty::UntypedInt => "integer".into(),
            Ty::Error => "<error>".into(),
        }
    }
}
