# 0001 — Own x86-64 backend and ELF writer

## Problem
The compiler must turn OIR into a running native executable. Which component
generates machine code and links the image?

## Existing approaches
- LLVM (Rust, Zig, Clang): excellent optimization, enormous dependency, slow builds,
  its IR loses source-level facts (zones, views, capabilities) unless re-encoded as metadata.
- GCC backend / GIMPLE: same shape, harder to embed.
- Cranelift: smaller, still an external IR with its own type system and ABI rules.
- C transpilation: inherits C's undefined behavior and toolchain.
- External assembler + linker (`as`, `ld`): the compiler cannot control bytes or layout.

## Oli-- approach
`olic` contains its own x86-64 instruction encoder (table-driven, byte-exact
tests per instruction form), its own linear-scan register allocator and its
own ELF64 writer. `olic hello.oli` produces a runnable ELF with no external tool.

## Advantages
- Zero external dependencies; deterministic bytes; fast compiles.
- `--show-asm`, `--show-bytes`, `--explain` describe exactly what was emitted.
- OIR keeps Oli-- facts all the way to lowering (check elision by region/bounds knowledge).
- `machine` blocks are validated by the same encoder; no string blobs.
- Freestanding output (sections, load address, entry) is controlled directly.

## Disadvantages
- No mature optimizer: early Oli-- code will be slower than LLVM output in many benchmarks.
- Every instruction form must be implemented and tested by hand.
- Only x86-64 at first; each new architecture is a new encoder.

## Machine cost
None at run time. Compile time is expected to be lower than LLVM-based compilers.

## Safety implications
The encoder is small enough to test exhaustively per form; miscompilation
risk is concentrated in a few thousand lines that are fully under test.

## Alternatives rejected
- "LLVM temporarily, own backend later": the directive forbids it, and it would
  shape OIR around LLVM's needs.
- Emitting textual assembly for an external assembler: hides bytes, adds a dependency.
