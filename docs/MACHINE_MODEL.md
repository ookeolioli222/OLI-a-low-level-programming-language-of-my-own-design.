# Oli-- Machine Model

The abstract machine that Oli-- semantics are defined against. It is
deliberately close to a real x86-64 (or any modern register machine) so that
every language construct has an obvious lowering and an honest cost.

## 1. Storage

| Storage | Properties |
|---------|-----------|
| **registers** | a small fixed set of machine words; bindings may live here; no address |
| **frame** | per-activation memory; created by the procedure prologue, destroyed at `ret`; holds places |
| **zone** | bump region: `(base, cursor, limit)`; holds places and byte buffers |
| **static** | memory in the executable image: `.rodata` (read-only), `.data` (initialized), `.bss` (zeroed) |
| **raw** | any address the program computes; the compiler knows nothing about it |
| **device** | MMIO ranges and I/O ports; every access is a visible side effect |

A **word** is the machine's natural width: 64 bits on x86-64. `word` is the
signed and `uword` the unsigned word type; `addr T` is word-sized.

## 2. Types and their representation

| Type | Bits | Representation |
|------|------|----------------|
| `u8 u16 u32 u64` / `s8 s16 s32 s64` | as named | two's complement; `sN` sign-extends on widening, `uN` zero-extends |
| `word` / `uword` | 64 | machine word |
| `byte` | 8 | alias of `u8` |
| `bool` | 8 | `0` or `1`; any other bit pattern is a program error, never observed by safe code |
| `f32` / `f64` | 32/64 | IEEE-754 binary32/64 in SSE registers |
| `be T` / `le T` | as T | integer stored in that byte order; loads/stores insert `bswap` when it differs from the target order |
| `addr T` | 64 | virtual address |
| `physaddr` | 64 | physical address; no dereference |
| `port T` | 16 | I/O port number; accessed with `in`/`out` of width T |
| `ref T` | 64 | address of one live T |
| `view T` | 128 | `{ addr: 64, len: 64 }`, len counts elements |
| `zone` | 192 | `{ base, cursor, limit }`; passed as a `ref` to this triple |
| `[N]T` | N·size(T) | contiguous elements, no header |
| `layout` | sum of fields + padding | fields in declaration order; natural alignment unless `packed`/`align` |
| `choice` | tag + max payload | tag is the smallest unsigned integer that fits; payload follows at its alignment |
| `T or E` | as a two-variant choice | tag `0` = ok, `1` = fail |
| `never` | 0 | no value; control does not return |

Layout of every type is **fixed and documented**; the compiler never reorders
fields. `Layout.size`, `Layout.align` and `Layout.field.offset` are compile-time constants.

## 3. Operations and their lowering

| Operation | Lowering (x86-64) | Cost |
|-----------|-------------------|------|
| `a + b` (trapping) | `add` + `jo trap` (`jc` for unsigned) | `CHECK` |
| `wrap(a + b)` | `add` | `ZERO` |
| `sat(a + b)` | `add` + `cmov` | `ZERO` |
| `checked(a + b)` | `add` + `setc/seto` into the tag | `ZERO` (the caller's `case` is the branch) |
| `a / b` | `div`/`idiv` after a zero check; `MIN / -1` traps | `CHECK` |
| comparisons, bit ops, shifts | one instruction; shift counts are masked to width (defined) | `ZERO` |
| `v[i]` | `cmp i, len` + `jae trap` + `mov` | `CHECK` |
| `[a]` | `mov` | `ZERO` |
| `mmio` access | `mov` marked volatile: never removed, merged, split or reordered with other volatile accesses | `KERNEL` |
| `p.out(v)` / `p.in()` | `out`/`in` | `KERNEL` |
| `x <- e` (place) | `mov [frame+off], reg` | `ZERO` |
| layout by value | `rep movsb` or unrolled moves | `COPY` |
| `zone.bytes(n)` | add, cmp, `ja trap` | `ZONE` |
| `call` | argument moves + `call` | `CALL` |
| `os.syscall` | register moves + `syscall` | `SYSCALL` |
| `fail e` / `ret v` | tag + payload into result registers, `ret` | `ZERO` |
| `cpu.halt()` | `hlt` | `KERNEL` |
| trap | `ud2` or `call core.trap` | `TRAP` |

## 4. Traps

A trap is the defined outcome of a failed check. Kinds: `bounds`, `overflow`,
`div_zero`, `misaligned`, `zone_exhausted`, `unreachable`, `assert`
(`core.TrapKind`).

| Mode | Behavior |
|------|----------|
| hosted (Linux) | `call core.trap` — a ~20-instruction routine that writes `trap: <kind> at <module>:<line>` to fd 2 with `write` and calls `exit_group(134)`. No libc. Included in the binary only if a trap site exists. |
| freestanding | the program's procedure declared with the `traps` clause is called with the kind and location; if none is declared, `ud2` is emitted. |

Traps are not exceptions: nothing unwinds, nothing is caught. A program that
wants to recover uses `checked(..)`, `try_bytes`, or a fallible result.

## 5. Memory spaces, volatility, ordering

- Ordinary loads/stores may be reordered, merged or removed by the optimizer
  when the program cannot observe the difference in a single thread.
- `mmio` accesses are volatile: each is performed exactly once, in program order
  relative to other `mmio` accesses and to `cpu.fence()`.
- Port I/O is volatile in the same sense.
- Atomics (V1): `atomic.load/store/add/cas(ref rw T, ..., order)` with `relaxed`,
  `acquire`, `release`, `acq_rel`, `seq_cst`; `cpu.fence(order)`. Lowering to
  `lock`-prefixed instructions and `mfence`.
- V0 defines only single-threaded execution.

## 6. Control transfer

| Construct | Machine |
|-----------|---------|
| procedure (`calls sysv`, the default) | System V AMD64 prologue/epilogue; see `ABI.md` |
| `calls none` | no prologue/epilogue; body must be a single `machine` block |
| `calls interrupt` | saves all caller-visible registers, aligns the stack, ends with `iretq` |
| `-> never` | no `ret` is emitted; falling off the end is a compile error |
| `entry` clause | the symbol the ELF header points to; no hidden prologue |
| hosted `main` | reached from a 5-instruction generated `_start` (documented, printed by `--explain`) |

## 7. What is undefined in C and what Oli-- does instead

| C undefined behavior | Oli-- |
|----------------------|-------|
| signed overflow | trap; or `wrap`/`sat`/`checked` by choice |
| shift ≥ width | count masked to width (x86 semantics), documented |
| out-of-bounds index | `CHECK` → trap |
| null dereference | `ref` is never null; `addr` deref needs `memory.raw` (programmer's contract) |
| use after free | zone/frame escape rejected at compile time |
| uninitialized read | rejected at compile time (definite assignment); statics are zeroed |
| misaligned access | `CHECK` on `Layout.at`; `packed` layouts use unaligned moves |
| data race | V0: single thread; V1: atomics have defined orders |
| `INT_MIN / -1` | trap |
| division by zero | trap |
| strict aliasing violations | none: memory is bytes; `Layout.at` is the sanctioned reinterpretation |

## 8. What `--explain` reports per procedure

frame size · register assignment of parameters and long-lived bindings ·
allocations (`ZONE`/`SYSCALL`) · copies with sizes · views created · checks emitted
and checks removed (with the proof kind) · syscalls · capabilities used · machine
blocks and their clobbers · total instruction count.
