# Contributing to Oli--

## The autonomy rule

The Oli-- toolchain is written in Oli-- and bootstrapped from hand-written
machine code (`genesis/`, design record 0017). No Rust, C, C++ or any other
language may implement any part of it. Shell scripts may orchestrate and test
but never compile. The repository contains no code in any other language;
the fixture corpus under `tests/` is the acceptance test for the Oli-- front end.

## Build and test

```bash
sh genesis/test.sh          # on Linux x86-64; from Windows: wsl -e sh genesis/test.sh
```

Every genesis layer must (a) reproduce the previous layer's binary from its
listing and (b) pass its tests before the next layer starts. Listings are
text: two hex digits per byte, `;` comments, and (from hex2 on) labels.

## Process (every stage)

DESIGN → CRITIQUE → ORIGINALITY REVIEW → SPECIFICATION → SMALL IMPLEMENTATION →
TEST → BENCHMARK → REVIEW → DOCUMENT. Stop after each stage and report what
works, what does not, tests, design changes, performance, next milestone.

## Rules

- Language semantics change only together with `spec/` and a record in `docs/design/`.
- No fake implementations: an unsupported feature reports `E0900 feature not implemented`.
- The compiler never crashes on user input: every failure is a diagnostic or a defined exit status.
- Every diagnostic has a code; every new code gets a negative fixture in `tests/parse/err/`.
- Every syntax form appears in an `ok/` fixture and, if it is a reference program, in a snapshot.
- Docs are English; design records follow the fixed eight-heading format.

## Adding a negative fixture

Create `tests/parse/err/name.oli` (syntax) or `tests/sema/err/name.oli`
(semantics; start the file with `-- target: freestanding` to analyze it as a
freestanding program); end it with one `-- expect: CODE @ LINE:COL`
line per expected diagnostic (positions count characters, 1-based; the
directive lines themselves do not change positions when placed at the end).
The Oli-- front end (G4) must report exactly that set — no more, no fewer.

## AST and semantic snapshots

`tests/snapshots/*.ast` and `*.sema` are the expected `--show-ast` /
`--show-sema` output for the reference programs. They are frozen: change one
only together with a `spec/` change and a design record, and review the diff
before committing.
