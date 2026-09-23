# 0010 — Keyword/`end` block syntax (Proposal A "Ledger")

## Problem
Choose the surface syntax of Oli-- among three deliberately different
proposals (`docs/SYNTAX_EXPERIMENTS.md`).

## Existing approaches
- Braces `{ }` (C, C++, Rust, Zig, Go, Java).
- Offside rule (Python, Haskell, Nim).
- `begin`/`end` or keyword/`end` (Pascal, Ada, Lua, Ruby, Elixir).
- Parenthesized forms (Lisp family).

## Oli-- approach
Keyword-opened, `end`-closed blocks; newline-terminated statements; three
memory operators; procedure headers with contract clauses; `case`/`when`;
`machine` sub-language as data. Adopted from the other proposals: one-line
`if c then stmt`, bracket sub-views `v[a..b]`, shared mode vocabulary for
conversions.

## Advantages
- Best scores on readability, low-level usefulness, machine-model visibility, scalability.
- Parser: keyword-directed recursive descent, `end` as a resync point, no layout lexer.
- Whitespace-insensitive: safe for generated code, copy-paste, merges.
- Reads unlike C/Rust/Zig at a glance while staying typeable on any keyboard.

## Disadvantages
- More keystrokes than the offside proposal (`end`, `then`).
- `else` has two roles (branch vs fallback), resolved by a position rule.
- No `if` expression in V0.

## Machine cost
None (syntax).

## Safety implications
None directly; explicit closers reduce misparsing of long kernel procedures.

## Alternatives rejected
- Braces: the signature of the C family; also consumes `{ }` needed for aggregate literals.
- Offside rule: fragile under generation/merging; INDENT/DEDENT lexer; tab policy.
- S-expressions: lowest readability for arithmetic-heavy and register-heavy code.
