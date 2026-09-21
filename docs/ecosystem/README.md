# Oli-- ecosystem libraries (planned)

These are libraries that will be **written in Oli--**, on top of the language
and its standard library, once the self-hosted compiler is stable. They are
part of the plan, not part of the current work: nothing here can be built
before the genesis chain reaches a working `olic` (design 0017) and the core
language features they depend on exist.

| Library | Purpose | Depends on (language) | Gate |
|---------|---------|------------------------|------|
| [oli.compute](oli-compute.md) | GPU-first tensor / AI / HPC stack | SIMD types, `own`, atomics, threads, C FFI, GPU target in OIR | after self-hosting + SIMD (V2) |
| [oli.sec](oli-sec.md) | native security-research substrate (authorized testing, defensive research, CTF/lab, bug bounty) | binary layouts, views, syscalls, `oli.compute` (GPU), FFI | after self-hosting + net/binary stdlib |

Both briefs keep their full form as the source of truth; these files capture
the architecture, the phase plan, the non-goals and the originality gate so the
project roadmap carries them. Each library must pass an **Oli originality
review** (does it merely clone an existing tool, or does it use Oli--'s own
strengths — explicit memory, zero-copy views, observable cost, own IR?) before
any implementation.

## Where they sit in the overall roadmap

```
genesis (hex0 → hex2 → asm → oli1 → olic)        ← now (G2)
        ↓
self-hosted olic + core/std in Oli--
        ↓
SIMD, threads, atomics, FFI, GPU target          ← language V1/V2
        ↓
oli.compute            oli.sec                    ← these libraries
```
