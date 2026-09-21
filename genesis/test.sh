#!/bin/sh
# test.sh - verifies the genesis chain. Run on Linux x86-64 from any directory:
#   sh genesis/test.sh
# Uses only POSIX sh and coreutils (cmp, od, wc, tr, head, yes, fold, sed).
set -e
cd "$(dirname "$0")"
mkdir -p build
fail() { echo "FAIL: $1"; exit 1; }

# --- layer 0: hex0 ---
sh hexbin.sh 0-hex0/hex0.hex build/hex0.bin
chmod +x build/hex0.bin
[ "$(wc -c < build/hex0.bin)" -eq 322 ] || fail "hex0.bin is not 322 bytes"
./build/hex0.bin < 0-hex0/hex0.hex > build/hex0.self.bin
cmp build/hex0.bin build/hex0.self.bin || fail "hex0 does not reproduce itself"
echo "ok: hex0 reproduces its own binary (322 bytes)"

./build/hex0.bin < 0-hex0/tests/exit42.hex > build/exit42
chmod +x build/exit42
set +e
./build/exit42
status=$?
set -e
[ "$status" -eq 42 ] || fail "exit42 returned $status"
echo "ok: hex0 output runs (exit42)"

./build/hex0.bin < 0-hex0/tests/format.hex > build/format.bin
got=$(od -A n -t x1 build/format.bin | tr -d ' \n')
[ "$got" = "0001abcdef10ff7fa5" ] || fail "format test produced $got"
echo "ok: comments (; # -), case, tabs, missing final newline"

yes 'ab' | head -6000 > build/big.hex
./build/hex0.bin < build/big.hex > build/big.bin
[ "$(wc -c < build/big.bin)" -eq 6000 ] || fail "big output is $(wc -c < build/big.bin) bytes"
od -v -A n -t x1 build/big.bin | tr -d ' \n' > build/big.got
yes 'ab' | head -6000 | tr -d '\n' > build/big.want
cmp build/big.got build/big.want || fail "big output differs"
echo "ok: 6000-byte output crosses the buffer boundary"

echo "genesis: layer 0 passed"

# --- layer 1: hex2 (labels) ---
./build/hex0.bin < 1-hex2/hex2.hex0 > build/hex2.bin
chmod +x build/hex2.bin
[ "$(wc -c < build/hex2.bin)" -eq 882 ] || fail "hex2.bin is not 882 bytes"
./build/hex2.bin < 1-hex2/hex2.hex0 > build/hex2.self.bin
cmp build/hex2.bin build/hex2.self.bin || fail "hex2 does not reproduce itself"
echo "ok: hex2 built by hex0 and reproduced by itself (882 bytes)"

./build/hex2.bin < 1-hex2/tests/labels.hex2 > build/labels
./build/hex0.bin < 1-hex2/tests/labels.expect.hex > build/labels.expect
cmp build/labels build/labels.expect || fail "label forms produced wrong bytes"
chmod +x build/labels
set +e
./build/labels
status=$?
set -e
[ "$status" -eq 42 ] || fail "labels program returned $status"
echo "ok: labels (: % ! & @) resolve and the program runs"

set +e
./build/hex2.bin < 1-hex2/tests/undefined.hex2 > build/undef.bin 2> build/undef.err
status=$?
set -e
[ "$status" -eq 2 ] || fail "undefined label: exit $status"
grep -q "hex2: undefined label nowhere" build/undef.err || fail "undefined label: message missing"
set +e
./build/hex2.bin < 1-hex2/tests/range.hex2 > build/range.bin 2> build/range.err
status=$?
set -e
[ "$status" -eq 3 ] || fail "rel8 range: exit $status"
grep -q "hex2: rel8 out of range" build/range.err || fail "rel8 range: message missing"
echo "ok: undefined label -> exit 2, rel8 out of range -> exit 3"

echo "genesis: layer 1 passed"

# --- layer 2: encoding behavioral oracle (each test must exit 42) ---
for t in A B C D E; do
    got=$(sh 2-asm/runcode.sh 2-asm/tests/t$t.hex)
    [ "$got" = "42" ] || fail "encoding test $t returned $got, want 42"
done
echo "ok: encoding behavioral tests A-E exit 42 on the real CPU"
echo "genesis: layer 2 (encoding oracle) passed"

# --- layer 2b: the asm assembler (register encoder, checked ELF writer) ---
./build/hex2.bin < 2-asm/asm.hex2 > build/asm.bin
chmod +x build/asm.bin
[ "$(wc -c < build/asm.bin)" -eq 8239 ] || fail "asm.bin size $(wc -c < build/asm.bin), want 8239"
./build/hex2.bin < 2-asm/asm.hex2 > build/asm.again
cmp build/asm.bin build/asm.again || fail "asm build is not deterministic"
./build/asm.bin < 2-asm/tests/exit42.asm > build/prog.elf
chmod +x build/prog.elf
set +e; ./build/prog.elf; st=$?; set -e
[ "$st" -eq 42 ] || fail "asm-built exit42 returned $st"
echo "ok: asm assembles a bytes-only program that runs (exit 42)"
sh 2-asm/test.sh
echo "genesis: layer 2b (asm register encoder) passed"

# --- layer 3: oli1 oli-core compiler (step 0: ret <int>) ---
./build/asm.bin < 3-oli1/oli1.oli > build/oli1.bin || fail "asm could not build oli1"
chmod +x build/oli1.bin
./build/asm.bin < 3-oli1/oli1.oli > build/oli1.again
cmp build/oli1.bin build/oli1.again || fail "oli1 build is not deterministic"
for v in 0 42 200; do
    printf 'proc start
 entry
 ret %s
end
' "$v" | ./build/oli1.bin > build/o3.elf
    chmod +x build/o3.elf
    set +e; ./build/o3.elf; st=$?; set -e
    [ "$st" = "$v" ] || fail "oli1: ret $v produced exit $st"
done
./build/oli1.bin < 3-oli1/tests/ret42.oli > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf; st=$?; set -e
[ "$st" = 42 ] || fail "oli1: ret42.oli fixture produced exit $st"
echo "ok: oli1 compiles 'ret <int>' oli-core to a native ELF that exits with the value"
# step 1: os.syscall + multi-statement bodies (real variable-length codegen)
./build/oli1.bin < 3-oli1/tests/syscall_exit.oli > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf; st=$?; set -e
[ "$st" = 42 ] || fail "oli1: os.syscall(60,42) exit $st"
printf 'proc start
 entry
 os.syscall(39)
 ret 3
end
' | ./build/oli1.bin > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf; st=$?; set -e
[ "$st" = 3 ] || fail "oli1: multi-statement (getpid then ret) exit $st"
printf 'proc start
 entry
 os.syscall(60, 8, 0, 0, 0, 0, 0)
end
' | ./build/oli1.bin > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf; st=$?; set -e
[ "$st" = 8 ] || fail "oli1: six-argument syscall exit $st"
echo "ok: oli1 compiles os.syscall(nr, args) and multi-statement bodies to native code"
# step 2: symbol table, expression precedence, bindings, names as syscall args
for pair in "bind 42" "arith 14" "prec 16" "nameos 7"; do
    set -- $pair
    ./build/oli1.bin < "3-oli1/tests/$1.oli" > build/o3.elf
    chmod +x build/o3.elf
    set +e; ./build/o3.elf; st=$?; set -e
    [ "$st" = "$2" ] || fail "oli1: $1.oli expected exit $2 got $st"
done
echo "ok: oli1 folds expressions with precedence, bindings and names in os.syscall args"
# step 3: string literals, .addr/.len, the oli-core hello
./build/oli1.bin < 3-oli1/tests/hello.oli > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf > build/o3.got; st=$?; set -e
[ "$st" = 0 ] || fail "oli1: hello.oli exit $st"
printf 'Hello Oli--\n' > build/o3.want
cmp build/o3.got build/o3.want || fail "oli1: hello.oli wrote wrong bytes"
./build/oli1.bin < 3-oli1/tests/escapes.oli > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf > build/o3.got; st=$?; set -e
[ "$st" = 0 ] || fail "oli1: escapes.oli exit $st"
got=$(od -A n -t x1 build/o3.got | tr -d ' \n')
[ "$got" = "610962415c22270d000a" ] || fail "oli1: escapes.oli wrote $got"
./build/oli1.bin < 3-oli1/tests/two_strings.oli > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf > build/o3.got; st=$?; set -e
[ "$st" = 5 ] || fail "oli1: two_strings.oli exit $st"
printf 'two\none\n' > build/o3.want
cmp build/o3.got build/o3.want || fail "oli1: two_strings.oli wrote wrong bytes"
./build/oli1.bin < 3-oli1/tests/strlen.oli > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf; st=$?; set -e
[ "$st" = 42 ] || fail "oli1: strlen.oli expected exit 42 got $st"
for bad in 'x := 5
 ret x.addr' 's := "abc
 ret 1' 's := "a\qb"
 ret 1' 's := "ab"
 ret s.foo' 's := "\x4G"
 ret 1' ' ret nosuch'; do
    set +e
    printf 'proc start\n entry\n %s\nend\n' "$bad" | ./build/oli1.bin > build/rj.elf 2> build/rj.err
    st=$?
    set -e
    [ "$st" = 2 ] || fail "oli1: malformed program exited $st instead of 2"
    [ "$(wc -c < build/rj.elf)" -eq 0 ] || fail "oli1: malformed program produced output"
    grep -q 'oli1: error' build/rj.err || fail "oli1: malformed program gave no diagnostic"
done
echo "ok: oli1 binds string literals with every escape, .addr/.len, rejects misuse"
# step 4: run-time locals, expressions, comparisons, if/elif/else, while, break, continue
for pair in "while_sum 55" "break_continue 7" "ops 26" "nested 9"; do
    set -- $pair
    ./build/oli1.bin < "3-oli1/tests/$1.oli" > build/o3.elf
    chmod +x build/o3.elf
    set +e; timeout 10 ./build/o3.elf; st=$?; set -e
    [ "$st" = "$2" ] || fail "oli1: $1.oli expected exit $2 got $st"
done
./build/oli1.bin < 3-oli1/tests/if_chain.oli > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf > build/o3.got; st=$?; set -e
[ "$st" = 0 ] || fail "oli1: if_chain.oli exit $st"
printf 'seven\n' > build/o3.want
cmp build/o3.got build/o3.want || fail "oli1: if_chain.oli took the wrong branch"
./build/oli1.bin < 3-oli1/tests/echo.oli > build/o3.elf
chmod +x build/o3.elf
set +e; printf 'Oli-- reads stdin\n' | ./build/o3.elf > build/o3.got; st=$?; set -e
[ "$st" = 18 ] || fail "oli1: echo.oli returned $st instead of the byte count"
printf 'Oli-- reads stdin\n' > build/o3.want
cmp build/o3.got build/o3.want || fail "oli1: echo.oli did not echo its input"
set +e; ./build/o3.elf < /dev/null > build/o3.got; st=$?; set -e
[ "$st" = 0 ] || fail "oli1: echo.oli on empty input exit $st"
[ "$(wc -c < build/o3.got)" -eq 0 ] || fail "oli1: echo.oli wrote on empty input"
echo "ok: oli1 compiles run-time locals, if/elif/else, while, break, continue, syscall results"
for bad in ' x <- 1' ' break' ' continue' ' if 1
 ret 1' ' while 1' ' ret 5 junk' ' s := "x"
 ret s' ' x := 1
 ret x.len' ' x := 1
 ret x <- 1' ' os.syscall(1 2)' ' foo bar' ' s := "x"
 s <- 1' ' os.syscall(1,2,3,4,5,6,7,8)' ' elif 1' ' ret (1 + 2'; do
    set +e
    printf 'proc start\n entry\n%s\nend\n' "$bad" | ./build/oli1.bin > build/rj.elf 2> build/rj.err
    st=$?
    set -e
    [ "$st" = 2 ] || fail "oli1: malformed program exited $st instead of 2"
    [ "$(wc -c < build/rj.elf)" -eq 0 ] || fail "oli1: malformed program produced output"
    grep -q 'oli1: error' build/rj.err || fail "oli1: malformed program gave no diagnostic"
done
echo "ok: oli1 rejects undefined names, stray break/continue/elif, unterminated blocks, trailing tokens"
echo "genesis: layer 3 (oli-core compiler, steps 0-4) passed"
