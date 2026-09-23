# Freestanding Mode

> **Status (2026-09-23).** The shape of §1, §3, §4 and §5 is implemented in
> `olic` (back-end stage 14): a program that says `-- target: freestanding`
> gets no generated code before its `entry` (`-> never`, `calls none`: the
> first instruction of the image, `section ".text.boot"` first), no
> `os.syscall` (E0401) and no `mmap` zone (E0330), zones `at` an address or
> `from` a buffer, `cpu.halt()`/`cpu.pause()` as one instruction, `align N`
> on statics, and a `traps` procedure that every trap site reaches with the
> `core.TrapKind` and a `core.Site` built on the stack (SysV memory class);
> without one a trap is `ud2`. A `-> never` body ends in `ud2`. This is
> verified by execution: `tests/run/freestanding.oli` runs as a plain Linux
> process — it can, because the image holds nothing but the program's own
> bytes, and it exits through a system call written in a `machine` block —
> and `tests/freestanding/trap_line.oli` exits with the line of its overflow,
> delivered to `traps`. Of §2, `load_address` is read from a `-- load:` line
> in the source (stage 15) — statics are named through 64-bit immediates, so
> any address works; a `[static]` operand inside a `machine` block is a
> sign-extended disp32 and needs the low or the top 2 GiB — and the §8 target
> program exists: `examples/kernel.oli`, a Multiboot2 header as a static
> layout with an initialiser in `.text.boot` (statics in `.text*`/`.rodata*`
> sections go in front of the code), COM1 through `out dx, al` (design
> 0023), the VGA text buffer through raw stores, `cpuid` through a block.
> The harness checks the image structurally — load address, entry, header
> bytes at offset 176, port I/O and `cpuid` in the blocks, no system call
> anywhere — and `qemu-system-x86_64 -kernel kernel.elf -serial stdio` runs
> it. Not implemented: the profile *file* of §2, the section order beyond
> `.text.boot`, `mem.mmio`, `own`, the red-zone rule, and every library name
> beyond `core.TrapKind`, `core.Site` and `core.CpuId`; `lib/core.oli` and
> `lib/core/mem.oli` hold what is real. See `docs/LANGUAGE.md` §13 and
> `docs/PROJECT_STATUS.md`.

```
olic --freestanding kernel.oli          # planned
olic --target x86_64-freestanding kernel.oli
```

Freestanding mode produces a binary that assumes **no operating system**.

## 1. What is absent

| Hosted | Freestanding |
|--------|--------------|
| generated `_start` calling the `entry` procedure (`-> s32`) | **no generated code before `entry`**; the `entry` procedure (`-> never`) is the first instruction |
| `os.syscall` permitted | `permit os.syscall` is a compile error: there is no kernel to call |
| top-level `zone` gets memory from `mmap` | top-level `zone` needs `at ADDR` or `from SOURCE`; otherwise a compile error |
| traps print a message and exit (in the genesis compiler `oli1`: `oli: trap`, exit 3; in `olic` a `core` trap procedure, planned) | traps call the procedure declared with `traps`, or execute `ud2` |
| `std` available | only `core` (no OS, no heap, no I/O) |
| red zone available | red zone **disabled**: interrupts can arrive at any time |
| base address `0x400000` | load address from the target profile |
| stack provided by the kernel | the program declares and installs its own stack |

Nothing else changes: the language, the checks, the cost model and the
capability system are identical. A freestanding program is not "unsafe
mode"; it is the same language with fewer sources of memory and no OS.

## 2. Target profile

A target profile is a small file (`x86_64-freestanding.oli-target`, TOML) that
`oli.toml` or `--target` points at:

```toml
[target]
arch          = "x86_64"
os            = "none"
code_model    = "kernel"      # small | kernel: kernel = addresses in the top 2 GiB, RIP-relative + sign-extended imm32
load_address  = 0xFFFFFFFF80100000
red_zone      = false
stack_probe   = false
entry         = "kernel.start"
sections      = [".text.boot", ".text", ".rodata", ".data", ".bss"]   # emission order
align_sections = 4096
```

Everything in the profile can also be given on the command line
(`--load-address`, `--code-model`, `--entry`). The profile is the single
place where "linker script" knowledge lives; Oli-- has no separate linker
and no separate linker-script language.

## 3. Entry, stack, sections

```oli
module kernel

boot_stack : [16K]u8
    section ".bss.boot"
    align 16

pub proc start -> never
    entry
    calls none                     -- no prologue: the stack is whatever the loader left
    section ".text.boot"
    permit cpu.control
    cpu.stack <- addr boot_stack + 16K   -- hardware places and commands (design 0015)
    cpu.frame <- 0
    cpu.call(main)
    cpu.halt()
end

proc main -> never
permit cpu.halt, memory.mmio
    ...
    loop
        cpu.halt()
    end
end
```

- `entry` marks the ELF entry symbol. There is exactly one per program.
- `calls none` disables the prologue/epilogue; the body holds only hardware
  statements (`cpu.stack <- …`, `cpu.call(...)`, `cpu.halt()`) and `machine` blocks —
  the compiler cannot promise a valid stack, so no locals and no ordinary calls.
- `section` places code or data in a named section; the profile decides the order.
- `align N` on a static place or procedure sets its alignment.
- `-> never` procedures never emit `ret`; the compiler verifies the body cannot fall through.

## 4. Memory in freestanding programs

There is no heap unless the program builds one:

```oli
zone boot 1M at addr u8 (0x200000)       -- requires memory.raw in the enclosing proc
    tables := boot.make(PageTables)
    ...
end
```

or from a static region:

```oli
heap_region : [4M]u8
    section ".bss"
    align 4096

zone kheap 4M from heap_region           -- carve a zone out of a static array: ZERO cost
```

Physical memory managers, page allocators and slab allocators are ordinary
Oli-- code in `core.mem`/the kernel; they hand out `zone` handles or
`own` pages, and the escape analysis applies to whatever they return.

## 5. Traps

```oli
proc on_trap(kind : core.TrapKind, site : core.Site) -> never
    traps                                -- clause: this procedure receives all traps
permit cpu.halt, memory.mmio
    serial.write("trap: ") ...
    loop
        cpu.halt()
    end
end
```

At most one procedure may carry `traps`. Without it, every trap is `ud2`
(a `#UD` fault the kernel's IDT will see). The trap procedure runs on the
current stack; it must not return (`-> never`).

## 6. `core` vs `std`

| Library | Depends on | Contents |
|---------|-----------|----------|
| `core` | nothing | **today:** `TrapKind`, `Site`, `CpuId` (`lib/core.oli`) and `mem.equal` (`lib/core/mem.oli`). **Planned:** integer helpers, `mem.copy/set/zero/secure_zero`, `view` helpers, endian helpers, `InterruptFrame`, CPU intrinsics wrappers, minimal formatting into a `rw view u8` |
| `std` | `core` + an OS | `os` (syscall numbers, `Error`), files, mapping-backed zones, process, threads, networking |

A kernel imports `core` only. `olic --freestanding` rejects any import of `std`.

## 7. Compiler guarantees in this mode

1. No instruction sequence is emitted that the program did not write or that
   `--explain` does not list (prologues, checks, trap calls are listed).
2. No symbol is required from outside the program: no libc, no `memcpy`
   (`mem.copy` is generated inline or as an internal `core` procedure that is
   part of the binary; the intrinsic itself is planned — `mem.mmio` and the
   other `mem.*` intrinsics are currently `E0900`).
3. No red-zone use, no stack probes, no TLS access, no floating point unless
   the program uses `f32`/`f64` (then SSE must be enabled by the program before use).
4. The image is deterministic: identical input produces identical bytes.

## 8. Milestone 3 target program

A binary that: has its own `entry`, installs its own stack, writes a string to
the COM1 port (`io.port`) and the VGA text buffer (`memory.mmio`), reads
`cpuid` through a `machine` block, and halts — with no syscalls and no libc.
It is run under QEMU as a flat kernel (`-kernel` with a Multiboot2 header
placed in `.text.boot` as a static `layout` with `section`).
