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
| 3 | string literals with every §2.4 escape; `name.addr`/`name.len` as factors; hello | string pool before code, fixed addresses | **done** |
| 4 | run-time locals (frame slots), `<-` stores, `/ %`, unary `-`, comparisons, `if/elif/else`, `while`, `break`, `continue`, `os.syscall` as a value | frame + rax/stack codegen, rel32 branches with fixups | **done** |
| 5 | several `proc`s per file, parameters (`p: T`, up to six), `-> T`, calls as statements and factors, forward calls, recursion | SysV registers, `call rel32` with fixups, `leave; ret` | **done** |
| 6 | views, `zone`, layouts, `T or E` — enough for a compiler | frames + checks | next |

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

## Step 3 (implemented)

`name := "text"` copies the literal, with every escape of `spec/OLI_SYNTAX_V0.md`
§2.4 (`\n \t \r \0 \\ \" \' \xHH`), into a string pool in scratch. Symbol
entries grow to 32 bytes: name pointer, name length, value, kind (0 = integer,
string length + 1 = string). The output file is `header | pool | code`, so a
string's address is fixed at bind time — `0x400078` + its pool offset — and no
fixups are needed; at finalize the code is moved behind the pool and `e_entry`
is patched to `0x400078 + pool size`. `name.addr` and `name.len` are factors of
the expression grammar; `.addr`/`.len` on an integer, an unknown member, an
unterminated string, a bad escape or an undefined name reject with exit 2, no
stdout and `oli1: error` on stderr. Proven by `hello.oli` (byte-exact stdout),
`escapes.oli` (all ten escape bytes), `two_strings.oli` (two pools entries,
integer binding between them, exit 5), `strlen.oli` (`greeting.len + extra` = 42)
and six rejection programs in the harness. Limits: 64 KiB of string data.

## Step 4 (implemented)

The constant folder of step 2 is replaced by a code generator. Every integer
binding gets an 8-byte slot in a stack frame (`push rbp; mov rbp,rsp; sub
rsp,FRAME`, FRAME patched at the end of the procedure to 16-byte alignment) and
lives at `[rbp - 8*(slot+1)]`. Expressions evaluate into `rax`: the left operand
is pushed, the right evaluated, then `mov rcx,rax; pop rax; op rax,rcx`.
Grammar: `expr = add [cmpop add]`, `add = mul {(+|-) mul}`, `mul = unary
{(*|/|%) unary}`, `unary = - unary | primary`, `primary = int | name |
name.addr | name.len | os.syscall(args) | (expr)`. Comparisons produce 1/0 via
`cmp; setcc al; movzx eax,al`; `/` and `%` are signed (`cqo; idiv`). Integer
literals are emitted as `mov rax, imm64`.

`os.syscall(...)` is an expression: each argument is evaluated and pushed, the
values are popped into `rax, rdi, rsi, rdx, r10, r8, r9` in reverse order, then
`syscall`; the result in `rax` is the value, so `buf := os.syscall(9, ...)`
and `n := os.syscall(0, 0, buf, 4096)` give oli-core programs run-time input
(`tests/echo.oli`).

Control flow: `if`/`elif`/`else`/`end` and `while`/`end` compile to
`test rax,rax; jz rel32` and `jmp rel32` with forward references recorded on
two compile-time fixup stacks in scratch — one for the jumps to the end of an
`if` chain, one for `break` — and patched when the target is known; `continue`
jumps back to the loop condition directly. The two stacks are separate so that a
`break` inside an `if` is patched by the enclosing loop, not by the `if`.

The statement parser now knows the whole file: an optional `module` line,
`proc NAME`, then `entry`/`calls`/`permit` lines, the body, `end`. Unknown
statements, trailing tokens after a statement (anything but a `--` comment),
`elif`/`else`/`end` without an opener, `break`/`continue` outside a loop, a
block left open at EOF, a string used as a value, `.addr`/`.len` on an integer,
storing into a string, `<-` inside an expression, a missing comma or more than
seven syscall arguments all reject with exit 2 and `oli1: error`.

Known deviations from V0, to be closed by G4: names are one flat scope per
procedure (a binding inside a block stays visible after it; rebinding a name
shadows it instead of being an error); integers are untyped 64-bit; there is
no `and`/`or`/`not`, no `loop`, no shifts/bitwise operators yet.

Proven by `while_sum.oli` (55), `break_continue.oli` (7), `ops.oli` (26:
precedence, parentheses, `/`, `%`, unary minus, all six comparisons),
`nested.oli` (9: nested loops with `continue` in an inner `if`), `if_chain.oli`
(`elif` branch taken, byte-exact stdout), `echo.oli` (stdin echoed through an
`mmap` buffer, exit = byte count, empty input handled) and fifteen rejection
programs in the harness. Every step 0–3 fixture still passes unchanged.

## Step 5 (implemented)

A file is an optional `module` line followed by procedures. `proc NAME`
takes an optional parameter list `(p: T, ...)` — at most six, each parameter
name bound to slots 0..n-1 and spilled from `rdi rsi rdx rcx r8 r9` in the
prologue — and an optional `-> T`; type names are read and ignored (every
value is a 64-bit integer in oli-core). The symbol table and the slot counter
are reset per procedure. `NAME(args)` is a factor and a statement: arguments
are evaluated left to right and pushed, popped into the argument registers in
reverse order, then `call rel32`. The callee's `ret expr` leaves the value in
`rax` (`leave; ret`); falling off the end returns 0. In the procedure that
carries `entry`, `ret` remains `exit`. The ELF entry point is the entry
procedure's code address wherever it appears in the file.

Procedures live in a table (name, code position, parameter count). A call to
a procedure declared later creates a placeholder entry; every `call rel32` is
recorded as a fixup (field position, procedure index, argument count) and
resolved at finalize, which also checks that every called procedure was
defined, that argument counts match parameter counts, and that exactly one
procedure has `entry`. Duplicate procedure names, more than six parameters
or arguments, a parameter without a type, an empty `-> `, and any declaration
other than `proc` reject with exit 2.

Stack alignment before `call` is not maintained (the callee is always oli-core
code that does not depend on it); this becomes an obligation of G4, which
must follow `docs/ABI.md` exactly.

Proven by `fact.oli` (recursive factorial, 120), `fib.oli` (forward reference
to a doubly recursive procedure, 55), `six_args.oli` (all six argument
registers, 91), `put.oli` (a `write_all` procedure with a loop and a result,
byte-exact stdout), `noret.oli` (procedure without `ret`, call as statement,
42) and eight rejection programs in the harness. Every earlier fixture passes
unchanged.

## Diagnostics

Out-of-scope or malformed input must be rejected without emitting a partial ELF,
following `asm`'s convention. Since step 3 every rejection exits 2 and writes
`oli1: error` to stderr; positions and codes arrive with the lexer in later
steps.
