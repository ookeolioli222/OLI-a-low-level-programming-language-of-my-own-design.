# Oli-- roadmap

The order is fixed: each layer is finished, tested and documented before the
next. Nothing depends on another language (design 0017).

## Language and compiler

| Stage | Content | Status |
|-------|---------|--------|
| Phase 0 | Language design (syntax, memory, machine, OIR, ABI) | done |
| Phase 1 / 1b | Front-end design and the fixture corpus (`tests/`, `tests/snapshots`); the temporary foreign oracle was removed 2026-09-21 | done |
| Genesis G0 | `hex0`: hand-written bytes, self-reproducing | done |
| Genesis G1 | `hex2`: labels and displacements | done |
| Genesis G2 | `asm`: r32/r64 encoder, memory operands (ModRM/SIB/REX), two-pass symbols, rel32, read-only data, executable Hello Oli--; out-of-scope operands rejected | **suite green (`genesis/test.sh`); multi-segment ELF + remaining forms pending** |
| Genesis G3 | `oli1`: oli-core compiler in `machine x64` (asm); steps 0–6f: locals, expressions, strings, control flow, syscalls, procedures, zones, views, raw memory, layouts, refs, fallible results, module constants, typed places, `rw` field types, `loop`, explicit conversions | **steps 0–6f green (`genesis/test.sh` layer 3); step 6f (`T(x)`, `T.wrap(x)`, `T.bits(x)`) closed the last measured gap between oli-core and V0 — the 93 sites `olic` found in `compiler/` now carry the conversion they perform** |
| Genesis G4 | `olic` in oli-core (`compiler/`, design 0022); fixpoint `stage2 == stage3`; passes every fixture in `tests/` | **in progress: front end green (layer 4) and back end stages 1-2 green (layer 5) — `compiler/oir.oli` (the instruction stream), `compiler/cfg.oli` (basic blocks, dominators, the OIR_SPEC §8 verifier), `compiler/ssa.oli` (`mem2reg`: places become values, phis by iterated dominance frontier), `compiler/opt.oli` (the passes of §6: const fold, check elision with a recorded proof, copy-prop, DCE), `compiler/x64.oli` (lowering, phis as edge copies, trapping arithmetic, zones, views, `core.trap`) and `compiler/elf.oli` (one-segment ELF64) compile a program end to end; `--show-oir`, `--show-ssa` and `--show-oir=opt` are pinned by twelve snapshots, `compiler/verify_check.oli` breaks the OIR ten ways and the verifier catches each, and `olic` analyses all seventeen of its own modules without a diagnostic. Layouts, refs and fallible results are lowered (`z.make`, `Name.at`, field access, `ref x`, `fail`/`else`/`case`; `tests/run/layouts.oli`, `fallible.oli`). Next: `choice`, `machine` blocks, statics, `[N]T` places, register allocation, and common-subexpression elimination — then the fixpoint itself. The fixpoint `stage2 == stage3` needs all of them** |
| M1 | `olic hello.oli && ./hello` (raw syscalls, no libc) | **done: `genesis/test.sh` layer 5 builds `examples/hello.oli` with `olic` and runs it — a 284-byte static ELF64, no libc and no linker** |
| M2 | variables, arithmetic, control flow, procedures, layouts, views, zones | **in progress: variables, trapping arithmetic (`tests/run/trap/`), `wrap(e)`, control flow, procedures, zones, views, `each`, layouts and refs run (`tests/run/`, `memory.oli`, `layouts.oli`)** |
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
