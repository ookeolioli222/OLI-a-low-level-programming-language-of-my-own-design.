# The commands of Oli-- — annotated reference

Every construct a programmer can write and every tool that acts on a program,
grouped by what it does to the machine: what it means, what it costs, an
example, and — for each one — whether it works today or is still to come.

Reviewed 2026-09-23 against `compiler/` (the `olic` compiler, written in
Oli--), `genesis/` and `genesis/test.sh`. Where this file and the code
disagree, the code is right and this file is stale; say so.

## How to read the status column

The status is about **`olic`**, the compiler of `compiler/`, because that is
the compiler the language will keep. Four words, in decreasing order of what
exists:

| Status | Meaning | Proof |
|--------|---------|-------|
| **runs** | `olic` compiles it to a native x86-64 executable and the executable behaves as the language says | a fixture under `tests/run/` or `tests/run/trap/`, run by `genesis/test.sh` layer 5 |
| **analysed** | `olic` parses it, resolves names, types every expression, checks every rule of `docs/LANGUAGE.md` it can — and then refuses to generate code for it with `E0900`, never approximating | the fixture corpus (`tests/parse`, `tests/sema`, `tests/snapshots`), layer 4 |
| **reserved** | the syntax is in V0 and parses, but the checker itself reports `E0900`: the feature belongs to V1 or V2 and V0 implements no part of it | `tests/sema/err/not_implemented.oli` |
| **planned** | specified in the design documents; nothing in the compiler knows it yet | the specification named in the row |

Some **analysed** constructs already *execute* under `oli1`, the genesis
compiler of the oli-core subset (`genesis/3-oli1/`), because `olic` is
written in that subset and `oli1` had to run it first. Those rows say
**analysed (runs under `oli1`)**: the semantics are pinned by real programs
(`genesis/3-oli1/tests/`), only `olic`'s own lowering of them is still to be
written. `docs/LANGUAGE.md` marks constructs by "covered by the test chain",
which counts `oli1`; this file marks them by `olic` alone.

**Cost classes** (from `docs/LANGUAGE_VISION.md` §7; `--explain-cost` will
print them): `ZERO` no instructions or only register moves · `CHECK` a
compare-and-branch a pass may remove with a proof · `STACK` frame space ·
`COPY` a memory copy of a stated size · `ZONE` a bump allocation · `CALL` a
procedure call · `SYSCALL` a kernel entry · `KERNEL` a privileged or device
instruction · `TRAP` a deliberate abort.

The guiding idea: **you can state the cost of any line without reading the
assembly.** Nothing allocates, copies, or calls invisibly.

---

## 1. The three memory operators — the core of the language

Where C, Rust and Zig all write `=`, Oli-- uses three different operators so
the reader always sees whether a line touches a register or memory, and
whether it copies or moves.

```oli
sum := a + b          -- bind: a name for a value, immutable, no address
counter : u32 <- 0    -- place: a typed slot in memory, initialised
counter <- counter + 1  -- store: into an existing place
buf[i] <- 0             -- store: into an element of a view
hdr.len <- 20           -- store: into a field through a ref
```

| Command | Meaning | Cost | Status |
|---------|---------|------|--------|
| `name := expr` | bind a name to a value; it may live in a register and has no address | ZERO | **runs** |
| `name : T` / `name : T <- expr` | declare a place of a scalar, a view or a zone; `<-` initialises it | STACK | **runs** |
| `name : [N]T` | declare an aggregate place (an array in the frame) | STACK | analysed |
| `place <- expr` into a local | store | ZERO | **runs** |
| `v[i] <- expr` | store into a view element, bounds-checked | CHECK + ZERO | **runs** |
| `r.f <- expr` | store into a field through a `ref`, at the field's width | ZERO | **runs** |
| `[p] <- expr` | raw store through an `addr` (`permit memory.raw`) | ZERO | analysed (runs under `oli1`) |
| `place <~ expr` | move an `own` value; the source becomes unusable | ZERO | reserved (V1) |
| `addr x` | the raw address of a place | ZERO | analysed |
| `ref x` / `rw ref x` | a safe reference to one live object — a local keeps its frame words once its address is taken, a field is the address of that part of the record | ZERO | **runs** |
| `[p]` | raw load through an `addr` (`permit memory.raw`) | ZERO | analysed (runs under `oli1`) |

### Views — `(address, length)` over existing memory, never a copy

```oli
head := packet[0..20]    -- ZERO: a sub-view, one range check, no bytes moved
b    := packet[i]        -- CHECK then load: `i < packet.len` is verified
n    := packet.len       -- ZERO
a    := packet.addr      -- ZERO: the first byte's address
```

| Command | Meaning | Cost | Status |
|---------|---------|------|--------|
| `v[i]` | element load, bounds-checked; the element is any integer type, loaded at its width | CHECK + ZERO | **runs** |
| `v[a..b]`, `v[..b]`, `v[a..]` | a subview; one `check.range` proves `a <= b <= len` | CHECK | **runs** |
| `v.len`, `v.addr` | the two halves of the pair | ZERO | **runs** |
| a view as a parameter, a result, a local, a string literal | the pair travels in two words (`rax:rdx` for a result) | ZERO | **runs** |
| `v[i]` where the element is a `layout` | a view of records | CHECK | analysed |

The bounds check is an instruction, and it leaves only with a proof: a
constant index below a constant length, or an index below a dominating
`i < len` — the loop of `each` and of `while i < v.len` — is removed by
`compiler/opt.oli` and printed where it stood by `--show-oir=opt`.

---

## 2. Zones — memory without a garbage collector or per-object free

A `zone` is a lexically scoped bump region. You carve buffers and objects out
of it; at `end` the whole region is released in one step. The memory source
is always explicit.

```oli
zone scratch 64K              -- hosted: one mmap (SYSCALL); freed whole at `end`
    pkt := scratch.bytes(4096)   -- ZONE: 4096 rw bytes, zeroed
    hdr := scratch.make(Header)  -- ZONE: one Header, aligned, zeroed -> rw ref Header
end                           -- everything from `scratch` dies here
```

| Command | Meaning | Cost | Status |
|---------|---------|------|--------|
| `zone z N ... end` from the operating system | one anonymous `mmap`, rounded to a page; `munmap` at `end`; the mapping the kernel refuses traps `zone_exhausted` | SYSCALL | **runs** |
| `z.bytes(n)` | `n` zeroed bytes → `rw view u8`; the cursor is rounded to 16; traps `zone_exhausted` past the limit | ZONE | **runs** |
| `z` passed to a `zone` parameter | the handle is the address of the (base, cursor, limit) triple | ZERO | **runs** |
| `z.try_bytes(n)` | as `bytes`, but `→ rw view u8 or none` instead of trapping | ZONE | analysed |
| `z.make(T)` | one zeroed `T` → `rw ref T`, at the layout's alignment | ZONE | **runs** |
| `zone z N at ADDR` | a zone over raw memory at an address (`permit memory.raw`); nothing is released at `end` | ZERO | analysed (runs under `oli1`) |
| `zone z N from parent` / `from buffer` | carved from an enclosing zone or a static array | ZONE | analysed |
| a `zone` inside a `zone` | the inner one is carved from the outer | ZONE | analysed |
| `ret` inside a zone | in the entry procedure: `exit_group` releases everything | — | **runs** |
| `ret`, `break`, `continue` that would leave a zone elsewhere | must release the zone on that edge first | SYSCALL | analysed (the back end refuses rather than leak) |

---

## 3. Reinterpreting bytes — layouts over a view

`T.at(v)` reads a byte view as a structured layout with zero copy (one
alignment + bounds check). `T.size` / `T.align` are compile-time constants.

```oli
hdr := Header.at(pkt)     -- CHECK: rw ref Header over pkt's bytes, no copy
if pkt.len < Header.size then fail too_short
```

| Command | Meaning | Cost | Status |
|---------|---------|------|--------|
| `layout NAME [packed] [align N] ... end` | a record with a fixed, documented layout: integer, view, ref, address and by-value layout fields, `align N` on a field | — | **runs** (`tests/run/layouts.oli`) |
| fields of type `be T` / `le T` | an integer stored in a given byte order | ZERO | analysed |
| `T.size`, `T.align` | compile-time constants | ZERO | **runs** |
| `T.at(v)` | a `ref T` over a view; `check.range` traps `bounds` when short, `check.align` traps `misaligned` when not aligned (unless `packed`) | CHECK | **runs** |
| `r.f` | a field load at the field's width, sign- or zero-extended; a view field as its two words; a by-value layout field as the address of that part | ZERO | **runs** |
| `choice NAME ... end` | a tagged union of variants with optional payload | — | analysed |
| `tag`, `payload` access on a `choice` | — | ZERO | analysed |

---

## 4. Declarations and contracts

A procedure header is a **contract**: not just types, but what hardware it may
touch, its calling convention and where it lives.

```oli
proc checksum(data : view u8) -> u32          -- a function: params in, result out
    total : u32 <- 0
    each b in data
        total <- wrap(total + b)
    end
    ret total
end

proc write_all(fd : s32, data : view u8) -> uword or os.Error
    permit os.syscall                          -- capability: may enter the kernel
    ...
end
```

| Command | Meaning | Status |
|---------|---------|--------|
| `module a.b` | one file = one module; `cpu`/`mem`/`os` always in scope, `core` implicit | **runs** |
| `import a.b [as x]`, `pub` | imports resolve under `lib/`, names are checked across modules | analysed as a whole program; no run fixture imports a module yet |
| `proc NAME(params) -> T ... end` | a procedure: arguments in the six SysV registers, then on the stack right to left; a view that does not fit in the registers left goes to the stack whole and the next integer still takes a register (the SysV rule, `docs/ABI.md` §1–2); one result (a view: two words) | **runs** (`tests/run/args.oli`) |
| `permit cap, ...` | capabilities the body may use; `os.syscall`, `memory.raw` and `cpu.asm` are enforced by the checker (`E0401` without them) | **runs** (`os.syscall`) / analysed (the rest) |
| `calls sysv` | the default convention | analysed |
| `calls none` | no prologue (boot code) | analysed |
| `calls interrupt` | an interrupt handler | reserved (V1) |
| `section "x"`, `align N`, `export ["sym"]` | placement and linkage | analysed (the ELF writer emits one segment and no symbol table yet) |
| `entry` | the program's start; `-> s32` hosted (the result is the exit status), `-> never` freestanding. There is no `main` | **runs** |
| `traps` | the procedure that receives traps (freestanding) | analysed |
| `NAME := expr` | a module-level constant | **runs** |
| `NAME : T := expr` (aggregate constant) | a constant that lives in `.rodata` | analysed |
| `NAME : T [<- expr]` at module level | a static place in `.bss`/`.data` | analysed |

---

## 5. Control flow

```oli
if a > b then ret a           -- one-line form (no else)
if n < 0                      -- block form
    tag <- 'n'
elif n == 0
    tag <- 'z'
else
    tag <- 'p'
end
while i < n ... end           -- pre-tested loop
loop ... break ... end        -- infinite until break/ret/fail
each x in view ... end        -- iterate a view
each i in 0..n ... end        -- iterate integers a..b-1
```

Errors are values, not exceptions — there is no hidden control flow:

```oli
proc parse(v : view u8) -> ref Header or ParseError
    if v.len < Header.size then fail too_short   -- fail = the error exit
    ret Header.at(v)                             -- ret = the success exit
end
hdr := parse(pkt) else ret 1     -- resolve a fallible value: `else fail | ret | default`
case parse(pkt)                  -- or match it exhaustively
when ok h        ...
when fail too_short ...
end
```

| Command | Meaning | Status |
|---------|---------|--------|
| `if/elif/else/end`, `if c then s` | branch (the one-line form has no else) | **runs** |
| `while`, `loop`, `break`, `continue` | loops | **runs** |
| `each x in v` | iterate a view; the bounds check is emitted and removed by the loop-bound proof | **runs** |
| `each i in a..b`, `each x in array_place` | iterate an integer range, or an array in the frame | analysed |
| `ret [e]` | the success exit; in the entry procedure, the exit status; in a `T or E` procedure, the pair (tag 0, e) | **runs** |
| `fail [e]` | the failure exit of a `T or E` procedure: the pair (tag 1, e), or (1, 0) for `none` | **runs** (`tests/run/fallible.oli`) |
| `e else fail` / `e else ret [v]` / `e else default` | resolve a fallible value: pass the failure on, leave, or take a default (a phi) | **runs** |
| `case e when ok [x] ... when fail ... end`, with `else` for either arm | match a fallible value whose `E` is an integer type or `none` | **runs** |
| `fail VARIANT {…}`, `case … when fail VARIANT` | a `choice` error, built and matched | analysed |
| `case e when variant ... else ... end` | match a `choice`, exhaustively | analysed |
| `machine x64 ... end` | inline machine code with declared inputs, outputs and clobbers (see §8) | analysed (assembled and run by genesis `asm` — `oli1` itself is written in these blocks) |

---

## 6. Arithmetic, overflow modes and conversions

Plain arithmetic **traps** on overflow — never the silent wraparound or
undefined behaviour of C. You opt into other behaviour explicitly, per
expression.

```oli
x + y            -- traps on overflow: `trap: overflow at file:line`, exit 134
wrap(x + y)      -- two's-complement wraparound (hashes, crypto)
sat(x + y)       -- saturating (clamps to the type's range)
checked(x + y)   -- yields `T or Overflow`, handled with else/case
```

| Command | Meaning | Cost | Status |
|---------|---------|------|--------|
| `+ - * / %` | trapping arithmetic at the width of the type; `/` and `%` trap on a zero divisor, `/` on MIN/-1 (`%` answers 0 there) | CHECK | **runs** |
| `- x` | negation; traps for an unsigned value other than 0 and for signed MIN | CHECK | **runs** |
| `wrap(e)` | every operator inside `e` wraps at its width; the two division traps stay | ZERO | **runs** |
| `sat(e)`, `checked(e)` | saturating / fallible arithmetic | CHECK | analysed |
| `== != < <= > >=` | comparisons → `bool`; they do not chain | ZERO | **runs** |
| `and`, `or`, `not` | boolean, short-circuiting | ZERO | **runs** |
| `& \| ^ ~ << >>` | bit operations on equal integer types; a shift count is masked to the width | ZERO | **runs** |
| `T(x)` | a lossless widening; emits nothing | ZERO | **runs** |
| `T.wrap(x)`, `T.bits(x)` | truncate to the width of `T`, wrapping / reinterpret at the same width | ZERO | **runs** |
| `T.sat(x)`, `T.checked(x)` | saturating / fallible narrowing | CHECK | analysed |
| `physaddr(n)`, `addr T (n)` | an integer as a physical / raw virtual address (`permit memory.raw` for the latter) | ZERO | analysed |
| an implicit narrowing | `E0202`: the conversion must be written | — | **runs** (as a diagnostic) |

The passes of `docs/OIR_SPEC.md` §6 decide constant expressions at compile
time, turn a branch on a constant into a jump, and fold an operation whose
constant answer its type cannot hold into the trap it always took; every
check they remove is printed with its proof by `--show-oir=opt`.

---

## 7. Intrinsics (always in scope)

| Command | What it does | Permit | Cost | Status |
|---------|--------------|--------|------|--------|
| `os.syscall(nr, a1..a6)` | a raw Linux system call — this is how `hello` prints without libc | `os.syscall` | SYSCALL | **runs** |
| `cpu.halt()`, `cpu.pause()` | `hlt` / `pause` | `cpu.halt` / — | KERNEL / ZERO | analysed |
| `mem.copy(dst, src, n)` | copy `n` bytes (overlap allowed) | — | COPY | analysed |
| `mem.set(dst, b)`, `mem.zero(dst)` | fill / zero a byte view | — | COPY | analysed |
| `mem.secure_zero(dst)` | zero that no pass may delete (wipes a secret) | — | COPY | analysed |
| `mem.get_u16/32/64`, `get_be*`, `get_le*` | read an integer of a given width and endianness from bytes | — | CHECK | analysed |
| `mem.put_u16/32/64`, `put_be*`, `put_le*` | write one | — | CHECK | analysed |
| `mem.mmio(...)` | memory-mapped I/O access | `memory.mmio` | KERNEL | reserved (V1) |

---

## 8. Hardware places and commands (design 0015)

The machine's own state is reached with the *same* operators as ordinary
memory: `<-` stores into a register, a call runs a command.

```oli
cpu.stack <- addr boot_stack + 16K   -- the stack pointer is a place
cpu.call(main)                       -- transfer control
id := cpu.id(0)                      -- cpuid -> core.CpuId { a, b, c, d }
arch.x64.cr3 <- page_table           -- V1: a control register (physaddr-typed)
port.u8[0x3F8] <- b                  -- V1: port I/O as an indexed place
```

| Command | Permit | Status |
|---------|--------|--------|
| `cpu.stack`, `cpu.frame`, `cpu.call`, `cpu.jump` | `cpu.control` | planned (`docs/design/0015-hardware-commands.md`) |
| `cpu.id(leaf)` | — | planned |
| `cpu.interrupts(on/off)`, `cpu.fence(order)`, `cpu.tsc()` | `cpu.interrupt` / — | planned (V1) |
| `arch.x64.cr0/2/3/4/8`, `msr[n]`, `gdt`, `idt`, `tr`, `segments()` | `cpu.control` / `cpu.msr` | planned (V1) |
| `port.u8/u16/u32[n]`, the `port T` type | `io.port` | reserved (V1) |
| `atomic.load/store/add/sub/and/or/xor/cas(ref, ..., order)` | — | planned (V1) |
| `machine x64 ... end` with `in`, `out`, `clobber` | `cpu.asm` | analysed (assembled and run by genesis `asm`) |

Every hardware access is volatile, cost class `KERNEL`, and will be listed by
`--explain`.

---

## 9. Types — the vocabulary of the machine

| Type | Meaning | Status |
|------|---------|--------|
| `u8 u16 u32 u64` / `s8 s16 s32 s64` | integers of fixed width (`s`, not `i`); arithmetic traps at the width | **runs** |
| `word` / `uword` / `byte` / `bool` | machine word (= s64/u64), `u8`, boolean | **runs** |
| `view T` / `rw view T` with an integer `T` | `(addr, len)` over many — the everyday memory handle | **runs** |
| `zone` | a zone handle | **runs** |
| `addr T` | a raw virtual address; as a value (e.g. `v.addr` handed to a syscall) it runs, dereferencing needs `memory.raw` | **runs** as a value / analysed as a place |
| `view T` of a `layout` | a view of records | analysed |
| `ref T` / `rw ref T` | reference to one object — safe, writable; one word | **runs** |
| `[N]T` | fixed array (a place/static type, not a value) | analysed |
| `layout` names | nominal record types | **runs** |
| `choice` names | nominal tagged unions | analysed |
| `T or E`, `T or none` | a fallible value / an optional; `T` an integer or a ref, `E` an integer type or `none` | **runs** |
| `T or E` with a `choice` `E`, or a view `T` | the error a tagged union, or a payload that needs `sret` | analysed |
| `never` | a procedure that does not return | analysed |
| `physaddr` | a physical address — never dereferenced; cannot mix with `addr` | analysed |
| `be T` / `le T` | an integer stored big/little-endian, in a `layout` | analysed |
| `mmio ref T` / `mmio view T`, `port T`, `own T` | volatile device memory, port I/O, ownership | reserved (V1) |
| `f32 f64` | floating point | reserved (V2) |

Type identity is nominal for `layout`/`choice`, structural for everything
else. Every type's size, alignment and field offsets are fixed and
documented; the compiler never reorders fields.

---

## 10. The compiler and its tools

The compiler exists as one binary per stage: `genesis/build/olic` and the
`genesis/build/show_*` drivers, all built by `./genesis/test.sh`, all reading
the source on stdin. The flags below name the same operations; they become
flags of one binary when the compiler can read its command line (the `oli`
tool).

| Command | What it does | Status |
|---------|--------------|--------|
| `genesis/build/olic < f.oli > f` | build a static native ELF64 — own x86-64 encoder, own ELF writer, no libc, no linker; exits 1 with diagnostics, 4 on an internal verifier failure, and writes no file unless the whole pipeline agreed | **runs** |
| `olic f.oli [-o out]`, `olic --check f.oli`, `olic --show-STAGE f.oli` | the same compiler with a command line: `bin/olic`, a shell script on the PATH that picks the driver and redirects (`tools/vscode-oli/README.md` §2). It stands in until an Oli-- program can read `argv` | **runs** (a wrapper outside the toolchain) |
| `show_tokens` (`--show-tokens`) | one token per line | **runs** |
| `show_ast` (`--show-ast`) | the syntax tree as an S-expression; `tests/snapshots/*.ast` | **runs** |
| `show_sema` (`--show-sema`) | the semantic graph — items, signatures, locals, a type and a region on every expression — and every check; `tests/snapshots/*.sema` | **runs** |
| `show_oir` (`--show-oir`) | the OIR in basic blocks before any pass, verified against `OIR_SPEC` §8; `tests/snapshots/*.oir` | **runs** |
| `show_ssa` (`--show-ssa`) | after `mem2reg`: places promoted to values, a phi where two definitions meet; `*.ssa` | **runs** |
| `show_opt` (`--show-oir=opt`) | after the passes: constants folded, dead code gone, every removed check printed where it stood with its proof, statics nothing names marked removed; `*.opt` | **runs** |
| `verify_check` | the verifier's own test: breaks the OIR of a program ten ways, each must be rejected with the invariant it violates | **runs** |
| `--show-machine-ir`, `--show-asm`, `--show-bytes` | the lowering as MIR-x64, as assembly, as bytes | planned (the lowering exists; the printers do not) |
| `--explain` | per procedure: frame size, register assignment, allocations, copies, views, checks and which were removed, syscalls, capabilities | planned (`docs/OIR_SPEC.md` §1) |
| `--explain-cost` | the cost class of every line | planned |
| `--check` / `--check-syntax` | analyse / parse only | planned as flags; `show_sema`/`show_ast` with output discarded do it today |
| `--freestanding` | target with no operating system: no `mmap` zones, `-> never` entry, `traps` | analysed (`tests/sema/ok/freestanding.oli`); no driver selects it yet |
| `--lib DIR` | where imported modules are found | planned; `lib/` under the working directory today |
| `oli new/build/run/test/fmt/check/bench/doc/package/fuzz` | the project tool | planned |
| `./genesis/test.sh` | build the whole chain from 322 hand-written bytes and run every test, layers 0–5 | **runs** |
| `.vscode/tasks.json`, `tools/vscode-oli/` | build, run and inspect the current file from VS Code; syntax highlighting | **runs** (a convenience outside the toolchain; `olis`/`olide` of `docs/IDE_PLAN.md` replace it) |

### What `olic` refuses today, in one list

Everything marked *analysed* above is reported as `E0900` by the back end,
with the position of the construct, and no file is written. As of this
review that is: `[N]T` places, raw `[p]` access, `z.try_bytes`, `at`/`from`
zones and a zone inside a zone, a jump out of a zone in a non-entry
procedure, `each` over a range or an array, a view of records, `choice` (and
a `T or E` whose `E` is one), a view inside a `T or E`, `be`/`le` fields,
`fail` and every form of `T or E`, `case`, `machine` blocks, `sat`/`checked`
in both forms, `physaddr(n)`/`addr T (n)`, the `cpu.*` and `mem.*`
intrinsics, an `os.syscall` with more than seven words (the number and six
arguments are all the registers a system call has), aggregate constants and
statics.
The front end already checks all of them, so a program using them is
type-checked before it is refused.

### Error-handling plan

Oli-- has three deliberately different kinds of failure. They must not be
collapsed into one exception mechanism, because each has a different cost and
different recovery point.

| Kind | Example | Recovery | Planned proof |
|------|---------|----------|---------------|
| **source error** | missing `end`, unknown name, type mismatch, missing `permit` | the compiler reports it; no native file is written | exact diagnostic fixtures and parser-recovery tests |
| **fallible result** | short input, invalid header, failed allocation or syscall | the procedure returns `T or E`; the caller must use `else` or exhaustive `case` | `olic` lowering of `fail`, `else` and `case`, then library APIs using them |
| **trap** | overflow, division by zero, bounds violation, exhausted zone | deterministic termination through `core.trap`; no recovery in hosted V0 | trap message, source position and exit status fixtures |

The implementation order is:

1. **Compiler boundary:** keep lexing and parsing recovery, collect multiple
    diagnostics per file, attach a stable code, primary span, notes and a
    suggested fix, and suppress output until the complete pipeline succeeds.
    A malformed file must never leave a misleading executable behind.
2. **Semantic boundary:** finish the typed `T or E` representation and make
    `fail`, `else fail`, `else ret`, `else default` and exhaustive `case` work
    in `olic`, with diagnostics for discarded or unhandled failures.
3. **Runtime boundary:** standardize `core.trap` records: kind, source file,
    line and column, then keep hosted exit `134` and stderr output stable. A
    freestanding target may replace the handler through `traps`, but may not
    silently continue after a violated memory or arithmetic invariant.
4. **Tool boundary:** add machine-readable diagnostics to `oli check/build`
    and the planned `olis` protocol. Human output remains concise and
    clickable; structured output carries code, severity, span, notes and
    fix-its.
5. **Test boundary:** add positive and negative fixtures for every recovery
    path, truncated/random input for parser and protocol robustness, and tests
    that assert no output file exists after any diagnostic or backend error.

This plan is partly present today: parser recovery and exact `E0xxx`
diagnostics run in the front end, and arithmetic/memory traps run in the
backend. The missing milestone is the native `olic` implementation of
fallible results and their propagation; until then those constructs remain
analysed or `E0900`, rather than being approximated with hidden exceptions.

---

## 11. Genesis tools (the bootstrap — not language syntax)

These make the language exist without any other compiler (design 0017).

| Tool | Written in | Purpose | Status |
|------|-----------|---------|--------|
| `hex0` | 322 hand-encoded bytes | hex listing → bytes; reproduces itself | done |
| `hex2` | hex0 notation | hex with labels and relative/absolute fixups | done |
| `asm` | hex2 notation | assembles Oli-- `machine x64` blocks into ELF64: r32/r64 encoder, ModRM/SIB/REX operands, two-pass symbols, read-only data | suite green; narrow operands, the remaining memory forms, writable data and multi-segment ELF pending |
| `oli1` | `machine x64` blocks (assembled by `asm`) | compiles the oli-core subset of Oli--: locals, expressions, strings, control flow, syscalls, procedures, zones, views, raw memory, layouts, refs, fallible results, `case`, constants, typed places, `loop`, explicit conversions | done, steps 0–6f; its output region holds 2 MiB of code |
| `olic` | oli-core Oli-- (`compiler/`), compiled by `oli1` | the full compiler: the V0 front end complete for every rule it checks; the back end lowers the **runs** rows above through OIR, SSA, the passes and the verifier | **self-hosting: `stage2 == stage3`** — `olic` compiles its 17 modules into a compiler that compiles them to the same bytes (`genesis/test.sh` layer 6); the *analysed* rows are what it does not lower yet, for itself as for any program |

---

## 12. Ecosystem libraries (written in Oli--, after self-hosting)

Not commands of the language, but the libraries the commands above are for
(`docs/ecosystem/`, design 0018): **oli.compute** (GPU-first tensors / AI /
HPC) and **oli.sec** (authorized security research). Both are deferred until
the self-hosted compiler and the needed language features (SIMD, `own`,
atomics, FFI, GPU target; net/binary stdlib) exist.
