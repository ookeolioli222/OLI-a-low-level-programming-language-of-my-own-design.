# 0019 — Checked executable genesis assembler subset

Status: implemented subset, 2026-09-20. Continues 0017; does not complete G2.

## Problem
The original assembler emitted bytes but ignored other source lines, including
unknown instructions and entry declarations. Passing the hand-written encoding
oracle did not establish that it could encode instructions from source.

## Existing approaches
An external assembler or a foreign-language compiler could implement this
quickly, but would violate 0017. A large rewrite covering every target feature
at once would make individual encoding and bootstrap failures harder to isolate.

## Oli-- approach
Extend the existing hand-encoded hex2 program in checked increments: r64
instructions, memory operands, two-pass symbols, read-only data. Use independent
expected byte fixtures and executable CPU tests. Keep unsupported syntax an
error. Reuse the same deterministic parser and encoder in both passes; record
symbol offsets in the first and resolve addresses in the second. Explicit
displacements use disp32, push immediates use imm32, and immediate shifts use C1,
so source operand shape determines output length.

## Advantages
The chain remains bytes -> hex0 -> hex2 -> asm, without a foreign compiler or
assembler. Programs now use real mnemonics, loops, calls and static strings.
Negative fixtures distinguish syntax, undefined symbols, range, entry, capacity
and I/O failures; validation failures do not emit partial executables.

## Disadvantages
Manual encoding remains expensive to maintain. The fixed symbol table has 819
entries and linear lookup, giving quadratic worst-case symbol processing. The
current ELF has one RX segment, so writable static data and BSS are unsupported.
Narrow operands and some instruction forms remain unimplemented. Most error
messages have no source position yet. These are explicit G2 completion gates.

## Machine cost
The assembler is 6,878 bytes. The declared address space is 0x220000 bytes,
including input/output buffers and the bounded symbol table. A local WSL
measurement of the 6,876-byte pre-range-check revision assembled 10,000
`mov r8,0` instructions 20 times in 280,619,930 ns total (about 14 ms/run).
Input was 90,060 bytes; output was 100,120 bytes. This includes process launch
and file I/O and is a local smoke benchmark, not a cross-machine guarantee.

## Safety implications
Input/output/symbol-table limits are checked. Signed immediate fields retain
the literal's sign: a large positive u64 cannot silently become negative s32.
Partial writes complete and interrupted I/O retries. Emitted programs contain
raw machine code and are not sandboxed. Read-only data currently shares the RX
code segment; writable output is rejected until separate segments exist.

## Alternatives rejected
Silently accepting unknown statements, treating an encoding table as proof of
the implementation, introducing an external assembler, and marking the whole
bootstrap complete merely because a machine-level hello works.
