# 0004 — `addr` / `ref` / `view` / `own` instead of one pointer

## Problem
A single pointer type cannot be checked: the compiler cannot know whether it
is an array base (needs bounds), a reference (needs liveness), an owner
(needs release) or a raw machine address (needs nothing but trust).

## Existing approaches
- C: `T*` for everything; `size_t` length carried separately by convention.
- Rust: `&T`, `&mut T`, `&[T]`, `Box<T>`, `*const T`, `*mut T` — checked, but with lifetimes.
- Zig: `*T`, `[]T`, `[*]T`, `?*T` — distinguishes slices and many-pointers, no liveness.

## Oli-- approach
| Kind | Machine | Checks |
|------|---------|--------|
| `addr T` | one word | none; deref `[a]` needs `memory.raw` |
| `ref T` | one word | liveness via region tracking |
| `view T` | two words (`addr`, `len`) | liveness + bounds on `v[i]`, `v[a..b]` |
| `own T` | the resource itself | linear: consumed exactly once |

Writability (`rw`) and memory space (`mmio`) are orthogonal modifiers on `ref`
and `view`. There is no null: absence is expressed with `T or none`.

## Advantages
- Each kind enables exactly one class of check; none costs more than a C pointer.
- Views carry their length, so bounds checks need no extra state and can be elided by proof.
- `own` gives leak- and double-free-freedom without destructors.
- Raw access is greppable (`addr`, `[a]`, `memory.raw`).

## Disadvantages
- Four names to learn instead of one.
- Converting `addr` back into `ref`/`view` is the programmer's responsibility under `memory.raw`.
- No uniqueness guarantee on `rw` views: fewer `noalias` optimizations than Rust.

## Machine cost
ZERO beyond the checks listed above; `view` uses two registers instead of one.

## Safety implications
Bounds, liveness and single-use are checked; raw access is confined to declared
procedures. Aliasing of writable views is permitted and documented as such.

## Alternatives rejected
- Fat pointers everywhere (always carry length): doubles the size of every reference.
- Nullable references: the machine has no null; `T or none` makes absence explicit.
- Copying Rust's `&`/`&mut` uniqueness: kernel code aliases by nature (DMA buffers, MMIO).
