# 0022 — Genesis layer 4: `olic` written in oli-core

Status: in progress. Lexer implemented and tested 2026-09-21.

## Problem
Layer 3 (`oli1`) compiles oli-core. The self-hosted compiler `olic` must be
written so that `oli1` builds it (stage 1), the resulting `olic` builds itself
(stage 2) and that build reproduces itself (stage 3 == stage 2), with no
language outside the repository at any step (design 0017).

## Existing approaches
- Bootstrapping compilers in a restricted dialect of their own language (early
  Pascal, Oberon, Go's C-to-Go transition): the dialect is chosen so that the
  bootstrap compiler stays small.
- Writing the first compiler in another language: excluded by 0017.

## Oli-- approach
`olic` is written in oli-core — a *subset* of V0, not a dialect — so the same
source is accepted by `oli1` now and by `olic` later; nothing needs porting at
the fixpoint. oli-core grows only when a compiler module genuinely needs a
feature (6c fallible results, 6d module constants were added for this), and
each growth step is proven in the layer-3 harness before use.

The compiler is a set of modules concatenated on stdin (`compiler/SPEC.md`).
Each front-end stage gets a driver that prints its output (`--show-tokens`,
`--show-ast`, `--show-sema`) and is accepted against the fixture corpus
(`tests/`, `tests/snapshots`) — the corpus is the only oracle. Records live in
zone arenas; tables are `T.at` views over `z.bytes` allocations.

The lexer (first module) validates the approach: 600 lines of oli-core,
~19 KiB of machine code, accepts the whole corpus and reports the exact
lexical diagnostics of the negative fixtures.

## Advantages
- No port step at self-hosting; `stage2 == stage3` is a byte comparison.
- Every oli-core feature the compiler relies on is already tested at layer 3.

## Disadvantages
- oli-core is verbose (no `and`/`or`, flat scopes, six argument words); the
  compiler is written with early returns and small procedures.
- `oli1`'s code generator is naive; `olic` stage 1 will be slow and large.
  It only has to build stage 2 once.

## Machine cost / safety
Each driver allocates one zone (16 MiB) for source, tables and arenas; limits
(1 MiB source, 200 000 tokens) trap rather than corrupt. Diagnostics never
stop a stage.

## Alternatives rejected
- A richer bootstrap dialect with `machine` blocks in `compiler/`: forbidden by
  `docs/COMPILER_ARCHITECTURE.md` policies (no `machine` outside `genesis/`).
- Generating oli-core from V0 by a converter: another compiler to bootstrap.
