# Oli-- Memory V0 (normative)

The subset of `docs/MEMORY_MODEL.md` that the V0 compiler implements and
enforces. Everything here is a test obligation for the bootstrap compiler.

## 1. Regions in V0

| Region | Holds | Lifetime |
|--------|-------|----------|
| `static` | module-level places (`.data`/`.bss`), aggregate constants and string literals (`.rodata`) | program |
| `frame(P)` | places declared in procedure `P`'s body (all nesting levels) | activation of `P` |
| `zone(Z)` | objects and buffers allocated from zone `Z` | the `zone Z ... end` block |
| `param(i)` | whatever parameter `i` points to | at least the current activation |
| `raw` | `addr T` values | unknown |

The compiler assigns a region to every `ref`, `view` and `zone` value at
type-checking time. Integers, `bool`, `physaddr` and `addr T` carry no region.

## 2. Places

- A **frame place** `x : T` occupies `T.size` bytes at `T.align` in the frame;
  `[N]T` places occupy `N * T.size`. A *scalar* place (integer, `bool`, address,
  `ref`, `view`, `zone`) is not initialized; reading it before a store is `E0220`.
  Definite assignment is flow-sensitive over `if`/`case` branches and loops (a
  store inside a `while`/`each` body does not count for code after the loop; a
  store before `break` in `loop` does). An *aggregate* place (`[N]T`, layout,
  choice, `T or E`) without an initializer is zero-filled at its declaration -
  a visible `STACK` cost reported by `--explain-cost` - because element-wise
  initialization cannot be proven complete; a store into one field or element
  never makes the whole place "assigned". A `machine` block's `out REG -> p`
  counts as a store to `p`.
- A **static place** `x : T` (module level) lives in `.bss` (no initializer, zeroed)
  or `.data` (`<- e` with a constant initializer). `section`/`align` clauses apply.
- A **static binding** `X : [N]T := { ... }` or `X : Name := Name { ... }` lives in
  `.rodata`; it is a read-only place: `addr X` is allowed, `X <- ...` is `E0111`.
- A **zone object** comes from `z.make(T)` (`rw ref T`) or `z.bytes(n)` (`rw view u8`);
  it is zero-filled.

## 3. Zones

```
zone NAME SIZE                  -- source: enclosing zone block of the same procedure,
                                --         else the OS (hosted, needs permit os.syscall),
                                --         else E0330 (freestanding: no memory source)
zone NAME SIZE at ADDR          -- source: raw memory at ADDR : addr u8 (needs memory.raw)
zone NAME SIZE from SOURCE      -- source: a zone handle (V0) or an allocator (V1)
```

- `SIZE` is a `uword` expression; for OS-sourced zones it is rounded up to 4096.
- Inside the block `NAME : zone` is a handle usable in expressions and passable to callees.
- `z.bytes(n)` returns `n` bytes aligned to 16; `z.make(T)` returns `T.size` bytes aligned
  to `T.align`. Both trap with `zone_exhausted` when the cursor would pass the limit.
  `z.try_bytes(n)` returns `none` instead of trapping.
- At `end` the zone is released: OS-sourced → `munmap` (`SYSCALL`); parent-sourced → the
  parent's cursor is restored; `at`/`from` → nothing.
- Zones are released in reverse creation order; `break`/`continue`/`ret`/`fail` out of a zone
  block release it on the way out (the compiler emits the release on every exit edge).
- A zone handle's region is `zone(Z)`; storing it in a place that outlives the block is `E0300`.

## 4. Views and references

| Expression | Result type | Region | Check |
|------------|-------------|--------|-------|
| string literal | `view u8` | static | none |
| array place `a` used as a view | `view T` / `rw view T` | region of `a` | none |
| `z.bytes(n)` | `rw view u8` | `zone(z)` | `zone_exhausted` |
| `z.make(T)` | `rw ref T` | `zone(z)` | `zone_exhausted` |
| `ref x` / `rw ref x` | `ref T` / `rw ref T` | region of place `x` | none |
| `v[a..b]`, `v[a..]` | same kind as `v` | region of `v` | `bounds` |
| `Name.at(v)` | `ref Name` / `rw ref Name` | region of `v` | `bounds`, `align` unless `packed` |
| `r.f` for a layout field of layout type | `ref F` / `rw ref F` | region of `r` | none |
| parameter `p : view T` | as declared | `param(i)` | none |

- `rw` coerces to non-`rw`; never the reverse. `mmio` never coerces away.
- `v[i]` loads/stores one element after a `bounds` check. `each x in v` needs no per-element check.
- Bounds and alignment checks are removed only with a proof (constant index against
  constant length; `each` induction; a dominating identical check). `--explain` lists them.

## 5. Region rules (escape analysis)

Let `outlives(A, B)` hold when region `A` is guaranteed to live at least as long as `B`:
`static` outlives everything; `param(i)` outlives `frame(P)` and every `zone` of `P`;
`frame(P)` outlives every `zone` created in `P`; an outer zone outlives an inner zone.

1. **Store rule.** `p <- v` where `v` carries region `R` and place `p` lives in region `S`
   requires `outlives(R, S)`. Otherwise `E0300`.
2. **Return rule.** `ret v` / `fail v` where `v` carries `frame(P)` or any `zone` of `P`
   is `E0300: view escapes destroyed memory`.
3. **Parameter rule (default).** The result of calling `f(a1..an)` carries the
   **intersection** of the regions of all `ref`/`view`/`zone` arguments: it is treated
   as possibly derived from each of them. With no such arguments the result is `static`.
4. **Binding rule.** A binding carries the region of its initializer.
5. **Case rule.** A payload bound by `when ok x` / `when variant { f }` carries the region of the scrutinee.

Named result regions (`-> ref T in z`) are V1; V0 programs that need more
precision than rule 3 pass an explicit `zone` parameter and allocate the result from it,
which rule 3 already handles (the result carries the zone parameter's region).

The analysis is per procedure and needs no annotations; it rejects some safe
programs and accepts no unsafe ones with respect to static, frame and zone regions.

## 6. Raw memory

- `addr T` values arise from `v.addr`, `addr place`, `addr ref`, `addr f`, and
  `addr T (n)` (integer to address, `memory.raw`).
- Operations: `a + n` / `a - n` (bytes, `uword`), comparisons, `[a]` load and `[a] <- e`
  store (`memory.raw`), `uword(a)` (`memory.raw`).
- `[a]` has no bounds, no alignment check, no region. It is the programmer's contract.
- `physaddr` never dereferences; `physaddr(n)` and `uword(p)` need no permit.

## 7. Alignment

Every place and zone allocation is aligned to its type's alignment; `align N`
raises it. Unaligned access can only arise through `Name.at` on a non-`packed`
layout (checked) or through raw addresses (unchecked, `memory.raw`).

## 8. Copies

A copy occurs only when: a layout is passed or returned by value; a layout place
is stored from another layout value (`h2 <- h1`); `mem.copy`/`set`/`zero` run.
`--explain-cost` prints every one with its byte size:

```
packet_demo.oli:41  copy      12 bytes   (Header by value)
packet_demo.oli:58  zone      4096 bytes (scratch.bytes)
packet_demo.oli:57  syscall   mmap 65536 bytes (zone scratch)
packet_demo.oli:60  view      zero-copy (pkt[0..4])
```

## 9. Test obligations for the V0 compiler

Must accept: string literal returned from a procedure (static); sub-view of a
parameter returned (`param`); `Header.at(pkt)` returned when `pkt` is a parameter;
result of `z.bytes` returned when `z` is a zone parameter; nested zones; `break`
out of a zone block; `each` over views without per-element checks.

Must reject with `E0300`: returning a view of a frame array; returning `z.bytes`
of a local zone; storing a local zone's view into a static place; storing an
inner zone's view into a place declared in an outer zone's block; binding a
zone handle and using it after `end`.

Must reject otherwise: `[a]` without `memory.raw` (`E0401`); store through
`view u8` (`E0111`); `x <- e` for a binding (`E0110`); read before store (`E0220`);
`physaddr + addr` (`E0203`); top-level zone in freestanding without `at`/`from` (`E0330`).

## 10. Revision history

| Date | Change | Reason |
|------|--------|--------|
| 2026-09-18 | V0 memory rules written | Phase 0 |
| 2026-09-20 | §2 aggregate places are zero-filled; scalar places are tracked | definite assignment of arrays and layouts is otherwise unprovable |
