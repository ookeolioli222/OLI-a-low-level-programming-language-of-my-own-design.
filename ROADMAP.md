# Oli-- roadmap

The order is fixed: each layer is finished, tested and documented before the
next. Nothing depends on another language (design 0017).

## Language and compiler

| Stage | Content | Status |
|-------|---------|--------|
| Phase 0 | Language design (syntax, memory, machine, OIR, ABI) | done |
| Phase 1 / 1b | Reference front end in Rust (lexer, parser, semantics) — oracle only, under `reference/` | done |
| Genesis G0 | `hex0`: hand-written bytes, self-reproducing | done |
| Genesis G1 | `hex2`: labels and displacements | done |
| Genesis G2 | `asm`: r32/r64 encoder, memory operands (ModRM/SIB/REX), two-pass symbols, rel32, read-only data, executable Hello Oli--; out-of-scope operands rejected | **suite green (`genesis/test.sh`); multi-segment ELF + remaining forms pending** |
| Genesis G3 | `oli1`: oli-core compiler in `machine x64` (asm); step 0 compiles `ret <int>` -> native ELF | **in progress; step 0 green (`genesis/test.sh` layer 3)** |
| Genesis G4 | `olic` in Oli--; fixpoint `stage2 == stage3`; delete Rust oracle | planned |
| M1 | `olic hello.oli && ./hello` (raw syscalls, no libc) | planned |
| M2 | variables, arithmetic, control flow, procedures, layouts, views, zones | planned |
| M3 | freestanding program (own entry, own stack) | planned |
| M4 | minimal kernel in Oli-- | planned |
| V1 | generics, `own`, atomics, port I/O, MMIO, interrupts, bitfields, `arch.x64.*` | planned |
| V2 | SIMD types, floating point, threads, C FFI, shared/static libraries | planned |
| Windows | PE/COFF target and Windows syscalls | planned |

## Standard library (in Oli--, after self-hosting)

core (freestanding) then std (fs, net, thread, sync, time, math, simd, endian,
random, os, ffi) and collections (Vec, HashMap, …) with explicit allocators.

## Ecosystem libraries (in Oli--, gated on the above)

| Library | Gate | Plan |
|---------|------|------|
| [oli.compute](docs/ecosystem/oli-compute.md) | self-hosting + SIMD (V2) + GPU target in OIR | GPU-first tensor / AI / HPC stack |
| [oli.sec](docs/ecosystem/oli-sec.md) | self-hosting + net/binary stdlib | authorized security-research substrate |

Full briefs: `docs/ecosystem/*.brief.md`. Both must pass an Oli originality
review and cannot begin before the language features they need exist.

The verified baseline and ordered completion gates are in
[`docs/PROJECT_STATUS.md`](docs/PROJECT_STATUS.md). A working genesis hello is
not completion of M1: M1 requires the high-level Oli-- compiler.

## Final stage: Oli-- IDE

After the language, toolchain and preceding roadmap stages, build a dedicated
IDE for writing Oli--. The IDE is planned; it is not implemented.
See [`docs/IDE_PLAN.md`](docs/IDE_PLAN.md) for the stages, the language service
protocol, budgets and acceptance criteria, and design record 0021 for the
architecture. Its language service `olis` and the editor `olide` are written in
Oli--, consistent with design 0017.

| Stage | Content | Gate |
|-------|---------|------|
| I0 | specifications: OLIS/0 protocol, editor model, UI, test corpus | none; can start now |
| I1 | `olis` language service over OLIS/0 | G4 + M2 + `std` file/process/pipe + `std.alloc` |
| I2 | `olide` terminal editor: buffers, undo, recovery, highlighting, diagnostics | I1 |
| I3 | project integration: build, run, test, completion, navigation, rename, format | I2 + the `oli` tool |
| — | packaging and the initial release (Linux) | I3 |
| I4 | debugger: ODI debug information, `ptrace` client | I3 |
| I5 | Oli-- tools: cost gutter, capability lens, lifetimes, layout inspector, pipeline views | I3 |
| I6 | Windows console renderer | Windows target |
| I7 | native window: Wayland/Win32 renderer, own font rasterizer | V2 threads |
