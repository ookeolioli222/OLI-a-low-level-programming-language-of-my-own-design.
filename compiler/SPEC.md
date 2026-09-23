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
    compiler/load.oli compiler/items.oli compiler/sema.oli compiler/body.oli \
    compiler/check.oli compiler/show_sema.oli | $B/oli1.bin > $B/show_sema
chmod +x $B/show_tokens $B/show_ast $B/show_sema

$B/show_tokens < examples/hello.oli      # one token per line
$B/show_ast    < examples/hello.oli      # the tree of tests/snapshots/hello.ast
$B/show_sema   < examples/hello.oli      # the semantic graph of tests/snapshots/hello.sema
                                         # (run from the repo root: imports are resolved under lib/)
```

Each driver writes its result on stdout and its diagnostics on stderr, and
exits 1 when it reported any. `show_sema` resolves `import` by reading
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
| `olic.body` | `body.oli` | expression typing (contexts, conversions, regions), the typed-body printer and E0201/E0202/E0203 | done |
| `olic.show_sema` | `show_sema.oli` | driver for `--show-sema`: the whole semantic graph, and every check | done |
| `olic.oir` | `oir.oli` | the OIR instruction stream, statics, trap sites and the printer of both block forms; E0900 for anything outside the lowered subset; zones, views, `each`, layouts and refs (the memory instructions of `OIR_SPEC` §4) | done |
| `olic.cfg` | `cfg.oli` | basic blocks with one terminator each, predecessors, reverse postorder, immediate dominators, and the verifier of `OIR_SPEC` §8 | done |
| `olic.ssa` | `ssa.oli` | `mem2reg`: places become values, phis placed by the iterated dominance frontier, the procedure rebuilt without its unreachable blocks | done |
| `olic.opt` | `opt.oli` | the passes of `OIR_SPEC` §6 to a fixpoint: constant folding (arithmetic, comparisons, conversions, a branch on a constant, and a constant overflow folded to `trap`), check elision with a proof recorded for every removal (`constant`, `divisor`, `loop-bound`), copy propagation over phis, dead code, and the statics nothing names dropped from the image | done |
| `olic.x64` | `x64.oli` | machine lowering stage 2: the blocks, every value in a frame slot, a phi as parallel copies on its edges, SysV calls, the overflow/division checks and `core.trap`, `mmap`/`munmap` zones with a bump cursor, bounds-checked view access and view results in rax:rdx | done |
| `olic.elf` | `elf.oli` | the ELF64 writer: one loadable segment, no section headers and no symbol table | done |
| `olic.show_oir` | `show_oir.oli` | driver for `--show-oir`: stdin → the blocks before any pass | done |
| `olic.show_ssa` | `show_ssa.oli` | driver for `--show-ssa`: stdin → the blocks after `mem2reg` | done |
| `olic.show_opt` | `show_opt.oli` | driver for `--show-oir=opt`: stdin → the blocks after the passes, removed checks and their proofs included | done |
| `olic.verify_check` | `verify_check.oli` | the verifier's negative test: ten corruptions of a real program, each rejected with the invariant it breaks | done |
| `olic.main` | `olic.oli` | driver: source on stdin, a native ELF64 on stdout | done |
| — | — | raw word access, `choice`, `machine` lowering, statics, `[N]T` places, common-subexpression elimination and register allocation | planned |

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
reduced to the item: `core.TrapKind` prints as `TrapKind`. Since stage 3 the
initialiser is typed by `olic.body`, so arithmetic, conversions and constants
are covered too; a local whose initialiser this stage still cannot type — a
compiler intrinsic such as `cpu.id`, or a call into a module that was not
loaded — prints `?`.

A name is resolved to the declaration that *precedes its use*: every local
records the statement it was declared in, and a lookup takes the latest
declaration at or before the statement being analysed. That is what makes a
name reused in two sibling blocks resolve to the right one, which a flat table
otherwise gets wrong.

## Typed bodies (`body.oli`)

Stage 3 gives every expression a type, a region and a printed form, which is
the rest of `--show-sema`. There is no typed tree: the printer derives the
type as it prints, and a separate walk (`tstmt`/`texpr`) derives the same
types to report diagnostics, so `--check` never has to print.

**Context types.** A type flows *down* as `want`: the declared type of a
place, the annotation of a binding, the type of a store target, the result
type of the procedure for `ret`, its failure type for `fail`, the parameter
type of a call, the field type of a literal, `uword` for an index, a slice
bound and a zone size, `bool` for a condition, and the width of a register for
`in REG <- e`. An integer literal takes that type; a literal that never meets
one is `E0201`, and one that does not fit is `E0202`.

**Conversions.** Where a value's own type differs from the context's, the
printer inserts what the conversion actually is: `(widen x)` for a lossless
widening, `(bits x)` for an explicit same-width reinterpretation, `(inttoaddr
x)` / `(inttophys x)` for an address, nothing at all when only `rw` is
dropped or the representation is identical. `wrap(e)` / `sat(e)` /
`checked(e)` set the mode of the arithmetic inside `e`, which prints as
`(add/wrap …)`. Known gap: `checked(e)` and `T.checked(x)` are typed as plain
`T`, not as `T or Overflow` — `spec/OLI_SEMANTICS_V0.md` defers naming the
`Overflow` type, so there is no failure type for the checker to build, and
`tests/sema/ok/flow.oli` resolves one with `else` today. It is recorded here
rather than refused with `E0900`, because refusing it would reject a fixture
the corpus accepts; it closes when the spec names the type. `os.syscall` is special-cased the way the ABI is: an unsigned
argument is reinterpreted (`(bits …):word`), a signed one is widened.

**Regions** (design 0014) are a bit set: bit `i` is `param#i`, bit 16 is the
frame, and bit `17 + k` is the `k`-th zone the procedure opens. A local
records the region of the value it holds; an expression inherits it
structurally, a call result is the union of its argument regions (rule 3 of
`spec/OLI_MEMORY_V0.md`), and a value whose type cannot point anywhere carries
none — which is exactly when the printer leaves `@…` out.

**Places and values are different forms.** A read of memory prints as
`(load [rw local total])`, `(load [field [deref (local hdr):ref Header] magic])`
or `(load [rw raw (local a):addr u8])`; an array place read as a value prints
as `(view-of [rw static heap_region])`. The `rw` of a place follows the base:
a frame place and a static are writable, a field of a `ref` is not and a field
of a `rw ref` is.

Implicit narrowing of a *computed* value (`p <- e` where `e` is wider than
`p`) is a V0 `E0202` and is reported like any other rule. It was the last
deviation: the check was written but gated behind `NARROW_STRICT`, because
enabling it reported 93 sites in `compiler/` itself (and **none** in `lib/`,
`examples/` or the fixtures) and oli-core had no conversions to write instead.
That measurement was the specification of oli1 step 6f (`T(x)`, `T.wrap(x)`,
`T.bits(x)`), exactly as the step 6e measurement was. Step 6f landed, all 93
sites now name the conversion they perform, the gate is gone, and
`tests/sema/err/narrow.oli` holds the rule — with the accepted explicit forms
beside it — to its exact positions. Re-measuring found no further gap: every
construct `compiler/` uses is both oli-core and valid V0.

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

- **E0201 / E0202 / E0203, types** (`body.oli`, below).

Acceptance (`genesis/test.sh`, layer 4; layer 6 is the fixpoint `stage2 ==
stage3`, reached 2026-09-23): **all fourteen** `tests/sema/err`
fixtures report exactly their expected codes and positions; every
`tests/sema/ok` fixture, example and library module stays clean; and
`tests/parse/ok/kernel_sketch.oli` — a file whose own header says it is
rejected later — reports the missing `memory.raw` permit and the three V1
constructs it uses.

### The compiler analyses itself

Running the checks over `compiler/` measured exactly how far oli-core was from
V0: 290 stores into bindings, 93 stores through fields that could not be
declared `rw`, and 8 infinite loops written as `while 1`. oli1 step 6e added
the three constructs that were missing — typed places, `rw` in field types and
`loop` — and the sources were converted to use them.

`olic` now analyses its own source, all eleven modules as one program,
**without a single diagnostic**: no shadowed name, no store into a binding or a
read-only view, no read before assignment, no unreachable statement, no
procedure that falls off its end, no unhandled failure, no missing permit.
The harness runs that self-analysis on every build, so the compiler's source
cannot drift out of V0.

Acceptance (`genesis/test.sh`, layer 4): all three `tests/snapshots/*.sema`
are reproduced **byte for byte** — items, signatures, locals and typed bodies;
`kernel_sketch` exercises `packed`, `align N` on a layout and on a field;
`statements` exercises a choice whose variant carries two layouts by value;
every fixture, library module and compiler source produces a program.

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
