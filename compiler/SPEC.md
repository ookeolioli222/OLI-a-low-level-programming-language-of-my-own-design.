# compiler/ — `olic`, the Oli-- compiler written in Oli-- (genesis layer 4)

`olic` is written in **oli-core**, the subset of Oli-- V0 that `genesis/3-oli1`
compiles (`genesis/3-oli1/SPEC.md`). Every module here is therefore both a
valid oli-core program (so `oli1` builds it today) and a valid V0 program (so
`olic` will build it tomorrow); that is what makes the fixpoint
`stage2 == stage3` possible without any foreign tool. Design record 0022.

## Build

`oli1` reads one source on stdin, so a program is the concatenation of its
modules; `module` lines are accepted anywhere and module-level constants must
precede the procedures that use them:

```sh
cat compiler/io.oli compiler/lex.oli compiler/show_tokens.oli | genesis/build/oli1.bin > show_tokens
./show_tokens < examples/hello.oli          # one token per line, diagnostics on stderr
```

`sh genesis/test.sh` (layer 4) builds every driver, checks the build is
deterministic and runs the acceptance tests below.

## Modules

| Module | File | Contents | Status |
|--------|------|----------|--------|
| `olic.io` | `io.oli` | `put`, `put_uint` (u64, printed as 2^63 + r above the signed range), `read_all` | done |
| `olic.lex` | `lex.oli` | the V0 lexer: token table, keywords, operators, literals, comments, doc comments, E0001–E0006 | done |
| `olic.show_tokens` | `show_tokens.oli` | driver for `--show-tokens`: stdin → token lines on stdout | done |
| `olic.diag` | — | §8 renderer with source excerpts, sorted diagnostics, file names | planned |
| `olic.ast`, `olic.parse` | — | syntax tree in a zone arena, recursive descent with recovery, `--show-ast` against `tests/snapshots/*.ast` | planned |
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

Acceptance (`genesis/test.sh`, layer 4): `examples/hello.oli` tokenizes byte
for byte as `tests/snapshots/hello.tokens`; every file under `tests/parse/ok`,
`tests/sema`, `examples`, `lib`, `compiler` and `genesis/3-oli1/tests` lexes
without diagnostics; each `tests/parse/err` fixture yields exactly its
expected `E0001`–`E0006` diagnostics at the expected line:column; literal
boundaries (`0xFFFF800000000000`, `2^64 - 1`, `2^64`, `64K`, `2M`, `1G`,
binary, octal, `'a'`, `'\x41'`) give the values the AST snapshots record.
