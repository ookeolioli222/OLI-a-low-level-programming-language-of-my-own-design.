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

**Cost classes** (from `docs/LANGUAGE_VISION.md` §7; `olic --explain` prints
them per line): `ZERO` no instructions or only register moves · `CHECK` a
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
| `name : T` / `name : T <- expr` | declare a place of a scalar, a view or a zone; `<-` initialises it. At module level a static: an integer, an array or a layout, initialised by a constant, `{ a, b, … }` element by element or `Name { f: c, … }` field by field (`tests/run/aggregates.oli`) | STACK | **runs** |
| `name : [N]T` in a procedure | an array in the frame: zero on declaration, read as the writable view of its bytes | STACK | **runs** (`tests/run/frames.oli`) |
| `place <- expr` into a local | store | ZERO | **runs** |
| `v[i] <- expr` | store into a view element, bounds-checked | CHECK + ZERO | **runs** |
| `r.f <- expr` | store into a field through a `ref`, at the field's width | ZERO | **runs** |
| `[p] <- expr` | raw store through an `addr`, at the width of its element type (`permit memory.raw`) | ZERO | **runs** (`tests/run/statics.oli`) |
| `place <~ expr` | move an `own` value; the source becomes unusable | ZERO | reserved (V1) |
| `addr x` | the raw address of a place | ZERO | analysed |
| `ref x` / `rw ref x` | a safe reference to one live object — a local keeps its frame words once its address is taken, a field is the address of that part of the record | ZERO | **runs** |
| `[p]` | raw load through an `addr` (`permit memory.raw`) | ZERO | **runs** |

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
| `z.try_bytes(n)` | as `bytes`, but `→ rw view u8 or none` instead of trapping: the cursor and the limit are compared in the OIR, the cursor moves only on the ok path, and `else` / `case` resolve the view (`tests/run/zones.oli`) | ZONE | **runs** |
| `z.make(T)` | one zeroed `T` → `rw ref T`, at the layout's alignment | ZONE | **runs** |
| `zone z N at ADDR` | a zone over raw memory at an address (`permit memory.raw`); `zone.new.raw` in OIR, nothing is released at `end` | ZERO | **runs** |
| `zone z N from parent` / `from buffer` | carved from an enclosing zone (`zone.new.from`: the parent's `zone_exhausted` trap; at `end` the parent's cursor is back where it was) / laid over a buffer whose length is proved to hold `N` (`check.range`, then `zone.new.raw`) | ZONE | **runs** |
| a `zone` inside a `zone` | each has its own source: two mappings when both are from the operating system, or `from` the outer one when written so | ZONE / SYSCALL | **runs** |
| `ret`, `fail`, `break`, `continue` inside a zone | the compiler releases every zone the jump leaves, innermost first, on that edge — `zone.end` (`munmap`) or `zone.end.from` — before the jump; a zone is never leaked and never released twice | SYSCALL / ZONE | **runs** |

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
| a layout held by value: `p : Name <- Name { f: e, … }`, `q : Name <- p`, `r.f <- p`, `f(p)`, `-> Name` | the bytes live in the frame (or the record); a literal is a fresh area written field by field; a copy is exact (words, then 4, 2, 1); a parameter or result of sixteen bytes or less travels in registers (SysV INTEGER class), a wider parameter on the stack and copied into the frame on entry, a wider result through `sret` (the caller's area, its address as a hidden first argument and back in rax); a view or array of layouts reaches an element by its address (`v[i]`, `v[i] <- p`, `each e in v`) | COPY / ZERO | **runs** (`tests/run/records.oli`) |
| fields of type `be T` / `le T` | an integer stored in a given byte order: a `be` field is byte-swapped after the load and before the store (shifts and masks; no `bswap` instruction yet), a `le` one is the machine's own order | ZERO | **runs** (`tests/run/bytes.oli`) |
| `T.size`, `T.align` | compile-time constants | ZERO | **runs** |
| `T.at(v)` | a `ref T` over a view; `check.range` traps `bounds` when short, `check.align` traps `misaligned` when not aligned (unless `packed`) | CHECK | **runs** |
| `r.f` | a field load at the field's width, sign- or zero-extended; a view field as its two words; a by-value layout field as the address of that part | ZERO | **runs** |
| `choice NAME ... end` | a tagged union of variants with optional payload; one that fits eight bytes travels as its memory image in one word — the tag (variant number from 0, in declaration order) in the first byte, each field at its offset | — | **runs** when ≤ 8 bytes; a wider one is E0900 |
| `tag`, `payload` access on a `choice` | — | ZERO | reserved |

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
| `calls none` | no prologue, no frame: the body is `machine` blocks alone, without `in`/`out` (boot code) | **runs** (`tests/run/freestanding.oli`) |
| `calls interrupt` | an interrupt handler (`permit cpu.interrupt`): the prologue pushes every general register but rsp and rbp, the one parameter is `ref core.x64.InterruptFrame` — the rip, cs, rflags, rsp and ss the CPU pushed — and the return is `iretq`; no result | **runs** (`tests/run/interrupt.oli` enters one through a frame pushed by hand; `examples/kernel.oli` installs one on vector 3 and reaches it with `int 3`) |
| `calls interrupt` | an interrupt handler | reserved (V1) |
| `section "x"`, `align N`, `export ["sym"]` | placement and linkage: `section ".text.boot"` places a procedure first in the code and a static in the read-only segment in front of the code (as does `".rodata"`), `align N` on a static is honoured (the natural alignment otherwise); other sections and `export` are recorded but not placed (the ELF writer emits two segments and no symbol table) | **runs** for those; the rest analysed |
| `entry` | the program's start; `-> s32` hosted (the result is the exit status), `-> never` freestanding. There is no `main` | **runs** |
| `traps` | the procedure that receives traps in a freestanding program: `(kind : core.TrapKind, site : core.Site) -> never`; every trap site jumps to a routine that passes the kind and builds the `Site` (file, line) on the stack; without one a trap is `ud2` | **runs** (`tests/freestanding/trap_line.oli` exits with the line of its overflow) |
| `NAME := expr` | a module-level constant | **runs** |
| `NAME : T := expr` (aggregate constant) | a constant that lives in `.rodata` | analysed |
| `NAME : T [<- expr]` at module level | a static place: with an initialiser its bytes are in the file (`.data`), without one it is zero memory (`.bss`); the image gets a second, read+write segment | **runs** |
| `NAME : [N]T` at module level | a static array, read as the view of its bytes: `.len`, `[i]`, `each`, passed where a view is | **runs** |
| a static with an aggregate initialiser `:= { … }` | a constant in `.rodata` | analysed |

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
| `each i in a..b`, `each x in array_place` | iterate an integer range (`i` from `a` while below `b`, both `uword`), or an array in the frame | **runs** (`tests/run/records.oli`, `frames.oli`) |
| `ret [e]` | the success exit; in the entry procedure, the exit status; in a `T or E` procedure, the pair (tag 0, e) | **runs** |
| `fail [e]` | the failure exit of a `T or E` procedure: the pair (tag 1, e), or (1, 0) for `none` | **runs** (`tests/run/fallible.oli`) |
| `e else fail` / `e else ret [v]` / `e else default` | resolve a fallible value: pass the failure on, leave, or take a default (a phi) | **runs** |
| `case e when ok [x] ... when fail ... end`, with `else` for either arm | match a fallible value whose `E` is an integer type or `none` | **runs** |
| `fail VARIANT`, `fail VARIANT {…}`, `case … when fail VARIANT { f }` | a `choice` error: the image built with masks and shifts, the tag byte compared arm by arm, each named field read back at its width and sign (`tests/run/choice.oli`) | **runs** (choice ≤ 8 bytes) |
| `case e when variant [{ f }] ... else ... end` | match a plain `choice` value (its tag byte, the fields bound), a `bool` (`when true` / `when false`) or an integer (`when 3`, `when - 1`), exhaustively or with `else` | **runs** (`tests/run/records.oli`) |
| `machine x64 ... end` | inline machine code with declared inputs, outputs and clobbers (see §8); `olic` assembles it with its own encoder (`compiler/asm.oli`) — the genesis assembler's subset byte for byte (`tests/machine/*.hex`) plus `hlt`, `cli`, `sti`, `nop`, `iretq`, `cpuid`, `rdmsr`, `wrmsr`, `rdtsc`, `pause`, `lgdt`, `lidt`, control registers — a line it does not know is `E0900` | **runs** (`tests/run/machine.oli`) |

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
| `sat(e)`, `checked(e)` | saturating / fallible arithmetic: every operator inside `e` clamps to its type's range / `checked(e)` is `T or Overflow` for `else` and `case` (`tests/run/saturate.oli`) | CHECK | **runs** |
| `== != < <= > >=` | comparisons → `bool`; they do not chain | ZERO | **runs** |
| `and`, `or`, `not` | boolean, short-circuiting | ZERO | **runs** |
| `& \| ^ ~ << >>` | bit operations on equal integer types; a shift count is masked to the width | ZERO | **runs** |
| `T(x)` | a lossless widening; emits nothing | ZERO | **runs** |
| `T.wrap(x)`, `T.bits(x)` | truncate to the width of `T`, wrapping / reinterpret at the same width | ZERO | **runs** |
| `T.sat(x)`, `T.checked(x)` | saturating / fallible narrowing: clamp to the range of `T` / `T or Overflow` | CHECK | **runs** |
| `physaddr(n)`, `addr T (n)` | an integer as a physical / raw virtual address (`permit memory.raw` for the latter) | ZERO | analysed |
| an implicit narrowing | `E0202`: the conversion must be written | — | **runs** (as a diagnostic) |
| a wrong field in a literal or pattern | `E0208`: a field the layout or variant does not declare, one named twice, or one left out of a literal (`tests/sema/err/fields.oli`) | — | **runs** (as a diagnostic) |

The passes of `docs/OIR_SPEC.md` §6 decide constant expressions at compile
time, turn a branch on a constant into a jump, and fold an operation whose
constant answer its type cannot hold into the trap it always took; every
check they remove is printed with its proof by `--show-oir=opt`.

---

## 7. Intrinsics (always in scope)

| Command | What it does | Permit | Cost | Status |
|---------|--------------|--------|------|--------|
| `os.syscall(nr, a1..a6)` | a raw Linux system call — this is how `hello` prints without libc | `os.syscall` | SYSCALL | **runs** |
| `cpu.halt()`, `cpu.pause()` | `hlt` / `pause`, one instruction each (`cpu.halt` in OIR) | `cpu.halt` / — | KERNEL / ZERO | **runs** |
| `mem.copy(dst, src, n)` | copy `n` bytes (overlap allowed) | — | COPY | analysed |
| `mem.set(dst, b)`, `mem.zero(dst)` | fill / zero a byte view | — | COPY | analysed |
| `mem.secure_zero(dst)` | zero that no pass may delete (wipes a secret) | — | COPY | analysed |
| `mem.get_u16/32/64`, `get_be*`, `get_le*` | read an integer of a given width and endianness from a view: `check.range` that the view holds it (`bounds`), one load, a byte swap for `be` | — | CHECK | **runs** (`tests/run/bytes.oli`) |
| `mem.put_u16/32/64`, `put_be*`, `put_le*` | write one, the same way | — | CHECK | **runs** |
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
| `machine x64 ... end` with `in`, `out`, `clobber` | `cpu.asm` | **runs**: `in REG <- e` loads the value before the block, `out REG -> place` stores the register after it, a callee-saved register the block names (rbx, r12–r15) is kept for the caller; `.label:` and jumps to it stay inside the block; port I/O (`in al\|ax\|eax, dx\|imm8`, `out dx\|imm8, al\|ax\|eax`), `mov SREG, ax`, `mov ax, SREG`, `mov ax, imm16`, `retfq`, `pushfq`/`popfq`, `int n`/`int3` and `lea r64, [.label]` per design 0023; any other 8/16-bit form is `E0900` |

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
| `[N]T` | fixed array (a place type, not a value): in a frame or as a static, read as `rw view T` | **runs** |
| `layout` names | nominal record types | **runs** |
| `choice` names | nominal tagged unions; a value is its image in a word when it fits eight bytes | **runs** (≤ 8 bytes) |
| `T or E`, `T or none` | a fallible value / an optional; `T` an integer or a ref, `E` an integer type or `none` | **runs** |
| `T or E` with a `choice` `E` | the error a tagged union in the payload word | **runs** (choice ≤ 8 bytes) |
| `T or E` with a view `T` from a procedure | a payload that needs `sret` | analysed (`z.try_bytes` is the one view payload that runs) |
| `never` | a procedure that does not return | analysed |
| `physaddr` | a physical address — never dereferenced; cannot mix with `addr` | analysed |
| `be T` / `le T` | an integer stored big/little-endian, in a `layout` | **runs** |
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
| `--show-asm` | the bytes of every `machine x64` block as emitted, after fixups, with the procedure and line | **runs** (`tests/machine/`) |
| `--show-machine-ir`, `--show-bytes` | the lowering as MIR-x64, as bytes | planned (the lowering exists; the printers do not) |
| `--explain` | per procedure: the frame in words (locals, temps, values, phi scratch), the checks kept and the ones removed by each proof (`constant`, `divisor`, `loop-bound`, `unreachable`, `dominance`), zones opened / released / allocated from, calls, syscalls, operations proved to always trap — then the cost class of every line that produced an instruction (`tests/snapshots/cse.explain`) | **runs** (`(registers values=N saved=…)` is the linear scan's assignment) |
| `--explain-cost` | the same report; the per-line part is its `(lines …)` section | **runs** |
| `--check` / `--check-syntax` | analyse / parse only | planned as flags; `show_sema`/`show_ast` with output discarded do it today |
| `--freestanding` | target with no operating system, selected by a `-- target: freestanding` line at the top of the program: no `mmap` zones (`E0330`), no `os.syscall` (`E0401`), a `-> never` entry that is the first instruction of the image (`section ".text.boot"` first, no frame with `calls none`), `traps`, `cpu.halt()`; statics honour `align N`; a `-> never` body ends in `ud2`, never `ret`; `-- load: 0x100000` links and loads the image at that address (any address: the code names statics through 64-bit immediates; a `[static]` operand of a machine block needs the low or the top 2 GiB) | **runs** (`tests/run/freestanding.oli`, `tests/freestanding/`, `examples/kernel.oli` with its Multiboot2 header); the TOML profile file of FREESTANDING.md §2 is not read — its fields live in the source lines |
| `--lib DIR` | where imported modules are found | planned; `lib/` under the working directory today |
| `oli new/build/run/test/fmt/check/bench/doc/package/fuzz` | the project tool | planned |
| `./genesis/test.sh` | build the whole chain from 322 hand-written bytes and run every test, layers 0–5 | **runs** |
| `.vscode/tasks.json`, `tools/vscode-oli/` | build, run and inspect the current file from VS Code; syntax highlighting | **runs** (a convenience outside the toolchain; `olis`/`olide` of `docs/IDE_PLAN.md` replace it) |

### What `olic` refuses today, in one list

Everything marked *analysed* above is reported as `E0900` by the back end,
with the position of the construct, and no file is written. As of this
review that is: a `choice` or a layout failure wider than eight bytes, a view inside a `T or E` that a procedure returns,
a `machine` line the encoder does not know (the 8/16-bit and segment forms outside design 0023, `bytes`), `physaddr(n)`, the `cpu.*` intrinsics beyond `halt`/`pause` and the `mem.*` intrinsics beyond `get_*`/`put_*`,
an `os.syscall` with more than seven words (the number and six
arguments are all the registers a system call has) and aggregate constants.
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
