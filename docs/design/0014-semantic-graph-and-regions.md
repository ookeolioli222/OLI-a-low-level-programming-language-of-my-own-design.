# 0014 — The semantic graph: typed places and values, regions on every value

## Problem
The stage between the syntax tree and OIR must resolve names and modules,
check types, prove definite assignment and reachability, enforce
capabilities, and perform the escape analysis that replaces lifetimes — and
hand OIR lowering a form that needs no re-analysis.

## Existing approaches
- rustc: HIR → THIR → MIR with a separate borrow checker on MIR and lifetimes in types.
- Zig: a single "Sema" pass producing AIR (typed, untyped-comptime values) with no escape analysis.
- C compilers: type checking on the AST, no memory-safety analysis at all.

## Oli-- approach
One pass over each procedure body builds the **semantic graph** (`oli_sema::hir`):

1. **Places and values are different nodes.** `Place` (local, static, field,
   element, `deref`, raw) is typed memory with an `rw` flag; `Expr` is a value.
   A read is an explicit `Load(place)`; `ref x` is `RefOf(place)`; an array place
   used as a value becomes `ViewOfArray(place)`. Lowering never has to guess
   whether something is an lvalue.
2. **Every value carries its regions** (`Static`, `Param(i)`, `Frame`, `Zone(z)`),
   computed structurally: literals are static; parameters are `Param(i)`; a value
   loaded from memory of region R carries R; a call result carries the union of its
   argument regions (rule 3 of `spec/OLI_MEMORY_V0.md`); literals of aggregates
   carry the union of their fields. A value whose type cannot point anywhere carries
   `Static`. Two checks use them: a store requires `outlives(value, target scope)`,
   a return rejects `Frame`/`Zone`. No per-place history is needed because the store
   rule guarantees that whatever sits in memory of scope R outlives R.
3. **Bidirectional integer typing.** Literals and unannotated constants are
   `UntypedInt` until the other operand, the declared type, the parameter or the
   field types them; arithmetic on two untyped operands folds at compile time; a
   literal that never meets a type is `E0201`. The compiler never guesses a width.
4. **Flow state is a bit per local plus a reachability flag**, cloned at branches
   and merged with intersection; loops restore the pre-loop state (`while`, `each`)
   or merge the `break` states (`loop`). Aggregate places are zero-filled at
   declaration so element-wise initialization does not need a proof.
5. **Modules are files, intrinsics are namespaces.** `cpu`, `mem`, `os` are always
   in scope; `import std.os` merges the module's public names into `os`; `core` is
   implicit. Layouts, choices, constants and static initializers are resolved on
   demand with cycle detection, so declaration order never matters.
6. **`--show-sema`** prints the graph with a type on every node and a region on
   every non-static value, so tests pin the analysis, not just the diagnostics.

## Advantages
- One data structure for lowering: no lvalue/rvalue re-derivation, no lifetime
  syntax, regions and checks already decided.
- Diagnostics are specific ("points into zone scratch, stored where frame outlives it").
- The analysis is per procedure and linear in the body size.

## Disadvantages
- Rule 3 is conservative: a callee that returns a static string still yields the
  union of its argument regions; V1 adds `-> T in z` annotations.
- Aggregate zero-fill costs a store per declaration without an initializer
  (visible in `--explain-cost`; V1 may add an explicit opt-out).
- Aliasing of `rw` views is not tracked (by design, `docs/MEMORY_MODEL.md` §3).

## Machine cost
None at run time beyond the zero-fill of uninitialized aggregate places.

## Safety implications
Use-after-scope of views, refs and zone handles, reads of uninitialized scalars,
stores through read-only memory, unhandled failures, non-exhaustive `case`,
undeclared hardware access and mixed address spaces are all compile-time errors.

## Alternatives rejected
- A borrow checker with lifetimes: rejected in 0003/0004.
- Side tables over the AST instead of a new tree: cheaper to build, but every
  later stage would repeat the place/value and region derivations.
- Tracking assignment per aggregate field: exact but unprovable for loops over
  arrays; zero-fill is predictable and honest about its cost.
