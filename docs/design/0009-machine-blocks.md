# 0009 — `machine` blocks assembled by the compiler

## Problem
Inline assembly is unavoidable in kernels (segment loads, `iretq`, `cpuid`,
`rdmsr`), but GCC-style asm is an opaque string with a cryptic constraint
language the compiler cannot check.

## Existing approaches
- GCC/Clang: `asm volatile("..." : outputs : inputs : clobbers)`; string templates, `"=r"`, `"memory"`.
- Rust: `asm!` with named operands; still a string template, assembled by LLVM.
- Zig: same model as GCC with Zig syntax.
- MSVC: `__asm` blocks with direct variable access (x86 only, no constraints).

## Oli-- approach
```oli
machine x64
    in  eax <- leaf            -- inputs loaded before the block
    in  ecx <- 0
    cpuid                      -- instruction lines, validated by the Oli-- encoder
    out ebx -> b               -- outputs stored after the block
    out edx -> d
    clobber ecx, memory        -- registers/memory the block modifies
end
```
- Instructions are tokens, not strings; unknown mnemonics or bad operands are compile errors.
- `in`/`out`/`clobber` are the complete interface; flags are always clobbered.
- Local labels `.name:` are allowed; jumps outside the block are not.
- Requires `permit cpu.asm`; treated by the optimizer as a call with the declared effects.
- `--show-asm` prints the block unchanged inside the surrounding generated code.

## Advantages
- The compiler checks what it assembles; no constraint-string mistakes.
- Register allocation respects the block's registers precisely.
- Same syntax whether the block is the whole body (`calls none`) or a fragment.

## Disadvantages
- Only instructions the Oli-- encoder knows can be used (the encoder grows with need).
- No support for other assemblers' macro features.

## Machine cost
Exactly the written instructions plus input/output moves.

## Safety implications
`memory` clobber is required for blocks that touch memory; omitting it is a
correctness error the verifier cannot catch — the same trust boundary as any
`memory.raw` procedure, and it is confined to `cpu.asm` procedures.

## Alternatives rejected
- String-template asm: unverifiable.
- Only intrinsics, no asm: some sequences (segment reloads, `iretq`) have no intrinsic shape.
