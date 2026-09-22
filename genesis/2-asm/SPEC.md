# genesis/2-asm — the Oli-- assembler: grammar and x86-64 encoding table

This is the specification of `asm`, layer 2 of the genesis chain (design 0017):
the last hand-encoded tool, written in `hex2` notation, that assembles the
Oli-- `machine x64` sub-language into a runnable ELF64. It is also the
normative definition of what a `machine x64` block may contain (design 0009),
so `spec/` refers here for instruction encodings.

## Provenance of this table

The 83 encoding forms below were derived and verified by the `asm-encoding-spec`
workflow: 6 agents produced the forms from the Intel SDM, then **3 independent
byte-exact re-derivations per form** (distinct lenses: opcode-table, operand-first,
disassembler-prediction) voted — every form has >=2 agreeing derivations and a
verifier consensus equal to the claim. The table was then **confirmed on a real
CPU**: representative encodings are composed into ELF programs by our own `hex0`
and run (`genesis/2-asm/tests/`, harness `genesis/test.sh` layer 2) — all exit 42.
No external assembler or disassembler exists in the toolchain or was used.

## 1. Source grammar (EBNF)

```ebnf
(* ===== Oli-- GENESIS ASSEMBLER SOURCE SUBSET (layer 2 = genesis/2-asm/asm.hex2) =====
   Line-oriented and stateful, so a hand-written hex2 parser can dispatch on the
   first token of each line. A logical line ends at LF. Tokens are whitespace-
   separated (any run of SPACE/TAB). "--" begins a comment to end of line.
   Blank and comment-only lines are ignored everywhere. Indentation is
   insignificant. Case-sensitive. Consistent with docs/design/0013 (newline-
   terminated statements) and 0009 (machine blocks). *)

module       = module-line , { blank | comment-line | data-block | proc-block } ;
module-line  = "module" , ws , dotted-ident , eol ;
dotted-ident = ident , { "." , ident } ;               (* becomes the ELF symbol name; dots are legal, ABI.md sec 4 *)

(* ---- static data (byte arrays, 64-bit words, ASCII strings, label addresses) ---- *)
data-block   = [ "pub" , ws ] , section-kw , ws , dotted-ident , eol ,
               { data-entry } ,
               "end" , eol ;                            (* block name = one global data symbol at its start *)
section-kw   = "rodata" | "data" | "bss" ;              (* selects the .rodata / .data / .bss placement *)
data-entry   = byte-entry | word-entry | ascii-entry | asciiz-entry
             | addr-entry | res-entry | comment-line | blank ;
byte-entry   = "byte"   , ws , int  , { "," , int }  , eol ;   (* each value 0..255, 1 byte *)
word-entry   = "word"   , ws , int  , { "," , int }  , eol ;   (* each value u64, 8 bytes little-endian *)
ascii-entry  = "ascii"  , ws , string , eol ;                  (* raw bytes, no terminator *)
asciiz-entry = "asciiz" , ws , string , eol ;                  (* raw bytes then one 0x00 *)
addr-entry   = "addr"   , ws , dotted-ident , eol ;            (* 64-bit ABSOLUTE address of a symbol -> abs64 fixup *)
res-entry    = "res"    , ws , int , eol ;                     (* reserve N zero bytes (memsz only in bss) *)

(* ---- procedures: 'proc NAME calls none ... end', body = one or more machine blocks ---- *)
proc-block   = "proc" , ws , dotted-ident , eol ,
               { proc-clause } ,
               machine-block , { machine-block } ,
               "end" , eol ;                            (* proc name = one global code symbol at its entry *)
proc-clause  = ( "calls" , ws , "none"                  (* the only ABI accepted by the genesis assembler *)
               | "entry"                                (* this proc's address -> ELF e_entry (design 0016) *)
               | "pub"                                  (* STB_GLOBAL in the optional .symtab; else cosmetic *)
               | "export" , [ ws , string ] ) , eol ;
machine-block = "machine" , ws , "x64" , eol ,
                { instr-line | label-line | comment-line | blank } ,
                "end" , eol ;
label-line   = "." , ident , ":" , eol ;               (* local label, scoped to the enclosing proc *)

(* ---- instructions (the ~40 forms; byte encodings are owned by the other groups) ---- *)
instr-line   = mnemonic , [ ws , operand , { "," , operand } ] , eol ;
mnemonic     = "mov"|"lea"|"movzx"|"movsx"|"add"|"sub"|"and"|"or"|"xor"|"cmp"
             | "test"|"shl"|"shr"|"sar"|"inc"|"dec"|"neg"|"not"|"imul"|"mul"
             | "div"|"idiv"|"cqo"|"push"|"pop"|"jmp"|"call"|"ret"|"syscall"
             | "je"|"jne"|"jz"|"jnz"|"jl"|"jle"|"jg"|"jge"|"jb"|"jbe"|"ja"|"jae"|"js"|"jns" ;
operand      = register | mem | immediate | ref-target ;
ref-target   = dotted-ident            (* cross-procedure / data symbol *)
             | "." , ident ;           (* local label in the current proc *)
register     = r64 | r32 | r16 | r8 ;
r64          = "rax"|"rcx"|"rdx"|"rbx"|"rsp"|"rbp"|"rsi"|"rdi"|"r8"|"r9"|"r10"|"r11"|"r12"|"r13"|"r14"|"r15" ;
r32          = "eax"|"ecx"|"edx"|"ebx"|"esp"|"ebp"|"esi"|"edi"|"r8d"|"r9d"|"r10d"|"r11d"|"r12d"|"r13d"|"r14d"|"r15d" ;
r16          = "ax"|"cx"|"dx"|"bx"|"sp"|"bp"|"si"|"di"|"r8w"|"r9w"|"r10w"|"r11w"|"r12w"|"r13w"|"r14w"|"r15w" ;
r8           = "al"|"cl"|"dl"|"bl"|"spl"|"bpl"|"sil"|"dil"|"r8b"|"r9b"|"r10b"|"r11b"|"r12b"|"r13b"|"r14b"|"r15b" ;
mem          = "[" , mem-body , "]" ;
mem-body     = base , [ "+" , index , [ "*" , scale ] ] , [ "+" , disp ]
             | dotted-ident ;          (* absolute memory operand [symbol] -> abs32sx disp32 *)
base         = r64 ;
index        = r64 ;                   (* rsp is not a legal index, per SIB encoding *)
scale        = "1" | "2" | "4" | "8" ;
disp         = int ;
immediate    = int ;
int          = [ "-" ] , ( hex-int | dec-int ) ;
hex-int      = "0x" , hex-digit , { hex-digit } ;
dec-int      = digit , { digit } ;
string       = '"' , { str-char } , '"' ;   (* escapes: \n \t \0 \\ \" and \xHH *)
ident        = ( letter | "_" ) , { letter | digit | "_" } ;   (* <= 31 chars, matching hex2's name buffer *)
comment-line = ws-opt , comment , LF ;
blank        = ws-opt , LF ;
eol          = ws-opt , [ comment ] , LF ;
comment      = "--" , { ? any char except LF ? } ;
ws           = ( " " | TAB ) , { " " | TAB } ;
ws-opt       = { " " | TAB } ;
```

## 2. Directives

Every line is dispatched on its first token; there are no macros, no expressions
beyond base+index*scale+disp, and no conditional assembly. Keyword catalogue:

STRUCTURAL
  module NAME            first non-blank line; sets the module prefix for symbol names.
  proc NAME             opens a procedure; NAME is a global code symbol at the proc entry.
  calls none            required proc clause; the genesis assembler supports only this ABI.
  entry                 proc clause; records this proc as e_entry (design 0016). Exactly one
                        proc in the module may carry it; zero is an error (exit 5).
  pub / export [STR]    proc/data binding clauses; affect only the optional .symtab (STB_GLOBAL
                        and the exported name). Ignored for execution since output is a single
                        non-linked executable.
  machine x64           opens an instruction block inside a proc; only "x64" is accepted.
  end                   closes the innermost open construct (machine block, proc, or data block);
                        the assembler keeps a 1-deep state (data-block XOR proc, and within a
                        proc a machine-block flag), so "end" is unambiguous.
  .name:                defines a local label at the current .text offset, scoped to the proc.

DATA PLACEMENT (choose the section, then list entries; block name = symbol at offset 0)
  rodata NAME ... end   read-only initialized bytes -> .rodata (PT_LOAD R).
  data   NAME ... end   writable initialized bytes  -> .data   (PT_LOAD R+W).
  bss    NAME ... end   zero-initialized reservation -> .bss    (memsz only, no file bytes).

DATA ENTRIES
  byte   v[,v...]       one u8 each (0..255); the primitive for hand-laid tables and headers.
  word   v[,v...]       one u64 each, little-endian; for 64-bit constants and pointers.
  ascii  "text"         raw bytes of the string, no terminator.
  asciiz "text"         raw bytes plus a trailing 0x00.
  addr   SYM            an 8-byte absolute address of SYM -> abs64 fixup (the "label address"
                        static; the only way to embed a resolved pointer in data).
  res    N              N bytes of zero. In .bss it costs memsz only; in .data/.rodata it emits
                        N zero file bytes.

OPERAND / IMMEDIATE NOTATION
  Integers are decimal or 0x-hex, optional leading '-'. Registers are bare names
  (r64/r32/r16/r8). Memory is [base], [base+disp], [base+index*scale+disp], or
  [SYM] for an absolute operand. A bare identifier or ".local" in a branch/call/lea
  operand is a reference resolved by a fixup.

DETERMINISM CONTRACT (what makes two passes sufficient)
  The encoded LENGTH of every instruction depends ONLY on the operand SHAPE
  (register width, presence of memory, presence of an immediate, presence of a
  symbol field) and NEVER on the numeric VALUE of an immediate or address. The
  assembler therefore knows every size in pass 1 without knowing any address, and
  never relaxes:
    - jmp/call/jcc to a symbol or label ALWAYS use the rel32 form (no short-jump
      selection); e9 cd / e8 cd / 0f 8x cd.
    - loading a 64-bit immediate or an "addr"-style pointer into a register ALWAYS
      uses the 10-byte movabs (REX.W B8+r io).
    - immediate ALU forms (add/sub/and/or/xor/cmp/test with an imm) ALWAYS use the
      imm32 encoding (81 /r id, or the imm8-shift forms for shl/shr/sar); the value
      must fit the fixed field or it is exit 4.
  This is the single canonical form required of the encoding groups: one encoding
  per (mnemonic, operand-shape), fixed length.

DIAGNOSTICS / EXIT CODES (mirroring hex2's convention)
  0 ok · 1 syntax error or unknown mnemonic/operand shape · 2 undefined symbol
  (name printed to stderr) · 3 rel8 displacement out of range · 4 immediate/field
  overflow (value does not fit its fixed field) · 5 zero or multiple 'entry' clauses.

## 3. ELF64 emission

TARGET (docs/ABI.md sec 7): static ET_EXEC, EM_X86_64, ELFCLASS64/ELFDATA2LSB, non-PIE, no PT_INTERP, no PT_DYNAMIC, no relocations, base 0x400000. See the detailed plan in the fixups/directives narrative above. Segments: PH0 PT_LOAD R+X (Ehdr+Phdrs+.text at 0x400000), PH1 PT_LOAD R (.rodata, next page), PH2 PT_LOAD R+W (.data + .bss, next page), optional PH3 PT_GNU_STACK. e_entry = vaddr of the 'entry' proc; e_phoff=64, e_phentsize=56. Minimal seed may fold .rodata into PH2 and drop PH3. Two-pass: (1) parse into buffers, size each item from operand shape, assign section bases text_base=0x400000+64+phnum*56, rodata_base/data_base page-aligned, resolve every symbol to section_base+offset; (2) fill Ehdr/Phdrs, patch all fixup holes, stream Ehdr, Phdrs, .text, .rodata, .data (.bss is memsz only). No relocations remain in the file.

## 4. Fixups

A fixup is {section, field-offset, kind, target}. Let F = absolute vaddr of the
field's first byte (section_base + field-offset), T = resolved absolute vaddr of
the target symbol/label. All fields are written little-endian. Four kinds:

  rel32  - 4-byte signed displacement, value = T - (F + 4).
           Used by every branch/call to a symbol or ".local": jmp/call/jcc, and by
           lea reg,[SYM] when RIP-relative addressing is chosen. This is the
           default and preferred branch fixup (no relaxation).
           Example: at F=0x401005, "e8 00 00 00 00" (call foo) with foo at
           T=0x401014 -> disp = 0x14-(0x05+4) ... = 0x0000000a, patched bytes
           "0a 00 00 00", instruction "e8 0a 00 00 00".
           Range: must fit signed 32-bit; overflow -> exit 4 (cannot happen within
           a single <2GiB image, but checked).

  rel8   - 1-byte signed displacement, value = T - (F + 1).
           Reserved for any mnemonic the encoding groups define as short-only
           (e.g. an explicit short jmp/loop); the mainline branches all use rel32,
           so a portable genesis build need not emit rel8 at all. Out of the
           signed -128..127 range -> exit 3 (as in hex2's "!name").
           Example: F=0x401010, target T=0x401020 -> 0x20-(0x10+1) ... = 0x0f,
           byte "0f".

  abs32sx - 4-byte field holding the low 32 bits of T (T truncated to 32 bits).
           The CPU sign-extends it at use; since the whole image lives near
           0x400000 (< 0x80000000) sign-extension is a no-op, so it equals the
           absolute address. Used for absolute memory operands [SYM] (ModRM
           mod=00,rm=100 SIB base=101,index=100 disp32) and for any "mov r/m,imm32"
           that names a low-image symbol. The assembler asserts T < 0x80000000
           else exit 4.
           Example: SYM at T=0x402000, field "00 00 00 00" -> "00 20 40 00".

  abs64  - 8-byte field holding the full 64-bit T.
           Used by movabs reg,SYM (REX.W B8+r io) and by the "addr SYM" / "word
           <pointer>" data directives - i.e. every embedded resolved pointer.
           Example: SYM at T=0x0000000000402000 -> bytes
           "00 20 40 00 00 00 00 00".

No other fixup kinds exist; there is no GOT/PLT, no PC-relative data relocation
type, and nothing is deferred to a linker or loader - every hole is patched in
pass 2, so the emitted ELF is relocation-free.

## 5. Encoding fundamentals

- **Register numbers** (3-bit): rax=0 rcx=1 rdx=2 rbx=3 rsp=4 rbp=5 rsi=6 rdi=7;
  r8..r15 = 0..7 with the REX extension bit set.
- **REX** = `0100 WRXB` (0x40 base): W=64-bit operand, R=ModRM.reg ext,
  X=SIB.index ext, B=ModRM.rm / SIB.base / opcode-reg ext.
- **ModRM** = `mod(2) reg(3) rm(3)`; **SIB** = `scale(2) index(3) base(3)`.
- **Canonical choice for register-to-register:** a `reg, reg` operand pair is
  encoded with the `r/m, r` opcode family (`89`, `01`, `09`, `21`, `29`, `31`,
  `39`, `85`, ...). The `r, r/m` opcodes (`8b`, `03`, ...) are emitted only when
  the source is memory. This gives exactly one byte string per instruction so
  the assembler is deterministic and its output is testable byte-for-byte.
- All displacements and immediates are little-endian. `imm8` forms (opcode `83`)
  sign-extend to 64 bits; the assembler picks `83` over `81` when the immediate
  fits signed 8-bit, else `81` (imm32).

## 6. Instruction table

Every row: the operand form, one concrete example, its exact bytes (lowercase
hex), and the rule to encode arbitrary operands of that form.

### Data movement

| form | example | bytes | rule |
|------|---------|-------|------|
| `mov r/m64, r64` | `mov rbx, rax` | `48 89 c3` | reg field carries the SOURCE r64, rm field the DESTINATION r/m64. REX = 0x48 \| (src>=8 ? 0x04 (R) : 0) \| (dst>=8 ? 0x01 (B) : 0). opcode = 0x89. For register-direct dst: ModRM = 0xC0 \| ((src&7)<... |
| `mov r64, r/m64` | `mov rax, rbx` | `48 8b c3` | reg field carries the DESTINATION r64, rm field the SOURCE r/m64. REX = 0x48 \| (dst>=8 ? 0x04 (R) : 0) \| (src>=8 ? 0x01 (B) : 0). opcode = 0x8b. Register-direct: ModRM = 0xC0 \| ((dst&7)<<3) \| (... |
| `mov r64, imm32 (sign-extended to 64)` | `mov rax, 1` | `48 c7 c0 01 00 00 00` | REX = 0x48 \| (dst>=8 ? 0x01 (B) : 0). opcode = 0xC7. ModRM = 0xC0 \| (dst&7) (reg field is the /0 opcode extension, always 000). Append imm32 as 4 little-endian bytes. Only valid when the constant... |
| `movabs r64, imm64` | `movabs rax, 0x1122334455667788` | `48 b8 88 77 66 55 44 33 22 11` | REX = 0x48 \| (dst>=8 ? 0x01 (B) : 0). opcode = 0xB8 + (dst&7). Append the full imm64 as 8 little-endian bytes. This is the only form that loads a complete arbitrary 64-bit constant. |
| `mov qword [base + disp], imm32` | `mov qword [rbx + 0x10], 1` | `48 c7 43 10 01 00 00 00` | REX = 0x48 \| (base>=8 ? 0x01 (B) : 0) (add 0x02 X if a SIB index>=8 is used). opcode = 0xC7, reg field = /0 = 000. Encode the memory operand in mod/rm/SIB/disp exactly as for lea (see memory rules... |
| `mov r/m32, r32 (no REX.W)` | `mov ecx, eax` | `89 c1` | Identical to mov r/m64, r64 but WITHOUT REX.W: emit a REX byte only if an extended register (r8d-r15d) participates, as 0x40 \| (src>=8?0x04) \| (dst>=8?0x01). opcode = 0x89, ModRM = 0xC0 \| ((src&... |
| `mov r32, r/m32 (no REX.W)` | `mov eax, ecx` | `8b c1` | Identical to mov r64, r/m64 but WITHOUT REX.W. REX emitted only for extended regs: 0x40 \| (dst>=8?0x04 R) \| (src>=8?0x01 B). opcode = 0x8b, register-direct ModRM = 0xC0 \| ((dst&7)<<3) \| (src&7)... |
| `lea r64, [base + disp8/disp32]` | `lea rax, [rbx + 0x10]` | `48 8d 43 10` | REX = 0x48 \| (dst>=8?0x04 R) \| (base>=8?0x01 B). opcode = 0x8d, reg = (dst&7). Choose mod by disp: mod=00 (no disp) only if disp==0 AND base not in {rbp,r13}; mod=01 with 1-byte disp if disp fits... |
| `lea r64, [base + index*scale + disp]` | `lea rax, [rbx + rcx*4 + 0x10]` | `48 8d 44 8b 10` | rm field of ModRM = 100 to signal a SIB byte; reg = (dst&7); mod chosen from disp as usual. SIB = (log2(scale)<<6) \| ((index&7)<<3) \| (base&7), scale in {1,2,4,8} -> {00,01,10,11}. REX = 0x48 \| ... |
| `lea r64, [rip + disp32]` | `lea rax, [rip + 0x100]` | `48 8d 05 00 01 00 00` | REX = 0x48 \| (dst>=8?0x04 R). opcode = 0x8d. ModRM = 0x05 \| ((dst&7)<<3) (i.e. mod=00, rm=101). Always followed by a 4-byte LE disp32 = target - (address_of_instruction_end). The assembler comput... |
| `movzx r64, r/m8` | `movzx rax, bl` | `48 0f b6 c3` | REX = 0x48 \| (dst>=8?0x04 R) \| (src>=8?0x01 B). opcode bytes = 0F B6. reg = (dst&7); rm = the r/m8 source, register-direct ModRM = 0xC0 \| ((dst&7)<<3) \| (src&7), or a memory operand encoded as ... |
| `movzx r64, r/m16` | `movzx rax, bx` | `48 0f b7 c3` | Same as movzx r/m8 but opcode second byte is B7. REX = 0x48 \| (dst>=8?0x04) \| (src>=8?0x01). ModRM register-direct = 0xC0 \| ((dst&7)<<3) \| (src&7); memory source encoded normally. Zero-extends ... |
| `movsx r64, r/m8` | `movsx rax, bl` | `48 0f be c3` | REX = 0x48 \| (dst>=8?0x04 R) \| (src>=8?0x01 B). opcode = 0F BE. reg = (dst&7); register-direct ModRM = 0xC0 \| ((dst&7)<<3) \| (src&7); memory source encoded normally. Sign-extends bit 7 across a... |
| `movsx r64, r/m16` | `movsx rax, bx` | `48 0f bf c3` | Same as movsx r/m8 but opcode second byte is BF. REX = 0x48 \| (dst>=8?0x04) \| (src>=8?0x01). ModRM register-direct = 0xC0 \| ((dst&7)<<3) \| (src&7); memory encoded normally. Sign-extends the 16-... |
| `movsxd r64, r/m32` | `movsxd rax, ecx` | `48 63 c1` | REX = 0x48 \| (dst>=8?0x04 R) \| (src>=8?0x01 B) (add 0x02 X for an extended SIB index in memory forms). opcode = 0x63. reg = (dst&7); register-direct ModRM = 0xC0 \| ((dst&7)<<3) \| (src&7); a 32-... |

### ALU

| form | example | bytes | rule |
|------|---------|-------|------|
| `add r/m64, r64` | `add rbx, rax` | `48 01 c3` | opcode is 01 (the /r store form: reg field = SOURCE r64, rm field = DEST r/m64). REX=0x40\|8\|(R<<2)\|(X<<1)\|B with W=1 always, R=1 if source reg is r8-r15, B=1 if dest reg is r8-r15, X=0. ModRM =... |
| `or r/m64, r64` | `or rbx, rax` | `48 09 c3` | opcode 09 (/r store form). REX and ModRM computed exactly as add_rm64_r64: REX.W=1, R=src r8-r15, B=dst r8-r15; ModRM = 0xC0 \| ((src&7)<<3) \| (dst&7). |
| `and r/m64, r64` | `and rbx, rax` | `48 21 c3` | opcode 21 (/r store form). REX/ModRM as add_rm64_r64: ModRM = 0xC0 \| ((src&7)<<3) \| (dst&7), REX.W=1 with R/B extension bits. |
| `sub r/m64, r64` | `sub rbx, rax` | `48 29 c3` | opcode 29 (/r store form). REX/ModRM as add_rm64_r64: ModRM = 0xC0 \| ((src&7)<<3) \| (dst&7). |
| `xor r/m64, r64` | `xor rbx, rax` | `48 31 c3` | opcode 31 (/r store form). REX/ModRM as add_rm64_r64: ModRM = 0xC0 \| ((src&7)<<3) \| (dst&7). |
| `cmp r/m64, r64` | `cmp rbx, rax` | `48 39 c3` | opcode 39 (/r store form). REX/ModRM as add_rm64_r64: ModRM = 0xC0 \| ((src&7)<<3) \| (dst&7). cmp writes no result, only flags. |
| `add r64, r/m64` | `add rax, rbx` | `48 03 c3` | opcode 03 (the /r load form: reg field = DEST r64, rm field = SRC r/m64 — reversed roles vs 01). REX.W=1, R=1 if dest reg is r8-r15, B=1 if src reg is r8-r15, X=0. ModRM = 0xC0 \| ((dst&7)<<3) \| (... |
| `or r64, r/m64` | `or rax, rbx` | `48 0b c3` | opcode 0b (/r load form). REX/ModRM as add_r64_rm64: reg=dest, rm=src; ModRM = 0xC0 \| ((dst&7)<<3) \| (src&7). |
| `and r64, r/m64` | `and rax, rbx` | `48 23 c3` | opcode 23 (/r load form). REX/ModRM as add_r64_rm64: ModRM = 0xC0 \| ((dst&7)<<3) \| (src&7). |
| `sub r64, r/m64` | `sub rax, rbx` | `48 2b c3` | opcode 2b (/r load form). REX/ModRM as add_r64_rm64: ModRM = 0xC0 \| ((dst&7)<<3) \| (src&7). |
| `xor r64, r/m64` | `xor rax, rbx` | `48 33 c3` | opcode 33 (/r load form). REX/ModRM as add_r64_rm64: ModRM = 0xC0 \| ((dst&7)<<3) \| (src&7). |
| `cmp r64, r/m64` | `cmp rax, rbx` | `48 3b c3` | opcode 3b (/r load form). REX/ModRM as add_r64_rm64: ModRM = 0xC0 \| ((reg&7)<<3) \| (rm&7). cmp writes only flags, computing (reg - rm). |
| `add r/m64, imm32` | `add rbx, 0x12345678` | `48 81 c3 78 56 34 12` | opcode 81, group /digit goes in ModRM.reg; for add digit=0. REX.W=1, B=1 if rm reg is r8-r15 (R/X=0). ModRM = 0xC0 \| (0<<3) \| (rm&7). Append imm32 little-endian (4 bytes). |
| `or r/m64, imm32` | `or rbx, 0x12345678` | `48 81 cb 78 56 34 12` | opcode 81, /digit=1 (or) in ModRM.reg. ModRM = 0xC0 \| (1<<3) \| (rm&7). REX.W=1, B for r8-r15 rm. Append imm32 LE. |
| `and r/m64, imm32` | `and rbx, 0x12345678` | `48 81 e3 78 56 34 12` | opcode 81, /digit=4 (and) in ModRM.reg. ModRM = 0xC0 \| (4<<3) \| (rm&7). REX.W=1, B for r8-r15 rm. Append imm32 LE. |
| `sub r/m64, imm32` | `sub rbx, 0x12345678` | `48 81 eb 78 56 34 12` | opcode 81, /digit=5 (sub) in ModRM.reg. ModRM = 0xC0 \| (5<<3) \| (rm&7). REX.W=1, B for r8-r15 rm. Append imm32 LE. |
| `xor r/m64, imm32` | `xor rbx, 0x12345678` | `48 81 f3 78 56 34 12` | opcode 81, /digit=6 (xor) in ModRM.reg. ModRM = 0xC0 \| (6<<3) \| (rm&7). REX.W=1, B for r8-r15 rm. Append imm32 LE. |
| `cmp r/m64, imm32` | `cmp rbx, 0x12345678` | `48 81 fb 78 56 34 12` | opcode 81, /digit=7 (cmp) in ModRM.reg. ModRM = 0xC0 \| (7<<3) \| (rm&7). REX.W=1, B for r8-r15 rm. Append imm32 LE. Writes only flags (rm - imm). |
| `add r/m64, imm8 (sign-extended)` | `add rbx, 0x7f` | `48 83 c3 7f` | opcode 83, /digit=0 (add) in ModRM.reg. ModRM = 0xC0 \| (0<<3) \| (rm&7). REX.W=1, B for r8-r15 rm. Append one imm8 byte, sign-extended to 64 bits at execution. |
| `or r/m64, imm8 (sign-extended)` | `or rbx, 0x7f` | `48 83 cb 7f` | opcode 83, /digit=1 (or) in ModRM.reg. ModRM = 0xC0 \| (1<<3) \| (rm&7). REX.W=1, B for r8-r15 rm. Append sign-extended imm8. |
| `and r/m64, imm8 (sign-extended)` | `and rbx, 0x7f` | `48 83 e3 7f` | opcode 83, /digit=4 (and) in ModRM.reg. ModRM = 0xC0 \| (4<<3) \| (rm&7). REX.W=1, B for r8-r15 rm. Append sign-extended imm8. |
| `sub r/m64, imm8 (sign-extended)` | `sub rbx, 0x7f` | `48 83 eb 7f` | opcode 83, /digit=5 (sub) in ModRM.reg. ModRM = 0xC0 \| (5<<3) \| (rm&7). REX.W=1, B for r8-r15 rm. Append sign-extended imm8. |
| `xor r/m64, imm8 (sign-extended)` | `xor rbx, 0x7f` | `48 83 f3 7f` | opcode 83, /digit=6 (xor) in ModRM.reg. ModRM = 0xC0 \| (6<<3) \| (rm&7). REX.W=1, B for r8-r15 rm. Append sign-extended imm8. |
| `cmp r/m64, imm8 (sign-extended)` | `cmp rbx, 0x7f` | `48 83 fb 7f` | opcode 83, /digit=7 (cmp) in ModRM.reg. ModRM = 0xC0 \| (7<<3) \| (rm&7). REX.W=1, B for r8-r15 rm. Append sign-extended imm8. Writes only flags (rm - imm). |
| `test r/m64, r64` | `test rbx, rax` | `48 85 c3` | opcode 85 (/r). REX.W=1, R=1 if the r64 (reg field) is r8-r15, B=1 if the r/m64 (rm field) is r8-r15, X=0. ModRM = 0xC0 \| ((reg&7)<<3) \| (rm&7). Computes rm AND reg, sets flags only, discards res... |
| `test r/m64, imm32` | `test rbx, 0x12345678` | `48 f7 c3 78 56 34 12` | opcode f7, /digit=0 in ModRM.reg (the f7 group's /1 is a second, redundant test encoding that a simple assembler never emits — always use /0). ModRM = 0xC0 \| (0<<3) \| (rm&7). REX.W=1, B for r8-r1... |

### Shift / multiply / divide

| form | example | bytes | rule |
|------|---------|-------|------|
| `shl r/m64, imm8` | `shl rax, 5` | `48 c1 e0 05` | 48 + REX.B (49) if rm is r8-r15. opcode c1. ModRM = 0xC0 + (4<<3) + (rm&7) for a register operand (mod=11); for memory use mod 00/01/10, low-3 rm, plus SIB/disp as needed and REX.X for an extended ... |
| `shr r/m64, imm8` | `shr rax, 5` | `48 c1 e8 05` | Same as shl_rm64_imm8 but reg field = 101 (/5): ModRM = 0xC8 + (rm&7) for register direct. |
| `sar r/m64, imm8` | `sar rax, 5` | `48 c1 f8 05` | Same as shl_rm64_imm8 but reg field = 111 (/7): ModRM = 0xF8 + (rm&7) for register direct. |
| `shl r/m64, cl` | `shl rbx, cl` | `48 d3 e3` | 48 + REX.B (49) if rm is r8-r15. opcode d3, reg field /4 = 100. ModRM = 0xE0 + (rm&7) for register direct. The count register is implicitly cl; it is NOT encoded in ModRM. |
| `shr r/m64, cl` | `shr rbx, cl` | `48 d3 eb` | opcode d3, reg /5 = 101. ModRM = 0xE8 + (rm&7) for register direct. |
| `sar r/m64, cl` | `sar rbx, cl` | `48 d3 fb` | opcode d3, reg /7 = 111. ModRM = 0xF8 + (rm&7) for register direct. |
| `shl r/m64, 1` | `shl rdx, 1` | `48 d1 e2` | 48 + REX.B (49) if rm is r8-r15. opcode d1, reg /4 = 100. ModRM = 0xE0 + (rm&7) for register direct. shr-by-1 uses /5 (ModRM 0xE8+rm), sar-by-1 uses /7 (ModRM 0xF8+rm). |
| `inc r/m64` | `inc rax` | `48 ff c0` | 48 + REX.B (49) if rm is r8-r15. opcode ff, reg /0 = 000. ModRM = 0xC0 + (rm&7) for register direct. |
| `dec r/m64` | `dec rcx` | `48 ff c9` | opcode ff, reg /1 = 001. ModRM = 0xC8 + (rm&7) for register direct. |
| `neg r/m64` | `neg rax` | `48 f7 d8` | 48 + REX.B (49) if rm is r8-r15. opcode f7, reg /3 = 011. ModRM = 0xD8 + (rm&7) for register direct. |
| `not r/m64` | `not rbx` | `48 f7 d3` | opcode f7, reg /2 = 010. ModRM = 0xD0 + (rm&7) for register direct. |
| `imul r64, r/m64` | `imul rax, rbx` | `48 0f af c3` | REX = 0x48 \| (dst>=8?0x04:0) \| (src>=8?0x01:0). opcode 0f af. ModRM = 0xC0 + ((dst&7)<<3) + (src&7) for register direct. The destination is ModRM.reg, the source r/m; result is truncated to 64 bi... |
| `imul r64, r/m64, imm32` | `imul rax, rbx, 0x100` | `48 69 c3 00 01 00 00` | REX = 0x48 \| (dst>=8?0x04:0) \| (rm-reg>=8?0x01:0) (+REX.X for an extended memory index). opcode 69. ModRM = 0xC0 + ((dst&7)<<3) + (src&7) for register direct. Append imm32 in little-endian (4 byt... |
| `imul r/m64` | `imul rbx` | `48 f7 eb` | 48 + REX.B (49) if rm is r8-r15. opcode f7, reg /5 = 101. ModRM = 0xE8 + (rm&7) for register direct. Signed: rdx:rax = rax * r/m64 (128-bit product, high half in rdx). |
| `mul r/m64` | `mul rbx` | `48 f7 e3` | opcode f7, reg /4 = 100. ModRM = 0xE0 + (rm&7) for register direct. Unsigned: rdx:rax = rax * r/m64. |
| `div r/m64` | `div rbx` | `48 f7 f3` | opcode f7, reg /6 = 110. ModRM = 0xF0 + (rm&7) for register direct. Unsigned: rax = rdx:rax / r/m64, rdx = remainder. |
| `idiv r/m64` | `idiv rbx` | `48 f7 fb` | 48 + REX.B (49) if rm is r8-r15. opcode f7, reg /7 = 111. ModRM = 0xF8 + (rm&7) for register direct. Signed: rax = rdx:rax / r/m64, rdx = remainder. |
| `cqo` | `cqo` | `48 99` | Fixed two bytes: 48 99. Always exactly these bytes; there are no operand variations. |

### Stack and control flow

| form | example | bytes | rule |
|------|---------|-------|------|
| `push r64` | `push rbx` | `53` | reg 0-7 (rax..rdi): one byte 0x50+reg. reg 8-15 (r8..r15): two bytes 0x41 (REX.B) then 0x50+(reg-8). Register codes: rax=0 rcx=1 rdx=2 rbx=3 rsp=4 rbp=5 rsi=6 rdi=7; r8..r15 = 0..7 with REX.B. |
| `push imm32` | `push 0x12345678` | `68 78 56 34 12` | Emit 0x68, then imm32 as 4 little-endian bytes. Use this form when the immediate does NOT fit in a signed 8-bit range (i.e. outside -128..+127); otherwise the canonical assembler chooses push imm8. |
| `push imm8` | `push 0x7f` | `6a 7f` | Emit 0x6a, then the single immediate byte. The canonical assembler selects this form precisely when the signed immediate satisfies -128 <= imm <= 127; larger values use push imm32 (0x68). |
| `pop r64` | `pop rbp` | `5d` | reg 0-7: one byte 0x58+reg. reg 8-15: 0x41 (REX.B) then 0x58+(reg-8). Same register-code table as push. |
| `jmp rel8` | `jmp $  (jump to self, 2-byte instruction)` | `eb fe` | Emit 0xeb, then disp8 = target - (instr_start + 2) as one two's-complement byte. Only usable when that value is in the signed 8-bit range -128..+127; otherwise use jmp rel32. |
| `jmp rel32` | `jmp target  (target = instr_start + 0x100)` | `e9 fb 00 00 00` | Emit 0xe9, then disp32 = target - (instr_start + 5) as 4 little-endian two's-complement bytes. Signed 32-bit range -2147483648..+2147483647, always relative to the instruction end. |
| `jmp r/m64 (register indirect)` | `jmp rax` | `ff e0` | Emit 0xff, then ModRM byte 0xE0 + (reg & 7) for the mod=11 register form; if reg >= 8 prepend 0x41 (REX.B). ModRM.reg is fixed at 100 (the /4 opcode extension). |
| `jcc rel8` | `je target  (target = instr_start + 0x10)` | `74 0e` | Emit (0x70 + cc), then disp8 = target - (instr_start + 2) as one two's-complement byte (range -128..+127). Requested set: je/jz=74, jne/jnz=75, jl=7c, jle=7e, jg=7f, jge=7d, jb/jc=72, jbe=76, ja=77... |
| `jcc rel32` | `je target  (target = instr_start + 0x20)` | `0f 84 1a 00 00 00` | Emit 0x0f, then (0x80 + cc), then disp32 = target - (instr_start + 6) as 4 little-endian two's-complement bytes (signed 32-bit range). Requested set: je/jz=0f 84, jne/jnz=0f 85, jl=0f 8c, jle=0f 8e... |
| `call rel32` | `call target  (target = instr_start + 5, the next instruction)` | `e8 00 00 00 00` | Emit 0xe8, then disp32 = target - (instr_start + 5) as 4 little-endian two's-complement bytes. Signed 32-bit range -2147483648..+2147483647, relative to the instruction end. |
| `call r/m64 (register indirect)` | `call rax` | `ff d0` | Emit 0xff, then ModRM byte 0xD0 + (reg & 7) for the mod=11 register form; if reg >= 8 prepend 0x41 (REX.B). ModRM.reg is fixed at 010 (the /2 opcode extension). |
| `ret (near return)` | `ret` | `c3` | Always exactly one byte: c3. |
| `syscall` | `syscall` | `0f 05` | Always exactly two bytes: 0f 05. |

### Addressing matrix (ModRM / SIB / REX)

| form | example | bytes | rule |
|------|---------|-------|------|
| `mov r/m64, r64  (mod=11, register direct)` | `mov rax, rbx` | `48 89 d8` | mod=11 means the rm field is a register, not a memory operand — no SIB/disp ever follow. Encode reg=src, rm=dst each as its 3-bit code; set REX.W=1 for 64-bit, REX.R for reg>=r8, REX.B for rm>=r8. ... |
| `mov r64, [base]  (mod=00, no displacement)` | `mov rax, [rbx]` | `48 8b 03` | For [base] with base in {rax,rcx,rdx,rbx,rsi,rdi} (low 3 bits != 100 and != 101): mod=00, rm=base's 3-bit code. ModRM = (reg<<3) \| rm. REX.B if base>=r8. Bases rsp/r12 (rm=100) and rbp/r13 (rm=101... |
| `mov r64, [base + disp8]  (mod=01, signed 8-bit disp)` | `mov rax, [rbx+16]` | `48 8b 43 10` | Use mod=01 when disp fits a signed byte (-128..127). ModRM = 0x40 \| (reg<<3) \| rm, then one disp byte (two's complement). If rm=100 a SIB byte is inserted BEFORE the disp8; if base is rbp/r13 thi... |
| `mov r64, [base + disp32]  (mod=10, signed 32-bit disp)` | `mov rax, [rbx+0x1000]` | `48 8b 83 00 10 00 00` | Use mod=10 when disp exceeds signed-byte range. ModRM = 0x80 \| (reg<<3) \| rm, then 4 little-endian disp bytes (two's complement, sign-extended to 64). SIB inserted before disp32 when rm=100. |
| `mov r64, [rsp \| r12 (+disp)]  (rm=100 forces SIB) — GOTCHA 1` | `mov rax, [rsp]` | `48 8b 04 24` | Whenever the base's low 3 bits = 100 (rsp or r12), you cannot name it in ModRM.rm directly; set rm=100 and emit a SIB with index=100 (no index) and base=100. REX.B distinguishes r12 from rsp. disp ... |
| `mov r64, [rbp \| r13]  (must use mod=01 disp8=0) — GOTCHA 2` | `mov rax, [rbp]` | `48 8b 45 00` | When the base's low 3 bits = 101 (rbp or r13) and you want [base] with no displacement, you MUST promote to mod=01 with an explicit disp8 of 0 (or mod=10 disp32=0). REX.B distinguishes r13 from rbp... |
| `mov r64, [rip + disp32]  (mod=00, rm=101)` | `mov rax, [rip+0]` | `48 8b 05 00 00 00 00` | mod=00 with rm=101 (and no SIB) always means [rip+disp32] in long mode. The disp32 is signed and relative to the end of the whole instruction. Used for position-independent references to statics; t... |
| `mov r64, [disp32]  (absolute, SIB base=101 no base)` | `mov rax, [0x1000]` | `48 8b 04 25 00 10 00 00` | To encode a bare absolute address [disp32] with no base and no index: mod=00, rm=100 (SIB), SIB index=100, SIB base=101. The 4-byte disp32 is then the full (zero-extended, non-rip) address. This is... |
| `mov r64, [base + index*scale + disp]  (full SIB)` | `mov rax, [rbx + rcx*4 + 16]` | `48 8b 44 8b 10` | Scale encodes as 1->00, 2->01, 4->10, 8->11 (scale = log2). SIB = (scale<<6) \| (index<<3) \| base. ModRM.rm=100 selects the SIB; mod chooses the disp width (00 none / 01 disp8 / 10 disp32) exactly... |
| `REX.R / REX.X / REX.B extending reg / index / base — GOTCHA 3` | `mov r8, [r9 + r10*2]` | `4f 8b 04 51` | REX = 0x40 \| (W<<3) \| (R<<2) \| (X<<1) \| B. Each register's 4th (high) bit goes into a different REX bit by ROLE: ModRM.reg -> REX.R; SIB.index -> REX.X; ModRM.rm OR SIB.base OR opcode-embedded ... |
| `mov rax, [BASE + disp]  — canonical bytes for the six tricky bases at disp=0 and disp=16` | `mov rax, [rbp+16]` | `48 8b 45 10` | Pick REX = 0x48 \| (0x01 if base>=r8). ModRM.reg=000 (rax dest). Then classify the base by its low 3 bits: 100 (rsp/r12) -> rm=100 + SIB 0x24 (index=100 none, base=100); 101 (rbp/r13) -> rm=101 but... |

## 7. The four addressing gotchas (must be exact)

From the verified `mem_*` / `rex_*` rows above:
- `[rsp]` and `[r12]` need a SIB byte even with no index (rm=100, SIB index=100).
  `mov rax,[rsp]` = `48 8b 04 24`.
- `[rbp]` and `[r13]` cannot use mod=00 (that means RIP-relative); use mod=01,
  disp8=0. `mov rax,[rbp]` = `48 8b 45 00`.
- REX.B extends ModRM.rm or SIB.base; REX.X extends SIB.index; REX.R extends
  ModRM.reg. `mov r8,[r9+r10*2]` = `4f 8b 04 51` (REX.WRXB all set).
- `[disp32]` with no base = SIB base=101, mod=00. `mov rax,[0x1000]` =
  `48 8b 04 25 00 10 00 00`.

## 10. Not encoded here (deferred)

SSE/AVX, x87, 8/16-bit ALU beyond what movzx/movsx need, `rep` string ops, and
privileged instructions (`lgdt`, `iretq`, ...) are out of scope for `asm`; the
self-hosted `olic` backend adds them. `asm` only needs enough to compile the
oli-core compiler (layer 3).

## 9. Implementation status (`genesis/2-asm/asm.hex2`)

`asm` is being hand-written in `hex2` notation, module by module, each proven on
the real CPU (harness `genesis/test.sh`, layer 2b):

| Step | Adds | Proven by | Status |
|------|------|-----------|--------|
| 1a | ELF writer (template copy, size patch, write) | emits an exit(42) ELF | done |
| 1b | line reader, keyword dispatch, `bytes HEX...` directive | assembles `tests/exit42.asm` -> runs, exit 42 | done |
| 2 | r32/r64 register + immediate encoding, checked structure and entry | exact bytes + CPU execution + rejection fixtures | done |
| 3 | base/index/scale/displacement memory operands, ModRM/SIB/REX | `memory.oli/.hex`, `memory_run.oli` | done for supported r64 forms |
| 4 | local/procedure symbols, rel32/absolute references, multi-section ELF | `branches.oli/.hex`, `branches_run.oli` | symbols done; separate segments pending |
| 5 | full encoding table + data directives | `data.oli/.hex`, `examples/genesis/hello.oli` | read-only data and hello done; remaining forms pending |

8/16-bit general-purpose registers (`ax`, `al`, `r8w`, `r8b`, ...) are **rejected**
with exit 1 (`asm: error`), per section 8: they are out of scope until a design
record specifies them. `reg32_match` recognises the names so the diagnostic is a
clean "unsupported register", not "undefined symbol". The whole layer-2b suite
(`genesis/2-asm/test.sh`) — exact-byte, execution and rejection fixtures — passes
via `sh genesis/test.sh`.

`bytes HEX...` inside a `machine x64` block is the bootstrap scaffold: it emits
raw bytes so `asm` is useful before its encoder is complete. Each later step
replaces `bytes` uses with real mnemonics. `bytes` stays in the language as the
documented raw-emit escape (like `.byte` in traditional assemblers).

### Implemented subset (2026-09-20)

The current assembler is 6,878 bytes, still built exclusively by `hex2`.
`module`, `proc`, `calls none`, `entry`, `machine x64` and `end` are checked,
including placement, required clauses and exactly one entry point. Procedures
must have a machine block. Unknown or unsupported syntax fails instead of
being ignored. Comments, tabs, CRLF and EOF without a final newline work.

- All 16 r64 registers; `mov`, `add`, `sub`, `and`, `or`, `xor`, `cmp`, `test`
  with register/register, register/memory, memory/register and immediate forms.
  Memory/immediate `mov` uses sign-extended imm32. Register/immediate `mov`
  always uses imm64 and also accepts a procedure/data/local symbol address.
- `inc`, `dec`, `neg`, `not`, `mul`, `div`, `idiv` on r64 or memory; one-operand
  `imul` on r64 or memory, two/three-operand `imul` on registers.
- `shl`, `shr`, `sar` on r64 or memory with imm8 or `cl`; `lea r64,[address]`.
- `push r64`, `pop r64`, `push imm32`; indirect `call`/`jmp` through registers
  or memory; direct `call`/`jmp`, all listed conditional branches and aliases;
  `ret`, `cqo`, `syscall`.
- Procedure names, scoped `.local:` labels, forward/backward references and
  duplicate/undefined-symbol checks. Two passes have identical instruction
  sizes. No external relocation or linking stage is involved.
- `rodata` blocks: `byte`, `word`, `ascii`, `asciiz`, `addr`, `res`. Strings
  implement every escape listed in §1. Read-only bytes currently share the RX
  load segment with code; there is no writable output segment yet. `entry`
  points to its actual procedure even if data or other procedures precede it.

Explicit memory displacements always use disp32 (including `+0`), preserving
shape-based sizing; omitted displacement on rbp/r13 uses disp8=0. Both
`[rbp+-8]` and the shorthand `[rbp-8]` are accepted. `rsp` cannot be an index;
`r12` can. Immediate shifts always use C1, including count 1. Immediate push
always uses 68/imm32. Compact encodings in the catalogue are CPU examples,
not competing canonical outputs of this bootstrap implementation.

Limits: input smaller than 1 MiB, output at most 1 MiB including its 120-byte
ELF header, at most 819 symbols, at most 63 bytes per complete symbol name
and 31 characters per dotted component. Exit codes 1/2/4/5 retain their §2
meanings; 6 is a capacity limit and 7 is an I/O error. Parse/validation errors
produce no stdout. Undefined-symbol diagnostics include the symbol name.
Other errors currently print `asm: error` and are distinguished by exit code.
Interrupted reads/writes retry, and successful partial writes are completed.

**Still required before G2 is complete:** 8/16/32-bit operands and extension
instructions; memory-source two/three-operand `imul`; memory push/pop;
symbol-only memory operands; writable `data`, file-free `bss`, separate ELF
segment permissions; `pub`/`export` clauses. The optional symbol table and
source-line diagnostics are not emitted. The module name is validated but
no qualified ELF symbol table is generated. G3 (`oli1`) is green through step
6e and G4's front end is complete; neither needs the missing forms above yet.
