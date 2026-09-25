#!/bin/sh
# run-kernel - compile examples/kernel.oli (or the file given) and boot it
# under QEMU: COM1 on this terminal, no window, and the isa-debug-exit
# device the kernel writes when it is done (QEMU then exits with 33).
#
#   tools/run-kernel.sh [FILE.oli]          boot
#   tools/run-kernel.sh --gdb [FILE.oli]    boot halted, gdb server on :1234
#
# QEMU is not part of the toolchain: `sudo apt install qemu-system-x86`,
# or name a binary with OLI_QEMU (OLI_QEMU_BIOS / OLI_QEMU_DATA for its
# firmware directories). OLI_QEMU_MEM sets the memory (default 64 MiB).
# Nothing here is part of the toolchain.

root=$(cd "$(dirname "$0")/.." && pwd)
gdb=
[ "$1" = "--gdb" ] && { gdb="-s -S"; shift; }
src=${1:-$root/examples/kernel.oli}
out="$root/genesis/build/$(basename "${src%.oli}").elf"

qemu=${OLI_QEMU:-$(command -v qemu-system-x86_64)}
[ -n "$qemu" ] || { echo "run-kernel: no qemu-system-x86_64 (sudo apt install qemu-system-x86, or set OLI_QEMU)" >&2; exit 2; }
fw=
[ -n "$OLI_QEMU_BIOS" ] && fw="-L $OLI_QEMU_BIOS"
[ -n "$OLI_QEMU_DATA" ] && fw="$fw -L $OLI_QEMU_DATA"

"$root/bin/olic" "$src" -o "$out" || exit $?
echo "run-kernel: $out, $(wc -c < "$out") bytes"
[ -n "$gdb" ] && echo "run-kernel: halted before the first instruction; gdb server on localhost:1234"

"$qemu" $fw -accel tcg -display none -nic none -no-reboot -m "${OLI_QEMU_MEM:-64}" \
    -serial stdio -device isa-debug-exit,iobase=0x501,iosize=1 -kernel "$out" $gdb
st=$?
if [ "$st" = 33 ]; then
    echo "[the kernel finished: isa-debug-exit, qemu status 33]"
    exit 0
fi
echo "[qemu status $st]"
exit $st
