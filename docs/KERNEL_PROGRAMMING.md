# Kernel Programming in Oli--

How boot code, drivers and kernels are written in Oli--, and which language
construct answers each system-programming requirement.

> **Status (2026-09-23).** A design for milestone M4. What M3 delivered
> generates code today (`docs/FREESTANDING.md`, `docs/PROJECT_STATUS.md`
> stages 14–15): a freestanding entry with no frame, its own stack, `traps`
> with `core.Site`, zones `at`/`from`, `cpu.halt()`, `machine` blocks with
> port I/O and control-register moves, `-- load:`, a Multiboot2 header as a
> static in `.text.boot` — `examples/kernel.oli` is a bootable image whose
> structure the harness checks. The "Version" column below is otherwise the
> plan, not the state: V1 and V2 rows report `E0900` (`feature not
> implemented`) — `tests/sema/err/not_implemented.oli` pins that behaviour,
> and `tests/parse/ok/kernel_sketch.oli` is checked to report `E0900` for
> `port u8 (…)` and `E0401` for the raw-address conversion it performs
> without `permit memory.raw`. `calls interrupt` handlers run (stage 17):
> `lib/core/x64.oli` holds `InterruptFrame`, `IdtGate` and `TablePointer`;
> `mem.mmio` runs (stage 20): `mmio rw view T` with every element access a
> volatile load or store (`tests/run/mmio.oli`; `examples/kernel.oli` writes
> the VGA cells through it); `core.x64.paging` is still planned. See
> `docs/LANGUAGE.md`.

## 1. Requirement → construct

| Requirement | Oli-- construct | Version |
|-------------|-----------------|---------|
| volatile memory | `mmio view T` from `mem.mmio(T, a, n)` — every element access is a volatile instruction, kept by every pass (**runs**); `mmio ref T` | V0 (view, intrinsic), V1 (ref) |
| atomic operations | `atomic.load/store/add/sub/and/or/xor/cas(ref rw T, ..., order)` | V1 |
| memory barriers | `cpu.fence(order)` → `mfence`/`lfence`/`sfence` | V1 |
| interrupt handlers | `proc h(frame : ref core.x64.InterruptFrame)` with `calls interrupt` — **runs**: every general register pushed, `iretq` on return (`tests/run/interrupt.oli`, `examples/kernel.oli`) | V0 |
| naked functions | `calls none` (body = one `machine` block) | V0 |
| custom calling conventions | `calls sysv` (default), `calls none`, `calls interrupt`; `calls c` = alias of `sysv` | V0/V1 |
| packed structures | `layout X packed` | V0 |
| explicit alignment | `align N` on layouts, fields, statics, procedures, zone allocations | V0 |
| bit fields | `bit` and `bits N` field types inside `packed` layouts | V1 |
| CPU intrinsics | `cpu.halt`, `cpu.pause`, `cpu.cpuid`, `cpu.rdmsr/wrmsr`, `cpu.cr3`, `cpu.interrupts(on/off)`, `cpu.tsc`, `cpu.lgdt/lidt` | V0 (halt, pause), V1 (rest) |
| hardware places and commands | `cpu.stack <- a`, `cpu.call(p)`, `cpu.id(leaf)`, `arch.x64.cr3 <- p`, `arch.x64.gdt <- ref t`, `arch.x64.segments(...)`, `port.u8[n] <- b` (design 0015) | V0 (cpu.*), V1 (arch.x64.*, port.*) |
| inline machine code | `machine x64 ... end` with `in`/`out`/`clobber`, assembled by `olic` — the escape hatch | V0 |
| physical memory | `physaddr` type; `zone ... at`; `memory.raw` | V0 |
| virtual memory | `addr T` is a virtual address; page-table layouts in `core.x64.paging` | V0 type, V1 library |
| MMIO | `mmio` views from `mem.mmio` + `memory.mmio` capability | V0 (**runs**) |
| port I/O | `port.u8/u16/u32[n]` as a place under `io.port` — **runs**: one `in` or `out` each, volatile (`tests/run/hw.oli`; the kernel example programs COM1, the 8259 PICs and the 8253 PIT with them); the `port T` value type, `p.in()`, `p.out(v)` | V0 (place), V1 (type) |
| syscalls | `os.syscall` (hosted programs); a kernel *implements* syscalls with `calls interrupt` or a `machine` `syscall` entry stub | V0 / V1 |
| page tables | `layout` + `packed` + `bits` + `physaddr` | V1 |
| SIMD registers | `machine` blocks in V0; native vector types in V2 | V0 / V2 |
| TLS | `cpu.fs_base`/`gs_base` intrinsics; `thread` statics | V2 |
| custom allocators | `zone ... from HANDLE`; allocator layouts in `core.mem` | V0 / V1 |
| manual stack manipulation | `machine` blocks (`lea rsp, [...]`), `calls none`, `boot_stack` statics | V0 |

If any row above cannot be written in Oli-- once its version ships, the
language is not low-level enough and the design must change — not the requirement.

## 2. Capabilities used by kernels

| Capability | Grants |
|------------|--------|
| `memory.raw` | `[a]` on `addr T`; `addr T (integer)`; `zone ... at` |
| `memory.mmio` | creating `mmio` views/refs from addresses |
| `io.port` | `port` reads and writes |
| `cpu.asm` | `machine` blocks |
| `cpu.halt` | `cpu.halt()` |
| `cpu.interrupt` | `cpu.interrupts(on/off)`, `lidt`, `calls interrupt` procedures |
| `cpu.msr` | `rdmsr`/`wrmsr` |
| `cpu.control` | control registers (`cr0`, `cr3`, `cr4`), `lgdt`, segment loads |

`grep permit` over a kernel tree lists every procedure that touches hardware.

## 3. Sketches in V0 syntax

### Multiboot2 header as data in a named section

```oli
layout Multiboot2Header align 8
    magic         : u32
    architecture  : u32
    header_length : u32
    checksum      : u32
    end_tag_type  : u16
    end_tag_flags : u16
    end_tag_size  : u32
end

mb_header : Multiboot2Header := Multiboot2Header {
        magic: 0xE85250D6, architecture: 0,
        header_length: Multiboot2Header.size,                      -- constant: converts like a literal
        checksum: wrap(0 - (0xE85250D6 + 0 + u32(Multiboot2Header.size))),
        end_tag_type: 0, end_tag_flags: 0, end_tag_size: 8 }
    section ".text.boot"
```

### VGA text output through MMIO (runs)

```oli
proc vga_put(col : uword, row : uword, ch : u8, attr : u8)
permit memory.mmio
    vga := mem.mmio(u16, 0xB8000, 80 * 25)           -- mmio rw view u16
    vga[row * 80 + col] <- u16(attr) << 8 | u16(ch)  -- one volatile 16-bit store, bounds CHECK
end
```

### Serial output through port I/O (runs)

```oli
proc serial_put(b : u8)
    permit io.port
    while port.u8[0x3FD] & 0x20 == 0     -- one `in al, dx`, zero-extended
        cpu.pause()
    end
    port.u8[0x3F8] <- b                  -- one `out dx, al`
end
```

`examples/kernel.oli` goes further: `pic_init` remaps the two 8259s and masks
every line but the timer's, `pit_init(100)` programs channel 0 of the 8253,
`on_timer` (`calls interrupt`, vector 32) counts ticks and writes the end of
interrupt, and `cpu.interrupts(on)` is the `sti` after which `cpu.halt()`
waits a second for a hundred ticks.

### GDT with packed descriptor and `lgdt`

```oli
layout GdtPointer packed
    limit : u16
    base  : addr u64
end

gdt : [3]u64 := { 0, 0x00AF9A000000FFFF, 0x00CF92000000FFFF }
    align 16

proc load_gdt
permit cpu.control
    p : GdtPointer <- GdtPointer { limit: 3 * 8 - 1, base: addr gdt }
    arch.x64.gdt <- ref p                        -- lgdt (design 0015, V1)
    arch.x64.segments(code: 0x08, data: 0x10)    -- the far-return + segment-reload sequence
end
```

The same with a `machine` block is still legal under `permit cpu.asm`; it is
the escape hatch for instructions that have no command.

### Interrupt handler (V1)

```oli
proc on_timer(frame : ref core.x64.InterruptFrame)
    calls interrupt
permit memory.mmio, cpu.interrupt
    ticks <- wrap(ticks + 1)
    pic.end_of_interrupt(0)
end
```

### Physical vs virtual addresses

```oli
HHDM : uword := 0xFFFF800000000000            -- higher-half direct map offset

proc virt_of(p : physaddr) -> addr u8
    ret addr u8 (uword(p) + HHDM)             -- explicit conversion; the only way
end
```

`physaddr + addr` is a type error; the conversion is one visible procedure.

## 4. The kernel's memory story

1. Boot: no zones; only statics (`.bss` stack, page tables) and `machine` blocks.
2. Early init: `zone boot N at ADDR` over a known-free physical range (identity-mapped).
3. Physical memory manager: a bitmap/stack of frames in `core`-style Oli--, handing out
   `physaddr` values.
4. Kernel heap: a `zone` per subsystem or a slab allocator exposing `zone ... from` handles.
5. Per-request scratch: nested zones, freed at `end`, no `free` calls, no leaks.

Every step is visible: `--explain-cost kernel.oli` lists each `ZONE`, `KERNEL`,
`CHECK` and `COPY` per line.

## 5. Milestone 4 scope

boot via Multiboot2 → own stack → serial + VGA output → `cpuid` → `hlt` loop.
Then, one at a time: GDT, IDT + handlers, PIC/APIC timer, physical memory
manager, paging, kernel heap, a cooperative scheduler.
