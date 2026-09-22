# compiler/ — `olic`, the Oli-- compiler written in Oli-- (genesis layer 4)

`olic` is written in **oli-core**, the subset of Oli-- V0 that `genesis/3-oli1`
compiles (`genesis/3-oli1/SPEC.md`). Every module here is therefore both a
valid oli-core program (so `oli1` builds it today) and a valid V0 program (so
`olic` will build it tomorrow); that is what makes the fixpoint
`stage2 == stage3` possible without any foreign tool. Design record 0022.

## Build

`oli1` reads one source on stdin, so a program is the concatenation of its
modules, in dependency order (a module-level constant must precede its use):

```sh
sh genesis/test.sh                       # builds every driver and runs the acceptance tests
B=genesis/build
cat compiler/io.oli compiler/lex.oli compiler/diag.oli compiler/show_tokens.oli | $B/oli1.bin > $B/show_tokens
cat compiler/io.oli compiler/lex.oli compiler/diag.oli compiler/ast.oli compiler/parse.oli \
    compiler/show_ast.oli | $B/oli1.bin > $B/show_ast
cat compiler/io.oli compiler/lex.oli compiler/diag.oli compiler/ast.oli compiler/parse.oli \
    compiler/load.oli compiler/items.oli compiler/show_items.oli | $B/oli1.bin > $B/show_items
chmod +x $B/show_tokens $B/show_ast $B/show_items

$B/show_tokens < examples/hello.oli      # one token per line
$B/show_ast    < examples/hello.oli      # the tree of tests/snapshots/hello.ast
$B/show_items  < examples/hello.oli      # layouts, choices and constants (run from the repo root:
                                         # imports are resolved under lib/)
```

Each driver writes its result on stdout and its diagnostics on stderr, and
exits 1 when it reported any. `show_items` resolves `import` by reading
`lib/<path>.oli`, so run it from the repository root.

`sh genesis/test.sh` (layer 4) builds every driver, checks the build is
deterministic and runs the acceptance tests below.

## Modules

| Module | File | Contents | Status |
|--------|------|----------|--------|
| `olic.io` | `io.oli` | `put`, `put_uint` (u64, printed as 2^63 + r above the signed range), `read_all` | done |
| `olic.lex` | `lex.oli` | the V0 lexer: token table, keywords, operators, literals, comments, doc comments, E0001–E0006 | done |
| `olic.ast` | `ast.oli` | the node arena, the shared `Ctx`, the S-expression printer of `tests/snapshots/*.ast` | done |
| `olic.parse` | `parse.oli` | recursive descent over §3–§7 with recovery; E0010–E0032, W0001 | done |
| `olic.show_tokens` | `show_tokens.oli` | driver for `--show-tokens`: stdin → token lines on stdout | done |
| `olic.show_ast` | `show_ast.oli` | driver for `--show-ast`: stdin → the tree on stdout | done |
| `olic.diag` | `diag.oli` | the §8 renderer: header, `--> file:line:col`, the source line and a caret | done |
| `olic.load` | `load.oli` | reading files (`openat`/`read`/`close`) and resolving imports under `lib/` | done |
| `olic.items` | `items.oli` | modules, item collection, layout/choice layout (ABI §3), constant evaluation, the item section of `--show-sema` | done |
| `olic.sema` | `sema.oli` | the local table of every procedure: parameters, places, bindings, zone handles and `case` patterns, with inferred types | done |
| `olic.check` | `check.oli` | capabilities (E0401), unimplemented features (E0900), constant cycles (E0106), constant range (E0212), recursive layouts (E0204) | done |
| `olic.show_items` | `show_items.oli` | driver for the item, signature and local sections, and for the checks | done |
| `olic.bodies` | — | typed statements and expressions, regions, capabilities, flow — the rest of `--show-sema` | planned |
| `olic.sema` | — | items, layouts, signatures, constants, bodies, program rules, `--show-sema` | planned |
| `olic.oir`, `olic.x64`, `olic.elf` | — | back end | planned |

## Conventions imposed by oli-core

- Flat scope per procedure: a name is bound once per procedure with one type
  (rebinding allocates a new slot and shadows; avoid it). Early `ret` replaces
  `and`/`or`/`not`, which oli-core lacks.
- At most six argument words: a `view` costs two, a `ref`/`zone`/integer one.
- Records live in zones: `z.make(T)` for one, `z.bytes(n * T.size)` plus
  `T.at(buf[off..off + T.size])` for tables (`tok_at` in `lex.oli`).
- Integers are 64-bit words compared signed; u64 values with the top bit set
  are handled explicitly where it matters (`put_uint`, literal overflow).
- Module-level constants (`TK_IDENT := 2`) name every code; there are no
  `choice` types yet.
- Diagnostics are printed when found (stderr) and counted; the §8 excerpt
  lines and sorting arrive with `olic.diag`.

## Lexer (`lex.oli`)

Tokens (`Tok`: kind, sub, start, len, line, col, val) follow
`spec/OLI_SYNTAX_V0.md` §2 and the lexer notes in
`docs/COMPILER_ARCHITECTURE.md`:

- kinds `eof nl ident int char str kw op doc`; `sub` is the 1-based keyword
  index (spec §2.3 order, reserved words last), the two-character operator
  index (`:= <- <~ -> .. == != <= >= << >>`) or `100 + byte` for one-character
  operators; `val` is the integer or character value.
- Runs of newlines collapse into one `nl`, leading newlines are dropped, the
  stream ends with `nl` then `eof` even when the file has no final newline.
- `--` comments vanish; `---` yields `doc` holding the text after `--- `
  (one leading space and a trailing CR trimmed), positioned at the `---`.
- Integers: decimal, `0x`, `0b`, `0o`, `_` separators, `K`/`M`/`G` suffixes,
  u64 range (V0 says u128 — deviation to close in the self-hosted stage);
  E0003 at the literal for overflow, E0004 at the first bad character (or at
  the literal when a prefix has no digits), the bad characters are consumed.
- Strings and chars: every §2.4 escape; E0006 at the backslash of a bad
  escape; E0002 at the opening quote of a string reaching end of line; E0005
  at a bad character literal.
- E0001 at any other byte; one report per UTF-8 character. Columns count
  characters (continuation bytes do not advance the column).

## Parser (`ast.oli`, `parse.oli`)

The tree is one arena of uniform nodes (kind, sub, val, token range, child
list), so the printer is a single dispatch and no node needs its own layout.
`Ctx` carries the source, the token table, the arena, the output buffer, the
cursor and the recovery state; parser and printer both take `ref Ctx`.

- Grammar: `spec/OLI_SYNTAX_V0.md` §3–§6 — declarations (`module`, `import`,
  `layout`, `choice`, constants, statics, `proc` with parameters, result type
  and clauses), statements (bind, typed bind, place, store, move, `if`/`elif`/
  `else`, one-line `if ... then`, `while`, `each`, `loop`, `case`/`when`,
  `zone ... at/from`, `machine`, `ret`, `fail`, `break`, `continue`) and the
  full expression chain including `else` fallbacks, ranges, conversions,
  literals, arrays, raw loads and `machine` operands.
- §7 disambiguation: `{` after a path is a literal but never after a condition
  (E0032); `[` at the start of an expression is a raw load; `T(x)`, `T.wrap(x)`
  and `addr T (x)` are conversions while a bare `T` in an argument is
  `(type T)`; after `.` any keyword is an ordinary member name.
- §1 line rules: newlines are ignored inside unclosed brackets and after a
  binary operator, `:=`, `<-` or `.`; a missing operand after a continuation is
  reported at the newline that was skipped.
- Recovery: a block closer (`end`, `else`, `elif`, `when`) is never consumed by
  an error path, so one bad line costs one diagnostic; `{` used as a block
  opener is read as the literal it looks like; a `zone` without a name does not
  open a block.

Acceptance (`genesis/test.sh`, layer 4): the four `tests/snapshots/*.ast` are
reproduced byte for byte; every fixture, library module and the compiler's own
source parses without a diagnostic; each `tests/parse/err` fixture reports
exactly its expected `E0001`–`E0032`/`W0001` set at the expected positions and
still prints a module after recovery.

## Items (`load.oli`, `items.oli`)

Stage 1 of semantic analysis: the root module is read from stdin, `core` is
loaded because it is always available, then every `import` of the root, each
into its own `Ctx`. Items are collected per module in declaration order and
printed in the order the snapshots use — all layouts, then all choices, then
constants and statics, numbered from zero within each group, the root module's
items before the imported ones.

- Layout (`docs/ABI.md` §3): a field's alignment is `min(size, 8)`, raised by
  `align N` on the field and dropped to 1 by `packed` on the layout; the
  layout's alignment is the largest field alignment or its own `align N`, and
  its size is rounded up to it. A choice is a one-byte tag followed by the
  payload at the first offset with the payload's alignment; every variant is
  laid out from there and the size is the largest variant, rounded up.
- Type sizes come from the token range of the type: primitives from a table,
  `view T` = 16, `ref`/`addr`/`zone`/`physaddr` = 8, `[N]T` = `N × T`, and a
  layout or choice name resolves that item first (a cycle reports E0204).
- Constants are evaluated as integers: literals, `+ - * / %`, unary minus,
  `Name.size`, `Name.align` and other constants.

## Signatures and locals (`items.oli`, `sema.oli`)

After the items, every procedure prints its signature — qualified name, result
type, `permits=[…]`, `calls=`, `entry`, `section=`, `traps` — and its local
table in source order: parameters, then places, bindings, zone handles and
`case` pattern names as the body introduces them.

A local's type is materialised as text in one shared buffer, so the printer is
a copy and a type is a view. Inference covers what the corpus needs: an
annotation, a string literal (`view u8`), `z.bytes(n)` (`rw view u8`),
`z.make(T)` (`rw ref T`), `T.at(v)` (`ref T`), `os.syscall(…)` (`word`), a call
(the callee's declared result), a subview (the base's type), an index (the
element type), `.addr`/`.len`, a conversion (`u64.bits(x)` → `u64`), an `each`
binding (the element type of the iterable), `ok x` in a `case` (the `T` of
`T or E`) and `fail V {f}` (the field of that variant). A type a path names is
reduced to the item: `core.TrapKind` prints as `TrapKind`. An initialiser this
stage cannot type yet — an arithmetic expression, a `checked(…)` fallback, an
integer range in `each`, a compiler intrinsic such as `cpu.id` — prints `?`.

## Checks (`check.oli`)

The checks that need no type graph run after the items are laid out, over every
layout field, procedure signature and body:

- **E0401, capabilities.** A raw load or store (`[a]`), a raw address
  conversion (`addr T (x)`) and `zone … at` require `permit memory.raw`; a
  `machine` block requires `permit cpu.asm`. The walk carries the procedure
  whose clauses are in force, so a capability granted to one procedure never
  leaks into another.
- **E0900, not implemented.** `own` in a type, `f32`/`f64`, the move operator
  `<~`, `port T (x)`, `mem.mmio(…)` and `calls interrupt` are parsed, then
  refused — the rule of `CONTRIBUTING.md`: no feature is approximated.
- **E0106 / E0212, constants.** Constants are evaluated on demand, so a cycle
  is caught at the constant that closes it; a value that does not fit an
  annotated unsigned type is reported at the start of its expression.
- **E0204, recursive layouts.** Resolving a layout that is already being
  resolved reports at its declaration.
- **E0101, scopes.** A binding, parameter, place, zone handle or pattern name
  that shadows something already in scope — including a module-level constant,
  static, layout or choice — is reported at the declaration. Names starting
  with `_` are exempt.
- **E0220, definite assignment.** A place must be stored before it is read.
  An assignment inside a branch or a loop does not survive it, except when
  every arm of an `if … else` makes it; an array place is storage and counts as
  initialised; `out REG -> p` in a `machine` block assigns `p`.
- **E0230 / E0231, reachability.** Control may not reach the end of a
  procedure with a result, and a statement after one that cannot fall through
  is unreachable. `ret`, `fail`, `break`, `continue`, a `loop` without a
  `break` and an `if … else` whose every branch diverges do not fall through;
  a `while` may always run zero times.
- **E0110 / E0111, writability.** A store into a binding, a parameter or a
  zone handle, and a store through a view or reference that is not `rw` or
  into a module constant, are refused.
- **E0300, regions.** Zone memory may not be returned, nor stored into a place
  that was declared outside that zone (a module static counts as outside); a
  view of a frame array and a reference to a frame local may not be returned.
- **E0310 / E0311, failures and `case`.** A fallible call used as a statement
  is an unhandled failure; `else fail` needs a procedure that can fail; a
  `case` must cover every variant, both channels of `T or E`, `true` and
  `false`, or end in `else` — for integers only `else` is exhaustive.

Acceptance (`genesis/test.sh`, layer 4): twelve of the fourteen
`tests/sema/err` fixtures report exactly their expected codes and positions —
`items`, `permits`, `not_implemented`, `flow`, `shadow`, `unassigned`,
`unhandled`, `not_exhaustive`, `readonly`, `freestanding_zone`, `escape_frame`
and `escape_zone`; every `tests/sema/ok` fixture, example and library module
stays clean; and `tests/parse/ok/kernel_sketch.oli` — a file whose own header
says it is rejected later — reports the missing `memory.raw` permit and the
three V1 constructs it uses. The two fixtures that remain, `literals.oli` and
`mixed_addr.oli`, need expression typing (E0201, E0202, E0203).

### The compiler analyses itself

Running the checks over `compiler/` measured exactly how far oli-core was from
V0: 290 stores into bindings, 93 stores through fields that could not be
declared `rw`, and 8 infinite loops written as `while 1`. oli1 step 6e added
the three constructs that were missing — typed places, `rw` in field types and
`loop` — and the sources were converted to use them.

`olic` now analyses its own source, all ten modules as one program, **without
a single diagnostic**: no shadowed name, no store into a binding or a
read-only view, no read before assignment, no unreachable statement, no
procedure that falls off its end, no unhandled failure, no missing permit.
The harness runs that self-analysis on every build, so the compiler's source
cannot drift out of V0.

Acceptance (`genesis/test.sh`, layer 4): every `(proc …)` and `(local …)` line
of all three `tests/snapshots/*.sema` is reproduced exactly — 46 lines across
the three programs, covering every local kind and every inference rule above.

Acceptance (`genesis/test.sh`, layer 4): the item section of
`tests/snapshots/hello.sema`, `packet_demo.sema` and `freestanding.sema` is
reproduced line for line (the head of each snapshot, down to the first
procedure); `kernel_sketch` exercises `packed`, `align N` on a layout and on a
field; `statements` exercises a choice whose variant carries two layouts by
value; every fixture, library module and compiler source produces a program.

Deviations to close in later stages: an aggregate constant (a layout literal or
an array) prints as `0` because only integer constants are evaluated; a
constant without a type annotation prints `word`; `(program entry=none)` is
printed for a module without an entry procedure instead of checking the
one-entry rule (that belongs to the program pass).

Deviations to close in later stages: named call arguments parse and are
checked for mixing (E0022) but the tree keeps only the value; `import ... as`
is parsed but not printed; E0013/E0015/E0021/E0024/E0030 are not yet produced
by any fixture; diagnostics print in the order they are found (the §8 excerpt
and sorting arrive with `olic.diag`).

Acceptance of the lexer: `examples/hello.oli` tokenizes byte
for byte as `tests/snapshots/hello.tokens`; every file under `tests/parse/ok`,
`tests/sema`, `examples`, `lib`, `compiler` and `genesis/3-oli1/tests` lexes
without diagnostics; each `tests/parse/err` fixture yields exactly its
expected `E0001`–`E0006` diagnostics at the expected line:column; literal
boundaries (`0xFFFF800000000000`, `2^64 - 1`, `2^64`, `64K`, `2M`, `1G`,
binary, octal, `'a'`, `'\x41'`) give the values the AST snapshots record.
