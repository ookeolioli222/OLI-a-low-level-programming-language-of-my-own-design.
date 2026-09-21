# Oli-- Syntax V0 (normative)

This document defines the lexical structure and grammar of Oli-- V0 — the
subset the bootstrap compiler implements for Milestones 1–3. Semantics are in
`OLI_SEMANTICS_V0.md`; memory rules in `OLI_MEMORY_V0.md`. Rationale is in
`docs/SYNTAX_EXPERIMENTS.md` and `docs/design/`.

Notation: EBNF. `X*` zero or more, `X+` one or more, `[X]` optional,
`NL` a newline token, `"kw"` a literal token.

## 1. Source text

- UTF-8. Lines end with LF or CRLF (both produce one `NL` token).
- One statement per line. The language is **not** indentation-sensitive;
  indentation is style only (the formatter uses 4 spaces).
- A line whose last significant token is a binary operator, `:=`, `<-`, `<~`, `.`,
  `,`, `(`, `[` or `{` continues on the next line. Inside unclosed `( ) [ ] { }`
  newlines are ignored. A line that *starts* with an operator never continues
  the previous line.
- Tabs are permitted as whitespace and are never significant.

## 2. Lexical structure

### 2.1 Comments
```
-- to end of line
--- doc comment attached to the following declaration (kept by tooling)
```

### 2.2 Identifiers and paths
```
ident  := letter { letter | digit | "_" }        ; letter = A–Z a–z _
path   := ident { "." ident }                     ; module.sub.name, os.WRITE, hdr.flags
```
Identifiers are case-sensitive. Leading `_` marks an intentionally unused binding.

### 2.3 Keywords (reserved in V0)
```
module import pub proc permit calls section align entry export traps
ret fail if then elif else end while each in loop break continue
zone at from case when layout choice packed machine clobber
and or not true false none never
rw mmio addr ref view own port be le
wrap sat checked
```
Reserved for later versions (cannot be used as identifiers): `bit bits atomic
thread extern generic const static volatile`.

### 2.4 Literals
```
int     := dec | hex | bin | oct
dec     := digit { digit | "_" } [ size ]
hex     := "0x" hexdigit { hexdigit | "_" }
bin     := "0b" bindigit { bindigit | "_" }
oct     := "0o" octdigit { octdigit | "_" }
size    := "K" | "M" | "G"                    ; ×1024, ×1024², ×1024³
char    := "'" ( plain | escape ) "'"           ; a u8 value (ASCII/byte)
string  := '"' { plain | escape } '"'           ; a view u8 into .rodata, no terminator
escape  := "\n" | "\t" | "\r" | "\0" | "\\" | "\"" | "\'" | "\x" hexdigit hexdigit
```
Integer literals have no type of their own; they take the type required by
context (§4.9). Floating-point literals are not in V0.

### 2.5 Operators and punctuation
```
:=  <-  <~  ->  ..  :  ,  .
+  -  *  /  %
==  !=  <  <=  >  >=
&  |  ^  ~  <<  >>
(  )  [  ]  {  }
```

### 2.6 Tokens the lexer must produce
`Ident`, `Int`, `Char`, `String`, `Keyword`, `Op`, `NL`, `Eof`. The lexer
tracks line and column of every token for diagnostics.

## 3. Grammar — module level

```
module      := [ "module" path NL ] { import } { decl }
import      := "import" path [ "as" ident ] NL

decl        := [ "pub" ] ( const_bind | static_place | proc_decl | layout_decl | choice_decl )

const_bind  := ident [ ":" type ] ":=" expr NL { place_clause NL } ; compile-time constant;
                                                                ; clauses only for aggregates (.rodata)
static_place:= ident ":" type [ "<-" expr ] NL { place_clause NL }
place_clause:= "section" string | "align" int

proc_decl   := "proc" ident [ "(" [ params ] ")" ] [ "->" type ] NL
               { proc_clause NL }
               block
               "end" NL
params      := param { "," param }
param       := ident ":" type
proc_clause := "permit" cap { "," cap }
             | "calls" ( "sysv" | "c" | "none" | "interrupt" )
             | "section" string
             | "align" int
             | "entry"
             | "export" [ string ]
             | "traps"
cap         := path                                             ; memory.raw, cpu.asm, ...

layout_decl := "layout" ident [ "packed" ] [ "align" int ] NL
               { field NL }
               "end" NL
field       := ident ":" type [ "align" int ]

choice_decl := "choice" ident NL { variant NL } "end" NL
variant     := ident [ "{" field { "," field } "}" ]
```

A file without a `module` line has the module name of its file stem.

## 4. Grammar — statements

```
block       := { stmt }
stmt        := bind | place | store | move | expr_stmt
             | if_stmt | while_stmt | each_stmt | loop_stmt
             | "break" NL | "continue" NL
             | ret_stmt | fail_stmt | zone_stmt | case_stmt | machine_stmt

bind        := ident [ ":" type ] ":=" expr NL
place       := ident ":" type [ "<-" expr ] NL
store       := lvalue "<-" expr NL
move        := lvalue "<~" expr NL
expr_stmt   := expr NL                                  ; calls; a fallible value must be resolved

lvalue      := ident
             | lvalue "." ident                         ; field of a place / ref
             | lvalue "[" expr "]"                      ; element of a view or array place
             | "[" expr "]"                             ; raw store target (addr)

if_stmt     := "if" expr NL block { "elif" expr NL block } [ "else" NL block ] "end" NL
             | "if" expr "then" stmt                    ; one-line form; no else; the stmt may not contain a fallback else
while_stmt  := "while" expr NL block "end" NL
each_stmt   := "each" ident "in" expr NL block "end" NL ; expr : view T  or  range
loop_stmt   := "loop" NL block "end" NL
ret_stmt    := "ret" [ expr ] NL
fail_stmt   := "fail" [ expr ] NL                       ; bare fail only inside an else-handler
zone_stmt   := "zone" ident expr [ "at" expr | "from" expr ] NL block "end" NL
case_stmt   := "case" expr NL { "when" pattern NL block } [ "else" NL block ] "end" NL
pattern     := "ok" [ ident ]
             | "fail" [ sub_pattern ]
             | sub_pattern
sub_pattern := ident [ "{" ident { "," ident } "}" ]    ; variant with bound payload fields
             | [ "-" ] int | char | "true" | "false" | "none"

machine_stmt:= "machine" ident NL { mline NL } "end" NL ; ident = x64
mline       := "in" reg "<-" expr
             | "out" reg "->" lvalue
             | "clobber" ( reg | "memory" ) { "," ( reg | "memory" ) }
             | label ":"                                 ; local label  .name:
             | mnemonic [ operand { "," operand } ]      ; validated by the encoder
```

## 5. Grammar — expressions

Precedence from lowest to highest; all binary operators are left-associative
except comparisons, which do not chain.

```
expr        := fallback
fallback    := range [ "else" handler ]
handler     := "fail" | "ret" [ expr ] | range          ; propagate | diverge | default value
range       := or_expr [ ".." [ or_expr ] ]              ; a..b, a.. (to end); only in index/each positions
or_expr     := and_expr { "or" and_expr }
and_expr    := cmp_expr { "and" cmp_expr }
cmp_expr    := bor_expr [ ( "==" | "!=" | "<" | "<=" | ">" | ">=" ) bor_expr ]
bor_expr    := bxor_expr { "|" bxor_expr }
bxor_expr   := band_expr { "^" band_expr }
band_expr   := shift_expr { "&" shift_expr }
shift_expr  := add_expr { ( "<<" | ">>" ) add_expr }
add_expr    := mul_expr { ( "+" | "-" ) mul_expr }
mul_expr    := unary { ( "*" | "/" | "%" ) unary }
unary       := ( "not" | "-" | "~" ) unary
             | "addr" unary | "ref" unary | "rw" "ref" unary
             | postfix
postfix     := primary { "." ident | "(" [ args ] ")" | "[" expr "]" }
primary     := int | char | string | "true" | "false" | "none"
             | path
             | "(" expr ")"
             | "[" expr "]"                               ; raw load through an addr
             | ( "wrap" | "sat" | "checked" ) "(" expr ")"
             | type "(" expr ")"                          ; conversion (lossless)
             | type "." ( "wrap" | "sat" | "checked" | "bits" ) "(" expr ")"
             | path "{" [ field_init { "," field_init } ] "}"   ; layout / variant literal
             | "{" [ expr { "," expr } ] "}"              ; array literal
args        := arg { "," arg }
arg         := [ ident ":" ] expr                         ; named arguments allowed
field_init  := ident ":" expr
```

Intrinsic calls use ordinary call syntax on reserved paths: `os.syscall(...)`,
`cpu.halt()`, `mem.copy(...)`, `Layout.at(v)`, `Layout.size`, `z.bytes(n)`,
`z.make(T)`, `v.len`, `v.addr`.

## 6. Grammar — types

```
type        := prim
             | path                                       ; layout or choice name
             | "[" int "]" type                            ; fixed array (a place type)
             | [ "mmio" ] [ "rw" ] ( "view" | "ref" ) type
             | "addr" [ type ]                             ; addr alone = addr u8
             | "own" type                                  ; reserved in V0 (parsed, rejected by the checker)
             | "port" prim                                 ; port u8 | u16 | u32
             | ( "be" | "le" ) prim                        ; integer with fixed byte order
             | "zone"
             | type "or" type                              ; fallible: T or E
             | "none"                                       ; the empty type; `T or none` = optional T
             | "never"
prim        := "u8" | "u16" | "u32" | "u64" | "s8" | "s16" | "s32" | "s64"
             | "word" | "uword" | "byte" | "bool" | "physaddr"
```

`f32`/`f64` are reserved type names (V2). `or` binds loosest: `ref Header or E`
is `(ref Header) or E`.

## 7. Disambiguation rules

1. `else` at the start of a line belongs to an `if`/`case`; `else` inside a line is a
   fallback handler. A fallback `else` inside a one-line `if ... then` is an error.
2. `[` at the start of an expression is a raw load; after an expression it is an index.
3. `name := ...` at module level is a compile-time constant; inside a procedure it is a binding.
4. `T(x)` is a conversion only when `T` is a type name; otherwise it is a call.
5. `a or b` is a boolean expression; `A or B` in a type position is a fallible type.
6. In a `machine` block, the first identifier of a line decides: `in`/`out`/`clobber`
   are directives, `.name:` is a label, anything else is a mnemonic. `in REG <- e` is the
   directive; `in REG, REG` is the x86 instruction (the token after the register decides).
7. After `.`, any keyword is an ordinary member name: `v.addr`, `Header.at(v)`,
   `Header.align`, `io.port`. Keywords are reserved only at the start of a name.
8. `addr` followed by a primitive type, by `(`, or by `Name (` starts a conversion
   (`addr u8 (n)`, `addr (n)`, `addr Regs (n)`); otherwise it is the address-of operator.

## 8. Diagnostics format (required of the implementation)

```
error[E0012]: expected expression
 --> test.oli:12:15
   |
12 | total <-
   |         ^ expression expected here
```

Every diagnostic has a code, a file/line/column, the source line and a caret
range. The parser recovers at the next line, `end` or declaration keyword and
continues, so one file reports all its syntax errors. The lexer/parser codes
(`E0001`–`E0032`, `W0001`) are listed in `docs/COMPILER_ARCHITECTURE.md`.

## 9. Milestone programs

### M1 — hello (must compile and run with `olic hello.oli && ./hello`)

```oli
module hello
import std.os

proc start -> s32
    entry
    permit os.syscall
    msg := "Hello Oli--
"
    os.syscall(os.WRITE, 1, msg.addr, msg.len)
    ret 0
end
```

### M2 — exercises every V0 construct

See `docs/SYNTAX_EXPERIMENTS.md` §3 (`packet_demo`, Proposal A), which is the
V0 reference program for the integration test suite.

## 10. Style (informative)

4-space indentation; clauses at the same indentation as the body; one blank
line between procedures; `end` is always on its own line. The formatter
(`oli fmt`) is defined by this document plus a printer for the AST; it never
changes tokens.

## 11. Revision history

| Date | Change | Reason |
|------|--------|--------|
| 2026-09-18 | V0 grammar written | Phase 0 |
| 2026-09-19 | §7 rules 7–8: keywords after `.` are member names; `addr` conversion forms | `v.addr`, `Header.at`, `io.port` did not parse |
| 2026-09-19 | `const_bind` accepts `section`/`align` clauses | aggregate constants live in `.rodata` and need placement (Multiboot header) |
| 2026-09-19 | `sub_pattern` accepts `- int` | negative literals in `case` on signed values |
| 2026-09-19 | `none` added to `type` | `T or none` is the documented optional type |
| 2026-09-19 | §1 continuation rule made precise | implementation |
| 2026-09-20 | §9 M1 program starts at `proc start` with `entry` | design 0016 |
