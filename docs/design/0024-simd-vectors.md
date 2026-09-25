# 0024 — 128-bit vector types

## Problem
`LANGUAGE_VISION.md` promises "native SIMD", `KERNEL_PROGRAMMING.md` says
"native vector types in V2", and `oli.compute`/`oli.sec` need them; until
now the only way to a vector register was a `machine` block. A vector type
needs a representation, operations whose cost is visible, and a rule for
what the target does not have.

## Existing approaches
- C: vendor intrinsics (`_mm_add_ps`), one name per instruction.
- Rust `std::simd`, Zig `@Vector(4, f32)`: a generic vector type with
  operators, lowered to whatever the target has, sometimes to a loop.
- GCC vector extensions: `typedef float v4 __attribute__((vector_size(16)))`.

## Oli-- approach
Ten 128-bit types — `f32x4 f64x2 s8x16 u8x16 s16x8 u16x8 s32x4 u32x4
s64x2 u64x2` — each sixteen bytes, aligned to sixteen, a value like a
layout held by value (copied, passed and returned as its two eightbytes
between Oli-- procedures). The baseline is SSE2, which every x86-64 CPU has,
so nothing depends on a CPU feature test:

| Form | Meaning | Instruction |
|------|---------|-------------|
| `f32x4(a, b, c, d)` | the lanes, in order (each typed as the lane) | stores |
| `T.splat(x)` | every lane `x` | stores |
| `T.load(v, i)` / `T.store(v, i, x)` | lanes `v[i..i+N]` of a view of the lane type, bounds-checked | copy |
| `x[i]` / `x[i] <- e` | one lane, bounds-checked | load / store |
| `a + b`, `a - b` | per lane; integers wrap (lane arithmetic has no overflow trap) | `addps/addpd/paddb..paddq`, `sub…` |
| `a * b` | `f32x4`, `f64x2`, `s16x8`/`u16x8` (low half, wrapping) | `mulps/mulpd/pmullw` |
| `a / b` | `f32x4`, `f64x2` | `divps/divpd` |
| `a & b`, `a \| b`, `a ^ b` | every type (bits) | `andps/orps/xorps`, `pand/por/pxor` |
| `a == b`, `a != b` | the whole vector: every lane equal (floats: a NaN lane is never equal) | `pcmpeqb`/`cmpeqps` + `pmovmskb`/`movmskps` |

Anything else on a vector — an operation SSE2 has no instruction for
(`*` on 8-, 32- or 64-bit integer lanes, a lane comparison `<`) — is `E0900`,
not a loop behind the reader's back. A vector parameter or result of an
`extern` or `export` procedure is `E0900` too: SysV passes `__m128` in an
xmm register, which the aggregate path does not do.

## Machine cost
Each operator is two unaligned loads (`movdqu`), the instruction, and one
store into a sixteen-byte frame area — ZERO class, no call, no check.
`load`/`store`/`x[i]` carry the bounds check a view access carries.

## Safety implications
None beyond views': lanes and loads are bounds-checked like any element.
Integer lane arithmetic wraps, as every SIMD instruction set does; the
table says so and `T.wrap` is not needed.

## Alternatives rejected
- Intrinsics named after instructions: the reader learns Intel's names.
- A generic `vector(N, T)`: sizes other than 128 bits need AVX or a loop;
  both would hide cost. Wider vectors can come as `f32x8` with an explicit
  `target.cpu` requirement.
- Lowering missing operations to scalar loops: a vector type that is
  sometimes a loop is not a machine type.

## Status
Implemented 2026-09-25 (stage 42): `tests/run/simd.oli`.
