# 0013 — Newline-terminated statements, bracket trivia and `end`-anchored recovery

## Problem
The parser must (a) report every syntax error in a file in one run, (b) never
panic on any input, (c) point at the place a human would point at, and (d) do
this without a separate layout/indentation lexer.

## Existing approaches
- C/Rust/Zig: `;` terminators; recovery skips to the next `;` or `}`; errors in
  a missing `;` cascade badly.
- Python: an INDENT/DEDENT lexer; whitespace-sensitive; fragile for generated code.
- Go: automatic semicolon insertion by token class; a documented but surprising rule.
- Lua/Ruby: keyword/`end` blocks; recovery by `end` keywords.

## Oli-- approach
1. **Newlines are tokens.** A statement ends at a newline; runs of newlines are
   collapsed by the lexer, so blank lines and comment-only lines are invisible.
2. **Continuation is structural, not textual.** After a binary operator, `:=`, `<-`,
   `<~`, `,` or `.` the parser skips newlines (a line that *ends* in an operator
   continues). Inside unclosed `( [ {` every newline is trivia. A line that
   *starts* with an operator never continues the previous line.
3. **Statement-level recovery.** A failed simple statement is dropped and parsing
   resumes at the next line. A failed *block header* (`if`, `while`, `zone`, …)
   yields an `Error` expression and keeps its body, so its `end` still closes it.
4. **`end` is sacred.** Line recovery never skips an `end`/`else`/`elif`/`when`
   that begins a line: the error was on the previous line (typically a dangling
   operator) and the keyword must still close its block. A declaration keyword
   inside a body means "missing `end` above": the parser reports it once and
   unwinds to module level, where the next declaration parses normally.
5. **Anchored positions.** "Expected X here" points at the current token, unless a
   line ended since the last significant token — then it points just after that
   token. `total <-` followed by `end` therefore reports at column 13 of the
   `total <-` line, exactly as `spec/OLI_SYNTAX_V0.md` §8 shows.
6. **Bounded recursion.** Expression and block nesting is limited (`E0030`), and
   `olic` runs its front end on a 64 MiB thread; truncation and garbage inputs are
   part of the test suite.

## Advantages
- One token of lookahead almost everywhere (two for `name :`, `T.wrap(`, `in REG <-`).
- Whitespace-insensitive source: safe for generators, copy-paste and merges.
- Cascades are rare: one dangling operator or one `{` produces one primary error
  plus at most one explanatory note (`E0032`).

## Disadvantages
- An unclosed `(` swallows the following line(s) until `)`; the swallowed
  statements are lost for that run (a second run after the fix reports them).
- Deeply generated expressions must stay under the nesting limit (96 levels).

## Machine cost
None (front end only). The parser handles ~600k lines/s in a release build.

## Safety implications
The compiler cannot be crashed by source text: no `unwrap`, `expect`, `panic`
or slice indexing is allowed in the compiler crates (denied by clippy), and the
fuzz-style tests exercise every truncation of the reference programs.

## Alternatives rejected
- Semicolons: noise the machine does not need; worse cascades.
- Offside rule: rejected in `docs/SYNTAX_EXPERIMENTS.md`.
- Error nodes everywhere (never dropping a statement): more partial-tree
  complexity for later stages with little diagnostic gain.
