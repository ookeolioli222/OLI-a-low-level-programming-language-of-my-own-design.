# 0002 — Three memory operators: `:=` bind, `<-` store, `<~` move

## Problem
One `=` in C/Rust/Zig means "create a variable", "overwrite memory", "copy a
struct" and "transfer ownership" depending on context. The reader cannot see
which machine operation occurs.

## Existing approaches
- C: `=` for everything; `const` as an afterthought.
- Rust: `let` / `let mut` / `=`; moves are implicit on assignment of non-`Copy` types.
- Zig: `const` / `var` / `=`; copies are implicit.
- Go: `:=` declares, `=` assigns.

## Oli-- approach
| Operator | Meaning | Left side |
|----------|---------|-----------|
| `name := expr` | **bind** a name to a value; immutable; no address | new name |
| `name : T` / `name : T <- expr` | declare a **place** (memory slot), optionally store | new place |
| `place <- expr` | **store** into a place | existing place, field, element, `[addr]` |
| `place <~ expr` | **move** an `own` value into a place; source dies | place of `own` type |

Bindings may live in registers; places always have an address. Passing a
layout by value is the only implicit copy, and `--explain-cost` reports it.

## Advantages
- The reader sees register-vs-memory and copy-vs-move at every line.
- Immutability is the default without a keyword (`let`/`const` not needed).
- `<-` visually points at memory: "value goes into slot".
- Move is a distinct, greppable operator.

## Disadvantages
- Three operators to learn; `:=` vs `<-` confusion is possible for newcomers
  (the compiler reports "cannot store to a binding; declare a place with `name : T`").
- Declaring a mutable counter takes a type annotation (`i : uword <- 0`), which is
  deliberate: a memory slot is a layout decision.

## Machine cost
`:=` ZERO. `<-` one store (or a register move after `mem2reg`). `<~` ZERO (handle copy).

## Safety implications
Accidental mutation of bindings is impossible; the address of a binding cannot
be taken, so no dangling pointers to register values; `own` misuse is a type error.

## Alternatives rejected
- `let`/`var` keywords: copies Rust/Zig and hides the machine distinction behind words.
- `=` for stores: the single most overloaded symbol in existing languages.
- `x <- 20` for both bind and store (as in some ML dialects): loses the binding/place split.
