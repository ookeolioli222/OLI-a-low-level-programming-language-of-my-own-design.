# Oli-- Syntax Experiments

Three competing, deliberately different surface syntaxes for the same
semantic core, one complete example program each, an evaluation against
eight criteria, and the selection of the V0 syntax.

## 0. Originality review (before designing)

| # | Question | Answer before starting |
|---|----------|------------------------|
| 1 | Am I copying C, Rust, Zig or C++? | Banned from all three proposals: `{ }` blocks, `fn`, `let`, `mut`, `struct`, `*p`, `&x`, `->` as member access, `?` propagation, `//` comments, `;` terminators, `switch`, `#include`, `unsafe {}` as the only safety boundary. |
| 2 | Does each feature fit the Oli-- philosophy? | Every construct must map to a machine fact (§1 of `LANGUAGE_VISION.md`). |
| 3 | Can we make a simpler mechanism? | Zones + views + escape analysis replace a borrow checker; capabilities replace a single `unsafe`. |
| 4 | Same hardware control with less code? | `machine` blocks with typed operands replace GCC-style constraint strings. |
| 5 | Does the programmer know what happens to memory? | Three operators (`:=` bind, `<-` store, `<~` move) instead of one `=`. |
| 6 | Hidden allocations or copies? | None allowed; every copy is a written operation or a reported parameter pass. |
| 7 | Predictable behavior? | Every construct gets a cost class tag. |
| 8 | Safer than C? | Bounds, overflow, alignment and escape are checked or proven; raw access needs a capability. |
| 9 | Simpler than Rust? | No lifetime syntax; zone names are the only "lifetimes" and only when the default rule is too conservative. |
| 10 | More its own language? | Zones, views, permits, layouts, `ret`/`fail`, `machine` blocks, cost classes. |

## 1. The shared semantic core

All three proposals express exactly the same semantics. Only the surface differs.

| Concept | Semantics |
|---------|-----------|
| binding | immutable name for a value; no address |
| place | typed slot in frame / zone / static memory; stores allowed |
| move | transfer of an `own` value; source dies |
| procedure | one `call`/`ret`; header declares parameters, result, capabilities |
| fallible result | `T or E`: success payload or failure payload, tag in a register |
| zone | lexically scoped bump region; released at block end; no escapes |
| view | `(addr, len)`; read-only unless `rw`; volatile if `mmio` |
| raw memory | `addr T`; dereference requires `memory.raw` |
| layout | structural type; fields have offsets; `packed`, `align`, `be`/`le` fields |
| choice | tagged union of named variants with optional payload |
| capability | per-procedure permit list |
| syscall | intrinsic `os.syscall(nr, args...)`, requires `os.syscall` |
| machine block | inline instructions with declared `in`/`out`/`clobber` |
| overflow | plain operators trap; `wrap(..)`, `sat(..)`, `checked(..)` select other semantics |

## 2. The benchmark program

Each proposal implements the same program, `packet_demo`, which must contain:
binding · mutable state · procedure · conditions · loop · zone · view · raw memory ·
structural data · error handling · module/import · syscall · inline machine operation.

The program: build a packet in a zone, parse its header through a zero-copy
layout view, checksum the payload, read the CPU vendor string with `cpuid`,
write results with the Linux `write` syscall, and peek one byte through a raw address.

## 3. Proposal A — "Ledger"

**Shape.** Keyword-opened, `end`-closed blocks. One statement per line, newline
terminated. Words for machine operations, symbols only for the three memory
operators (`:=` bind, `<-` store, `<~` move) and arithmetic. Procedure headers
are followed by *clauses* (`permit`, `section`, `align`, `entry`, `calls`) that
form a readable contract before the body.

```oli
-- packet_demo.oli — Proposal A
module packet_demo

import core.mem
import std.os

PAGE : uword := 4096                       -- module-level binding = compile-time constant

layout Header
    magic  : u32                           -- native byte order
    length : be u16                        -- big-endian on the wire; loads emit a byte swap
    flags  : u16
end

choice ParseError
    too_short
    bad_magic { found : u32 }
end

proc checksum(data : view u8) -> u32
    total : u32 <- 0                       -- a place (frame slot), written in the loop
    each b in data
        total <- wrap(total + b)           -- explicit wrapping arithmetic
    end
    ret total
end

proc parse_header(pkt : view u8) -> ref Header or ParseError
    if pkt.len < Header.size then fail too_short
    hdr := Header.at(pkt)                  -- zero-copy reinterpretation, one alignment CHECK
    if hdr.magic != 0x4F4C4931 then fail bad_magic { found: hdr.magic }
    ret hdr
end

proc write_all(fd : s32, data : view u8) -> uword or os.Error
permit os.syscall
    done : uword <- 0
    while done < data.len
        rest := data[done..]               -- sub-view: ZERO
        n := os.syscall(os.WRITE, fd, rest.addr, rest.len)
        if n < 0 then fail os.Error { code: n }
        done <- done + uword.bits(n)      -- same-width reinterpretation; n >= 0 was checked
    end
    ret done
end

proc peek(a : addr u8) -> u8
permit memory.raw
    ret [a]                                -- unchecked machine load
end

proc cpu_vendor(out : rw view u8)
permit cpu.asm
    b : u32
    c : u32
    d : u32
    machine x64
        in  eax <- 0
        cpuid
        out ebx -> b
        out ecx -> c
        out edx -> d
        clobber eax
    end
    mem.put_u32(out[0..4], b)
    mem.put_u32(out[4..8], d)
    mem.put_u32(out[8..12], c)
end

proc main -> s32
entry
permit os.syscall
    zone scratch 64K                       -- hosted: one mmap SYSCALL; freed as a whole at end
        pkt := scratch.bytes(PAGE)         -- rw view u8, 4096 bytes: ZONE
        mem.put_u32(pkt[0..4], 0x4F4C4931)
        mem.put_be16(pkt[4..6], 12)
        pkt[6] <- 0
        pkt[7] <- 0

        case parse_header(pkt[0..12])
        when ok hdr
            sum := checksum(pkt[8 .. 8 + uword(hdr.length)])
            if sum == 0
                write_all(1, "empty\n") else ret 1
            end
        when fail too_short
            write_all(2, "short\n") else ret 1
        when fail bad_magic { found }
            write_all(2, "bad magic\n") else ret 1
        end

        vendor := scratch.bytes(12)
        cpu_vendor(vendor)
        write_all(1, vendor) else ret 1
        write_all(1, "\n") else ret 1

        first := peek(pkt.addr)            -- raw address of the view's first byte
        if first != 0x31 then ret 2
    end
    ret 0
end
```

**Notes on A.**
- `if c then stmt` is the one-line form; the multi-line form has no `then` and ends with `end`.
  A one-line `if` has no `else` branch, and a fallback `else` inside it is a compile error
  ("ambiguous else - use the block form"), so `else` never has two readings on one line.
- A is statement-oriented: there is no `if` *expression*. `max` is written as
  `if a > b then ret a` followed by `ret b`.
- `x else handler` resolves a fallible expression: `else fail` propagates, `else ret 1`
  diverges, `else 0` supplies a default value.
- `[a]` is the raw load; `v[i]` is the bounds-checked element; `v[a..b]` is a sub-view.
  Brackets always mean "memory access"; the operand decides whether it is checked.
- The parser is keyword-directed; `end` gives trivial error recovery.

## 4. Proposal B — "Offside"

**Shape.** Indentation defines blocks (offside rule, no `end`). Everything is a
binding: procedures, layouts and choices are *values* bound with `:=`, so there
is exactly one declaration form. Expression-oriented: the last expression of a
procedure body is its result. Symbols preferred over words; `T:` postfix
conversions; `T | E` for fallible results; `:` opens an inline block.

```oli
# packet_demo.oli — Proposal B
module packet_demo
import core.mem
import std.os

PAGE := 4096:uword

Header := layout
  magic:  u32
  length: be u16
  flags:  u16

ParseError := choice
  too_short
  bad_magic {found: u32}

checksum := proc (data: view u8) -> u32
  total: u32 <- 0
  each b in data
    total <- total +~ b            # +~ wrapping add, +^ saturating, +? checked
  total                            # result = last expression

parse_header := proc (pkt: view u8) -> ref Header | ParseError
  if pkt.len < Header.size: fail too_short
  hdr := Header.at(pkt)
  if hdr.magic != 0x4F4C4931: fail bad_magic {found: hdr.magic}
  hdr

write_all := proc (fd: s32, data: view u8) -> uword | os.Error  permit os.syscall
  done: uword <- 0
  while done < data.len
    rest := data[done..]
    n := os.syscall(os.WRITE, fd, rest.addr, rest.len)
    if n < 0: fail os.Error {code: n}
    done <- done + uword.bits(n)
  done

peek := proc (a: addr u8) -> u8  permit memory.raw
  [a]

cpu_vendor := proc (out: rw view u8)  permit cpu.asm
  b, c, d: u32
  asm x64
    eax <- 0
    cpuid
    b <- ebx
    c <- ecx
    d <- edx
    clobber eax
  mem.put_u32(out[0..4], b)
  mem.put_u32(out[4..8], d)
  mem.put_u32(out[8..12], c)

main := proc -> s32  permit os.syscall
  zone scratch 64K
    pkt := scratch.bytes(PAGE)
    mem.put_u32(pkt[0..4], 0x4F4C4931)
    mem.put_be16(pkt[4..6], 12)
    pkt[6] <- 0
    pkt[7] <- 0

    case parse_header(pkt[0..12])
      ok hdr:
        sum := checksum(pkt[8 .. 8 + hdr.length:uword])
        if sum == 0: write_all(1, "empty\n") !! ret 1
      fail too_short:
        write_all(2, "short\n") !! ret 1
      fail bad_magic {found}:
        write_all(2, "bad magic\n") !! ret 1

    vendor := scratch.bytes(12)
    cpu_vendor(vendor)
    write_all(1, vendor) !! ret 1
    write_all(1, "\n") !! ret 1

    first := peek(pkt.addr)
    if first != 0x31: ret 2
  0
```

**Notes on B.**
- `!!` is the fallback operator (`x !! ret 1`, `x !! fail`, `x !! 0`), chosen so it
  cannot collide with `if`/`else`.
- `asm x64` uses the same `<-` store operator for register moves in both directions:
  `eax <- 0` loads a register from a value, `b <- ebx` stores a register to a place.
  No `in`/`out` words: the direction is the position of the register.
- Blocks are values; `proc`, `layout`, `choice`, `zone` are expression forms.
- Indentation must be spaces; tabs are a lexical error.

## 5. Proposal C — "Form"

**Shape.** Every construct is a parenthesized *form* `(head operands...)`.
One grammar rule for the whole language, including types, layouts, patterns
and machine blocks. Dotted symbols (`hdr.magic`, `os.WRITE`) are single
tokens. Because code is data, compile-time code generation needs no second
language: a compile-time procedure returns forms.

```oli
;; packet_demo.oli — Proposal C
(module packet_demo)
(import core.mem)
(import std.os)

(bind PAGE uword 4096)

(layout Header
  (magic  u32)
  (length (be u16))
  (flags  u16))

(choice ParseError
  too_short
  (bad_magic (found u32)))

(proc checksum ((data (view u8))) u32
  (place total u32 0)
  (each b data
    (store total (wrap (+ total b))))
  total)

(proc parse_header ((pkt (view u8))) (or (ref Header) ParseError)
  (if (< pkt.len Header.size) (fail too_short))
  (bind hdr (Header.at pkt))
  (if (!= hdr.magic 0x4F4C4931) (fail (bad_magic (found hdr.magic))))
  hdr)

(proc write_all ((fd s32) (data (view u8))) (or uword os.Error)
  (permit os.syscall)
  (place done uword 0)
  (while (< done data.len)
    (bind rest (sub data done))
    (bind n (os.syscall os.WRITE fd rest.addr rest.len))
    (if (< n 0) (fail (os.Error (code n))))
    (store done (+ done (uword.bits n))))
  done)

(proc peek ((a (addr u8))) u8
  (permit memory.raw)
  (load a))

(proc cpu_vendor ((out (rw view u8)))
  (permit cpu.asm)
  (place b u32) (place c u32) (place d u32)
  (machine x64
    (in eax 0)
    (cpuid)
    (out ebx b) (out ecx c) (out edx d)
    (clobber eax))
  (mem.put_u32 (sub out 0 4) b)
  (mem.put_u32 (sub out 4 8) d)
  (mem.put_u32 (sub out 8 12) c))

(proc main () s32
  (permit os.syscall)
  (zone scratch 64K
    (bind pkt (scratch.bytes PAGE))
    (mem.put_u32 (sub pkt 0 4) 0x4F4C4931)
    (mem.put_be16 (sub pkt 4 6) 12)
    (store (at pkt 6) 0)
    (store (at pkt 7) 0)
    (case (parse_header (sub pkt 0 12))
      ((ok hdr)
        (bind sum (checksum (sub pkt 8 (+ 8 (uword hdr.length)))))
        (if (== sum 0) (else (write_all 1 "empty\n") (ret 1))))
      ((fail too_short) (else (write_all 2 "short\n") (ret 1)))
      ((fail (bad_magic found)) (else (write_all 2 "bad magic\n") (ret 1))))
    (bind vendor (scratch.bytes 12))
    (cpu_vendor vendor)
    (else (write_all 1 vendor) (ret 1))
    (else (write_all 1 "\n") (ret 1))
    (bind first (peek pkt.addr))
    (if (!= first 0x31) (ret 2)))
  0)
```

**Notes on C.**
- `(else expr handler)` is the fallback form; `(load a)` / `(store place v)` are the
  memory operations; `(sub view a b)` is a sub-view; `(at view i)` an element place.
- The reader/lexer is ~100 lines; the parser is the reader. Error recovery is
  by paren balance only.
- Machine blocks are ordinary forms, so the same reader validates them.

## 6. Evaluation

Scores 1 (worst) – 5 (best). Higher is always better, so "parser complexity"
scores the *simplicity* of the parser and "ambiguity" scores its *absence*.

| Criterion | A Ledger | B Offside | C Form | Reasoning |
|-----------|:--------:|:---------:|:------:|-----------|
| readability | **5** | 4 | 2 | A: explicit block closers keep 200-line kernel procedures navigable; clauses read as a contract. B: excellent for short procedures, hard to follow across deep nesting and long `case` arms. C: paren density hides structure; field access and arithmetic become prefix noise. |
| parser complexity | 4 | 3 | **5** | A: keyword-directed recursive descent, NL tokens, no lookahead beyond one token; `end` is a resync point. B: needs an INDENT/DEDENT layer, tab policy, continuation rules, and inline-`:` blocks; formatters and generators must reason about whitespace. C: the reader is the parser. |
| ambiguity | 4 | 3 | **5** | A: one documented conflict (`else` on a one-line `if`) resolved by a rule. B: `x: T` type annotation vs `x:` inline block vs `v:T` conversion share a character; multi-line expressions inside indented blocks need continuation rules. C: none. |
| low-level usefulness | **5** | 4 | 4 | A: `machine` blocks read like assembly listings; clauses (`section`, `calls`, `align`, `entry`) map directly to ELF/ABI knobs. B: `asm` with positional `<-` is elegant but hides in/out direction in long blocks. C: fine but verbose for register-level code. |
| typing speed | 3 | **5** | 2 | A: `end` and `then` cost keystrokes. B: minimal. C: parens everywhere. |
| machine-model visibility | **5** | 4 | 4 | A: `zone`, `view`, `ref`, `addr`, `[a]`, `wrap(..)`, `permit` are words the `--explain` output can echo verbatim. B: `+~`, `+^`, `+?`, `!!` are compact but must be learned as a cipher. C: uniform but neutral. |
| originality | **4** | 3 | 3 | A: the bind/store/move triad, permits, zones, `ret`/`fail`, native machine blocks form a recognizable whole; `end` blocks recall Lua/Ruby/Ada but the content does not. B: reads as Nim/Python at a glance. C: reads as Lisp at a glance. |
| scalability | **5** | 3 | 3 | A: error recovery at `end`, stable formatting, diff-friendly, tooling-friendly. B: whitespace-sensitive code is fragile under generation, copy-paste and merge. C: mismatched parens cascade; large files need editor support. |
| **total** | **35** | 29 | 28 | |

### Against the selection criteria of the directive

| Criterion | A | B | C |
|-----------|:-:|:-:|:-:|
| originality | 4 | 3 | 3 |
| low-level control | 5 | 4 | 4 |
| readability | 5 | 4 | 2 |
| minimal syntax noise | 3 | 5 | 2 |
| parser simplicity | 4 | 3 | 5 |
| predictable machine cost | 5 | 5 | 5 |
| OS development | 5 | 4 | 3 |
| **total** | **31** | 28 | 24 |

Machine-cost predictability is a property of the shared semantics, so all three tie there.

### Where A is weakest, and what is borrowed to fix it

A loses only on **syntax noise**. Three ideas from B and C are adopted into A
without changing its character:

1. From B: the one-line guard `if cond then stmt` (already in A) and bracket
   sub-views `v[a..b]` instead of `v.view(a..b)` — brackets are the universal
   "memory access" notation, so no new method is needed.
2. From B: `T(x)` conversion syntax stays a call, but the *mode* words are shared with
   arithmetic (`u8.wrap(x)`, `u8.sat(x)`, `u8.checked(x)`), so there is one vocabulary
   for overflow everywhere.
3. From C: the `machine` sub-language is *data*: mnemonic lines are parsed by the
   same tokenizer, validated by the encoder, and printed back by `--show-asm`
   unchanged. Compile-time evaluation (V1) will reuse the ordinary Oli-- expression
   grammar, not a second language.

Rejected from B: implicit "last expression is the result" (hides a `ret`; makes a
stray expression statement change the result type) and symbolic overflow operators
(`+~ +^ +?`: unreadable in crypto-heavy code with many operators per line).
Rejected from C: prefix arithmetic and the form-only surface.

## 7. Decision

**Proposal A ("Ledger") with the borrowings of §6 is the Oli-- V0 syntax.**
It is specified normatively in `spec/OLI_SYNTAX_V0.md`. The decision record is
`docs/design/0010-block-syntax-selection.md`.

## 8. Originality review (after)

| # | Question | Answer after selection |
|---|----------|------------------------|
| 1 | Copying C/Rust/Zig/C++? | No `{}` blocks, no `fn`/`let`/`struct`/`unsafe`, no `*`/`&` pointer sigils, no `?`. Universal words kept where the concept is universal: `if`, `else`, `while`, `break`, `continue`, `import`, `true`, `false`. |
| 2 | Fits the philosophy? | Every keyword maps to a row of the machine table in `LANGUAGE_VISION.md` §3. |
| 3 | Simpler mechanism? | One declaration shape per concept; three memory operators; no lifetimes. |
| 4 | Less code for the same control? | `permit` + `machine` + clauses replace attribute soup and constraint strings. |
| 5 | Programmer knows what happens to memory? | `:=` / `<-` / `<~`, `zone`, `view`, `[a]` are all visible. |
| 6 | Hidden allocations/copies? | Only pass-by-value of a layout copies, and `--explain-cost` reports it. |
| 7 | Predictable? | Cost classes on every construct. |
| 8 | Safer than C? | Yes: checked bounds/overflow/alignment/escape, capabilities. |
| 9 | Simpler than Rust? | Yes for the target domain: no borrow checker, no lifetimes, no traits in V0. |
| 10 | More its own language? | Yes. The remaining resemblance (`end` blocks) is a deliberate readability choice with a written rationale. |
