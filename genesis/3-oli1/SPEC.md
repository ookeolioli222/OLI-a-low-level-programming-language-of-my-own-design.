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
| 6a | `zone z SIZE [at ADDR] … end`, `z.bytes(n)`, views as two-word values (literals, params, results, locals), `v[i]`, `v[i] <- x`, `v[a..b]`, `v[..b]`, `v[a..]`, `.addr`/`.len`, raw `[a]`/`[a] <- x`, traps | mmap/munmap, bump allocation, `cmp`/`jcc` to a shared trap stub | **done** |
| 6b | `layout Name [packed] [align N] … end` with `f : T [align N]`, `Name.size`/`.align`/`.at(v)`, `z.make(Name)`, `ref Name` locals/params/results, `r.f` loads (zero/sign-extended) and `r.f <- x` stores, view fields | field offsets, sized moves, `cmp`/`test` + trap | **done** |
| 6d | `NAME := <decimal>` at module level: integer constants, visible in every procedure after the line; a local of the same name shadows one | `mov rax, imm64` | **done** |
| 6c | `-> T or E` results, `fail [e]`, `e else fail` / `e else ret [v]` / `e else v`, `case e … when ok [x] … when fail [e] … end` (second arm may be `else`) | tag in `rax`, payload in `rdx`; `test`/`jcc` per resolution | **done** |

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
no `and`/`or`/`not`, no `loop`, no shifts/bitwise operators yet; `rw` is not
distinguished from read-only views (only static strings are read-only);
`.addr`/`.len`/`[i]`/`.f` apply to names, not to arbitrary expressions
(`r.f.len` needs an intermediate binding).

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

## Step 6a (implemented)

Two compile-time types: integer word and view. A view is two words — `rax` =
address, `rdx` = length — exactly the ABI classification of `view T`
(`docs/ABI.md` §2), so a view local takes two frame slots, a view parameter two
argument registers, and a `-> view u8` procedure returns in `rax:rdx`. The type
of the last expression is tracked in compiler state and checked wherever an
integer is required (arithmetic, comparisons, conditions, indices, syscall
arguments, exit codes) and where a store, return or argument must match.

Memory constructs, all following `spec/OLI_MEMORY_V0.md` §3–4 in the hosted
subset: `zone z SIZE` rounds SIZE up to 4096, `mmap`s it and stores the
`(base, cursor, limit)` triple in the frame; the handle `z` is the address of
the triple (ABI §2), so it can be passed to a `z: zone` parameter. `z.bytes(n)`
bump-allocates 16-byte-aligned zero-filled memory and traps when the cursor
would pass the limit; `end` releases the zone with `munmap` (`at ADDR` zones
release nothing). `ret` inside a zone of a non-entry procedure and `break`/
`continue` that would leave a zone are rejected — the release on every exit
edge is deferred to G4. `v[i]` and `v[i] <- x` are bounds-checked byte
accesses; `v[a..b]`, `v[..b]`, `v[a..]` are bounds-checked subviews; `[a]` and
`[a] <- x` are raw word accesses (the `memory.raw` permit is not yet enforced).
String literals are views wherever an expression is allowed. Every trap goes
through one 36-byte stub placed at the start of the code that writes
`oli: trap` and exits 3.

Pass 1 now reads every `proc` header into the procedure table before any code
is generated, so calls are checked for parameter count, view/integer type of
every argument and word count where they occur, and a call's result type is
known even for a procedure declared later.

Proven by `zone_bytes.oli` (`Hi`, 116), `view_param.oli` (byte counting over
string-literal arguments, 23), `subview.oli` (`OliHelloello`, 12), `raw.oli`
(word store/load through a zone buffer, 68), `slurp.oli` (a zone handle passed
to a procedure that returns a view of stdin, upper-cased in place by another
procedure, `HELLO, ZONE`), `zone_scope.oli` (a zone created and released on
every loop iteration, 6), six trapping programs and fourteen rejection
programs in the harness. Every earlier fixture passes unchanged.

## Step 6b (implemented)

Pass 0 reads every `layout` block into a layout table before the procedure
headers are read, so `ref Name` parameter and result types resolve whatever
the declaration order. Fields are laid out in declaration order with natural
alignment and padding (`docs/ABI.md` §3): the alignment of a field is
`min(size, 8)`, raised by `align N` on the field, dropped to 1 by `packed` on
the layout; the layout's alignment is the maximum (or the `align N` clause)
and its size is rounded up to it. Field types come from a fixed table:
`u8 s8 byte bool` (1), `u16 s16` (2), `u32 s32` (4), `u64 s64 word uword addr
physaddr ref` (8) and `view` (16, two words); trailing type words are ignored.
Nested layouts by value, `be`/`le` and arrays are not in oli-core.

A third expression type, `ref Name` (code 3 + layout index), is one word.
`Name.at(v)` takes a view, traps when `v.len < Name.size` or, unless packed,
when `v.addr` is not a multiple of `Name.align`, and yields the ref;
`z.make(Name)` allocates `Name.size` zero-filled bytes from a zone (16-byte
aligned, so any layout alignment up to 16 is honoured). `r.f` loads by the
field's size and signedness — `movzx`/`movsx`/`movsxd`/`mov` — or two words
for a view field; `r.f <- x` stores by size and requires the value to match
the field type. Parameter type codes are packed eight bits each into the
procedure entry and every argument is checked against its parameter where
the call is parsed, including refs to distinct layouts. `Name.size` and
`Name.align` are compile-time constants.

Proven by `layout_basic.oli` (four fields, stores through a ref obtained by
`Header.at` visible as bytes of the buffer, 154), `layout_signed.oli` (s8/u8
… s64 read back sign- or zero-extended, 39), `layout_view.oli` (a `view u8`
field, a ref passed to a procedure, `hello`, 9), `layout_param.oli`
(`-> rw ref Rect` and `ref Rect` parameters, `z.make`, 44),
`layout_packed.oli` (`packed`, `align 32` on a layout, `align 4` on a field,
`P.at` of an odd address, 80), two `Name.at` traps and fifteen rejection
programs in the harness. Every earlier fixture passes unchanged.

## Step 6c (implemented)

Fallible results follow design 0007 and `docs/ABI.md` §2: a `-> T or E`
procedure returns the tag in `rax` (0 = ok, 1 = fail) and the payload in
`rdx`. In oli-core `T` is an integer or `ref Name` (a view payload would need
`sret`; rejected) and `E` is an integer type or `none`; the compile-time type
code of a fallible result is `T + 256`, plus 512 when `E` is `none`. Only
results and call values carry these bits: a parameter of type `T or E` and a
fallible `entry` procedure are rejected.

Inside a fallible procedure `ret v` (`v : T`) emits `mov rdx,rax; xor eax,eax;
leave; ret` and `fail e` (`e : E`) emits `mov rdx,rax; mov eax,1; leave; ret`;
bare `fail` is allowed only when `E` is `none` (it clears `rdx`) and `fail e`
is rejected there. A call whose result is fallible must be resolved where it
occurs — `expr` is `cmp_expr [else handler]` — and every other use (binding,
statement, operand, argument, condition) rejects an unresolved value:

- `e else fail` — `test rax,rax; jz ok; mov eax,1; leave; ret; ok: mov rax,rdx`:
  the callee's payload propagates unchanged; the current procedure must be
  fallible with the same `E` kind (`none` or integer), and, as for `ret`, not
  inside a zone.
- `e else ret [v]` — the ok test, then the `ret` statement's code (exit in the
  entry procedure, ok-tagged return in a fallible one), then `mov rax,rdx`.
- `e else v` — `v` must have type `T`; the fail branch evaluates `v` and jumps
  past the `mov rax,rdx` of the ok branch.

`case e` requires a fallible subject and exactly two arms in either order:
`when ok [x]` and `when fail [e]`, where the second may be `else`. The subject
is followed by `test rax,rax` and a `jz`/`jnz` to the second arm; each arm's
optional name becomes a new frame slot (`x : T`, `e : E`) stored from `rdx`
before the arm's block; the first arm ends with `jmp` past the second. A
binding in a `when fail` arm is rejected when `E` is `none`; the same arm
twice, a missing arm, or `when` outside `case` reject.

Proven by `fallible.oli` (defaults, both arm orders, 157), `propagate.oli`
(two-level `else fail`, `else v` in a non-fallible caller, `case` writing
`ok`/`short`/`bad`, 51), `optional.oli` (`or none`, bare `fail`, `when fail`
without a binding, `else` arm, `else ret 0`, a `ref Pair or none` payload,
140), `else ret 7` in the entry procedure, `none` propagation with a bare
`else ret`, fallible `if` conditions, a call as the default value, `case`
inside `while` with `break` in an arm, and twenty-three rejection programs in
the harness. Every earlier fixture passes unchanged.

Known deviations from V0, to be closed by G4: `T or E` values cannot be bound
or passed; `E` is untyped (any integer type name is one word); `fail` in an
`else` handler propagates only from a call (there is no other fallible
expression); `case` on integers, `choice` types and payload field patterns are
not in oli-core.

## Step 6d (implemented)

`NAME := <decimal>` at module level defines an integer constant (V0 §7 rule
3), kept in a table of its own that name lookup consults after the current
procedure's locals, so a parameter or local of the same name shadows it. A
constant is a factor lowered to `mov rax, imm64`; storing into it, `.addr`/
`.len`/`[i]` on it, a non-literal value and a duplicate name reject. Proven by
`consts.oli` (42) and four rejection programs in the harness.

## Diagnostics

Out-of-scope or malformed input must be rejected without emitting a partial ELF,
following `asm`'s convention. Since step 3 every rejection exits 2 and writes
`oli1: error at line N` to stderr, N being the input line at which the
rejection was detected (added with G4, when `oli1` started compiling
multi-module programs); codes arrive with `olic`.

Two limits were raised for G4: the scratch mapping is 4 MiB and the code
temporary used at finalize has 2 MiB, so a compiled program may hold up to
384 KiB of code and string data (the output region). `name..` after a name in
an index (`v[i..]`) is a range, not a member access, since G4.
