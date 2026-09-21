# 0020 — Genesis layer 3: the oli-core compiler (oli1)

Status: in progress. Steps 0–3 implemented and tested 2026-09-21.

## Problem
Layer 2 (`asm`) turns the `machine x64` sub-language into ELF. To reach a
self-hosted `olic` (layer 4) the chain needs a compiler for a real, if minimal,
Oli-- subset — oli-core — written in that sub-language (design 0017), so the step
from assembler to compiler is small and every increment is runnable.

## Oli-- approach
`genesis/3-oli1/oli1.oli` is compiled by `asm` into `oli1`, which reads oli-core
on stdin and writes a native ELF. oli-core is frozen incrementally
(`genesis/3-oli1/SPEC.md`); each step adds grammar and a behavioural test in the
genesis harness (layer 3). Step 0 compiles `proc … entry … ret <int> … end` to an
ELF that exits with the value.

Two constraints from `asm` shaped the design:
- asm-produced ELFs map only their file image, so oli1 takes scratch memory from
  an anonymous `mmap` rather than a fixed address.
- asm has no 8/16-bit register ops, so oli1 reads source bytes with a masked
  64-bit load and builds output from a template patched by 32-bit stores. Byte
  load/store is added to asm later, when oli1 emits variable-length code.

Step 3 places string data *before* the code (`header | pool | code`) so that a
string's address is known the moment it is bound and no fixup table is needed
in the bootstrap compiler; the code is moved behind the pool at finalize and
`e_entry` is patched. This costs one extra copy of the code per compile and
nothing at run time.

## Advantages
- The assembler→compiler step is small and each increment runs on real hardware.
- No foreign toolchain; oli1 is built and tested entirely inside the chain.

## Disadvantages
- Template-and-patch output is a scaffold; real codegen waits on asm byte ops.

## Machine cost / safety
oli1 uses one `mmap` and raw `read`/`write`/`exit` syscalls. Malformed or
out-of-scope input must reject without emitting a partial ELF.

## Alternatives rejected
- Fixed scratch buffers (0x410000): unmapped in asm-produced ELFs (read → EFAULT).
- Writing oli1 in raw hex2: asm exists precisely so layer 3 is written in mnemonics.
