# genesis/3-oli1 — the oli-core compiler: subset and step plan

`oli1` is layer 3 of the genesis chain (design 0017): a compiler for **oli-core**,
the smallest subset of Oli-- V0 sufficient to eventually express the self-hosted
compiler `olic` (layer 4). `oli1` is written in the `machine x64` sub-language and
assembled by layer 2 (`asm`); it reads oli-core source on stdin and writes a
native ELF64 to stdout. No foreign compiler, assembler or linker is involved.

## Pipeline

```
oli-core source ──> oli1 (an ELF built by asm from oli1.oli) ──> native ELF64
```

## Execution constraints inherited from `asm`

- `asm`-produced ELFs map only their file image, so `oli1` obtains scratch memory
  from an anonymous `mmap` (2 MiB: 1 MiB input, 1 MiB output), not a fixed address.
- `asm` has no 8/16-bit register ops yet, so `oli1` reads source bytes with a
  64-bit load masked to 8 bits (`mov rax,[rsi]; and rax,255`) and builds output
  from a fixed ELF template patched with 32-bit stores. When `oli1` needs to emit
  variable-length code, `asm` gains byte load/store (`movzx r,byte[m]`,
  `mov byte[m],al`) — the forms SPEC section 8 keeps in scope — as a separate,
  tested step, and this document is updated.

## oli-core V0 (target subset)

Frozen incrementally; each step adds grammar and is proven by a behavioural test
in `genesis/test.sh` (layer 3) — compile an oli-core program, run it, check the
observable result. Nothing is marked done without such a test.

| Step | oli-core it accepts | Lowering | Status |
|------|---------------------|----------|--------|
| 0 | `proc NAME` + `entry` + `ret <decimal>` + `end` | exit(value) | **done** |
| 1 | `permit os.syscall`; `os.syscall(nr, a1..a6)` (integer args); multi-statement bodies | mov-imm per arg reg + `syscall` | **done** |
| 2 | `name := <expr>` bindings; `ret <expr>`; `+ - *` with precedence; names as syscall args | symbol table + compile-time constant folding | **done** |
| 3 | string literals + `.addr`/`.len`; write a message (hello) | rodata + syscall | next |
| 4 | `if/elif/else`, `while`, comparisons | rel32 branches | planned |
| 5 | multiple `proc`s and calls; parameters (SysV) | call/ret, arg regs | planned |
| 6 | views, `zone`, layouts, `T or E` — enough for a compiler | frames + checks | planned |

Steps 2–6 grow oli-core until it can express `olic` (layer 4), at which point the
compiler is ported into oli-core, `oli1` compiles it, and the fixpoint
`stage2 == stage3` establishes self-hosting and `olic` passes every fixture in `tests/`.

## Step 0 (implemented)

Accepts a procedure whose body contains `ret <decimal>` and emits an ELF that
exits with that value. `oli1` scans for the `ret` keyword, skips spaces, parses a
decimal, copies a 132-byte template and patches the `mov edi, imm32` field with
the value. Proven by `genesis/3-oli1/tests/ret42.oli` and inline values 0/42/200
in the harness; `oli1` itself rebuilds deterministically from `oli1.oli`.

## Step 1 (implemented)

A procedure body is a sequence of statements emitted in order. `ret <int>` and
`os.syscall(nr, a1..a6)` are lowered by a real code generator: `emit_byte` writes
one output byte via an 8-byte store of a value < 256 (the seven trailing zeros
are overwritten by the next byte and never written past the true code length).
`os.syscall` loads `rax=nr` and `rdi,rsi,rdx,r10,r8,r9 = a1..a6` (each `mov r32,
imm32`, with a REX.B prefix for r8/r9/r10), then emits `0f 05`. The ELF header is
copied from a 120-byte template and its `p_filesz`/`p_memsz` patched with the
final size. Proven by `syscall_exit.oli`, a getpid-then-`ret` multi-statement
program and a six-argument call in the harness (layer 3).

## Step 2 (implemented)

Adds a symbol table (`name` -> compile-time integer value) in the mmap scratch,
an expression grammar with precedence (`expr = term (('+'|'-') term)*`,
`term = factor ('*' factor)*`, `factor = int | name`) evaluated by constant
folding, and `name := <expr>` bindings. `ret` and each `os.syscall` argument
accept a full expression, so `os.syscall(60, code)` and `ret a * b + 1` compile.
Because the step-2 subset is entirely compile-time constant, expressions are
folded in the compiler and lowered to `mov <reg>, imm32`; runtime evaluation
(stack frame + register allocation) arrives when non-constant sources appear.
Proven by `bind.oli` (42), `arith.oli` (2+3*4 = 14), `prec.oli` (5*3+1 = 16) and
`nameos.oli` (7) in the harness. A bug worth noting was fixed: the whitespace
skipper must preserve the accumulator register, since the expression evaluator
calls it while a computed operand is live.

## Diagnostics

Out-of-scope or malformed input must be rejected without emitting a partial ELF,
following `asm`'s convention (a non-zero exit and an `oli1:` diagnostic). Step 0
exits non-zero when no `ret` is found; richer diagnostics arrive with the lexer
in later steps.
