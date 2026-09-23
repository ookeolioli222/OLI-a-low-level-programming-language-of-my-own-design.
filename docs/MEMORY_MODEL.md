# Oli-- Memory Model

This is the conceptual model. The subset the V0 compiler enforces is in
`spec/OLI_MEMORY_V0.md`.

## 1. Regions

All memory an Oli-- program touches belongs to exactly one **region**. A region
has a lifetime that the compiler either knows statically or the programmer
takes responsibility for.

| Region | Created by | Destroyed by | Lifetime known to compiler |
|--------|-----------|--------------|----------------------------|
| static | module-level places/bindings (`.data`, `.bss`, `.rodata`) | never | whole program |
| frame | places declared in a procedure body | `ret`/`fail` of that procedure | lexical |
| zone | `zone name size ... end` | the matching `end` | lexical |
| raw | `addr` values from the machine, from `at`, from other regions | programmer | **unknown** |

The compiler's central job in the memory model is to prove that no reference
into a region is used after the region is destroyed. For static, frame and
zone regions this is decidable lexically. For raw regions it is not, so raw
dereference requires the `memory.raw` capability and the responsibility
moves to the programmer.

## 2. Values and places

A **binding** (`x := e`) names a value. It has no address and cannot be stored
to. The compiler may keep it in a register or fold it away. Cost: `ZERO`.

A **place** (`x : T`, optionally `x : T <- e`) is a typed slot in a region.
It can be stored to with `<-`, read by naming it, and addressed with
`addr x` / `ref x`. Cost: `STACK` (frame), `ZONE` (zone) or nothing (static).

This separation is the machine's own: a value in a register has no address; a
value in memory does. Languages that give every variable an address make the
compiler guess which ones matter; Oli-- makes the programmer say so.

## 3. Address kinds

| Type | Meaning | Size | Bounds | Deref needs | Region tracked |
|------|---------|------|--------|-------------|----------------|
| `addr T` | raw virtual address of a `T` | 8 | none | `memory.raw` | no |
| `ref T` | reference to one live `T` | 8 | implicit (one object) | nothing | yes |
| `view T` | `(addr, len)` over `len` consecutive `T` | 16 | checked | nothing | yes |
| `own T` | unique holder of a resource `T` | size of T | n/a | n/a | linear |

Writability is a separate, explicit property: `rw ref T`, `rw view T`.
A read-only kind coerces to nothing more permissive; `rw` coerces to read-only.
String literals are `view u8` into `.rodata` and can therefore never be written.

Memory space is a third property: `mmio view T` / `mmio ref T` perform volatile
accesses that the optimizer must neither merge, reorder, nor remove.

### Why four kinds instead of one pointer

A C pointer is simultaneously an address, a possibly-null reference, an array
base and an ownership token; the compiler cannot check any of these because
it cannot tell which one is meant. Splitting them costs nothing at run time
(`ref` and `addr` are one register; `view` is two) and makes each check
possible: bounds on `view`, liveness on `ref`/`view`, single-use on `own`,
and an explicit capability on `addr`.

### Does this replace a borrow checker?

Rust's borrow checker answers two questions: (1) does a reference outlive its
referent? (2) is there a write while another reference is live? Oli-- answers
(1) with region tracking (§5) and answers (2) *only for the cases the machine
cares about*: an `mmio` access is never reordered, an `own` value cannot be
duplicated, and a `zone` cannot be freed while views into it exist. Plain data
aliasing between two `rw view u8` of the same buffer is allowed — as it is in
the machine — and the optimizer treats any two writable views as possibly
aliasing unless it can prove otherwise. What is lost: some optimizations Rust
can perform from `noalias`. What is gained: no lifetime annotations, no
"fighting the borrow checker", and a model that kernel code (which aliases
by nature) can actually use.

## 4. Zones

A zone is a lexically scoped bump region.

```oli
zone scratch 64K                 -- region: 64 KiB
    pkt := scratch.bytes(4096)   -- rw view u8 carved from the region: ZONE
    hdr := scratch.make(Header)  -- rw ref Header, aligned, zero-initialized: ZONE
    ...
end                              -- everything carved from scratch is dead here
```

Machine representation: three words — `base`, `cursor`, `limit`. Allocation is
`cursor = align(cursor); result = cursor; cursor += size; if cursor > limit: trap`.
Release is nothing (frame zones) or a single `munmap`/return-to-parent.

**Where the memory comes from** (explicit, never hidden):

| Form | Source | Cost |
|------|--------|------|
| `zone z N` inside another zone's block | carved from the enclosing zone | `ZONE` |
| `zone z N` with no enclosing zone, hosted | `mmap` from the OS; requires `permit os.syscall` | `SYSCALL` |
| `zone z N` with no enclosing zone, freestanding | **compile error**: no memory source | — |
| `zone z N at A` | raw memory at address `A`; requires `memory.raw` | `ZERO` |
| `zone z N from S` | carved from zone or allocator handle `S` | `ZONE` / `ALLOC` |

Zones nest lexically. A zone handle (`zone` type) may be passed to callees, which
may allocate from it; anything they return that points into it is tied to the
zone by the region rules below. There is no implicit "current zone": a procedure
that needs scratch memory receives a zone parameter, exactly as it would receive
an allocator. Zones are not thread-safe by design; a zone belongs to one thread.

`z.bytes(n)` traps on exhaustion (`TRAP`); `z.try_bytes(n)` returns `rw view u8 or none`.
V0 has no `reset`; a zone is released only at its `end`.

## 5. Escape analysis (the replacement for lifetimes)

Every `ref`/`view` value carries a **region** in the type checker (not at run time):
static, a specific frame, a specific zone, or a specific parameter's region.

Rules:

1. A value may be stored into a place only if the value's region outlives the
   place's region. (A frame place may hold a static view; a static place may not
   hold a frame view.)
2. A procedure result's region is, by default, the **intersection** of the regions
   of all `ref`/`view`/`zone` parameters: the result is assumed to be derived from
   any of them. This needs no annotation and is safe.
3. When rule 2 is too conservative, the result type names its source:
   `-> ref Header in z` (from zone parameter `z`) or `-> view u8 in pkt`.
4. A `ret`/`fail` of a value whose region is the current frame or a zone of the
   current procedure is an error: *"view escapes destroyed memory zone"*.
5. `addr T` has no region. Converting `ref`/`view` to `addr` is always allowed
   (`v.addr`); converting back requires `memory.raw` and is the programmer's
   responsibility.

The V0 analysis is per-procedure and flow-insensitive; it rejects some correct
programs (they can use `addr` under `memory.raw`) and accepts no incorrect ones.

## 6. Bounds and alignment

- `v[i]` is a `CHECK` (`i < v.len`) followed by a load or store. Proven-safe
  indices (loop counters bounded by `v.len`, constants against constant lengths)
  have the check removed; `--explain` reports how many were removed.
- `v[a..b]` is a `CHECK` (`a <= b <= v.len`) and produces a view: `ZERO`.
- `Layout.at(v)` checks `v.len >= Layout.size` and, unless the layout is `packed`,
  `v.addr % Layout.align == 0`. Both are `CHECK`.
- Frame places are aligned to their type's alignment; zones align every allocation
  to the requested type's alignment; `align N` overrides.
- A failed check is a `TRAP`, never undefined behavior.

## 7. Raw memory

`addr T` values come from `v.addr`, `addr place`, integer conversion
(`addr u8 (0xB8000)` — needs `memory.raw`), or the machine. The only operations
on them are arithmetic in bytes (`a + 8`), comparison, and — under `memory.raw` —
load/store with `[a]`. There are no bounds, no region, no alignment check; the
capability marks the procedure as the place where the programmer took over.

## 8. Ownership without destructors

`own T` is a **linear** value: it must be consumed exactly once — moved with `<~`
into a place or a parameter, or passed to a consuming procedure. Dropping it
silently is a compile error; using it after a move is a compile error. There is
no hidden destructor call; releasing a resource is a visible call to a
procedure that takes `own T`. This gives leak-freedom and double-free-freedom at
zero run-time cost and zero hidden control flow. V0 reserves the type; `std`
introduces the first `own` resources (file descriptors, mappings).

## 9. Memory spaces

| Type | Space | Deref | Arithmetic | Needs |
|------|-------|-------|-----------|-------|
| `addr T` | virtual | `[a]` | `+ uword` | `memory.raw` |
| `physaddr` | physical | never | `+ uword` | conversions via paging code |
| `mmio view T` / `mmio ref T` | device | volatile `[]`/fields | sub-views | `memory.mmio` to create |
| `port T` | x86 I/O ports | `p.in()`, `p.out(v)` | none | `io.port` |

`physaddr + addr` and `physaddr + virt` without an explicit conversion are type errors.

## 10. Cost summary

| Operation | Cost |
|-----------|------|
| bind, sub-view, `ref`→`addr`, widening | `ZERO` |
| element access, `Layout.at`, `+` overflow guard | `CHECK` |
| frame place | `STACK` |
| zone allocation | `ZONE` |
| top-level hosted zone | `SYSCALL` |
| layout passed by value, `mem.copy` | `COPY` (size reported) |
| `mmio` access | `KERNEL` (volatile, one instruction, never merged) |
