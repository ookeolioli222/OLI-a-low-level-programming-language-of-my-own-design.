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
olic --explain hello.oli  # the frame, the checks and their proofs, the cost of every line
olic --show-asm hello.oli # the bytes of every machine x64 block
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
| `Run Task… > oli: boot the kernel in QEMU` | compiles `examples/kernel.oli` and boots it (§5); COM1 is the terminal |
| `Terminal > Run Test Task` (or `Run Task… > oli: build toolchain and run every test`) | `./genesis/test.sh`: the whole chain, layers 0–6 |

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

## 5. Booting the kernel

`examples/kernel.oli` is a kernel that boots from a Multiboot loader. QEMU
runs it; install it once:

```bash
sudo apt install qemu-system-x86
```

In VS Code: `Terminal > Run Task… > oli: boot the kernel in QEMU`. From a
terminal it is the same script:

```bash
tools/run-kernel.sh                  # examples/kernel.oli
tools/run-kernel.sh my_kernel.oli    # any freestanding program with a Multiboot header
OLI_QEMU_MEM=128 tools/run-kernel.sh # more memory: the kernel reports more free frames
```

The script compiles the file to `genesis/build/NAME.elf`, starts QEMU with no
window and COM1 on the terminal, and ends when the kernel writes QEMU's
isa-debug-exit port. The output is:

```
Oli-- kernel
cpu: AuthenticAMD                          (or GenuineIntel)
memory: 2 regions, 15840 frames free
frame: 0000000000200000 written and read
heap: 1000 records at 0000000000200000, sum of squares 332833500, 1M refused, frames returned
int 3 at 0000000000102229
back from int 3
timer: 100 ticks
[the kernel finished: isa-debug-exit, qemu status 33]
```

To watch the VGA text screen too, run QEMU yourself without `-display none`:

```bash
qemu-system-x86_64 -kernel genesis/build/kernel.elf -serial stdio \
    -device isa-debug-exit,iobase=0x501,iosize=1
```

## 6. Debugging

Every executable carries DWARF line tables, so gdb stops on `.oli` lines,
names procedures (`kernel.main`, `core.frames.alloc`) and prints the call
stack. VS Code drives gdb through the **C/C++** extension
(`ms-vscode.cpptools`); install it from the Extensions view. `gdb` itself
comes with `sudo apt install gdb`.

`.vscode/launch.json` has two configurations (`Run and Debug` view, or F5):

| Configuration | What it does |
|---------------|--------------|
| **oli: debug current file (gdb)** | compiles the open `.oli` file (it must have `entry`) and runs it under gdb; a breakpoint set by clicking left of a line number stops there |
| **oli: debug the kernel in QEMU (gdb)** | boots the kernel halted (`tools/run-kernel.sh --gdb`), attaches gdb to QEMU on port 1234, and runs; breakpoints in `examples/kernel.oli` and in `lib/` stop the virtual CPU; stopping the session stops QEMU |

`.vscode/settings.json` sets `debug.allowBreakpointsEverywhere`, because VS
Code only lets you click a breakpoint into a language a debugger declares.

Two things to know. The line table names each module's file as the module
path says (`core/frames.oli` for `core.frames`), so both configurations tell
gdb where to look: next to the program, in `examples/`, and in `lib/`.

The Variables pane shows parameters and locals with their types — integers,
`bool`, arrays, layouts with their fields, views (`addr`, `len`; the text is
`-exec p *msg.addr@msg.len`), addresses and refs. By default the compiler keeps
most scalars in registers (`mem2reg`), and those show as `<optimized out>`;
put this line at the top of the file to keep every local in its frame words
while you debug it, the `-O0` of this compiler:

```oli
-- debug: frame
```

The same from a terminal:

```bash
bin/olic tests/run/pmm.oli -o /tmp/pmm
gdb -ex 'directory tests/run:lib' -ex 'break pmm.oli:57' -ex run /tmp/pmm

tools/run-kernel.sh --gdb &             # halted, gdb server on :1234
gdb -ex 'directory examples:lib' -ex 'target remote :1234' \
    -ex 'break kernel.memory_init' -ex continue genesis/build/kernel.elf
```

## 7. What to expect

Everything marked **runs** in `docs/COMMANDS.md` compiles and runs; anything
marked **analysed** is type-checked and then refused with `E0900` and no
file — the compiler never approximates. There is no formatter and no
language server yet; those are I1–I4 of `docs/IDE_PLAN.md`.
