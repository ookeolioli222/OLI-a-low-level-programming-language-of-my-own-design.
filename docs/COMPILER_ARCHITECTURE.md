# Compiler Architecture

**Autonomy (design 0017).** The Oli-- toolchain is bootstrapped from
hand-written machine code (`genesis/`) and written in Oli--; no Rust, C or
C++ is part of it. Phases 1–1b were validated with a temporary foreign
front end that was **removed on 2026-09-21** (the user's decision; 0017
amendment). What remains of it is its output — the fixture corpus in `tests/`
and the snapshots — and the architecture below, which the Oli-- front end
(G4) must implement. Module and file names below are the *planned* Oli--
layout, not existing code.

## The genesis chain (`genesis/`)

| Layer | Artifact | Written in | Status |
|---|---|---|---|
| 0 | `0-hex0/hex0.hex` → `hex0.bin` (322 B) | hand-encoded bytes | done, self-reproducing |
| 1 | `1-hex2/hex2.hex0` → `hex2.bin` (882 B) | hex0 notation | done: labels `:name`, `%rel32`, `!rel8`, `&abs32`, `@abs64` |
| 2 | `2-asm/` | hex2 notation | next: assembler for Oli-- `machine x64` blocks |
| 3 | `3-oli1/` | Oli-- machine blocks | oli-core compiler |
| 4 | `compiler/` | oli-core Oli-- | `olic`, fixpoint |

`sh genesis/test.sh` rebuilds and verifies every layer on Linux x86-64.

## Modules of `olic` (G4, written in Oli--)

| Module | Stage | Contents | Status |
|-------|-------|----------|--------|
| `oli_diag` | all | `Span`, `SourceFile` (line table), `Diagnostic`/`Diagnostics`, the §8 renderer | **`compiler/diag.oli` (oli-core): header, location, source line and caret; diagnostics print in the order they are found, sorting is still to come** |
| `oli_lexer` | 1 | tokens (`Kw`, `Prim`, operators, `Doc`, `Newline`), the lexer | **`compiler/lex.oli` (oli-core), built by `oli1`; passes the corpus (layer 4)** |
| `oli_ast` | 1 | syntax tree types, S-expression printer (`--show-ast`, snapshots) | **`compiler/ast.oli` (oli-core); reproduces every snapshot byte for byte** |
| `oli_parser` | 1 | recursive-descent parser with recovery (`decl`, `stmt`, `expr`, `types`, `machine`) | **`compiler/parse.oli` (oli-core); E0010–E0032, W0001 exact on the negative fixtures** |
| `oli_sema` | 1b | module loader, `hir` (the semantic graph), name/type resolution, layouts, constant evaluation, flow, regions, capabilities, `--show-sema` printer | **done for V0 without a back end: `compiler/load.oli` (imports under `lib/`), `compiler/items.oli` (items, layout/choice layout, constants, signatures), `compiler/sema.oli` (local tables), `compiler/body.oli` (expression typing, regions, conversions, typed bodies, E0201–E0203) and `compiler/check.oli` (capabilities, scopes, flow, failures, regions) reproduce every `tests/snapshots/*.sema` byte for byte and the diagnostics of all fourteen `tests/sema/err` fixtures** |
| `olic` | driver | CLI: `--show-tokens`, `--show-ast`, `--show-sema`, `--check-syntax`, `--check`, `--freestanding`, `--lib` | `--show-tokens`, `--show-ast` and `--show-sema` exist as `compiler/show_tokens.oli`, `compiler/show_ast.oli` and `compiler/show_sema.oli` (stdin → stdout, one driver per stage until argv access exists); the flags themselves, `--check` and `--lib DIR` wait for command-line access |
| `lib/` | library | `core.oli` (`TrapKind`, `Site`), `core/mem.oli`, `std/os.oli` — written in Oli-- | V0 subset |
| `oli_oir` | 2 | OIR, verifier, passes | not started |
| `oli_x64` | 2 | machine lowering, register allocation, encoder | not started |
| `oli_elf` | 2 | ELF64 writer | not started |

Pipeline and observability flags: `OIR_SPEC.md` §1.

## Policies

- The compiler never traps on user input: every failure path is a diagnostic
  (`spec/OLI_SYNTAX_V0.md` §8) or a defined exit status; bounds and overflow
  checks stay enabled in the compiler itself (design 0006).
- No `machine` blocks outside `genesis/` and `arch.x64.*`.
- CI (`.github/workflows/ci.yml`): `sh genesis/test.sh` on Linux x86-64.

## Semantic analysis (Phase 1b)

`oli_sema::analyze` loads the root and every imported module (`loader.rs`),
then runs the passes of `sema/mod.rs` in order:

| Pass | File | Produces |
|------|------|----------|
| collect | `items.rs` | module symbol tables, item shells; `E0102`/`E0103`/`E0109` |
| layouts/choices | `items.rs` | fields with offsets, `size`/`align`, recursion (`E0204`) — on demand |
| signatures | `items.rs` | parameter/result types, permits, clauses |
| constants/statics | `consts.rs` | `ConstValue`s, backing `.rodata` statics for aggregate constants, cycles (`E0106`) |
| bodies | `body.rs`, `expr.rs`, `machine.rs` (Oli--: `body.oli`, `check.oli`) | the typed tree: places vs values, regions, flow state, capabilities |
| program | `program.rs` | `main`/`entry`/`traps` rules, symbol uniqueness |

Design record: `design/0014-semantic-graph-and-regions.md`. The graph and its
printer are `oli_sema::hir` and `oli_sema::printer`.

## Front end design (Phase 1)

### Lexer
- Byte-oriented over UTF-8; identifiers are ASCII; non-ASCII outside strings and
  comments is `E0001`. Positions are byte offsets; the line table converts to
  1-based line/column (columns count characters).
- Runs of newlines collapse into one `Newline`; leading newlines are dropped.
- `---` comments become `Doc` tokens; `--` comments vanish.
- Integer literals are `u128` after suffix scaling (`64K`); overflow is `E0003`.
- Maximal munch: `<-` is always the store operator; write `< -1` with a space.
- Errors never stop the lexer; the stream always ends with `Eof`.

### Parser
- Keyword-directed recursive descent; one token of lookahead, two in three
  documented places (`name :` / `name :=` at statement start, `T.wrap(`, and
  `in REG <-` vs the `in` instruction).
- Newlines are significant except inside unclosed `( [ {` and after an operator
  (`spec/OLI_SYNTAX_V0.md` §1). See `design/0013-parser-recovery.md`.
- Blocks are parsed by `parse_block(kind)`, which stops at `end` (or `else`/`elif`/
  `when` where legal) and leaves the closer to the caller; `expect_end` reports
  `E0014` with a secondary "opened here" span.
- The tree is purely syntactic: `Header.at(v)` is a call, `Name { … }` is a
  literal whose meaning (layout vs variant) is decided later, `u8(x)` is a
  `Convert` node because the callee is syntactically a type.
- `ExprKind::Error` / `PatternKind::Error` appear only after an error was reported.

### Diagnostics
Format fixed by `spec/OLI_SYNTAX_V0.md` §8. All diagnostics of one file are
sorted by position before printing. Codes used by the front end:

| Code | Meaning |
|------|---------|
| E0001 | invalid character (with hints for `=`, `;`, `!`, `#`/`@`) |
| E0002 | unterminated string literal |
| E0003 | integer literal too large |
| E0004 | bad digits or suffix on an integer literal |
| E0005 | bad character literal |
| E0006 | invalid escape sequence |
| E0007 | source file is not valid UTF-8 |
| E0010 | reserved word used as a name |
| E0011 | expected X, found Y (generic) |
| E0012 | expected expression |
| E0013 | expected a type / malformed type |
| E0014 | missing `end` (or a declaration inside a body) |
| E0015 | expected a declaration / stray token at module level |
| E0016 | ambiguous `else` inside a one-line `if … then` |
| E0017 | store target is not a place |
| E0018 | bad, duplicate or misplaced clause |
| E0019 | bad machine-block line or operand |
| E0020 | expected end of line (with a hint for `<-` in expressions) |
| E0021 | block statement after `then` |
| E0022 | mixed named and positional arguments |
| E0023 | import after a declaration |
| E0024 | `module` line not first |
| E0030 | nesting too deep |
| E0031 | chained comparison |
| E0032 | `{` at end of line used as a block opener |
| E0900 | feature not implemented (code generation, and the V0 exclusions) |
| W0001 | doc comment not attached to a declaration |

Semantic codes (`E01xx`–`E06xx`, `W0002`, `W0003`, `W0100`) are listed in
`spec/OLI_SEMANTICS_V0.md` §12.

## Tests

| Kind | Where | What |
|------|-------|------|
| unit | `olic` self-tests (planned) | positions, rendering, every token class, precedence, disambiguation rules, recovery, nesting limit, truncation and garbage inputs |
| fixtures | `tests/parse/ok/*.oli`, `examples/*.oli` | must parse with zero diagnostics |
| negative fixtures | `tests/parse/err/*.oli` | diagnostics must equal the file's `-- expect: CODE @ LINE:COL` lines, exactly |
| snapshots | `tests/snapshots/*.ast` | `--show-ast` of the reference programs; frozen (see CONTRIBUTING) |
| semantic unit | `olic` self-tests (planned) | every diagnostic code, layout sizes, escape analysis (accept and reject), flow, freestanding rules |
| semantic fixtures | `tests/sema/ok/*.oli`, `tests/sema/err/*.oli` | analyzed with the real `lib/`; `-- target: freestanding` on the first line selects the target |
| semantic snapshots | `tests/snapshots/*.sema` | `--show-sema` of `hello`, `packet_demo`, `freestanding` |
| CLI | `olic` self-tests (planned) | flags, exit codes, the exact §8 format, non-UTF-8 input, `--check`, `--show-sema`, `E0900` honesty |

Today only the genesis chain is executable: `sh genesis/test.sh`. The rows
above are the acceptance suite the Oli-- `olic` must pass.

## Performance baseline (Phases 1–1b, measured on the removed foreign front end)

Kept as the number the Oli-- front end must at least match.
Synthetic 19.4k-line / 553 KiB file (`packet_demo` ×200), release build, desktop x86-64,
process start (~68 ms on Windows) subtracted:

| Stage | Time | Rate |
|-------|------|------|
| lex + parse | ≈ 25 ms | ≈ 780k lines/s |
| lex + parse + semantic analysis (with `lib/`) | ≈ 67 ms | ≈ 290k lines/s |

No optimization has been attempted; the checker clones flow state at every branch.
