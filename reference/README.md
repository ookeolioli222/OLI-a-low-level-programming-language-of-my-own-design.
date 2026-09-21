# Reference implementation (not part of the toolchain)

This directory holds the Rust implementation of the Oli-- V0 front end
(lexer, parser, semantic analysis) written during Phases 1–1b, before the
project committed to full autonomy (`docs/design/0017-genesis-bootstrap.md`).

It is **not** part of the Oli-- toolchain and never will be: the language is
bootstrapped from hand-written machine code (`genesis/`) and then written in
Oli-- itself. The reference exists only as an *oracle*: it produced the
snapshots in `tests/snapshots/` and the expected diagnostics of
`tests/parse/err/` and `tests/sema/err/`, and it lets the Oli-- compiler be
compared against a second implementation of the same specification until it
passes every fixture. After that it is deleted.

```bash
cd reference
cargo test --workspace
cargo run -q --bin olic -- ../examples/packet_demo.oli --show-sema
```
