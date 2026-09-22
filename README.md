# Oli--

Oli-- is a new low-level systems language designed from the machine up:
own syntax, own semantics, own memory model (zones + views + capabilities),
own intermediate representation (OIR), own x86-64 encoder and own ELF64 writer
— and, by decision, **own everything**: the toolchain is bootstrapped from
hand-written machine code and then written in Oli-- itself. No Rust, C or
C++ in the toolchain. No LLVM, GCC, `as` or `ld`. No runtime, no libc.

## Guiding question

> If C, C++ and Rust had never existed, but we knew modern CPUs, RAM and
> operating systems — how would we design Oli--?

## Project status

**Genesis — bootstrapping from hand-written machine code (`docs/design/0017`).
G0/G1 reproduce their binaries. G2 assembles real r64 instructions,
memory operands, branches and read-only data into runnable Linux ELF files.
G3 (`oli1`, written in `machine x64`) compiles oli-core programs with
run-time locals, expressions, strings, `if`/`while`, syscall results and
procedures (SysV parameters, recursion, forward calls), zones (`mmap`-backed,
`z.bytes`, `z.make`), bounds-checked views, raw memory, layouts, refs and
fallible results (`T or E`, `fail`, `else` handlers, `case`) —
`genesis/3-oli1/tests/hello.oli` prints `Hello Oli--`, `fib.oli` computes
fib(10), `slurp.oli` reads stdin into a zone and upper-cases it in place and
`propagate.oli` propagates parse failures through two procedures, all from
oli-core source. G4 has begun: the V0 lexer, parser and
item collection (`compiler/`), written in oli-core and built by `oli1`,
tokenize every fixture, reproduce every `tests/snapshots/*.ast` byte for byte
with the exact diagnostics of the negative fixtures, and lay out the layouts,
choices and constants of every `tests/snapshots/*.sema` (`docs/design/0022`).**

| Phase | Content | Status |
|-------|---------|--------|
| 0 | Language design: syntax experiments, memory model, machine model, OIR, ABI | done |
| 1, 1b | Front-end design validated on the fixture corpus (`tests/`, `tests/snapshots`); the temporary foreign oracle was removed on 2026-09-21 — the corpus is now the target for the Oli-- front end | done |
| G0 | `genesis/0-hex0`: hex listing → bytes, self-reproducing | **done** |
| G1 | `genesis/1-hex2`: labels and relative/absolute addresses | done |
| G2 | `genesis/2-asm`: assembler for Oli-- `machine x64` blocks | working subset; narrower operands and multi-segment ELF remain |
| G3 | `genesis/3-oli1`: oli-core compiler written in Oli-- machine blocks | steps 0–6d done: locals, expressions, strings, `if`/`while`, syscalls, procedures, zones, views, raw memory, traps, layouts, refs and fallible results (`T or E`, `fail`, `else`, `case`) and module constants; next: `oli1` compiles the first `olic` modules (G4) |
| G4 | `compiler/`: `olic` in oli-core (design 0022) — lexer, parser, §8 diagnostics, module loader and item collection; `--show-tokens`, `--show-ast` and `--show-items` reproduce every AST snapshot and the item section of every semantic snapshot; the rest of semantic analysis and the back end next; fixpoint `stage2 == stage3` | **in progress** |
| M1 | `olic hello.oli && ./hello` prints `Hello Oli--` via raw Linux syscalls | not started |
| M2 | Variables, arithmetic, control flow, procedures, layouts, views, zones | not started |
| M3 | Freestanding binary with own entry point and own stack | not started |
| M4 | Minimal kernel written in Oli-- | not started |

## Try it

```bash
sh genesis/test.sh                      # Linux x86-64 (WSL is fine): builds and verifies the genesis chain
```

After the genesis tests, run this from the repository root in Linux/WSL:

```bash
genesis/build/asm.bin < examples/genesis/hello.oli > genesis/build/hello.elf
chmod +x genesis/build/hello.elf
genesis/build/hello.elf                  # Hello Oli--

genesis/build/oli1.bin < genesis/3-oli1/tests/fib.oli > genesis/build/fib.elf
chmod +x genesis/build/fib.elf
genesis/build/fib.elf; echo $?           # 55 - an oli-core program compiled by oli1

genesis/build/show_tokens < examples/hello.oli    # the front end of olic, built by oli1
genesis/build/show_ast    < examples/hello.oli
genesis/build/show_items  < examples/hello.oli
```

This example uses the genesis machine sub-language. The high-level
`examples/hello.oli`, self-hosting, kernel and ecosystem libraries are still
future milestones. See [verified status and completion gates](docs/PROJECT_STATUS.md)
and [the exact assembler subset](genesis/2-asm/SPEC.md#9-implementation-status-genesis2-asmasmhex2).

## Document map

Design documents (`docs/`):

| File | Purpose |
|------|---------|
| [LANGUAGE_VISION.md](docs/LANGUAGE_VISION.md) | Goals, principles, cost model, non-goals |
| [COMPILER_ARCHITECTURE.md](docs/COMPILER_ARCHITECTURE.md) | Crates, policies, front-end design, diagnostic codes, tests |
| [SYNTAX_EXPERIMENTS.md](docs/SYNTAX_EXPERIMENTS.md) | Three competing syntax designs, evaluation, selection |
| [MEMORY_MODEL.md](docs/MEMORY_MODEL.md) | Regions, zones, views, references, raw addresses, escape analysis |
| [MACHINE_MODEL.md](docs/MACHINE_MODEL.md) | The abstract machine Oli-- programs run on; traps; cost classes |
| [OIR_SPEC.md](docs/OIR_SPEC.md) | Oli Intermediate Representation and the backend pipeline |
| [ABI.md](docs/ABI.md) | Calling convention, value layout, symbols, ELF output, Linux syscalls |
| [FREESTANDING.md](docs/FREESTANDING.md) | `--freestanding` mode: no OS, no libc, own entry point |
| [KERNEL_PROGRAMMING.md](docs/KERNEL_PROGRAMMING.md) | Writing boot code and kernels in Oli-- |
| [ROADMAP.md](ROADMAP.md) | Language, compiler, stdlib and ecosystem-library plan |
| [PROJECT_STATUS.md](docs/PROJECT_STATUS.md) | Verified baseline, implemented work and completion gates |
| [IDE_PLAN.md](docs/IDE_PLAN.md) | The Oli-- IDE: `olis` language service, `olide` editor, stages and budgets (planned) |
| [ecosystem/](docs/ecosystem/) | Planned libraries written in Oli--: oli.compute, oli.sec |
| [design/](docs/design/) | Decision records (Problem / approaches / Oli-- approach / cost / safety) |

Normative V0 specification (`spec/`):

| File | Purpose |
|------|---------|
| [OLI_SYNTAX_V0.md](spec/OLI_SYNTAX_V0.md) | Lexical structure and grammar of Oli-- V0 |
| [OLI_SEMANTICS_V0.md](spec/OLI_SEMANTICS_V0.md) | Static and dynamic semantics of V0 |
| [OLI_MEMORY_V0.md](spec/OLI_MEMORY_V0.md) | The V0 subset of the memory model the compiler enforces |

## Naming

| Thing | Name |
|-------|------|
| Source file | `name.oli` |
| Compiler | `olic` |
| Project tool | `oli` |
| Manifest | `oli.toml` |
| Intermediate representation | OIR |

## Repository layout

```
genesis/             the bootstrap chain: hex0 (bytes) → hex2 → asm → oli1 → olic
compiler/            olic written in oli-core (G4): io, lexer, diagnostics, AST,
                     parser, module loader, item collection, one driver per stage
genesis/hexbin.sh    the only non-Oli-- build step: materializes hex0.bin once (POSIX sh)
genesis/test.sh      verifies every layer (POSIX sh + coreutils)
lib/                 core and std library modules written in Oli-- (V0 subset)
examples/            hello.oli, packet_demo.oli (reference programs)
tests/parse/ok       programs that must parse cleanly
tests/parse/err      programs with `-- expect: CODE @ LINE:COL` lines
tests/sema/ok,err    the same for semantic analysis (with the real lib/)
tests/snapshots      expected --show-ast and --show-sema output (target for the Oli-- front end)
docs/, spec/         design documents and the normative V0 specification
```

## Development process

Every stage: DESIGN → CRITIQUE → ORIGINALITY REVIEW → SPECIFICATION →
SMALL IMPLEMENTATION → TEST → BENCHMARK → REVIEW → DOCUMENT. Then stop and report.
See [CONTRIBUTING.md](CONTRIBUTING.md).
