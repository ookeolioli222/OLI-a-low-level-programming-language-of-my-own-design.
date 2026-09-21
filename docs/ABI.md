# Oli-- ABI (x86-64)

Oli-- has one stable low-level ABI per target. On x86-64 it is the System V
AMD64 convention, chosen because it is the machine's own convention on Linux
and gives C interoperability at zero cost. Oli-- adds precise rules for its
own types (views, fallible results, choices) by classifying them exactly as
the equivalent C aggregates.

## 1. Procedure calls (`calls sysv`, the default)

| Item | Rule |
|------|------|
| integer/address arguments | `rdi, rsi, rdx, rcx, r8, r9`, then the stack (right to left) |
| floating arguments | `xmm0–xmm7` |
| integer result | `rax` (and `rdx` for the second word) |
| floating result | `xmm0` (`xmm1`) |
| caller-saved | `rax rcx rdx rsi rdi r8–r11 xmm0–xmm15` |
| callee-saved | `rbx rbp r12–r15` |
| stack alignment | `rsp % 16 == 0` immediately before `call` |
| frame pointer | V0 always emits `push rbp; mov rbp, rsp` (kept for `--explain` and debugging) |
| red zone | hosted: available but **not used** by V0 code generation; freestanding: disabled |

## 2. Classification of Oli-- types

| Oli-- type | Passed as | Returned in |
|------------|-----------|-------------|
| `u8…u64`, `s8…s64`, `word`, `uword`, `bool`, `addr T`, `ref T`, `physaddr`, `port T` | one INTEGER register | `rax` |
| `f32`, `f64` | one SSE register | `xmm0` |
| `view T` | two INTEGER registers: `addr` then `len` | `rax` = addr, `rdx` = len |
| `zone` (handle) | one INTEGER register holding the address of the `(base, cursor, limit)` triple | `rax` |
| layout ≤ 16 bytes, all-integer fields | up to two INTEGER registers (SysV eightbyte classification) | `rax`, `rdx` |
| layout > 16 bytes, or `packed` with unaligned fields | copied to the stack by the caller (`COPY`, reported) | hidden `sret` pointer in `rdi`; `rax` returns it |
| `choice` | as the equivalent C struct `{ tag; union payload }` | same |
| `T or E` | as the two-variant choice: tag `0` = ok, `1` = fail | `rax` = tag, `rdx` = payload if the whole value fits in 16 bytes; else `sret` |
| `never` | — | control does not return; the compiler emits no code after the call |
| `[N]T` | never by value; pass a `view` or `ref` | — |

Rule of thumb: whatever a C compiler would do with the equivalent `struct`,
Oli-- does. This makes every Oli-- procedure callable from C with the obvious
prototype and every C function callable from Oli-- (V2 adds the `extern` declaration syntax).

## 3. Layout rules

- Fields are laid out in declaration order with natural alignment and padding, never reordered.
- `packed`: no padding, alignment 1; accesses use unaligned moves (`ZERO` cost on x86-64, `CHECK`-free).
- `align N` on a layout raises its alignment; `align N` on a field inserts padding before it.
- `be T` / `le T` fields occupy exactly the bytes of `T` in that byte order.
- `bool` is one byte holding 0 or 1. `choice` tags are the smallest unsigned integer that fits the variant count.
- `view T` is `{ addr: u64, len: u64 }` in memory; `zone` is `{ base, cursor, limit: u64 }`.

## 4. Symbols

| Declaration | ELF symbol |
|-------------|-----------|
| `proc f` in module `a.b` | `a.b.f` (local binding, `STB_LOCAL`) |
| `pub proc f` in module `a.b` | `a.b.f` (`STB_GLOBAL`) |
| `export` clause | the procedure's bare name, e.g. `f`; `export "name"` chooses the name |
| `entry` clause | the symbol placed in `e_entry` |
| static places/bindings | `a.b.name`, in `.data`, `.bss` or `.rodata` (or the `section` clause) |

Dots are legal in ELF symbol names; no mangling scheme is needed until generics (V1),
which will append a stable hash of the instantiation arguments.

## 5. Linux syscall convention

| Item | Register |
|------|----------|
| number | `rax` |
| arguments 1–6 | `rdi rsi rdx r10 r8 r9` |
| result | `rax`; values in `-4095..-1` are `-errno` |
| clobbered by the kernel | `rcx`, `r11`, flags |

`os.syscall(nr, a1..a6)` returns `word`; arguments are `word`, `uword` or any `addr T`.
Numbers used by `core` on x86-64 Linux: `read 0`, `write 1`, `mmap 9`, `munmap 11`,
`exit 60`, `exit_group 231`.

## 6. Hosted startup

`olic` emits this `_start` for hosted programs (printed by `--explain`, never
hidden); it calls the procedure carrying the `entry` clause (design 0016):

```
_start:
    xor  ebp, ebp            ; outermost frame
    and  rsp, -16            ; ABI alignment
    call <entry>             ; the `entry` procedure: -> s32, no parameters
    mov  edi, eax            ; exit status
    mov  eax, 231            ; exit_group
    syscall
```

V0 entry procedures take no arguments. V1 passes `argc`/`argv` as `view view u8`
built from the initial stack (`[rsp] = argc`, `[rsp+8..] = argv`).

## 7. ELF64 output

| Property | Value |
|----------|-------|
| type | `ET_EXEC`, static, non-PIE, no `PT_INTERP`, no `PT_DYNAMIC` |
| machine | `EM_X86_64`, little-endian, `ELFCLASS64` |
| base address | `0x400000` hosted; `--load-address` / target profile in freestanding mode |
| segments | `PT_LOAD R+X` (`.text`), `PT_LOAD R` (`.rodata`), `PT_LOAD RW` (`.data` + `.bss`), page-aligned; `PT_GNU_STACK` non-executable |
| sections | `.text .rodata .data .bss .symtab .strtab .shstrtab`; user sections from `section` clauses |
| relocations | none in the executable: all addresses are resolved by the writer (RIP-relative for code/data, absolute 32-bit sign-extended in `code kernel`) |
| debug | `.symtab` always; DWARF line tables planned (tooling phase) |

`--emit-obj` (later) produces `ET_REL` with `R_X86_64_PC32`/`R_X86_64_64`
relocations for use with external loaders; it is not needed for Milestones 1–4.

## 8. Interrupt procedures (`calls interrupt`)

The CPU pushes `ss, rsp, rflags, cs, rip` (and an error code for some vectors).
The compiler emits: save of all caller-saved and callee-saved registers the body
touches, stack realignment, the body, restores, optional error-code pop, `iretq`.
The procedure takes `frame : ref InterruptFrame` (a `core` layout of the pushed
words) and returns nothing. It may not be called with `call`.
