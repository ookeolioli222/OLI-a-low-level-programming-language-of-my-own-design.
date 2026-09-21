#!/bin/sh
# hexbin.sh - materialize a hex listing as raw bytes with nothing but POSIX sh.
#
# This is the one and only step of the Oli-- bootstrap that is not performed
# by an Oli-- tool: it turns genesis/0-hex0/hex0.hex into the seed binary
# exactly as a person could by typing the bytes into a hex editor. From then
# on hex0 itself converts every listing, including its own.
#
# Listing format: two hex digits per byte; ';', '#' or '-' start a comment
# that runs to the end of the line; whitespace is ignored.
set -e
in="$1"; out="$2"
bs=$(printf '\134')
: > "$out"
{ sed 's/[;#-].*//' "$in" | tr -d ' \t\r\n'; echo; } | fold -w2 | while read -r pair; do
    [ -n "$pair" ] || continue
    printf "${bs}$(printf '%03o' "$((0x$pair))")" >> "$out"
done
