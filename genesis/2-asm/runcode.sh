#!/bin/sh
# runcode.sh CODEHEX -- behavioral oracle for encodings.
# Builds an ELF from the fixed 120-byte prefix + the given code hex, padded to
# 256 bytes, using our own hex0; runs it; prints the exit status. No external
# assembler is used anywhere. Argument is a file of hex bytes (code only).
set -e
here="$(dirname "$0")"
hex0="$here/../build/hex0.bin"
code="$1"
out="$here/../build/code.elf"
# 120 prefix bytes + code + zero padding to 256.
nbytes=$(sed 's/[;#-].*//' "$code" | tr -cd '0-9a-fA-F' | wc -c)
nbytes=$((nbytes / 2))
pad=$((256 - 120 - nbytes))
{
    cat "$here/tests/elf-prefix.hex"
    cat "$code"
    i=0; while [ $i -lt $pad ]; do printf '00 '; i=$((i+1)); done; echo
} | "$hex0" > "$out"
chmod +x "$out"
set +e
"$out"
echo $?
