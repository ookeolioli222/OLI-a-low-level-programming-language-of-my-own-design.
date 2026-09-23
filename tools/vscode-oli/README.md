# Oli-- in VS Code, step by step

Everything here is outside the toolchain: a command-line wrapper, tasks that
call it, and a grammar for highlighting. The editor of the roadmap — `olis`
and `olide` (`docs/IDE_PLAN.md`) — is written in Oli-- and replaces this.

## 1. Build the compiler once

```bash
cd /path/to/NOWY
./genesis/test.sh          # builds the whole chain from 322 bytes and runs every test (a few minutes)
```

This produces `genesis/build/olic` and the `genesis/build/show_*` drivers.
Nothing else needs installing: no libc, no assembler, no linker.

## 2. Put `olic` on the PATH

`bin/olic` is a shell script that gives the compiler a command line (the
compiler itself reads stdin; an Oli-- program cannot read `argv` yet):

```bash
echo 'export PATH="/path/to/NOWY/bin:$PATH"' >> ~/.bashrc   # or ~/.zshrc, ~/.profile
source ~/.bashrc
olic --help
```

Then, from any directory:

```bash
olic hello.oli            # writes ./hello, executable
./hello
olic hello.oli -o out/hello
olic --check hello.oli    # diagnostics only, no file
olic --show-opt hello.oli # what the compiler made of it, after the passes
```

The exit status is the compiler's: 0 built, 1 diagnostics reported (and no
file written), 4 a defect in the compiler itself. `import` resolves under
`lib/` of the current directory, so a program that imports library modules
is compiled from the repository root.

## 3. Highlighting

VS Code loads an extension from `~/.vscode/extensions`; a symlink is enough:

```bash
ln -s /path/to/NOWY/tools/vscode-oli ~/.vscode/extensions/oli-minus-minus
```

Reload the window (`Developer: Reload Window`). `.oli` files are now the
language "Oli--": keywords, primitive types, literals, comments and doc
comments, capabilities, and bind `:=`, store `<-` and move `<~` as three
distinct scopes, as `docs/IDE_PLAN.md` asks. A keyword after `.` is a member
name and is not coloured (`spec/OLI_SYNTAX_V0.md` §2.3).

## 4. Testing a program from the editor

Open the repository folder in VS Code (`File > Open Folder`), so that
`.vscode/tasks.json` is picked up. Then, with a `.oli` file open:

| Do | Result |
|----|--------|
| **Ctrl+Shift+B** | compiles the file next to itself; diagnostics appear in the **Problems** pane, each one clickable to its line and column |
| `Terminal > Run Task… > oli: compile and run current file` | compiles, runs, prints the program's output and `[exit N]` |
| `Run Task… > oli: check current file` | analyses without writing a file — the fastest way to see if a construct is accepted, or which `E0xxx` it gets |
| `Run Task… > oli: show OIR after the passes` | the program after constant folding and check elision; every removed check is printed where it stood with its proof |
| `Run Task… > oli: show semantic graph` | the type and region of every expression |
| `Terminal > Run Test Task` (or `Run Task… > oli: build toolchain and run every test`) | `./genesis/test.sh`: the whole chain, layers 0–5 |

The convention of the repository's own fixtures is the one to copy for a
test program: exit **42** when every check held and **N** for the number of
the check that failed, so the exit status names the failure —

```oli
module mine
proc start -> s32
    entry
    if 2 + 2 != 4
        ret 1
    end
    ret 42
end
```

A program that must trap says so in a comment, and the harness checks the
message and the status 134:

```oli
-- expect: trap: overflow at stdin:6
```

Drop a file into `tests/run/` (exit 42, or a `.out` file it must print and
exit 0; a `.in` file is its stdin, `/dev/null` otherwise) or into
`tests/run/trap/`, and `./genesis/test.sh` picks it up on the next run; that
is how every construct in `docs/COMMANDS.md` marked **runs** is pinned.

## 5. What to expect

Everything marked **runs** in `docs/COMMANDS.md` compiles and runs; anything
marked **analysed** is type-checked and then refused with `E0900` and no
file — the compiler never approximates. There is no debugger, no formatter
and no language server yet; those are I1–I4 of `docs/IDE_PLAN.md`.
