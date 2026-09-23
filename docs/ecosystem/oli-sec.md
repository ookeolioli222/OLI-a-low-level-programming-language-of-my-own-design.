# oli.sec — native security-research substrate (planned)

Condensed roadmap of the full brief (`OLI_SECURITY_LIBRARY`). It is a platform
for **authorized** security research: defensive testing, CTF/lab environments,
software verification, bug-bounty programs (in scope), and security
engineering. It is not a collection of ready attacks; its distinctive goal is
**cross-layer correlation** — keeping the same bytes traceable from a captured
packet through protocol parsing, the owning process, executed basic blocks,
binary functions and dataflow, so previously separate workflows compose.

## Guardrails baked into the design (not optional)
- **Scope is a first-class type** (`oli.sec.scope`: `Scope`, `ScopeRule`,
  `ScopeToken`, `TestBudget`, `RatePolicy`). Every active networking component
  takes a scope object; an out-of-scope operation fails **before** a packet is
  sent. Modes: `observe`, `safe-active` (default), `lab`, `explicit-destructive`.
- **Evidence-first.** A finding carries structured evidence (class, confidence,
  request/response, state before/after, timing, parser interpretation, trace
  refs, minimized reproducer, mitigation notes) — never a bare "possible vuln".
- **Reproducibility.** Active tests optionally emit a replay bundle
  (`finding.olisec/`: finding.json, pcapng, har, replay.oli, trace, checksums).
- **No magic.** Any copy, allocation, socket, thread, probe, process pause,
  network request, sync or GPU launch is observable.
- **Secrets protected.** TLS session secrets are never embedded in captures by
  default; a `Secret` taint class tracks keys/tokens; a redaction engine runs on
  evidence. Observation APIs are kept separate from active-mutation APIs.

## Architecture (independent subsystems, not one monolith)
scope · bytes (byte provenance) · binary · codec · crypto · net · capture ·
packet · protocol · trace · instrument · reverse · disasm · lift · decompile ·
emulate · fuzz · crash · taint · symbolic · static · web · browser · api · auth ·
mobile · cloud · container · supply · firmware · hardware · ai · policy ·
evidence · profiler · lab. A common data model (`Artifact` + `ArtifactGraph`)
gives every object id/type/origin/timestamp/hash/parent/provenance.

## Distinctive engines
- **Packet Reality** — multi-vantage capture (XDP, TCX, socket, TLS plaintext)
  with a loss map per plane, offload-awareness (GSO/TSO reconstruction marked
  *synthetic*, never ground truth), hardware timestamps, PCAPNG++ metadata.
- **Parser Tribunal** — differential parsing (URL/HTTP/JSON/Unicode/JWT/…): a
  disagreement is a research lead, not an automatic vulnerability.
- Security IR (SIR) with unknown/partial values for reverse engineering; binary
  lifting keeping machine address + original bytes; CFG with confidence on
  indirect edges; patch-diff and mitigation analyzer that *explain*, not score.
- Fuzzing platform (replaceable Executor/Mutator/Corpus/Feedback/Oracle/…),
  semantic + value-guided + structure-aware + protocol-state coverage, CrashDNA
  dedup, deterministic `TimeBox` sandbox, snapshot fuzzing.
- Taint (incl. `Secret`), SecretLife lifetime analyzer, constant-time analysis,
  crypto policy + CBOM, AI/LLM/agent security (prompt/tool provenance,
  prompt-injection **test harness**, MCP/RAG analysis) — all evidence-first.

## Non-goals
Not a clone of OpenSSL/Scapy/libpcap/Frida/Ghidra/AFL++; not "every tool in one
package"; not a red/green scanner. No mandatory Python or heavy runtime.
Own crypto is educational unless audited (`crypto.experimental`); production
crypto uses vetted implementations.

## Phase plan (after the self-hosted compiler + net/binary stdlib)
1 binary+bytes · 2 network-capture foundation · 3 Packet Reality · 4 HTTP
research core · 5 API/auth · 6 fuzzing · 7 reverse engineering · 8 dynamic
instrumentation · 9 snapshot fuzzing/emulation · 10 static+taint · 11 supply
chain/containers · 12 mobile · 13 AI security · 14 advanced research engines ·
15 GPU / massive-scale analysis (via `oli.compute`).

## Originality gate (before any subsystem)
Cloning an API, or exposing machine/security state more clearly? zero-copy?
byte provenance preserved? correlates with another subsystem? deterministic and
replayable? explains *why* it concluded something? can drop to raw
bytes/memory/frames/instructions? works without Python / without a heavy runtime?
