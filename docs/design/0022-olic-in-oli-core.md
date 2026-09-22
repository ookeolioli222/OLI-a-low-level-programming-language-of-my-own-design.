# 0022 — Genesis layer 4: `olic` written in oli-core

Status: in progress. Lexer, parser, diagnostic renderer, module loader, item
collection, signatures and local tables implemented and tested 2026-09-22.

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

The lexer and parser validate the approach: ~2 900 lines of oli-core, 84 KiB
of machine code from `oli1`, reproducing all four AST snapshots byte for byte
and the exact diagnostic set of all fifteen negative parse fixtures. The
parser also parses its own source cleanly, which is the first real evidence
that oli-core is expressive enough for `olic`.

Two decisions shape the tree. It is a single arena of uniform nodes (kind,
sub, val, token range, child list) rather than one layout per construct: in
oli-core a layout costs a declaration, a constructor and a field-access path,
while a uniform node costs one. And a type is kept as a *token range*, printed
by joining its tokens, so the parser needs no type tree before semantic
analysis needs one — `[16K]u8` prints as `[16384]u8` because the printer takes
an integer token's value, not its text.

Recovery follows one rule: an error path never consumes a block closer, so a
bad line costs one diagnostic instead of cascading to the end of the file.

Semantic analysis is built in the pass order of
`docs/COMPILER_ARCHITECTURE.md`, and each pass is accepted against the part of
`tests/snapshots/*.sema` it produces, so the snapshot is reached in verified
increments instead of one unverifiable jump. Stage 1 (modules, items, layout,
constants) prints exactly the head of each snapshot; stage 2 adds every
procedure signature and every local with its inferred type. Types are carried
as text in one buffer rather than as a type graph: the printer is this stage's
consumer, and text made the increment verifiable against the snapshot without
inventing a representation the body pass may have to replace. Because a genesis driver
cannot read its command line, the root module is stdin and imports are read
from `lib/`, which is what `--lib DIR` will select later; the loader uses
`openat`/`read`/`close` directly, with no libc and no buffering.

The checks that need no type graph — capabilities, unimplemented features,
constant cycles and ranges, recursive layouts — are implemented before the
type graph, because they are exactly the checks a kernel author relies on
(`docs/KERNEL_PROGRAMMING.md` §2) and because each one is pinned by a fixture
that already exists. Twelve of the fourteen `tests/sema/err` fixtures pass
exactly; the two that remain need expression typing.

Running those checks over `compiler/*.oli` measured the distance between
oli-core and V0 for the first time: 290 stores into bindings (V0 wants
`x : T <- e` places), 93 stores through layout fields that oli-core cannot
declare `rw`, and 8 procedures whose infinite loop is `while 1` because
oli-core has no `loop`. Nothing else — so the measurement *was* the
specification of oli1 step 6e. With those three constructs added and the
sources converted, `olic` analyses its own source without a single
diagnostic, and the harness re-runs that self-analysis on every build.

This is the first point where the compiler is a real user of its own rules,
and it paid immediately: converting the sources turned up a name declared
twice with two different types in one procedure, parameters that were written
through without being `rw`, and a procedure that read a place before assigning
it. None of those are errors in oli-core; all of them are errors in V0.

Writing the compiler in its own subset pays for itself here: `olic` lays out
its own `Tok`, `Lex`, `Ctx`, `Node`, `Item` and `Prog` layouts, and the parser
found every reserved word the compiler's own source used as a name.

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
