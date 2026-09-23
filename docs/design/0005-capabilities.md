# 0005 — Per-procedure hardware capabilities (`permit`)

## Problem
A single `unsafe` boundary says "something here is dangerous" but not what.
Kernel code needs to know, per procedure, whether it touches raw memory,
device registers, I/O ports, privileged instructions or the OS.

## Existing approaches
- C: nothing; every function may do anything.
- Rust: `unsafe` blocks/functions; one bit of information.
- Zig: no unsafe marker at all; `@intToPtr` etc. are ordinary builtins.
- Capability-based OS designs (seL4, CHERI): capabilities at run time, not in the language.

## Oli-- approach
A procedure header lists the facilities its body may use directly:

```oli
proc load_gdt
permit cpu.control, cpu.asm
```

Capabilities: `memory.raw`, `memory.mmio`, `io.port`, `cpu.asm`, `cpu.halt`,
`cpu.interrupt`, `cpu.msr`, `cpu.control`, `os.syscall`. They are not
transitive: calling a permitted procedure needs no permit. The semantic graph
and the OIR verifier both check them. Hosted mode rejects hardware permits
that would fault in user mode with a warning; freestanding mode rejects `os.syscall`.

## Advantages
- The declaration says what hardware the body touches; `grep permit` audits a kernel.
- Finer than `unsafe`: a procedure allowed to use ports cannot silently dereference raw memory.
- No run-time cost, no runtime.

## Disadvantages
- Some boilerplate in low-level modules (mitigated by grouping hardware code in few procedures).
- Non-transitivity means a "safe" wrapper can hide a permitted operation — as with any safe abstraction; the wrapper itself remains greppable.

## Machine cost
None.

## Safety implications
Privileged operations are impossible outside declared procedures, including
inside `machine` blocks (which need `cpu.asm`). A missing permit is a compile
error, never a run-time fault.

## Alternatives rejected
- One `unsafe` keyword: too coarse for OS work.
- Module-level permits: hide which procedure is dangerous.
- Transitive (effect-system-style) permits: every caller up to `main` would need them; noise without benefit.
