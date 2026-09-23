# Design decision records

One file per major decision, in the fixed format:
Problem · Existing approaches · Oli-- approach · Advantages · Disadvantages ·
Machine cost · Safety implications · Alternatives rejected.

| # | Decision |
|---|----------|
| [0001](0001-own-backend-no-llvm.md) | Own x86-64 encoder and ELF writer; no LLVM/GCC/Cranelift/C |
| [0002](0002-bind-store-move.md) | Three memory operators: `:=` bind, `<-` store, `<~` move |
| [0003](0003-zones.md) | Zones as the allocation model |
| [0004](0004-address-kinds.md) | `addr` / `ref` / `view` / `own` instead of one pointer |
| [0005](0005-capabilities.md) | Per-procedure hardware capabilities (`permit`) |
| [0006](0006-overflow-and-bounds.md) | Trapping arithmetic with `wrap`/`sat`/`checked`; checked views |
| [0007](0007-fallible-results.md) | `T or E`, `ret`/`fail`, `else`, `case` |
| [0008](0008-procedures-not-flows.md) | Procedures with contract headers instead of a "flow" model |
| [0009](0009-machine-blocks.md) | `machine` blocks assembled by the compiler |
| [0010](0010-block-syntax-selection.md) | Keyword/`end` block syntax (Proposal A) |
| [0011](0011-endian-typed-integers.md) | `be T` / `le T` integer types |
| [0012](0012-memory-spaces.md) | `physaddr`, `mmio`, `port` as distinct types |
| [0013](0013-parser-recovery.md) | Newline-terminated statements, bracket trivia and `end`-anchored recovery |
| [0014](0014-semantic-graph-and-regions.md) | The semantic graph: typed places and values, regions on every value |
| [0015](0015-hardware-commands.md) | Hardware places and commands (`cpu.*`, `arch.x64.*`, `port.*`) — accepted, spec text pending |
| [0016](0016-entry-everywhere.md) | `entry` as the only entry point on every target — accepted, spec text pending |
| [0017](0017-genesis-bootstrap.md) | Full autonomy: bootstrap from hand-written machine code (the genesis chain) |
| [0018](0018-ecosystem-libraries.md) | Ecosystem libraries oli.compute and oli.sec (accepted, deferred) |
| [0019](0019-genesis-assembler-progress.md) | Checked executable genesis assembler subset, tests and completion limits |
| [0020](0020-genesis-oli-core.md) | Genesis layer 3: the oli-core compiler (oli1) |
| [0021](0021-ide-architecture.md) | IDE architecture: terminal first, one event loop, out-of-process service over a layout-framed protocol |
