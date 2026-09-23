# 0006 — Trapping arithmetic with `wrap`/`sat`/`checked`; checked views

## Problem
Integer overflow and out-of-bounds access must never be undefined. The
programmer must be able to choose wrapping (hashes, crypto), saturating (DSP)
or checked (parsers) semantics explicitly, and the default must be safe.

## Existing approaches
- C: signed overflow is undefined; unsigned wraps; no bounds checks.
- Rust: panics in debug, wraps in release (unless `overflow-checks`); `wrapping_add` methods; bounds checked always.
- Zig: safety-checked in Debug/ReleaseSafe, undefined in ReleaseFast; `+%` wrap, `+|` saturate.
- Swift: traps always; `&+` wraps.

## Oli-- approach
- Plain `+ - * / %` **trap** on overflow (defined behavior) in every build mode.
- `wrap(expr)`, `sat(expr)`, `checked(expr)` change the mode of every arithmetic
  operator inside the parenthesized expression; `checked` yields `T or Overflow`.
- Conversions share the vocabulary: `u8(x)` lossless (verified statically),
  `u8.wrap(x)`, `u8.sat(x)`, `u8.checked(x)`.
- `v[i]` and `v[a..b]` on views are bounds-checked (`CHECK`); the optimizer removes
  checks it can prove and `--explain` reports the count.
- Shifts mask the count to the operand width (x86 semantics, documented).
- Division by zero and `MIN / -1` trap.

## Advantages
- No build mode changes semantics: release binaries behave like debug binaries.
- The mode is a word the reader understands; multiple operators share one mode.
- Hash/crypto loops opt into `wrap` once per expression, not per operator.

## Disadvantages
- A `jo`/`jc` after every plain operation (well-predicted; removed when proven unnecessary).
- `wrap(...)` is longer than a symbolic operator.

## Machine cost
`CHECK` per plain operation (typically 1 branch, elided in counted loops);
`ZERO` for `wrap`; `ZERO` + `cmov` for `sat`; `ZERO` for `checked` (the tag is the flag).

## Safety implications
No silent wraparound, no undefined results; every bounds violation is a trap
with a kind and a source site.

## Alternatives rejected
- Wrapping by default: silent bugs (C unsigned behavior).
- Symbolic operators `+% +| +?`: Zig's exact spelling; unreadable in dense expressions.
- Undefined in release: the directive forbids it.
