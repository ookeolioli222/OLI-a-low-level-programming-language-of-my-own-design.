# Contributing to Oli--

## The autonomy rule

The Oli-- toolchain is written in Oli-- and bootstrapped from hand-written
machine code (`genesis/`, design record 0017). No Rust, C, C++ or any other
language may implement any part of it. Shell scripts may orchestrate and test
but never compile. The Rust code under `reference/` is an oracle for the test
fixtures and is deleted once the Oli-- compiler passes them.

## Build and test

```bash
sh genesis/test.sh          # on Linux x86-64; from Windows: wsl -e sh genesis/test.sh
```

Every genesis layer must (a) reproduce the previous layer's binary from its
listing and (b) pass its tests before the next layer starts. Listings are
text: two hex digits per byte, `;` comments, and (from hex2 on) labels.

### The reference (optional)

```bash
cd reference
cargo test --workspace
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
```

Rust stable is enough; there are no external dependencies. On Windows the
`x86_64-pc-windows-gnu` toolchain works without the Windows SDK
(`rustup set default-host x86_64-pc-windows-gnu`).

Windows note: Smart App Control may block a freshly built unsigned `olic.exe`
("application control policy blocked this file"). The verdict is per file
hash; rebuilding after any source change produces a new hash. The CLI tests
print `SKIPPED` instead of failing when this happens; the in-process tests are
unaffected. Changing that policy is a system-level decision — not required.

## Process (every stage)

DESIGN → CRITIQUE → ORIGINALITY REVIEW → SPECIFICATION → SMALL IMPLEMENTATION →
TEST → BENCHMARK → REVIEW → DOCUMENT. Stop after each stage and report what
works, what does not, tests, design changes, performance, next milestone.

## Rules

- Language semantics change only together with `spec/` and a record in `docs/design/`.
- No fake implementations: an unsupported feature reports `E0900 feature not implemented`.
- Compiler crates never panic on user input (`unwrap`/`expect`/`panic`/indexing are denied by clippy).
- Every diagnostic has a code; every new code gets a negative fixture in `tests/parse/err/`.
- Every syntax form appears in an `ok/` fixture and, if it is a reference program, in a snapshot.
- Docs are English; design records follow the fixed eight-heading format.

## Adding a negative fixture

Create `tests/parse/err/name.oli` (syntax) or `tests/sema/err/name.oli`
(semantics; start the file with `-- target: freestanding` to analyze it as a
freestanding program); end it with one `-- expect: CODE @ LINE:COL`
line per expected diagnostic (positions count characters, 1-based; the
directive lines themselves do not change positions when placed at the end).
`cargo test -p olic --test fixtures` checks the set matches exactly.

## Updating AST snapshots

```bash
UPDATE_SNAPSHOTS=1 cargo test -p olic --test snapshots
UPDATE_SNAPSHOTS=1 cargo test -p olic --test sema_fixtures
```

Review the diff of `tests/snapshots/*.ast` before committing.
