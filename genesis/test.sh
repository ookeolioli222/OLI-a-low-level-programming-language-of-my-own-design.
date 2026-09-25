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
# step 5: procedures, parameters, calls (SysV registers), forward calls, recursion
for pair in "fact 120" "fib 55" "six_args 91" "noret 42"; do
    set -- $pair
    ./build/oli1.bin < "3-oli1/tests/$1.oli" > build/o3.elf
    chmod +x build/o3.elf
    set +e; timeout 10 ./build/o3.elf; st=$?; set -e
    [ "$st" = "$2" ] || fail "oli1: $1.oli expected exit $2 got $st"
done
./build/oli1.bin < 3-oli1/tests/put.oli > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf > build/o3.got; st=$?; set -e
[ "$st" = 0 ] || fail "oli1: put.oli exit $st"
printf 'first second\n' > build/o3.want
cmp build/o3.got build/o3.want || fail "oli1: put.oli wrote wrong bytes"
echo "ok: oli1 compiles procedures with parameters, recursion, forward calls and results"
for bad in 'proc start
 entry
 ret nosuch(1)
end' 'proc f(a: u64)
 ret a
end
proc start
 entry
 ret f(1, 2)
end' 'proc start
 ret 1
end' 'proc start
 entry
 ret 1
end
proc other
 entry
 ret 2
end' 'proc f
 ret 1
end
proc f
 ret 2
end
proc start
 entry
 ret f()
end' 'proc f(a: u64, b: u64, c: u64, d: u64, e: u64, g: u64, h: u64)
 ret a
end
proc start
 entry
 ret f(1,2,3,4,5,6,7)
end' 'proc f(a)
 ret a
end
proc start
 entry
 ret f(1)
end' 'proc start
 entry
 ret 1
end
choice P
end'; do
    set +e
    printf '%s\n' "$bad" | ./build/oli1.bin > build/rj.elf 2> build/rj.err
    st=$?
    set -e
    [ "$st" = 2 ] || fail "oli1: malformed program exited $st instead of 2"
    [ "$(wc -c < build/rj.elf)" -eq 0 ] || fail "oli1: malformed program produced output"
    grep -q 'oli1: error' build/rj.err || fail "oli1: malformed program gave no diagnostic"
done
echo "ok: oli1 rejects undefined/duplicate procedures, arity mismatches, missing or multiple entry"
# step 6a: zones, views, raw memory, traps
for pair in "view_param 23" "raw 68" "zone_scope 6"; do
    set -- $pair
    ./build/oli1.bin < "3-oli1/tests/$1.oli" > build/o3.elf
    chmod +x build/o3.elf
    set +e; timeout 10 ./build/o3.elf; st=$?; set -e
    [ "$st" = "$2" ] || fail "oli1: $1.oli expected exit $2 got $st"
done
./build/oli1.bin < 3-oli1/tests/zone_bytes.oli > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf > build/o3.got; st=$?; set -e
[ "$st" = 116 ] || fail "oli1: zone_bytes.oli exit $st"
printf 'Hi\n' > build/o3.want
cmp build/o3.got build/o3.want || fail "oli1: zone_bytes.oli wrote wrong bytes"
./build/oli1.bin < 3-oli1/tests/subview.oli > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf > build/o3.got; st=$?; set -e
[ "$st" = 12 ] || fail "oli1: subview.oli exit $st"
printf 'OliHelloello\n' > build/o3.want
cmp build/o3.got build/o3.want || fail "oli1: subview.oli wrote wrong bytes"
./build/oli1.bin < 3-oli1/tests/slurp.oli > build/o3.elf
chmod +x build/o3.elf
set +e; printf 'hello, zone\n' | ./build/o3.elf > build/o3.got; st=$?; set -e
[ "$st" = 12 ] || fail "oli1: slurp.oli exit $st"
printf 'HELLO, ZONE\n' > build/o3.want
cmp build/o3.got build/o3.want || fail "oli1: slurp.oli did not upper-case its input in place"
echo "ok: oli1 compiles zones (mmap/munmap), z.bytes, views, subviews, byte and raw word access"
for trap in ' s := "abc"
 ret s[3]' ' s := "abc"
 w := s[2..1]
 ret w.len' ' s := "abc"
 w := s[1..4]
 ret w.len' ' zone z 4096
 b := z.bytes(5000)
 ret 1
 end' ' zone z 4096
 b := z.bytes(4000)
 c := z.bytes(100)
 ret 1
 end' ' zone z 4096
 b := z.bytes(10)
 i := 10
 b[i] <- 1
 ret 1
 end'; do
    printf 'proc start\n entry\n%s\nend\n' "$trap" | ./build/oli1.bin > build/t.elf
    chmod +x build/t.elf
    set +e; ./build/t.elf > build/t.out 2> build/t.err; st=$?; set -e
    [ "$st" = 3 ] || fail "oli1: bounds/zone violation exited $st instead of trapping"
    [ "$(wc -c < build/t.out)" -eq 0 ] || fail "oli1: trapping program wrote to stdout"
    grep -q 'oli: trap' build/t.err || fail "oli1: trap gave no diagnostic"
done
echo "ok: bounds, subview and zone-exhaustion violations trap with exit 3"
for bad in 'proc start
 entry
 s := "x"
 s[0] <- 1
end' 'proc start
 entry
 x := 1
 ret x + "a"
end' 'proc start
 entry
 v := "a"
 v <- 1
end' 'proc f(v: view u8)
 ret 1
end
proc start
 entry
 ret f(1)
end' 'proc f(x: u64)
 ret 1
end
proc start
 entry
 ret f("a")
end' 'proc f() -> view u8
 ret 1
end
proc start
 entry
 ret 1
end' 'proc f() -> u64
 ret "a"
end
proc start
 entry
 ret 1
end' 'proc f()
 zone z 4096
 ret 1
 end
end
proc start
 entry
 ret f()
end' 'proc start
 entry
 while 1
 zone z 4096
 break
 end
 end
end' 'proc start
 entry
 x := 1
 ret x.bytes(1)
end' 'proc f(a: view u8, b: view u8, c: view u8, d: view u8)
 ret 1
end
proc start
 entry
 ret 1
end' 'proc start
 entry
 x := 1
 ret x[0]
end' 'proc start
 entry
 ret "a" == "a"
end' 'proc start
 entry
 zone z
 ret 1
 end
end'; do
    set +e
    printf '%s\n' "$bad" | ./build/oli1.bin > build/rj.elf 2> build/rj.err
    st=$?
    set -e
    [ "$st" = 2 ] || fail "oli1: malformed program exited $st instead of 2"
    [ "$(wc -c < build/rj.elf)" -eq 0 ] || fail "oli1: malformed program produced output"
    grep -q 'oli1: error' build/rj.err || fail "oli1: malformed program gave no diagnostic"
done
echo "ok: oli1 rejects type mismatches, stores into strings, ret/break across zones, oversize signatures"
# step 6b: layouts, refs, field access, Name.at, z.make
for pair in "layout_basic 154" "layout_signed 39" "layout_param 44" "layout_packed 80"; do
    set -- $pair
    ./build/oli1.bin < "3-oli1/tests/$1.oli" > build/o3.elf
    chmod +x build/o3.elf
    set +e; timeout 10 ./build/o3.elf; st=$?; set -e
    [ "$st" = "$2" ] || fail "oli1: $1.oli expected exit $2 got $st"
done
./build/oli1.bin < 3-oli1/tests/layout_view.oli > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf > build/o3.got; st=$?; set -e
[ "$st" = 9 ] || fail "oli1: layout_view.oli exit $st"
printf 'hello\n' > build/o3.want
cmp build/o3.got build/o3.want || fail "oli1: layout_view.oli wrote wrong bytes"
echo "ok: oli1 compiles layouts (natural/packed/align), refs, sized field loads and stores, Name.at, z.make"
for trap in ' s := "abcd"
 h := H.at(s)
 ret 1' ' zone z 4096
 b := z.bytes(64)
 h := H.at(b[1..])
 ret 1
 end'; do
    printf 'layout H\n a : u32\n b : u64\nend\nproc start\n entry\n%s\nend\n' "$trap" | ./build/oli1.bin > build/t.elf
    chmod +x build/t.elf
    set +e; ./build/t.elf > build/t.out 2> build/t.err; st=$?; set -e
    [ "$st" = 3 ] || fail "oli1: Name.at violation exited $st instead of trapping"
    grep -q 'oli: trap' build/t.err || fail "oli1: Name.at trap gave no diagnostic"
done
echo "ok: Name.at traps on short and misaligned views"
for bad in 'layout H
 a : u32
end
proc start
 entry
 ret H.at(5)
end' 'layout H
 a : u32
end
proc start
 entry
 zone z 4096
 h := z.make(H)
 ret h.nofield
 end
end' 'layout H
 a : u32
end
proc start
 entry
 zone z 4096
 h := z.make(H)
 h.a <- "x"
 ret 1
 end
end' 'layout H
 a : u32
 t : view u8
end
proc start
 entry
 zone z 4096
 h := z.make(H)
 h.t <- 1
 ret 1
 end
end' 'layout H
 a : float
end
proc start
 entry
 ret 1
end' 'layout H
 a : u32
end
layout H
 b : u8
end
proc start
 entry
 ret 1
end' 'layout H
 a : u32
 a : u8
end
proc start
 entry
 ret 1
end' 'proc start
 entry
 zone z 4096
 h := z.make(Nope)
 ret 1
 end
end' 'layout A
 x : u8
end
layout B
 x : u8
end
proc f(a: ref A)
 ret 1
end
proc start
 entry
 zone z 4096
 b := z.make(B)
 ret f(b)
 end
end' 'layout A
 x : u8
end
proc f(a: ref A)
 ret 1
end
proc start
 entry
 ret f(1)
end' 'layout A
 x : u8
end
proc f() -> ref A
 ret 1
end
proc start
 entry
 ret 1
end' 'layout A
 x : u8
end
proc start
 entry
 ret A.nope
end' 'layout A
 x : u8
proc start
 entry
 ret 1
end' 'layout A
 x : u8
end
layout B
 x : u8
end
proc start
 entry
 zone z 4096
 a := z.make(A)
 a <- z.make(B)
 ret 1
 end
end' 'proc start
 entry
 layout A
 x : u8
 end
 ret 1
end'; do
    set +e
    printf '%s\n' "$bad" | ./build/oli1.bin > build/rj.elf 2> build/rj.err
    st=$?
    set -e
    [ "$st" = 2 ] || fail "oli1: malformed program exited $st instead of 2"
    [ "$(wc -c < build/rj.elf)" -eq 0 ] || fail "oli1: malformed program produced output"
    grep -q 'oli1: error' build/rj.err || fail "oli1: malformed program gave no diagnostic"
done
echo "ok: oli1 rejects unknown fields/types/layouts, duplicate layouts and fields, ref type mismatches"
# step 6c: fallible results (T or E), fail, else handlers, case
for pair in "fallible 157" "optional 140"; do
    set -- $pair
    ./build/oli1.bin < "3-oli1/tests/$1.oli" > build/o3.elf
    chmod +x build/o3.elf
    set +e; timeout 10 ./build/o3.elf; st=$?; set -e
    [ "$st" = "$2" ] || fail "oli1: $1.oli expected exit $2 got $st"
done
./build/oli1.bin < 3-oli1/tests/propagate.oli > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf > build/o3.got; st=$?; set -e
[ "$st" = 51 ] || fail "oli1: propagate.oli exit $st"
printf 'ok\nshort\nbad\n' > build/o3.want
cmp build/o3.got build/o3.want || fail "oli1: propagate.oli wrote wrong bytes"
printf 'proc f(x: word) -> word or word
 if x > 5
  fail x
 end
 ret x
end
proc start
 entry
 a := f(9) else ret 7
 ret 1
end
' | ./build/oli1.bin > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf; st=$?; set -e
[ "$st" = 7 ] || fail "oli1: else ret in the entry procedure exit $st"
printf 'proc f(x: word) -> word or none
 if x > 5
  fail
 end
 ret x
end
proc g(x: word) -> word or none
 y := f(x) else fail
 ret y + 1
end
proc h(x: word)
 y := g(x) else ret
 os.syscall(60, y)
end
proc start
 entry
 h(9)
 h(3)
 ret 1
end
' | ./build/oli1.bin > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf; st=$?; set -e
[ "$st" = 4 ] || fail "oli1: none propagation and bare else ret exit $st"
printf 'proc f(x: word) -> word or word
 if x > 5
  fail x
 end
 ret x
end
proc g() -> word
 ret 30
end
proc pick(x: word) -> word
 case f(x)
 when ok v
  ret v * 10
 when fail e
  ret e
 end
end
proc start
 entry
 n := 0
 if f(9) else 0
  n <- 1
 end
 if f(2) else 0
  n <- n + 2
 end
 m := f(7) else g()
 i := 0
 s := 0
 while i < 10
  case f(i)
  when fail e
   break
  when ok v
   s <- s + v
  end
  i <- i + 1
 end
 ret n + m + s + pick(3) + pick(8)
end
' | ./build/oli1.bin > build/o3.elf
chmod +x build/o3.elf
set +e; timeout 10 ./build/o3.elf; st=$?; set -e
[ "$st" = 85 ] || fail "oli1: fallible conditions, call defaults, case in a loop exit $st"
echo "ok: oli1 compiles T or E results, fail, else fail/ret/default handlers and case with ok/fail arms"
for bad in 'proc f() -> word or word
 ret 1
end
proc start
 entry
 x := f()
 ret 1
end' 'proc f() -> word or word
 ret 1
end
proc start
 entry
 f()
 ret 1
end' 'proc f() -> word
 ret 1
end
proc start
 entry
 x := f() else 0
 ret 1
end' 'proc f() -> word
 fail 1
end
proc start
 entry
 ret 1
end' 'proc f() -> word or none
 fail 1
end
proc start
 entry
 ret 1
end' 'proc f() -> word or word
 fail
end
proc start
 entry
 ret 1
end' 'proc f() -> word or word
 ret 1
end
proc g() -> word
 ret f() else fail
end
proc start
 entry
 ret 1
end' 'proc f() -> word or word
 ret 1
end
proc g() -> word or none
 ret f() else fail
end
proc start
 entry
 ret 1
end' 'proc f() -> word or word
 ret 1
end
proc start
 entry
 case f()
 when ok x
  ret x
 end
end' 'proc f() -> word or word
 ret 1
end
proc start
 entry
 case f()
 when ok x
  ret x
 when ok y
  ret y
 end
end' 'proc f() -> word or word
 ret 1
end
proc start
 entry
 x := f() else "s"
 ret 1
end' 'proc f() -> view u8 or word
 ret "s"
end
proc start
 entry
 ret 1
end' 'proc f(x: word or none) -> word
 ret 1
end
proc start
 entry
 ret 1
end' 'proc start -> word or word
 entry
 ret 1
end' 'proc f() -> word or none
 fail
end
proc start
 entry
 case f()
 when ok x
  ret 1
 when fail e
  ret 2
 end
end' 'proc f() -> word or word
 ret
end
proc start
 entry
 ret 1
end' 'proc f() -> word
 ret 1
end
proc start
 entry
 case f()
 when ok x
  ret 1
 when fail e
  ret 2
 end
end' 'proc f() -> word or word
 ret 1
end
proc start
 entry
 x := f() else fail
 ret 1
end' 'proc f() -> word or word
 ret 1
end
proc start
 entry
 if f() else 0
  ret 1
 when ok x
  ret 2
 end
end' 'proc f() -> word or word
 ret 1
end
proc start
 entry
 x := f() == 1 else 0
 ret 1
end' 'proc f() -> word or word
 zone z 4096
  fail 1
 end
end
proc start
 entry
 ret 1
end' 'proc f() -> word or word
 ret 1
end
proc g() -> word or word
 zone z 4096
  x := f() else fail
 end
 ret 1
end
proc start
 entry
 ret 1
end' 'proc f() -> word or word or none
 ret 1
end
proc start
 entry
 ret 1
end'; do
    set +e
    printf '%s\n' "$bad" | ./build/oli1.bin > build/rj.elf 2> build/rj.err
    st=$?
    set -e
    [ "$st" = 2 ] || fail "oli1: malformed program exited $st instead of 2"
    [ "$(wc -c < build/rj.elf)" -eq 0 ] || fail "oli1: malformed program produced output"
    grep -q 'oli1: error' build/rj.err || fail "oli1: malformed program gave no diagnostic"
done
echo "ok: oli1 rejects unresolved/discarded fallible values, fail outside fallible procedures, E mismatches, bad case arms"
# step 6e: typed places, rw field types and loop
./build/oli1.bin < 3-oli1/tests/places.oli > build/o3.elf
chmod +x build/o3.elf
set +e; timeout 10 ./build/o3.elf; st=$?; set -e
[ "$st" = 44 ] || fail "oli1: places.oli expected exit 44 got $st"
for bad in 'proc start
 entry
 z : zone
 ret 1
end' 'proc start
 entry
 v : view u8 <- 5
 ret 1
end' 'proc start
 entry
 n : u64 <- "s"
 ret 1
end' 'proc start
 entry
 loop
 ret 1
end'; do
    set +e
    printf '%s\n' "$bad" | ./build/oli1.bin > build/rj.elf 2> build/rj.err
    st=$?
    set -e
    [ "$st" = 2 ] || fail "oli1: malformed place/loop program exited $st instead of 2"
    [ "$(wc -c < build/rj.elf)" -eq 0 ] || fail "oli1: malformed program produced output"
done
echo "ok: oli1 compiles typed places, view places, rw field types and loop; rejects zone places and type mismatches"
# step 6d: module-level integer constants
./build/oli1.bin < 3-oli1/tests/consts.oli > build/o3.elf
chmod +x build/o3.elf
set +e; ./build/o3.elf; st=$?; set -e
[ "$st" = 42 ] || fail "oli1: consts.oli expected exit 42 got $st"
for bad in 'A := 1
A := 2
proc start
 entry
 ret 1
end' 'A := x
proc start
 entry
 ret 1
end' 'A := 1
proc start
 entry
 A <- 2
 ret 1
end' 'A := 1
proc start
 entry
 ret A.len
end'; do
    set +e
    printf '%s\n' "$bad" | ./build/oli1.bin > build/rj.elf 2> build/rj.err
    st=$?
    set -e
    [ "$st" = 2 ] || fail "oli1: malformed program exited $st instead of 2"
    [ "$(wc -c < build/rj.elf)" -eq 0 ] || fail "oli1: malformed program produced output"
    grep -q 'oli1: error' build/rj.err || fail "oli1: malformed program gave no diagnostic"
done
echo "ok: oli1 compiles module-level constants; rejects duplicates, non-literal values, stores and members"
# step 6f: explicit conversions `T(x)`, `T.wrap(x)` and `T.bits(x)`
./build/oli1.bin < 3-oli1/tests/convert.oli > build/o4.elf
chmod +x build/o4.elf
set +e; ./build/o4.elf; st=$?; set -e
[ "$st" = 42 ] || fail "oli1: convert.oli expected exit 42 got $st"
# The widths are the encoder's, not the interpreter's: check the emitted bytes.
printf 'module w\nproc start\n entry\n n : u64 <- 1\n a : u64 <- u8.wrap(n)\n b : u64 <- s8.wrap(n)\n c : u64 <- u16.wrap(n)\n d : u64 <- s16.wrap(n)\n e : u64 <- u32.wrap(n)\n f : u64 <- s32.wrap(n)\n g : u64 <- u64.wrap(n)\n h : u64 <- u64(n)\n ret 0\nend\n' > build/cvw.oli
./build/oli1.bin < build/cvw.oli > build/cvw.elf
for want in 0fb6c0 480fbec0 0fb7c0 480fbfc0 89c0 4863c0; do
    od -v -A n -t x1 build/cvw.elf | tr -d ' \n' | grep -q "$want" || fail "oli1: conversion bytes $want missing"
done
[ "$(od -v -A n -t x1 build/cvw.elf | tr -d ' \n' | grep -c '0fb6c0')" = 1 ] || fail "oli1: u8.wrap emitted more than once"
for bad in 'proc start
 entry
 ret u8.sat(3)
end' 'proc start
 entry
 ret u8.checked(3)
end' 'proc start
 entry
 ret u8.nope(3)
end' 'proc start
 entry
 ret u8 3
end' 'proc start
 entry
 s := "hi"
 ret u8.wrap(s)
end' 'proc start
 entry
 ret u8.wrap(3
end'; do
    set +e
    printf '%s\n' "$bad" | ./build/oli1.bin > build/rj.elf 2> build/rj.err
    st=$?
    set -e
    [ "$st" = 2 ] || fail "oli1: malformed conversion exited $st instead of 2"
    [ "$(wc -c < build/rj.elf)" -eq 0 ] || fail "oli1: malformed conversion produced output"
    grep -q 'oli1: error' build/rj.err || fail "oli1: malformed conversion gave no diagnostic"
done
echo "ok: oli1 compiles T(x)/T.wrap(x)/T.bits(x) with the exact truncation bytes; rejects sat, checked, unknown modes, bare type names and view operands"
echo "genesis: layer 3 (oli-core compiler, steps 0-6f) passed"

# --- layer 4: olic (G4), written in oli-core and compiled by oli1 ---
# The compiler in dependency order; a driver (`show_*.oli`, `olic.oli`) is
# appended to it to make a program.
OLIC_MODULES="../compiler/io.oli ../compiler/lex.oli ../compiler/diag.oli ../compiler/ast.oli
../compiler/parse.oli ../compiler/load.oli ../compiler/items.oli ../compiler/sema.oli
../compiler/body.oli ../compiler/check.oli ../compiler/oir.oli ../compiler/cfg.oli
../compiler/ssa.oli ../compiler/opt.oli ../compiler/x64.oli ../compiler/elf.oli
../compiler/asm.oli"
OLIC_FRONT="../compiler/io.oli ../compiler/lex.oli ../compiler/diag.oli ../compiler/ast.oli
../compiler/parse.oli ../compiler/load.oli ../compiler/items.oli ../compiler/sema.oli
../compiler/body.oli ../compiler/check.oli"
# The whole back end: the OIR drivers carry the machine too, because a
# `machine x64` block is assembled once while the OIR is built (a dry run
# that reports what the encoder does not know).
OLIC_OIR="../compiler/oir.oli ../compiler/cfg.oli ../compiler/ssa.oli ../compiler/opt.oli
../compiler/x64.oli ../compiler/elf.oli ../compiler/asm.oli"
cat ../compiler/io.oli ../compiler/lex.oli ../compiler/diag.oli ../compiler/show_tokens.oli > build/show_tokens.oli
./build/oli1.bin < build/show_tokens.oli > build/show_tokens || fail "oli1 could not compile compiler/ (show_tokens)"
chmod +x build/show_tokens
./build/oli1.bin < build/show_tokens.oli > build/show_tokens.again
cmp build/show_tokens build/show_tokens.again || fail "show_tokens build is not deterministic"
./build/show_tokens < ../examples/hello.oli > build/hello.tokens 2> build/tok.err || fail "lexer: hello.oli reported diagnostics"
cmp build/hello.tokens ../tests/snapshots/hello.tokens || fail "lexer: hello.oli token stream differs from tests/snapshots/hello.tokens"
echo "ok: olic lexer (oli-core, built by oli1) tokenizes examples/hello.oli byte for byte as the snapshot"
for f in ../tests/parse/ok/*.oli ../tests/sema/ok/*.oli ../tests/sema/err/*.oli ../examples/*.oli ../lib/*.oli ../lib/*/*.oli ../compiler/*.oli 3-oli1/tests/*.oli; do
    ./build/show_tokens < "$f" > build/tok.out 2> build/tok.err || fail "lexer: diagnostics for $f: $(cat build/tok.err)"
    tail -n 1 build/tok.out | grep -q ' eof$' || fail "lexer: $f does not end with eof"
done
echo "ok: olic lexer accepts every fixture, example, library module and its own source without diagnostics"
for f in ../tests/parse/err/*.oli; do
    want=$(grep -- '-- expect: E000' "$f" | sed 's/-- expect: //' | sort)
    set +e; ./build/show_tokens < "$f" > build/tok.out 2> build/tok.err; st=$?; set -e
    got=$(awk '/^error\[/ {code=substr($1,7,5)} /^ --> stdin:/ {split($2,a,":"); print code " @ " a[2] ":" a[3]}' build/tok.err | sort)
    [ "$got" = "$want" ] || fail "lexer: $f expected [$want] got [$got]"
    if [ -n "$want" ]; then [ "$st" = 1 ] || fail "lexer: $f exit $st"; else [ "$st" = 0 ] || fail "lexer: $f exit $st"; fi
    tail -n 1 build/tok.out | grep -q ' eof$' || fail "lexer: $f does not end with eof"
done
echo "ok: olic lexer reports exactly the E0001-E0006 diagnostics the parse/err fixtures expect, at their positions"
printf 'a := 0xFFFF800000000000\nb := 18446744073709551615\nc := 18446744073709551616\nd := 64K + 2M + 1G\ne := 0b1010_1010 + 0o17\nf := %s\ng := %s\n' "'a'" "'\\x41'" > build/tok.in
set +e; ./build/show_tokens < build/tok.in > build/tok.out 2> build/tok.err; st=$?; set -e
[ "$st" = 1 ] || fail "lexer: literal boundaries exit $st"
got=$(grep -c 'E0003' build/tok.err)
[ "$got" = 1 ] || fail "lexer: expected one E0003, got $got"
got=$(grep ' int \| char ' build/tok.out | awk '{print $3}' | tr '\n' ' ')
[ "$got" = "18446603336221196288 18446744073709551615 0 65536 2097152 1073741824 170 15 97 65 " ] || fail "lexer: literal values [$got]"
echo "ok: olic lexer scales K/M/G, reads hex/binary/octal, u64 boundaries, char and escape values"

cat ../compiler/io.oli ../compiler/lex.oli ../compiler/diag.oli ../compiler/ast.oli ../compiler/parse.oli ../compiler/show_ast.oli > build/show_ast.oli
./build/oli1.bin < build/show_ast.oli > build/show_ast || fail "oli1 could not compile compiler/ (show_ast)"
chmod +x build/show_ast
./build/oli1.bin < build/show_ast.oli > build/show_ast.again
cmp build/show_ast build/show_ast.again || fail "show_ast build is not deterministic"
for pair in "hello ../examples/hello.oli" "packet_demo ../examples/packet_demo.oli" "statements ../tests/parse/ok/statements.oli" "kernel_sketch ../tests/parse/ok/kernel_sketch.oli" "when ../tests/run/when.oli"; do
    set -- $pair
    ./build/show_ast < "$2" > build/$1.ast 2> build/ast.err || fail "parser: diagnostics for $2: $(cat build/ast.err)"
    cmp build/$1.ast ../tests/snapshots/$1.ast || fail "parser: $1 differs from tests/snapshots/$1.ast"
done
echo "ok: olic parser reproduces every tests/snapshots/*.ast byte for byte (--show-ast)"
for f in ../tests/sema/ok/*.oli ../tests/sema/err/*.oli ../lib/*.oli ../lib/*/*.oli ../compiler/*.oli 3-oli1/tests/*.oli; do
    ./build/show_ast < "$f" > build/ast.out 2> build/ast.err || fail "parser: diagnostics for $f: $(cat build/ast.err)"
    head -c 8 build/ast.out | grep -q '^(module' || fail "parser: $f produced no module"
done
echo "ok: olic parser accepts every fixture, library module and its own source without diagnostics"
for f in ../tests/parse/err/*.oli; do
    want=$(grep -- '-- expect: ' "$f" | sed 's/-- expect: //' | sort)
    set +e; ./build/show_ast < "$f" > build/ast.out 2> build/ast.err; st=$?; set -e
    got=$(awk '/^error\[|^warning\[/ {code=substr($1,index($1,"[")+1,5)} /^ --> stdin:/ {split($2,a,":"); print code " @ " a[2] ":" a[3]}' build/ast.err | sort)
    [ "$got" = "$want" ] || fail "parser: $f expected [$want] got [$got]"
    [ "$st" = 1 ] || fail "parser: $f exit $st"
    head -c 8 build/ast.out | grep -q '^(module' || fail "parser: $f produced no module after recovery"
done
echo "ok: olic parser reports exactly the E0001-E0032/W0001 diagnostics the parse/err fixtures expect, and recovers"
cat ../compiler/io.oli ../compiler/lex.oli ../compiler/diag.oli ../compiler/ast.oli ../compiler/parse.oli ../compiler/load.oli ../compiler/items.oli ../compiler/sema.oli ../compiler/body.oli ../compiler/check.oli ../compiler/show_sema.oli > build/show_sema.oli
./build/oli1.bin < build/show_sema.oli > build/show_sema || fail "oli1 could not compile compiler/ (show_sema)"
chmod +x build/show_sema
./build/oli1.bin < build/show_sema.oli > build/show_sema.again
cmp build/show_sema build/show_sema.again || fail "show_sema build is not deterministic"
# The whole semantic graph must equal the snapshot, byte for byte. It is read
# from the repository root because imports are resolved under lib/.
for pair in "hello examples/hello.oli" "packet_demo examples/packet_demo.oli" "freestanding tests/sema/ok/freestanding.oli"; do
    set -- $pair
    ( cd .. && genesis/build/show_sema < "$2" > genesis/build/$1.out 2> genesis/build/items.err ) || fail "sema: diagnostics for $2: $(cat build/items.err)"
    cmp build/$1.out ../tests/snapshots/$1.sema || fail "sema: $1 differs from tests/snapshots/$1.sema"
done
echo "ok: olic reproduces every tests/snapshots/*.sema byte for byte - items, signatures, locals and typed bodies (--show-sema)"
( cd .. && genesis/build/show_sema < tests/parse/ok/kernel_sketch.oli > genesis/build/ks.items 2>/dev/null ) || true
grep -q '(layout GdtPointer#1 size=10 align=1 (limit u16 @0) (base addr u64 @2))' build/ks.items || fail "items: packed layout"
grep -q '(layout Multiboot2Header#0 size=24 align=8 ' build/ks.items || fail "items: explicit layout alignment"
( cd .. && genesis/build/show_sema < tests/parse/ok/statements.oli > genesis/build/st.items 2>/dev/null ) || true
grep -q '(choice Shape#0 size=20 align=4 tag=1 payload@4 (dot) (line (a Point @4) (b Point @12)))' build/st.items || fail "items: choice with a layout payload"
for f in ../tests/sema/ok/*.oli ../lib/*.oli ../lib/*/*.oli; do
    ( cd .. && genesis/build/show_sema < "${f#../}" > genesis/build/items.out 2> genesis/build/items.err ) || fail "items: diagnostics for $f: $(cat build/items.err)"
    head -c 9 build/items.out | grep -q '^(program' || fail "items: $f produced no program"
done
# The compiler analyses its own source, as one program and file by file, and
# must report nothing: `compiler/` is valid V0, not merely valid oli-core.
: > build/self.oli
for f in $OLIC_MODULES ../compiler/olic.oli; do
    if [ -s build/self.oli ]; then grep -v '^module ' "$f" >> build/self.oli; else cat "$f" >> build/self.oli; fi
done
( cd .. && genesis/build/show_sema < genesis/build/self.oli > genesis/build/self.out 2> genesis/build/self.err ) \
    || fail "self-analysis: olic reports $(grep -c 'error\[' build/self.err) diagnostics on its own source: $(head -2 build/self.err)"
grep -q '(proc olic.io.check_proc' build/self.out || fail "self-analysis: the whole compiler was not analysed"
grep -q '(proc olic.io.lower_all' build/self.out || fail "self-analysis: the back end was not analysed"
for f in ../compiler/*.oli; do
    ( cd .. && genesis/build/show_sema < "${f#../}" > genesis/build/items.out 2> genesis/build/items.err ) || fail "items: $f reports $(head -1 build/items.err)"
    head -c 9 build/items.out | grep -q '^(program' || fail "items: $f produced no program"
done
echo "ok: olic analyses its own source - front end and back end, all seventeen modules as one program - without a single diagnostic"

echo "ok: olic resolves packed/aligned layouts, nested payloads and every library module it imports"
# Procedure signatures and locals again on their own, so a failure says which
# stage broke when the whole-file comparison above fails.
for pair in "hello examples/hello.oli" "packet_demo examples/packet_demo.oli" "freestanding tests/sema/ok/freestanding.oli"; do
    set -- $pair
    ( cd .. && genesis/build/show_sema < "$2" 2>/dev/null ) | grep -E '^  \(proc |^    \(local ' > build/$1.sig
    grep -E '^  \(proc |^    \(local ' ../tests/snapshots/$1.sema > build/$1.sigwant
    cmp build/$1.sigwant build/$1.sig || fail "signatures: $1 differs from tests/snapshots/$1.sema"
    [ -s build/$1.sig ] || fail "signatures: $1 produced nothing"
done
# Semantic checks that need no type graph: capabilities, unimplemented
# features, constant cycles and ranges, recursive layouts.
for f in ../tests/sema/err/*.oli; do
    want=$(grep -- '-- expect: ' "$f" | sed 's/-- expect: //' | sort)
    set +e; ( cd .. && genesis/build/show_sema < "${f#../}" > genesis/build/chk.out 2> genesis/build/chk.err ); st=$?; set -e
    got=$(awk '/^error\[|^warning\[/ {code=substr($1,index($1,"[")+1,5)} /^ --> stdin:/ {split($2,a,":"); print code " @ " a[2] ":" a[3]}' build/chk.err | sort)
    [ "$got" = "$want" ] || fail "checks: $f expected [$want] got [$got]"
    [ "$st" = 1 ] || fail "checks: $f exit $st"
done
( cd .. && genesis/build/show_sema < tests/parse/ok/kernel_sketch.oli > /dev/null 2> genesis/build/chk.err ) || true
grep -q 'E0401' build/chk.err || fail "checks: kernel_sketch must report the missing memory.raw permit"
grep -q 'E0200' build/chk.err || fail "checks: kernel_sketch must report its type mismatch"
! grep -q 'E0900' build/chk.err || fail "checks: kernel_sketch uses nothing that is E0900 any more"
for f in ../tests/sema/ok/*.oli ../examples/*.oli ../lib/*.oli ../lib/*/*.oli; do
    ( cd .. && genesis/build/show_sema < "${f#../}" > /dev/null 2> genesis/build/chk.err ) || fail "checks: $f reported $(head -1 build/chk.err)"
done
echo "ok: olic reports exactly the diagnostics of all twenty tests/sema/err fixtures - capabilities, E0900, constants, layouts, scopes, definite assignment, reachability, failures, exhaustiveness, read-only places, region escapes, literal types, literal and pattern fields, address spaces, implicit narrowing, linear own values, members of scalars, float rules and vector rules - and none on any positive fixture"
echo "ok: olic prints every procedure signature and every local - parameters, places, bindings, zones and case patterns with inferred types - exactly as tests/snapshots/*.sema"
echo "genesis: layer 4 (olic front end and semantic analysis) passed"

# --- layer 5: olic back end - OIR, x86-64 lowering and the ELF writer ---
for d in show_oir show_ssa show_opt verify_check; do
    cat $OLIC_FRONT $OLIC_OIR ../compiler/$d.oli > build/$d.oli
    ./build/oli1.bin < build/$d.oli > build/$d || fail "oli1 could not compile compiler/ ($d)"
    chmod +x build/$d
    ./build/oli1.bin < build/$d.oli > build/$d.again
    cmp build/$d build/$d.again || fail "$d build is not deterministic"
done
# --explain and --show-asm read what the lowering laid out.
for d in explain show_asm; do
    cat $OLIC_MODULES ../compiler/$d.oli > build/$d.oli
    ./build/oli1.bin < build/$d.oli > build/$d || fail "oli1 could not compile compiler/ ($d)"
    chmod +x build/$d
done
for pair in "hello examples/hello.oli" "control tests/run/control.oli" "values tests/run/values.oli" "memory tests/run/memory.oli" "layouts tests/run/layouts.oli" "fallible tests/run/fallible.oli" "statics tests/run/statics.oli" "frames tests/run/frames.oli" "saturate tests/run/saturate.oli" "zones tests/run/zones.oli" "cse tests/run/cse.oli" "choice tests/run/choice.oli" "machine tests/run/machine.oli" "freestanding tests/run/freestanding.oli" "aggregates tests/run/aggregates.oli" "records tests/run/records.oli" "interrupt tests/run/interrupt.oli" "bytes tests/run/bytes.oli" "wide tests/run/wide.oli" "mmio tests/run/mmio.oli" "hw tests/run/hw.oli" "paging tests/run/paging.oli" "atomic tests/run/atomic.oli" "memops tests/run/memops.oli" "segments tests/run/segments.oli" "own tests/run/own.oli" "rodata tests/run/rodata.oli" "floats tests/run/floats.oli" "simd tests/run/simd.oli"; do
    set -- $pair
    ( cd .. && genesis/build/show_oir < "$2" > genesis/build/$1.oir 2> genesis/build/oir.err ) || fail "oir: diagnostics for $2: $(cat build/oir.err)"
    cmp build/$1.oir ../tests/snapshots/$1.oir || fail "oir: $1 differs from tests/snapshots/$1.oir"
    ( cd .. && genesis/build/show_ssa < "$2" > genesis/build/$1.ssa 2> genesis/build/ssa.err ) || fail "ssa: diagnostics for $2: $(cat build/ssa.err)"
    cmp build/$1.ssa ../tests/snapshots/$1.ssa || fail "ssa: $1 differs from tests/snapshots/$1.ssa"
    ( cd .. && genesis/build/show_opt < "$2" > genesis/build/$1.opt 2> genesis/build/opt.err ) || fail "opt: diagnostics for $2: $(cat build/opt.err)"
    cmp build/$1.opt ../tests/snapshots/$1.opt || fail "opt: $1 differs from tests/snapshots/$1.opt"
done
echo "ok: olic cuts every block of examples/hello.oli and tests/run/{control,values,memory,layouts,fallible,statics,frames,saturate,zones,cse,choice,machine,freestanding,aggregates,records,interrupt,bytes,wide,mmio,hw,paging,atomic,memops,segments,own,rodata,floats,simd}.oli exactly as tests/snapshots/*.oir (--show-oir), and the verifier accepts each"

# What mem2reg must have done: no place is left, every join that needs one has
# a phi, and every block of the printed form is one a path can reach.
for n in hello control values; do
    if grep -q 'frame\.' build/$n.ssa; then
        fail "ssa: $n still loads or stores a frame place after mem2reg"
    fi
done
# A zone's triple has its address taken, so it is the one place that stays.
[ "$(grep -c 'addr\.of frame\.' build/memory.ssa || true)" -ge 1 ] || fail "ssa: the zone of memory.oli lost its place"
if grep -q 'load frame\.\|store frame\.' build/memory.ssa; then
    fail "ssa: memory.ssa still loads or stores a promotable place"
fi
grep -q 'phi \[bb0 %2\] \[bb3 %10\]' build/memory.ssa || fail "ssa: the each loop of memory.oli did not get the phi OIR_SPEC 5 shows"
[ "$(grep -c 'phi \[' build/control.ssa)" -ge 3 ] || fail "ssa: control.ssa has fewer than three phis"
grep -q 'phi \[bb0 %1\] \[bb2 %6\]' build/control.ssa || fail "ssa: the loop of sum_to did not get the phi OIR_SPEC 5 shows"
[ "$(grep -c 'phi \[' build/hello.ssa)" = 0 ] || fail "ssa: hello.ssa has a phi and has no join"
echo "ok: mem2reg promotes every place to a value, puts a phi exactly where two definitions meet, and builds no unreachable block (--show-ssa)"

# The passes of OIR_SPEC 6. A check leaves only with a proof, which is the
# rule the verifier enforces and the printed form shows where it stood.
for n in hello control values memory layouts fallible statics frames saturate zones cse choice machine freestanding aggregates records interrupt bytes wide mmio hw paging atomic memops segments own rodata floats simd; do
    a=$(grep -c '; check\.' build/$n.opt || true)
    b=$(grep -c 'removed: proof(' build/$n.opt || true)
    [ "$a" = "$b" ] || fail "opt: $n prints $a removed checks and $b proofs"
done
[ "$(grep -c '^      check\.' build/values.opt || true)" = 0 ] \
    || fail "opt: values.opt still carries a check, and every operand in it is a constant"
[ "$(grep -c 'removed: proof(constant)' build/values.opt || true)" -ge 10 ] \
    || fail "opt: values.opt removed fewer than ten checks by constant folding"
grep -q 'ret %86' build/values.opt || fail "opt: the self-test of values.oli did not fold to its answer"
grep -q '; check\.div_zero %5 -- removed: proof(divisor)' build/control.opt \
    || fail "opt: the constant divisor of control.oli did not remove the zero check"
[ "$(grep -c '^      check\.overflow' build/control.opt || true)" = 4 ] \
    || fail "opt: control.opt should keep the four checks whose operands are not constants"
grep -q '; check\.bounds %[0-9]* -- removed: proof(constant)' build/memory.opt \
    || fail "opt: a bounds check on two constants was not proved away"
[ "$(grep -c 'removed: proof(loop-bound)' build/memory.opt || true)" -ge 2 ] \
    || fail "opt: the checks of each and of while-below-len were not proved by the loop bound"
grep -q '^      check\.range' build/memory.opt \
    || fail "opt: memory.opt should keep the range check whose bound is a parameter"
grep -q 'check\.bounds' build/memory.oir || fail "oir: each must emit its bounds check, so that removing it is a proof and not an omission"
# Data the passes left nothing naming is not in the image.
[ "$(grep -c '(static [0-9]* removed)' build/values.opt || true)" = 9 ] \
    || fail "opt: the nine trap messages of values.oli should all be removed with their checks"
# Layouts: a `Name.at` carries its size and alignment checks, `z.make` asks
# for the layout's alignment, and a field is read at its own width.
grep -q 'check\.align' build/layouts.oir || fail "oir: Name.at over a non-packed layout must check the alignment"
grep -q 'zone\.alloc .* align=32' build/layouts.oir || fail "oir: z.make of a layout aligned 32 must ask for it"
grep -q 'raw\.load\.16\.s' build/layouts.oir || fail "oir: an s16 field must be loaded sign-extended at 16 bits"
grep -q 'raw\.store\.32' build/layouts.oir || fail "oir: a u32 field must be stored at 32 bits"
# Fallible results: a call answers with (tag, payload), `ret`/`fail` return a
# pair, and a default joins the ok path through a phi.
grep -q 'call\.payload' build/fallible.oir || fail "oir: a fallible call must read its payload out"
# The arithmetic modes beside wrap: a 64-bit sat or checked reads the flag
# the operation set, a narrow one compares with the type's range.
( cd .. && genesis/build/show_oir < tests/run/saturate.oli > genesis/build/saturate.oir 2>/dev/null ) || fail "oir: saturate.oli"
grep -q 'ovf\.of' build/saturate.oir || fail "oir: a 64-bit sat/checked operation must read the overflow flag"
# The fixture writes four subtractions outside any mode (`0 - 100`,
# `0 - 127 - 1`, `0 - 300`): those trap, nothing inside sat() or checked() does.
[ "$(grep -c 'check\.overflow' build/saturate.oir || true)" = 4 ] || fail "oir: exactly the four trapping subtractions outside sat()/checked() carry a check"
# The other sources of a zone (OIR_SPEC 4) and the release on every exit
# edge: a zone at an address or over a proved buffer is zone.new.raw, one
# carved from a parent is zone.new.from and gives the cursor back with
# zone.end.from, and a ret, fail, break or continue inside a zone releases
# it on its way out, so a zone.new has as many zone.end as the block has exits.
( cd .. && genesis/build/show_oir < tests/run/zones.oli > genesis/build/zones.oir 2>/dev/null ) || fail "oir: zones.oli"
grep -q 'zone\.new\.raw' build/zones.oir || fail "oir: a zone at an address or over a buffer must be zone.new.raw"
grep -q 'check\.range .*' build/zones.oir || fail "oir: a zone over a buffer must prove the buffer holds it"
grep -q 'zone\.new\.from' build/zones.oir || fail "oir: a zone carved from a parent must be zone.new.from"
grep -q 'zone\.end\.from' build/zones.oir || fail "oir: a zone carved from a parent must give the parent its cursor back"
[ "$(sed -n '/(proc first_word/,/(proc pick/p' build/zones.oir | grep -c 'zone\.end')" = 2 ] \
    || fail "oir: first_word has a ret inside its zone and an end: two releases for one zone.new"
[ "$(sed -n '/(proc count_loops/,/(proc start/p' build/zones.oir | grep -c 'zone\.end')" = 3 ] \
    || fail "oir: count_loops leaves its zone by continue, break and end: three releases"
grep -q 'raw\.load\.64' build/zones.oir || fail "oir: try_bytes must read the cursor and the limit in the stream"
# Common-subexpression elimination (OIR_SPEC 6): an operation equal to one
# that dominates it goes, and a check it carried goes with the proof
# `dominance` — cse.oli has exactly four such checks, one per duplicated
# operation, and keeps the six that stand first.
[ "$(grep -c 'removed: proof(dominance)' build/cse.opt || true)" = 4 ] \
    || fail "opt: cse.oli must remove exactly four checks by dominance"
[ "$(grep -c '^      check\.' build/cse.opt || true)" = 6 ] \
    || fail "opt: cse.oli must keep the 6 checks that stand first on every path"
# A choice that fits a word is its image: the tag in the first byte, a field
# masked and shifted to its offset, and read back at its width and sign.
grep -q 'trunc\.8\.s' build/choice.oir || fail "oir: an s8 field of a variant must be read back sign-extended"
grep -q ' = shl ' build/choice.oir || fail "oir: a field of a variant must be shifted to its offset"
# A machine block stands in the stream as its ins, the block and its outs.
grep -q 'machine\.in rax, %' build/machine.oir || fail "oir: an in of a machine block must be a machine.in"
grep -q '= machine\.out rax' build/machine.oir || fail "oir: an out of a machine block must be a machine.out"
grep -q '^      machine x64$' build/machine.oir || fail "oir: the block itself must stand in the stream"
# The encoder, byte for byte: every block of tests/machine/NAME.oli must come
# out as NAME.hex, the canonical bytes the genesis assembler's fixtures were
# derived by hand from (genesis/2-asm/tests).
for f in ../tests/machine/*.oli; do
    n=$(basename "$f" .oli)
    ( cd .. && genesis/build/show_asm < "tests/machine/$n.oli" > genesis/build/m_$n.asm 2> genesis/build/m_$n.err ) || fail "asm: $n.oli: $(head -1 build/m_$n.err)"
    got=$(grep -E '^    [0-9a-f]{2}' build/m_$n.asm | tr -d ' \r\n)')
    want=$(tr -d ' \r\n' < "../tests/machine/$n.hex")
    [ -n "$got" ] || fail "asm: $n produced no bytes"
    [ "$got" = "$want" ] || fail "asm: the bytes of $n differ from tests/machine/$n.hex"
done
echo "ok: olic assembles every machine x64 block of tests/machine/ to the canonical bytes of the genesis assembler - registers, memory operands, conditional branches and r32 forms (--show-asm)"
# A layout by value travels as its eightbytes: sixteen bytes or less in
# registers (`arg` per word, `call.word1` for the second word back), more
# on the stack; a layout literal is a fresh area of the frame.
grep -q 'call\.word1' build/records.oir || fail "oir: a layout result of sixteen bytes must come back in rax and rdx"
grep -q 'addr\.of frame' build/records.oir || fail "oir: a layout literal must be an area of the frame"
# --explain reads the same form plus the frame the lowering laid out: the
# per-procedure report and the cost of every line are pinned as snapshots.
for n in cse zones; do
    ( cd .. && genesis/build/explain < tests/run/$n.oli > genesis/build/$n.explain 2> genesis/build/explain.err ) || fail "explain: $n.oli: $(head -1 build/explain.err)"
    cmp build/$n.explain ../tests/snapshots/$n.explain || fail "explain: $n differs from tests/snapshots/$n.explain"
done
grep -q 'dominance=1' build/cse.explain || fail "explain: cse.oli must report the check removed by dominance"
grep -q '(registers values=[1-9][0-9]* saved=rbx' build/cse.explain || fail "explain: the linear scan must give values of cse.oli registers, rbx first"
grep -q 'SYSCALL=' build/zones.explain || fail "explain: a zone from the operating system must be a SYSCALL on its line"
grep -q '(line [0-9]* .*ZONE=' build/zones.explain || fail "explain: an allocation from a zone must be ZONE on its line"
# An access through an `mmio` view is in space mmio (OIR_SPEC 1.4): volatile,
# so the passes keep every one - the unused load of tests/run/mmio.oli too -
# and --explain bills each as KERNEL.
[ "$(grep -c 'raw.load.[0-9]*.mmio' build/mmio.opt)" = 7 ] || fail "opt: mmio.oli must keep its seven volatile loads, the unused ones too"
[ "$(grep -c 'raw.store.[0-9]*.mmio' build/mmio.opt)" = 5 ] || fail "opt: mmio.oli must keep its five volatile stores"
( cd .. && genesis/build/explain < tests/run/mmio.oli > genesis/build/mmio.explain 2> genesis/build/explain.err ) || fail "explain: mmio.oli: $(head -1 build/explain.err)"
[ "$(grep -o 'KERNEL=1' build/mmio.explain | wc -l | tr -d ' ')" = 12 ] || fail "explain: mmio.oli must bill its twelve volatile accesses as KERNEL"
echo "ok: --explain reports every procedure's frame, checks with their proofs, zones, calls, syscalls, the values the linear scan put in callee-saved registers, and the cost class of every line that produced an instruction"
# Statics: an initialised one is bytes in the file, a zero one is memory past
# it, and the image has the read+write segment ABI.md 7 asks for.
grep -q '(data 0 size=8 init)' build/statics.oir || fail "oir: the initialised static of statics.oli must be data"
grep -q '(data 1 size=4 zero)' build/statics.oir || fail "oir: the zero static of statics.oli must be bss"
grep -q 'addr\.of data\.' build/statics.oir || fail "oir: a static must be reached through its address"
# An array in a frame keeps its words: its address is taken, so mem2reg
# leaves the place, and it starts zero.
[ "$(grep -c 'addr\.of frame\.' build/frames.ssa || true)" -ge 4 ] || fail "ssa: the array places of frames.oli must keep their frame words"
grep -q 'raw\.store\.64' build/frames.oir || fail "oir: an array place must be zeroed on declaration"
[ "$(grep -c 'phi \[' build/fallible.ssa || true)" -ge 2 ] || fail "ssa: an else-default must become a phi"
( cd .. && genesis/build/show_opt < tests/run/trap/narrow.oli > genesis/build/narrow.opt )
grep -q '= trap overflow' build/narrow.opt \
    || fail "opt: a constant sum that its type cannot hold did not fold to a trap"
echo "ok: the passes fold constants, turn a branch on one into a jump, drop the blocks and the values that leaves, and remove a check only with a proof recorded (--show-oir=opt)"

set +e
( cd .. && genesis/build/verify_check < tests/run/control.oli 2> genesis/build/vc.err )
st=$?
set -e
[ "$st" = 0 ] || fail "verify_check: exit $st - the verifier missed a corruption ($(head -1 build/vc.err))"
[ "$(grep -c 'OIR invariant' build/vc.err)" = 10 ] || fail "verify_check: $(grep -c 'OIR invariant' build/vc.err) invariants reported, want 10"
echo "ok: the verifier accepts the OIR olic builds and rejects all ten hand-made corruptions of it - terminators, targets, dominance, phi shape and a check removed without a proof"

cat $OLIC_MODULES ../compiler/olic.oli > build/olic.oli
./build/oli1.bin < build/olic.oli > build/olic || fail "oli1 could not compile compiler/ (olic)"
chmod +x build/olic
./build/oli1.bin < build/olic.oli > build/olic.again
cmp build/olic build/olic.again || fail "olic build is not deterministic"
echo "ok: oli1 builds olic - the whole pipeline, front end and back end - deterministically"

# M1: the hello program, compiled by olic, running with no libc and no linker.
( cd .. && genesis/build/olic < examples/hello.oli > genesis/build/hello.elf 2> genesis/build/hello.err ) \
    || fail "olic: examples/hello.oli reported $(head -1 build/hello.err)"
chmod +x build/hello.elf
( cd .. && genesis/build/olic < examples/hello.oli > genesis/build/hello.again )
cmp build/hello.elf build/hello.again || fail "olic: hello.elf is not byte-identical on a second run"
head -c 4 build/hello.elf | od -A n -t x1 | tr -d ' \n' | grep -q '^7f454c46$' || fail "olic: hello.elf is not an ELF file"
got=$(./build/hello.elf) || fail "olic-built hello exited non-zero"
[ "$got" = "Hello Oli--" ] || fail "olic-built hello printed [$got]"
echo "ok: M1 - olic compiles examples/hello.oli to a static ELF64 that prints 'Hello Oli--' (no libc, no linker)"

# Behavioral fixtures: a fixture with a .out file writes it and exits 0, any
# other fixture exits 42. The exit status is the assertion, so a wrong value
# names the check that failed. A fixture reads NAME.in on stdin when there is
# one and /dev/null otherwise, so no fixture can ever wait on a terminal.
for f in ../tests/run/*.oli; do
    n=$(basename "$f" .oli)
    ( cd .. && genesis/build/olic < "tests/run/$n.oli" > genesis/build/$n.elf 2> genesis/build/$n.err ) \
        || fail "run: olic could not compile $f: $(head -1 build/$n.err)"
    chmod +x build/$n.elf
    stdin=/dev/null
    [ -f "../tests/run/$n.in" ] && stdin="../tests/run/$n.in"
    if [ -f "../tests/run/$n.out" ]; then
        set +e; timeout 20 ./build/$n.elf < "$stdin" > build/$n.got; st=$?; set -e
        [ "$st" = 0 ] || fail "run: $n exited $st"
        cmp build/$n.got "../tests/run/$n.out" || fail "run: $n wrote output differing from tests/run/$n.out"
    else
        set +e; timeout 20 ./build/$n.elf < "$stdin"; st=$?; set -e
        [ "$st" = 42 ] || fail "run: $n exited $st, want 42 (the check number that failed)"
    fi
done
echo "ok: olic compiles and runs every tests/run fixture - arithmetic in all four modes, control flow, procedures with register and stack arguments, constants, conversions, zones from every source with try_bytes and a release on every exit edge, views, each, layouts, refs, fallible results with else and case, choices as failures with variant patterns, fallible results carried in the caller's area when a view, a layout or a wide choice is the value or the failure is wider than a word, a choice wider than a word held by its address as a local, a parameter and a result, views of device memory from mem.mmio with every element access volatile and bounds-checked, port places, cpu.interrupts and control registers compiled behind a byte that never arrives, procedures and constants of an imported library module (core.x64.paging) called and read, atomics at every width with their old values and fences, a ref to a static, block copies over an overlap and fills, case over a bool, an integer or a plain choice with its fields, each over a range, layouts by value in places, literals, parameters, arguments and results, a calls-interrupt handler entered through a frame pushed by hand and left through iretq, machine x64 blocks with in, out, clobber, local labels, a callee-saved register kept across a call and a system call by hand, a freestanding program with its own entry and stack, statics, arrays in frames, raw access and stdout"
# Port places and the interrupt flag (design 0015): each form of
# tests/run/hw.oli is exactly one instruction in the image, at its width,
# found by binutils; a read is zero-extended to the canonical image.
objdump -d --no-show-raw-insn build/hw.elf > build/hw.dis
for pin in 'out    %al,(%dx)=3' 'out    %ax,(%dx)=2' 'out    %eax,(%dx)=1' 'in     (%dx),%al=2' 'in     (%dx),%ax=2' 'in     (%dx),%eax=1' 'cli$=1' 'sti$=1' 'movzwl %ax,%eax=3' 'mov    %rax,%cr3=1' 'mov    %cr3,%rax=1' 'mov    %cr0,%rax=1' 'mov    %rax,%cr0=1' 'mov    %cr2,%rax=1' 'mov    %cr4,%rax=1' 'mov    %rax,%cr4=1' 'rdmsr=1' 'wrmsr=1' 'lgdt   (%rax)=1' 'lidt   (%rax)=1' 'mov    %rsp,%rax=2' 'mov    %rax,%rsp=1' 'mov    %rbp,%rax=2' 'mov    %rax,%rbp=1' 'jmp    \*%rax=1' 'cpuid=1' 'rdtsc=2' '<hw.nop_proc>$=1'; do
    pat=${pin%=*}; want=${pin##*=}
    [ "$(grep -c "$pat" build/hw.dis)" = "$want" ] || fail "hw: [$pat] must be $want times in hw.elf, is $(grep -c "$pat" build/hw.dis)"
done
[ "$(grep -c 'str  *%' build/hw.dis)" = 1 ] || fail "hw: the task register must be read once (str)"
[ "$(grep -c 'ltr  *%' build/hw.dis)" = 1 ] || fail "hw: the task register must be written once (ltr)"
[ "$(grep -c 'hw\.' build/hw.opt)" = 35 ] || fail "opt: hw.oli must keep its thirty-five hardware operations"
echo "ok: port.u8/u16/u32[n] and port T values with .in()/.out() are one in/out each at the width, cpu.interrupts(off/on) one cli/sti, arch.x64.cr0/2/3/4 read and written one mov each, arch.x64.msr[n] rdmsr/wrmsr, arch.x64.gdt/idt lgdt/lidt, arch.x64.tr str/ltr, cpu.stack and cpu.frame moves, cpu.jump, cpu.id cpuid, cpu.tsc rdtsc, cpu.call a call (objdump on hw.elf); the passes keep every hardware access"
# tests/run/segments.oli: arch.x64.segments(code, data) is the far return
# and five segment loads, once each, executed for real under the selectors
# a Linux process runs with and read back through the segment moves of
# design 0023; a `bytes` line stands in the image as the bytes it lists;
# Name.field.offset/size fold to constants before the OIR.
objdump -d --no-show-raw-insn build/segments.elf > build/segments.dis
for pin in 'lretq$=1' 'lea    0x3(%rip),%rax=1' 'mov    %ecx,%ds=1' 'mov    %ecx,%es=1' 'mov    %ecx,%ss=1' 'mov    %ecx,%fs=1' 'mov    %ecx,%gs=1' 'mov    %cs,%eax=1' 'mov    %ds,%eax=1' 'mov    %ss,%eax=1' 'mov    \$0x2a,%eax=1'; do
    pat=${pin%=*}; want=${pin##*=}
    [ "$(grep -c "$pat" build/segments.dis)" = "$want" ] || fail "segments: [$pat] must be $want times in segments.elf, is $(grep -c "$pat" build/segments.dis)"
done
[ "$(grep -c 'hw.cmd arch.x64.segments' build/segments.opt)" = 1 ] || fail "opt: segments.oli must keep its segment reload"
! grep -q 'field' build/segments.oir || fail "oir: Name.field.offset and Name.field.size must fold to constants, not load a field"
( cd .. && genesis/build/show_sema < tests/run/segments.oli > genesis/build/segments.sema 2>/dev/null ) || fail "segments: show_sema"
grep -q '(intrinsic Segments (int 51):u16 (int 43):u16)' build/segments.sema || fail "segments: the command must type its two selectors u16"
grep -q '(ne (int 16):? (int 16):?)' build/segments.sema || fail "segments: Pair.c.offset must be the constant 16 in the .sema form"
( cd .. && genesis/build/explain < tests/run/segments.oli > genesis/build/segments.explain 2> genesis/build/explain.err ) || fail "explain: segments.oli: $(head -1 build/explain.err)"
[ "$(grep -c 'KERNEL=1' build/segments.explain)" = 1 ] || fail "explain: segments.oli must bill exactly the segment reload KERNEL"
echo "ok: arch.x64.segments(code, data) is lretq and five segment loads executed under a process's own selectors, bytes stands as listed, Name.field.offset/size are constants (objdump on segments.elf, .oir, .sema, --explain)"
# tests/run/own.oli: `own T` is the handle's own bits - no instruction for
# `own u64 (…)` or `u64(h)` - and `<~` is a store; the linear rules are the
# checker's (tests/sema/err/linear.oli pins all six, E0340-E0345).
! grep -q 'trunc' build/own.oir || fail "oir: own u64 (x) and u64(h) must add no instruction"
( cd .. && genesis/build/show_sema < tests/run/own.oli > genesis/build/own.sema 2>/dev/null ) || fail "own: show_sema"
[ "$(grep -c '(move ' build/own.sema)" = 3 ] || fail "own: the three moves must stand in the .sema form, $(grep -c '(move ' build/own.sema) do"
grep -q '(proc own.acquire -> own u64' build/own.sema || fail "own: a procedure may return an own value"
grep -q '(local 0 h param0 own u64)' build/own.sema || fail "own: a parameter may be an own value"
grep -q '(local 1 slot place own u64)' build/own.sema || fail "own: a place may be an own value"
# tests/run/rodata.oli: every aggregate constant is a read-only symbol at
# its natural alignment, kept aligned when dead statics are compacted.
for sym in 'TABLE 4' 'WORDS 8' 'SQUARES 2' 'ORIGIN 8'; do
    set -- $sym
    a=$(nm build/rodata.elf | awk -v s="rodata.$1" '$3==s && $2=="R"{print $1}')
    [ -n "$a" ] || fail "rodata: $1 must be a read-only symbol"
    [ "$((0x$a % $2))" = 0 ] || fail "rodata: $1 at 0x$a is not $2-aligned"
done
readelf -lW build/rodata.elf | awk '$1=="LOAD"' | head -1 | grep -q 'R E' || fail "rodata: the constants must be in the read-only segment"
echo "ok: aggregate constants - arrays and a layout - live in the read-only segment under their own aligned symbols and are read, indexed, iterated, passed as views and by ref (rodata.elf)"
# tests/run/floats.oli: SSE in the image, one instruction family each.
objdump -d --no-show-raw-insn build/floats.elf > build/floats.dis
for ins in addsd subsd mulsd divsd mulss divss ucomisd ucomiss cvtsi2sd cvtsi2ss cvttsd2si cvtss2sd cvtsd2ss btc; do
    grep -q "$ins" build/floats.dis || fail "floats: $ins must be in floats.elf"
done
# Literals rounded from the decimal text, once per format (lex.oli).
printf 'module fl\na := 0.1\nb := 5e-324\nc := 2.4703282292062327e-324\nd := 1e23\ne := 9007199254740993.0\nf := 3.4028236e38\n' > build/fl.oli
./build/show_tokens < build/fl.oli > build/fl.tok || fail "floats: show_tokens"
for want in '0.1 float 4591870180066957722 1036831949' '5e-324 float 1 0' '2.4703282292062327e-324 float 0 0' '1e23 float 4950912855330343670 1705601046' '9007199254740993.0 float 4845873199050653696 1509949440' '3.4028236e38 float 5183643170920245180 4294967296'; do
    lit=${want%% *}; rest=${want#* }
    grep -q "$rest" build/fl.tok || fail "floats: the literal $lit must lex as [$rest]"
done
echo "ok: f32/f64 run - SSE arithmetic, NaN-correct comparisons, conversions both ways (u64 past 2^63 too), .bits, float statics, arrays and fields, xmm arguments and results; literals correctly rounded to binary64 and binary32 from the decimal text"
# tests/run/simd.oli: the SSE2 families of design 0024 in the image.
objdump -d --no-show-raw-insn build/simd.elf > build/simd.dis
for ins in movdqu addps subps mulps divps addpd paddd paddb paddq pmullw pand cmpeqps cmpeqpd pcmpeqb movmskps movmskpd pmovmskb; do
    grep -q "$ins" build/simd.dis || fail "simd: $ins must be in simd.elf"
done
# A vector argument of an extern procedure is refused: SysV would pass it
# in an xmm register, which the aggregate path does not do.
printf -- '-- output: object\nmodule vx\nextern proc c_take(v : f32x4) -> s32\npub proc f() -> s32\n    ret c_take(f32x4.splat(1.0))\nend\n' > build/vx.oli
set +e
( cd .. && genesis/build/olic < genesis/build/vx.oli > genesis/build/vx.o 2> genesis/build/vx.err )
st=$?
set -e
[ "$st" = 1 ] || fail "simd: a vector argument of an extern procedure exited $st, want E0900"
grep -q 'E0900' build/vx.err || fail "simd: a vector argument of an extern procedure must be E0900"
echo "ok: 128-bit vectors run - f32x4 f64x2 and the integer lanes: construction, splat, bounds-checked load/store and lanes, + - * / & | ^ and whole-vector ==/!= in SSE2 (objdump on simd.elf), passed and returned; what SSE2 lacks is E0900"
echo "ok: own T and <~ run - a file descriptor acquired, moved, passed and released exactly once and never twice (own.elf exits 42 because closing it again fails), the wrap and the unwrap add no instruction, and the six linear diagnostics are exact"
# Atomics (MACHINE_MODEL.md 5): every read-modify-write of tests/run/atomic.oli
# is a lock-prefixed instruction at its width, and/or/xor a cmpxchg loop, a
# sequentially consistent store an xchg, every fence its instruction; no
# pass removes one, and --explain bills each line ATOMIC.
objdump -d --no-show-raw-insn build/atomic.elf > build/atomic.dis
for pin in 'lock xadd %rcx,(%rdi)=3' 'lock xadd %cl,(%rdi)=1' 'lock xadd %cx,(%rdi)=1' 'lock xadd %ecx,(%rdi)=1' 'lock cmpxchg %rdx,(%rdi)=5' 'xchg   %rcx,(%rdi)=2' 'xchg   %ecx,(%rdi)=1' 'mfence=1' 'lfence=1' 'sfence=1' 'sete   %al=3' 'neg    %rcx=1'; do
    pat=${pin%=*}; want=${pin##*=}
    [ "$(grep -c "$pat" build/atomic.dis)" = "$want" ] || fail "atomic: [$pat] must be $want times in atomic.elf, is $(grep -c "$pat" build/atomic.dis)"
done
[ "$(grep -c 'atomic\.' build/atomic.opt)" = 18 ] || fail "opt: atomic.oli must keep its eighteen atomic operations"
[ "$(grep -c 'cpu.fence' build/atomic.opt)" = 4 ] || fail "opt: atomic.oli must keep its four fences"
( cd .. && genesis/build/explain < tests/run/atomic.oli > genesis/build/atomic.explain 2> genesis/build/explain.err ) || fail "explain: atomic.oli: $(head -1 build/explain.err)"
[ "$(grep -c 'ATOMIC=' build/atomic.explain)" = 22 ] || fail "explain: atomic.oli must bill twenty-two lines ATOMIC"
echo "ok: atomic.load/store/add/sub/and/or/xor/exchange/cas and cpu.fence are lock-prefixed instructions, xchg and fences at their widths (objdump on atomic.elf), kept by every pass and billed ATOMIC"
# mem.copy/set/zero/secure_zero (OIR_SPEC 4): a copy is rep movsb in both
# directions (an overlap copies right), a fill rep stosb; the five copies
# and three fills of tests/run/memops.oli stay through the passes and are
# COPY in --explain.
objdump -d --no-show-raw-insn build/memops.elf > build/memops.dis
for pin in 'rep movsb=10' 'rep stos=3' 'std$=5' 'cld$=5'; do
    pat=${pin%=*}; want=${pin##*=}
    [ "$(grep -c "$pat" build/memops.dis)" = "$want" ] || fail "memops: [$pat] must be $want times in memops.elf, is $(grep -c "$pat" build/memops.dis)"
done
[ "$(grep -c 'mem\.' build/memops.opt)" = 8 ] || fail "opt: memops.oli must keep its eight block operations"
( cd .. && genesis/build/explain < tests/run/memops.oli > genesis/build/memops.explain 2> genesis/build/explain.err ) || fail "explain: memops.oli: $(head -1 build/explain.err)"
[ "$(grep -c 'COPY=' build/memops.explain)" = 8 ] || fail "explain: memops.oli must bill eight lines COPY"
echo "ok: mem.copy (rep movsb, backwards over an overlap, bounds-checked), mem.set, mem.zero and mem.secure_zero (rep stosb) run, stay through every pass and are COPY in --explain"
# The image of a program with statics has two loadable segments, the second
# read+write on the page after the first; hello.elf still has one.
[ "$(od -An -tu2 -j56 -N2 build/statics.elf | tr -d ' ')" = 2 ] || fail "elf: statics.elf should have two program headers"
[ "$(od -An -tu2 -j56 -N2 build/hello.elf | tr -d ' ')" = 1 ] || fail "elf: hello.elf should have one program header"
# The tables after the image (ABI.md 4): section headers binutils read, and
# a symbol table with every procedure and static under its module's name -
# local unless pub, an export clause a second global name.
readelf -SW build/hello.elf | grep -q ' \.text  *PROGBITS  *0000000000400084' || fail "elf: hello.elf must have a .text section header at the code"
[ "$(nm build/hello.elf)" = "0000000000400084 t hello.start" ] || fail "elf: nm must list hello.start, local, at the entry"
objdump -d build/hello.elf | grep -q '^0000000000400084 <hello.start>:' || fail "elf: objdump -d must disassemble hello.elf by its symbols"
nm build/statics.elf | grep -q ' B statics.buf$' || fail "elf: the zero static buf of statics.oli must be a .bss symbol"
nm build/statics.elf | grep -q ' t olic.trap$' || fail "elf: the trap routine must be the local symbol olic.trap"
nm build/wide.elf | grep -q ' t wide.relay$' || fail "elf: every procedure must be a symbol"
readelf -a build/statics.elf > build/statics.readelf 2> build/readelf.err || fail "elf: readelf -a rejects statics.elf"
[ ! -s build/readelf.err ] || fail "elf: readelf -a warns about statics.elf: $(head -1 build/readelf.err)"
echo "ok: every image carries section headers and a symbol table (module.name, local unless pub, statics in .data/.bss/.rodata, olic.trap): nm, readelf -S and objdump -d read it, and readelf -a has no complaint"
# Debug information (ABI.md 7): DWARF 4 line tables and a compile unit with
# a subprogram per procedure - what addr2line, objdump --dwarf and gdb read.
[ "$(objdump --dwarf=decodedline build/hello.elf | grep -c '^hello.oli')" -ge 4 ] || fail "dwarf: hello.elf must carry line rows for hello.oli"
hstart=$(grep -n '^proc start' ../examples/hello.oli | cut -d: -f1)
[ "$(addr2line -e build/hello.elf 0x400084)" = "hello.oli:$hstart" ] || fail "dwarf: the entry of hello.elf must resolve to hello.oli:$hstart, got $(addr2line -e build/hello.elf 0x400084)"
[ "$(readelf --debug-dump=line build/hello.elf 2>&1 | grep -ci 'warn\|error')" = 0 ] || fail "dwarf: readelf complains about hello.elf's line table"
# The kernel image is built here, before its line table is read: the M3/M4
# checks below compile it again, and must find the same bytes.
( cd .. && genesis/build/olic < examples/kernel.oli > genesis/build/kernel.elf 2> genesis/build/kernel.err ) || fail "dwarf: olic could not compile examples/kernel.oli: $(head -1 build/kernel.err)"
cp build/kernel.elf build/kernel.dwarf.elf
kmain=$(grep -n '^proc main' ../examples/kernel.oli | cut -d: -f1)
gdb -batch -ex 'info line kernel.main' build/kernel.elf 2>&1 | grep -q "Line $kmain of \"kernel.oli\" starts at address" || fail "dwarf: gdb must place kernel.main on kernel.oli:$kmain: $(gdb -batch -ex 'info line kernel.main' build/kernel.elf 2>&1 | tail -1)"
objdump --dwarf=decodedline build/kernel.elf | grep -q '^core/x64/paging.oli' || fail "dwarf: the lines of the library module core.x64.paging must be in the kernel's line table"
# Variables: tests/run/debugvars.oli is compiled with `-- debug: frame`, so
# gdb reads every parameter and local at the `ret` of `work`; without the
# pragma a promoted scalar is <optimized out>, never a stale value, while an
# array or a layout (in its frame words either way) still prints.
dvl=$(grep -n '^    ret total' ../tests/run/debugvars.oli | cut -d: -f1)
gdb -batch -ex 'directory ../tests/run' -ex "break debugvars.oli:$dvl" -ex run -ex 'info args' -ex 'info locals' -ex 'p *msg.addr@msg.len' -ex 'ptype pt' build/debugvars.elf > build/debugvars.gdb 2>&1 || true
for want in 'n = 5' 'scale = -2' 'total = 20' 'buf = {11, 0, 0, 44}' 'pt = {x = 7, y = -3}' 'i = 5' 'flag = true' '= "hi"' 'u32 x;' 's16 y;'; do
    grep -qF "$want" build/debugvars.gdb || fail "dwarf: gdb must print [$want] at the ret of debugvars.work"
done
sed '1d' ../tests/run/debugvars.oli > build/debugvars_o2.oli
( cd .. && genesis/build/olic < genesis/build/debugvars_o2.oli > genesis/build/debugvars_o2.elf ) || fail "dwarf: debugvars without the pragma"
chmod +x build/debugvars_o2.elf
gdb -batch -ex 'directory build' -ex "break debugvars.oli:$((dvl - 1))" -ex run -ex 'info locals' build/debugvars_o2.elf > build/debugvars_o2.gdb 2>&1 || true
grep -qF 'total = <optimized out>' build/debugvars_o2.gdb || fail "dwarf: a promoted local must be <optimized out> without the pragma"
grep -qF 'pt = {x = 7, y = -3}' build/debugvars_o2.gdb || fail "dwarf: a layout in its frame words must print without the pragma"
[ "$(readelf --debug-dump=info build/debugvars.elf 2>&1 | grep -ci 'warn\|error')" = 0 ] || fail "dwarf: readelf complains about the variables of debugvars.elf"
echo "ok: DWARF 4 line tables and a compile unit with a subprogram per procedure: objdump --dwarf decodes them, addr2line names the entry's line, gdb places kernel.main on its line, library modules are files of their own"
# The self-test of arith.oli is decided at compile time, and the messages of
# the checks that were proved away are not in its image.
arithsz=$(readelf -lW build/arith.elf | awk '$1=="LOAD"{print $5; exit}')
[ "$((arithsz))" -lt 200 ] || fail "opt: arith.elf's code segment ($arithsz bytes) still carries the messages of checks that were proved away"

# Trapping arithmetic (design 0006, MACHINE_MODEL.md §4): the message names the
# kind and the line, and the status is 134.
for f in ../tests/run/trap/*.oli; do
    n=$(basename "$f" .oli)
    want=$(grep -- '-- expect: ' "$f" | sed 's/-- expect: //')
    ( cd .. && genesis/build/olic < "tests/run/trap/$n.oli" > genesis/build/t_$n.elf 2> genesis/build/t_$n.err ) \
        || fail "trap: olic could not compile $f: $(head -1 build/t_$n.err)"
    chmod +x build/t_$n.elf
    set +e; ./build/t_$n.elf 2> build/t_$n.out; st=$?; set -e
    [ "$st" = 134 ] || fail "trap: $n exited $st, want 134"
    got=$(cat build/t_$n.out)
    [ "$got" = "$want" ] || fail "trap: $n printed [$got], want [$want]"
done
echo "ok: overflow, narrow-width overflow, division by zero, MIN/-1, negation, an index or subview outside its view, a short or misaligned Name.at, an exhausted zone, a child zone its parent cannot hold and a zone over a short buffer trap with the kind and line on fd 2 and exit 134"

# Freestanding (FREESTANDING.md): the entry has no frame, `cpu.halt()` is
# one instruction, and a trap reaches the `traps` procedure with its
# `core.Site` — tests/freestanding/NAME.oli exits with the status its
# `-- expect: exit N` line says and writes nothing, having no message to print.
grep -q '^      cpu\.halt$' build/freestanding.oir || fail "oir: cpu.halt() must be one instruction of its own"
grep -q 'zone\.new\.raw' build/freestanding.oir || fail "oir: a freestanding zone is at an address or from a buffer"
for f in ../tests/freestanding/*.oli; do
    n=$(basename "$f" .oli)
    want=$(grep -- '-- expect: exit ' "$f" | sed 's/.*-- expect: exit //')
    ( cd .. && genesis/build/olic < "tests/freestanding/$n.oli" > genesis/build/fs_$n.elf 2> genesis/build/fs_$n.err ) || fail "freestanding: olic could not compile $f: $(head -1 build/fs_$n.err)"
    chmod +x build/fs_$n.elf
    set +e; timeout 10 ./build/fs_$n.elf > build/fs_$n.out 2>&1; st=$?; set -e
    [ "$st" = "$want" ] || fail "freestanding: $n exited $st, want $want"
    [ ! -s build/fs_$n.out ] || fail "freestanding: $n wrote output, and it has no OS to write to"
done
echo "ok: a freestanding program runs from its own entry with no frame, installs its own stack, and a trap in it reaches the traps procedure with the kind and the core.Site of the line"
# The target profile (FREESTANDING.md 2): tests/freestanding/profile.oli is
# laid out by tests/freestanding/x86_64-profile.oli-target - loaded where
# it says, its code sections in its order and on its alignment, its zero
# statics grouped by section - and the section headers say so.
readelf -lW build/fs_profile.elf | grep -q 'LOAD .*0x0000000000500000 0x0000000000500000' || fail "profile: the image must be loaded at the profile's load_address 0x500000"
[ "$(readelf -SW build/fs_profile.elf | grep -oP '\] \.\S+' | tr -d '] ' | tr '\n' ' ')" = ".rodata .text.boot .text.init .text .text.trap .data .bss.stack .bss .symtab .strtab .shstrtab .debug_abbrev .debug_info .debug_line " ] || fail "profile: the sections must follow the profile's order: $(readelf -SW build/fs_profile.elf | grep -oP '\] \.\S+' | tr -d '] ' | tr '\n' ' ')"
for s in .text.boot .text.init .text .text.trap; do
    a=$(readelf -SW build/fs_profile.elf | grep -oP "\] $s\s+PROGBITS\s+[0-9a-f]{16}" | awk '{print $NF}')
    [ $(( 0x$a % 64 )) = 0 ] || fail "profile: $s at 0x$a is not on the profile's 64-byte section alignment"
done
objdump -t build/fs_profile.elf | grep -q '\.text\.init.*profile\.init_a$' || fail "profile: init_a must lie in .text.init"
objdump -t build/fs_profile.elf | grep -q '\.bss\.stack.*profile\.stack$' || fail "profile: the stack must lie in .bss.stack"
readelf -a build/fs_profile.elf > /dev/null 2> build/readelf.err || fail "profile: readelf -a rejects fs_profile.elf"
[ ! -s build/readelf.err ] || fail "profile: readelf -a warns: $(head -1 build/readelf.err)"
echo "ok: a target profile lays the image out - load_address, the order of sections, align_sections - and every code and zero-static section has its own header"
# Conditional compilation (LANGUAGE.md 13): `when target.FACT ... else ... end`
# at declaration level keeps one branch and drops the other; the hosted
# program has six procedures and none that halts the CPU, the freestanding
# one exits with its own answer.
( cd .. && genesis/build/show_sema < tests/run/when.oli > genesis/build/when.sema 2>/dev/null ) || fail "when: show_sema"
[ "$(grep -c '(proc when_test' build/when.sema)" = 6 ] || fail "when: the hosted program must have exactly its six taken procedures, has $(grep -c '(proc when_test' build/when.sema)"
! grep -q 'CpuHalt' build/when.sema || fail "when: the freestanding branch must have been dropped"
[ "$(grep -c ' dropped' build/when.ast)" = 5 ] || fail "when: --show-ast must show the five dropped branches"
echo "ok: when target.freestanding/hosted/object/os/arch keeps one branch of declarations and drops the other, in the AST for --show-ast and nowhere else"

# The reference program of the language documents runs: examples/packet_demo.oli
# builds a packet in a zone, parses its header through a layout with a `be`
# field, sums the payload, writes on stdout, reads cpuid through a machine
# block and prints the vendor, and peeks a raw address — every construct the
# document introduces, compiled by olic and executed here.
( cd .. && genesis/build/olic < examples/packet_demo.oli > genesis/build/packet_demo.elf 2> genesis/build/packet_demo.err ) || fail "packet_demo: olic could not compile examples/packet_demo.oli: $(head -1 build/packet_demo.err)"
chmod +x build/packet_demo.elf
set +e; timeout 10 ./build/packet_demo.elf > build/packet_demo.out 2>&1; st=$?; set -e
[ "$st" = 0 ] || fail "packet_demo: exited $st, want 0"
[ "$(head -1 build/packet_demo.out)" = "empty" ] || fail "packet_demo: the first line must be the checksum verdict"
[ "$(wc -c < build/packet_demo.out)" = 19 ] || fail "packet_demo: the output is the verdict, the twelve-byte cpuid vendor and a newline"
echo "ok: examples/packet_demo.oli - the reference program of the language documents - compiles and runs: zone, layout with a be field, choice failures, each, syscalls, machine block, raw address"

# M3/M4 (FREESTANDING.md 8): the kernel image, checked structurally first:
# linked at the address its profile names, the Multiboot2 header (a static
# layout with an initialiser, placed in `.text.boot`) right after the ELF
# and program headers with the magic, length and checksum the loader will
# verify, the Multiboot 1 header of `mb1` (the a.out kludge QEMU's -kernel
# needs) with the entry `start`, the 32-bit trampoline of `start` decoding
# as the instructions that enter long mode, port I/O and cpuid in its
# blocks, and not one system call in any procedure. Then, when QEMU is on
# the path (or OLI_QEMU names it), the image is booted for real.
( cd .. && genesis/build/olic < examples/kernel.oli > genesis/build/kernel.elf 2> genesis/build/kernel.err ) || fail "kernel: olic could not compile examples/kernel.oli: $(head -1 build/kernel.err)"
cmp build/kernel.elf build/kernel.dwarf.elf || fail "kernel: a second compile of examples/kernel.oli gave other bytes"
kentry=$(readelf -h build/kernel.elf | sed -n 's/.*Entry point address: *0x\([0-9a-f]*\).*/\1/p')
ksize=$(readelf -lW build/kernel.elf | awk '$1=="LOAD"{print $5; exit}')
[ "$((0x$kentry))" -ge "$((0x100000))" ] && [ "$((0x$kentry))" -lt "$((0x100000 + $ksize))" ] \
    || fail "kernel: the entry 0x$kentry is not in the segment loaded at 0x100000 ($ksize bytes)"
readelf -l build/kernel.elf | grep -q 'LOAD .*0x0000000000100000 0x0000000000100000' || fail "kernel: the first segment is not loaded at 0x100000"
[ "$(od -An -tx1 -j176 -N24 build/kernel.elf | tr -d ' \n')" = "d65052e800000000180000001200ad17000000000800000000" ] \
    || [ "$(od -An -tx1 -j176 -N24 build/kernel.elf | tr -d ' \n')" = "d65052e8000000001800000012afad170000000008000000" ] \
    || fail "kernel: the Multiboot2 header is not at offset 176: $(od -An -tx1 -j176 -N24 build/kernel.elf | tr -d '\n')"
( cd .. && genesis/build/show_asm < examples/kernel.oli > genesis/build/kernel.asm 2>/dev/null ) || fail "kernel: show_asm"
objdump -d --no-show-raw-insn build/kernel.elf > build/kernel.dis
[ "$(grep -c 'out    %al,(%dx)' build/kernel.dis)" -ge 15 ] || fail "kernel: COM1, the two PICs and the PIT are written through port places, one out dx, al each ($(grep -c 'out    %al,(%dx)' build/kernel.dis) found)"
[ "$(grep -c 'sti$' build/kernel.dis)" = 1 ] || fail "kernel: cpu.interrupts(on) must be one sti"
[ "$(grep -c 'iretq' build/kernel.dis)" = 2 ] || fail "kernel: the int 3 and timer handlers must both end in iretq"
[ "$(grep -c 'mov    %rax,%cr3' build/kernel.dis)" = 2 ] || fail "kernel: cr3 is written twice - the trampoline's tables and paging_init's"
[ "$(grep -c 'mov    %cr3,%rax' build/kernel.dis)" = 1 ] || fail "kernel: cr3 must be read back once"
grep -q 'cpuid' build/kernel.dis || fail "kernel: cpu.id(0) must be one cpuid"
grep -q 'lidt   (%rax)' build/kernel.dis || fail "kernel: arch.x64.idt <- must be one lidt"
grep -q 'call.*<kernel.main>' build/kernel.dis || fail "kernel: cpu.call(main) must be one call in the frameless entry"
grep -q '^    cd 03)' build/kernel.asm || fail "kernel: the software interrupt must be int 3"
( cd .. && genesis/build/show_oir < examples/kernel.oli > genesis/build/kernel.oir 2>/dev/null ) || fail "kernel: show_oir"
[ "$(grep -c 'raw.store.8.mmio' build/kernel.oir)" = 2 ] || fail "kernel: the VGA cells must be written through mem.mmio, volatile"
[ "$(grep -c 'hw.store port.u8' build/kernel.oir)" = 15 ] || fail "kernel: fifteen port writes - COM1 setup and byte, ICW1-4 and the masks of two PICs, the PIT's mode and divisor, the EOI"
[ "$(grep -c 'hw.store arch.x64.cr3' build/kernel.oir)" = 1 ] || fail "kernel: the page tables go into cr3 through a hardware place"
[ "$(grep -c 'hw.store cpu.stack\|hw.store cpu.frame\|hw.cmd cpu.call main\|hw.store arch.x64.idt\|hw.cmd cpu.id' build/kernel.oir)" = 5 ] || fail "kernel: the entry, the IDT load and cpuid must be hardware statements in the OIR"
nm build/kernel.elf | grep -q '^00000000001000b0 R kernel.header$' || fail "kernel: the Multiboot2 header must be the .rodata symbol kernel.header at 0x1000b0"
[ "$(readelf -SW build/kernel.elf | grep -oP '\] \.\S+' | tr -d '] ' | tr '\n' ' ')" = ".rodata .text.boot .text .text.trap .data .bss.boot .bss .symtab .strtab .shstrtab .debug_abbrev .debug_info .debug_line " ] || fail "kernel: the sections must follow examples/x86_64-kernel.oli-target: $(readelf -SW build/kernel.elf | grep -oP '\] \.\S+' | tr -d '] ' | tr '\n' ' ')"
nm build/kernel.elf | grep -q ' T core.x64.paging.identity_2m$' || fail "kernel: the pub procedures of core.x64.paging must be global symbols"
grep -q 'call core.x64.paging.identity_2m\|= call ' build/kernel.oir || fail "kernel: the page directory must be filled by core.x64.paging"
[ "$(od -An -tx1 -v build/kernel.elf | tr -d '\n' | grep -o '48 cf' | wc -l)" = 2 ] || fail "kernel: exactly two calls-interrupt handlers end in iretq"
( cd .. && genesis/build/explain < examples/kernel.oli > genesis/build/kernel.explain 2>/dev/null ) || fail "kernel: explain"
[ "$(grep -c 'syscalls=0' build/kernel.explain)" = "$(grep -c '(proc ' build/kernel.explain)" ] || fail "kernel: a procedure of the kernel makes a system call"
# The boot path: the Multiboot 1 header (magic, flags bit 16, checksum,
# header_addr = mb1, load_addr 0x100000, load_end 0, bss_end 0x200000,
# entry_addr = start) at `mb1`, and `start` read as 32-bit code.
kmb1=$(nm build/kernel.elf | awk '$3=="kernel.mb1"{print $1}')
kstart=$(nm build/kernel.elf | awk '$3=="kernel.start"{print $1}')
kstart64=$(nm build/kernel.elf | awk '$3=="kernel.start64"{print $1}')
[ -n "$kmb1" ] && [ -n "$kstart" ] && [ -n "$kstart64" ] || fail "kernel: mb1, start and start64 must be symbols"
[ "$((0x$kmb1 % 4))" = 0 ] || fail "kernel: the Multiboot 1 header must be four-byte aligned"
[ "$((0x$kmb1 - 0x100000))" -lt 8192 ] || fail "kernel: the Multiboot 1 header must lie in the first 8 KiB"
mb1got=$(od -An -tx1 -j$((0x$kmb1 - 0x100000)) -N32 build/kernel.elf | tr -d ' \n')
mb1want="02b0ad1b00000100fe4f51e4$(printf '%08x' 0x$kmb1 | sed 's/\(..\)\(..\)\(..\)\(..\)/\4\3\2\1/')000010000000000000002000$(printf '%08x' 0x$kstart | sed 's/\(..\)\(..\)\(..\)\(..\)/\4\3\2\1/')"
[ "$mb1got" = "$mb1want" ] || fail "kernel: the Multiboot 1 header at mb1 is [$mb1got], want [$mb1want]"
[ "$((0x$kentry))" = "$((0x$kstart))" ] || fail "kernel: the ELF entry must be start (the trampoline), is 0x$kentry"
objdump -d -M i386 --no-show-raw-insn --start-address=0x$kstart --stop-address=0x$kstart64 build/kernel.elf > build/kernel.boot32
for pin in 'cli$' 'mov    %ebx,0x' 'mov    %eax,%cr3' 'mov    %cr4,%eax' 'or     $0x20,%eax' 'mov    %eax,%cr4' 'mov    $0xc0000080,%ecx' 'rdmsr' 'or     $0x100,%eax' 'wrmsr' 'movl   $0x209a00,0xc(%edi)' 'movl   $0x9200,0x14(%edi)' 'movw   $0x17,(%esi)' 'lgdtl  (%esi)' 'mov    %cr0,%eax' 'or     $0x80000001,%eax' 'mov    %eax,%cr0' 'add    $0x200000,%edx' 'mov    $0x200,%ecx'; do
    grep -q "$pin" build/kernel.boot32 || fail "kernel: the 32-bit trampoline must contain [$pin]"
done
grep -q "ljmp   \$0x8,\$0x$(printf '%x' 0x$kstart64)$" build/kernel.boot32 || fail "kernel: the trampoline must far-jump to 0x8:start64"
grep -q 'lretq' build/kernel.dis || fail "kernel: start64 must reload the segments (arch.x64.segments, lretq)"
grep -q 'hw.cmd arch.x64.segments' build/kernel.oir || fail "kernel: start64 must reload the segments through the command"
echo "ok: examples/kernel.oli carries a Multiboot 1 header (a.out kludge, entry start) at mb1 and start is a 32-bit trampoline - tables, cr3, PAE, EFER.LME, a GDT, cr0.PG - ending in ljmp 0x8:start64 (objdump -M i386)"
# The boot itself. QEMU is not part of the toolchain; when it is present the
# image is loaded by QEMU's Multiboot loader in 32-bit protected mode, the
# kernel talks on COM1 and leaves through the isa-debug-exit device (status
# 33). Without QEMU this step says so; the run of 2026-09-24 with QEMU
# 10.0.13 is transcribed in docs/PROJECT_STATUS.md (stage 34).
QEMU=${OLI_QEMU:-$(command -v qemu-system-x86_64 || true)}
if [ -n "$QEMU" ]; then
    qbios=""
    [ -n "${OLI_QEMU_BIOS:-}" ] && qbios="-L $OLI_QEMU_BIOS"
    [ -n "${OLI_QEMU_DATA:-}" ] && qbios="$qbios -L $OLI_QEMU_DATA"
    rm -f build/kernel.serial
    set +e
    timeout 120 "$QEMU" $qbios -accel tcg -display none -nic none -no-reboot -m 64 -serial file:build/kernel.serial -device isa-debug-exit,iobase=0x501,iosize=1 -kernel build/kernel.elf > build/kernel.qemu 2>&1
    st=$?
    set -e
    [ "$st" = 33 ] || fail "boot: qemu exited $st, want 33 (isa-debug-exit after the timer): $(head -2 build/kernel.qemu)"
    [ "$(sed -n 1p build/kernel.serial)" = "Oli-- kernel" ] || fail "boot: the first serial line must be the greeting, is [$(sed -n 1p build/kernel.serial)]"
    grep -q '^cpu: \(GenuineIntel\|AuthenticAMD\|other\)$' build/kernel.serial || fail "boot: the kernel must report its cpuid vendor on COM1"
    grep -q '^int 3 at 00000000001[0-9a-f]\{5\}$' build/kernel.serial || fail "boot: the int 3 handler must report the return address"
    grep -q '^back from int 3$' build/kernel.serial || fail "boot: the kernel must return from int 3"
    grep -q '^timer: 100 ticks$' build/kernel.serial || fail "boot: the PIT must have ticked a hundred times"
    grep -q '^memory: [1-9][0-9]* regions, [0-9]* frames free$' build/kernel.serial || fail "boot: the kernel must report the RAM of the loader's memory map"
    grep -q '^frame: 0000000000[0-9a-f]\{6\} written and read$' build/kernel.serial || fail "boot: a frame must be allocated, written and read back"
    grep -q '^heap: 1000 records at 0000000000[0-9a-f]\{6\}, sum of squares 332833500, 1M refused, frames returned$' build/kernel.serial || fail "boot: the kernel heap - a zone at a megabyte of frames - must hold a thousand records, refuse 1M more and give its frames back"
    [ "$(wc -l < build/kernel.serial)" = 8 ] || fail "boot: COM1 must carry exactly eight lines, carries $(wc -l < build/kernel.serial)"
    # The same image with twice the memory: the frame count must follow the
    # machine by exactly 64 MiB / 4 KiB, so it is read, not assumed.
    set +e
    timeout 120 "$QEMU" $qbios -accel tcg -display none -nic none -no-reboot -m 128 -serial file:build/kernel128.serial -device isa-debug-exit,iobase=0x501,iosize=1 -kernel build/kernel.elf > build/kernel128.qemu 2>&1
    st=$?
    set -e
    [ "$st" = 33 ] || fail "boot: qemu -m 128 exited $st, want 33"
    f64=$(sed -n 's/^memory: .* regions, \([0-9]*\) frames free$/\1/p' build/kernel.serial)
    f128=$(sed -n 's/^memory: .* regions, \([0-9]*\) frames free$/\1/p' build/kernel128.serial)
    [ -n "$f64" ] && [ -n "$f128" ] && [ "$((f128 - f64))" = 16384 ] || fail "boot: 128 MiB must free 16384 frames more than 64 MiB ($f64, $f128)"
    echo "ok: BOOT - $("$QEMU" -version | head -1 | sed 's/ (.*//') loads kernel.elf by its Multiboot header in 32-bit mode, the trampoline enters long mode, the kernel greets on COM1, reports cpuid, frees the RAM of the loader's memory map ($f64 frames at 64 MiB, $f128 at 128 MiB) and writes an allocated frame, runs a heap zone laid at a megabyte of frames, takes int 3 and 100 timer ticks, and exits through isa-debug-exit with 33"
else
    echo "skipped: no qemu-system-x86_64 here (set OLI_QEMU, and OLI_QEMU_BIOS/OLI_QEMU_DATA for its firmware) - the boot of examples/kernel.oli was verified with QEMU 10.0.13 on 2026-09-24, transcript in docs/PROJECT_STATUS.md"
fi
echo "ok: examples/kernel.oli is an ELF64 loaded at 0x100000 with its Multiboot2 header at offset 176, COM1, the PICs and the PIT written through port places, cpuid, lidt and int 3 in its machine blocks, two calls-interrupt handlers ending in iretq, sti, and no system call anywhere (run it: qemu-system-x86_64 -kernel kernel.elf -serial stdio)"

# Object files (ABI.md 6, `-- output: object`): a relocatable ELF with the
# same sections at address 0, .rela.text for every absolute address and
# every extern call, an undefined global symbol per `extern proc`, and
# `_start` for an entry procedure. Two Oli-- objects link with ld alone and
# run; the C half of tests/c links with cc, calls into Oli-- and is called
# back, and prints what tests/c/expected.out says.
for n in oli_side lib_side main_side pic_side float_side; do
    ( cd .. && genesis/build/olic < tests/c/$n.oli > genesis/build/$n.o 2> genesis/build/$n.err ) || fail "object: olic could not compile tests/c/$n.oli: $(head -1 build/$n.err)"
done
readelf -h build/oli_side.o | grep -q 'Type: *REL' || fail "object: oli_side.o is not ET_REL"
readelf -SW build/oli_side.o | grep -q '\.rela\.text  *RELA' || fail "object: oli_side.o has no .rela.text"
[ "$(nm build/oli_side.o | grep -c ' U c_double$\| U write$')" = 2 ] || fail "object: the two extern procedures must be undefined symbols"
nm build/oli_side.o | grep -q ' T oli_add$' || fail "object: the exported procedure must be the global symbol oli_add"
[ "$(readelf -rW build/oli_side.o | grep -c 'R_X86_64_PLT32')" = 2 ] || fail "object: one PLT32 relocation per extern call"
[ "$(readelf -rW build/oli_side.o | grep -c 'R_X86_64_PC32  *0000000000000000 .rodata')" = 2 ] || fail "object: the string literal and the trap message are reached rip-relative through .rodata (PC32)"
readelf -a build/oli_side.o > /dev/null 2> build/readelf.err || fail "object: readelf -a rejects oli_side.o"
[ ! -s build/readelf.err ] || fail "object: readelf -a warns about oli_side.o: $(head -1 build/readelf.err)"
nm build/main_side.o | grep -q ' T _start$' || fail "object: the entry procedure must also be _start"
ld -o build/two build/main_side.o build/lib_side.o 2> build/ld.err || fail "object: ld could not link two Oli-- objects: $(head -1 build/ld.err)"
set +e; ./build/two; st=$?; set -e
[ "$st" = 42 ] || fail "object: the program linked from two Oli-- objects exited $st, want 42"
if command -v cc > /dev/null 2>&1; then
    cc -no-pie -o build/c_prog build/oli_side.o ../tests/c/c_side.c 2> build/cc.err || fail "object: cc could not link oli_side.o with tests/c/c_side.c: $(head -1 build/cc.err)"
    ./build/c_prog > build/c_prog.out || fail "object: the C program linked with Oli-- exited non-zero"
    cmp build/c_prog.out ../tests/c/expected.out || fail "object: the C program's output differs from tests/c/expected.out"
    echo "ok: object files: two Oli-- objects link with ld alone and run; oli_side.o links with C by cc, C calls oli_add, oli_add calls C's c_double and libc's write, and the output is tests/c/expected.out"
    # Position-independent code: every static is reached rip-relative, so
    # the object links into cc's default PIE and into a shared library with
    # no text relocation, and both run.
    ! readelf -rW build/pic_side.o | grep -q 'R_X86_64_64\|R_X86_64_32' || fail "pic: the compiler's own code must not carry an absolute relocation"
    cc -o build/pic_pie ../tests/c/pic_main.c build/pic_side.o 2> build/cc.err || fail "pic: cc could not link a PIE: $(head -1 build/cc.err)"
    [ ! -s build/cc.err ] || fail "pic: linking the PIE warned: $(head -1 build/cc.err)"
    file build/pic_pie | grep -q 'pie executable' || fail "pic: cc did not make a PIE"
    ./build/pic_pie > build/pic_pie.out || fail "pic: the PIE exited non-zero"
    cmp build/pic_pie.out ../tests/c/pic_expected.out || fail "pic: the PIE's output differs from tests/c/pic_expected.out"
    cc -shared -o build/libpic_side.so build/pic_side.o 2> build/cc.err || fail "pic: cc could not make a shared library: $(head -1 build/cc.err)"
    [ ! -s build/cc.err ] || fail "pic: linking the shared library warned: $(head -1 build/cc.err)"
    ! readelf -d build/libpic_side.so | grep -q TEXTREL || fail "pic: the shared library needs text relocations"
    cc -o build/pic_so ../tests/c/pic_main.c -Lbuild -lpic_side 2> build/cc.err || fail "pic: cc could not link against libpic_side.so"
    LD_LIBRARY_PATH=build ./build/pic_so > build/pic_so.out || fail "pic: the program linked against the shared library exited non-zero"
    cmp build/pic_so.out ../tests/c/pic_expected.out || fail "pic: the shared-library program's output differs"
    echo "ok: position-independent code - statics rip-relative (PC32), the object linked into a PIE and into libpic_side.so without text relocations, both run"
    # Floats across the C boundary: xmm arguments and results both ways,
    # libm's sqrt called from Oli--, and narrow integers re-extended.
    cc -o build/float_c ../tests/c/float_main.c build/float_side.o -lm 2> build/cc.err || fail "float: cc could not link float_side.o: $(head -1 build/cc.err)"
    ./build/float_c > build/float_c.out || fail "float: the C program exited non-zero"
    cmp build/float_c.out ../tests/c/float_expected.out || fail "float: the C program printed $(cat build/float_c.out), want $(cat ../tests/c/float_expected.out)"
    echo "ok: f64/f32 across the C boundary - sqrt from libm and a C function with mixed xmm and integer arguments called from Oli--, Oli-- float procedures called from C, narrow integers re-extended both ways"
else
    echo "ok: object files: two Oli-- objects link with ld alone and run (no cc on this machine: the C half of tests/c was not linked)"
fi

# A program with no trap site carries no trap routine.
printf 'module notrap\nproc start -> s32\n    entry\n    ret 0\nend\n' > build/notrap.oli
( cd .. && genesis/build/olic < genesis/build/notrap.oli > genesis/build/notrap.elf ) || fail "olic could not compile notrap.oli"
notrapsz=$(readelf -lW build/notrap.elf | awk '$1=="LOAD"{print $5; exit}')
[ "$((notrapsz))" -lt 200 ] || fail "notrap.elf's code segment is $notrapsz bytes: the trap routine was emitted anyway"
echo "ok: the trap routine and its messages are in the binary only when a trap site is"

# Anything the back end cannot lower is E0900, never approximated: here a
# read of arch.x64.gdt (there is no `sgdt` form, COMMANDS.md) — it must be
# refused, not a zero in its place.
printf 'module z\nimport core.x64\nproc start -> s32\n    entry\n    permit cpu.control\n    t := arch.x64.gdt\n    ret 0\nend\n' > build/nolower.oli
set +e
( cd .. && genesis/build/olic < genesis/build/nolower.oli > genesis/build/nolower.elf 2> genesis/build/nolower.err )
st=$?
set -e
[ "$st" = 1 ] || fail "back end: an unlowered construct exited $st"
# A system call has the number and six argument registers (ABI.md 5) and no
# stack words: an eighth word is refused.
printf 'module s8\nproc start -> s32\n    entry\n    permit os.syscall\n    os.syscall(1, 2, 3, 4, 5, 6, 7, 8)\n    ret 0\nend\n' > build/s8.oli
set +e
( cd .. && genesis/build/olic < genesis/build/s8.oli > genesis/build/s8.elf 2> genesis/build/s8.err )
st=$?
set -e
[ "$st" = 1 ] || fail "back end: an eight-word syscall exited $st, want a refusal"
grep -q 'E0900' build/s8.err || fail "back end: an eight-word syscall must report E0900"
[ ! -s build/s8.elf ] || fail "back end: an eight-word syscall wrote a file"
# A `bytes` line lists bytes and nothing else (design 0023): 256 is refused.
printf 'module b9\nproc start -> s32\n    entry\n    permit cpu.asm\n    machine x64\n        bytes 0x90, 256\n    end\n    ret 0\nend\n' > build/b9.oli
set +e
( cd .. && genesis/build/olic < genesis/build/b9.oli > genesis/build/b9.elf 2> genesis/build/b9.err )
st=$?
set -e
[ "$st" = 1 ] || fail "back end: bytes 256 exited $st, want a refusal"
grep -q 'E0900' build/b9.err || fail "back end: a value above 255 in a bytes line must report E0900"
[ ! -s build/b9.elf ] || fail "back end: a refused bytes line still wrote a file"
grep -q 'E0900' build/nolower.err || fail "back end: an unlowered construct must report E0900"
[ ! -s build/nolower.elf ] || fail "back end: a refused program still wrote a binary"
echo "ok: a construct the back end cannot lower is E0900 and writes no file"
# A profile naming a target the backend does not generate for is refused:
# never x86-64 code under another architecture's name.
printf '[target]\narch = "aarch64"\nos = "none"\n' > build/arm.oli-target
printf -- '-- target: freestanding\n-- profile: genesis/build/arm.oli-target\nmodule arm\nproc start -> never\n    entry\n    calls none\n    permit cpu.halt\n    loop\n        cpu.halt()\n    end\nend\n' > build/arm.oli
set +e
( cd .. && genesis/build/olic < genesis/build/arm.oli > genesis/build/arm.elf 2> genesis/build/arm.err )
st=$?
set -e
[ "$st" = 1 ] || fail "profile: arch aarch64 exited $st, want a refusal"
grep -q 'not a target of this compiler' build/arm.err || fail "profile: arch aarch64 must be refused by name"
[ ! -s build/arm.elf ] || fail "profile: a refused target still wrote a file"
# tests/run/switch.oli: a calls-none procedure's machine block is the whole
# procedure - the lowering saves nothing through rbp, which it does not own.
objdump -d --no-show-raw-insn build/switch.elf | awk '/<switch_test.switch_to>:/,/^$/' > build/switch.dis
[ "$(sed -n 2p build/switch.dis | grep -c 'push   %rbp')" = 1 ] || fail "switch: switch_to must begin with the block's own push rbp"
! grep -q '(%rbp)' build/switch.dis || fail "switch: a calls-none procedure must not store through rbp"
echo "ok: an unknown target architecture is refused, and a calls-none context switch is exactly its block (switch.elf runs five round trips between two stacks)"
echo "genesis: layer 5 (olic back end - OIR, x86-64, ELF) passed"

# --- layer 6: self-hosting (G4, design 0022) ---
# The compiler built by oli1 (stage 1) compiles its own source into stage 2;
# stage 2 compiles the same source into stage 3; the two must be the same
# bytes. build/self.oli is the concatenation layer 4 analysed.
[ -s build/self.oli ] || fail "self-hosting: build/self.oli is missing"
( cd .. && genesis/build/olic < genesis/build/self.oli > genesis/build/stage2 2> genesis/build/stage2.err ) \
    || fail "self-hosting: olic could not compile itself: $(head -1 build/stage2.err)"
chmod +x build/stage2
[ "$(wc -c < build/stage2)" -gt 100000 ] || fail "self-hosting: stage2 is implausibly small"
( cd .. && timeout 600 genesis/build/stage2 < genesis/build/self.oli > genesis/build/stage3 2> genesis/build/stage3.err ) \
    || fail "self-hosting: the self-compiled olic could not compile olic: $(head -1 build/stage3.err)"
cmp build/stage2 build/stage3 || fail "self-hosting: stage2 and stage3 differ"
echo "ok: G4 - olic compiles its own source, and the compiler that produces compiles it again to the same bytes (stage2 == stage3, $(wc -c < build/stage2) bytes)"

# The self-compiled compiler agrees with the genesis-built one on every
# program of the corpus, byte for byte, diagnostics included.
for f in ../examples/hello.oli ../tests/run/*.oli ../tests/run/trap/*.oli; do
    n=$(basename "$f" .oli)
    ( cd .. && genesis/build/olic < "${f#../}" > genesis/build/s1_$n.elf 2> genesis/build/s1_$n.err )
    st1=$?
    set +e
    ( cd .. && timeout 60 genesis/build/stage2 < "${f#../}" > genesis/build/s2_$n.elf 2> genesis/build/s2_$n.err )
    st2=$?
    set -e
    [ "$st1" = "$st2" ] || fail "self-hosting: stage1 exited $st1 and stage2 $st2 on $f"
    cmp build/s1_$n.elf build/s2_$n.elf || fail "self-hosting: stage1 and stage2 compile $f differently"
    cmp build/s1_$n.err build/s2_$n.err || fail "self-hosting: stage1 and stage2 report $f differently"
done
for f in ../tests/sema/err/*.oli; do
    n=$(basename "$f" .oli)
    set +e
    ( cd .. && genesis/build/olic < "${f#../}" > /dev/null 2> genesis/build/s1_$n.err ); st1=$?
    ( cd .. && timeout 60 genesis/build/stage2 < "${f#../}" > /dev/null 2> genesis/build/s2_$n.err ); st2=$?
    set -e
    [ "$st1" = "$st2" ] || fail "self-hosting: stage1 exited $st1 and stage2 $st2 on $f"
    cmp build/s1_$n.err build/s2_$n.err || fail "self-hosting: stage1 and stage2 report $f differently"
done
echo "ok: the self-compiled olic compiles every run and trap fixture to the same bytes as the genesis-built one, and reports every negative fixture the same way"
echo "genesis: layer 6 (self-hosting: stage2 == stage3) passed"
