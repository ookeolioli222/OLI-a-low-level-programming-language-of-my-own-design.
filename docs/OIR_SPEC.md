# OIR — Oli Intermediate Representation

> **Status (2026-09-23).** Stage 6 of this document exists, and `olic` built
> on it reaches its fixpoint (`stage2 == stage3`, `genesis/test.sh` layer 6). `compiler/oir.oli`
> builds the instruction stream, `compiler/cfg.oli` cuts it into the basic
> blocks of §3 with one terminator each and computes predecessors, reverse
> postorder and immediate dominators, and `compiler/ssa.oli` is `mem2reg`: it
> promotes every place of a procedure to a value and puts a phi where two
> definitions meet, placed by the iterated dominance frontier, so the form is
> built minimal rather than built maximal and pruned. `--show-oir` prints the
> blocks before any pass and `--show-ssa` prints them after `mem2reg`
> (`tests/snapshots/*.oir` and `*.ssa`). The verifier of §8 runs on both, and
> `compiler/verify_check.oli` is its negative test: ten hand-made corruptions,
> each rejected with the invariant it breaks. `compiler/x64.oli` lowers the
> blocks — a phi becomes parallel copies on the edges that reach it — and
> `compiler/elf.oli` writes the executable, so `olic` compiles a program end to
> end for the subset in `compiler/oir.oli`; anything outside it is `E0900`.
> `compiler/opt.oli` then runs the passes of §6 to a fixpoint — constant
> folding (including a branch on a constant, and an operation whose constant
> answer its type cannot hold, which becomes the `trap` it always took), check
> elision, copy propagation over phis, and dead code — and `--show-oir=opt`
> prints the result with every removed check written where it stood and the
> proof that allowed it. §8.7 is enforced as an identity: the checks that
> stood before the passes equal the checks that stand after plus the proofs
> recorded. Since stage 4 the stream also carries `addr.of frame`,
> `raw.load`/`raw.store`, `check.bounds`/`check.range` and
> `zone.new`/`zone.alloc`/`zone.end` and `check.align`, so zones, views,
> `each` over a view, layouts and refs compile and run
> (`tests/snapshots/memory.*`, `layouts.*`; `tests/run/memory.oli`,
> `layouts.oli`), and `mem2reg` keeps the place an `addr.of frame` names, as
> §2.1 says — `ref x` of a local is the first thing that takes one. A
> `T or E` travels as the pair (tag, payload) of §2 — `ret`, `fail`, the
> `else` handlers and `case` all read and write that pair, and a default joins
> the ok path through a phi. A static place is `addr.of data.k` in a second,
> read+write segment of the image — its initialised bytes in the file, its
> zero ones past it — and raw `[p]` is `raw.load`/`raw.store` at the width
> of the address's element. Three
> proofs remove checks today — `constant`, `divisor` and `loop-bound` (a
> dominating branch on the same `cmp.lt`, which is the loop of `each` and of
> `while i < v.len`) — and the dead-code pass also drops the statics nothing
> names, so a proved-away trap leaves no message in the image.
>
> Stage 9 added the other sources of a zone — `zone.new.raw %t, %size,
> %addr` over memory that exists (an address, or a buffer whose length
> `check.range` proved) and `zone.new.from %t, %size, %parent` /
> `zone.end.from %t, %parent` carved from an enclosing zone — and the
> release of every open zone on every exit edge of its block, plus `ovf.of`
> for the `sat` and `checked` modes of a 64-bit operation.
>
> What does **not** exist yet: `view.load`/`view.store` and
> `ref.field`/`ref.load`/`ref.store` as instructions of their own (a view or
> field access is its check plus address arithmetic and a raw access), the
> `own`, hardware and intrinsic groups of §4 (a choice that fits a word
> needs no group: it is built and taken apart with the bit operations and
> `trunc` of the integer group, stage 11; a `machine x64` block is three
> instructions of its own since stage 12 — `machine.in REG, %v` before it,
> `machine x64` itself, `%n = machine.out REG` after it — its bytes coming
> from `compiler/asm.oli`, and `--show-asm` prints them), and since stage 13
> a linear scan (§7) over rbx, r12-r15 gives values registers — every value
> keeps its frame word too, so the lowering reads a value from wherever it
> is through one abstraction, and a procedure saves the registers it uses
> on entry and restores them at every `ret`; what does not exist is the MIR
> with virtual
> registers and the linear-scan allocator of §7, and the `--show-machine-ir`
> / `--show-asm` flags. The lowering still
> allocates nothing: each value owns a frame word. Constant folding answers
> only inside a window of ±2^31 per operand, because oli-core compares and
> divides signed; outside it the operation and its check both stay. The
> proof §6 calls dominance by an equal check is recorded by
> common-subexpression elimination (stage 10): an operation equal to one
> that dominates it goes, its uses move to the earlier one, and its check
> goes as `proof(dominance)`. Invariants 3-6 and 8
> of §8 are not checked because no construct that can break them is lowered
> yet.

OIR is the compiler's central data structure between the semantic graph and
machine lowering. It is designed for Oli-- semantics — zones, views, memory
spaces, capabilities, checks — not borrowed from LLVM. Nothing an optimizer
needs about *why* memory is safe is thrown away before machine lowering.

## 1. Pipeline

```
source (.oli)
  ↓  lexer                    --show-tokens
  ↓  parser → AST             --show-ast
  ↓  semantic graph           (names, types, regions, capabilities, definite assignment)
  ↓  OIR                      --show-oir        (blocks, before any pass)
  ↓  OIR passes               --show-ssa (after mem2reg), --show-oir=opt (after all of §6)
  ↓  machine lowering → MIR-x64 (virtual registers, two-address form)   --show-machine-ir
  ↓  register allocation (linear scan)                                  --show-machine-ir=alloc
  ↓  x86-64 encoder → bytes                                             --show-asm, --show-bytes
  ↓  ELF64 writer → executable
```

`--explain` and `--explain-cost` are computed from OIR *after* passes plus the
frame the lowering laid out (and the register allocation result, once there
is one), so what they print is what was emitted.

## 2. Design rules

1. **Typed values, explicit places.** SSA values (`%n`) are immutable and typed.
   Mutable state lives in named *places* (`frame.total`, `static.counter`, or a
   zone allocation) accessed with explicit `load`/`store`. `mem2reg` later
   promotes places whose address is never taken.
2. **Checks are instructions.** `check.bounds`, `check.overflow`, `check.align`,
   `check.zone` are real OIR instructions with a trap kind. Passes remove them by
   proof and record the proof; nothing else may remove a check.
3. **Views are first-class.** `view.make`, `view.sub`, `view.len`, `view.addr`,
   `view.load`, `view.store` keep the `(addr, len)` pair together so bounds facts
   survive to the optimizer.
4. **Memory space is on the instruction.** Every load/store carries `space=normal|mmio|port`
   and volatility follows from it. The optimizer never touches volatile instructions.
5. **Regions are metadata, not values.** Each `ref`/`view` value carries a region
   id (`static`, `frame#k`, `zone#k`, `param#k`, `raw`) from the semantic graph;
   passes may read but not change it.
6. **Ownership is explicit.** `own.move %src` produces the moved value and kills
   `%src`; the verifier rejects any later use.
7. **Machine blocks are opaque nodes** with typed inputs, outputs, clobbers and a
   `memory` side-effect flag; passes treat them as calls with those effects.
8. **Capabilities are annotations** on the procedure; the verifier checks that
   every `raw.*`, `mmio`, `port.*`, `syscall`, `machine` and privileged intrinsic
   is covered by a permit, so no pass can smuggle in a privileged operation.

## 3. Structure

```
module NAME
  static NAME : TYPE [section S] [align N] = INIT | zero
  proc NAME(params) -> RESULT
     permits: [...]          calls: sysv|none|interrupt   section, align, entry, export
     frame:  NAME : TYPE [align N]   ...      ; places
     bbN:    instructions ...  terminator
```

Blocks end in exactly one terminator: `jump`, `branch`, `ret`, `fail`, `trap`,
`unreachable`. Values are `%name`; places are identifiers qualified by
`frame.` or `static.`; constants are `const.TYPE VALUE`.

## 4. Instruction catalog (V0)

| Group | Instructions |
|-------|-------------|
| constants | `const.T v`, `addr.of static.x`, `addr.of frame.x` |
| arithmetic | `add.T`, `sub.T`, `mul.T`, `div.T`, `rem.T`, each with a mode `trap`, `wrap`, `sat` or `checked`; `neg.T`, `and.T`, `or.T`, `xor.T`, `not.T`, `shl.T`, `shr.T`, `sar.T` |
| compare | `cmp.eq.T`, `cmp.ne.T`, `cmp.lt.T`, `cmp.le.T`, `cmp.gt.T`, `cmp.ge.T` → `bool` |
| convert | `widen.T1.T2`, `trunc.T1.T2` (mode `wrap`, `sat`, `checked`), `bits.T1.T2` (same width), `bswap.T` |
| places | `load.T place`, `store.T place, %v` |
| views | `view.make %addr, %len`, `view.sub %v, %a, %b`, `view.len %v`, `view.addr %v`, `view.load.T %v, %i`, `view.store.T %v, %i, %x` |
| refs | `ref.field %r, FIELD`, `ref.load.T %r`, `ref.store.T %r, %x`, `ref.of_view %v, LAYOUT` (after `check.align`/`check.bounds`) |
| raw | `raw.load.T %a` (space), `raw.store.T %a, %x` (space), `raw.add %a, %off` |
| zones | `zone.new %size` (source: parent, os, raw, handle), `zone.alloc %z, %size, %align` → `%addr`, `zone.end %z` |
| choices | `tag %c`, `payload.VARIANT %c`, `make.VARIANT %fields`, `ok %v`, `fail %e` |
| checks | `check.bounds %i, %len`, `check.range %a, %b, %len`, `check.overflow %flag`, `check.align %addr, N`, `check.zone %z, %size` — each with a kind and a source site |
| control | `jump bbN`, `branch %c, bbT, bbF`, `ret %v`, `fail %e`, `trap KIND`, `unreachable`, `call PROC(%args)` → `%r`, `syscall %nr, %args` → `%r` |
| machine | `machine x64` node: inputs (reg, %v), outputs (reg, place), clobbers, memory flag, encoded body |
| ownership | `own.move %v` |
| hardware (design 0015) | `hw.load PLACE` → `%v`, `hw.store PLACE, %v`, `hw.cmd NAME(%args)` → `%r`; `PLACE` is `cpu.stack`, `cpu.frame`, `arch.x64.cr3`, `port.u8[%n]`, `arch.x64.msr[%n]`, …; all volatile, each annotated with its capability |
| intrinsics | `cpu.halt`, `cpu.pause`, `cpu.fence ORDER`, `mem.copy %dst, %src, %n`, `mem.set`, `mem.zero`, `mem.secure_zero` (never removed), `port.in.T`, `port.out.T` |

## 5. Text form example

Source:

```oli
proc checksum(data : view u8) -> u32
    total : u32 <- 0
    each b in data
        total <- wrap(total + b)
    end
    ret total
end
```

OIR after passes:

```
proc checksum(%data: view u8 [region param#0]) -> u32
  permits: []   calls: sysv
  frame:
    total : u32                 ; promoted by mem2reg -> %t, %t2
  bb0:
    %len = view.len %data
    jump bb1
  bb1:
    %i  = phi [bb0: const.uword 0] [bb2: %i2]
    %t  = phi [bb0: const.u32 0]   [bb2: %t2]
    %c  = cmp.lt.uword %i, %len
    branch %c, bb2, bb3
  bb2:
    ; check.bounds %i, %len   -- removed: proof(loop-bound: %i < %len)
    %b  = view.load.u8 %data, %i
    %bw = widen.u8.u32 %b
    %t2 = add.u32 wrap %t, %bw
    %i2 = add.uword trap %i, const.uword 1
    ; check.overflow %i2      -- removed: proof(%i < %len <= 2^63)
    jump bb1
  bb3:
    ret %t
```

## 6. Passes (V0) and their obligations

| Pass | Effect | Test obligation |
|------|--------|-----------------|
| verify | types, terminators, dominance, region/capability/own rules | rejects hand-written invalid OIR |
| check-elision | removes `check.*` with a recorded proof (constant, loop bound, dominance by an equal check) | snapshot before/after; count reported by `--explain` |
| const-fold | folds constant arithmetic, compares, conversions; a trapping op with constant overflow folds to `trap`, never to a wrong value | snapshot |
| mem2reg | promotes frame places whose address is never taken into phi values | snapshot |
| copy-prop + DCE | removes dead values and unreachable blocks; never removes volatile ops, unproven checks, `mem.secure_zero`, `machine`, `syscall` | snapshot |

Every pass supports `--show-oir=before:PASS` and `--show-oir=after:PASS` for snapshot tests.

## 7. Machine lowering (MIR-x64)

- Instruction selection is direct: each OIR instruction maps to one short
  pattern; a view becomes two virtual registers; a fallible result becomes a
  `(tag, payload)` register pair; `phi` becomes parallel copies on edges.
- MIR-x64 is two-address with virtual registers and explicit moves; frame
  places become `[rbp - off]`. V0 always keeps a frame pointer so `--explain`
  can show the frame; omitting it is a later optimization.
- Register allocation: linear scan over live intervals with spilling to the
  frame; caller-saved registers are spilled around calls; registers named by a
  `machine` block are excluded across it.
- Encoder: table-driven x86-64 (`REX`, ModRM, SIB, displacement, immediates)
  with one test per instruction form asserting the exact bytes.
- ELF64 writer: static non-PIE executable, `PT_LOAD` segments for text and data,
  section headers and a symbol table for debugging. See `ABI.md`.

## 8. Verifier invariants

1. Each value is defined once and dominates its uses.
2. Each block has exactly one terminator; every jump target exists.
3. Every `raw.*`, `hw.*`, `syscall`, `machine` and privileged `cpu.*` intrinsic is covered by a permit.
4. No use of a value after `own.move`.
5. `ret`/`fail` operands never carry the region `frame#self` or `zone#self`.
6. Volatile instructions keep their relative order.
7. A `check.*` may be removed only with a proof term attached to the removal.
8. `never`-typed calls are followed by `unreachable`, never by fall-through.
