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
echo "genesis: layer 3 (oli-core compiler, steps 0-6d) passed"

# --- layer 4: olic (G4), written in oli-core and compiled by oli1 ---
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
for pair in "hello ../examples/hello.oli" "packet_demo ../examples/packet_demo.oli" "statements ../tests/parse/ok/statements.oli" "kernel_sketch ../tests/parse/ok/kernel_sketch.oli"; do
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
cat ../compiler/io.oli ../compiler/lex.oli ../compiler/diag.oli ../compiler/ast.oli ../compiler/parse.oli ../compiler/load.oli ../compiler/items.oli ../compiler/sema.oli ../compiler/check.oli ../compiler/show_items.oli > build/show_items.oli
./build/oli1.bin < build/show_items.oli > build/show_items || fail "oli1 could not compile compiler/ (show_items)"
chmod +x build/show_items
./build/oli1.bin < build/show_items.oli > build/show_items.again
cmp build/show_items build/show_items.again || fail "show_items build is not deterministic"
# The item section must equal the head of the semantic snapshot, and it is read
# from the repository root because imports are resolved under lib/.
for pair in "hello examples/hello.oli" "packet_demo examples/packet_demo.oli" "freestanding tests/sema/ok/freestanding.oli"; do
    set -- $pair
    ( cd .. && genesis/build/show_items < "$2" > genesis/build/$1.out 2> genesis/build/items.err ) || fail "items: diagnostics for $2: $(cat build/items.err)"
    sed -n '/^  (proc /q;p' build/$1.out > build/$1.items
    n=$(wc -l < build/$1.items)
    head -n "$n" ../tests/snapshots/$1.sema > build/$1.want
    cmp build/$1.want build/$1.items || fail "items: $1 differs from the head of tests/snapshots/$1.sema"
    [ "$n" -ge 5 ] || fail "items: $1 produced only $n lines"
done
echo "ok: olic item collection matches the layouts, choices, constants and statics of every tests/snapshots/*.sema"
( cd .. && genesis/build/show_items < tests/parse/ok/kernel_sketch.oli > genesis/build/ks.items 2>/dev/null ) || true
grep -q '(layout GdtPointer#1 size=10 align=1 (limit u16 @0) (base addr u64 @2))' build/ks.items || fail "items: packed layout"
grep -q '(layout Multiboot2Header#0 size=24 align=8 ' build/ks.items || fail "items: explicit layout alignment"
( cd .. && genesis/build/show_items < tests/parse/ok/statements.oli > genesis/build/st.items 2>/dev/null ) || true
grep -q '(choice Shape#0 size=20 align=4 tag=1 payload@4 (dot) (line (a Point @4) (b Point @12)))' build/st.items || fail "items: choice with a layout payload"
for f in ../tests/sema/ok/*.oli ../lib/*.oli ../lib/*/*.oli ../compiler/*.oli; do
    ( cd .. && genesis/build/show_items < "${f#../}" > genesis/build/items.out 2> genesis/build/items.err ) || fail "items: diagnostics for $f: $(cat build/items.err)"
    head -c 9 build/items.out | grep -q '^(program' || fail "items: $f produced no program"
done
echo "ok: olic resolves packed/aligned layouts, nested payloads and every library module it imports"
# Procedure signatures and parameter locals, compared against the same lines of
# the semantic snapshot (bodies and inferred locals are the next stage).
for pair in "hello examples/hello.oli" "packet_demo examples/packet_demo.oli" "freestanding tests/sema/ok/freestanding.oli"; do
    set -- $pair
    ( cd .. && genesis/build/show_items < "$2" 2>/dev/null ) | grep -E '^  \(proc |^    \(local ' > build/$1.sig
    grep -E '^  \(proc |^    \(local ' ../tests/snapshots/$1.sema > build/$1.sigwant
    cmp build/$1.sigwant build/$1.sig || fail "signatures: $1 differs from tests/snapshots/$1.sema"
    [ -s build/$1.sig ] || fail "signatures: $1 produced nothing"
done
# Semantic checks that need no type graph: capabilities, unimplemented
# features, constant cycles and ranges, recursive layouts.
for f in ../tests/sema/err/items.oli ../tests/sema/err/permits.oli ../tests/sema/err/not_implemented.oli; do
    want=$(grep -- '-- expect: ' "$f" | sed 's/-- expect: //' | sort)
    set +e; ( cd .. && genesis/build/show_items < "${f#../}" > genesis/build/chk.out 2> genesis/build/chk.err ); st=$?; set -e
    got=$(awk '/^error\[|^warning\[/ {code=substr($1,index($1,"[")+1,5)} /^ --> stdin:/ {split($2,a,":"); print code " @ " a[2] ":" a[3]}' build/chk.err | sort)
    [ "$got" = "$want" ] || fail "checks: $f expected [$want] got [$got]"
    [ "$st" = 1 ] || fail "checks: $f exit $st"
done
( cd .. && genesis/build/show_items < tests/parse/ok/kernel_sketch.oli > /dev/null 2> genesis/build/chk.err ) || true
grep -q 'E0401' build/chk.err || fail "checks: kernel_sketch must report the missing memory.raw permit"
grep -q 'E0900' build/chk.err || fail "checks: kernel_sketch must report the V1 features it uses"
echo "ok: olic reports missing capabilities (E0401), unimplemented features (E0900), constant cycles (E0106), constant range (E0212) and recursive layouts (E0204) exactly where tests/sema/err expects them"
echo "ok: olic prints every procedure signature and every local - parameters, places, bindings, zones and case patterns with inferred types - exactly as tests/snapshots/*.sema"
echo "genesis: layer 4 (olic lexer, parser and item collection) passed"
