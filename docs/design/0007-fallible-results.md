# 0007 — `T or E`, `ret`/`fail`, `else`, `case`

## Problem
Errors must be values with zero hidden control flow, must be impossible to
ignore, and must be cheap to propagate.

## Existing approaches
- C: return codes and `errno`; ignorable.
- C++/Java: exceptions; hidden control flow, unwinding runtime.
- Rust: `Result<T, E>` + `?`; excellent but generic-heavy and `?` is easy to miss.
- Zig: `!T` error unions + `try`; error sets without payloads.
- Go: `(T, error)` tuples; verbose, ignorable.

## Oli-- approach
- A fallible type is `T or E` (E is any type; `none` is the built-in empty failure).
- Inside a `-> T or E` procedure, `ret v` succeeds and `fail e` fails.
- A fallible expression must be resolved by one of:
  `expr else fail` (propagate), `expr else ret v` (diverge), `expr else v` (default),
  or `case expr when ok x ... when fail e ... end`.
- Discarding a fallible value is a compile error.
- Representation: a two-variant choice; tag in `rax`, payload in `rdx` when it fits.

## Advantages
- Two keywords (`ret`, `fail`) make the success and failure channels visible.
- `else` reads as English, is impossible to overlook, and handles all three needs.
- No generics, no traits, no unwinding; the tag is a register.
- `T or none` replaces a separate `Option` type.

## Disadvantages
- `else` also introduces `if` branches; the rule "fallback `else` may not appear
  in a one-line `if`" is needed.
- Error types are not automatically unioned (`fail` requires an exact or convertible type).

## Machine cost
`ZERO` to produce, `CHECK` (one tag compare) to resolve.

## Safety implications
Unhandled failure is a compile error; no exceptions can cross a `machine` block
or an interrupt boundary because there are none.

## Alternatives rejected
- `?` postfix: copies Rust/Swift; one character is too easy to miss in security-critical code.
- `try` prefix: copies Zig.
- Exceptions: hidden control flow and runtime.
