# 0023 — The narrow forms a `machine x64` block may use

## Problem
`genesis/2-asm/SPEC.md` §8 leaves 8-bit and 16-bit registers out of the
assembler's subset until a design record says which forms exist and why. A
kernel cannot avoid three of them: port I/O goes through `al`/`ax`/`eax` and
`dx`, a segment register is loaded from `ax`, and `retfq` reloads `cs`.

## Existing approaches
Every x86 assembler accepts every register name; the cost is that a block
can name a narrow register whose upper bits the compiler's own code relies on,
and nothing says so.

## Oli-- approach
The encoder in `compiler/asm.oli` accepts exactly these forms, and nothing
else that names a narrow register (E0900 otherwise):

| Form | Bytes |
|------|-------|
| `in al, dx` / `in ax, dx` / `in eax, dx` | `EC` / `66 ED` / `ED` |
| `in al, imm8` / `in ax, imm8` / `in eax, imm8` | `E4 ib` / `66 E5 ib` / `E5 ib` |
| `out dx, al` / `out dx, ax` / `out dx, eax` | `EE` / `66 EF` / `EF` |
| `out imm8, al` / `out imm8, ax` / `out imm8, eax` | `E6 ib` / `66 E7 ib` / `E7 ib` |
| `mov es\|cs\|ss\|ds\|fs\|gs, ax` | `8E /r` |
| `mov ax, es\|cs\|ss\|ds\|fs\|gs` | `8C /r` |
| `mov ax, imm16` | `66 B8 iw` |
| `retfq` | `48 CB` |
| `pushfq` / `popfq` | `9C` / `9D` |
| `int imm8` / `int3` | `CD ib` / `CC` |
| `lea r64, [.label]` | `48 8D /r` with `[rip + rel32]` to the label |

The last three are not narrow forms; they are listed here because they
entered the encoder with the interrupt work of M4 (a handler is tested in a
hosted program by pushing the frame the CPU would push and jumping to it).

`in`/`out` directives (`in REG <- e`, `out REG -> place`) keep r64 and r32
only: the accumulator's low byte is what the block computes from a value
that arrived in `eax`, which is how `examples/kernel.oli` writes COM1
(`in eax <- u32(byte)`, `out dx, al`).

## Machine cost
Exactly the listed bytes. A 16-bit form costs the `66` prefix.

## Safety implications
None beyond a block's: the callee-saved rule of design 0009 is unchanged
(`ax` is `rax`, never callee-saved), and a segment load is a `cpu.asm`
matter as before.

## Alternatives rejected
- Accepting every narrow register: the subset would stop being the one the
  genesis assembler proves.
- Intrinsics `cpu.inb`/`cpu.outb` instead: they would be exactly these bytes
  with a call around them; they can still come later as `core` wrappers.
