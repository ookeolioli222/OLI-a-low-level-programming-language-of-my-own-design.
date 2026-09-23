# 0017 — Full autonomy: bootstrap from hand-written machine code

Status: accepted 2026-09-20 (the user's directive), supersedes the
"Rust bootstrap" assumption of 0001.

## Problem
The user requires that Oli-- depend on no other language and no other
compiler — not Rust, not C, not C++ — at any point of its toolchain. Yet a
compiler written in Oli-- must be compiled by something that already exists.

## Existing approaches
- Every self-hosted language kept a foreign seed (OCaml for Rust, B/assembly
  for C, C for Zig) and later called it "historical".
- Bootstrappable-builds (`stage0`): a ~250-byte hand-written `hex0`, then
  `hex1`, `hex2`, `M0`, `cc_x86`, `mescc` … up to GCC, with every step
  auditable as text. Months of work; proven possible.
- Forth systems: a tiny machine-code kernel and everything else in Forth.

## Oli-- approach — the genesis chain
| Layer | Artifact | Written in | Proves |
|---|---|---|---|
| 0 | `genesis/0-hex0/hex0.hex` (+ `hex0.bin` materialized once by `genesis/hexbin.sh`) | 322 hand-encoded bytes with comments | reads a listing, writes bytes; `hex0 < hex0.hex` reproduces `hex0.bin` |
| 1 | `genesis/1-hex2/hex2.hex0` | hex0 notation | labels and relative/absolute addresses — no hand-computed jumps |
| 2 | `genesis/2-asm/asm.hex2` | hex2 notation | assembles Oli-- `machine x64` blocks (labels, statics, a fixed instruction table) into ELF64: the last hand-encoded layer |
| 3 | `genesis/3-oli1/*.oli` | Oli-- `machine` blocks | compiles "oli-core": the subset of V0 the real compiler is written in |
| 4 | `compiler/*.oli` | oli-core | `olic`: the full V0 compiler; `olic(olic) == olic(olic(olic))` |

Rules:
1. **The only non-Oli-- step is materializing `hex0.bin`** from its listing,
   done by `hexbin.sh` (POSIX sh, ~15 lines) or by a person with a hex
   editor. Shell scripts orchestrate and test; they never compile anything.
2. Every layer is checked by reproducing the previous layer's binary from its
   listing (`cmp`) and by running the fixtures of `tests/`.
3. From layer 2 on, every source file is Oli-- syntax: the language
   bootstraps from its own `machine` sub-language, which is why `machine`
   blocks remain in the language (0015 keeps them as the escape hatch).
4. The foreign implementation of Phases 1–1b produced the snapshots and
   expected diagnostics in `tests/`. It was kept briefly as an *oracle*
   and removed on 2026-09-21 (see the amendment below); the fixture corpus
   is the only oracle the Oli-- compiler is compared against.
5. Linux x86-64 first (the seed is an ELF that uses `read`/`write`/`exit`);
   the Windows PE target is added by the Oli-- compiler after the fixpoint.

## Advantages
- The finished toolchain's provenance is: bytes anyone can audit → Oli--.
- Each layer is small enough to read; the chain is reproducible by `sh genesis/test.sh`.
- The design work of Phases 0–1b (spec, fixtures, snapshots, diagnostics) is
  reused unchanged; only the implementation language changes.

## Disadvantages
- The longest phase of the project: layers 2 and 3 are thousands of lines of
  labeled hex and machine-block assembly, debugged by running them.
- Until layer 4, the compiler runs only on Linux x86-64 (WSL on the user's PC).
- The language must stay frozen enough during layers 2–4 that oli-core is a
  stable target; changes go through the spec first.

## Machine cost
None at run time. Build cost: the chain runs in under a second once written.

## Safety implications
No foreign compiler can inject behaviour into the toolchain; the seed is
322 bytes and is verified by self-reproduction on every test run.

## Alternatives rejected
- Rust seed discarded after self-hosting: rejected by the user (no Rust, ever).
- A Forth-like interpreted bootstrap language: adds a second language of our
  own design; the `machine` sub-language of Oli-- already plays that role.
- Checking in later-stage binaries: allowed only for the seed; every other
  binary must be rebuilt from text by the chain.

## Amendment 2026-09-21 — oracle removed early
The user directed that no foreign-language code remain in the repository at
all, even as a non-toolchain oracle. `reference/` and its CI job were deleted
before G4. Consequence: until the Oli-- front end exists, the fixtures in
`tests/` and the snapshots cannot be executed by anything in the repository;
they are frozen and serve as the acceptance suite for G4. No design decision
of this record changes.
