# 0020 — Genesis layer 3: the oli-core compiler (oli1)

Status: oli-core subset complete. Steps 0–6d implemented and tested 2026-09-21.

## Problem
Layer 2 (`asm`) turns the `machine x64` sub-language into ELF. To reach a
self-hosted `olic` (layer 4) the chain needs a compiler for a real, if minimal,
Oli-- subset — oli-core — written in that sub-language (design 0017), so the step
from assembler to compiler is small and every increment is runnable.

## Oli-- approach
`genesis/3-oli1/oli1.oli` is compiled by `asm` into `oli1`, which reads oli-core
on stdin and writes a native ELF. oli-core is frozen incrementally
(`genesis/3-oli1/SPEC.md`); each step adds grammar and a behavioural test in the
genesis harness (layer 3). Step 0 compiles `proc … entry … ret <int> … end` to an
ELF that exits with the value.

Two constraints from `asm` shaped the design:
- asm-produced ELFs map only their file image, so oli1 takes scratch memory from
  an anonymous `mmap` rather than a fixed address.
- asm has no 8/16-bit register ops, so oli1 reads source bytes with a masked
  64-bit load and builds output from a template patched by 32-bit stores. Byte
  load/store is added to asm later, when oli1 emits variable-length code.

Step 3 places string data *before* the code (`header | pool | code`) so that a
string's address is known the moment it is bound and no fixup table is needed
in the bootstrap compiler; the code is moved behind the pool at finalize and
`e_entry` is patched. This costs one extra copy of the code per compile and
nothing at run time.

Step 4 abandons constant folding for a uniform run-time model: every integer
binding is a frame slot and every expression is compiled, even when constant.
The bootstrap compiler stays small (one code path per construct) and
non-constant sources — syscall results — need no special case. Forward branches
use compile-time fixup stacks in scratch instead of a second pass; `if`-chain
fixups and `break` fixups are separate stacks because they are resolved by
different enclosing constructs.

Step 5 keeps the single pass: a call to a not-yet-declared procedure creates a
placeholder table entry and a fixup; finalize resolves every `call rel32` and
checks arity, so declaration order never matters (design 0016: `entry` may be
anywhere). Parameters are spilled to frame slots in the prologue so that the
rest of the generator treats them as ordinary locals.

Step 6a chooses the ABI's own representation for the two aggregate values the
compiler needs: a view is `rax:rdx` and two slots, a zone handle is the
address of its `(base, cursor, limit)` triple. That keeps one expression
model (a value is one or two registers) and makes views and handles passable
to procedures with no special cases. Traps share one stub rather than being
inlined, and a pre-pass over `proc` headers replaces placeholders so that
types are checked at the call site. The per-exit-edge zone release of
`OLI_MEMORY_V0.md` §3 is not implemented; the compiler rejects the cases where
it would be needed instead of silently leaking.

Step 6b adds layouts as the ABI §3 describes them and a `ref` type that is
plainly an address with a compile-time layout attached, so field access is an
offset and a sized move. Layouts are collected in a pass of their own before
procedure headers, which keeps the single code-generation pass. With views,
zones and layouts, oli-core can now express tokens, tree nodes and symbol
tables — the data structures of a compiler.

Step 6c uses the ABI's representation of `T or E` directly (tag `rax`,
payload `rdx`) and restricts `T` to one word so that no `sret` path exists in
the bootstrap compiler. Resolution is attached to the expression grammar
(`expr = cmp_expr [else handler]`) rather than to bindings, so every context
that evaluates an expression enforces "a fallible value must be resolved" with
one check, and `case` is the only construct that sees the raw tagged value.
Fallible values are deliberately not bindable or passable in oli-core: the
compiler needs propagation and defaults, not storage of results.

## Advantages
- The assembler→compiler step is small and each increment runs on real hardware.
- No foreign toolchain; oli1 is built and tested entirely inside the chain.

## Disadvantages
- Template-and-patch output is a scaffold; real codegen waits on asm byte ops.

## Machine cost / safety
oli1 uses one `mmap` and raw `read`/`write`/`exit` syscalls. Malformed or
out-of-scope input must reject without emitting a partial ELF.

## Alternatives rejected
- Fixed scratch buffers (0x410000): unmapped in asm-produced ELFs (read → EFAULT).
- Writing oli1 in raw hex2: asm exists precisely so layer 3 is written in mnemonics.
