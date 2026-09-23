# Oli-- IDE — plan

Status: planned (design record 0021). Nothing described here is implemented.
This is the plan; the normative specifications it calls for (service protocol,
editor model, debug information) are written under `spec/` and `docs/ide/` at
stage I0, before any code. The IDE is the final roadmap stage and follows the
same process as every other stage: DESIGN → CRITIQUE → ORIGINALITY REVIEW →
SPECIFICATION → SMALL IMPLEMENTATION → TEST → BENCHMARK → REVIEW → DOCUMENT.

## 1. Goal

A development environment for Oli-- that

- is written in Oli-- and built by the Oli-- toolchain (design 0017): no
  foreign editor framework, UI toolkit, script runtime or language server;
- reuses the compiler's own lexer, parser, semantic graph and backend, so what
  the editor shows is what `olic` does;
- makes the language's own ideas visible while typing: cost classes,
  capabilities, zones and view lifetimes, layouts with byte offsets, the bytes
  of a `machine` block;
- runs first in a terminal on Linux x86-64, then on Windows, and last as a
  native window.

Non-goals of the initial release: a plugin runtime, collaboration, AI
assistance, editing other languages, an extension marketplace.

## 2. Names

| Thing | Name | Kind |
|-------|------|------|
| IDE | `olide` | program; `oli ide` launches it |
| language service | `olis` | program; child process of `olide` or of an editor adapter |
| service protocol | OLIS/0 | binary frames defined as Oli-- layouts, over pipes (§6) |
| LSP adapter | `olis-lsp` | JSON-RPC ↔ OLIS/0 bridge for foreign editors; optional, after the initial release |
| debug information | ODI | own ELF section `.oli.debug`; specified at stage I4 |
| settings and keymap | `olide.toml` | read by the same TOML reader as `oli.toml` |
| recorded session | `name.olirec` | event stream for replay tests and bug reports |

## 3. Prerequisites: which roadmap gate unlocks which IDE stage

| IDE stage | Needs from the language and toolchain |
|-----------|----------------------------------------|
| I0 specifications | nothing; paper only, can start now |
| I1 `olis` | G4 (`olic` written in Oli--), M2 (procedures, layouts, views, zones), `std.os` file/process/pipe/`poll` over raw syscalls, `std.alloc` (an explicit mmap-backed allocator) |
| I2 `olide` terminal editor | I1; termios via `ioctl`, `poll`, `timerfd`, `signalfd`: all raw syscalls, no threads |
| I3 project integration | I2; the `oli` tool (`oli.toml`, `new/build/check/run/test/fmt`) with machine-readable diagnostics |
| I4 debugger | I3; ODI emitted by `olic`; `ptrace`/`waitpid` |
| I5 Oli-- tools | I3; backend observability (`--show-oir`, `--show-machine-ir`, `--show-bytes`, `--explain`, `--explain-cost`) |
| I6 Windows | the Windows target (PE/COFF, console API through V2 FFI) |
| I7 native window | V2 threads; a pixel backend (Wayland `wl_shm` on Linux, Win32 on Windows); an own font rasterizer |

The initial release is I0–I3 plus packaging, on Linux. This changes the earlier
plan, which put a windowed application before the first release: a terminal IDE
is reachable right after M2 and self-hosting, a windowed one only after V2,
Windows and a text-rendering stack. Both share every layer except the renderer
(§4). Rationale and rejected alternatives: design 0021.

## 4. Architecture

```
+------------------------- olide: one process, one event loop -------------------------+
| core/   piece-table documents, undo tree, selections, search, encodings, atomic save  |
| ui/     widgets, layout, commands, key map, palette, themes; renderer-independent     |
| term/   raw mode, VT input parser, cell grid, damage renderer               (I2)      |
| gfx/    pixel renderer: Wayland / Win32, glyph cache, font rasterizer       (I7)      |
| lang/   OLIS/0 client: document sync, versioned queries, stale-result rejection       |
| build/  oli.toml model, process supervisor, output and diagnostics panels, test tree  |
| debug/  ptrace client, breakpoints, stepping, frames, places, memory, disasm (I4)     |
| tools/  AST / OIR / machine / layout / cost / capability views              (I5)      |
+--------------------------------------------------------------------------------------+
      | pipes: OLIS/0            | pipes: stdout, stderr, exit status       | ptrace
+-------------------+     +------------------------------+        +----------------+
| olis              |     | oli build / run / test, olic |        | debuggee       |
| imports compiler.*|     +------------------------------+        +----------------+
+-------------------+
```

Principles:

1. **One event loop, no threads before V2.** `poll` over terminal input, the
   service pipe, child-process pipes, a timer and a signal fd. Every handler is
   bounded; anything long runs in a child process. When threads arrive (V2)
   the service may move in-process; the protocol and the client do not change.
2. **Zones for transient work, one explicit allocator for long-lived state.**
   Each event-loop iteration opens a scratch zone (rendering, one query, one
   frame decode) and releases it at `end`. Documents, undo history and caches
   live in `std.alloc`, an mmap-backed size-class allocator written in Oli--;
   `ALLOC` is a visible cost class, so allocation sites are listed by
   `--explain`. Lexical zones cannot hold state across iterations, so the
   allocator is a prerequisite (§3), not an optimization.
3. **Versioned documents.** Every edit increments the document version; every
   service result carries the version it was computed for; the client applies
   a result only when versions match. Pure decorations (highlighting, folding)
   may be mapped through the edit log instead of discarded.
4. **Same code as the command line.** `olis` imports the `compiler.*` modules;
   nothing about the language is re-implemented in the editor. Highlighting
   comes from the compiler's token stream, never from regular expressions.
   For zero-latency colouring of the line being typed, `olide` links the same
   `compiler.lexer` module in-process; there is one lexer.
5. **Deterministic and replayable.** Input is a stream of events. A session can
   be recorded to `.olirec` and replayed headless; every crash report is a
   replay test.

## 5. Editor core (I2)

**Text model.** A piece table: an original buffer (the file, read once or
mapped read-only), an append-only add buffer, and a piece list. Per-piece line
counts give line ↔ offset lookup; positions are 1-based line and character
column, exactly as compiler diagnostics count them, so a diagnostic maps to a
cursor position with no conversion. UTF-8 only; invalid input is opened
read-only with `E0007`-style markers. Line endings are detected per file,
preserved on save and shown in the status bar. Tabs display at a configurable
width; the formatter writes four spaces.

**Undo.** An undo tree of piece-list snapshots. Pieces are small, so a snapshot
per edit group is cheap; branches are kept, never discarded.

**Selections.** Multiple selections from day one (a data-structure decision that
cannot be retrofitted); one primary. Block selection is a later addition.

**Files.** Atomic save: write a temporary file, `fsync`, `rename`. External
change detection by size and mtime on focus and before save, `inotify` later.
A save over a file that changed on disk opens a reconciliation view (theirs,
mine, merge) and never overwrites silently. A recovery journal per open
document (an append-only edit log in the state directory) is replayed after a
crash; the journal is deleted on a successful save.

**Search.** Literal, whole-word and case-insensitive search in a document and
across the project, using the same buffer code. Regular expressions arrive
with `std.regex`, not before.

**Terminal renderer.** Raw mode through `ioctl(TCGETS/TCSETS)`; `SIGWINCH`
through `signalfd`; a VT input parser (keys, modifiers, SGR mouse, bracketed
paste); a cell grid with attributes; damage-based output of changed cells only;
synchronized output where supported; truecolor with 256- and 16-colour
fallback. Terminal matrix for tests: xterm, Windows Terminal over WSL, kitty,
tmux, the Linux console.

**UI.** Widgets: editor view, tab strip, file tree, bottom panel (diagnostics,
output, tests, search), status bar, command palette, prompts. Every action is
a named command; the palette lists them; the key map binds them and is fully
rebindable in `olide.toml`. Default bindings are non-modal chords.

## 6. Language service `olis` and the OLIS/0 protocol (I1)

**Process model.** `olide` spawns `olis` with two pipes. Both directions carry
frames; a frame is a header layout followed by a payload layout. Because every
field is an endian-typed integer, the protocol is written in Oli-- itself and
parsed with `Header.at(v)`, with no text format and no parser to maintain:

```oli
layout FrameHeader packed
    magic   : le u32        -- "OLIS"
    length  : le u32        -- payload bytes that follow
    kind    : le u16        -- message kind (§6 table)
    flags   : le u16
    request : le u32        -- request id; 0 for notifications
    doc     : le u32        -- document id; 0 when not about a document
    version : le u32        -- document version the message refers to
end
```

Messages, client → service:

| Kind | Meaning |
|------|---------|
| `set_project` | root directory, library directories, target (hosted/freestanding) |
| `open`, `change`, `close` | document lifecycle; `change` carries the full text in OLIS/0 (the parser re-parses whole files anyway); a range-edit kind is reserved |
| `cancel` | cancel a request id |
| `tokens`, `folding` | classified token stream and fold ranges |
| `diagnostics` | request the current set explicitly (also pushed) |
| `symbols` | document or project symbols |
| `complete`, `signature`, `hover` | at a position |
| `definition`, `references` | at a position |
| `rename_preview`, `format` | return edits; the client applies them |
| `explain_cost`, `layout_info`, `capabilities` | Oli-- specific queries (§9) |

Service → client: `result`, `error` (with a code), `diagnostics` (pushed after
each analysis), `progress`, `cancelled`.

Rules:

- Every answer echoes `doc` and `version`. A request whose version is older
  than the newest `change` of that document is answered `cancelled` at once.
- Requests are processed in order; the service polls its input pipe between
  analysis passes and honours `cancel` at pass boundaries, so cancellation
  latency is bounded by one pass over one module.
- A malformed frame ends the session with an error frame; the IDE restarts the
  service and re-sends its open documents. A service crash never takes the
  editor down and never loses text.
- Neither side may crash on any byte sequence; truncated and random frames are
  part of the test suite, like truncated source is for the parser.

**Analysis model.** Per document: lex and parse on every change (whole file;
the parser recovers, so unfinished programs still yield complete trees and
all their diagnostics, design 0013). Semantic analysis over the module graph
with invalidation at module granularity: a change re-analyzes the module and
its dependents; other results stay cached by module and version set. Every
query answers from the semantic graph of design 0014, the same structure that
`olic --check` builds. Per-declaration incrementality is a later optimization
gated on measurements, not assumed.

**Foreign editors.** `olis-lsp`, written in Oli--, maps JSON-RPC to OLIS/0 so
Neovim, Helix or VS Code can use the service. It needs `std.json`, comes
after the initial release, and is never a prerequisite for `olide`.

## 7. Language features and their stage

| Feature | Source of truth | Stage |
|---------|-----------------|-------|
| highlighting: keyword, primitive type, identifier, path, integer, char, string, comment, doc comment, operator, clause keyword, capability, mnemonic, register, local label; **bind `:=`, store `<-` and move `<~` are three distinct classes** | compiler token stream | I2 |
| diagnostics inline and in the panel, with code and the explanation text generated from the spec code tables | `olis` | I2 |
| folding of `proc`, `layout`, `choice`, `zone`, `if`, `while`, `each`, `loop`, `case`, `machine` | parser | I2 |
| completion: names in scope, fields after `.`, variants after `when`, capabilities after `permit`, header clauses, mnemonics and registers inside `machine`, keywords by position | semantic graph | I3 |
| signature help; hover: type, binding vs place, region (frame, zone, static), cost class of the line, capabilities a call needs | semantic graph, `--explain-cost` | I3 |
| go to definition, references, document and project symbols | semantic graph | I3 |
| rename with a reviewable multi-file preview; comments and unrelated names untouched | semantic graph | I3 |
| format through the `oli fmt` printer, which never changes tokens | printer | I3 |
| code actions: add the missing `permit`, insert a missing `end`, resolve a fallible value with `else fail`, turn `if … then` into a block | diagnostics | I3, optional |

## 8. Build, run, test and debug

**I3, project integration.** The file tree and targets come from `oli.toml`.
Build, check, run and test are commands with an explicit target and profile.
`olic` gains a `--diagnostics=olis` flag that writes OLIS/0 diagnostic frames
to a file descriptor, so the build panel shows clickable errors without
parsing text. The process supervisor starts children with argument arrays,
never a shell string; runs them in their own process group; stops exactly that
group; forwards stdin; and streams stdout and stderr into the output panel
without ever blocking the event loop. A build result carries the hash of the
artifact it produced, and Run refuses an artifact whose hash does not match
the last successful build: a failed build never runs a stale executable. Test
results appear as a tree with pass, fail and clickable failure positions.

**I4, debugger.** First a specification: ODI, the own debug information in an
ELF section `.oli.debug`: a line table, procedure ranges, frame layout, the
location of every place (frame offset, register or static), types and layouts,
and the zone map. `olic --debug` emits it. Then the client in `olide` over
`ptrace`: breakpoints by `int3` patching, step into, over and out through the
line table, the call stack through the frame chain of `docs/ABI.md`, a places
view typed by ODI, memory and register views, and disassembly through an own
decoder whose tables are shared with the encoder. A trap is explained in
source terms: which `CHECK` fired, on which line, with its cost tag. Debugger
availability is its own acceptance gate.

## 9. Oli-- tools (I5): what makes this an Oli-- IDE

- **Cost gutter.** The cost class of every line (`ZERO`, `CHECK`, `ZONE`,
  `CALL`, `SYSCALL`, …) from `--explain-cost`, live while typing.
- **Capability lens.** A procedure header shows the capabilities its body
  uses transitively; a call needing a capability the procedure does not
  `permit` is marked before the compiler error.
- **Zone and view lifetimes.** For a selected view or reference: its region,
  the zone it belongs to and the point where it would escape.
- **Layout inspector.** Hovering a `layout` shows its byte map: offsets, sizes,
  padding, alignment and the endianness of every field.
- **Pipeline views.** Source, AST, OIR, machine IR and bytes side by side,
  synchronized by source range.
- **Machine blocks.** The encoded bytes of every line of a `machine x64` block
  shown inline, from the same encoder that builds the program.

None of these needs a new analysis; they expose what `olic --explain` already
computes. The originality review of the IDE stage checks that each one shows
something no other editor can show, because no other language has it.

## 10. Packaging, settings and documentation

- One static executable per target next to `olic`; `oli ide` finds it. No
  installer dependencies; a clean machine needs only the file.
- State directory: `$XDG_STATE_HOME/olide` on Linux, `%LOCALAPPDATA%\olide`
  on Windows: journals, session, recent projects.
- `olide.toml` holds settings and the key map, with a `format` version field;
  unknown keys are warnings, never errors.
- `docs/ide/USER_GUIDE.md`; the key map reference is generated from the
  command table so it cannot drift.
- Reproducible build: building `olide` twice yields identical bytes, which
  the deterministic-compiler gate already requires.

## 11. Testing and benchmarks

- **Editor core.** Unit tests for the piece table, undo tree and encodings;
  randomized edit sequences checked against a naive string model.
- **Rendering.** Golden tests: render to a cell grid and compare a text dump;
  no terminal is needed.
- **Replay.** `.olirec` recordings replayed headless; every crash reproduction
  becomes a test.
- **Service.** Every `tests/parse/err` and `tests/sema/err` fixture must yield
  the same codes and positions through OLIS/0 as through `olic`; protocol fuzz
  (truncated and random frames) on both sides; stale-version and cancellation
  tests with injected delays.
- **Integration.** From a clean machine: create, edit, save, reopen, build and
  run a hello project; `kill -9` during save leaves the old or the new file
  intact; an external modification forces reconciliation.
- **Benchmarks.** Measured with `oli bench` on recorded small, medium and
  large projects; the reference machine (CPU, RAM, kernel, terminal) is
  recorded in every log and published with each release candidate.

| Measure | Provisional budget, confirmed at I2 exit |
|---------|------------------------------------------|
| key to paint, terminal, file ≤ 1 MB | ≤ 16 ms at p99 |
| open a 10 MB file | ≤ 200 ms |
| diagnostics after the last keystroke, module ≤ 2k lines | ≤ 100 ms |
| completion popup | ≤ 50 ms |
| resident memory | ≤ 32 MB plus three times the open text |

## 12. Stages and exit criteria

| Stage | Deliverables | Exit criteria |
|-------|--------------|---------------|
| **I0 specifications** (can start now) | `spec/OLIS_PROTOCOL_V0.md`; `docs/ide/EDITOR_MODEL.md` (piece table, undo, positions, journal); `docs/ide/UI.md` (commands, default key map, themes, token classes); `docs/ide/TEST_CORPUS.md` (the recorded projects); design 0021 accepted after critique and originality review | specs reviewed; no code written |
| **I1 `olis`** | the service over OLIS/0 with `set_project`, document lifecycle, `tokens`, `folding`, `diagnostics`, `symbols`, cancellation | all parse and sema fixtures pass through the service; fuzz and stale-version tests pass; diagnostics for a 1k-line module within budget |
| **I2 `olide` terminal editor** | core, ui, term, lang client; highlighting, diagnostics, folding; search; recovery | edit, save, reopen; `kill -9` and conflict tests pass; golden and replay suites exist; budgets measured |
| **I3 project integration** | build, run, test, output panel, completion, hover, navigation, rename, format | hello project end to end from a clean install; rename preserves comments; Run never starts a stale artifact |
| **Packaging → initial release** | static executables, `olide.toml`, user guide, benchmark report | §13 passes on Linux |
| I4 debugger | ODI spec, `olic --debug`, ptrace client | separate gate |
| I5 Oli-- tools | cost gutter, capability lens, lifetimes, layout inspector, pipeline views | separate gate |
| I6 Windows | console renderer over the Windows console API, path and process differences | §13 passes on Windows |
| I7 native window | Wayland and Win32 pixel renderers, font rasterizer, glyph cache | same suites as I2 through the pixel renderer |

Each stage ends with a report: what works, what does not, tests, design
changes, measured performance, next milestone (CONTRIBUTING.md).

## 13. Acceptance criteria for the initial release

- From a clean install: create, edit, save, reopen, build and run an Oli--
  hello project without a foreign compiler anywhere in the chain.
- A syntax or type error appears at the correct position; fixing it removes
  it. Editing while analysis runs never applies results for an older version.
- Completion and navigation use the same symbol resolution as a command-line
  build; rename updates every reference and leaves comments and unrelated
  names untouched.
- Undo, crash recovery and external changes never lose user text silently;
  conflicting disk changes require reconciliation before an overwrite.
- Build and test failures stay visible, failed builds never run a stale
  executable, cancellation works, and child-process output cannot freeze the
  UI.
- The §11 budgets are met on the documented reference machine and the
  measurements are published.
- The IDE, the service and the protocol are written in Oli--; platform
  bindings and packaging introduce no foreign implementation of the language.

## 14. Risks and how the plan answers them

| Risk | Answer |
|------|--------|
| lexical zones cannot hold editor state | `std.alloc` is a prerequisite of I1; zones serve per-iteration scratch |
| no threads before V2 | one event loop; every blocking operation is a child process; handlers are bounded |
| terminal diversity | a test matrix and graceful fallback to 16 colours and plain keys |
| a font rasterizer is a large component | I7 is last and independent; the terminal IDE is the release |
| an out-of-process service adds a copy per query | payloads are layouts, copied once; measured against the budgets before any redesign |
| scope creep | every feature outside §7 through §9 needs a design record first |

## 15. Completion evidence

Store with each release: end-to-end test logs, reproducible build
instructions, platform checks, editor recovery tests, replay recordings and
latency measurements. Mark the IDE complete only when §13 passes on both
supported targets.
