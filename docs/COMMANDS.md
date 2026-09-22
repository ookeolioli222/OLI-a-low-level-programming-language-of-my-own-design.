# The commands of Oli-- — annotated reference

Every construct a programmer can write, grouped by what it does to the machine,
each with what it does, its cost class, an example, and its status.

**Status:** `impl` = parsed and analysed by `olic` (`compiler/`, written in
Oli--) and pinned by the fixture corpus; code generation is G4's next stage, so
nothing below runs as a native binary yet outside oli-core (`docs/LANGUAGE.md`
marks what runs) · `spec` = accepted design, specification written ·
`V1`/`V2` = planned language version · `genesis` = a bootstrap tool, not
language syntax.

**Cost classes** (from `docs/LANGUAGE_VISION.md` §7; `--explain-cost` prints them):
`ZERO` no instructions or only register moves · `CHECK` a compare-and-branch the
optimizer may remove · `STACK` frame space · `COPY` a memory copy of a stated
size · `ZONE` a bump allocation · `CALL` a procedure call · `SYSCALL` a kernel
entry · `KERNEL` a privileged/device instruction · `TRAP` a deliberate abort.

The guiding idea: **you can state the cost of any line without reading the
assembly.** Nothing allocates, copies, or calls invisibly.

---

## 1. The three memory operators — the core of the language

Where C, Rust and Zig all write `=`, Oli-- uses three different operators so the
reader always sees whether a line touches a register or memory, and whether it
copies or moves. This is the single most distinctive thing about the language.

### `name := expr` — bind
Gives a name to a value. Immutable, has no address, may live purely in a
register. This is not a variable in the C sense — it is a name for a computation.
```oli
sum := a + b          -- ZERO: `sum` is just a name for the result
```
**Cost:** ZERO. **Status:** impl.

### `name : T` / `name : T <- expr` — declare a place
A *place* is a typed slot in memory (frame, zone or static). Only places have
addresses and can be written to after creation. Declaring one is a layout
decision, so it needs a type.
```oli
counter : u32 <- 0    -- STACK: a 4-byte slot, initialized to 0
scratch : [64]u8      -- STACK: 64 bytes, zero-filled (aggregate)
```
**Cost:** STACK (frame) or ZONE (in a zone). **Status:** impl.

### `place <- expr` — store
Writes a value into an existing place, a field, an array/view element, or a raw
address. The arrow points at the memory the value goes into.
```oli
counter <- counter + 1
buf[i]  <- 0
hdr.len <- 20
```
**Cost:** ZERO (one store). **Status:** impl.

### `place <~ expr` — move
Transfers an `own` value; the source becomes unusable, so a resource is never
copied or double-freed by accident. Reserved until `own` lands in V1.
**Cost:** ZERO. **Status:** V1.

### Addresses and references
```oli
p := addr counter     -- addr T : raw machine address (deref needs `permit memory.raw`)
r := ref counter      -- ref T  : safe reference to one live object
w := rw ref counter   -- writable reference
x := [p]              -- raw load through an addr (permit memory.raw)
```
`addr`/`ref`/`rw ref` are ZERO; a raw load/store `[p]` is ZERO but gated by the
`memory.raw` capability. **Status:** impl.

### Views — `(address, length)` over existing memory, never a copy
A view is the everyday way to pass and slice memory. It carries its length, so
element access is bounds-checked and the check can be proven away.
```oli
head := packet[0..20]    -- ZERO: a sub-view, no bytes moved
b    := packet[i]        -- CHECK then load: `i < packet.len` is verified
n    := packet.len       -- ZERO
a    := packet.addr      -- ZERO: the first byte's address
```
**Status:** impl.

---

## 2. Zones — memory without a garbage collector or per-object free

A `zone` is a lexically scoped bump region. You carve buffers and objects out of
it; at `end` the whole region is released in one step. No `malloc`/`free` per
object, no leaks, no use-after-free (the compiler rejects any view that would
outlive its zone). The memory source is always explicit.

```oli
zone scratch 64K              -- hosted: one mmap (SYSCALL); freed whole at `end`
    pkt := scratch.bytes(4096)   -- ZONE: 4096 rw bytes, zeroed
    hdr := scratch.make(Header)  -- ZONE: one Header, aligned, zeroed -> rw ref Header
end                           -- everything from `scratch` dies here

zone boot 1M at addr u8 (0x200000)   -- raw memory at an address (permit memory.raw)
zone heap 4M from region             -- carved from a static [4M]u8 array
```

| Command | Meaning | Cost |
|---------|---------|------|
| `zone N ... end` | region from the enclosing zone, or the OS (hosted), or `at`/`from` | ZONE / SYSCALL |
| `z.bytes(n)` | `n` zeroed bytes → `rw view u8`; traps on exhaustion | ZONE |
| `z.try_bytes(n)` | same, but `→ rw view u8 or none` instead of trapping | ZONE |
| `z.make(T)` | one zeroed `T` → `rw ref T` | ZONE |

**Status:** impl.

---

## 3. Reinterpreting bytes — layouts over a view

`T.at(v)` reads a byte view as a structured layout with zero copy (one alignment
+ bounds check). `T.size` / `T.align` are compile-time constants.
```oli
hdr := Header.at(pkt)     -- CHECK: rw ref Header over pkt's bytes, no copy
if pkt.len < Header.size then fail too_short
```
**Status:** impl.

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
| `module a.b`, `import a.b [as x]`, `pub` | one file = one module; `cpu`/`mem`/`os` always in scope, `core` implicit | impl |
| `proc NAME(params) -> T ... end` | a procedure (the language's "function") = one `call`/`ret` | impl |
| `permit cap, ...` | capabilities the body may use: `memory.raw`, `memory.mmio`, `io.port`, `cpu.asm`, `cpu.halt`, `cpu.interrupt`, `cpu.msr`, `cpu.control`, `os.syscall` | impl |
| `calls sysv` / `calls none` / `calls interrupt` | convention; `none` = no prologue (boot code); `interrupt` is V1 | impl / impl / V1 |
| `section "x"`, `align N`, `export ["sym"]` | placement and linkage | impl |
| `entry` | the program's start on every target; `-> s32` hosted, `-> never` freestanding. There is no `main` | spec |
| `traps` | the procedure that receives traps (freestanding) | impl |
| `layout NAME [packed] [align N] ... end` | a struct with a controllable memory layout; fields may be `be T`/`le T` | impl |
| `choice NAME ... end` | a tagged union of variants with optional payload | impl |
| `NAME := expr` / `NAME : T := expr` | a compile-time constant (aggregates live in `.rodata`) | impl |
| `NAME : T [<- expr]` at module level | a static place in `.bss`/`.data` | impl |

---

## 5. Control flow

```oli
if a > b then ret a           -- one-line form (no else)
ret b

if n < 0                      -- block form
    tag <- 'n'
elif n == 0
    tag <- 'z'
else
    tag <- 'p'
end

while i < n ... end           -- pre-tested loop
loop ... break ... end        -- infinite until break/ret/fail
each x in view ... end        -- iterate a view (no per-element bounds check)
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
| `if/elif/else/end`, `if c then s` | branch (the one-line form has no else) | impl |
| `while`, `loop`, `break`, `continue` | loops | impl |
| `each x in v` / `each i in a..b` | iterate a view or an integer range | impl |
| `ret [e]` / `fail [e]` | the success / failure exit of a procedure | impl |
| `e else fail` / `else ret [v]` / `else default` | resolve a `T or E` value | impl |
| `case e when p ... else ... end` | exhaustive match (checks all variants) | impl |
| `machine x64 ... end` | inline machine code (see §8) — the escape hatch | impl |

---

## 6. Arithmetic, overflow modes and conversions

Plain arithmetic **traps** on overflow — never the silent wraparound or
undefined behavior of C. You opt into other behavior explicitly, per expression.
```oli
x + y            -- traps on overflow (defined behavior, every build)
wrap(x + y)      -- two's-complement wraparound (hashes, crypto)
sat(x + y)       -- saturating (clamps to the type's range)
checked(x + y)   -- yields `T or Overflow`, handled with else/case
```
Conversions are explicit and lossless by default:
```oli
u32(b)           -- widen u8 -> u32, verified lossless at compile time
u8.wrap(x)       -- truncate u32 -> u8, wrapping
u8.sat(x)        -- truncate, saturating
u8.checked(x)    -- -> u8 or Overflow
uword.bits(n)    -- same-width reinterpretation (signed<->unsigned)
physaddr(0x1000) -- integer -> physical address
addr u8 (0xB8000)-- integer -> raw virtual address (permit memory.raw)
```
Comparisons yield `bool`, do not chain (`a < b and b < c`, not `a < b < c`), and
`and`/`or`/`not` are boolean. Bit ops `& | ^ ~ << >>` need equal integer types;
shift counts are masked to the width (defined). **Status:** impl.

---

## 7. Intrinsics (always in scope)

| Command | What it does | Permit | Cost |
|---------|--------------|--------|------|
| `os.syscall(nr, a1..a6)` | a raw Linux system call — this is how `hello` prints without libc | `os.syscall` | SYSCALL |
| `cpu.halt()`, `cpu.pause()` | `hlt` / `pause` | `cpu.halt` / — | KERNEL / ZERO |
| `mem.copy(dst, src, n)` | copy `n` bytes (move semantics; overlap allowed) | — | COPY |
| `mem.set/zero(dst[, b])` | fill / zero a byte view | — | COPY |
| `mem.secure_zero(dst)` | zero that the optimizer may never delete (wipes a secret) | — | COPY |
| `mem.get_u16/32/64`, `get_be*`, `get_le*` | read an integer of a given width/endianness from bytes | — | CHECK |
| `mem.put_u16/32/64`, `put_be*`, `put_le*` | write one | — | CHECK |

**Status:** impl.

---

## 8. Hardware places and commands (design 0015)

The machine's own state is reached with the *same* operators as ordinary memory:
`<-` stores into a register, a call runs a command. No mnemonics needed for the
common cases — a kernel is written in Oli-- to the bottom.
```oli
cpu.stack <- addr boot_stack + 16K   -- the stack pointer is a place
cpu.call(main)                       -- transfer control
cpu.halt()                           -- hlt
id := cpu.id(0)                      -- cpuid -> core.CpuId { a, b, c, d }
arch.x64.cr3 <- page_table           -- V1: a control register (physaddr-typed)
arch.x64.gdt <- ref gdt_pointer      -- V1: lgdt
port.u8[0x3F8] <- b                  -- V1: port I/O as an indexed place
```

| Command | Permit | Status |
|---------|--------|--------|
| `cpu.stack`, `cpu.frame`, `cpu.call`, `cpu.jump` | `cpu.control` | spec (V0) |
| `cpu.id(leaf)` | — | spec (V0) |
| `cpu.interrupts(on/off)`, `cpu.fence(order)`, `cpu.tsc()` | `cpu.interrupt` / — | V1 |
| `arch.x64.cr0/2/3/4/8`, `msr[n]`, `gdt`, `idt`, `tr`, `segments()` | `cpu.control` / `cpu.msr` | V1 |
| `port.u8/u16/u32[n]` | `io.port` | V1 |
| `atomic.load/store/add/sub/and/or/xor/cas(ref, ..., order)` | — | V1 |

Every hardware access is volatile, cost class `KERNEL`, and listed by `--explain`.

### Inline machine code — the escape hatch
For instructions without a command. Assembled by Oli--'s own encoder (no external
assembler), with declared inputs, outputs and clobbers — not opaque strings.
```oli
machine x64
    in  eax <- 0        -- load a register from a value before the block
    cpuid
    out ebx -> b        -- store a register to a place after the block
    clobber ecx, memory
end
```
**Permit:** `cpu.asm`. **Status:** impl.

---

## 9. Types — the vocabulary of the machine

| Type | Meaning |
|------|---------|
| `u8 u16 u32 u64` / `s8 s16 s32 s64` | unsigned / signed integers of fixed width (`s`, not `i`) |
| `word` / `uword` / `byte` / `bool` | machine word (= s64/u64), `u8`, boolean |
| `physaddr` | a physical address — never dereferenced; cannot mix with `addr` |
| `addr T` | a raw virtual address (deref under `memory.raw`) |
| `ref T` / `rw ref T` / `mmio ref T` | reference to one object — safe, writable, volatile |
| `view T` / `rw view T` / `mmio view T` | `(addr, len)` over many — the everyday memory handle |
| `[N]T` | fixed array (a place/static type, not a value) |
| `be T` / `le T` | an integer stored big/little-endian (for wire and file layouts) |
| `zone` | a zone handle |
| `T or E` | a fallible value (success `T` or failure `E`); `T or none` is an optional |
| `never` | a procedure that does not return |
| `port T` (V1) · `own T` (V1) · `f32 f64` (V2) | reserved for later versions |

Type identity is nominal for `layout`/`choice`, structural for everything else.
Every type's size, alignment and field offsets are fixed and documented — the
compiler never reorders fields. **Status:** impl (V1/V2 items reserved).

---

## 10. Compiler and toolchain

| Command | What it does |
|---------|--------------|
| `olic file.oli` | build a native ELF — own x86-64 encoder, own ELF writer, no linker |
| `--show-tokens / --show-ast / --show-sema` | inspect the front end (all three exist today as the `genesis/build/show_*` drivers, one per stage, reading stdin) |
| `--show-oir / --show-machine-ir / --show-asm / --show-bytes` | inspect the back end (planned) |
| `--explain` | per procedure: frame size, register assignment, allocations, copies, views, checks (and which were removed), syscalls, capabilities |
| `--explain-cost` | the cost class of every line |
| `--check` / `--check-syntax` | analyze / parse only |
| `--freestanding` / `--lib DIR` | target with no OS / where to find imported modules |
| `oli new/build/run/test/fmt/check/bench/doc/package/fuzz` | the project tool (later) |

---

## 11. Genesis tools (the bootstrap — not language syntax)

These make the language exist without any other compiler (design 0017).

| Tool | Written in | Purpose | Status |
|------|-----------|---------|--------|
| `hex0` | 322 hand-encoded bytes | hex listing → bytes; reproduces itself | done |
| `hex2` | hex0 notation | hex with labels and relative/absolute fixups | done |
| `asm` | hex2 notation | assembles Oli-- `machine x64` blocks into ELF64 | in progress |
| `oli1` | Oli-- machine blocks | compiles the oli-core subset | planned |
| `olic` | oli-core Oli-- | the full compiler; `stage2 == stage3` | planned |

---

## 12. Ecosystem libraries (written in Oli--, after self-hosting)

Not commands of the language, but the libraries the commands above are for
(`docs/ecosystem/`, design 0018): **oli.compute** (GPU-first tensors / AI / HPC)
and **oli.sec** (authorized security research). Both are deferred until the
self-hosted compiler and the needed language features (SIMD, `own`, atomics,
FFI, GPU target; net/binary stdlib) exist.
