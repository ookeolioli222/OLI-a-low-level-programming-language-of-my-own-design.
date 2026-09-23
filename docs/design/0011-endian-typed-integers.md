# 0011 — `be T` / `le T` integer types

## Problem
Binary formats and network protocols store integers in a fixed byte order.
Reading them with the wrong order is a silent bug; converting by hand
(`ntohs`) is error-prone and pollutes parsing code.

## Existing approaches
- C: `ntohs`/`htonl` calls; `__attribute__((scalar_storage_order))` in GCC only.
- Rust: `u32::from_be_bytes`, `byteorder` crate, `zerocopy` with `U32<BigEndian>` types.
- Zig: `std.mem.readInt(u32, bytes, .big)`.

## Oli-- approach
`be u16`, `le u32`, `be s64` … are integer types whose memory representation
is in the named byte order. Loading one yields the native integer (the
compiler inserts `bswap` when needed); storing converts back. They are used
in `layout` fields and in `mem.put_be16`-style helpers.

## Advantages
- A packet header is declared once with its wire order and read like a struct.
- No conversion calls to forget; the type system tracks order.
- Zero cost on matching order; one `bswap`/`movbe` otherwise.

## Disadvantages
- Two more type constructors.
- Arithmetic on a `be` value requires reading it into a native integer first (automatic on load).

## Machine cost
`ZERO` or one `bswap`.

## Safety implications
Prevents an entire bug class in parsers, the core of the security tooling goals.

## Alternatives rejected
- Library-only conversion helpers: forgettable.
- Layout-level attribute (`layout X be`): too coarse; headers mix orders.
