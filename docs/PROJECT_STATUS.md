# Oli-- completion plan and verified baseline

Reviewed 2026-09-20; foreign oracle removed 2026-09-21. The requested scope is the entire roadmap, in order.
This document records actual implementation, not an assertion that the project
is complete. The working directory has no `.git` metadata.

## Baseline assessment

- The design and normative V0 specification are substantial and usable as a
  target. Design 0017 explicitly requires an autonomous bootstrap, without a
  Rust/C/C++ compiler or an external assembler/linker in the toolchain.
- G0 and G1 build and reproduce their binaries on Linux x86-64 under WSL.
- The original G2 implementation accepted only `bytes`, silently ignored other
  lines, and did not validate structure or entry selection. Its encoding-table
  CPU fixtures validated manually written bytes, not the assembler's encoder.
- The temporary foreign front end (Phases 1–1b) was removed on 2026-09-21 at
  the user's direction. Its last run passed all 77 fixture tests; that corpus
  (`tests/`, `tests/snapshots`) is now the acceptance suite for the Oli--
  front end. Fixture groups can contain multiple source files. Fixtures
  marked `-- reference: skip` were excluded by the old harness; passing the
  suite does not prove these features exist. Until G4, no program in the
  repository can parse high-level Oli--.
- `genesis/3-oli1/` (the oli-core compiler `oli1`, written in `machine x64`)
  exists and passes layer 3 of `genesis/test.sh` for steps 0–6d: locals,
  expressions, strings, control flow, syscalls, procedures, zones, views, raw
  memory, layouts, refs, fallible results and module constants. `compiler/`
  has begun (design 0022): the V0 lexer in oli-core, built by `oli1`, passes
  the fixture corpus at layer 4. The parser, semantic analysis, OIR, native
  backend, standard-library implementation and kernel do not yet exist.
- README and ROADMAP originally disagreed with each other and with G2's code.

## Completion gates

1. **G2 assembler:** checked structure; instruction encodings; memory operands;
   procedure/local symbols and fixups; static data and ELF segment permissions;
   byte-exact tests, CPU execution, rejection tests and deterministic rebuilds.
2. **G3 oli-core:** freeze the smallest V0 subset needed by the compiler; write
   its compiler in the machine sub-language; verify examples and diagnostics.
3. **G4 self-hosting V0:** port lexer/parser/semantics into Oli--; implement OIR,
   x64 lowering and ELF; pass the shared fixtures; establish stage2 == stage3.
   The fixture corpus is the required test set; there is no other oracle.
4. **M1-M2:** run the high-level hello and packet examples; exercise arithmetic,
   control flow, procedures, layouts, views and zones with native runtime tests.
5. **M3-M4:** freestanding output, own entry/stack, bootable minimal kernel;
   document an emulator-based reproducible test and its success condition.
6. **V1:** generics, ownership, atomics, MMIO, port I/O, interrupts and bitfields;
   specify each feature, implement it and add both acceptance/rejection tests.
7. **V2 and Windows:** floating point, SIMD, threads, FFI, libraries; PE/COFF
   emission and platform bindings; cross-target compile/run tests.
8. **Libraries:** core/std and allocator-explicit collections, then oli.compute
   and oli.sec after their prerequisites are genuinely implemented.

Do not replace the autonomy requirement with a convenient foreign bootstrap,
mark design-only features complete, or treat machine-byte tests as proof of
source-level compiler functionality.

## Implemented during this review

The 645-byte bytes-only assembler was expanded to a 6,878-byte, hand-encoded
two-pass assembler. It now checks structure and entry selection; encodes r64
arithmetic, stack and control flow; supports base/index/scale/displacement
memory operands; resolves scoped labels and procedures; and emits read-only
data with numeric, string and address directives. The runnable example is
`examples/genesis/hello.oli`; its stdout is checked byte for byte.

The genesis harness now checks independent expected instruction/data bytes,
runtime arithmetic and memory operations, loops and procedure calls, duplicate
and missing symbols, integer bounds, all three capacity limits, EOF/CRLF/short
reads, I/O failures, deterministic rebuilds and ELF size/permissions. The
original reference suite remains unchanged and all 77 tests pass.

G2 is **not yet complete**: finish narrow operands and extension instructions,
the remaining memory instruction forms, symbol-only memory operands, writable
data/BSS with separate segments, and pub/export clauses. G3/G4, high-level
compilation, kernel, Windows and ecosystem libraries remain unimplemented.
See `genesis/2-asm/SPEC.md` for the precise accepted subset and limits and
design record 0019 for the decisions and local benchmark.
