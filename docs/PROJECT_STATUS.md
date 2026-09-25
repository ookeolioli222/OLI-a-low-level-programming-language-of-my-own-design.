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
  fifteen `tests/parse/err` fixtures and **all sixteen** `tests/sema/err`
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

## Implemented in back-end stage 7 (2026-09-23): statics and raw access

- **Static places.** A module-level `NAME : T [<- e]` is a place in a
  second, read+write segment of the image (ABI.md §7): the ones with an
  initialiser come first and their bytes are in the file (`.data`), the ones
  without follow as memory past the file (`.bss`), which the kernel zeroes.
  `addr.of data.k` reaches one; a scalar is loaded and stored at its width,
  a static array is read as the view of its bytes, so `.len`, `[i]`, `each`
  and passing it to a view parameter all work as for any view. The ELF
  writer emits the second program header only when a program has statics, so
  `hello` is still 284 bytes with one; the lowering and the writer compute
  the segment's address by one formula — the first attempt computed it two
  ways and read `.data` from the wrong page, which the fixture caught at its
  first check.
- **Raw access.** `[p]` and `[p] <- x` through an `addr T` are a raw load
  and store at the width of `T`, under `permit memory.raw` as the checker
  requires.
- `tests/run/statics.oli` pins twelve checks, `statics.{oir,ssa,opt}` the
  form, and the harness checks the program-header count of both images.

Not lowered: an array place in a frame (`name : [N]T` inside a procedure —
it needs frame slots of more than three words, which the fixed stride does
not give) and a static with an aggregate initialiser (`:= { … }`, a
constant in `.rodata`).

## Implemented in back-end stage 8 (2026-09-23): arrays in frames — M2 complete

- **The frame is laid out by size.** A local no longer owns a fixed three
  words: `gen_proc` lays the locals of each procedure out in order — an
  integer one word, a view two, a zone three, an array `[N]T` as many as its
  bytes need — and `local_off` gives each its first word; the compiler-made
  places, the values and the scratch words of the edge copies follow as
  before. `name : [N]T` in a procedure starts zero (a loop over its words,
  since a frame is not a fresh mapping) and reads as the writable view of
  its bytes, so `.len`, `[i]`, `each`, a view argument and `Name.at` all
  work as for any view; its address is taken, so `mem2reg` keeps its words.
  `tests/run/frames.oli` pins eleven checks, two procedures with arrays of
  their own among them, and `frames.{oir,ssa,opt}` the form. With it every
  construct of M2 runs.
- **Two front-end defects found by the first program with a local array.**
  `elem_of` did not know `[N]T`, so `buf[3]` was typed `[16]u8`; and a local
  array read as a value kept its place type, so `Header.at(buf)` gave a
  read-only ref. Both are fixed where they were wrong (`elem_of` reads the
  element of an array; `name_type` gives a local array the `rw view T` a
  static one already had). No snapshot fixture has a local array, which is
  why nothing had noticed.

## Implemented in back-end stage 9 (2026-09-23): the arithmetic modes, every zone source, a release on every exit edge

- **`sat(e)` and `checked(e)`, `T.sat(x)` and `T.checked(x)` run.** A 64-bit
  operation in either mode reads the flag the machine set — `ovf.of %v` in
  OIR, `seto`/`setc` right after the operation in x86-64 — and a narrower one
  compares the whole-word result with the range of its type (signed where
  the raw result can be negative, unsigned where a product of two `u32` can
  pass 2^63). `sat` clamps through two compares and two selects; `checked`
  accumulates the flags of every operator inside `e` into one place and
  fails when any was set, so `else` and `case` resolve it exactly as a
  fallible call. `tests/run/saturate.oli` pins twenty-one checks, both
  narrow and whole-word, both ends of both signednesses; the harness pins
  that a 64-bit mode reads the flag and that exactly the four trapping
  subtractions written outside any mode carry a check.
- **Every source of a zone (`spec/OLI_MEMORY_V0.md` §3) runs.** `at ADDR`
  and a zone over a buffer are `zone.new.raw %t, %size, %addr` — the triple
  laid over memory that exists, nothing released at `end`; the buffer form
  first proves with `check.range` that the buffer holds the size, so a short
  buffer is a `bounds` trap at the zone's line (`trap/zone_buffer.oli`). A
  zone `from` a parent is `zone.new.from %t, %size, %parent`: the bytes are
  carved as `zone.alloc` carves them, with the parent's `zone_exhausted`
  trap (`trap/zone_from.oli`), and `zone.end.from %t, %parent` gives the
  parent its cursor back — set to the child's base, which every later
  allocation rounds to the same place the old cursor would have rounded to.
  A zone inside a zone is whichever of these it says.
- **A zone is released on every exit edge.** The builder keeps a stack of
  the zones open in the procedure; `ret`, `fail` and the `else ret` / `else
  fail` handlers release all of them innermost first after the value is
  computed and before the procedure is left, `break` and `continue` release
  those opened inside the loop, and the block's `end` releases its own. So
  a `zone.new` has as many `zone.end` as the block has ways out, and the
  E0900 that refused `ret` in a zone of a non-entry procedure, a jump out of
  a zone and a zone inside a zone is gone. `z.try_bytes(n)` is computed in
  the stream — cursor and limit read, the cursor rounded and compared, moved
  only on the ok path — and answers `rw view u8 or none`, the first view
  payload; `else` joins its two words through two places and `case` binds
  them. `tests/run/zones.oli` pins twenty checks over all of it.
- **A defect the first zone in a non-entry procedure found.** A local of
  several words — a zone's triple, an array, the target of `ref x` — was
  addressed from its *first* frame word and written upward, into the words of
  the locals before it and the saved frame pointer. In the entry procedure
  nothing noticed: its `ret` is `exit_group`. In any other procedure the
  return crashed. `local_base` now takes the address of the last word, which
  is where the bytes of a downward-growing frame begin, and every snapshot
  with a zone, an array or a `ref` of a local changed accordingly.

## Implemented in back-end stage 10 (2026-09-23): common subexpressions and `--explain`

- **Common-subexpression elimination with the `dominance` proof.** Value
  numbering over the dominator tree (`cse_proc`, `compiler/opt.oli`):
  blocks are walked in reverse postorder and every operation whose value its
  operands alone determine — a constant, an arithmetic, bit, shift or
  compare operation, a truncation, an address of a static or a frame word —
  is looked up in a table keyed by opcode, operator, operands, width and
  signedness. An equal instruction in a dominating block computes the same
  value, so the later one's uses move to it and it goes. A check equal to a
  dominating check, or an operation whose check a dominating equal operation
  also carries, is removed with the proof §6 calls dominance by an equal
  check: on every path here the earlier one ran on the same operands and
  trapped first if this one would have. What is not merged: loads (memory
  may have changed), calls, syscalls, allocations, parameters, phis, and an
  operation whose flag `ovf.of` reads. `tests/run/cse.oli` pins four such
  removals — a second read of the same element, the same sum twice, a sum
  in the entry block reused in both arms — and keeps the four checks that
  stand first; every `.opt` snapshot changed where duplicated constants
  merged. The hash is reduced with masks, not `%`: oli-core divides signed,
  and the first program with a constant above 2^63 found the hash negative.
- **`olic --explain` and `--explain-cost` run** (`compiler/explain.oli`,
  `bin/olic --explain`). Per procedure, from the OIR after the passes and
  the frame the lowering laid out: the frame in words and its parts, the
  checks kept and the ones removed by each proof, zones opened / released /
  allocated from, calls, syscalls, operations proved to always trap; then
  every source line that produced an instruction with the count of each
  cost class of `docs/LANGUAGE_VISION.md` §7 it carries, highest first.
  Every instruction now records the statement it came from (`ins.node`),
  which is what the line report reads. `tests/snapshots/cse.explain` and
  `zones.explain` pin the report byte for byte. What it cannot say yet is
  a register assignment, because there is no register allocator.

## Implemented in back-end stage 11 (2026-09-23): `choice` as a failure

- **A choice that fits eight bytes runs.** Its value is its memory image
  (ABI.md §3) in one word: the tag — the variant's number in declaration
  order, from 0 — in the first byte, each field at the offset
  `resolve_choice` gave it. `fail VARIANT { f: e }` builds the word with a
  mask at the field's width and a shift to its offset, a bare `fail VARIANT`
  is the tag alone, and a choice-typed place or parameter is one word, so
  `fail e` from a place works too. `case … when fail VARIANT { f }` compares
  the tag byte arm by arm in the order written and reads each named field
  back — shifted down and passed through `trunc` at the field's width and
  signedness, so an `s8` comes back sign-extended — into the local the
  pattern declared; a bare `when fail` or `else` takes the rest. `else fail`
  hands the word on unchanged. A choice wider than a word would need memory
  and was `E0900`, which the harness probed until stage 19 lowered it
  (`tests/run/wide.oli`). `tests/run/choice.oli` pins
  twenty-three checks; no OIR instruction was added — the integer group's
  `and`, `shl`, `shr`, `or` and `trunc` are all it takes.
- **Two front-end defects found on the way.** A pattern local was resolved
  at the `case` statement rather than at the pattern that declared it, so
  two arms binding the same name stored into the first one's frame word and
  read the second's — the verifier caught the dangling operand. The builder
  now resolves pattern names where the front end declared them. And a
  pattern or literal naming a field the variant does not declare was
  accepted (`{ by4 }` bound an untyped local silently): `E0208` — missing,
  unknown or duplicate field in a literal or pattern — is now reported, at
  the field, once per literal (`tests/sema/err/fields.oli`, the sixteenth
  negative fixture).

## Implemented in back-end stage 12 (2026-09-23): `machine x64` blocks

- **An x86-64 encoder of the compiler's own** (`compiler/asm.oli`, design
  0009). The parser had cut every line of a block into an instruction and
  its operands; the encoder reads that tree — r64 and r32 registers, an
  immediate, `[base + index*scale + disp]`, `[static + disp]`, `.label`, a
  procedure's name — and writes the bytes: the whole subset the genesis
  assembler proves (`mov`, the ALU group, `test`, the one-operand group,
  `imul`, the shifts, `lea`, `push`/`pop`, `call`/`jmp` direct and
  indirect, every conditional branch, `ret`, `cqo`, `syscall`) plus the
  fixed opcodes a kernel needs (`hlt`, `cli`, `sti`, `nop`, `iretq`,
  `cpuid`, `rdmsr`, `wrmsr`, `rdtsc`, `pause`, `lgdt`, `lidt`, `mov` to and
  from a control register). Its output is pinned byte for byte:
  `tests/machine/{registers,memory,conditions,mov32}.oli` carry the lines
  of the genesis assembler's fixtures and their `.hex` files the bytes those
  fixtures were derived by hand from; `olic --show-asm` prints every block's
  bytes after the fixups and the harness compares — 568 bytes, all equal.
  A line the encoder does not know is `E0900` at that line, found by a dry
  run into the empty code arena while the OIR is built, so nothing is
  written.
- **The block in the stream and in the frame.** `in REG <- e` becomes
  `machine.in REG, %v` just before the block, `out REG -> place` a `%n =
  machine.out REG` just after it, and the block `machine x64`; the lowering
  loads the ins into their registers, assembles the block in place, stores
  each out's register into its value's word, and — before and after — keeps
  every callee-saved register the block names (rbx, r12–r15) in words of
  the frame reserved for it, so a caller that holds a value in rbx across
  the call gets it back. A `mov r64, name` of a procedure or a static and a
  `call`/`jmp` to a procedure are fixups the block hands to the ELF
  writer's table (two new kinds, absolute 64-bit). `tests/run/machine.oli`
  pins seven checks: `in`/`out` arithmetic, a loop over local labels, a
  callee-saved register kept across a call into a block that uses it, an
  r32 `out` zero-extended, a static read by name, and `getpid` written by
  hand. What remains `E0900` inside a block: 8/16-bit and segment
  registers, port I/O and `bytes`, which the genesis assembler rejects too.
- Building this found an oli-core rule the compiler's own source must obey:
  a constant or a layout is visible to `oli1` only after its declaration,
  so the encoder's tables and its operand record live with the arenas in
  `compiler/oir.oli`.

## Implemented in back-end stage 13 (2026-09-23): register allocation

- **Linear scan over the callee-saved registers** (`alloc_proc`,
  `compiler/x64.oli`; OIR_SPEC §7). The interval of a value runs from its
  definition to its last use — an instruction reads where it stands, a phi
  reads its operand at the end of the predecessor, an argument or a machine
  input by the end of its block, a terminator at the end of its block — and
  a value live into a loop header is extended to the end of the loop, back
  edge by back edge, until nothing changes. The scan hands out rbx, r12,
  r13, r14 and r15: five registers no call, syscall, zone operation or trap
  check the lowering emits ever touches, because all of those work in rax,
  rcx, rdx, rsi, rdi and r8–r11. When none is free the value that ends last
  gives its register up if it ends after the new one. Every value keeps
  its frame word, so giving a register back costs nothing and nothing
  changes in the OIR or its snapshots.
- **One abstraction for where a value is.** Every read and write of a
  value in the lowering — fifty-four sites — goes through `ld_val`,
  `st_val` and `push_val`, which read the location map and emit either the
  frame access or a register move; the lowering itself does not know which.
  A procedure saves the registers the scan used in words of the frame
  reserved past the machine-block save words and restores them before every
  `ret`; the entry procedure exits and saves nothing. A machine block's
  inputs and outputs stay in the frame (a swap through two `in` lines would
  otherwise clobber), and `cpuid` counts as naming rbx.
- `--explain` prints `(registers values=N saved=…)` per procedure;
  `tests/snapshots/cse.explain` and `zones.explain` pin it. The self-compiled
  compiler is what verifies the allocation: layer 6 builds `olic` with the
  allocating compiler and requires every fixture and the compiler itself to
  come out byte for byte the same.

## Implemented in back-end stage 14 (2026-09-23): the freestanding shape of M3

- **A program that says `-- target: freestanding` compiles and runs.** Its
  entry is a `-> never` procedure with `calls none`: no prologue, no frame,
  no saved registers — the machine blocks of its body are the whole
  procedure, and the OIR builder refuses anything else in it (E0900). A
  procedure with `section ".text.boot"` is lowered first, so the entry is
  the first instruction of the image. `cpu.halt()` and `cpu.pause()` are
  `cpu.halt` / `cpu.pause` in OIR and `hlt` / `pause` in the code. A
  `-> never` body that falls off its end ends in `ud2`, never in `ret`.
  Statics honour `align N` (the natural alignment of their size otherwise),
  in `.data` and in `.bss` alike. `tests/run/freestanding.oli` runs as a
  plain Linux process — it can, because the image holds nothing but the
  program's own bytes — installing its stack from a static array, carving
  a zone `at` an address and one `from` a buffer, and exiting through a
  system call written in a `machine` block.
- **Traps reach the `traps` procedure.** Every trap site in a freestanding
  program loads the kind (as `core.TrapKind`, the variant's number), the
  address and length of its module's file static and the line, and calls a
  routine that aligns the stack, pushes the three words of the `core.Site`
  (SysV MEMORY class, 24 bytes) and calls the procedure declared with
  `traps`; without one a trap is `ud2`. On the callee's side a layout
  parameter wider than sixteen bytes is now lowered: its bytes lie above
  the return address and the parameter's word is their address, the same
  form a by-value field takes, so `site.line` reads straight through it. A
  layout of sixteen bytes or less as a parameter, and a layout argument on
  the caller's side, stay E0900. `tests/freestanding/trap_line.oli` exits
  with the line of its overflow, delivered this way; the harness runs every
  program in that directory and requires the status its `-- expect: exit`
  line names and no output at all.
- **Two things the first freestanding trap found.** The jump over a trap
  site skipped a fixed twenty-five bytes — the hosted message-and-call
  sequence — where the freestanding one is forty-five, so the fall-through
  path landed inside the site; the length now follows the mode. And the
  file statics the sites name were dropped by the dead-static pass, which
  only sees instructions; a live site now keeps its file.
- Not implemented (FREESTANDING.md §2, §8): the target profile — load
  address, code model, section order beyond `.text.boot` — the Multiboot
  header, port I/O, `mem.mmio`, `own`. The image is the same two-segment
  ELF64 at `0x400000` as a hosted program's.

## Implemented in back-end stage 15 (2026-09-23): M3 closed — load address, read-only and aggregate statics, the kernel image

- **`-- load: ADDR`** near the top of a program is the load address of the
  image (FREESTANDING.md §2's `load_address`, read from the source since
  `olic` reads only stdin); default `0x400000`. Every static is named
  through a 64-bit immediate, so the address can be anything: the fixups
  for read-only and writable statics now patch all eight bytes of the
  `movabs` they stand in. A `[static + disp]` operand of a `machine` block
  is a sign-extended disp32 and the encoder refuses it unless the load
  address is in the low or the top 2 GiB (two fixup kinds of their own).
- **Statics with `section ".text…"` or `".rodata…"`** go in the read-only
  segment, in front of the code, at their alignment — where a boot loader's
  header must be — and are kept whether or not the program names them.
  They are read through `addr.of static`; a store to one is E0900.
- **Aggregate initialisers of statics** run: `{ a, b, … }` for an array,
  element by element at the element's width (the rest zero), and `Name {
  f: c, … }` for a layout, field by field at its offset and width; every
  value a constant expression. A layout held by value as a static reads as
  its address, as a by-value field does, so `pair.a` and `pair.a <- 6`
  work. `tests/run/aggregates.oli` pins ten checks.
- **Design 0023: the narrow forms a block may use** — port I/O through
  `al`/`ax`/`eax` and `dx` or an `imm8`, `mov SREG, ax`, `mov ax, SREG`,
  `mov ax, imm16`, `retfq` — exactly those, encoded from the tokens; every
  other 8/16-bit form stays E0900.
- **`examples/kernel.oli`**, the M3 target program of FREESTANDING.md §8:
  a Multiboot2 header as a static layout with an initialiser in
  `.text.boot`, its own entry and stack, COM1 through `out dx, al`, the VGA
  text buffer through raw stores, `cpuid` through a block, `hlt` forever.
  There is no emulator on this machine, so the harness checks the image
  structurally — an ELF64 loaded at `0x100000`, the entry inside it, the
  header's magic, length, checksum and end tag at file offset 176, `ee`
  and `0f a2` in the blocks, `syscalls=0` in every procedure's `--explain`
  — and `qemu-system-x86_64 -kernel kernel.elf -serial stdio` is the
  documented way to run it. M3 is closed with that: what FREESTANDING.md
  still lists as planned is the profile *file*, the section order beyond
  `.text.boot`, `mem.mmio` and `own`.

## Implemented in back-end stage 16 (2026-09-23): layouts by value, `case` over values, `each` over a range

- **Layouts held by value run everywhere the language puts them.** A local
  `p : Name` owns as many frame words as its bytes and reads as their
  address, exactly as a by-value field does; `Name { f: e, … }` in an
  expression is a fresh area of the frame, zeroed and written field by
  field; `q : Name <- p`, `q <- p` and `r.f <- p` copy the bytes exactly —
  words, then a four, a two and a one, so nothing past the object is
  touched (the COPY class). A parameter or result of sixteen bytes or less
  travels as its eightbytes in registers (SysV INTEGER class: `arg` per
  word, `call.word1` for the second word back), a wider parameter on the
  stack (MEMORY class), copied into the frame on entry so the local reads
  like any other; a wider result goes through `sret` — the caller owns an
  area of its frame and passes the address as a hidden first argument, the
  callee copies the bytes there and hands the address back in rax. A view
  or an array of layouts reaches an element by its address, so `v[i].f`,
  `v[i] <- p` (a copy) and `each e in v` (a copy per element) all run.
- **`case` over a `bool`, an integer or a plain `choice`**: the arms in the
  order written, each a compare and a jump past it — `true`/`false` and an
  integer against the value, a variant against the tag byte of the image
  with its fields bound — `else` takes the rest. The front end had never
  declared the fields a bare variant pattern binds (only those under
  `fail`), so `when line { len }` left `len` unknown; `collect_pattern`
  and the definite-assignment binder now handle both. No semantic snapshot
  changed: the corpus has no such `case`.
- **`each i in a..b`** counts from `a` while below `b` (both `uword`), the
  counter a place that becomes the phi at the head.
- `tests/run/records.oli` pins twenty-three checks over all of it.

## Implemented in back-end stage 17 (2026-09-23): M4 begins — `calls interrupt`

- **Interrupt handlers run.** A procedure with `calls interrupt` (which
  needs `permit cpu.interrupt`; the front end used to refuse it as V1) gets
  a prologue that pushes every general register but rsp and rbp before the
  ordinary frame, its one parameter — a `ref core.x64.InterruptFrame` — is
  the address of what the CPU pushed (rip, cs, rflags, rsp, ss), above the
  saved rbp and the fourteen registers, and every `ret` restores the
  allocated registers, drops the frame, pops the fourteen and ends in
  `iretq`. It answers nothing. `lib/core/x64.oli` holds the frame, the
  IDT gate and the `lidt`/`lgdt` pointer layouts.
- **Verified by execution, in a hosted program.** `tests/run/interrupt.oli`
  pushes in a machine block exactly the frame the CPU would push — ss, the
  rsp of before, rflags (`pushfq`), cs, the address of a local label — and
  jumps to the handler; the handler reads `frame.rip` and `frame.cs` into
  statics, and its `iretq` comes back to the label with rbx as it was.
  Three checks, exit 42. The encoder gained `pushfq`, `popfq`, `int n`,
  `int3` and `lea r64, [.label]` (RIP-relative), and a memory operand
  without a size word now carries its bracket's token, so a refused operand
  is reported on its own line rather than at 1:1.
- **`examples/kernel.oli`** builds a 256-entry IDT (`core.x64.IdtGate`),
  installs a `calls interrupt` handler on vector 3 with `lidt`, raises
  `int 3` and prints the interrupted rip on COM1 from the handler; the
  harness checks the `cd 03`, the `lidt` and the one `iretq` in the image.
  `tests/sema/err/not_implemented.oli` now expects `E0401` where `calls
  interrupt` lacks its permit, not `E0900`.

## Implemented in back-end stage 18 (2026-09-23): byte order, `mem.get/put`, the reference program runs

- **`be T` / `le T` fields** (ABI.md §3): a `be` field is loaded at its
  width and byte-swapped — shifts, masks and ors of the integer group, no
  `bswap` instruction yet — then re-extended when signed; stored swapped;
  a `le` field is the machine's own order. **`mem.get_u16/u32/u64`,
  `get_be*`, `get_le*`, `put_*`** over a view: `check.range` that the view
  holds the width (`bounds` when short), one load or store, the swap for
  `be`. `tests/run/bytes.oli` pins nineteen checks by looking at the bytes.
- **A layout as the failure of a fallible result** (`fail os.Error { code:
  n }`): its bytes as the payload word when it is eight bytes or less; a
  qualified literal head (`os.Error { … }`) resolves to the item. A wider
  layout failure stays E0900.
- **`examples/packet_demo.oli`, the reference program of the language
  documents, compiles and runs** under `olic`: a zone, a packet built with
  `mem.put_*`, `Header.at` over a view with a `be u16` length, a `choice`
  failure taken apart with `case`, `each` over the payload, `os.syscall`
  writes through a fallible procedure with `else ret`, `cpuid` through a
  `machine` block with three outputs, a raw address peek. The harness runs
  it and checks its exit status and output — the vendor string is the
  machine's own `cpuid`.

## Implemented in back-end stage 19 (2026-09-23/24): fallible results in the caller's area, wide choices as values

- **A `T or E` the register pair cannot carry** — a view, a layout or a
  choice wider than a word as `T`; a `choice`, a layout wider than a word or
  a view as `E` — travels the way a wide layout result does (ABI.md §2):
  the caller owns an area of its frame, one word for the tag and as many as
  the wider payload needs, and passes its address as a hidden first
  argument; `ret v` writes tag 0 and the value's words (a view's two, an
  aggregate's bytes), `fail e` tag 1 and the failure's word, two words or
  bytes, and the address comes back in rax. The caller reads the tag from
  the area, a view's two words or a payload word after it, and for a layout
  or a wide choice keeps the address of its bytes, which the binding copies:
  `case … when fail VARIANT { f }` reads the tag byte and the fields from
  memory, `else fail` copies the bytes into the procedure's own area. A
  layout of one word as `T` still travels in the pair, as its word.
- **A `choice` wider than a word is a value like a layout by value**: its
  image in memory — the tag in the first byte, each field at its offset —
  reached by its address (`detail { code: x, extra: y }`, `nothing`), copied
  into a local, a parameter (the pair up to sixteen bytes, the stack beyond,
  as for a layout) or a result (`sret`), and matched by `case` from the tag
  byte with each field loaded at its offset, width and sign.
- **Found on the way, and closed.** A layout or a wide choice as the `T` of
  a `T or E`, and a wide choice as a plain value, compiled without a
  diagnostic to the *address* of an area in the callee's frame — a dangling
  value that happened to read right. A view as `E` kept its address and
  lost its length. An aggregate constant (`TABLE : [4]u8 := { … }`) read as
  a zero-length view, so `TABLE[2]` trapped `bounds`. The first three are
  lowered above; the constant is `E0900` (it would live in `.rodata`), and
  the harness probes it in place of the wide choice it probed before.
  `physaddr(n)` and `addr T (n)`, listed as analysed, run as the integer
  they are; the tables say so now.
- `tests/run/wide.oli` pins forty-one checks: `find` answering a view or a
  wide failure, `first_word` answering `view u8 or none`, `relay` passing a
  wide failure on with `else fail`, `mk_pair`/`relay_pair` (a layout of two
  words as the value), `mk_small`/`mk_sm` (a layout of one word, with `none`
  and a wide choice as the failure), `mk_big` (a wide choice as the value),
  `fp`/`relay_fp` (a layout as the failure), `f_view`/`g_view` (a view as
  the failure), `mk_choice`/`sum_of` (a wide choice as a result, a parameter
  and the subject of `case`), and every arm of `case` over them. Nothing in
  the `T or E` family is `E0900` now.

## Implemented in back-end stage 20 (2026-09-24): `mem.mmio`, views of device memory

- **`mem.mmio(T, a, n)`** — `T` one of `u8`…`u64`, `a` and `n` `uword`, under
  `permit memory.mmio` (`E0401` without it) — is the view `(a, n)` of type
  `mmio rw view T`. No instruction is emitted for the call itself: what
  makes the view `mmio` is every element access through it. `v[i]` and
  `v[i] <- x` are `raw.load.T.mmio` / `raw.store.T.mmio` — the OIR's memory
  space on the instruction (OIR_SPEC §1.4), printed as a suffix — after the
  bounds check any view carries; the dead-code pass keeps a volatile load
  whatever reads it, common-subexpression elimination never merged loads,
  and nothing reorders them; `--explain` bills each as `KERNEL`
  (MACHINE_MODEL.md §3). `v[a..b]`, `.len`, `.addr` and passing the view to
  a parameter typed `mmio view T` / `mmio rw view T` work as for any view.
- **`mmio` is never dropped.** The classifications read a type without its
  `mmio` (`unqual`, `elem_of`), but a conversion between an `mmio` view and
  a plain one is a type mismatch (`E0200`) in either direction, so a plain
  view cannot become device memory except through `mem.mmio`, and a
  procedure that takes device memory says so in its signature. `mem.get_*`
  / `mem.put_*` take plain views; `each` and `Name.at` over an `mmio` view
  are `E0900` — a ref would drop the mark, a loop would read plain memory.
- `tests/run/mmio.oli` pins eleven checks over an array in the frame taken
  as device memory: stores and loads through `u16` and `u8` views of the
  same bytes, a subview, the view passed as `mmio rw view u16` and as
  `mmio view u16`, and an unused load that the passes keep;
  `tests/run/trap/mmio_bounds.oli` traps `bounds`. The harness pins the
  five loads and four stores of the fixture in `--show-oir=opt`, the nine
  `KERNEL` lines of `--explain`, and the two volatile stores of
  `examples/kernel.oli`, whose `vga_write` now goes through `mem.mmio`
  instead of raw stores. `tests/sema/err/not_implemented.oli` reports
  `E0401` for `mem.mmio` without the capability where it reported `E0900`.

## Implemented in back-end stage 21 (2026-09-24): port places, `cpu.interrupts`, the PIC and the PIT

- **`port.u8[n]`, `port.u16[n]`, `port.u32[n]`** (design 0015) are places:
  read, one `in` at the width with the value zero-extended to the canonical
  image; written with `<-`, one `out`; the port number is a `u16` (what `dx`
  holds). The parser tells `port.` from the `port T` type by the dot, the
  checker asks for `permit io.port` (`E0401`), and the OIR has two new
  instructions, `hw.load port.uN %p` and `hw.store port.uN %p, %v` —
  volatile: the dead-code pass keeps a load whatever reads it, nothing
  merges or moves them, `--explain` bills them `KERNEL` (as it now does
  `cpu.halt`, `cli` and `sti`).
- **`cpu.interrupts(on)` / `cpu.interrupts(off)`** are `sti` / `cli` under
  `permit cpu.interrupt`; any other argument is `E0900`.
- `tests/run/hw.oli` compiles every form and runs to 42 with the port code
  behind a byte that never arrives on stdin, since a process may touch
  neither a port nor the interrupt flag; the harness disassembles the image
  with binutils and finds each of `out %al/%ax/%eax`, `in %al/%ax/%eax`,
  `cli`, `sti` and the zero-extension exactly once, and the six port
  accesses in `--show-oir=opt`. `hw.{oir,ssa,opt}` pin the form.
- **`examples/kernel.oli` programs its hardware from Oli--**: `outb` is a
  port place; `pic_init` remaps the two 8259s to vectors 32..47 and masks
  every line but the timer's; `pit_init(100)` sets channel 0 of the 8253;
  `on_timer` (`calls interrupt`, vector 32) counts `ticks` and writes the
  end of interrupt; `main` enables interrupts and waits a second in `hlt`
  before its final `hlt` loop. The harness finds fifteen `out dx, al`, one
  `sti` and two `iretq` in the image, fifteen `hw.store port.u8` in its
  OIR, and the machine blocks that remain (`cpuid`, `lidt`, `int 3`, the
  handler addresses) as before. No QEMU on this machine: the image is
  checked structurally.

## Implemented in back-end stage 22 (2026-09-24): control registers, `core.x64.paging`, calls into libraries

- **`arch.x64.cr0`, `cr2`, `cr3`, `cr4`, `cr8`** are places under
  `permit cpu.control` (design 0015): read, one `mov rax, crN` (`hw.load
  arch.x64.crN`); written with `<-`, one `mov crN, rax` (`hw.store
  arch.x64.crN, %v`); cr3 is typed `physaddr` (design 0012), the others
  `u64`. The parser recognises the five-token shape `arch . x64 . crN` and
  leaves any other `arch` an ordinary name. Volatile like every hardware
  place: kept by the passes, `KERNEL` in `--explain`.
- **A library's procedures can be called.** `mod.proc(...)` was typed by
  the front end but refused by the back end (`gen_call` took only a bare
  name); it now takes the member name the front end resolved, so
  `lib/core/mem.oli` and the new `lib/core/x64/paging.oli` are usable —
  the first executed cross-module calls. Names stay program-wide: a
  parameter of a library procedure may not shadow a static of the program
  (`E0101`), which the kernel example met (`pd`) and renamed around.
- **`core.x64.paging`** (`lib/core/x64/paging.oli`): the entry flags as
  `pub` constants, `make_entry`, `entry_target`, `entry_present`, the four
  indices and two offsets of a virtual address, and `identity_2m`, which
  fills a page directory with two-megabyte pages. `tests/run/paging.oli`
  runs fourteen checks against it, including the identity map of a
  gigabyte built into a frame array.
- **`examples/kernel.oli`** carries three 4096-aligned tables in `.bss`,
  fills them (`paging_init`), installs them with
  `arch.x64.cr3 <- physaddr(u64(pml4.addr))` and reads cr3 back; the
  harness finds one `mov cr3, rax`, one `mov rax, cr3`, the `hw.store` in
  the OIR and the call into `core.x64.paging`. `tests/run/hw.oli` pins all
  seven control-register accesses once each by objdump.

## Implemented in back-end stage 23 (2026-09-24): symbol table and section headers

- **The image is readable by the tools.** After the loaded segments
  `compiler/elf.oli` writes `.symtab` and `.strtab` exactly as ABI.md §4
  lays them out — every procedure as `module.name` (`STB_LOCAL` unless
  `pub`, `STT_FUNC`, with its size from the new `codeend` of its `OProc`),
  an `export` clause as a second global symbol (the bare name or the string
  given), every static as `module.name` (`STT_OBJECT`, its size) in
  `.data`, `.bss` or `.rodata`, the trap routine as `olic.trap` — then
  `.shstrtab` and eight section headers: null, `.rodata`, `.text`, `.data`,
  `.bss`, `.symtab`, `.strtab`, `.shstrtab`. Nothing loaded changes: the
  segments, the entry, the Multiboot2 header offset and the self-hosting
  fixpoint are as before, with the tables appended.
- The harness reads `hello.elf` with `nm` (`hello.start`, local, at the
  entry), `readelf -S` (`.text` at the code) and `objdump -d`
  (`<hello.start>:`), finds `statics.buf` in `.bss` and `olic.trap`, the
  Multiboot2 header of the kernel as `kernel.header` in `.rodata` at
  `0x1000b0` and the `pub` procedures of `core.x64.paging` as global
  symbols, and requires `readelf -a` to raise no warning. The two size
  pins that guarded against emitted trap messages now measure the code
  segment rather than the file.

## Implemented in back-end stage 24 (2026-09-24): object files and `extern`

- **`-- output: object`** makes `olic` write a relocatable ELF (`ET_REL`)
  instead of an executable: `.rodata`, `.text`, `.data`, `.bss` at address
  0 each, no program headers, the symbol table of stage 23 with the four
  section symbols in front and `_start` for an entry procedure, and
  `.rela.text` built from the same fixes the executable would have patched
  — `R_X86_64_64` for an absolute address of a static or a procedure
  (`R_X86_64_32S` for a disp32 inside a machine block), against the section
  it names, and `R_X86_64_PLT32` (addend −4) for a call to an `extern`
  procedure. `apply_fixes` leaves those fields zero in object mode. No entry
  procedure is required.
- **`extern proc f(params) [-> T]`** is a signature the linker resolves:
  parsed without clauses, body or `end`; an `OProc` with no instructions,
  so procedure and `OProc` indices stay one; an undefined global symbol
  under its bare name; `al` cleared before every call to it (a variadic C
  callee's vector count). Allowed only with `-- output: object`: `E0900`
  in an executable, which has no linker to resolve it.
- **Both directions of the SysV ABI, executed.** `tests/c/oli_side.oli`
  exports `oli_add`, which writes a line through libc's `write` and answers
  through C's `c_double`; `tests/c/c_side.c` calls it from `main`. The
  harness compiles the object, checks `ET_REL`, `.rela.text`, the two
  undefined symbols, the two `PLT32` and two `.rodata` relocations and
  `readelf -a`, links with `cc -no-pie`, runs, and compares the output with
  `tests/c/expected.out`; `main_side.oli` + `lib_side.oli` link with `ld`
  alone, no C runtime, and exit 42. Cautions in ABI.md §2: C-facing
  parameters should be 64-bit types, and a view is two words.

## Implemented in back-end stage 25 (2026-09-24): atomics and fences

- **`atomic.load/store/add/sub/and/or/xor/exchange/cas(ref, …, order)`**
  over a `ref T` / `rw ref T` to an integer of any width (`rw` for
  anything but `load`, `E0111` otherwise; a known operation, its arity and
  one of the five orders as a bare name, `E0900` otherwise). One OIR
  instruction, `atomic.OP.W %addr, %v[, %desired] ORDER`, never removed or
  merged, class ATOMIC in `--explain`. On x86-64: a load is one move with
  a signed value re-extended; a store one move, or `xchg` for `seq_cst`;
  `add`/`sub` `lock xadd` (the old value back), `exchange` `xchg`;
  `and`/`or`/`xor` a `lock cmpxchg` loop; `cas` `lock cmpxchg` with
  `sete`, answering `bool`. Every read-modify-write is a full barrier and
  wraps at its width. **`cpu.fence(order)`** is `lfence`, `sfence` or
  `mfence`, nothing for `relaxed`.
- The parser lets `atomic.` start an expression or a statement although
  `atomic` is a reserved word, and `ref`/`rw ref` of a static — refused
  with `E0900` until now — is the static's address in `.data`, `.bss` or
  `.rodata`.
- `tests/run/atomic.oli` runs twenty-seven checks over u64, u8, s16 and
  u32 places and a static; the harness counts each instruction form in
  the disassembly, the eighteen atomics and four fences after the passes,
  and the twenty-two ATOMIC lines of `--explain`. Threads are not a
  construct: atomics are what a kernel, or a program starting threads
  through `os.syscall`, needs to be correct.

## Implemented in back-end stage 26 (2026-09-24): `mem.copy`, `mem.set`, `mem.zero`, `mem.secure_zero`

- **`mem.copy(dst, src, n)`** — `dst` a `rw view u8` (`E0111` otherwise),
  `src` a `view u8`, `n` a `uword` — checks `n` against both views
  (`check.range`, the `bounds` trap) and is one OIR instruction, `mem.copy
  %dst, %src, %n`, lowered to `rep movsb`: forwards when the destination
  does not lie above the source, else from the last byte down with the
  direction flag set and cleared, so an overlap copies right.
  **`mem.set(dst, b)`**, **`mem.zero(dst)`** and **`mem.secure_zero(dst)`**
  fill the whole view with `rep stosb`; none of the four is ever removed by
  a pass (`secure_zero` needs exactly that), and `--explain` bills them
  COPY. The `.sema` form types the arguments as the intrinsics expect.
- `tests/run/memops.oli` runs sixteen checks — a straight copy, an overlap
  forwards and backwards, a copy of nothing, a fill, a zero and a secure
  zero; `tests/run/trap/memcopy.oli` traps `bounds` on a count the
  destination cannot hold. The harness counts the `rep movsb`, `rep stosb`,
  `std` and `cld` of the image and the eight operations after the passes.
  Nothing of `mem.*` is `E0900` now.

## Implemented in back-end stage 27 (2026-09-24): `mmio ref T` and the `port T` type

- **`Name.at(v)` over an `mmio` view** answers `mmio rw ref Name` (or
  `mmio ref Name`): the mark stays, `layout_of` and the field typing read
  through it, and every field load or store through such a ref is
  `raw.load/store.T.mmio` — volatile, KERNEL — including a `be`/`le`
  field's raw load and a view field's two words. A record held by value
  inside device memory (`r.sub` where `sub` is a layout) is `E0900`: read
  as plain memory it would lose the mark.
- **`port T` as a value**: `port u8 (0x3F8)` converts a sixteen-bit port
  number into a `port u8`, a constant `COM1 : port u8 := 0x3F8`, a local or
  a parameter holds one, and `p.in()` / `p.out(v)` under `permit io.port`
  are the same `hw.load port.uN %p` / `hw.store port.uN %p, %v` as the
  place form — one `in`/`out` at the width of `T`. Literals meeting a
  `port T` context settle as `u16`.
- `tests/run/mmio.oli` (fourteen checks now) writes and reads a `Reg`
  layout through `Reg.at` over the byte view of the frame array and keeps
  an unused volatile field load; `tests/run/hw.oli` drives a `port u8`, a
  `port u16` and the constant `COM1` (behind the byte that never arrives),
  and the harness counts every `in`/`out` form (three `out dx, al`, two of
  the rest each), the eighteen hardware operations after the passes and
  the twelve KERNEL lines of `mmio.oli`. `tests/sema/err/not_implemented.oli`
  no longer expects `E0900` for `port u8 (…)`, and
  `tests/parse/ok/kernel_sketch.oli` — the reference kernel sketch of the
  documents — reports only its intended `E0401` and `E0200`: nothing in it
  is `E0900` any more.

## Implemented in back-end stage 28 (2026-09-24): the target profile and named sections

- **`-- profile: PATH`** names the target profile of FREESTANDING.md §2,
  a file of `key = value` lines read like a module (relative to the
  working directory; a missing file stops the compilation with its name).
  `load_address` overrides `-- load:`, `align_sections` sets the boundary
  every code section starts on (padded with `int3`), and `sections`
  orders the groups; the other keys are read for what they document.
- **Named sections are placed.** Every distinct `section "name"` on a
  procedure is a code group (`.text` for those without a clause,
  `.text.trap` for the trap routine, last), every distinct name on a zero
  static a group of the writable segment (`.bss`, `.bss.boot`, …); the
  order is the profile's list first, then first seen — without a profile
  `.text.boot`, `.text`, the rest, exactly the layout of stage 14. Each
  group has its own ELF section header with its address and size, so
  `readelf -S` shows the layout and `objdump -t` places every symbol in
  its section. The read-only data stays in front of the code (a boot
  loader's header must be there) and `.data` on the page after it.
- `tests/freestanding/profile.oli` with `x86_64-profile.oli-target`:
  loaded at 0x500000, `.text.boot`, `.text.init`, `.text`, `.text.trap`
  each on a 64-byte boundary in that order, `.bss.stack` before `.bss`,
  the program exits 33 through what `.text.init` computed; the harness
  reads the section list, the addresses, the load address and the symbol
  placement, and requires `readelf -a` to raise nothing.
  `examples/kernel.oli` now carries `examples/x86_64-kernel.oli-target`
  instead of `-- load:`, and its image lists `.text.boot .text .text.trap
  .data .bss.boot .bss`.

## Implemented in back-end stage 29 (2026-09-24): DWARF debug information

- **Line tables.** While lowering, `note_line` records where in the code
  a new source line begins (position, line, module), and `note_proc_line`
  a row for every procedure's first byte at the line of its declaration,
  so a debugger finds the procedure's start. `compiler/elf.oli` writes
  them as a DWARF 4 `.debug_line`: one sequence over the code, the files
  named after the modules (`core.x64.paging` is `core/x64/paging.oli`, the
  root module `kernel.oli`), standard opcodes only.
- **A compile unit and its subprograms.** `.debug_info` with
  `.debug_abbrev`: one `DW_TAG_compile_unit` (producer `olic`, language
  `0x8000` — the user-defined range, since DWARF has no code for Oli-- —
  the root module's name, the code range, the line table) and one
  `DW_TAG_subprogram` per procedure with its `module.name` and range.
  Three non-loaded sections after `.shstrtab`; nothing loaded changes.
  An object file carries no DWARF yet (its addresses would need
  relocations).
- The harness decodes `hello.elf`'s line table with `objdump --dwarf`,
  resolves the entry with `addr2line` to the line of `proc start`, asks
  `gdb` for `info line kernel.main` and expects the line of `proc main`,
  finds `core/x64/paging.oli` among the kernel's files, and requires
  `readelf --debug-dump=line` to raise nothing. `ins_line` moved from
  `compiler/explain.oli` into `compiler/oir.oli`, where `olic` has it.

## Implemented in back-end stage 30 (2026-09-24): the hardware commands of design 0015

- **Places**: `cpu.stack` and `cpu.frame` (`addr u8`; `mov rax, rsp/rbp`,
  `mov rsp/rbp, rax`), `arch.x64.msr[n]` (`u64` indexed by a `u32`;
  `rdmsr`/`wrmsr`), `arch.x64.gdt` and `arch.x64.idt` (written with a
  `ref core.x64.TablePointer`; `lgdt`/`lidt [rax]`; a read is `E0900`, there
  being no `sgdt` in the plan), `arch.x64.tr` (`u16`; `str`/`ltr`). The
  parser recognises `cpu.stack`/`cpu.frame` and `arch.x64.NAME[...]` as
  places (`N_HWPLACE` with a kind), the checker asks for `cpu.control` or
  `cpu.msr`. **Commands**: `cpu.id(leaf)` — `cpuid` with `ecx` zero, rbx
  saved around it, the four registers into a fresh area that is a
  `core.CpuId` by value; `cpu.tsc()` — `rdtsc` as one word; `cpu.call(p)`
  — a bare `call` to the procedure named; `cpu.jump(a)` — `jmp rax`. Six
  OIR instructions (`hw.load`, `hw.store`, `hw.cmd`), volatile, KERNEL.
- **A `calls none` body may hold hardware statements** — those places
  written, `cpu.call`, `cpu.jump`, `cpu.halt`, `cpu.pause`,
  `cpu.interrupts`, `loop`s of them, machine blocks without `in`/`out` —
  and its values are allocated to the callee-saved registers alone: a
  value that would need a frame slot is `E0900`, since there is no frame.
  `examples/kernel.oli` now starts exactly as design 0015 proposed
  (`cpu.stack <- …`, `cpu.frame <- …`, `cpu.call(main)`, `cpu.halt()`),
  loads its IDT with `arch.x64.idt <- ref idtr` and reads its vendor with
  `cpu.id(0)`; three machine blocks remain (`int 3` and the two handler
  addresses).
- `tests/run/hw.oli` runs `cpu.id(0)`, `cpu.tsc()` twice, reads the stack
  and frame pointers and calls a procedure through `cpu.call`, and holds
  the privileged forms behind the byte that never arrives; the harness
  counts every instruction form and the thirty-five hardware operations
  after the passes, and finds `cpuid`, `lidt` and the `call` to `main`
  in the kernel's code and the five hardware statements in its OIR.

## Implemented in front-end stage 31 (2026-09-24): conditional compilation

- **`when target.FACT … [else …] end` at declaration level.** The facts —
  `freestanding`, `hosted`, `object` (with `not`), `os == "…"`, `arch ==
  "…"` (with `!=`) — reach the parser from the pragmas and the profile
  (`target.os` is the profile's `os`, else `none` freestanding and `linux`
  hosted; `target.arch` the profile's `arch`, else `x86_64`), and the
  parser settles the condition itself. Both branches are parsed into
  `N_WHENDECL` nodes marked taken or dropped; item collection and the
  import loader descend into a taken branch only, so a dropped branch
  declares nothing and is never checked — a hosted program may carry a
  freestanding half that would not type-check hosted, and vice versa.
  `--show-ast` prints each branch with `taken` or `dropped`.
- `tests/run/when.oli` (hosted) and `tests/freestanding/when_free.oli`
  (freestanding, exiting 5 through a frameless entry) select opposite
  branches of the same shape; the harness pins the AST (`when.ast`), the
  six procedures of the hosted program's `.sema` form, the absence of the
  dropped `cpu.halt`, and both runs.

## Implemented in stage 32 (2026-09-24): field constants, `arch.x64.segments`, `bytes`

- **`Name.field.offset` and `Name.field.size`** (MACHINE_MODEL.md §3) are
  compile-time constants beside `Name.size` and `Name.align`, for a field
  of a layout or of a choice variant: the typer gives them the literal's
  type, the `.sema` form prints `(int N)`, the constant folder reads the
  field record (which now carries the size of its type beside its offset)
  and the OIR never loads a field for them. A field the item does not
  have is the error it was.
- **`arch.x64.segments(code, data)`** (design 0015) is a command on the
  hardware place, under `cpu.control`: `hw.cmd arch.x64.segments %code,
  %data` in the OIR, class KERNEL, never removed; lowered as the data
  selector into rcx, the code selector pushed, `lea rax, [rip + 3]`,
  `push rax`, `retfq` — the far return lands on the next instruction with
  cs reloaded — then `mov ds|es|ss|fs|gs, cx`. Exactly two arguments,
  typed `u16`, named or positional; anything else is E0900
  (`tests/sema/err/not_implemented.oli`).
- **`bytes b, b, …`** in a `machine x64` block (design 0023): the listed
  bytes as they stand, each 0..255; a line with none, a negative or a
  wider value is E0900 (the harness compiles one).
- `tests/run/segments.oli` runs all three: the constants of a layout and a
  choice checked against the ABI's offsets, the segment reload executed
  under the selectors a Linux process already runs with (cs 0x33, data
  0x2B, read back through `mov ax, cs|ds|ss` of design 0023), and a
  `bytes` line spelling `mov eax, 42`. The harness pins the eleven
  instructions by objdump, the `hw.cmd` kept through the passes, the
  absence of any field load in the OIR, the `u16` typing of the selectors
  in the `.sema` form and the KERNEL billing of `--explain`. Not runnable
  here: the sequence with a kernel's own GDT (no QEMU); what runs is the
  same bytes under the selectors of a process.

## Implemented in front-end stage 33 (2026-09-24): `own T` and `<~`

- **Linear values** (design 0002, MEMORY_MODEL.md §8). `own T` holds a
  handle — `T` an integer, `physaddr` or `addr T` (anything else is
  E0900) — made by `own T (e)` and unwrapped by the conversion `T(h)`;
  both are the same bits, no instruction. A local of `own` type is *full*
  when it holds a value and *empty* once the value moved out: every read
  consumes it (E0340 when it is empty), `<~` fills an `own` place (E0341
  for any other place, E0342 when it is still full, and an `own` place
  takes `<~`, never `<-`: E0343), a value still held where its scope ends,
  at `ret`/`fail`, or at `break`/`continue` for a local of the loop's
  body was never consumed (E0344), and the branches of an `if` or `case`
  — and every pass of a `while`/`each`/`loop` body — must leave each
  `own` local in the same state (E0345). The states are kept on a stack
  of cells (`Prog.owns`) beside the definite-assignment marks: a snapshot
  at every `if`/`case`/loop, a branch restored to it, the fall-through
  branches compared, the loop's exits compared with its entry.
- `<~` lowers as the store it is; a parameter, a result, a binding and a
  place may all be `own`.
- `tests/run/own.oli` runs a file descriptor through it: `dup(0)` wrapped
  as `own u64`, moved into a place, passed to `release(h : own u64)`,
  which unwraps it for `close`; every path — an `if … else`, a `while`,
  a one-line `if … then ret` — consumes exactly once, and the last check
  closes descriptor 3 again and must fail. `tests/sema/err/linear.oli`
  pins all six diagnostics at their constructs (the seventeenth negative
  fixture); the harness pins the `.sema` form (an `own` result, parameter
  and place, three moves) and that the OIR carries no conversion.

## Implemented in stage 46 (2026-09-25): register allocation on AArch64

- The linear scan of `compiler/x64.oli` (OIR_SPEC §7) runs for AArch64 too:
  it works on the OIR alone, and its five callee-saved registers — rbx and
  r12-r15 — map one for one onto x19-x23, which AAPCS64 also keeps across
  calls. A procedure saves the ones it uses after its prologue and restores
  them before `ret`, in the frame words the x86-64 back end uses; a value
  the scan leaves out stays in its frame word. `tests/a64/core.oli` now
  names x19-x23 in 359 instructions, and every AArch64 run of the harness —
  the 52 fixtures, the traps, the C interop and the bare-metal kernel —
  passes unchanged.

## Implemented in stage 45 (2026-09-25): AArch64 objects, C interop and bare metal

- **Objects.** `-- output: object` under the AArch64 profile writes an
  EM_AARCH64 ET_REL: calls inside the file resolved, a call to an `extern`
  procedure R_AARCH64_CALL26, an `adr` into .rodata R_AARCH64_ADR_PREL_LO21,
  an `adrp`+`add` into .data/.bss R_AARCH64_ADR_PREL_PG_HI21 and
  R_AARCH64_ADD_ABS_LO12_NC. The three C-interop programs of `tests/c`,
  compiled for AArch64 and linked by a cross gcc 14 against glibc, run under
  qemu-aarch64 with exactly the x86-64 outputs: C calls Oli-- and back
  (`expected.out`), a PIE and a shared library (`pic_expected.out`), floats
  in d0-d7 with libm's `sqrt` and narrow integers re-extended across AAPCS64
  (`float_expected.out`).
- **Bare metal.** `calls none` procedures lower with their values in
  x19-x28 (no frame; E0900 if more are needed), the generic hardware
  commands map to AArch64 (`cpu.stack <-` `mov sp`, `cpu.frame`, `cpu.call`
  `bl`, `cpu.jump` `br`, `cpu.halt` `wfi`, `cpu.pause` `yield`,
  `cpu.interrupts` `msr daifset/daifclr`, `cpu.tsc` `mrs cntvct_el0`), and
  a freestanding trap stops in `wfi`. `tests/a64/free/kernel.oli` boots on
  qemu-system-aarch64 10.0.13's `virt` board (`-kernel`, EL1, MMU off),
  sets its stack, and prints on the PL011 UART: `fib(20) = 6765, calls
  21891` (2·fib(21) − 1), `zone sum = 2016`, `timer ok`.
- The harness checks the objects' types and relocations and the kernel's
  load address always, links and runs them when `OLI_CC_AARCH64` and
  `OLI_AARCH64_SYSROOT` are set, and boots the kernel when
  `OLI_QEMU_SYSTEM_AARCH64` is. The tools were Debian packages unpacked into
  a scratch directory for these runs (gcc-14-aarch64-linux-gnu,
  libc6-dev-arm64-cross, qemu-user, qemu-system-arm), not installed.
- Left on AArch64: interrupt procedures and the exception vector table,
  the `traps` procedure called with its `core.Site` (register allocation
  came in stage 46).

## Implemented in stage 44 (2026-09-25): AArch64 runs the corpus

- **The rest of a hosted program on AArch64**: zones (mmap 222 with the
  page rounding and the refusal check, munmap 215, `at`/`from` zones and the
  allocation with its `zone_exhausted` checks), `mem.copy` (forwards, or
  backwards over an overlap) and `mem.set`/`zero`/`secure_zero` as byte
  loops, the `sat`/`checked` modes (the overflow of a 64-bit operation
  recomputed from its operands, since no flag survives the store), stack
  arguments past the sixth word, `f32`/`f64` (`fmov` into d0/d1, `fadd`…
  `fdiv`, `fcmp` with conditions false on a NaN but `!=`, `scvtf`/`ucvtf`,
  `fcvtzs`/`fcvtzu`, `fcvt`; d0-d7 for float arguments and d0 for the
  result), 128-bit vectors in NEON (`ldr/str q`, `fadd/add/sub/mul/…
  v.4s/2d/16b/8h`, `cmeq`/`fcmeq` and `uminv` for whole-vector equality),
  atomics as `ldaxr`/`stlxr` loops (loads `ldar`, stores `stlr`, `cas` with
  `clrex` on a mismatch) and fences as `dmb ishld`/`dmb ish`; volatile
  accesses as the plain ones this back end never reorders.
- **The same programs on both architectures**: `std.os` names the system
  calls of the target (`when target.arch`), and `io`, `memory` and `own` use
  the names instead of x86 numbers; the harness compiles every fixture of
  `tests/run` and `tests/run/trap` that is not x86 by nature with the AArch64
  profile in front and runs it under qemu-aarch64 — 52 of 59, the same exit
  status, output and trap message (one line later). The seven left are
  machine blocks, ports and control registers, interrupt procedures, the x86
  freestanding image and `when.oli`, which asserts that it was compiled for
  x86-64 (and on AArch64 says it was not, as it should).
- **Two encodings of stage 43 were wrong** and are fixed: `eor` carried a
  stray shifted register and `negs` read x0; the stage 43 programs used
  neither. Every constant of `compiler/a64.oli` has since been disassembled
  with its operands, not only its mnemonic.
- Still E0900 on AArch64: objects (no AArch64 relocations written yet) and
  freestanding images; no register allocator — every value in its frame
  word.

## Implemented in stage 43 (2026-09-25): a second architecture, AArch64

- **`compiler/a64.oli`**, the AArch64 back end, lowers the same OIR as the
  x86-64 one: constants (`movz`/`movk`), frame places (`ldur`/`stur`, or
  through x17), arithmetic with every trap of MACHINE_MODEL §4 — `adds`/
  `subs` and `b.vc`/`b.lo`/`b.hs`, `smulh` against the sign of the product
  or `umulh` against zero, the re-extension that catches a narrow overflow —
  `sdiv`/`udiv` with the zero and MIN/-1 checks and `msub` for `%`, shifts
  masked like x86-64's, comparisons into `cset`, raw loads and stores at
  every width, `adr` for the read-only segment and `adrp`+`add` for the
  writable one, views and their bounds and range checks, calls (`bl`, up to
  six argument words), system calls (`svc #0`, x8), phis as edge copies, and
  the entry's `exit_group`. Every value lives in its frame word — no
  register allocator yet — which is correct and slow. It is selected by a
  profile with `arch = "aarch64"` (`tests/a64/linux-aarch64.oli-target`);
  the ELF writer marks EM_AARCH64 and the DWARF frame base x29.
- **Verified on the target's instruction set**: every encoding was checked
  word by word against `aarch64-linux-gnu-objdump` (five constants were
  wrong on the first try and were fixed), and the programs run under
  `qemu-aarch64` 10.0.13: `tests/a64/core.oli` (23 checks), `limits.oli`
  (the edges that must not trap), `hello.oli` (stdout through write 64,
  numbers chosen with `when target.arch`) and ten trap programs (overflow
  at u8/s64/u64/u32, both multiply checks, division by zero, MIN/-1,
  unsigned negation, bounds, a subview past the end), each dying with its
  message and 134. The self-compiled compiler produces the same AArch64
  bytes as the genesis-built one.
- **Not yet on AArch64** (E0900, never approximated): zones, floats,
  vectors, atomics, `mem.copy`/`set`, machine blocks, x86 hardware places,
  more than six argument words, objects, freestanding images. The harness
  refuses a zone to prove it. QEMU user mode is not part of the toolchain:
  the harness runs the programs when `qemu-aarch64` is on the path or
  `OLI_QEMU_AARCH64` names it, and otherwise checks that each is an AArch64
  executable.

## Implemented in stage 42 (2026-09-25): 128-bit vectors (design 0024)

- **Ten types** — `f32x4 f64x2 s8x16 u8x16 s16x8 u16x8 s32x4 u32x4 s64x2
  u64x2` — sixteen bytes each, values like a layout held by value (the
  aggregate path gives locals, copies, parameters and results; `layout_param`
  answers 16). SSE2 only, which every x86-64 CPU has.
- **Forms.** `f32x4(a, b, c, d)` and `T.splat(x)` store the lanes into a
  frame area; `T.load(v, i)` / `T.store(v, i, x)` copy sixteen bytes after
  two bounds checks (first and last lane); `x[i]` and `x[i] <- e` reuse the
  checked element access with the lane count as the length.
- **Operators.** O_VBIN (`vadd.f32x4 %a, %b` …): `movdqu` of both operands,
  the instruction, `movdqu` into its own frame area, whose address is the
  value; O_VCMP: `cmpeqps`/`cmpeqpd`/`pcmpeqb`, the lane mask
  (`movmskps/pd`, `pmovmskb`) against all ones. `+ - & | ^` everywhere
  (integer lanes wrap), `*` on float lanes and `pmullw`, `/` on float lanes;
  an operator SSE2 lacks is E0900, one with no vector meaning or with
  operands of different types (a scalar, a literal) E0200.
- `tests/run/simd.oli` runs twenty-five checks (dot product through a
  procedure, u8 lanes wrapping 250 + 10 = 4, 300·300 in u16 lanes = 24464,
  a NaN lane unequal, a u64 lane carrying no carry into its neighbour, an
  accumulator in a loop); `tests/run/trap/simd_bounds.oli` traps on a load
  past the end; `tests/sema/err/vectors.oli` pins eight diagnostics; the
  harness finds every SSE2 family by objdump and refuses a vector argument
  of an `extern` procedure (SysV would want an xmm register).
- The negative-fixture count in the harness and README had stayed at
  eighteen after stage 41 added `floats.oli`; it is twenty now, both.

## Implemented in stage 41 (2026-09-25): `f32` and `f64`

- **Literals.** The lexer reads `1.5`, `2e-3`, `1_000.25`, `12E-1` as
  TK_FLOAT and rounds the decimal text to binary64 and, separately, to
  binary32 (to nearest, ties to even, subnormals included) with big-integer
  arithmetic of its own (`float_bits`/`float_round` in lex.oli), so an
  `f32` is never rounded twice. A value beyond binary64 is E0008; beyond
  binary32 is E0212 where an `f32` is wanted. Checked against an exact
  Fraction-based reference on 33 chosen and 2999 random literals, all equal,
  by the lexer `oli1` builds and by the one `olic` builds of itself. (The
  self-hosting layer caught one difference first: `e - BIAS_W + emax` went
  below zero on the way for every value under 1, which `oli1` wraps and
  `olic` traps as the language says; the terms are reordered.)
- **Types.** `f32`/`f64` are primitive (4 and 8 bytes); `+ - * /` and the
  comparisons apply, `%`, bit operations and shifts are E0200; nothing
  converts implicitly (E0200): `f64(n)`, `f32(n)`, `f64(f)`,
  `f32.wrap(d)`, `T.wrap(f)` (toward zero), `F.bits(u)`/`U.bits(f)`; a
  float literal or `f32(f64)` without `wrap` is E0201/E0202
  (`tests/sema/err/floats.oli`, eight diagnostics).
- **Code.** A float is its bits in a word (binary32 zero-extended), so
  loads, stores, frames, statics, arrays and layout fields need only the
  width; the OIR gains `fop`, `fcmp`, `fcvt`, `fret`, `fres`, lowered to
  `addsd/subsd/mulsd/divsd` (ss), `ucomisd` with `sete`+`setnp`, `setne`+`setp`,
  `seta`/`setae` on the swapped pair (false on NaN but `!=`),
  `cvtsi2sd`, `cvttsd2si`, `cvtss2sd`, `cvtsd2ss`, and a u64 past 2^63 both
  ways (halved and doubled; `2^63` subtracted and `btc`). Static and constant
  initialisers are evaluated as floats (`const_float`).
- **ABI.** Float arguments go in xmm0–xmm7 beside the integer registers,
  a float result in xmm0 (SysV), so `tests/c/float_side.oli` calls libm's
  `sqrt` and a C function with mixed arguments, and C calls its float
  procedures. Found on the way and fixed: C leaves the upper bits of a
  narrow integer undefined — an `export`ed procedure now re-extends its
  narrow parameters and an `extern` call's narrow result is re-extended
  (`oli_narrow(-7)` receives −7, `c_minus_five()` gives −5).
- `tests/run/floats.oli` runs thirty-nine checks (NaN comparisons, 0.1
  summed ten times is not 1.0 but `0x3FEFFFFFFFFFFFFF`, 1e19 to u64,
  18446744073709551615 to f64); the harness pins every SSE family by
  objdump, the lexer's bits for six hard literals, the C float link, and the
  snapshots. Not done: SIMD vectors, math intrinsics (libm through
  `extern proc` works), float types in DWARF, more than eight float
  arguments.

## Implemented in back-end stage 40 (2026-09-25): DWARF variables

- **Types and variables in `.debug_info`.** The compile unit carries the
  base types (u8…s64, `bool`), pointers to each and to `void`, a view
  structure per element type and a structure per layout (its integer and
  `bool` fields at their offsets); every subprogram gains a frame base
  (rbp) and its parameters and variables as children, each with its type.
- **Locations, honestly.** The OIR builder records every local (name, type
  class, frame words) and `mem2reg` marks which kept their words. Those get
  `DW_OP_fbreg` to their first byte; a local promoted to a register gets no
  location, so gdb prints `<optimized out>` — never a stale frame word.
- **`-- debug: frame`** at the top of a file promotes no local: every
  parameter and variable is in its words at every line (the `-O0` of this
  compiler).
- `tests/run/debugvars.oli` (with the pragma): the harness stops gdb at the
  `ret` of `work` and finds `n = 5`, `scale = -2`, `total = 20`, `buf =
  {11, 0, 0, 44}`, `pt = {x = 7, y = -3}`, `i = 5`, `flag = true`, the text
  `"hi"` behind a view and `ptype` of the layout; the same source without
  the pragma shows `total = <optimized out>` and still `pt`. `readelf`
  decodes the unit without a complaint.

## Implemented in back-end stage 39 (2026-09-25): position-independent code

- **Every static address is rip-relative.** `O_SADDR`, `O_DADDR` and the
  two trap sites (the message, and the file of a freestanding
  `core.Site`) were `movabs reg, imm64` with an absolute fix; they are
  `lea reg, [rip + disp32]` now (fix kinds 10 read-only, 11 writable),
  three bytes shorter each. An executable patches the displacement from
  the end of the instruction; an object file carries `R_X86_64_PC32`
  against `.rodata`, `.data` or `.bss` with the addend less four. The trap
  sites' fixed lengths followed (22 and 42 bytes).
- **Result:** an Oli-- object links into `cc`'s default PIE and into a
  shared library with no text relocations. `tests/c/pic_side.oli` (statics
  in `.data` and `.rodata`, a string, a call to `puts`) is linked both ways
  by the harness and must print `tests/c/pic_expected.out`; the object must
  carry no absolute relocation at all.
- Still absolute, by what the program says: a machine block's
  `mov r64, static`, `[static]`, `addr32`, and `olic`'s own executables,
  which remain ET_EXEC at a fixed address (a static PIE of its own — ET_DYN
  with no dynamic linker — is not written yet).

## Fixed in stage 38 (2026-09-25): the three silent defects of the audit

The audit of commit 237f1d6 found three places where `olic` accepted a
program and produced a wrong result without a word. Each is fixed and
pinned:

- **A member of a scalar.** `g.addr` on a `u64` static compiled to a load
  of `g`'s value, and `.len` or any unknown member of an integer, `bool` or
  address passed the checker (E0105 was specified but never emitted). The
  checker now reports E0105 for a member of an integer, `bool` or address
  type unless a layout field has that name
  (`tests/sema/err/members.oli`, the eighteenth negative fixture).
- **A machine block in a `calls none` procedure** had the callee-saved
  registers it names saved and restored through rbp — in a procedure with
  no frame, i.e. in its caller's frame. A frameless block now owns its
  registers; `tests/run/switch.oli` is a context switch between two stacks
  written that way (five round trips), and the harness pins that
  `switch_to` is exactly its block, with no store through rbp.
- **The profile's `arch`** was read only by `when target.arch`; a profile
  naming `aarch64` got an x86-64 image. `arch` other than `x86_64`, or `os`
  other than `none`/`linux`, is now refused with a message and no file (the
  harness compiles such a profile).

## Implemented in stage 37 (2026-09-25): a kernel heap

- **`frames.alloc_run(fmap, n)`** hands out the lowest run of `n` free
  frames (0 when there is none) and **`release_run`** gives a run back;
  `tests/run/pmm.oli` gains seven checks (a hole too short skipped, a run
  too long refused, a run of zero).
- **The kernel's heap is a zone over frames.** `heap_demo` takes 256 frames,
  opens `zone kheap 1M at addr u8 (base)` — the zone of the language, laid
  over physical memory the frame allocator handed out (the first gigabyte
  is identity-mapped) — makes a thousand `Record`s with `kheap.make`,
  chains and walks them through raw loads, takes 4 KiB with `bytes`, is
  refused a further megabyte by `try_bytes … else`, and after `end` gives
  the run back; the free count is what it was.
- **Verified under QEMU**: `heap: 1000 records at 0000000000200000, sum of
  squares 332833500, 1M refused, frames returned` (332833500 = Σ i² for
  i < 1000), eight COM1 lines in all; the harness pins the line.
- Found on the way: `r.addr` on a `ref` is E0900 (`.addr` is a view's
  member); `addr u8 (r)` is the conversion the language documents, and the
  demo uses it.

## Implemented in stage 36 (2026-09-25): a physical memory manager

- **`core.frames`** (`lib/core/frames.oli`, Oli-- only): one bit per
  four-kilobyte frame in a bitmap the caller owns, set = in use. `init`
  marks everything in use, `load_multiboot(fmap, info)` reads a Multiboot 1
  information structure through raw memory and frees every frame wholly
  inside an available region of its memory map, `reserve_range` marks a
  range in use again, `alloc` hands out the lowest free frame's physical
  address (0 when none), `release`, `count_free`, `in_use`, `capacity`.
- **The kernel uses it.** The trampoline's first instruction after `cli`
  saves ebx (the information structure's address) into the static
  `mb_info`; `memory_init` frees the loader's RAM into a 32 KiB bitmap for
  the first gigabyte, reserves the first two megabytes (BIOS area, the
  loader's tables, the image and its .bss), prints the count, allocates a
  frame, writes and reads it through its physical address and releases it.
- **Verified under QEMU 10.0.13**: seven COM1 lines, among them
  `memory: 2 regions, 15840 frames free` at `-m 64` and `frame:
  0000000000200000 written and read`; the same image at `-m 128` reports
  32224 frames — 16384 more, exactly 64 MiB, so the count is read from the
  machine. The harness pins both boots (when QEMU is present) and the
  difference.
- `tests/run/pmm.oli` runs the allocator in a process over a map built by
  hand in a static (the address a Multiboot table holds is 32 bits):
  seventeen checks, partial frames at the ends of a range included.
- Found on the way: `bits` is a
  reserved word, and a module-level name may not reappear as a parameter
  anywhere in the program (E0101) — the library's bitmap parameter is
  `fmap`.

## Implemented in back-end stage 35 (2026-09-24): aggregate constants

- **`NAME : [N]T := { … }` and a layout constant** are lowered: the data
  collection puts each in the read-only segment (`add_rodata`, as a static
  placed there) and the ELF writer gives it a symbol `module.NAME`; a read
  is the static read — an array the view of its bytes (index, `.len`,
  `each`, a `view` argument), a layout its address — and `ref NAME` is its
  address. A view constant is still E0900, an integer constant has no
  address (`ref K` E0900), and a store into an aggregate constant is the
  checker's E0111 as before. The harness's "unlowered construct" probe,
  which was such a constant, now reads `arch.x64.gdt` (no `sgdt`).
- **A bug found on the way**: `compact_statics` (compiler/opt.oli), which
  drops the trap messages of checks the passes removed, closed the gaps
  between the remaining statics without their alignment, so a static
  placed in `.text.boot` or `.rodata` with `align N` could move to an
  unaligned address (the kernel's Multiboot2 header stayed aligned only
  because it came first). `Stat.alg` now records the alignment and the
  compaction pads with zeros.
- `tests/run/rodata.oli` runs ten checks over three arrays and a layout
  constant; the harness pins each symbol read-only and at its alignment.

## Booted (2026-09-24), stage 34: the Multiboot path of `examples/kernel.oli`

- **`addr32 NAME`** in a `machine x64` block (design 0023): the four-byte
  absolute address of a procedure (fix kind 9, `R_X86_64_32` in an object)
  or a static (the kinds a `[static]` operand takes), as data.
- **The kernel example boots.** `mb1` is a never-called procedure whose
  block is a Multiboot 1 header with the a.out kludge (QEMU's `-kernel`
  loads no 64-bit ELF by its program headers): header_addr `addr32 mb1`,
  load_addr 0x100000, load_end 0 (the whole file), bss_end 0x200000, entry
  `addr32 start`. `start` is the 32-bit trampoline — `bytes` for the 32-bit
  instructions, `mov [static], r32`, `jnz .label` and `addr32` being the
  same bytes in both modes: cli; the PML4 and PDPT entries; the page
  directory filled with 512 two-megabyte pages in a loop; cr3; cr4.PAE;
  EFER.LME through rdmsr/wrmsr; a GDT (null, 64-bit code 0x00209A00…,
  data 0x00009200…) and its pointer written by hand and `lgdt`; cr0.PG|PE;
  `jmp far 0x08:start64`. `start64` reloads every segment register with
  `arch.x64.segments(code: 0x08, data: 0x10)` (stage 32), sets the stack
  and frame and calls `main`. After its hundred timer ticks `main` writes
  QEMU's isa-debug-exit port (0x501, value 0x10: exit status 33).
- **Verified for real** with QEMU 10.0.13 (Debian 1:10.0.13+ds-0+deb13u1,
  TCG, 64 MiB, `-display none -nic none -serial file: -device
  isa-debug-exit,iobase=0x501,iosize=1 -kernel kernel.elf`): exit status
  33 and exactly these five lines on COM1 —

  ```
  Oli-- kernel
  cpu: AuthenticAMD
  int 3 at 00000000001018b1
  back from int 3
  timer: 100 ticks
  ```

  i.e. the trampoline entered long mode, the segments were reloaded from
  the kernel's own GDT, `cpuid` ran, the IDT built by the kernel took a
  software interrupt and returned, the PICs and the PIT delivered a hundred
  interrupts to `on_timer`, and the kernel left through the port. (QEMU was
  obtained as Debian packages unpacked under a scratch directory — no
  installation — so it is not on this machine's path.)
- The harness checks the Multiboot 1 header's bytes against the symbols,
  the ELF entry, the trampoline's decoding with `objdump -M i386` (eighteen
  instructions and the far jump to `start64`), and — when
  `qemu-system-x86_64` is on the path or `OLI_QEMU` names it, with
  `OLI_QEMU_BIOS`/`OLI_QEMU_DATA` for its firmware directories — boots the
  image and pins the exit status and the five lines; otherwise it prints
  `skipped: no qemu-system-x86_64 here` with the date of this run. The
  audit's item #8 (a boot path a loader can take) is closed.

## Self-hosting reached (2026-09-23): `stage2 == stage3`

The gate of G4 (design 0022, completion gate 3): `olic`, built by `oli1`,
compiles its own source (`compiler/`, seventeen modules, 34,019 lines) into
stage 2; stage 2 compiles the same source into stage 3; the two files are the
same 1,545,376 bytes. `genesis/test.sh` layer 6 does this on every run, and
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
same limits, not a better one; every value still owns a frame word beside
the register the linear scan may give it, and the constructs still refused — `choice`,
`machine`, `be`/`le` — are refused by stage 2 exactly as by stage 1. The fixpoint proves the compiler agrees with itself;
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
