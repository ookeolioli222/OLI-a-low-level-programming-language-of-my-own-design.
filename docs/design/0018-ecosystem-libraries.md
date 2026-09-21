# 0018 — Ecosystem libraries: oli.compute and oli.sec (accepted, deferred)

Status: accepted 2026-09-20 as planned libraries; implementation deferred until
the self-hosted compiler and the language features they need exist.

## Problem
Two large library briefs (a GPU-first AI/HPC stack, `oli.compute`; a security-
research substrate, `oli.sec`) define much of Oli--'s eventual value. They must
be part of the plan without derailing the current bootstrap.

## Oli-- approach
Record both as ecosystem libraries written in Oli--, condensed into
`docs/ecosystem/` with their full briefs kept verbatim as `*.brief.md`, and
place them on `ROADMAP.md` after self-hosting. They are gated:

- `oli.compute` needs SIMD types, `own`, atomics, threads, C FFI (V1/V2) and a
  GPU execution model in OIR before phase 1 of its own plan.
- `oli.sec` needs the net/binary standard library, syscalls and views; its GPU
  phase depends on `oli.compute`.

Both carry an originality gate (do they clone a tool, or use Oli--'s explicit
memory / zero-copy views / observable cost / own IR?) and, for `oli.sec`, an
authorization model (scope as a first-class type, `safe-active` default,
evidence-first findings, secret protection) that is part of the design, not an
add-on.

## Advantages
- The project's long-term direction is captured and versioned in-repo now.
- The gates keep the bootstrap focused: no ecosystem code before the language runs.
- Each brief's own design-review checklist is preserved for when work starts.

## Disadvantages
- The briefs will drift from the language as V1/V2 land; they are roadmaps, not specs.

## Machine cost / safety
None now (documentation). `oli.sec`'s guardrails (scope, evidence,
observability, redaction, secret taint) are recorded as requirements so the
library is built for authorized/defensive use from the first phase.

## Alternatives rejected
- Starting either library now: impossible — the language cannot yet compile them.
- Keeping the briefs only on the user's Desktop: not versioned with the project.
