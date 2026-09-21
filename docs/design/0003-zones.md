# 0003 — Zones as the allocation model

## Problem
Programs need dynamic memory without a garbage collector, without per-object
`malloc`/`free`, without hidden allocations, and with a compile-time
guarantee that no reference outlives its memory.

## Existing approaches
- C: `malloc`/`free` per object; leaks and use-after-free are the two most common bugs.
- Rust: ownership + `Drop` + borrow checker; arenas exist as libraries (`bumpalo`) with lifetimes.
- Zig: explicit allocator parameters; arenas as a library; no lifetime checking.
- Region-based languages (Cyclone, MLKit): regions with region annotations.

## Oli-- approach
`zone name size ... end` is a language construct: a lexically scoped bump
region. Objects (`z.make(T)`) and buffers (`z.bytes(n)`) are carved from it;
everything is released at `end`. The type checker tags every `ref`/`view`
with its region and rejects escapes. Memory sources are explicit: parent zone,
`os` (hosted only, one `mmap` syscall), `at ADDR` (raw), `from HANDLE`.
There is no implicit current zone; procedures receive zone handles as parameters.

## Advantages
- One free per region instead of one per object; no leaks; no double free.
- Escape checking is lexical, so no lifetime syntax is needed in typical code.
- Bump allocation is 3 instructions; predictable and cache-friendly.
- Maps directly to how kernels and servers actually manage per-request memory.

## Disadvantages
- Memory in a zone is not reclaimed until the zone ends (no per-object free).
- Long-lived, individually freed objects need a pool/slab allocator (library, V1).
- The default "result region = intersection of parameters" is conservative;
  `in z` annotations are needed occasionally.

## Machine cost
`ZONE` per allocation (add, compare, branch). `SYSCALL` once per top-level hosted zone.
`ZERO` for `at`/`from` zones.

## Safety implications
Use-after-free through zones is a compile-time error. Zone exhaustion is a
trap (or a fallible `try_bytes`). Zones are single-threaded by design.

## Alternatives rejected
- Garbage collection: hidden cost, runtime, unusable in kernels.
- Rust-style borrow checking: requires lifetime annotations; rejects aliasing kernels need.
- Implicit thread-local "current arena": hidden state, breaks the no-hidden-allocation rule.
