# 0012 — `physaddr`, `mmio`, `port` as distinct types

## Problem
Kernels handle physical addresses, virtual addresses, device registers and
I/O ports. In C they are all integers or pointers; mixing them compiles and
crashes.

## Existing approaches
- C kernels: `phys_addr_t` typedefs (no checking), `volatile` (misused), `inb/outb` macros.
- Rust kernels: newtypes `PhysAddr`/`VirtAddr`, `read_volatile`, `x86_64` crate ports.
- Zig: `volatile` pointers, no physical/virtual distinction.

## Oli-- approach
| Type | Properties |
|------|-----------|
| `addr T` | virtual address; deref `[a]` under `memory.raw` |
| `physaddr` | physical address; no deref; `+ uword` allowed; conversion to `addr` is an explicit procedure |
| `mmio view T` / `mmio ref T` | volatile accesses, never merged or reordered; created under `memory.mmio` |
| `port T` | I/O port number of width T; `p.in()`/`p.out(v)` under `io.port` |

`physaddr + addr` is a type error. `mmio` is a type modifier, not a statement-level
`volatile` cast, so it cannot be forgotten at one access site.

## Advantages
- The compiler catches the classic "used a physical address as a pointer" bug.
- Volatility is a property of the memory, not of each access.
- Port I/O is typed by width; no `outb`/`outw` confusion.

## Disadvantages
- More types; conversions are explicit calls.

## Machine cost
None; `mmio` merely disables optimizations on those accesses.

## Safety implications
Wrong address space and missing volatile are compile-time errors, not run-time mysteries.

## Alternatives rejected
- Integers for all addresses: no checking.
- `volatile` as an access modifier: forgettable per site.
