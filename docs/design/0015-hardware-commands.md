# 0015 — Hardware places and commands (ACCEPTED)

Status: accepted 2026-09-20; specification text pending (tracked in `spec/*` revision history).

## Problem
Oli-- claims to reach memory and hardware directly, yet today the only way to
touch a register, a descriptor table, a control register or a port is a
`machine x64` block written in the CPU vendor's mnemonics (`mov`, `lea`,
`lgdt`, `retfq`). That is not another *language*, but it is not Oli--'s own
voice either: the programmer drops out of the language exactly where the
language should be strongest. The master directive asks for well-defined
intrinsics instead of inline assembly wherever one can be defined, and for an
`arch.x64` namespace for what is truly architecture-specific.

## Existing approaches
- C: compiler builtins (`__builtin_ia32_*`), vendor headers, `asm volatile`
  strings for everything else. Registers are never places.
- Rust: `core::arch::x86_64::*` functions plus `asm!`; the `x86_64` crate wraps
  `cli`/`lgdt`/`Cr3::write` as `unsafe` functions.
- Zig: a few builtins; everything else is `asm volatile`.
- Forth and machine monitors: registers and ports are first-class words —
  closest in spirit, but untyped.

## Oli-- approach
Everything the machine has is either a **place** or a **command**, and Oli--
already has the operators for both: `<-` stores into a place, a call runs a
command. Hardware gets the same treatment:

| Kind | Syntax | Meaning |
|------|--------|---------|
| architectural place | `cpu.stack <- addr boot_stack + 16K` | store to the stack pointer |
| architectural place | `x := arch.x64.cr3` | load a control register |
| indexed place | `arch.x64.msr[0xC0000080] <- v` | write a model-specific register |
| indexed place | `port.u8[0x3F8] <- b` / `s := port.u8[0x3FD]` | port I/O by width |
| table place | `arch.x64.gdt <- ref gdt_pointer` | `lgdt` |
| command | `cpu.halt()`, `cpu.pause()`, `cpu.call(main)`, `cpu.jump(a)` | control |
| command with result | `id := cpu.id(0)` → `CpuId { a, b, c, d }` | `cpuid` |
| command | `arch.x64.segments(code: 0x08, data: 0x10)` | far-return + segment reload sequence |
| command | `cpu.interrupts(off)` / `cpu.interrupts(on)` | `cli` / `sti` |

Rules:
1. Every hardware place has a **type** (`cpu.stack : addr u8`, `arch.x64.cr3 :
   physaddr`, `port.u8[n] : u8`, `arch.x64.msr[n] : u64`) and a **capability**
   (`cpu.control`, `cpu.msr`, `cpu.interrupt`, `io.port`, `memory.mmio`). A store
   of an `addr` into a `physaddr` register is `E0203`, like any mixed address space.
2. Every hardware access is **volatile** and has cost class `KERNEL`;
   `--explain` lists each one with the instructions it became.
3. `cpu.*` is architecture-neutral (stack, frame, halt, pause, id, interrupts,
   fence, call, jump, tsc); `arch.x64.*` is x86-64 only (cr0/2/3/4/8, msr,
   gdt, idt, tr, segments, fs/gs base). A program that names `arch.x64` does
   not compile for another target — that is the point of the prefix.
4. **`machine` blocks stay** as the escape hatch for instructions without a
   command (vendor mnemonics are the hardware's own vocabulary, not another
   language's), but a kernel written in the intended style never needs one.
   `--explain` counts `machine` blocks. The exact instruction encodings a
   `machine x64` block accepts are specified in `genesis/2-asm/SPEC.md`.

### Before / after (boot entry from `docs/FREESTANDING.md`)

```oli
-- today: the programmer leaves Oli--
pub proc start -> never
    entry
    calls none
    permit cpu.asm
    machine x64
        lea  rsp, [boot_stack + 16K]
        xor  ebp, ebp
        call kernel.main
        hlt
    end
end
```

```oli
-- proposed: Oli-- all the way down
pub proc start -> never
    entry
    calls none
    permit cpu.control
    cpu.stack <- addr boot_stack + 16K
    cpu.frame <- 0
    cpu.call(main)
    cpu.halt()
end
```

```oli
-- GDT load and segment reload, no mnemonics
proc load_gdt
permit cpu.control
    p : GdtPointer <- GdtPointer { limit: 3 * 8 - 1, base: addr gdt }
    arch.x64.gdt <- ref p
    arch.x64.segments(code: 0x08, data: 0x10)
end
```

### OIR representation
Three instructions, all volatile, all annotated with their capability:
`hw.load PLACE` → `%v`, `hw.store PLACE, %v`, `hw.cmd NAME(%args)` → `%r`.
`PLACE` is a fixed identifier (`cpu.stack`, `arch.x64.cr3`, `port.u8[%n]`,
`arch.x64.msr[%n]`). Machine lowering owns the instruction sequences (a
segment reload is one `hw.cmd` and about eight instructions). The verifier
rejects any `hw.*` not covered by a permit.

### Scope
| Version | Commands and places |
|---------|---------------------|
| V0 (Phase 2a, before OIR is frozen) | `cpu.stack`, `cpu.frame`, `cpu.call`, `cpu.jump`, `cpu.halt`, `cpu.pause`, `cpu.id` |
| V1 | `cpu.interrupts`, `cpu.fence`, `cpu.tsc`, `arch.x64.cr*`, `arch.x64.msr[]`, `arch.x64.gdt/idt/tr`, `arch.x64.segments`, `port.u8/u16/u32[]`, `mem.mmio`, `atomic.*` |

## Advantages
- The kernel examples in `docs/KERNEL_PROGRAMMING.md` become pure Oli--; the
  language reaches the hardware with its own syntax (`<-` into a register is
  the same operator as `<-` into a place — nothing new to learn).
- Typed hardware access: `physaddr` into `cr3`, `u8` into `port.u8`; the
  compiler knows what a command clobbers — no `clobber` lists.
- Capabilities become precise per place/command instead of one `cpu.asm`.
- `--explain` can describe every hardware touch in the programmer's terms.

## Disadvantages
- A catalog to design, document and test per architecture (each command needs
  a byte-exact encoding test like any instruction).
- Exotic instructions still need `machine` blocks (deliberate).
- `cpu.call(main)` in a `calls none` procedure assumes the callee is an Oli--
  procedure with the default convention; the compiler must check that.

## Machine cost
Exactly the instruction sequence of the command; `KERNEL` class; no runtime.
`cpu.id` writes four registers, so it returns a `CpuId` layout by value (16 bytes).

## Safety implications
Stores to hardware places are gated by capabilities and typed; a `machine`
block can still do anything under `cpu.asm`. Nothing new can be reached
without a permit.

## Alternatives rejected
- Functions-only intrinsics (`cpu.write_cr3(p)`): loses the place/store idea
  that makes Oli-- read uniformly; keeps the C flavour of "call a builtin".
- Registers as ordinary places everywhere (`rax <- 1` inside any procedure):
  conflicts with the compiler's register allocation; only `cpu.stack`/`cpu.frame`
  in `calls none` procedures are meaningful as places, and they are the ones offered.
- Removing `machine` blocks entirely: the escape hatch is what keeps the
  catalog honest and small.
