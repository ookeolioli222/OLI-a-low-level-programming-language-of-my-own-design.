# Oli-- Semantics V0 (normative)

Static and dynamic semantics of the V0 language defined syntactically in
`OLI_SYNTAX_V0.md`. Memory rules are in `OLI_MEMORY_V0.md`. The machine
these semantics are defined against is `docs/MACHINE_MODEL.md`.

## 1. Program structure

- A **program** is a set of modules; exactly one is the root given to `olic`.
- A **module** is one file. `import a.b` makes the public names of module `a.b`
  available as `b.name`; `import a.b as x` as `x.name`. Imports are not transitive.
- `pub` makes a declaration visible to importers. Everything else is module-private.
- The names `cpu`, `mem` and `os` are **intrinsic namespaces**, always in scope.
  An import whose last segment (or alias) is one of them - `import core.mem`,
  `import std.os` - adds that module's public names to the namespace; intrinsics
  take precedence over module members of the same name. `core` (the module
  `core.oli` of the library) is imported implicitly. A module may not declare a
  top-level name equal to any of these four (`E0103`).
- Module lookup: `import a.b` loads `a/b.oli` from the root file's directory,
  then from the library directories (`--lib`, `$OLI_LIB`, the `lib/` next to the
  compiler). A non-root module's `module` line must name its import path (`E0109`);
  the root module is named by its `module` line, or its file stem.
- A module may refer to itself by its own name (`call kernel.main` inside `kernel`).
- Module-level declarations may appear in any order and may refer to each other
  (procedures may be mutually recursive; constants may not be cyclic, `E0106`).

### Scopes and names
- Scopes: module → procedure → nested blocks (`if`, `while`, `each`, `loop`, `zone`, `case` arms).
- A name declared in a procedure may **not shadow** any name visible at its
  declaration point (parameters, outer blocks, module names). Diagnostics point
  at both declarations. Rationale: kernel code must never silently rebind `rsp`-like names.
- Names starting with `_` may be left unused; any other unused binding or place is a warning.

## 2. Types

| Type | Kind | Notes |
|------|------|-------|
| `u8 u16 u32 u64 s8 s16 s32 s64 word uword byte` | integer | `byte` ≡ `u8`; `word` ≡ `s64`, `uword` ≡ `u64` on x86-64 (distinct names, same representation, implicitly convertible) |
| `bool` | boolean | only `true`/`false` |
| `physaddr` | address-like | no dereference; `+`/`-` with `uword` |
| `addr T` | raw address | `[a]` under `memory.raw`; `+`/`-` with `uword` in **bytes** |
| `ref T`, `rw ref T` | reference | one object; field access; no arithmetic |
| `view T`, `rw view T` | view | `.len`, `.addr`, `v[i]`, `v[a..b]` |
| `mmio ref/view T` | device | as above, volatile |
| `port T` | I/O port | V1 methods; V0 parses the type and rejects use |
| `be T`, `le T` | ordered integer | only as layout field types and in `mem.put_*`/`get_*` helpers |
| `[N]T` | array place type | places and statics only; not a value type |
| `layout` names | aggregate | nominal; fields; `Name.size`, `Name.align`, `Name.at(v)` |
| `choice` names | tagged union | nominal; variants with optional payload |
| `T or E` | fallible | resolved by `else`/`case` |
| `zone` | zone handle | `z.bytes(n)`, `z.try_bytes(n)`, `z.make(T)` |
| `never` | bottom | procedures that do not return |
| `own T` | reserved | parsed; using it is `E0900 feature not implemented` |

Type identity is nominal for `layout`/`choice` and structural for everything else.

### Implicit conversions (lossless only)
| From | To |
|------|----|
| integer literal | any integer type whose range contains the value; `physaddr`; `addr T` only under `memory.raw` |
| `uN` | any wider `uN`, and `uword` |
| `sN` | any wider `sN`, and `word` |
| `rw view T` | `view T`; `rw ref T` → `ref T`; `mmio` is never dropped |
| `[N]T` place | `view T` (read) or `rw view T` (write context), `len = N` |
| `ref T` | a `ref T` denotes a place of type `T`: reading it where a `T` is expected is a load; `rw ref T` may be stored through with `<-`. A `view T` never auto-loads |
| compile-time constant integer | like a literal: any integer type whose range contains the value |
| integer literal / constant | `port T` (V1) |

### Explicit conversions
`T(x)` — lossless, verified statically (`u32(b)` for `b: u8`, `s32(b)` for `b: u8`;
`u8(x)` for `x: u32` is an error unless `x` is a compile-time constant that fits).
Explicit conversion also accepts unsigned to strictly wider signed, which implicit
widening does not. `T.wrap(x)` truncates; `T.sat(x)` saturates; `T.checked(x)` yields `T or Overflow`.
`uword(a)` for `a: addr T` and `addr T (n)` for an integer `n` require `memory.raw`.
`uword(p)` for `p: physaddr` is always allowed (a number); `physaddr(n)` likewise.
Between signed and unsigned of equal width: `T.bits(x)` (reinterpretation).

## 3. Integer literals and inference

An integer literal has the type demanded by its context: the other operand,
the declared type, the parameter, the field, the result type, the element
type. A literal with no context (`x := 10`) is an error `E0201: integer literal
needs a type` — write `x : u32 := 10` or `u32(10)`. The exception is a range
`a..b` in `each`: if neither bound has a type, it is `uword`.

## 4. Expressions

- Evaluation is strict and left-to-right, including arguments.
- Arithmetic (`+ - * / %`) requires both operands of one integer type after
  implicit widening; the result has that type; overflow, division by zero and
  `MIN / -1` **trap**. `-x` on unsigned is an error; on signed it traps for `MIN`.
- `wrap(e)`, `sat(e)`, `checked(e)` set the mode of every arithmetic operator lexically
  inside `e` (not inside nested calls). `checked(e)` has type `T or Overflow`.
- Bit operators `& | ^ ~` require equal integer types. Shifts `<< >>` take any
  unsigned count; counts ≥ width are masked to the width (x86 semantics, defined).
  `>>` is logical for unsigned, arithmetic for signed.
- Comparisons require equal types after widening and yield `bool`; they do not chain.
- `and`/`or` short-circuit on `bool`; `not` negates `bool`.
- `addr x`: address of a place `x`, of the referent of a `ref`, or of a view's first
  element. `ref x` / `rw ref x`: reference to a place (writability of `rw ref` requires
  a place, not a binding).
- `[a]` with `a : addr T` loads a `T` (under `memory.raw`).
- `v[i]`: element `i` of a view or array place; a **bounds check** (`CHECK`) precedes it.
- `v[a..b]`, `v[a..]`: sub-view; `a <= b <= v.len` is checked; type follows `v` (`rw` kept).
- `r.f`: field of a `ref`/`rw ref`/place of layout type; a `be`/`le` field reads as the native integer.
- `Name { f: e, ... }`: layout literal — **all** fields required, any order. Variant
  literal `variant { f: e }` or bare `variant`. `{ e1, e2, ... }`: array literal with exactly `N` elements.
- `Name.at(v)`: reinterprets the start of `v : view u8` as `ref Name` (`rw` follows `v`);
  checks `v.len >= Name.size` and, unless `packed`, alignment. Region follows `v`.
- Calls: `f(a, b)` or `f(x: a, y: b)` (all positional or all named). Arguments are
  passed by value: integers, addresses, refs and views copy their words; a **layout
  by value copies its bytes** (`COPY`, reported by `--explain-cost`).
- A fallible expression (`T or E`) must be resolved:
  `e else fail` (propagates `E` to the current procedure, whose failure type must accept it),
  `e else ret v`, `e else v` (a `T`), or `case`. A fallible value that is neither
  resolved nor bound is `E0310: unhandled failure`.

## 5. Statements

| Statement | Rules |
|-----------|-------|
| `x := e` | binding; immutable; type from annotation or `e`; no address |
| `x : T` | place; must be definitely stored before any read (`E0220: read of uninitialized place`) |
| `x : T <- e` | place with initial store |
| `p <- e` | `p` must be a place, a field/element of a writable place, an element of a `rw view`, a field of a `rw ref`, or `[a]` under `memory.raw`; `e` converts to `p`'s type |
| `p <~ e` | reserved (`own`); `E0900` in V0 |
| `if c` | `c : bool`; branches are separate scopes |
| `while c` | `c : bool` |
| `each x in v` | `v : view T` → `x : T` per element, in order; `v : [N]T` place likewise; `a..b` → integers from `a` to `b-1`; `x` is a binding |
| `loop` | infinite; leaves only by `break`, `ret`, `fail` or a `never` call |
| `break`, `continue` | innermost loop only |
| `ret e` | `e` converts to the procedure result type (or the `T` of `T or E`); `ret` alone only in procedures without a result |
| `fail e` | only in `-> T or E` procedures; `e` converts to `E`. Bare `fail` inside an `else` handler propagates the failure being handled; elsewhere it is allowed only when `E` is `none` |
| `zone z n [at a] [from s]` | see `OLI_MEMORY_V0.md` §3; `from s` takes a zone handle or a `rw view u8` (e.g. a static array) |
| `case e` | arms are tried in order; must be **exhaustive** (all variants, or `ok` and `fail`, or an `else` arm); payload fields are bound as bindings; for integers/chars only `else` provides exhaustiveness |
| `machine x64` | requires `permit cpu.asm`; see §8 |

Control never falls off the end of a procedure with a result type (`E0230: missing ret`).
A statement after `ret`, `fail`, `break`, `continue` or a `never` call is `E0231: unreachable`.

## 6. Procedures

- Parameters are immutable bindings in the procedure scope.
- A procedure with no `-> T` returns nothing; `ret` takes no operand.
- `-> never`: the body may not `ret`; the compiler verifies that control cannot
  reach the end (last statement is `loop` without `break`, a `never` call, `fail`, or a trap).
- Recursion is allowed. Procedures are not values in V0; `addr f` yields the
  code address of `f` as `addr u8` (for tables such as an IDT) and requires `memory.raw`.
- Clauses: `permit` (capabilities, §7); `calls sysv`/`c` (default), `calls none`
  (no prologue/epilogue; the body holds only hardware statements (§9.1) and
  `machine` blocks — no locals, no ordinary calls; no parameters; no result
  other than `never`), `calls interrupt` (V1: `E0900` in V0);
  `section "name"`; `align N` (power of two); `entry` (exactly one per program,
  on every target); `export ["sym"]`; `traps` (freestanding only; at most one;
  signature `(kind : core.TrapKind, site : core.Site) -> never`).
- **Entry point** (design 0016): the procedure carrying `entry` starts the
  program. Hosted: `-> s32` (the exit status), no parameters; the compiler adds
  the `_start` of `docs/ABI.md` §6. Freestanding: `-> never`, no parameters.
  The name `main` has no meaning.

## 7. Capabilities

| Capability | Gates |
|------------|-------|
| `memory.raw` | `[a]` loads/stores, `addr T (n)`, `uword(a)`, `addr f` of a procedure, `zone ... at`, `mem.*` on raw addresses |
| `memory.mmio` | creating `mmio` views/refs (`mem.mmio`, V1) |
| `io.port` | `port` methods (V1) |
| `cpu.asm` | `machine` blocks |
| `cpu.halt` | `cpu.halt()` |
| `cpu.interrupt`, `cpu.msr`, `cpu.control` | V1 intrinsics |
| `os.syscall` | `os.syscall(...)`; top-level `zone` in hosted mode |
| `cpu.control` | the hardware places and commands of §9.1 (`cpu.stack`, `cpu.frame`, `cpu.call`, `cpu.jump`); V1: `arch.x64.cr*`, `gdt`, `idt`, `segments` |

Permits are per procedure and not transitive. Module-level constant expressions
may form raw addresses (`VGA : addr u16 := addr u16 (0xB8000)`): a constant address
is a number, and the permit is required where it is dereferenced. In hosted mode `io.port`,
`cpu.halt`, `cpu.interrupt`, `cpu.msr`, `cpu.control` produce warning `W0100:
capability faults in user mode`. In freestanding mode `os.syscall` is `E0400`.
A gated operation without its permit is `E0401: operation requires permit X`.

## 8. Machine blocks

- `in REG <- e`: `e` is evaluated before the block and moved into `REG` (width from the
  register name; `e` converts to that width).
- `out REG -> p`: after the block `REG` is stored to place `p` (width must match); this counts
  as a definite store of `p`.
- Operands may name static places and procedures of the program (`lea rsp, [boot_stack + 16K]`,
  `call kernel.main`); the encoder resolves them like any other symbol.
- `clobber` lists every other register the block writes and `memory` if it reads or
  writes memory the compiler might hold in registers. Flags are always clobbered.
- Registers named in `in`/`out`/`clobber` are unavailable to the allocator across the block.
- Instruction lines are validated by the Oli-- encoder; an unknown mnemonic or an
  invalid operand form is `E0500`. Jumps may target only local labels of the same block.
- A block may not contain `ret`, `iretq`, `syscall`, `sysret`, `call` unless the enclosing
  procedure is `calls none` (then everything is allowed: the programmer owns the frame).
- The optimizer treats the block as a call with the declared effects; it never removes,
  duplicates or reorders it across other side effects.

## 9. Intrinsics (V0)

| Intrinsic | Signature | Permit | Cost |
|-----------|-----------|--------|------|
| `os.syscall(nr, a1..a6)` | `(word, word...) -> word`; `addr T` accepted for any argument; missing arguments are 0 | `os.syscall` | `SYSCALL` |
| `cpu.halt()` | `()`; executes `hlt` (returns after an interrupt) | `cpu.halt` | `KERNEL` |
| `cpu.pause()` | `()` | none | `ZERO` |
| `mem.copy(dst, src, n)` | `(rw view u8, view u8, uword)`; `n <=` both lengths checked; overlap allowed (move semantics) | none | `COPY` |
| `mem.set(dst, b)` / `mem.zero(dst)` | `(rw view u8, u8)` / `(rw view u8)` | none | `COPY` |
| `mem.secure_zero(dst)` | as `mem.zero`, never removed by any pass | none | `COPY` |
| `mem.get_u16/32/64(v)`, `mem.get_be16/32/64`, `mem.get_le*` | `(view u8) -> uN`; `v.len >= size` checked | none | `CHECK` |
| `mem.put_u16/32/64(v, x)`, `put_be*`, `put_le*` | `(rw view u8, uN)` | none | `CHECK` |
| `Name.size`, `Name.align` | compile-time `uword` | none | `ZERO` |
| `Name.at(v)` | `(view u8) -> ref Name` / `(rw view u8) -> rw ref Name` | none | `CHECK` |
| `v.len`, `v.addr` | `uword`, `addr T` | none | `ZERO` |
| `z.bytes(n)` | `(uword) -> rw view u8`, 16-byte aligned, zero-filled; traps on exhaustion | none | `ZONE` |
| `z.try_bytes(n)` | `-> rw view u8 or none` | none | `ZONE` |
| `z.make(T)` | `-> rw ref T`, aligned to `T.align`, zero-filled | none | `ZONE` |

### 9.1 Hardware places and commands (design 0015)

Everything the machine has is a place or a command. Hardware places are
stored to with `<-` and read like any place; hardware commands are called.
Every access is volatile, has cost class `KERNEL`, and is listed by `--explain`.
The namespaces `cpu`, `arch` and `port` are reserved; `arch.x64.*` compiles
only for x86-64.

| Place / command | Type | Permit | Where | Status |
|-----------------|------|--------|-------|--------|
| `cpu.stack` | `addr u8` (place) | `cpu.control` | `calls none` procedures only | V0 |
| `cpu.frame` | `addr u8` (place) | `cpu.control` | `calls none` procedures only | V0 |
| `cpu.call(proc)` | command; `proc` names an ordinary procedure | `cpu.control` | `calls none` procedures only | V0 |
| `cpu.jump(a : addr u8)` | command; never returns | `cpu.control` | `calls none` procedures only | V0 |
| `cpu.halt()`, `cpu.pause()` | commands | `cpu.halt` / none | anywhere | V0 |
| `cpu.id(leaf : u32) -> core.CpuId` | command (`cpuid`) | none | anywhere | V0 |
| `cpu.interrupts(on/off)`, `cpu.fence(order)`, `cpu.tsc()` | commands | `cpu.interrupt` / none | anywhere | V1 |
| `arch.x64.cr0/cr2/cr4/cr8 : u64`, `arch.x64.cr3 : physaddr` | places | `cpu.control` | anywhere | V1 |
| `arch.x64.msr[n : u32] : u64` | indexed place | `cpu.msr` | anywhere | V1 |
| `arch.x64.gdt`, `arch.x64.idt : ref T` (store only), `arch.x64.tr : u16` | places | `cpu.control` | anywhere | V1 |
| `arch.x64.segments(code: u16, data: u16)` | command | `cpu.control` | anywhere | V1 |
| `port.u8[n : u16] : u8`, `port.u16[n]`, `port.u32[n]` | indexed places | `io.port` | anywhere | V1 |
| `atomic.load/store/add/sub/and/or/xor/cas` | commands | none | anywhere | V1 |

A store of an `addr` into `arch.x64.cr3` is `E0203`; a hardware place used
outside its allowed context is `E0604`; a V1 place or command in V0 is `E0900`.
`machine` blocks remain the escape hatch for instructions without a command.

## 10. Compile-time evaluation

Module-level `:=` bindings are evaluated at compile time. A constant expression
may use literals, other constants, arithmetic in all modes, comparisons, bit
operators, conversions, `Name.size`/`align`, layout/array literals and
string literals. Overflow in a trapping constant expression is a compile error.
Constants of integer/bool type occupy no memory; aggregate constants live in `.rodata`.

## 11. Traps (dynamic errors)

`bounds`, `overflow`, `div_zero`, `align`, `zone_exhausted`, `unreachable`.
Hosted: message to fd 2, `exit_group(134)`. Freestanding: the `traps` procedure or `ud2`.

## 12. Diagnostic codes used by the V0 negative test suite

| Code | Meaning |
|------|---------|
| E0001–E0099 | lexical and syntax errors |
| E0100 | unknown name (also: no such variant) |
| E0101 | name shadows a visible name |
| E0102 | duplicate declaration, field, variant, parameter, alias or exported symbol |
| E0103 | `cpu`, `mem`, `os` or `core` declared as a top-level name |
| E0104 | imported module not found |
| E0105 | no such member / member not public |
| E0106 | cyclic constant |
| E0107 | not a compile-time constant (constant or static initializer) |
| E0109 | module name does not match its import path |
| E0110 | store to a binding (`x <- e` where `x` is `:=`) |
| E0111 | store through a read-only view/ref or into a constant |
| E0200 | type mismatch (also: `be`/`le` outside a layout, non-integer index) |
| E0201 | integer literal needs a type |
| E0202 | lossy conversion needs `wrap`/`sat`/`checked` |
| E0203 | mixed address spaces (`physaddr` with `addr`) |
| E0204 | layout or choice contains itself by value |
| E0205 | wrong number of arguments |
| E0206 | unknown, duplicate or missing named argument |
| E0207 | a call without a value used as a value |
| E0208 | missing, unknown or duplicate field in a literal or pattern |
| E0209 | array literal length mismatch |
| E0210 | expression is not callable |
| E0212 | constant expression overflows |
| E0213 | division by zero in a constant expression |
| E0220 | read of uninitialized place |
| E0230 | missing `ret` (or a reachable end in a `-> never` procedure) |
| E0231 | unreachable statement |
| E0232 | `break`/`continue` outside a loop |
| E0300 | view, ref or zone handle escapes its region |
| E0310 | unhandled failure |
| E0311 | `case` not exhaustive |
| E0330 | zone has no memory source (freestanding top-level zone without `at`/`from`) |
| E0400 | capability not available on this target |
| E0401 | operation requires permit |
| E0402 | unknown capability name |
| E0500 | invalid machine instruction, register or operand |
| E0600 | hosted program lacks `proc main -> s32` |
| E0601 | `entry`/`traps` on a hosted target |
| E0602 | zero or several `entry` (or several `traps`) procedures on a freestanding target |
| E0603 | invalid signature or body for `entry`, `traps` or `calls none` |
| E0604 | hardware place or command used outside a `calls none` procedure |
| E0900 | feature not implemented in this version |
| W0002 | binding, place or parameter never read (prefix with `_`) |
| W0003 | unreachable `case` arm |
| W0100 | privileged capability on a hosted target |

## 13. Not in V0 (the compiler says `E0900 feature not implemented`)

generics · `own` and `<~` · `port` methods · `mmio` creation · `calls interrupt` ·
atomics · floating point · procedure values · bitfields · compile-time `if` ·
`extern` declarations · threads · SIMD types · `zone` reset · module-level `if` ·
named result regions (`-> ref T in z`) · naming the `Overflow` type.

## 14. Revision history

| Date | Change | Reason |
|------|--------|--------|
| 2026-09-18 | V0 semantics written | Phase 0 |
| 2026-09-20 | §1 intrinsic namespaces merge with imports; `core` implicit; module self-reference; module naming rules | implementation of module loading |
| 2026-09-20 | §2 explicit conversion accepts unsigned to wider signed | `s32(x)` for `x : u8` is lossless |
| 2026-09-20 | §5 `zone ... from` accepts a `rw view u8` | static-array-backed zones (`docs/FREESTANDING.md`) |
| 2026-09-20 | §7 constant expressions may form raw addresses without a permit | kernels need address constants |
| 2026-09-20 | §12 diagnostic codes completed | negative test suite |
| 2026-09-20 | §6 `entry` is the entry point on every target; `main` has no meaning | design 0016 |
| 2026-09-20 | §9.1 hardware places and commands; `calls none` bodies may hold hardware statements | design 0015 |
