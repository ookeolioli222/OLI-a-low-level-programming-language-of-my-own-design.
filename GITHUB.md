# Oli--

## The complete project guide

Oli-- is a low-level systems language designed from the machine up. It has its
own syntax, semantics, memory model, intermediate representation, x86-64
encoder and ELF64 writer. The toolchain is bootstrapped from hand-written
machine code and then written in Oli-- itself: no Rust, C or C++, no LLVM, no
external assembler or linker, no libc and no runtime.

This file is the GitHub-facing overview of the project. The original documents
remain available as the detailed and normative references linked below.

> If C, C++ and Rust had never existed, but we knew modern CPUs, RAM and
> operating systems, how would we design a systems language?

## Contents

- [Status](#status)
- [Quick start](#quick-start)
- [Five-minute language tour](#five-minute-language-tour)
- [Design principles](#design-principles)
- [Memory model](#memory-model)
- [Compiler pipeline](#compiler-pipeline)
- [Genesis bootstrap](#genesis-bootstrap)
- [Specifications and contracts](#specifications-and-contracts)
- [Roadmap](#roadmap)
- [Documentation index](#documentation-index)
- [Contributing](#contributing)

## Status

The project is in Genesis G4 and M1/M2 development.

| Area | Status |
|------|--------|
| Language design, machine model, ABI, memory model and OIR | complete as the V0 design target |
| G0 hex0 and G1 hex2 bootstrap layers | complete and self-reproducing |
| G2 `machine x64` assembler | working tested subset; more operands and ELF segments remain |
| G3 `oli1` compiler | steps 0-6f tested, including zones, views, layouts, refs and explicit conversions |
| G4 front end | lexer, parser, semantic graph and diagnostics implemented in Oli--; fixture corpus passes |
| G4 back end | OIR, CFG, verifier, SSA, optimization, x86-64 lowering and ELF64 output implemented for a growing V0 subset |
| M1 | high-level `examples/hello.oli` compiles to a static ELF64 and runs without libc or a linker |
| M2 | variables, arithmetic, control flow, procedures, zones, views, `each`, layouts and refs run; more features remain |
| M3 freestanding output and M4 kernel | planned |
| Standard library, IDE and ecosystem libraries | planned or gated on later milestones |

The verified details and completion gates are maintained in
[`docs/PROJECT_STATUS.md`](docs/PROJECT_STATUS.md). This status is deliberately
conservative: specified features are not called implemented until executable
tests prove them.

## Quick start

Requirements: Linux x86-64 or WSL, POSIX shell and coreutils.

Build and run the full bootstrap chain:

```sh
sh genesis/test.sh
```

Build the high-level example with `olic`:

```sh
genesis/build/olic < examples/hello.oli > /tmp/hello
chmod +x /tmp/hello
/tmp/hello
```

Expected output:

```text
Hello Oli--
```

Inspect compiler stages:

```sh
genesis/build/show_tokens < examples/hello.oli
genesis/build/show_ast    < examples/hello.oli
genesis/build/show_sema   < examples/hello.oli
genesis/build/show_oir    < examples/hello.oli
genesis/build/show_ssa    < examples/hello.oli
genesis/build/show_opt    < examples/hello.oli
```

The repository convenience command is also available through `bin/olic`:

```sh
bin/olic examples/hello.oli
bin/olic --check examples/hello.oli
bin/olic --show-opt examples/hello.oli
```

For VS Code, the repository includes tasks for compiling, running, checking and
inspecting the current `.oli` file. Syntax highlighting lives in
[`tools/vscode-oli/`](tools/vscode-oli/).

## Five-minute language tour

```oli
module hello
import std.os

proc start -> s32
    entry
    permit os.syscall
    message := "Hello Oli--\n"
    os.syscall(os.WRITE, 1, message.addr, message.len)
    ret 0
end
```

Important syntax and semantics:

- `module` declares a module and `import` loads another module.
- `:=` creates an immutable binding to a value.
- `name : T` creates a typed place in memory.
- `<-` stores into an existing place; `<~` is the reserved move operator.
- `proc ... end` declares a procedure; `entry` marks the program entry point.
- `permit` declares capabilities such as `os.syscall` or `memory.raw`.
- `---` is a documentation comment; `--` is an ordinary comment.
- Strings are `(address, length)` views into read-only data, not terminated C strings.
- `if`, `while`, `loop`, `each`, `break` and `continue` are explicit control flow.
- Fallible procedures return `T or E`; callers resolve the result with `else`, `case` or propagation.
- Plain arithmetic traps on overflow. `wrap(e)` explicitly requests wrapping arithmetic.

The practical language reference, including construct-by-construct status, is
[`docs/LANGUAGE.md`](docs/LANGUAGE.md). The command reference is
[`docs/COMMANDS.md`](docs/COMMANDS.md), and the Polish project example is
[`docs/PROGRAM_PRZYKLAD.md`](docs/PROGRAM_PRZYKLAD.md).

## Design principles

1. **Visible cost.** Allocation, copying, calls, syscalls and checks are explicit.
2. **No hidden behavior.** There is no garbage collector, exception runtime, implicit allocation or hidden entry point.
3. **Defined low-level behavior.** Overflow, bounds, alignment and division failures have specified traps.
4. **Explicit authority.** Raw memory, MMIO, ports, assembly and syscalls require capabilities.
5. **Regional memory.** Zones and views make allocation and lifetime visible without requiring lifetime annotations in ordinary code.
6. **Small native toolchain.** The compiler owns its front end, optimizer, encoder and ELF writer.
7. **Honest implementation status.** Unsupported code generation reports `E0900` and emits no executable.

The complete vision, goals, non-goals and cost classes are in
[`docs/LANGUAGE_VISION.md`](docs/LANGUAGE_VISION.md). Syntax alternatives and
the design-selection process are in [`docs/SYNTAX_EXPERIMENTS.md`](docs/SYNTAX_EXPERIMENTS.md).

## Memory model

Every memory value belongs to a region:

| Region | Lifetime |
|--------|----------|
| `static` | the whole program |
| `frame` | the current procedure |
| `zone` | the lexical `zone ... end` block |
| `raw` | programmer-managed machine memory |

Oli-- separates four machine concepts that are often conflated by a pointer:

| Kind | Meaning |
|------|---------|
| `addr T` | raw address, with no bounds or lifetime tracking |
| `ref T` | reference to one live object |
| `view T` | address plus element count, bounds checked |
| `own T` | unique resource holder, consumed exactly once (later version) |

A zone is a bump-allocated region released as a whole. A view never copies its
contents. `T.at(view)` gives a checked view of bytes as a layout. Raw access is
available only with an explicit capability. The optimizer may remove a check
only after recording a proof in OIR.

See [`docs/MEMORY_MODEL.md`](docs/MEMORY_MODEL.md) for the conceptual model
and [`spec/OLI_MEMORY_V0.md`](spec/OLI_MEMORY_V0.md) for enforceable V0 rules.

## Compiler pipeline

```text
source
  -> lexer                 (--show-tokens)
  -> parser / AST          (--show-ast)
  -> semantic graph        (--show-sema)
  -> OIR                   (--show-oir)
  -> CFG and verifier
  -> SSA / mem2reg          (--show-ssa)
  -> OIR optimization       (--show-oir=opt)
  -> x86-64 lowering
  -> ELF64 writer
  -> native executable
```

The semantic graph attaches a type and region to every expression. The OIR
represents checks as instructions, keeps memory-space and capability metadata,
and preserves the evidence used to remove a check. The current optimizer covers
constant folding, check elision with recorded proofs, copy propagation and
dead-code elimination. The current x86-64 lowering uses frame words for values;
register allocation and common-subexpression elimination remain future work.

The detailed architecture is in
[`docs/COMPILER_ARCHITECTURE.md`](docs/COMPILER_ARCHITECTURE.md), while the
normative intermediate representation is in [`docs/OIR_SPEC.md`](docs/OIR_SPEC.md).
The ABI and ELF/SysV details are in [`docs/ABI.md`](docs/ABI.md).

## Genesis bootstrap

The bootstrap chain is intentionally small and auditable:

| Layer | Input and output | Purpose |
|------|------------------|---------|
| G0 | `hex0.hex` -> `hex0.bin` | materialize bytes and reproduce the binary |
| G1 | `hex2.hex0` -> `hex2.bin` | labels and relative/absolute fixups |
| G2 | `machine x64` source -> ELF64 | assemble the tested machine subset |
| G3 | oli-core source -> native ELF64 | compile the compiler subset with `oli1` |
| G4 | high-level Oli-- -> native ELF64 | self-hosted front end and back end |

`genesis/test.sh` rebuilds each layer and runs its tests. The G2 assembler
contract is [`genesis/2-asm/SPEC.md`](genesis/2-asm/SPEC.md), and the G3
language/compiler contract is [`genesis/3-oli1/SPEC.md`](genesis/3-oli1/SPEC.md).
The complete language/compiler architecture is documented in
[`docs/design/0017-genesis-bootstrap.md`](docs/design/0017-genesis-bootstrap.md)
and [`docs/design/0022-olic-in-oli-core.md`](docs/design/0022-olic-in-oli-core.md).

## Specifications and contracts

These are the normative documents. The overview above is explanatory and does
not replace them:

- [`spec/OLI_SYNTAX_V0.md`](spec/OLI_SYNTAX_V0.md): lexical rules, grammar and diagnostic format.
- [`spec/OLI_SEMANTICS_V0.md`](spec/OLI_SEMANTICS_V0.md): typing, control flow, capabilities, regions and errors.
- [`spec/OLI_MEMORY_V0.md`](spec/OLI_MEMORY_V0.md): address kinds, regions, bounds and memory spaces.
- [`docs/OIR_SPEC.md`](docs/OIR_SPEC.md): OIR structure, instructions, passes and verifier obligations.
- [`docs/ABI.md`](docs/ABI.md): SysV calling convention, layouts, symbols, ELF64 and Linux syscalls.
- [`docs/FREESTANDING.md`](docs/FREESTANDING.md): own entry point, stack and no-OS execution.
- [`docs/KERNEL_PROGRAMMING.md`](docs/KERNEL_PROGRAMMING.md): boot code and kernel direction.

## Roadmap

The order is intentional: each layer is designed, specified, implemented,
tested and documented before the next.

1. Finish G4: remaining V0 lowering, fallible results, `case`, machine blocks,
   statics, register allocation, common-subexpression elimination and the
   `stage2 == stage3` fixpoint.
2. Complete M2 with the full native runtime surface.
3. Build freestanding M3 output with an own entry point and stack.
4. Build the minimal M4 kernel in Oli--.
5. Add V1 features: generics, ownership resources, atomics, MMIO, port I/O,
   interrupts and bitfields.
6. Add V2 features: floating point, SIMD, threads, FFI, libraries and Windows PE/COFF.
7. Implement `core`, `std`, allocator-explicit collections, then the gated
   `oli.compute` and `oli.sec` ecosystems.
8. Build the `olis` language service and `olide` editor after the language and
   toolchain foundations are stable.

The detailed schedule is [`ROADMAP.md`](ROADMAP.md), the IDE plan is
[`docs/IDE_PLAN.md`](docs/IDE_PLAN.md), and the completion gates are in
[`docs/PROJECT_STATUS.md`](docs/PROJECT_STATUS.md).

## Documentation index

### Core references

- [`docs/LANGUAGE.md`](docs/LANGUAGE.md), [`docs/LANGUAGE_VISION.md`](docs/LANGUAGE_VISION.md)
- [`docs/COMMANDS.md`](docs/COMMANDS.md), [`docs/COMPILER_ARCHITECTURE.md`](docs/COMPILER_ARCHITECTURE.md)
- [`docs/MEMORY_MODEL.md`](docs/MEMORY_MODEL.md), [`docs/MACHINE_MODEL.md`](docs/MACHINE_MODEL.md)
- [`docs/OIR_SPEC.md`](docs/OIR_SPEC.md), [`docs/ABI.md`](docs/ABI.md)
- [`docs/PROJECT_STATUS.md`](docs/PROJECT_STATUS.md), [`ROADMAP.md`](ROADMAP.md)

### Runtime and platform

- [`docs/FREESTANDING.md`](docs/FREESTANDING.md)
- [`docs/KERNEL_PROGRAMMING.md`](docs/KERNEL_PROGRAMMING.md)
- [`docs/PROGRAM_PRZYKLAD.md`](docs/PROGRAM_PRZYKLAD.md)
- [`genesis/2-asm/SPEC.md`](genesis/2-asm/SPEC.md)
- [`genesis/3-oli1/SPEC.md`](genesis/3-oli1/SPEC.md)

### Design decisions

All decision records are listed in [`docs/design/README.md`](docs/design/README.md):

- [`0001`](docs/design/0001-own-backend-no-llvm.md) own backend; [`0002`](docs/design/0002-bind-store-move.md) bindings, stores and moves; [`0003`](docs/design/0003-zones.md) zones; [`0004`](docs/design/0004-address-kinds.md) address kinds.
- [`0005`](docs/design/0005-capabilities.md) capabilities; [`0006`](docs/design/0006-overflow-and-bounds.md) overflow and bounds; [`0007`](docs/design/0007-fallible-results.md) fallible results.
- [`0008`](docs/design/0008-procedures-not-flows.md) procedures; [`0009`](docs/design/0009-machine-blocks.md) machine blocks; [`0010`](docs/design/0010-block-syntax-selection.md) block syntax.
- [`0011`](docs/design/0011-endian-typed-integers.md) endian integers; [`0012`](docs/design/0012-memory-spaces.md) memory spaces; [`0013`](docs/design/0013-parser-recovery.md) parser recovery.
- [`0014`](docs/design/0014-semantic-graph-and-regions.md) semantic graph and regions; [`0015`](docs/design/0015-hardware-commands.md) hardware commands; [`0016`](docs/design/0016-entry-everywhere.md) entry points.
- [`0017`](docs/design/0017-genesis-bootstrap.md) Genesis bootstrap; [`0018`](docs/design/0018-ecosystem-libraries.md) ecosystem libraries; [`0019`](docs/design/0019-genesis-assembler-progress.md) assembler progress.
- [`0020`](docs/design/0020-genesis-oli-core.md) Genesis oli-core; [`0021`](docs/design/0021-ide-architecture.md) IDE architecture; [`0022`](docs/design/0022-olic-in-oli-core.md) `olic` in Oli--.

### Ecosystem and tooling

- [`docs/ecosystem/README.md`](docs/ecosystem/README.md)
- [`docs/ecosystem/oli-compute.md`](docs/ecosystem/oli-compute.md)
- [`docs/ecosystem/oli-sec.md`](docs/ecosystem/oli-sec.md)
- [`docs/ecosystem/oli-compute.brief.md`](docs/ecosystem/oli-compute.brief.md)
- [`docs/ecosystem/oli-sec.brief.md`](docs/ecosystem/oli-sec.brief.md)
- [`tools/vscode-oli/README.md`](tools/vscode-oli/README.md)
- [`docs/projekt_OLI--/README.md`](docs/projekt_OLI--/README.md)
- [`docs/projekt_OLI--/Oli_Projekt_Dokumentacja_EN.md`](docs/projekt_OLI--/Oli_Projekt_Dokumentacja_EN.md)
- [`docs/projekt_OLI--/Oli_Projekt_Dokumentacja_PL.md`](docs/projekt_OLI--/Oli_Projekt_Dokumentacja_PL.md)

## Contributing

The project follows:

```text
DESIGN -> CRITIQUE -> ORIGINALITY REVIEW -> SPECIFICATION
-> SMALL IMPLEMENTATION -> TEST -> BENCHMARK -> REVIEW -> DOCUMENT
```

Language changes update `spec/` and a design record. Unsupported features must
return `E0900`, never a fake implementation. New diagnostics require negative
fixtures. AST and semantic snapshots are frozen and change only with an
explicit specification change. The compiler must report diagnostics or defined
exit statuses rather than crash on user input.

Full contributor rules and fixture instructions are in
[`CONTRIBUTING.md`](CONTRIBUTING.md).

## License and project context

This repository is an evolving language and toolchain research project. Read
the linked specifications and verified status before relying on an experimental
feature in production code.
