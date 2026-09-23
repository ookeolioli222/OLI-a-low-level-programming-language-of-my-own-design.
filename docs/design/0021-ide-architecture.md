# 0021 — IDE architecture: terminal first, one event loop, out-of-process service

Status: proposed 2026-09-21; accepted when the I0 specifications of
`docs/IDE_PLAN.md` are reviewed.

## Problem
The roadmap ends with an IDE written in Oli-- (design 0017: no foreign
framework, runtime or language server). The earlier plan put a windowed
application in the first release, which gates the IDE on V2 threads, the
Windows target, a windowing binding and a text-rendering stack. The plan must
choose an architecture that (a) is reachable soon after self-hosting, (b) does
not lose user text, (c) reuses the compiler instead of re-implementing the
language, and (d) fits a language whose allocation model is lexical zones and
which has no threads before V2.

## Existing approaches
- VS Code and IntelliJ: a UI process, a language-server process, JSON-RPC
  (LSP), a plugin runtime; large foreign runtimes (Electron, JVM).
- Emacs and Vim/Neovim: terminal-first editors with a scripting language;
  language support through LSP clients.
- Helix and Kakoune: terminal-first, no plugin language, built-in LSP client,
  tree-sitter highlighting; rope or piece-based buffers.
- rust-analyzer and Roslyn: query-based, versioned, cancellable analysis
  separate from the editor.

## Oli-- approach
1. **Terminal first.** `olide` renders to a cell grid through raw syscalls
   (`ioctl`, `poll`, `signalfd`); the pixel renderer (Wayland `wl_shm`,
   Win32) is the last stage and shares every layer except `term/`.
2. **One event loop, no threads.** `poll` over terminal input, the service
   pipe, child processes, a timer and signals; bounded handlers; long work in
   child processes. V2 threads may move the service in-process later.
3. **Out-of-process language service, binary protocol as layouts.** `olis`
   imports `compiler.*` and answers versioned, cancellable queries over OLIS/0,
   whose frames are Oli-- `layout`s with `le` fields parsed by `Header.at(v)`.
   Stale results are rejected by version on both sides. A service crash is
   isolated; the IDE restarts it and re-sends its documents. An LSP bridge
   `olis-lsp` is a separate, optional program.
4. **Piece-table documents, undo tree, journal.** Original buffer plus
   append-only add buffer plus piece list; snapshots of the piece list form the
   undo tree; an append-only edit journal gives crash recovery; saves are
   atomic (temporary file, `fsync`, `rename`).
5. **Zones for per-iteration scratch, `std.alloc` for long-lived state.**
   Lexical zones cannot outlive an event-loop iteration, so an explicit
   mmap-backed allocator is a prerequisite; every allocation site remains a
   visible `ALLOC`.
6. **The compiler is the only source of truth.** Highlighting from the token
   stream, navigation from the semantic graph (0014), cost classes and
   capabilities from `--explain`; nothing about the language lives only in
   the editor.

## Advantages
- The initial release needs M2, self-hosting and a small `std`, not V2.
- No text protocol to parse; the frame format is the language's own layout
  feature, and OLIS/0 is fuzzed like the parser is.
- Crash isolation and stale-result rejection by construction.
- The Oli-- specific views (cost gutter, capability lens, lifetimes, layout
  byte maps, inline machine bytes) come for free from `olic --explain`.

## Disadvantages
- Every query copies its payload through a pipe once; per-declaration
  incrementality is deferred until measured.
- A terminal cannot show proportional fonts, images or fine-grained mouse
  interaction; the pixel renderer is a real second implementation.
- Terminal diversity (key encodings, colours, mouse) needs a test matrix.

## Machine cost
No cost to compiled programs. The IDE itself: one `poll` per event, one pipe
round trip per query, one scratch zone per iteration, `ALLOC` for document
edits. Budgets are in `docs/IDE_PLAN.md` §11 and are measured, not asserted.

## Safety implications
Neither the editor nor the service may crash on any input: source text,
frames, terminal input and child-process output are all untrusted and are
fuzzed. Child processes are started with argument arrays, never shell text, and
only the process group the IDE launched is ever stopped. Saves cannot lose
text; conflicting disk changes require reconciliation.

## Alternatives rejected
- **Windowed application first:** gated on V2, Windows, a windowing binding and
  a font rasterizer; the same editor core is reachable years earlier in a
  terminal.
- **In-process language service from the start:** without threads a long
  analysis would freeze the UI; the out-of-process design also isolates
  crashes. The protocol allows moving in-process later.
- **LSP as the native protocol:** JSON and JSON-RPC add a text parser and
  untyped messages to the toolchain for no gain inside our own IDE; LSP is
  served by a bridge for foreign editors instead.
- **A plugin or scripting language:** a second language of our own design, or a
  foreign one; rejected for the same reason as in 0017. Extension is by
  editing Oli-- source and rebuilding.
- **Rope buffers:** more code than a piece table for no measured benefit at
  the file sizes Oli-- projects have; can be revisited with data.
- **A general-purpose allocator inside the editor only:** `std.alloc` is
  needed by the compiler and the project tool as well; one implementation.
