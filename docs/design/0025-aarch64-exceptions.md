# 0025 — AArch64 system registers and exception handlers

## Problem
Design 0015 gave x86-64 its hardware places and commands, and `calls
interrupt` an IDT-driven handler. The AArch64 back end (stages 43–47) runs
freestanding images but cannot take an exception: there is no way to name
the vector base register or the generic timer, and no handler form. A
kernel needs both, and neither may be a machine-code escape.

## Existing approaches
- C: inline `msr`/`mrs` assembly and a hand-written `vectors.S` of 2 KiB
  with sixteen 128-byte slots, each saving registers and calling C.
- Rust (`aarch64-cpu`, `cortex-a`): register accessors per system register;
  the vector table in global assembly.
- Zig: inline assembly for both.

## Oli-- approach
**Places** (`permit cpu.control`, every one a `u64`), each one `mrs`/`msr`:

| Place | Register | Access |
|-------|----------|--------|
| `arch.a64.vbar` | VBAR_EL1 | read, written (then `isb`) |
| `arch.a64.cntv_ctl` | CNTV_CTL_EL0 | read, written |
| `arch.a64.cntv_tval` | CNTV_TVAL_EL0 | read, written |
| `arch.a64.cntfrq` | CNTFRQ_EL0 | read |
| `arch.a64.esr` | ESR_EL1 | read |
| `arch.a64.elr` | ELR_EL1 | read |
| `arch.a64.far` | FAR_EL1 | read |
| `arch.a64.cpacr` | CPACR_EL1 | read, written (then `isb`) |
| `arch.a64.icc_iar1` | ICC_IAR1_EL1 (GICv3) | read |
| `arch.a64.icc_eoir1` | ICC_EOIR1_EL1 | written only |
| `arch.a64.icc_pmr` | ICC_PMR_EL1 | read, written |
| `arch.a64.icc_igrpen1` | ICC_IGRPEN1_EL1 | read, written |
| `arch.a64.icc_sre` | ICC_SRE_EL1 | read, written (then `isb`) |
| `arch.a64.spsr` | SPSR_EL1 | read, written |
| `arch.a64.sp_el0` | SP_EL0 | read, written |

`arch.a64.elr` is written too (where the next `eret` goes). Two commands
complete user mode: `arch.a64.eret(p)` puts the address of procedure `p`
in ELR_EL1 and executes `eret` (with SPSR_EL1 = 0 it enters `p` at EL0 on
SP_EL0), and `arch.a64.svc(v) -> u64` puts `v` in x0, executes `svc #0`
and answers what the handler left in the saved x0. `svc` needs no `permit`:
it is how unprivileged code asks, and does nothing by itself.
| `arch.a64.vectors` | the vector table `olic` emits | read: its address |

**Handlers.** A `calls interrupt` procedure with `section ".vector.N"`
(N from 0 to 15, the architecture's slot order: synchronous, IRQ, FIQ,
SError for the current EL with SP0, with SPx, then the lower EL in AArch64
and AArch32) is the handler of slot N. When a program has handlers, the
back end emits the table as `.text.vectors` on a 2 KiB boundary: slot N
branches to a stub after the table that saves x0-x18, x30, ELR_EL1 and
SPSR_EL1 on the stack (x19-x28 the handler saves itself if it uses them),
and v0-v7 when CPACR_EL1 lets EL1 use the FP registers — after reset it does
not, and touching them would trap; the stub passes the address of that area
in x0 as a `ref core.a64.ExceptionFrame`, calls the handler with `bl`,
restores, and returns with `eret`; a slot with no handler stops the CPU in
`wfi`. `arch.a64.vbar <- arch.a64.vectors` installs it; `cpu.interrupts(on)`
(`msr daifclr, #2`) lets IRQs in.

On x86-64 every `arch.a64` place is E0900, as every `arch.x64` place is on
AArch64.

## Machine cost
A place is one `mrs` or `msr` (plus `isb` after VBAR). A taken exception
costs the slot's saves and restores (20 registers), a `bl`, the handler's
frame, and `eret` — KERNEL class, visible in the table's code.

## Safety implications
Under `permit cpu.control` (the places) and `cpu.interrupt` (a handler),
exactly as on x86-64. The saved area is the handler's to read through a
`ref` it is given; it outlives nothing.

## Alternatives rejected
- A `machine a64` block for the table: the one escape hatch would be
  mandatory for every kernel.
- A new clause keyword for the slot: `section` already names where code
  goes, and a section name costs no word of the language.
- Building the table at run time from branch instructions written into
  memory: code written as data needs cache maintenance the program would
  have to get right.

## Status
Implemented 2026-09-25 (stage 48): `tests/a64/free/timer.oli` takes a
hundred virtual-timer interrupts through the GICv2 of QEMU's `virt` board
(exactly 100 IRQs and no other exception in `qemu -d int`), with the FP
unit on so that every slot saves v0-v7 too. Stage 49: `tests/a64/free/gic3.oli`
on `virt,gic-version=3` drives the GICv3 CPU interface through the `icc_*`
places and takes 100 IRQs, a data abort in `main` and a data abort nested
inside an IRQ handler; the slot-4 handler reads ESR (class 0x25) and FAR
and moves the saved ELR past the load (`rw ref core.a64.ExceptionFrame`).
Nesting works because each slot saves ELR_EL1 and SPSR_EL1 before it
calls the handler. Stage 50: `tests/a64/free/user.oli` enters EL0 with
`arch.a64.eret(user_main)`; EL0 code prints through 48 `svc` calls served
from slot 8, and its read of VBAR_EL1 is refused by the CPU. Exceptions
from EL0 run on SP_EL1, apart from the user stack. Not yet: an MMU (EL0
still sees all physical memory), more than one CPU, GICv3 SGIs.
