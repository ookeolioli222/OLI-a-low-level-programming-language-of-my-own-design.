# Oli-- completion plan and verified baseline

Reviewed 2026-09-20; foreign oracle removed 2026-09-21; semantic analysis
completed, genesis step 6f (explicit conversions), stage 1 of the back end
— OIR, x86-64 lowering and the ELF64 writer, with M1 reached — stage 2
— basic blocks, SSA with phi, and the verifier — and stage 3 — the OIR
passes of OIR_SPEC §6 — landed 2026-09-22; stage 4 — zones, views and `each`
— stage 5 — layouts and refs — and stage 6 — fallible results — landed
2026-09-23, and the same day **`olic` reached its fixpoint: `stage2 ==
stage3`** (`genesis/test.sh` layer 6).
The requested scope is the entire roadmap, in order.
This document records actual implementation, not an assertion that the project
is complete.

## Baseline assessment

- The design and normative V0 specification are substantial and usable as a
  target. Design 0017 explicitly requires an autonomous bootstrap, without a
  Rust/C/C++ compiler or an external assembler/linker in the toolchain.
- G0 and G1 build and reproduce their binaries on Linux x86-64 under WSL.
- The original G2 implementation accepted only `bytes`, silently ignored other
  lines, and did not validate structure or entry selection. Its encoding-table
  CPU fixtures validated manually written bytes, not the assembler's encoder.
- The temporary foreign front end (Phases 1–1b) was removed on 2026-09-21 at
  the user's direction. Its last run passed all 77 fixture tests; that corpus
  (`tests/`, `tests/snapshots`) is now the acceptance suite for the Oli--
  front end. Fixture groups can contain multiple source files. Fixtures
  marked `-- reference: skip` were excluded by the old harness; passing the
  suite does not prove these features exist. Since 2026-09-22 the repository's
  own front end (`compiler/`, built by `oli1`) parses and analyses high-level
  Oli-- and reproduces the whole corpus.
- `genesis/3-oli1/` (the oli-core compiler `oli1`, written in `machine x64`)
  exists and passes layer 3 of `genesis/test.sh` for steps 0–6f: locals,
  expressions, strings, control flow, syscalls, procedures, zones, views, raw
  memory, layouts, refs, fallible results, module constants, typed places,
  `rw` field types, `loop` and the explicit conversions `T(x)`, `T.wrap(x)`
  and `T.bits(x)`. `compiler/` is the front end (design 0022): the V0 lexer, parser, §8 diagnostic renderer,
  module loader, item collection, signatures, local tables, expression typing
  and typed bodies in oli-core, built by `oli1`, pass the fixture corpus at
  layer 4 — all four `tests/snapshots/*.ast` **and all three
  `tests/snapshots/*.sema`** are reproduced byte for byte (items, signatures,
  locals, and a type and a region on every expression of every body), all
  fifteen `tests/parse/err` fixtures and **all fifteen** `tests/sema/err`
  fixtures give exactly their expected diagnostics (capabilities, `E0900`,
  constants, layouts, scopes, definite assignment, reachability, failures,
  exhaustiveness, read-only places, region escapes, literal types, address
  spaces and implicit narrowing) with no diagnostic on any positive fixture,
  and no semantic rule is gated any more. Since the same day the back end
  exists in two stages (layer 5 of the harness): `compiler/oir.oli` builds the
  OIR instruction stream, `compiler/cfg.oli` cuts it into basic blocks and
  computes dominators and the verifier of OIR_SPEC §8, `compiler/ssa.oli`
  promotes every place to a value with phi nodes (`mem2reg`),
  `compiler/opt.oli` runs the passes of OIR_SPEC §6 (constant folding, check
  elision with a recorded proof for every removal, copy propagation and dead
  code), `compiler/x64.oli` lowers the blocks to x86-64 bytes and
  `compiler/elf.oli` writes a one-segment static ELF64, so **M1 is reached** —
  `olic < examples/hello.oli` produces a 284-byte binary that prints
  `Hello Oli--` with no libc and no linker. Since 2026-09-23 the back end also
  lowers zones (`zone`, `z.bytes`), views (`v[i]`, `v[i] <- x`, subviews,
  `.addr`/`.len`, views as parameters and results) and `each` over a view,
  with `bounds` and `zone_exhausted` traps, and layouts and refs (`Name.at`,
  `z.make`, field loads and stores at every width, `ref x`) with the
  `misaligned` trap — `tests/run/memory.oli`, `layouts.oli` and six trap
  fixtures pin them; fallible results (`fail`, `else`, `case`) run since the
  same day (`fallible.oli`). Common-subexpression elimination and the
  register allocator do not exist; the lowering gives every value its own
  frame word. The standard-library implementation and the kernel
  do not exist. The checks
  measured the compiler's own source three times: the first measurement
  specified oli1 step 6e (typed places, `rw` field types, `loop`), the second,
  from expression typing, specified step 6f (explicit conversions — 93
  narrowing sites in `compiler/`, none in `lib/`, `examples/` or the
  fixtures), and the third, after step 6f landed and all 93 sites were
  rewritten, found no further gap: every construct `compiler/` uses is both
  oli-core and valid V0. `olic`
  analyses all seventeen of its own modules as one program without a single
  diagnostic — the harness re-runs that self-analysis on every build.
- README and ROADMAP originally disagreed with each other and with G2's code.

The single-file reference for the language and the working commands is
[`docs/LANGUAGE.md`](LANGUAGE.md); it marks every construct as running,
parsing or planned.

## Completion gates

1. **G2 assembler:** checked structure; instruction encodings; memory operands;
   procedure/local symbols and fixups; static data and ELF segment permissions;
   byte-exact tests, CPU execution, rejection tests and deterministic rebuilds.
2. **G3 oli-core:** freeze the smallest V0 subset needed by the compiler; write
   its compiler in the machine sub-language; verify examples and diagnostics.
3. **G4 self-hosting V0:** port lexer/parser/semantics into Oli--; implement OIR,
   x64 lowering and ELF; pass the shared fixtures; establish stage2 == stage3.
   The fixture corpus is the required test set; there is no other oracle.
4. **M1-M2:** run the high-level hello and packet examples; exercise arithmetic,
   control flow, procedures, layouts, views and zones with native runtime tests.
5. **M3-M4:** freestanding output, own entry/stack, bootable minimal kernel;
   document an emulator-based reproducible test and its success condition.
6. **V1:** generics, ownership, atomics, MMIO, port I/O, interrupts and bitfields;
   specify each feature, implement it and add both acceptance/rejection tests.
7. **V2 and Windows:** floating point, SIMD, threads, FFI, libraries; PE/COFF
   emission and platform bindings; cross-target compile/run tests.
8. **Libraries:** core/std and allocator-explicit collections, then oli.compute
   and oli.sec after their prerequisites are genuinely implemented.

Do not replace the autonomy requirement with a convenient foreign bootstrap,
mark design-only features complete, or treat machine-byte tests as proof of
source-level compiler functionality.

## Implemented in the semantic-analysis stage (2026-09-22)

`compiler/body.oli` gives every expression a type, a region and a printed
form, so `--show-sema` is complete: all three `tests/snapshots/*.sema` are
reproduced byte for byte and the last two negative fixtures (`literals.oli`,
`mixed_addr.oli`) report `E0201`, `E0202` and `E0203` at their exact
positions. `compiler/show_items.oli` became `compiler/show_sema.oli` because
it now prints the whole semantic graph, not only the items.

Running the new checks over the compiler's own source found and fixed four
real defects in it (an untyped literal binding, three drivers whose `entry`
procedure returned a value without declaring a result type, a reserved word
used as a local name, a store into an immutable binding) and measured the one
remaining gap between oli-core and V0: 93 implicit narrowing conversions,
which became the specification of oli1 step 6f — implemented below.

## Implemented in genesis step 6f (2026-09-22)

`oli1` gained one expression form — a word naming an integer type followed by
`(` or `.` — and with it `T(x)`, `T.wrap(x)` and `T.bits(x)` for `u8`, `s8`,
`byte`, `u16`, `s16`, `u32`, `s32`, `u64`, `s64`, `word` and `uword`. `T(x)`
emits nothing; `.wrap` and `.bits` truncate to the width of `T` and extend
again with its signedness, which for the 64-bit slots of oli-core is one
`movzx`, `movsx`, `movsxd` or `mov eax, eax`. `.sat` and `.checked` are V0
forms oli-core does not implement, so they are rejected rather than
approximated. `genesis/3-oli1/tests/convert.oli` proves every width and both
signs by execution, the harness checks the six emitted byte sequences exactly,
and six rejection programs cover the refused forms.

The 93 narrowing sites in `compiler/` were then rewritten to say what they
actually do — `u64.bits(idx)` where a signed index is used as an offset,
`word.bits(i)` for the reverse, `u8.wrap(…)` for a digit byte, `u32.wrap(…)`
for a node field — and one was a genuine latent confusion (`digit_value` held
its `-1` sentinel in a `u64`, so it is now a `word`). `NARROW_STRICT` was
removed from `compiler/body.oli`: E0202 on a computed value is reported like
every other rule, `tests/sema/err/narrow.oli` pins it and the accepted
explicit forms to their exact positions, and `olic` still analyses all
fourteen of its own modules without a single diagnostic.

## Implemented in back-end stage 1 (2026-09-22)

Four modules and a harness layer:

- `compiler/oir.oli` builds the **linear** form of the OIR instruction set:
  one instruction per value in evaluation order, labels and branches instead
  of a block graph, string literals as statics, and — since plain arithmetic
  traps (design 0006) — a width and a trap site on every operation that can
  overflow. `--show-oir` prints it; `tests/snapshots/hello.oir`, `control.oir`
  and `values.oir` are reproduced byte for byte.
- `compiler/x64.oli` lowers it. Every value owns a frame word and every
  instruction loads its operands into `rax`/`rcx` and stores the result back:
  the register allocator's worst case, chosen because it needs no liveness
  analysis and because the allocator will replace it without changing the
  instruction selection. It emits the SysV call and syscall sequences, the
  `jo`/`jc` overflow checks, the narrow-width check that a carry flag cannot
  see, the two division checks, and `core.trap`.
- `compiler/elf.oli` writes a static non-PIE executable with one loadable
  segment. Section headers and a symbol table are not written, and the file
  says so rather than writing empty ones.
- `compiler/olic.oli` is the driver: source on stdin, a native ELF64 on
  stdout. Nothing is written unless the whole pipeline agreed on it.

What this makes true, checked by layer 5 of `genesis/test.sh`:

- **M1.** `examples/hello.oli` compiles to a 390-byte static ELF64 that prints
  `Hello Oli--`, byte-identical on a second run, with no libc and no linker.
- `tests/run/` runs: arithmetic and precedence, `if`/`elif`/`else`, `while`,
  `loop`, `break`, `continue`, procedures with six parameters, recursion, view
  parameters, module constants, characters, booleans, short-circuiting
  `and`/`or`, the explicit conversions and writing to stdout.
- `tests/run/trap/` runs: eleven programs whose arithmetic overflows, divides
  by zero, divides the most negative value by -1 or negates an unsigned value.
  Each writes `trap: <kind> at stdin:<line>` on fd 2 and exits 134, and the
  message is compared exactly. A program with no trap site carries neither the
  routine nor a message.
- `wrap(e)` clears the overflow trap of every operator inside `e` and keeps the
  width, so the result is the value modulo 2^width (`tests/run/modes.oli`).
  The two division traps stay: neither has a wrapped answer to name. `sat(e)`
  and `checked(e)` are `E0900`.

What the back end refuses with `E0900` rather than approximating: zones,
views beyond a string's `.addr`/`.len`, layouts, refs, raw memory, fallible
results, `each`, `case`, and `machine` blocks. The fixpoint `stage2 == stage3`
needs all of them, so self-hosting is not close; what is closed is the claim
that `olic` has no code generator.

## Implemented in back-end stage 2 (2026-09-22)

Two modules, two drivers and the verifier's own test:

- `compiler/cfg.oli` cuts the instruction stream into the **basic blocks** of
  OIR_SPEC §3. A terminator lives in the block record rather than in the
  stream: a block that falls through into a label needs a `jump` the stream
  never emitted, and inserting one would renumber every value after it, since
  a value *is* its instruction's index. It then links predecessors, numbers
  the reachable blocks in reverse postorder — walking the second successor
  first, so the order comes back out the way the blocks were written — and
  computes immediate dominators by the Cooper-Harvey-Kennedy fixpoint.
- The same file holds the **verifier** of §8. It checks that every value is
  defined once and dominates every use of it (1), that each block ends in
  exactly one terminator and every target exists (2), that no check was
  removed without a proof (7), and the block form's own rule: a phi stands at
  the head of a block with one operand per predecessor, in predecessor order
  (9). Invariants 3-6 and 8 concern permits, `own.move`, returned regions,
  volatile order and `never`-typed calls; no construct that can break them is
  lowered yet, so they are not asserted. It runs on every compile, before
  anything is printed or emitted, and a failure is reported as a defect in the
  compiler with exit 4 — never as a diagnostic about the program.
- `compiler/ssa.oli` is **`mem2reg`**. Every place in this subset is
  promotable, because a local is read and written only by whole-word
  `load frame.k` / `store frame.k` and no construct that takes the address of
  one is lowered yet. Phis are placed by the iterated dominance frontier of
  the blocks that write each place (Cytron et al.), so the form is built
  minimal rather than built maximal and pruned. The rebuild appends: a phi
  cannot be inserted into the head of an existing block, so the procedure is
  rewritten and its `OProc` record moved onto the new stream. Blocks no path
  reaches are not rebuilt — not as dead-code elimination, but because a block
  with no reachable predecessor has no value for a place to arrive with.
- Two forms became ordinary constructs rather than staying special cases. A
  **parameter** is now a `param k` value stored into its place, which is what
  lets `mem2reg` give it back as a value; `spill_params` is gone from the
  lowering. The result of short-circuiting **`and`/`or`** is now a
  compiler-made place written by both arms, which `mem2reg` turns into exactly
  the phi the old `copy`/`move` pair was standing in for.
- `compiler/x64.oli` lowers the blocks. A phi becomes parallel copies on the
  edges that reach it (§7), read into scratch words before any phi is written,
  so a swap between two phis of one block cannot lose a value; an edge that
  needs copies and is the taken side of a branch gets a landing pad, and every
  other branch keeps the single `jz` it had.
- `--show-oir` now prints blocks, and `--show-ssa` prints the same program
  after `mem2reg`. Six snapshots pin them:
  `tests/snapshots/{hello,control,values}.{oir,ssa}`. The harness also asserts
  what the pass must have done — no `frame.` place survives it, the loop of
  `sum_to` gets exactly the phi OIR_SPEC §5 shows, and a program with no join
  gets none.
- `compiler/verify_check.oli` is the negative test §6 asks for. There is no
  reader for the OIR text form, so the invalid OIR is made: the program on
  stdin is compiled and then broken ten ways — a block with no terminator, a
  jump to a block that does not exist, a terminator inside a block, a value
  used before it is defined, a value from a block that does not dominate the
  use, an operand naming an instruction that defines no value, a phi with one
  operand too many, phi operands out of predecessor order, a phi after an
  ordinary instruction, and a check removed with no proof. Each must be
  rejected with the invariant it breaks, and the uncorrupted build must be
  accepted.

The measurable effect on output: `examples/hello.oli` went from 390 bytes to
284, because a promoted place no longer round-trips through the frame. Every
`tests/run` and `tests/run/trap` fixture still passes unchanged, which is what
says the rebuild preserved the programs.

What this stage does **not** do: the frame still reserves the words of the
places `mem2reg` promoted, because the slot numbering is shared with the raw
form; the remaining passes of §6 (check elision, constant folding, copy
propagation, DCE) do not exist; and the register allocator of §7 does not
exist, so every value still owns a frame word.

## Implemented in back-end stage 3 (2026-09-22)

`compiler/opt.oli` and the `show_opt` driver: the passes of OIR_SPEC §6, run
to a fixpoint, because each makes work for the others — folding a branch kills
a block, killing a block makes a phi trivial, replacing a trivial phi makes
its operands constant again.

- **Constant folding.** Arithmetic, comparisons, bit operations, shifts and
  conversions over constants become constants; a branch on a constant becomes
  a jump; and an operation whose constant answer its type cannot hold becomes
  the `trap` it always took, keeping the index the operation had so the
  operands that named it still name something — nothing reads what it leaves,
  because it does not return.
- **The window, and why it exists.** oli-core compares and divides *signed*,
  so the compiler cannot answer an unsigned question about a word with its top
  bit set without getting it wrong. Folding therefore requires each operand of
  a signed operation to lie in [-2^31, 2^31) and each operand of an unsigned
  one in [0, 2^31), where signed 64-bit arithmetic is exact for both readings
  and no sum, difference or product can overflow the word it is computed in.
  Equality is the exception: both readings answer it the same way, so it is
  decided on the bits. Outside the window nothing is folded and the check the
  operation carries stays. This is a loss of precision, never of meaning.
- **Check elision.** A check leaves only with a proof term recorded (§8.7).
  Two proofs fire today: `constant`, when the operation folded to an answer
  its type holds, and `divisor`, when the divisor is a constant that is
  neither 0 nor -1. The third §6 names, dominance by an equal check, waits for
  common-subexpression elimination to make two equal checked operations exist.
  §8.7 is now enforced as an identity — the checks standing before the passes
  equal the checks standing after plus the proofs recorded — and
  `--show-oir=opt` prints each removal where the check stood, in the form §5
  shows: `; check.div_zero %5 -- removed: proof(divisor)`.
- **Copy propagation** has only phis to work on, because `mem2reg` leaves no
  copy instruction behind: a phi whose operands are all one value is that
  value. **Dead code** removes a value nothing reads and the blocks no path
  reaches; an operation still carrying a check is not dead, because removing
  it would be removing the check without a proof.

Writing the pass found three defects, two of them in code that was already
green:

- The fold window was first written as ±2^31 on the word, which accepts an
  unsigned constant with the top bit set — `18446744073709551615 + 1` folded
  to 0 instead of trapping. `tests/run/trap/add_unsigned.oli` caught it.
- `divide_checks` in `compiler/x64.oli` returned early when the zero check was
  absent, which had never happened before a pass could prove one away. That
  skipped the MIN/-1 sequence with it — including the divisor replacement `%`
  needs for the hardware's sake even where it traps on neither.
  `tests/run/trap/div_min.oli` caught it, as SIGFPE rather than a trap.
- Folding an operation to `trap` first left its uses naming an instruction
  that defines no value; the verifier caught that one before any test did.

What it is worth, measured on the fixtures (bytes of ELF, passes off → on):
`tests/run/arith.oli` 3524 → 517, `values.oli` 3435 → 796, `modes.oli`
1374 → 631, `control.oli` 2260 → 2229, `procs.oli` and `examples/hello.oli`
unchanged. The three that shrink are self-tests written as constant
expressions, so the passes decide them at compile time: `values.oli` ends as
`ret 42` with all twelve of its checks removed with proofs. The two that do
not are the ones whose work is genuinely at run time, which is the honest
shape of the result. The trap messages of the checks that went are still
written into the image; dropping a static nothing references is a separate
piece of work.

## Implemented in back-end stage 4 (2026-09-23)

Zones, views and `each`, in the OIR, the passes and the lowering:

- **OIR.** Eight instructions of §4 join the stream: `addr.of frame.k`,
  `raw.load.T` and `raw.store.T`, `check.bounds` and `check.range`,
  `zone.new`, `zone.alloc` and `zone.end`, plus `call.len`, the second
  register of the pair a view-returning call answers with. A zone is a place
  of three words (base, cursor, limit) and its handle is the address of that
  place, so passing `z` to a `zone` parameter passes the address and every
  zone operation works through it. `v[i]` is `check.bounds` on the index and
  the length, then address arithmetic and a raw access of the element's
  width; `v[a..b]` is one `check.range` for `a <= b <= len` and a new pair.
  `each x in v` is a loop over a counter the loop owns, so its element access
  carries no bounds check — the counter is below the length by construction.
  Every local now owns three frame words (an integer uses one, a view two, a
  zone three), one stride for all of them.
- **`mem2reg` respects address-taken places**, which is the rule §2.1 states
  and the zone's triple is the first place that needs it: a slot an
  `addr.of frame.k` names keeps its words and its loads and stores stay in
  the stream. The placement also became **pruned**: a phi goes only where the
  place is live into the block. That is not an optimization. Cytron's minimal
  placement put a phi for the `each` variable at the loop header, where one
  predecessor had never written it, and there is no such thing as an operand
  that is nothing; the verifier caught it as an operand of -1. Definite
  assignment is what makes the pruned form complete: a place live at a join
  was written on every path to it.
- **Check elision** gained the proof for a `check.bounds` or `check.range`
  whose every operand is a constant that satisfies it — `b[0]` on a buffer of
  known length — recorded like the others (`; check.bounds %9 -- removed:
  proof(constant)`), while the checks with a run-time index stay.
- **Lowering.** `zone.new` is one anonymous `mmap`, rounded up to a page; a
  mapping the kernel refuses traps `zone_exhausted`. `zone.alloc` rounds the
  cursor up to sixteen and traps when the request would pass the limit — or
  wrap a word, which is the same answer. `zone.end` is `munmap`. A `ret`
  inside a zone is lowered only in the entry procedure, whose `ret` is
  `exit_group`; anywhere else it would leak the mapping, so it is E0900, as
  are `break` and `continue` out of a zone, a zone inside a zone, `at ADDR`
  and `from` (the other sources of §4), and `each` over a range or an array
  place. The kernel-refusal and word-wrap cases are checks that exist
  because they can fire, not because a fixture reaches them.
- **Trap kinds** are now numbered as `core.TrapKind` declares them, so the
  five this stage names — `bounds`, `overflow`, `div_zero`, `misaligned`,
  `zone_exhausted` — carry the number a `traps` procedure will be handed.

Two things this stage found that were not about zones:

- **A parser defect.** `v[..b]` and `v[a..]` were the same node: `add_kid`
  skips an absent child, so a range with one bound had one kid and nothing
  said which bound it was. The AST printer, the semantic printer and the new
  lowering all read the first kid as the low bound, so `b[..4]` on a
  sixteen-byte buffer had length twelve. No fixture in the corpus wrote a
  range with only a high bound, which is why three printers agreed on the
  wrong answer for two days. The node now records which bounds it has
  (`sub`: 1 low, 2 high) and every reader asks `range_lo`/`range_hi`.
  `tests/run/memory.oli` check 8 pins it.
- **The compiler outgrew genesis.** `oli1` held a compiled program in a
  384 KiB output region and `olic` had reached 383 KiB of it; the first build
  with zones was refused at the line where the region ran out. The region is
  now the 512 KiB between `base+1M` and the symbol table at `base+1.5M`,
  which is every byte between them (`genesis/3-oli1/oli1.oli`, one bound;
  `genesis/3-oli1/SPEC.md` records it). `olic` is 411 KiB today. The next
  raise needs the scratch layout rearranged, not one constant; register
  allocation, which shrinks the code, is the other way to buy room.

What this stage does **not** do: `z.make`, layouts, refs and raw word access
stay E0900.

## Closed the same day (2026-09-23): the three things stage 4 left open

- **Genesis headroom, properly.** The output region of `oli1` moved out of
  the crowded first two megabytes of its scratch mapping into the unused
  upper half, `[base+8M, base+12M)`; the code alone is bounded at 2 MiB,
  which is what the code temporary at finalize can hold. A program may now
  have 2 MiB of code and 1 MiB of string data; `olic` is 415 KiB. What the
  change also surfaced: `oli1` never bounded its *input* — it reads stdin
  until EOF into a 1 MiB region and would overwrite the symbol table with a
  source over that size. `genesis/3-oli1/SPEC.md` now says so; the bound
  belongs with the next change to that file.
- **Statics nothing names are gone from the image.** `compact_statics`, the
  dead-code pass over data, runs after the last pass over code: the trap
  message of a proved-away check and the string of a removed block are
  dropped, and the survivors move down to close the gaps — by index, which
  is what an instruction names, so nothing else changes. `--show-oir=opt`
  prints `(static N removed)` for each. `tests/run/arith.oli` went from 517
  bytes to 167, `values.oli` from 796 to 553, `modes.oli` from 631 to 604.
- **`each` emits its bounds check, and the loop-bound proof removes it.** A
  check is an instruction that leaves only with a proof (§2.2, §8.7), so
  leaving it out was an omission dressed as an optimization. The proof
  `compiler/opt.oli` records is a dominance argument: `check.bounds %i,
  %len` standing below a branch on `cmp.lt %i, %len` whose true side
  dominates it — the values are the same values and a value in this form
  never changes. That is the loop of `each` and of `while i < v.len`, and
  `memory.opt` shows both removed with `proof(loop-bound)`, exactly the line
  §5 draws. The index of the loop is not consulted at all, which is what
  makes the argument short enough to trust.
- **Found while writing `docs/COMMANDS.md`, then fixed:** a call or
  procedure with more than six argument words — the seventh goes on the
  stack (ABI.md §1) — was handed `r9` for every word past the sixth,
  silently. Now every argument word carries the location it travels in,
  placed by one rule on both sides (`place_arg` in `compiler/oir.oli`): the
  six registers in order, and an argument that does not fit in the registers
  left — a view needs two — goes to the stack whole while a later integer may
  still take a register, which is the SysV classification `docs/ABI.md` §2
  names. The caller pushes right to left, pads to sixteen for an odd count
  and takes the words back after the call; the callee reads them from
  `[rbp+16]` on. `tests/run/args.oli` pins seven and nine integers, a view
  split off the register boundary, calls inside arguments and a callee that
  calls. `os.syscall` keeps its bound of seven words: a system call has no
  stack words.
- **`bin/olic`, `.vscode/tasks.json`, `tools/vscode-oli/`:** the compiler as
  a command on the PATH (a shell script that picks the driver and redirects,
  until an Oli-- program can read `argv`), tasks that call it, and a step by
  step guide to building, running and testing a program from VS Code. All
  three are conveniences outside the toolchain. `COMMANDS.md` itself was rewritten so that every construct and tool
  says, for `olic` alone, whether it **runs**, is **analysed** and refused,
  is **reserved** for V1/V2, or is **planned** — with the proof for each.

## Implemented in back-end stage 5 (2026-09-23): layouts and refs

The constructs `compiler/` itself is built on, and the ones the fixpoint
`stage2 == stage3` needed next:

- **Layouts.** The item stage had laid every layout out since G4's front end
  landed (ABI.md §3: offsets, sizes, alignment, `packed`, `align N` on a
  layout or a field); the back end now reads those tables. `r.f` is the
  ref plus the field's offset and a `raw.load` at the field's width, sign-
  or zero-extended by its type — a view field as its two words, a ref or an
  address as one, a layout held by value as the address of that part of the
  record. `r.f <- x` stores the same widths. `Name.size` and `Name.align`
  were constants already.
- **`Name.at(v)`** is the view's address once two checks hold, both real
  instructions with a site: `check.range 0, size, len` — the same condition
  a subview has — trapping `bounds`, and `check.align %addr, N` trapping
  `misaligned`, which a `packed` layout does not carry. `tests/run/trap/
  at_short.oli` and `at_misaligned.oli` pin both messages.
- **`z.make(Name)`** is `zone.alloc` with the layout's alignment when that
  is more than the zone's sixteen (`align=32` in the printed form); a fresh
  mapping is zero and a zone never hands out a byte twice, so nothing is
  cleared.
- **Refs.** `ref x` of a local is `addr.of frame.k` — the first construct
  that takes a local's address, and `mem2reg` keeps that place, as §2.1 says
  and stage 4 built for the zone triple. `ref r.f` is the address of the
  field. A ref travels as one word: parameters, results, fields, locals.
- `tests/run/layouts.oli` pins thirty checks — every width and signedness,
  a view field read through a procedure, refs as results and parameters, a
  layout inside a layout, a ref field, `packed`, `align` on a layout and on
  a field, `ref` and `rw ref` of a field — and `layouts.{oir,ssa,opt}` pin
  the form.

Two defects found on the way, one in the front end:

- **`rw ref x` was `ref x`.** The parser skipped the `rw` and typed the
  reference read-only, so a store through `rw ref pr.first` was refused
  (`E0111`). The `rw` is now recorded on the node and the reference types
  writable; `--show-ast` prints `(rw-ref-of …)`, `--show-sema`
  `(rw-ref-of …)`. No fixture in the corpus had written one, which is why
  the snapshots did not change and why the defect had lived since G4's
  front end.
- `bits` is a reserved word (`T.bits(x)`), which `olic` says and `oli1`
  does not; a local of that name in the new code passed `oli1` and failed
  the self-analysis. The rule `compiler/` must satisfy both compilers now
  has three known examples (`need`, `from`/`entry`/`at`, `bits`).

What this stage does **not** do: `be`/`le` fields, `choice`, a view whose
element is a layout, `[N]T` places, raw `[p]` access, statics, fallible
results, `case` and `machine` blocks stay E0900.

## Implemented in back-end stage 6 (2026-09-23): fallible results

`T or E` with an integer or `none` error, exactly as design 0007 and ABI.md
§2 state it: a fallible call answers with the pair (tag, payload) in
`rax:rdx` — the call's value is the tag and `call.payload` reads the second
register — and `ret v` in a fallible procedure is the pair (0, v), `fail e`
the pair (1, e), bare `fail` the pair (1, 0). `e else fail` passes the payload
on as this procedure's own failure; `e else ret [v]` leaves (in the entry
procedure, the exit status); `e else default` writes the payload and the
default into one place from the two paths, which `mem2reg` makes the phi
OIR_SPEC §5 would draw; `case … when ok [x] … when fail … end` branches on
the tag and binds the ok name to the payload, the arms in either order and
`else` for whichever is missing. `tests/run/fallible.oli` pins seventeen
checks — both channels of a default, both arm orders, propagation through
two levels, `or none` with a ref payload, `else ret` in a non-fallible
procedure and in the entry procedure — and `fallible.{oir,ssa,opt}` pin the
form.

Refused with E0900, and why: a `choice` error (`fail VARIANT`, `when fail
VARIANT`) because choices are not laid out in the back end yet; a view inside
a `T or E` because the pair does not hold it and `sret` is not lowered; a
`ret`, `fail` or `else ret` that would leave a zone in any procedure but the
entry one, because the zone would leak.

The harness runs every fixture with its `NAME.in` on stdin, or `/dev/null`,
and a twenty-second limit: `tests/run/io.oli`, added from outside this
session, reads four bytes and would otherwise wait on a terminal forever.
With `io.in`/`io.out` it is the first run fixture that imports a module
(`std.os`).

## Self-hosting reached (2026-09-23): `stage2 == stage3`

The gate of G4 (design 0022, completion gate 3): `olic`, built by `oli1`,
compiles its own source (`compiler/`, seventeen modules, 17,589 lines) into
stage 2; stage 2 compiles the same source into stage 3; the two files are the
same 829,629 bytes. `genesis/test.sh` layer 6 does this on every run, and
also compiles every run, trap and negative fixture with both stage 1 and
stage 2 and requires the same bytes and the same diagnostics. The chain from
322 hand-written bytes to a compiler that reproduces itself is now closed,
with no compiler, assembler or linker from outside the repository at any
point — which is what design 0017 asked for.

Self-compilation was the strongest test the compiler has had, and it found
five defects that no fixture had:

- **The arenas were sized for fixtures.** The first attempt died on `static
  table exhausted`; every limit in `compiler/oir.oli` and `x64.oli` is now
  sized for the compiler's own source several times over, and the drivers
  map a 4 GiB zone lazily, which costs nothing until a program needs it.
- **A field named `len` was read as a view's length.** `tb.len` on a
  `ref Tok` produced an operand of nothing — the verifier caught it as
  "an operand names a value of another procedure" — because `gen_field`
  tested the name before the type of the base. The front end had the same
  order in four places (`ety`, the place test, the semantic printer,
  `infer`), so `x.len` on a layout with a `len` field was typed `uword`
  and printed `(len …)` since G4's front end; `tests/sema/ok/regions.oli`
  has such a field and nothing had noticed. All five now ask the layout
  first. The semantic snapshots did not change: no snapshot fixture has a
  field by that name.
- **The compiler's own source relied on unsigned wraparound three times:**
  `0 - (s + 1) * 8` for a frame displacement, `0 - v` for the image of a
  negative literal, `target - (pos + 4)` for a backward rel32. Under `oli1`
  these were silent; under `olic` plain arithmetic traps on the word it is
  computed in, as the language says it must, so the self-compiled compiler
  trapped at each line in turn. Each is now written without the overflow —
  `2^32 - 8(s+1)`, `2^64 - 1 - v + 1` guarded for zero, `2^32 + target -
  (pos + 4)` — and the trap that found them was the language working, not
  the compiler failing. `oli-core` has no `wrap(e)`, so the source cannot
  say `wrap` there; `docs/COMMANDS.md` records that.

What self-hosting does **not** say: stage 2 is the same compiler with the
same limits, not a better one; every value still owns a frame word (the
self-compiled binary is 829 KiB where `oli1`'s is 436 KiB, the price of no
register allocator), and the constructs still refused — `choice`,
`machine`, statics, `[N]T`, raw `[p]`, `be`/`le` — are refused by stage 2
exactly as by stage 1. The fixpoint proves the compiler agrees with itself;
the fixture corpus is what says it agrees with the language.

## Implemented during the G2 review

The 645-byte bytes-only assembler was expanded to a 6,878-byte, hand-encoded
two-pass assembler. It now checks structure and entry selection; encodes r64
arithmetic, stack and control flow; supports base/index/scale/displacement
memory operands; resolves scoped labels and procedures; and emits read-only
data with numeric, string and address directives. The runnable example is
`examples/genesis/hello.oli`; its stdout is checked byte for byte.

The genesis harness now checks independent expected instruction/data bytes,
runtime arithmetic and memory operations, loops and procedure calls, duplicate
and missing symbols, integer bounds, all three capacity limits, EOF/CRLF/short
reads, I/O failures, deterministic rebuilds and ELF size/permissions. The
original reference suite remains unchanged and all 77 tests pass.

G2 is **not yet complete**: finish narrow operands and extension instructions,
the remaining memory instruction forms, symbol-only memory operands, writable
data/BSS with separate segments, and pub/export clauses. G3/G4, high-level
compilation, kernel, Windows and ecosystem libraries remain unimplemented.
See `genesis/2-asm/SPEC.md` for the precise accepted subset and limits and
design record 0019 for the decisions and local benchmark.
