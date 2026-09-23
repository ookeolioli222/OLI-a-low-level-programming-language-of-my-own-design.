# 0016 — `entry` is the only entry point, hosted or freestanding (ACCEPTED)

Status: accepted 2026-09-20; specification text pending.

## Problem
Hosted programs start at a procedure that must be called `main` (a C/Unix
convention), freestanding ones at the procedure with the `entry` clause. Two
rules for one concept, and one of them is inherited from another language.

## Existing approaches
- C, Rust, Zig, Go: a magic name (`main`), with the real `_start` hidden by the linker.
- Freestanding Rust/Zig: `#[no_mangle] _start` or a linker-script symbol.

## Oli-- approach
Exactly one procedure per program carries `entry`, on every target:

| Target | `entry` signature | What the compiler adds |
|--------|-------------------|------------------------|
| hosted | `-> s32` (exit status), no parameters | the five-instruction `_start` from `docs/ABI.md` §6 that calls it and exits |
| freestanding | `-> never`, no parameters, usually `calls none` | nothing |

`main` has no special meaning anywhere. `hello.oli` becomes:

```oli
proc start -> s32
    entry
    permit os.syscall
    msg := "Hello Oli--\n"
    os.syscall(os.WRITE, 1, msg.addr, msg.len)
    ret 0
end
```

## Advantages
- One rule; the entry point is visible in the header, not implied by a name.
- The hosted startup stays explicit (printed by `--explain`), as before.

## Disadvantages
- Programmers coming from C look for `main`; the diagnostic for a program
  without `entry` can say so.

## Machine cost
None.

## Safety implications
None.

## Alternatives rejected
- Keeping `main` for hosted programs: an inherited convention with no machine reason.
- A `program` block instead of a procedure: a second way to write a body.
