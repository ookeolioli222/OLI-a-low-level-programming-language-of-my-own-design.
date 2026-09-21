# Oli-- Language Vision

## 1. What Oli-- is

Oli-- is a natively compiled systems language whose every construct has a
visible, predictable machine cost. It is meant for kernels, boot code,
drivers, embedded targets, hypervisors, CLI tools, servers, libraries,
compute engines, cryptography, networking, security tooling, AI/ML kernels
and SIMD code — including code that runs with no operating system at all.

Oli-- is **not**: C with new keywords, Rust with simpler syntax, a Zig clone,
an LLVM wrapper, a C transpiler, a VM language, a garbage-collected language,
a libc-dependent language, or a language with a heavy runtime.

## 2. The guiding question

> If C, C++ and Rust had never existed, but we knew modern CPUs, RAM and
> operating systems — how would we design Oli--?

Everything below is derived from the machine, not from the habits of
existing languages. Where Oli-- ends up resembling another language it is
because the machine pushed it there; where it differs it is because the
machine never asked for the thing being dropped.

## 3. What the machine actually offers

| Machine fact | Language consequence |
|--------------|----------------------|
| A small set of named registers | **bindings**: names for values, no address, may live in registers |
| Byte-addressed linear memory | **places**: typed memory slots you can store to and take the address of |
| Stack frames created by `call` / destroyed by `ret` | frame places die with the procedure |
| Bulk memory comes in large chunks (pages) | **zones**: bump-allocated regions freed as a whole |
| Most data is consumed in place | **views**: (address, length) pairs, never a copy |
| Loads/stores have a width and an alignment | fixed-width types, explicit alignment, explicit endianness |
| `add` sets overflow flags | overflow is *observable*, so it is never undefined |
| Some addresses are devices, not RAM | **memory spaces**: `mmio`, `port`, `physaddr` are distinct types |
| Some instructions are privileged or dangerous | **capabilities**: a procedure declares the hardware it may touch |
| `call` transfers control, `ret` returns | procedures, not "flows": the cost is exactly one `call`/`ret` |
| `syscall` enters the kernel | a syscall is a visible operation with a visible cost class |

## 4. Design principles

1. **Predictable.** The reader can state the cost of a line without reading assembly.
2. **Explicit.** Binding, store, move, copy and allocation are written differently.
3. **Nothing hidden.** No hidden allocations, copies, calls, entry points or runtime.
4. **Defined behavior wherever practical.** Overflow traps or wraps by choice; bounds are checked
   or proven; raw memory requires a declared capability.
5. **Low-level control is never sacrificed.** Raw addresses, MMIO, port I/O, inline machine
   code, custom calling conventions and section control are first-class.
6. **Small core.** Every feature must justify itself against the machine model.
7. **The compiler explains itself.** `olic --explain` and `--explain-cost` are part of the language.

## 5. Goals relative to C

fewer undefined behaviors · bounds-aware memory by default · explicit overflow ·
better diagnostics · monomorphized generics (later) · native views/slices ·
native SIMD (later) · native allocators (zones) · safer resource handling
(linear `own` values) · deterministic compile-time evaluation · a real module system.

## 6. Goals relative to Rust

simpler memory model (zones + views + escape analysis instead of a borrow checker) ·
no lifetime annotations in typical code · less compiler ceremony · predictable layouts by
default · easier kernel development (capabilities, memory spaces, sections, naked and
interrupt procedures are language features) · excellent raw-memory support · no runtime ·
direct hardware primitives · a simpler and faster compiler architecture · a stable low-level ABI.

Oli-- does **not** claim to be faster than C or Rust. It claims that its cost model is
visible; performance must be measured, never asserted.

## 7. Cost classes

Every operation in the documentation carries one of these tags. `--explain-cost` prints them.

| Tag | Meaning |
|-----|---------|
| `ZERO` | no instructions or only register moves (a view, a binding, a widening) |
| `CHECK` | a compare-and-branch the optimizer may remove (bounds, overflow, alignment) |
| `STACK` | frame space; no instructions beyond the frame setup |
| `COPY` | a memory-to-memory copy of a stated size |
| `ZONE` | a bump allocation: pointer add + limit compare |
| `ALLOC` | a call into a programmer-supplied allocator |
| `CALL` | a procedure call (`call`/`ret`, argument moves) |
| `SYSCALL` | a kernel entry |
| `KERNEL` | a privileged instruction or a device access |
| `TRAP` | a deliberate abort path (`ud2` or the program's trap handler) |

## 8. The core concepts (one paragraph each)

**Binding (`:=`)** — a name for a value. Immutable. Has no address. `ZERO`.

**Place (`name : T`, `<-`)** — a typed memory slot in a frame, a zone, or a static
section. Stores are written with `<-`. Taking its address is explicit. `STACK`/`ZONE`.

**Move (`<~`)** — transfers an `own` value; the source becomes unusable. The compiler
requires every `own` value to be consumed exactly once. `ZERO` (or `COPY` of the handle).

**Zone** — a lexically scoped bump region. Objects and byte buffers are carved from it;
the whole region is released at `end`. The compiler rejects any view or reference that
would outlive its zone. `ZONE` per allocation, one `SYSCALL` (hosted) or `ZERO`
(freestanding, memory supplied by the program) per zone.

**View** — `(addr, len)` over existing memory. Read-only by default, `rw` when writable,
`mmio` when volatile. Sub-views are `ZERO`. Element access is `CHECK`.

**Reference (`ref T`)** — a view of exactly one object; no length, no bounds check.

**Raw address (`addr T`)** — a machine address. Dereferencing with `[a]` requires the
`memory.raw` capability. `ZERO`.

**Capability (`permit`)** — a procedure header lists the hardware and OS facilities its
body may use directly: `memory.raw`, `memory.mmio`, `io.port`, `cpu.asm`, `cpu.interrupt`,
`cpu.msr`, `cpu.halt`, `os.syscall`. Reading a declaration tells you what it touches.

**Fallible result (`T or E`)** — a tagged value. `ret v` succeeds, `fail e` fails; the
caller must resolve it with `case`, `else` or propagation (`else fail`). `ZERO`/`CHECK`.

**Machine block (`machine x64 ... end`)** — inline machine code with declared inputs,
outputs and clobbers, assembled by Oli--'s own encoder — no string blobs, no external
assembler, mnemonics validated at compile time. `KERNEL` or as-written.

**Layout** — a structural type with a controllable memory layout (`packed`, `align N`,
endian-tagged fields). Reinterpreting a view as a layout is `ZERO` (+ one `CHECK`).

## 9. Non-goals

- Object orientation, inheritance, virtual dispatch as language features.
- Exceptions, garbage collection, reflection, async runtimes.
- A macro language separate from Oli--; compile-time code is Oli-- code.
- Source compatibility with C. Binary (ABI) compatibility with C is a goal.
- Being "fast by decree". Benchmarks decide.

## 10. Roadmap of the language itself

| Version | Scope |
|---------|-------|
| V0 | bindings, places, procedures, integers, bool, conditions, loops, layouts, choices, views, zones, fallible results, `machine` blocks, syscalls, capabilities, statics, sections |
| V1 | generics by monomorphization, `own` resources in `std`, compile-time `if`, atomics, port I/O and MMIO intrinsics, interrupt procedures, bitfields |
| V2 | SIMD types, floating point details, threads, C FFI declarations, shared/static library output |
| V3 | self-hosting: Oli-- compiler written in Oli-- |

## 11. Process

Each stage of the compiler is built only after its design has passed an
**Oli originality review** (see `SYNTAX_EXPERIMENTS.md` §0) and has a written
decision record in `docs/design/`.
