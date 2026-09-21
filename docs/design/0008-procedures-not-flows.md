# 0008 — Procedures with contract headers instead of a "flow" model

## Problem
The directive asked whether a distinctive execution model ("flow": data
streams, `each`, `yield`) should replace classic procedures.

## Existing approaches
- Procedures/functions: one `call`/`ret`; every systems language.
- Generators/coroutines (Python, C#, Rust async): compiler-built state machines;
  hidden frames, hidden allocations or hidden runtime.
- Dataflow languages (Lucid, LabVIEW): graphs, not sequential machine code.

## Oli-- approach
Keep the procedure — it *is* the machine's `call`/`ret` — but make its header
a readable **contract**: name, parameters, result, and clauses (`permit`,
`calls`, `section`, `align`, `entry`, `export`, `traps`). `each x in view`
is the primitive iteration form. `ret`/`fail` are the two exits. A "flow"
would either be a renamed procedure (no gain) or a generator (hidden state).

## Advantages
- Cost is exactly one `call`/`ret`; `--explain` maps 1:1.
- The contract header is where Oli-- differs from other languages: it states
  hardware access, convention and placement, not just types.
- `each` over views gives loop-bound proofs for free (bounds checks elided).

## Disadvantages
- No generators/iterators in V0; iteration over non-view structures uses `while`.

## Machine cost
`CALL`.

## Safety implications
None new; `-> never` and `calls none` are verified.

## Alternatives rejected
- `flow`/`yield` generators: hidden state machines contradict the zero-hidden-cost rule.
- Renaming `proc` to `flow` without new semantics: novelty without justification.
