# Oli-- ABI (x86-64)

> **Status (2026-09-22).** Partly implemented. The value layout rules of §3 are
> enforced today by `olic`'s item stage — every size, alignment and field
> offset in `tests/snapshots/*.sema` comes from them — and the argument
> registers of §1, the syscall convention of §5 and the ELF64 output of §7 are
> what `oli1` already emits for oli-core. What no program in the repository
> does yet is emit V0 code: the full type classification of §2, symbol naming
> (§4) and interrupt procedures (§8) are design until G4's back end lands.

Oli-- has one stable low-level ABI per target. On x86-64 it is the System V
AMD64 convention, chosen because it is the machine's own convention on Linux
and gives C interoperability at zero cost. Oli-- adds precise rules for its
own types (views, fallible results, choices) by classifying them exactly as
the equivalent C aggregates.

## 1. Procedure calls (`calls sysv`, the default)

| Item | Rule |
|------|------|
| integer/address arguments | `rdi, rsi, rdx, rcx, r8, r9`, then the stack (right to left) |
| floating arguments | `xmm0–xmm7` (`olic` since stage 41; a ninth float argument is E0900) |
| integer result | `rax` (and `rdx` for the second word) |
| floating result | `xmm0` (`xmm1`); a narrow integer (below 64 bits, `bool`) has undefined upper bits in SysV, so an `export`ed procedure re-extends its narrow parameters and the result of an `extern` call is re-extended |
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
| `T or E` | as the two-variant choice: tag `0` = ok, `1` = fail | `rax` = tag, `rdx` = payload if the whole value fits in 16 bytes (a layout of one word as its word); else `sret`: an area the caller owns — one word for the tag, then the words of the wider payload (a view's two, a layout's or a wide choice's bytes) — whose address is the hidden first argument and comes back in `rax` |
| `never` | — | control does not return; the compiler emits no code after the call |
| `[N]T` | never by value; pass a `view` or `ref` | — |

Rule of thumb: whatever a C compiler would do with the equivalent `struct`,
Oli-- does. This makes every Oli-- procedure callable from C with the obvious
prototype and every C function callable from Oli-- (`extern proc name(params) -> T`
declares one; stage 24). Two cautions: a narrow integer parameter (`s32`, `u8`, …)
of a procedure C calls is read as the canonical 64-bit image, which a C caller
does not promise for the upper bits — declare C-facing parameters as `s64`,
`u64`, `word` or `uword`; and a `view` is two words, not a C type — pass
`addr T` and a length.

## 3. Layout rules

- Fields are laid out in declaration order with natural alignment and padding, never reordered.
- `packed`: no padding, alignment 1; accesses use unaligned moves (`ZERO` cost on x86-64, `CHECK`-free).
- `align N` on a layout raises its alignment; `align N` on a field inserts padding before it.
- `be T` / `le T` fields occupy exactly the bytes of `T` in that byte order.
- `bool` is one byte holding 0 or 1. `choice` tags are the smallest unsigned integer that fits the variant count; a variant's tag is its number in declaration order, from 0. A choice whose image fits eight bytes travels in a register as that image (the tag in the low byte, each field at its offset); a wider one is held by its address and passed and returned like a layout of its size (the pair up to sixteen bytes, `sret` beyond). This is what `olic` does today.
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

Implemented (stage 23, 2026-09-24): every image `olic` writes carries `.symtab`
and `.strtab` with exactly the table above — `module.proc` (`STB_LOCAL` unless
`pub`), an `export` clause as a second `STB_GLOBAL` symbol, every static as
`module.name` (`STT_OBJECT`, with its size) in `.data`, `.bss` or `.rodata`, the
trap routine as `olic.trap` — and eight section headers (`.rodata`, `.text`,
`.data`, `.bss`, `.symtab`, `.strtab`, `.shstrtab`), so `nm`, `readelf -S`,
`objdump -d` and `gdb` read it as it is. The tables follow the loaded image
and are not mapped.

## 4a. AArch64 (`arch = "aarch64"`, stage 43)

The second back end (`compiler/a64.oli`) keeps the frame of §1 in AAPCS64
dress: `stp x29, x30, [sp, #-16]!; mov x29, sp`, the frame sixteen-byte
aligned, a value's word at `[x29 - 8 * (slot + 1)]` exactly as at
`[rbp - …]`; argument words in x0-x5 (a seventh is E0900 for now), results
in x0 and x1; float arguments in d0-d7 and a float result in d0; a stack
argument (past the sixth word) at `[sp + 8k]` at the call, read by the callee
at `[x29 + 16 + 8k]`; system calls with the number in x8 and `svc #0` — the
numbers are the target's (write 64, exit_group 94): `std.os` names them for
either architecture (`os.WRITE`, `os.MMAP`, …), or a program says which it
means with `when target.arch`. A float converted to an integer out of range
(or a NaN) is target-defined: x86-64 gives the integer indefinite 2^63,
AArch64 saturates and gives 0 for a NaN; every value in range agrees.
An AArch64 object file (`-- output: object`) carries R_AARCH64_CALL26 for a
call to an `extern` procedure, R_AARCH64_ADR_PREL_LO21 for a string or other
read-only static, and R_AARCH64_ADR_PREL_PG_HI21 with
R_AARCH64_ADD_ABS_LO12_NC for a writable one — position-independent, so it
links into an executable, a PIE or a shared library (`tests/c` under the
AArch64 profile). A freestanding AArch64 image (`os = "none"`) is entered
with no stack: its `calls none` entry keeps its values in x19-x28 and sets
sp with `cpu.stack <-` (`mov sp`); a trap stops the CPU in `wfi` with the
message in x0/x1. The entry procedure ends with
`exit_group(x0)`; `core.trap` writes its message with write(2, …) and exits
134, as on x86-64. The ELF is EM_AARCH64 (183), the DWARF frame base x29.

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
| sections | `.rodata`, the code groups in the profile's order (`.text.boot`, `.text`, named ones, `.text.trap`), `.data`, the zero groups (`.bss`, `.bss.boot`, …), `.symtab .strtab .shstrtab` — each with its own header (stage 28) |
| relocations | none in the executable: all addresses are resolved by the writer (RIP-relative for code/data, absolute 32-bit sign-extended in `code kernel`) |
| debug | `.symtab` always; DWARF 4 `.debug_line` (a row per change of source line, one file per module, `a.b` as `a/b.oli`) and `.debug_info`/`.debug_abbrev` (one compile unit — the root module, language `0x8000` — and a subprogram per procedure with its range and frame base, rbp) in every executable (stage 29); since stage 40 each subprogram's parameters and variables with their types — base types, pointers, arrays, views, a structure per layout — and, for a local that keeps its frame words, `DW_OP_fbreg` to its first byte (`rbp - 8 * (off + words)`); a local `mem2reg` promoted has no location (`<optimized out>`), and `-- debug: frame` keeps every local in its words; an object file carries none yet (its addresses would need relocations) |

`-- output: object` (stage 24) produces `ET_REL`: the same sections at address
0 each, no program headers, `.rela.text` with `R_X86_64_PC32` (addend
offset − 4) for every static the compiler's code reaches — since stage 39 every
static address is `lea r, [rip + disp32]`, in executables too, so the code is
position-independent and links into a PIE or a shared library without text
relocations — `R_X86_64_64`/`R_X86_64_32S` only for what a machine block
names absolutely (`mov r64, static`, `[static]`), `R_X86_64_32` for
`addr32`, all against the section symbol of what they name, and
`R_X86_64_PLT32` (addend −4) per call to an `extern` procedure, which is an
undefined global symbol under its bare name; an entry procedure is also
`_start`. Every call to an `extern` clears `al` first (a variadic C callee's
count of vector registers). The absolute relocations mean non-PIE linking:
`ld` as it is, or `cc -no-pie`; `tests/c` links two Oli-- objects with `ld`
and one with C by `cc`, and runs both.

## 8. Interrupt procedures (`calls interrupt`)

The CPU pushes `ss, rsp, rflags, cs, rip` (and an error code for some vectors).
The compiler emits: save of all caller-saved and callee-saved registers the body
touches, stack realignment, the body, restores, optional error-code pop, `iretq`.
The procedure takes `frame : ref InterruptFrame` (a `core` layout of the pushed
words) and returns nothing. It may not be called with `call`.
