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
| Genesis G4 | `olic` in oli-core (`compiler/`, design 0022); fixpoint `stage2 == stage3`; passes every fixture in `tests/` | **done 2026-09-23: `stage2 == stage3` — `olic` compiles its own 32,264 lines into 1,458,896 bytes and that compiler compiles them into the same bytes (`genesis/test.sh` layer 6), with every fixture compiled identically by both. Front end green (layer 4), back end stages 1-6 green (layer 5) — `compiler/oir.oli` (the instruction stream), `compiler/cfg.oli` (basic blocks, dominators, the OIR_SPEC §8 verifier), `compiler/ssa.oli` (`mem2reg`: places become values, phis by iterated dominance frontier), `compiler/opt.oli` (the passes of §6: const fold, check elision with a recorded proof, copy-prop, DCE), `compiler/x64.oli` (lowering, phis as edge copies, trapping arithmetic, zones, views, `core.trap`) and `compiler/elf.oli` (one-segment ELF64) compile a program end to end; `--show-oir`, `--show-ssa` and `--show-oir=opt` are pinned by twelve snapshots, `compiler/verify_check.oli` breaks the OIR ten ways and the verifier catches each, and `olic` analyses all seventeen of its own modules without a diagnostic. Layouts, refs and fallible results are lowered (`z.make`, `Name.at`, field access, `ref x`, `fail`/`else`/`case`; `tests/run/layouts.oli`, `fallible.oli`). Statics, raw access and arrays in frames followed, which completes M2; then the `sat`/`checked` modes, every zone source with a release on every exit edge, common-subexpression elimination with the `dominance` proof, `--explain`, `choice` as the failure of a fallible result, and `machine x64` blocks assembled by an encoder of its own, and a linear-scan register allocator over rbx, r12–r15; then M3: the freestanding shape, the load address, the Multiboot2 header and `examples/kernel.oli`. Next: M4, a minimal kernel — interrupts (`calls interrupt`), `mem.mmio`, page tables** |
| M1 | `olic hello.oli && ./hello` (raw syscalls, no libc) | **done: `genesis/test.sh` layer 5 builds `examples/hello.oli` with `olic` and runs it — a 284-byte static ELF64, no libc and no linker** |
| M2 | variables, arithmetic, control flow, procedures, layouts, views, zones | **in progress: variables, trapping arithmetic (`tests/run/trap/`), `wrap(e)`, control flow, procedures, **done 2026-09-23: every construct of the row runs — `tests/run/` pins variables, arithmetic, control flow, procedures, layouts, views, zones, and with them refs, fallible results, statics, arrays in frames and raw access** |
| M3 | freestanding program (own entry, own stack) | **done 2026-09-23**: `-- target: freestanding` programs compile and run — entry with no frame (`calls none`), own stack, `traps` receiving `core.TrapKind` and `core.Site`, zones at/from, `cpu.halt()`, `align N`, `.text.boot` first, `-- load:` address, a Multiboot2 header as a static layout in `.text.boot`, port I/O (design 0023); `examples/kernel.oli` is the §8 target program, checked structurally by the harness and bootable with QEMU |
| M4 | minimal kernel in Oli-- | **in progress 2026-09-23**: `calls interrupt` (push all, `ref core.x64.InterruptFrame`, `iretq`), `core.x64` layouts, an IDT loaded with `lidt` and a handler reached by `int 3` in `examples/kernel.oli`; `mem.mmio` (2026-09-24): `mmio rw view T` with every element access volatile, the VGA cells of the kernel example written through it; port places `port.u8/u16/u32[n]` and `cpu.interrupts(on/off)` (2026-09-24): COM1, the two 8259 PICs and the 8253 PIT programmed from Oli--, a `calls interrupt` timer handler on vector 32 counting ticks in the kernel example; page tables (2026-09-24): `core.x64.paging` with entries, indices and `identity_2m` (executed by `tests/run/paging.oli`), the control registers `arch.x64.cr0/2/3/4/8` as places, the kernel example identity-mapping its first gigabyte and installing the tables in cr3; procedures of imported modules are called across modules; symbols and section headers (2026-09-24): every image carries `.symtab`/`.strtab` per ABI.md §4 and `.rodata/.text/.data/.bss` headers, so `nm`, `readelf -S`, `objdump -d` and `gdb` read it. Object files and `extern` (2026-09-24): `-- output: object` writes ET_REL with `.rela.text`, `extern proc` is an undefined symbol; `tests/c` links two Oli-- objects with `ld` and one with C by `cc` — C calls `oli_add`, which calls C back — and runs both. Atomics and fences (2026-09-24): `atomic.*` at every width as lock-prefixed instructions, `cpu.fence` as `lfence`/`sfence`/`mfence`, executed and pinned by objdump. `mem.copy/set/zero/secure_zero` (2026-09-24): `rep movsb` in both directions and `rep stosb`, bounds-checked, never removed. `mmio ref T` and the `port T` type (2026-09-24): `Name.at` over an `mmio` view gives a ref whose fields are volatile; `port u8 (n)`, `p.in()`, `p.out(v)` are the same `in`/`out` as the place form; `tests/parse/ok/kernel_sketch.oli` has no `E0900` left. The target profile (2026-09-24): `-- profile: PATH` with `load_address`, `align_sections` and `sections` — code and zero-static groups by name, in the profile's order, each with its own section header; the kernel example carries `examples/x86_64-kernel.oli-target`. Debug information (2026-09-24): DWARF 4 line tables and a compile unit with a subprogram per procedure in every executable — `addr2line`, `objdump --dwarf` and `gdb` name lines. The hardware commands of design 0015 (2026-09-24): `cpu.id`, `cpu.tsc`, `cpu.stack`, `cpu.frame`, `cpu.call`, `cpu.jump`, `arch.x64.msr[n]`, `arch.x64.gdt/idt/tr`; the kernel example's entry is the design's "proposed" form and its IDT load and `cpuid` are commands. Conditional compilation (2026-09-24): `when target.freestanding/hosted/object/os/arch … else … end` at declaration level, the facts from the pragmas and the profile. Field constants, `arch.x64.segments` and `bytes` (2026-09-24): `Name.field.offset`/`.size` fold before the OIR, `arch.x64.segments(code, data)` is the far return and five segment loads, executed for real by `tests/run/segments.oli` under a process's own selectors, and a machine block may spell `bytes b, …`. `own T` and `<~` (2026-09-24): linear handles checked by the front end — consumed exactly once on every path, or E0340–E0345 say where — executed by `tests/run/own.oli` on a file descriptor. **Booted (2026-09-24)**: `start` is a 32-bit trampoline (page tables, PAE, EFER.LME, a GDT, cr0.PG, `jmp far 0x08:start64`) written as `bytes` and `addr32` in a machine block, `mb1` a Multiboot 1 header for QEMU's `-kernel`; QEMU 10.0.13 boots the image — five lines on COM1, `int 3`, a hundred timer ticks, exit through isa-debug-exit — and the harness repeats the boot whenever QEMU is present. Aggregate constants (2026-09-24): `NAME : [N]T := { … }` and layout constants in the read-only segment, aligned. A physical memory manager (2026-09-25): `core.frames`, a frame bitmap freed from the Multiboot memory map; the kernel reports the free frames of the machine it boots on (15840 at 64 MiB, 32224 at 128 MiB under QEMU) and writes a frame it allocated. A kernel heap (2026-09-25): `zone kheap 1M at` a run of frames from `frames.alloc_run`, a thousand records made in it, `try_bytes` refused at the limit, the frames returned. Next: a cooperative scheduler; `f32`/`f64` stay V2 |
| V1 | generics, `own`, atomics, port I/O, MMIO, interrupts, bitfields, `arch.x64.*` | planned |
| V2 | SIMD types, floating point, threads, C FFI, shared/static libraries | **partly done 2026-09-25**: scalar `f32`/`f64` in SSE with correctly rounded literals and SysV xmm arguments (stage 41); C FFI through objects, `extern proc` and `export` both ways, and PIE/shared-library linking (stages 24, 39). 128-bit SIMD types in SSE2 (stage 42, design 0024). Planned: wider vectors (AVX, with a CPU requirement), threads, a shared-library output of `olic`'s own |
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
