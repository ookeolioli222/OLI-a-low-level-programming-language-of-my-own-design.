# Oli-- — project and language documentation

## 1. Purpose and design goal

Oli-- is a low-level systems language designed to be explicit, transparent and machine-aware. Its main goals are:

- no hidden allocation,
- no hidden control flow,
- no implicit memory magic,
- explicit memory regions and object lifetimes,
- visible resource permissions and costs,
- compiler output that explains itself instead of hiding details.

The project is intentionally designed as a self-contained toolkit without Rust, C, C++, LLVM, or external linkers/assemblers. This is a fundamental design decision: the toolchain aims to bootstrap itself and remain autonomous.

Three levels are important:

1. User language — a readable systems-language syntax.
2. Memory model — regions, views, addresses, refs, zones and correct access rules.
3. Compiler pipeline — parser, semantic analysis, OIR, SSA, lowering to x86-64 and ELF output.

---

## 2. Core philosophy

The project documents describe Oli-- with the following principles:

- every operation has a visible cost,
- errors are values rather than exceptions,
- memory is explicit and region-based,
- permissions are granted, not assumed,
- every statement is visible and structured; one statement per line, blocks closed by `end`.

This gives Oli-- a low-level feel while preserving clarity and predictability.

---

## 3. Basic syntax

### 3.1 Modules and imports

```oli
module hello
import std.os
```

- `module` defines a compilation unit,
- `import` loads another module,
- names are resolved by the compiler according to project library rules.

### 3.2 Procedures

```oli
proc start -> s32
    entry
    permit os.syscall
    msg := "Hello from Oli--\n"
    os.syscall(os.WRITE, 1, msg.addr, msg.len)
    ret 0
end
```

Key concepts:

- `proc` declares a procedure,
- `entry` marks the program entry point,
- `permit` grants capability to use privileged features,
- `:=` binds a name to a value,
- `ret` returns a result,
- `end` closes the block.

### 3.3 Comments

```oli
-- ordinary comment
--- documentation comment
```

- `--` is a normal comment,
- `---` is a doc comment attached to the next declaration when applicable.

---

## 4. Memory model and regions

Oli-- does not treat memory as a single flat pointer area. Memory belongs to regions:

- static region,
- local/frame region,
- zone region,
- raw region,
- view region.

### 4.1 Zones

A `zone` is a bump-allocated region with lexical lifetime.

```oli
zone scratch 64K
    pkt := scratch.bytes(4096)
    hdr := scratch.make(Header)
end
```

Characteristics:

- memory is assigned explicitly,
- a whole zone is released at `end`,
- no garbage collector,
- region lifetime is visible and predictable.

### 4.2 Views

A `view` is a checked window into existing memory.

```oli
head := packet[0..20]
byte := packet[i]
len  := packet.len
```

A view includes:

- address,
- length,
- bounds-checked access,
- direct view into existing memory without copying.

### 4.3 Addresses and references

The language distinguishes:

- `addr T` — raw address,
- `ref T` — safe reference,
- `view T` — memory window,
- raw memory access — only under declared permission.

This distinction is central to the language: safe code and raw code are intentionally separated.

---

## 5. Data layouts and memory layout control

The language supports record-like layouts:

```oli
layout Header
    len : u32
    flags : u8
end
```

It can also define endian-sensitive fields and align/packed layouts. This gives the language real support for binary protocols, system structures, file formats and hardware interfaces.

---

## 6. Control flow

Oli-- supports the usual control structures:

```oli
if a > b then ret a
elif n == 0
    tag <- 'z'
else
    tag <- 'p'
end

while i < n
    ...
end

loop
    ...
    break
end
```

Supported constructs include:

- `if / elif / else`
- `while`
- `loop`
- `break`
- `continue`
- `each` over a view or range

---

## 7. Capabilities and safety model

Oli-- does not allow privileged operations without explicit permission.

Examples:

- `permit os.syscall`
- `permit memory.raw`
- `permit cpu.asm`
- `permit memory.mmio`
- `permit io.port`

This does two things:

1. restricts unsafe operations,
2. lets the compiler verify whether the code is allowed to perform the operation.

---

## 8. Commands and tools

The documentation describes the platform as a set of executable stages and inspection commands.

### 8.1 Genesis/build tools

| Command | Meaning | Status |
|---|---|---|
| `sh genesis/test.sh` | full pipeline verification | works |
| `genesis/build/oli1.bin` | oli-core compiler | works in genesis layer |
| `genesis/build/show_tokens` | print tokens | works |
| `genesis/build/show_ast` | print AST | works |
| `genesis/build/show_sema` | print semantic graph | works |
| `genesis/build/show_oir` | print OIR | works |
| `genesis/build/show_ssa` | print SSA | works |
| `genesis/build/show_opt` | print optimized OIR | works |
| `genesis/build/asm.bin` | `machine x64` assembler | works |

### 8.2 Planned `olic` commands

| Command | Meaning |
|---|---|
| `olic file.oli` | compile to ELF64 |
| `olic --show-tokens` | show tokens |
| `olic --show-ast` | show AST |
| `olic --show-sema` | show semantic graph |
| `olic --show-oir` | show OIR |
| `olic --show-ssa` | show SSA |
| `olic --show-oir=opt` | show optimized OIR |
| `olic --check` | analyse without code generation |
| `olic --freestanding` | freestanding mode |
| `olic --lib DIR` | library path |
| `olic --explain` | explain procedure cost and structure |

### 8.3 Why these commands matter

The commands are not mere convenience tools; they make the compiler transparent. A good systems language must allow the programmer to inspect:

- tokens,
- parse tree,
- semantic graph,
- OIR,
- SSA,
- output code quality and costs.

That is an important design idea in Oli--.

---

## 9. Compiler architecture

### 9.1 Genesis layers

The bootstrap stack is layered as follows:

| Layer | Produces | Status |
|---|---|---|
| 0 | `hex0.bin` | done |
| 1 | `hex2.bin` | done |
| 2 | `machine x64` assembler | done |
| 3 | oli-core compiler (`oli1`) | done |
| 4 | front-end in `compiler/` | done for analysis and parsing |

### 9.2 Front end

The front end includes:

- lexer,
- AST,
- parser,
- imports and module loading,
- semantic analysis,
- typing,
- flow and region tracking,
- diagnostics.

### 9.3 Middle end and backend

After semantic analysis come:

- OIR,
- CFG,
- SSA,
- optimization passes,
- lowering to x86-64,
- ELF64 output.

The goal is to produce native executables without external linkers or libc.

---

## 10. Freestanding and kernel programming

Two major directions of the project are:

1. freestanding programs without OS support,
2. kernels, boot code, drivers and hardware interfaces.

### 10.1 Freestanding mode

Freestanding means the program does not assume an operating system.

- no `os.syscall`,
- `entry` is the actual program start,
- no `std`,
- custom stack,
- no libc,
- explicit sections and addresses.

### 10.2 Kernel mode

The language design anticipates support for:

- MMIO,
- port I/O,
- CPU intrinsics,
- exception handlers,
- `calls interrupt`,
- `machine` blocks for direct hardware instructions.

This makes Oli-- a language for very explicit, hardware-aware system code.

---

## 11. Implementation status

The repository distinguishes several statuses:

- `runs` — implemented and tested,
- `analysed` — parsed and semantically checked, but not fully lowering to code,
- `reserved` — defined for a future language version,
- `planned` — specified but not implemented.

This distinction is crucial because it prevents overclaiming and keeps the language honest.

---

## 12. Sample program

```oli
module hello
import std.os

proc start -> s32
    entry
    permit os.syscall
    msg := "Hello from Oli--\n"
    os.syscall(os.WRITE, 1, msg.addr, msg.len)
    ret 0
end
```

This is the standard minimal example described by the documentation.

---

## 13. Why this project matters

Oli-- is not merely another systems language. It is a self-contained project in which:

- the compiler is part of the language story,
- the toolchain is designed from the ground up,
- memory and permissions are visible,
- hardware architecture is not hidden,
- the programmer is expected to understand the execution model.

That makes it suitable for people who want more than convenience: they want explicit control, correctness, and architectural transparency.

---

## 14. Summary

Oli-- combines three things:

1. a low-level systems language with readable syntax,
2. a memory and capability model with strong explicitness,
3. a compiler pipeline that explains structure, costs and generated code.

Its goal is not to hide complexity. Its goal is to make complexity visible, checkable and intentionally manageable.

This is the English version of the project documentation for Oli--.
