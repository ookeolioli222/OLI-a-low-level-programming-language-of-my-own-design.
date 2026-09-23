# Oli-- — the complete language and toolchain reference

One file: what the language is, how every construct works, every command you
can run today, and exactly which parts are implemented and which are not.
Normative wording lives in `spec/OLI_SYNTAX_V0.md`, `spec/OLI_SEMANTICS_V0.md`
and `spec/OLI_MEMORY_V0.md`; this document is the practical reference that ties
them, the design records and the working toolchain together.

Status marks used throughout:

| Mark | Meaning |
|------|---------|
| **[runs]** | implemented and covered by `sh genesis/test.sh` today |
| **[parses]** | the front end accepts and analyses it; the back end reports `E0900` and generates no code |
| **[planned]** | specified, not implemented |

---

## 1. What Oli-- is

Oli-- is a low-level systems language designed from the machine up: its own
syntax, its own memory model (zones, views, capabilities), its own
intermediate representation (OIR), its own x86-64 encoder and its own ELF64
writer. By decision (`docs/design/0017-genesis-bootstrap.md`) the toolchain
contains **no other language**: it is bootstrapped from hand-written machine
code and then written in Oli-- itself. No Rust, C or C++; no LLVM, `as` or
`ld`; no libc and no runtime.

Design principles (`docs/LANGUAGE_VISION.md`):

- **Nothing hidden.** No implicit allocation, no hidden control flow, no
  garbage collector, no exceptions. What a line costs is visible in the line.
- **Errors are values.** A failing procedure returns `T or E`; ignoring it is a
  compile error.
- **Memory is explicit and regional.** Every allocation belongs to a zone; a
  view is a bounds-checked window; escaping a region is a compile error.
- **Power is granted, not assumed.** Raw memory, MMIO, ports, assembly and
  syscalls each need a `permit`.
- **One statement per line.** No semicolons, no braces for blocks, `end` closes.

## 2. The five-minute tour

```oli
module hello
import std.os

--- Milestone 1: write a greeting with a raw Linux syscall, no libc.
proc start -> s32
    entry
    permit os.syscall
    msg := "Hello Oli--\n"
    os.syscall(os.WRITE, 1, msg.addr, msg.len)
    ret 0
end
```

- `module` names the unit, `import` brings another in.
- `---` is a doc comment, `--` an ordinary one.
- `proc … end` declares a procedure; `entry` marks the one the program starts
  at; `permit` grants a capability.
- `:=` binds an immutable name; `<-` stores into a place.
- A string literal is a `view u8` into read-only data: address plus length, no
  terminator.

## 3. Running the toolchain today

Everything below is a real command. Run them from the repository root on Linux
x86-64 (WSL works).

### 3.1 Build and verify the whole chain **[runs]**

```bash
sh genesis/test.sh
```

This is the one command that matters. It rebuilds every layer from source and
runs the acceptance suite:

| Layer | What it builds | What it proves |
|-------|----------------|----------------|
| 0 | `genesis/build/hex0.bin` from `genesis/0-hex0/hex0.hex` | a hex listing becomes bytes; hex0 reproduces its own binary |
| 1 | `genesis/build/hex2.bin` | labels and relative/absolute addresses; hex2 reproduces itself |
| 2 | `genesis/build/asm.bin` | the `machine x64` assembler: exact bytes, real execution, rejection of unsupported forms |
| 3 | `genesis/build/oli1.bin` | the oli-core compiler: every fixture in `genesis/3-oli1/tests` compiles, runs and exits with the expected status |
| 4 | `genesis/build/show_tokens`, `show_ast`, `show_sema` | the `olic` front end written in Oli--: tokens, AST snapshots, and the whole semantic graph — items, signatures, locals and typed bodies |

The only non-Oli-- code in the chain is POSIX shell (`genesis/hexbin.sh`
materialises the first binary once; `genesis/test.sh` orchestrates).

### 3.2 Compile and run an oli-core program **[runs]**

`oli1` reads a program on stdin and writes a native ELF64 on stdout.

```bash
genesis/build/oli1.bin < genesis/3-oli1/tests/fib.oli > /tmp/fib
chmod +x /tmp/fib
/tmp/fib; echo $?          # 55
```

Write your own:

```bash
cat > /tmp/mine.oli <<'EOF'
proc start
    entry
    permit os.syscall
    msg := "hi from Oli--\n"
    os.syscall(1, 1, msg.addr, msg.len)
    ret 0
end
EOF
genesis/build/oli1.bin < /tmp/mine.oli > /tmp/mine && chmod +x /tmp/mine && /tmp/mine
```

On a rejected program `oli1` writes `oli1: error at line N` on stderr, exits 2
and produces no output. A program that traps at run time (bounds, alignment,
zone exhausted) prints `oli: trap` and exits 3.

### 3.3 Inspect the compiler **[runs]**

```bash
genesis/build/show_tokens < examples/hello.oli   # one token per line
genesis/build/show_ast    < examples/hello.oli   # the syntax tree
genesis/build/show_sema   < examples/hello.oli   # the semantic graph, typed
genesis/build/show_oir    < examples/hello.oli   # the OIR in blocks
genesis/build/show_ssa    < examples/hello.oli   # the same, after mem2reg
genesis/build/show_opt    < examples/hello.oli   # and after the OIR passes
```

Each writes its result on stdout and diagnostics on stderr, and exits 1 if it
reported any. `show_oir` and `show_ssa` verify the OIR against
`docs/OIR_SPEC.md` §8 before printing it and exit 4 if it fails, which would
be a defect in the compiler rather than in the program.

`bin/olic` is the same compiler as a command — `olic f.oli`, `olic --check
f.oli`, `olic --show-opt f.oli` — for the PATH; in VS Code,
`.vscode/tasks.json` runs it on the current file and `tools/vscode-oli/` is
a grammar for highlighting. `tools/vscode-oli/README.md` is the step by
step guide. All of it is a convenience outside the toolchain. `show_sema` resolves `import` by reading `lib/<path>.oli`, so
run it from the repository root.

Token lines are `LINE:COL kind [value] [text]`; the tree is the S-expression
format of `tests/snapshots/*.ast`; the semantic graph is
`tests/snapshots/*.sema` exactly — every expression with its type, and with
its region when the value may point into a parameter's memory or a zone; the
OIR is `tests/snapshots/*.oir` exactly — one instruction per value, in
evaluation order, with `check.overflow` and `check.div_zero` where plain
arithmetic traps.

### 3.4 Compile a high-level program **[runs]**

```bash
genesis/build/olic < examples/hello.oli > /tmp/hello
chmod +x /tmp/hello && /tmp/hello       # Hello Oli--
```

`olic` reads the source on stdin and writes a static ELF64 on stdout — its own
encoder, its own ELF writer, no linker and no libc. It writes nothing unless
the whole pipeline agreed on the program: a front-end diagnostic or an `E0900`
from the back end stops it with exit 1.

The back end lowers procedures with SysV parameters and a result, integers,
characters, `true`/`false`, module constants, string literals, locals,
`.addr`/`.len`, the arithmetic, bit, shift and comparison operators,
`-`/`~`/`not`, the short-circuiting `and`/`or`, `wrap(e)`, the explicit
conversions, `if`/`elif`/`else`, `while`, `loop`, `break`, `continue`, calls
and `os.syscall`. Everything else — zones, views beyond a string's two words,
layouts, refs, raw memory, fallible results, `each`, `case`, `sat(e)`,
`checked(e)` and `machine` blocks — was `E0900` at that stage. Stages 4–9
lowered all of it, then a `choice` that fits a word and `machine x64`
blocks; a choice wider than eight bytes, a layout literal by value and a
machine line the encoder does not know remain `E0900`: refused, never
approximated.

### 3.5 Assemble a `machine x64` program **[runs]**

```bash
genesis/build/asm.bin < examples/genesis/hello.oli > /tmp/h.elf
chmod +x /tmp/h.elf && /tmp/h.elf      # Hello Oli--
```

### 3.6 The commands that do not exist yet **[planned]**

`olic` as a single binary with flags, and the project tool `oli`, are specified
in `docs/COMMANDS.md` but not implemented; today each front-end stage is its own
stdin→stdout driver (§3.3), because a genesis-built program cannot read its
command line yet.

| Planned command | Meaning |
|-----------------|---------|
| `olic file.oli` | compile to a native ELF — own encoder, own ELF writer, no linker |
| `olic --show-tokens/--show-ast/--show-sema/--show-oir/--show-ssa/--show-oir=opt` | the six dumps (all six exist as drivers, one per stage, reading stdin) |
| `olic --show-asm` | the bytes of every `machine x64` block, after fixups — **runs** (`tests/machine/`) |
| `olic --show-machine-ir/--show-bytes` | the remaining back-end dumps |
| `olic --check` / `--check-syntax` | analyse / parse only |
| `olic --freestanding` | no OS: own entry point, own stack — selected by `-- target: freestanding` in the source; **runs** (`tests/run/freestanding.oli`) |
| `olic --lib DIR` | where to find imported modules (today: always `lib/`) |
| `olic --explain` / `--explain-cost` | per procedure: frame, checks with their proofs, zones, calls, syscalls / the cost class of every line — **runs** (`tests/snapshots/cse.explain`) |
| `oli new/build/run/test/fmt/check/bench/doc/package/fuzz` | the project tool |

---

## 4. Lexical structure

### 4.1 Comments **[runs]**

```oli
-- to the end of the line
--- a doc comment, attached to the declaration that follows
```

A `---` comment becomes a token the tree keeps (`(doc "…")`); a `--` comment
vanishes. A doc comment that precedes no declaration is `W0001`.

### 4.2 Identifiers and paths **[runs]**

```
ident := letter { letter | digit | "_" }        -- letter = A-Z a-z _
path  := ident { "." ident }                    -- std.os, core.TrapKind, hdr.flags
```

Case-sensitive. A leading `_` marks an intentionally unused binding. After a
`.` any keyword is an ordinary member name (`msg.addr`, `Header.at`, `io.port`).

### 4.3 Keywords **[runs]**

```
module import pub proc permit calls section align entry export traps
ret fail if then elif else end while each in loop break continue
zone at from case when layout choice packed machine clobber
and or not true false none never
rw mmio addr ref view own port be le
wrap sat checked
```

Reserved for later versions and rejected as names (`E0010`): `bit bits atomic
thread extern generic const static volatile`.

Primitive type names (`u8`, `word`, …) are **not** keywords; they are ordinary
identifiers the compiler knows.

### 4.4 Literals **[runs]**

```oli
42        1_000_000     64K  2M  1G      -- decimal, digit separators, ×1024^n
0xFFFF800000000000      0b1010_1010      0o755
'a'   '\n'   '\x41'                      -- a u8 value
"text with \n \t \r \0 \\ \" \' \x41"    -- a view u8, no terminator
true   false   none
```

Integer literals have no type of their own: they take the type the context
requires. The genesis lexer accepts the full u64 range (V0 specifies u128);
overflow is `E0003`, a bad digit or suffix `E0004`.

### 4.5 Operators and punctuation **[runs]**

```
:=  <-  <~  ->  ..  :  ,  .
+  -  *  /  %
==  !=  <  <=  >  >=
&  |  ^  ~  <<  >>
(  )  [  ]  {  }
```

Maximal munch: `<-` is always the store operator, so write `x < -1` with a
space.

### 4.6 Lines and blocks **[runs]**

One statement per line; indentation is style only (the formatter uses four
spaces). A line continues onto the next when its last significant token is a
binary operator, `:=`, `<-`, `<~`, `.`, `,`, `(`, `[` or `{`, and newlines are
ignored inside unclosed brackets. A line that *starts* with an operator never
continues the previous one. Blocks close with `end`, never with braces; a `{`
left at the end of a line is `E0032`.

---

## 5. Types

### 5.1 The full set

| Type | Size / representation | Status |
|------|----------------------|--------|
| `u8 u16 u32 u64` | 1, 2, 4, 8 bytes, unsigned | **[parses]**, subset **[runs]** |
| `s8 s16 s32 s64` | 1, 2, 4, 8 bytes, signed | **[parses]** |
| `word` / `uword` | 8 bytes, signed / unsigned machine word | **[parses]** |
| `byte` / `bool` | 1 byte | **[parses]** |
| `physaddr` | 8 bytes, a physical address | **[parses]** |
| `addr T` | 8 bytes, a raw address of `T` (`addr` alone = `addr u8`) | **[runs]** |
| `view T` / `rw view T` | 16 bytes: address + length, bounds-checked | **[runs]** |
| `ref T` / `rw ref T` | 8 bytes, a reference to a place of type `T` | **[runs]** |
| `[N]T` | `N × T`, a place type only — pass a `view` or `ref` | **[parses]** |
| `layout` names | a record with fixed field offsets | **[runs]** |
| `choice` names | a tagged union: one-byte tag then the payload; one that fits eight bytes is a word | **[runs]** (≤ 8 bytes) |
| `T or E` | fallible: tag in `rax`, payload in `rdx` | **[runs]** |
| `none` | the empty type; `T or none` is the optional `T` | **[runs]** |
| `never` | the bottom type: the procedure does not return | **[parses]** |
| `zone` | 8 bytes, a zone handle | **[runs]** |
| `port T` | an I/O port (V1) | **[parses]** |
| `be T` / `le T` | an integer with a fixed byte order; a `be` field is swapped on load and store | **[runs]** |
| `mmio view/ref T` | memory-mapped I/O; never dropped by a conversion | **[parses]** |
| `own T` | reserved in V0: parsed, rejected by the checker (`E0900`) | **[parses]** |

`or` binds loosest in a type: `ref Header or E` is `(ref Header) or E`.

### 5.2 Layout of aggregates (`docs/ABI.md` §3) **[runs]**

A field's alignment is `min(size, 8)`, raised by `align N` on the field and
dropped to 1 by `packed` on the layout. The layout's alignment is the largest
field alignment, or its own `align N`; its size is rounded up to that
alignment. A choice is a one-byte tag followed by the payload at the first
offset with the payload's alignment; every variant is laid out from there and
the size is the largest variant, rounded up.

```oli
layout Header
    magic  : u32        -- @0
    length : be u16     -- @4
    flags  : u16        -- @6
end                     -- size 8, align 4

layout GdtPointer packed
    limit : u16         -- @0
    base  : addr u64    -- @2   (packed: no padding)
end                     -- size 10, align 1
```

`Name.size` and `Name.align` are compile-time constants; `Name.at(v)` makes a
`ref Name` from a `view u8`, trapping when the view is too short or misaligned.

### 5.3 Conversions

Implicit, and only when lossless: an integer literal to any integer type whose
range contains it; `uN` to a wider `uN` and to `uword`; `sN` to a wider `sN`
and to `word`; `rw view T` to `view T`, `rw ref T` to `ref T`; a `[N]T` place
to a view. `mmio` is never dropped.

Explicit forms:

```oli
u32(x)          -- lossless, verified statically           [runs]
u8.wrap(x)      -- wrapping                                [runs]
u64.bits(x)     -- reinterpret the bits                    [runs]
u8.sat(x)       -- saturating                              [runs]
u8.checked(x)   -- fallible: u8 or Overflow                [runs]
addr u8 (n)     -- integer to address; permit memory.raw   [parses]
```

`T(x)`, `T.wrap(x)` and `T.bits(x)` are compiled by `oli1` (genesis step 6f)
and checked by `olic`: `T(x)` emits nothing, `.wrap`/`.bits` truncate to the
width of `T` and extend again with its signedness. `olic` lowers `.sat` as a
clamp to the range of `T` (two compares and two selects) and `.checked` as the
fallible pair the `else` / `case` handlers of a call already resolve — the
payload is the value when it fits, the failure otherwise
(`tests/run/saturate.oli`). `oli1` compiles neither, which is why
`compiler/` does not use them.

---

## 6. Declarations

Every declaration may be prefixed with `pub` to export it from its module.

### 6.1 `module` and `import` **[runs]**

```oli
module packet_demo
import core.mem
import std.os as sys      -- `as` is parsed; the alias is not yet used
```

The `module` line is first (after any doc comment); imports come before
declarations (`E0023` otherwise). A file with no `module` line takes its
module name from the file stem. `core` is always available without an import.
The genesis loader resolves `std.os` to `lib/std/os.oli`.

### 6.2 Constants and statics **[runs]**

```oli
PAGE : uword := 4096              -- a compile-time constant
LIMIT := 100                      -- type inferred
ticks : u64 <- 0                  -- a static place with an initial value
boot_stack : [16K]u8              -- a static place, zero-filled
    section ".bss.boot"
    align 16
```

`:=` at module level is a constant; `:` with `<-` or with no value is a static
place. `section` and `align` clauses follow on their own lines. Constant
expressions evaluate integers, `Name.size`, `Name.align` and other constants.

### 6.3 `layout` **[runs]**

```oli
layout Multiboot2Header align 8
    magic        : u32
    architecture : u32
end

layout GdtPointer packed
    limit : u16
    base  : addr u64 align 8
end
```

### 6.4 `choice` **[runs]** (when it fits eight bytes)

```oli
choice ParseError
    too_short
    bad_magic { found : u32 }
end
```

A variant may carry named fields; `case` binds them by name. `olic` lowers
a choice whose image fits eight bytes as one word — the tag (the variant's
number, from 0, in declaration order) in the first byte, each field at its
offset — so `fail VARIANT { f: e }` builds the word with masks and shifts,
`case … when fail VARIANT { f }` compares the tag byte and reads each field
back at its width and sign, and `else fail` hands the word on
(`tests/run/choice.oli`). A wider choice would need memory and is `E0900`.
A field a literal or pattern names that the variant does not declare, one
named twice, or one a literal leaves out is `E0208`.

### 6.5 `proc` and its clauses **[runs]**

```oli
pub proc write_all(fd : s32, data : view u8) -> uword or os.Error
    permit os.syscall
    ...
end
```

| Clause | Meaning | Status |
|--------|---------|--------|
| `permit cap, cap` | grants capabilities (§10) | **[runs]** |
| `calls sysv` / `c` | the calling convention (default) | **[parses]** |
| `calls none` | no prologue or epilogue: only hardware statements and `machine` blocks, no parameters, no result but `never` | **[parses]** |
| `calls interrupt` | an interrupt handler (V1; `E0900` in V0) | **[parses]** |
| `section "name"` | place the code in a named section | **[parses]** |
| `align N` | align the procedure (a power of two) | **[parses]** |
| `entry` | the program starts here — exactly one per program | **[runs]** |
| `export ["sym"]` | export the symbol | **[parses]** |
| `traps` | the freestanding trap handler: `(kind : core.TrapKind, site : core.Site) -> never` — called from every trap site with the kind and the site | **[runs]** |

A duplicate, malformed or misplaced clause is `E0018`.

Entry point (`docs/design/0016-entry-everywhere.md`): the procedure carrying
`entry` starts the program wherever it appears in the file. Hosted: `-> s32`,
no parameters — the compiler adds `_start`. Freestanding: `-> never`, no
parameters. The name `main` has no special meaning.

---

## 7. Statements

| Statement | Form | Status |
|-----------|------|--------|
| binding | `x := expr` or `x : T := expr` — immutable, no address | **[runs]** |
| place | `x : T` or `x : T <- expr` — must be stored before it is read | **[runs]** |
| store | `place <- expr` | **[runs]** |
| move | `place <~ expr` — reserved for `own` (`E0900` in V0) | **[parses]** |
| expression | a call, or any fallible value that is resolved | **[runs]** |
| `ret` | `ret` or `ret expr` | **[runs]** |
| `fail` | `fail` or `fail expr`, only in a `-> T or E` procedure | **[runs]** |
| `if` | `if c` … `elif c` … `else` … `end`, or one line: `if c then stmt` | **[runs]** |
| `while` | `while c` … `end` | **[runs]** |
| `each` | `each x in v` … `end` over a view **[runs]**; over an array place or a range **[parses]** | **[runs]** |
| `loop` | `loop` … `end`, left only by `break`, `ret`, `fail` or a `never` call | **[runs]** |
| `break` / `continue` | innermost loop only | **[runs]** |
| `case` | `case e` … `when p` … `else` … `end`, exhaustive | **[runs]** for `ok`/`fail` |
| `zone` | `zone z SIZE [at a] [from s]` … `end` | **[runs]** |
| `machine` | `machine x64` … `end`, needs `permit cpu.asm`; `in REG <- e`, `out REG -> place`, `clobber`, `.label:`; assembled by `olic`'s own encoder, byte for byte the genesis assembler's subset | **[runs]** |

Control may not fall off the end of a procedure with a result (`E0230`); a
statement after `ret`, `fail`, `break`, `continue` or a `never` call is
`E0231`.

```oli
if x < -1
    x <- ~x
elif x == 0 or not (x > 3 and x < 9)
    x <- x * 2 + 1
else
    x <- sat(x - 1)
end

while i < n
    if b == 0 then continue
    i <- i + 1
end

each i in 0..buf.len
    buf[i] <- u8.wrap(i)
end
```

---

## 8. Expressions

### 8.1 Precedence, loosest first **[runs]**

| Level | Operators |
|-------|-----------|
| 1 | `expr else handler` — the fallback (§9) |
| 2 | `a .. b` — a range, only in index and `each` positions |
| 3 | `or` |
| 4 | `and` |
| 5 | `==` `!=` `<` `<=` `>` `>=` — never chained (`E0031`) |
| 6 | `\|` |
| 7 | `^` |
| 8 | `&` |
| 9 | `<<` `>>` |
| 10 | `+` `-` |
| 11 | `*` `/` `%` |
| 12 | prefix `not` `-` `~`, `addr x`, `ref x`, `rw ref x` |
| 13 | postfix `.field`, `(args)`, `[index]` |

### 8.2 Primary expressions

```oli
42  'a'  "text"  true  false  none        -- literals
name            module.name               -- names and paths
f(a, b)         f(x: a, y: b)             -- calls, all positional or all named (E0022)
v[i]            v[a..b]  v[..b]  v[a..]   -- element and subview, bounds-checked
Point { x: 0, y: 0 }                      -- a layout literal
{ 0, 1, 2 }                               -- an array literal (constants)
[a]                                       -- a raw load; requires permit memory.raw
(expr)
```

Members: `v.addr`, `v.len` on a view; `r.field` on a ref or a layout place;
`Name.size`, `Name.align`, `Name.at(v)` on a layout; `z.bytes(n)`,
`z.try_bytes(n)`, `z.make(T)` on a zone.

Arithmetic modes: `wrap(e)` **[runs]**, `sat(e)` **[runs]**, `checked(e)`
**[runs]** — `checked` yields a fallible value that must be resolved with
`else` or `case`. A 64-bit operation in either mode reads the overflow flag
the machine set (`ovf.of` in OIR); a narrower one compares the whole-word
result with the range of its type. `sat` clamps to that range, `checked`
accumulates the flags of every operator inside `e` and fails when any was set.
Overflow is otherwise a trap (`docs/design/0006-overflow-and-bounds.md`): a
program built by `olic` writes `trap: overflow at <module>:<line>` on fd 2 and
exits 134. `wrap(e)` clears that trap for every operator inside `e` and keeps
the width of the type, so the result is the value modulo 2^width. The two
traps of division stay inside `wrap`, because neither a zero divisor nor the
most negative value over -1 has a wrapped answer to name.

---

## 9. Fallible results — `T or E`, `fail`, `else`, `case` **[runs]**

A procedure that can fail returns `T or E`: `ret v` succeeds, `fail e` fails.
`E` may be any type; `none` is the empty failure, so `T or none` is the
optional `T`. Representation (`docs/ABI.md` §2): tag in `rax` (0 = ok,
1 = fail), payload in `rdx`.

A fallible value **must** be resolved where it occurs (`E0310` otherwise):

```oli
n := read(fd, buf) else fail        -- propagate to this procedure's E
n := read(fd, buf) else ret 0       -- diverge
n := read(fd, buf) else 0           -- a default value of type T
case read(fd, buf)                  -- handle both channels
when ok n
    used <- used + n
when fail e
    report(e)
end
```

`case` arms are tried in order and must be exhaustive: all variants, or `ok`
and `fail`, or an `else` arm. A `when fail` arm may bind the payload
(`when fail bad_magic {found}`); for integers and characters only `else`
provides exhaustiveness.

In the genesis compiler `oli1` the payload `T` is one word (an integer or a
`ref`), `E` is an integer type or `none`, and a fallible value is resolved at
the call site rather than stored — enough to write the compiler itself.

---

## 10. Memory model **[runs]**

`spec/OLI_MEMORY_V0.md`, `docs/MEMORY_MODEL.md`, design records 0003–0005.

### 10.1 Regions

Every value lives in a region: the **frame** (locals), a **zone**, **static**
memory, or **raw** memory. The region is part of the type's meaning, and a
reference may never outlive its region — escaping is a compile error
(`E0300`-class), not a dangling pointer.

### 10.2 Zones

```oli
zone scratch 64K                 -- hosted: mmap on entry, munmap at `end`
    pkt := scratch.bytes(PAGE)   -- bump allocation, 16-byte aligned, zeroed
    hdr := scratch.make(Header)  -- one record, zero-filled
end                              -- everything in the zone dies here

zone boot 1M at addr u8 (0x200000)   -- raw memory at a fixed address
zone kheap 4M from heap_region       -- backed by a static array
```

A zone is a bump allocator with a base, a cursor and a limit. Exhaustion traps
(`z.bytes`) or fails (`z.try_bytes`). Nothing allocated in a zone may escape
its block.

All of it runs under `olic` (`tests/run/zones.oli`): the operating-system
zone is `mmap`/`munmap`; `at ADDR` and a zone over a buffer are the triple
laid over memory that exists (`zone.new.raw`, with a bounds check that the
buffer holds the size); `from` a parent zone carves the bytes from the
parent's cursor with the parent's `zone_exhausted` trap and gives the cursor
back at `end` (`zone.end.from`); a zone inside a zone is either. A `ret`,
`fail`, `break` or `continue` inside a zone releases every zone it leaves on
that edge — the compiler emits the release before the jump, innermost first.

### 10.3 Views

A `view T` is an address and a length: a bounds-checked window over memory.
`v[i]`, `v[a..b]`, `v[..b]`, `v[a..]` are all checked; `v.addr` and `v.len`
expose the two words. `rw view T` permits stores. String literals are
read-only `view u8`.

### 10.4 References and raw memory

`ref T` is a reference to a place; `rw ref T` may be stored through. `addr T`
is a plain address with no guarantees: `[a]` loads and `[a] <- x` stores
through it, and both require `permit memory.raw`.

### 10.5 Traps

Bounds violations, misalignment, zone exhaustion, overflow in checked
arithmetic, division by zero and `unreachable` trap. Hosted programs built by
`oli1` print `oli: trap` and exit 3; freestanding programs call the procedure
marked `traps`.

---

## 11. Capabilities **[runs]**

A procedure may only do what it is permitted to do
(`docs/design/0005-capabilities.md`):

| Capability | Gates |
|------------|-------|
| `memory.raw` | `[a]` loads and stores, `addr T (n)`, `zone … at`, raw `mem.*` |
| `memory.mmio` | creating `mmio` views and refs |
| `cpu.asm` | `machine` blocks |
| `cpu.halt`, `cpu.control`, `cpu.interrupt` | halting, control registers, interrupt control |
| `io.port` | port input and output |
| `os.syscall` | raw system calls |

`permit` lists them on the procedure; the checker rejects a use without its
permit and reports an unused permit.

---

## 12. `machine` blocks **[runs]**

Inline machine code with checked operands, used by the genesis chain and by
kernel code:

```oli
proc load_gdt
permit cpu.control, cpu.asm
    machine x64
        mov  ax, 0x10
        mov  qword [rbp - 8], rax
        mov  rcx, [rax + rcx*8 + 16]
        lea  rax, [.next]
    .next:
        in   al, dx
        clobber rax, memory
    end
end
```

Lines are: `in REG <- expr` (feed a value in), `out REG -> place` (take one
out), `clobber regs`, `.label:`, or a mnemonic with operands. Operands are
registers, immediates, symbols and memory operands with base, index, scale and
displacement. The assembler validates every encoding; an unsupported operand is
rejected rather than silently ignored (`E0019` in the front end, exit 1 in
`asm`).

---

## 13. Freestanding and kernel mode **[runs]** (the M3 shape; the loader profile is planned)

The capability rules of §11 are enforced today: a raw load or store, a raw
address conversion and `zone … at` need `permit memory.raw`, and a `machine`
block needs `permit cpu.asm` — without them the compiler reports `E0401` at the
construct. Constructs the V0 front end accepts but does not implement — `own`,
`f32`/`f64`, `<~`, `port T (…)`, `mem.mmio`, `calls interrupt` — report `E0900`
rather than compiling to something approximate.

With `-- target: freestanding` at the top of the program there is no
operating system: the program supplies its own entry point (`entry` with
`-> never` and `calls none`: no frame, the first instruction of the image),
its own stack, and a `traps` procedure that receives every trap with the
kind and the `core.Site` of the line. `zone … at` and `zone … from` give
memory without an allocator, `cpu.halt()` is `hlt`, `machine` blocks give
control registers and interrupt setup, `align N` on a static is honoured
and `section ".text.boot"` puts a procedure first. `olic` compiles all of
it today: `tests/run/freestanding.oli` runs as a plain process (exiting
through a system call written in a block, since nothing else is in the
image) and `tests/freestanding/trap_line.oli` exits with the line of its
overflow, delivered to `traps`. `-- load: 0x100000` sets the load address,
a static with `section ".text.boot"` goes in front of the code, so a
Multiboot2 header is a static layout with an initialiser (`examples/kernel.oli`
— a kernel that writes COM1 through `out dx, al`, the VGA text buffer through
raw stores and reads `cpuid`; the harness checks the image structurally,
QEMU runs it). Still planned: the TOML profile file, the section order beyond
`.text.boot`, `mem.mmio`. See `docs/FREESTANDING.md` and
`docs/KERNEL_PROGRAMMING.md`; `tests/sema/ok/freestanding.oli` and
`tests/parse/ok/kernel_sketch.oli` are the reference shapes.

---

## 14. Diagnostics

Every diagnostic has a code and a position, and is printed in the format of
`spec/OLI_SYNTAX_V0.md` §8:

```
error[E0012]: expected expression
 --> stdin:2:13
  |
2 |     total <-
  |             ^
```

Implemented today **[runs]**:

| Code | Meaning |
|------|---------|
| E0001 | invalid character |
| E0002 | unterminated string literal |
| E0003 | integer literal too large |
| E0004 | bad digits or suffix on an integer literal |
| E0005 | bad character literal |
| E0006 | invalid escape sequence |
| E0010 | reserved word used as a name |
| E0011 | expected a different token |
| E0012 | expected expression |
| E0014 | missing `end` |
| E0016 | ambiguous `else` in a one-line `if … then` |
| E0017 | store target is not a place |
| E0018 | bad, duplicate or misplaced clause |
| E0019 | bad machine-block line or operand |
| E0020 | expected end of line |
| E0022 | mixed named and positional arguments |
| E0023 | import after a declaration |
| E0024 | `module` line is not first |
| E0031 | chained comparison |
| E0032 | `{` at the end of a line used as a block opener |
| E0101 | a name already in scope is shadowed |
| E0106 | constant depends on itself |
| E0110 | store into an immutable binding |
| E0111 | store into a read-only place |
| E0200 | type mismatch |
| E0201 | integer literal needs a type |
| E0202 | lossy conversion needs `wrap`, `sat` or `checked` |
| E0208 | missing, unknown or duplicate field in a literal or pattern |
| E0203 | mixed address spaces |
| E0204 | layout contains itself by value |
| E0212 | value does not fit its type |
| E0220 | read of an uninitialized place |
| E0230 | missing `ret` |
| E0231 | unreachable statement |
| E0300 | value does not outlive its region |
| E0310 | unhandled failure |
| E0311 | `case` is not exhaustive |
| E0330 | a freestanding zone needs `at` or `from` |
| E0401 | capability not permitted here |
| E0900 | feature not implemented |
| W0001 | doc comment documents nothing |

Specified and **[planned]**: E0013, E0015, E0021, E0030 (syntax); the E02xx
codes for arguments and literals (E0205–E0209); and E0602/E0603 with the rest
of the program rules. Implicit narrowing of a computed value (E0202) is no
longer gated — it is reported everywhere, and `tests/sema/err/narrow.oli`
holds it to its exact positions. The rule is absolute: an unimplemented
feature reports `E0900` — no silent fallback, and the compiler never crashes
on user input.

---

## 15. oli-core: the subset that compiles today

`oli1` (genesis layer 3, `genesis/3-oli1/SPEC.md`) compiles **oli-core**, the
smallest subset of V0 that can express the compiler itself. This is what runs
natively right now:

```
file    := [module path] { layout | proc }
proc    := "proc" NAME [ "(" p ":" T { "," p ":" T } ")" ] [ "->" T [ "or" E ] ]
           { "entry" | "calls …" | "permit …" } block "end"
stmt    := NAME ":=" expr | NAME "<-" expr | v[i] "<-" expr | "[" a "]" "<-" expr
         | r.f "<-" expr | "ret" [expr] | "fail" [expr] | call
         | "if" … ["elif" …] ["else"] "end" | "while" … "end" | "break" | "continue"
         | "zone" z SIZE ["at" a] … "end" | "case" e "when ok" … "when fail" … "end"
expr    := cmp ["else" ("fail" | "ret" [e] | e)]
cmp     := add [("=="|"!="|"<"|"<="|">"|">=") add]
add     := mul {("+"|"-") mul} ;  mul := unary {("*"|"/"|"%") unary}
unary   := "-" unary | primary
primary := int | "text" | name | name.addr | name.len | v[i] | v[a..b]
         | "[" a "]" | "(" e ")" | os.syscall(args) | NAME(args)
         | z.bytes(n) | z.make(Name) | Name.size | Name.align | Name.at(v) | r.f
```

Working: typed places (`x : T`, `x : T <- e`) and bindings, `loop`,
run-time locals in a stack frame, 64-bit integer arithmetic and
comparisons, string literals in read-only data, `if`/`while`/`break`/
`continue`, procedures with up to six argument words (SysV registers,
recursion, forward calls), zones backed by `mmap`, bounds-checked views and
subviews, raw word access, layouts with natural/packed/explicit alignment,
`ref` locals, parameters and results, fallible results with `fail`, `else`
handlers and `case`, module-level integer constants, and raw Linux syscalls.

Since step 6e the compiler's own source (`compiler/`) is valid V0 as far as
the implemented checks go: `olic` analyses all ten of its modules without a
diagnostic.

Known deviations from V0, to be closed by the self-hosted compiler: one flat
name scope per procedure; integers are untyped 64-bit words compared signed;
no `and`/`or`/`not`, no `loop`, no `each`, no bitwise operators; `rw` is not
distinguished from read-only except for static strings; `.addr`/`.len`/`[i]`/
`.f` apply to names rather than to arbitrary expressions; a zone is released at
the end of its block only, so `ret` and `break` across a zone are rejected
instead of being lowered.

---

## 16. How the compiler is built (and why it matters to you)

```
hex0  (hand-written bytes)
  └─ hex2      labels and addresses
       └─ asm         the machine x64 assembler
            └─ oli1        the oli-core compiler, written in machine x64
                 └─ olic        the V0 compiler, written in oli-core
```

`compiler/` holds `olic` itself, written in oli-core so the same source is
accepted by `oli1` and by `olic` — no porting step at self-hosting, which is
reached: `olic` compiles `olic`, and that compiler compiles it again to the
same bytes (`./genesis/test.sh` layer 6). Its modules: `io.oli`, `lex.oli`
(the V0 lexer), `diag.oli` (the §8 renderer), `ast.oli` (the node arena and
the S-expression printer), `parse.oli` (the recursive-descent parser with
recovery), `load.oli` (imports), `items.oli` (modules, layouts, choices,
constants, signatures), `sema.oli` (local tables with inferred types),
`body.oli` (expression typing, regions, conversions), `check.oli` (every
semantic rule), `oir.oli`, `cfg.oli`, `ssa.oli`, `opt.oli` (the OIR, its
blocks, SSA and passes), `x64.oli` and `elf.oli`, plus one driver per stage.

What is verified today: every AST and semantic snapshot is reproduced byte
for byte; every negative fixture reports exactly its expected diagnostics;
every construct marked **runs** in `docs/COMMANDS.md` is pinned by a program
that `olic` compiles and runs; and the self-compiled compiler compiles every
one of those programs to the same bytes as the genesis-built one. What
remains for V0 completeness: a `choice` wider than a word and `be`/`le`
fields in the back end; a linear-scan register allocator over the
callee-saved registers is in (`--explain` shows its assignment).

---

## 17. Map of the repository

| Path | Contents |
|------|----------|
| `genesis/` | the bootstrap chain: `0-hex0`, `1-hex2`, `2-asm`, `3-oli1`, `test.sh`, `hexbin.sh` |
| `compiler/` | `olic` written in oli-core (`SPEC.md` describes every module) |
| `lib/` | `core.oli`, `core/mem.oli`, `std/os.oli` — libraries written in Oli-- |
| `examples/` | `hello.oli`, `packet_demo.oli`, `genesis/hello.oli` |
| `tests/parse/ok,err` | programs that must parse, and programs with `-- expect: CODE @ L:C` |
| `tests/sema/ok,err` | the same for semantic analysis |
| `tests/run`, `tests/run/trap` | programs `olic` compiles and runs: a fixture exits 42, or writes its `.out` file and exits 0; a trap fixture prints its `-- expect:` line on fd 2 and exits 134 |
| `tests/snapshots/` | expected `--show-tokens`, `--show-ast`, `--show-sema`, `--show-oir`, `--show-ssa` and `--show-oir=opt` output |
| `spec/` | the normative V0 specification: syntax, semantics, memory |
| `docs/` | design documents; `design/` holds the numbered decision records |
| `docs/PROJECT_STATUS.md` | the verified baseline and the ordered completion gates |
| `ROADMAP.md` | the plan from here to the kernel, the libraries and the IDE |
