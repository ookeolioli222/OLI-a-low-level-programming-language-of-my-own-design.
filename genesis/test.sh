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
grep -q 'E0900' build/chk.err || fail "checks: kernel_sketch must report the V1 features it uses"
for f in ../tests/sema/ok/*.oli ../examples/*.oli ../lib/*.oli ../lib/*/*.oli; do
    ( cd .. && genesis/build/show_sema < "${f#../}" > /dev/null 2> genesis/build/chk.err ) || fail "checks: $f reported $(head -1 build/chk.err)"
done
echo "ok: olic reports exactly the diagnostics of all sixteen tests/sema/err fixtures - capabilities, E0900, constants, layouts, scopes, definite assignment, reachability, failures, exhaustiveness, read-only places, region escapes, literal types, literal and pattern fields, address spaces and implicit narrowing - and none on any positive fixture"
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
for pair in "hello examples/hello.oli" "control tests/run/control.oli" "values tests/run/values.oli" "memory tests/run/memory.oli" "layouts tests/run/layouts.oli" "fallible tests/run/fallible.oli" "statics tests/run/statics.oli" "frames tests/run/frames.oli" "saturate tests/run/saturate.oli" "zones tests/run/zones.oli" "cse tests/run/cse.oli" "choice tests/run/choice.oli" "machine tests/run/machine.oli" "freestanding tests/run/freestanding.oli" "aggregates tests/run/aggregates.oli" "records tests/run/records.oli"; do
    set -- $pair
    ( cd .. && genesis/build/show_oir < "$2" > genesis/build/$1.oir 2> genesis/build/oir.err ) || fail "oir: diagnostics for $2: $(cat build/oir.err)"
    cmp build/$1.oir ../tests/snapshots/$1.oir || fail "oir: $1 differs from tests/snapshots/$1.oir"
    ( cd .. && genesis/build/show_ssa < "$2" > genesis/build/$1.ssa 2> genesis/build/ssa.err ) || fail "ssa: diagnostics for $2: $(cat build/ssa.err)"
    cmp build/$1.ssa ../tests/snapshots/$1.ssa || fail "ssa: $1 differs from tests/snapshots/$1.ssa"
    ( cd .. && genesis/build/show_opt < "$2" > genesis/build/$1.opt 2> genesis/build/opt.err ) || fail "opt: diagnostics for $2: $(cat build/opt.err)"
    cmp build/$1.opt ../tests/snapshots/$1.opt || fail "opt: $1 differs from tests/snapshots/$1.opt"
done
echo "ok: olic cuts every block of examples/hello.oli and tests/run/{control,values,memory,layouts,fallible,statics,frames,saturate,zones,cse,choice,machine,freestanding,aggregates,records}.oli exactly as tests/snapshots/*.oir (--show-oir), and the verifier accepts each"

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
for n in hello control values memory layouts fallible statics frames saturate zones cse choice machine freestanding aggregates records; do
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
echo "ok: olic compiles and runs every tests/run fixture - arithmetic in all four modes, control flow, procedures with register and stack arguments, constants, conversions, zones from every source with try_bytes and a release on every exit edge, views, each, layouts, refs, fallible results with else and case, choices as failures with variant patterns, case over a bool, an integer or a plain choice with its fields, each over a range, layouts by value in places, literals, parameters, arguments and results, machine x64 blocks with in, out, clobber, local labels, a callee-saved register kept across a call and a system call by hand, a freestanding program with its own entry and stack, statics, arrays in frames, raw access and stdout"
# The image of a program with statics has two loadable segments, the second
# read+write on the page after the first; hello.elf still has one.
[ "$(od -An -tu2 -j56 -N2 build/statics.elf | tr -d ' ')" = 2 ] || fail "elf: statics.elf should have two program headers"
[ "$(od -An -tu2 -j56 -N2 build/hello.elf | tr -d ' ')" = 1 ] || fail "elf: hello.elf should have one program header"
# The self-test of arith.oli is decided at compile time, and the messages of
# the checks that were proved away are not in its image.
[ "$(wc -c < build/arith.elf)" -lt 200 ] || fail "opt: arith.elf still carries the messages of checks that were proved away"

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

# M3 (FREESTANDING.md 8): the kernel image. No emulator runs here, so the
# image is checked structurally: linked at the address its `-- load:` line
# names, the Multiboot2 header (a static layout with an initialiser, placed
# in `.text.boot`) right after the ELF and program headers with the magic,
# length and checksum the loader will verify, port I/O and cpuid in its
# blocks, and not one system call in any procedure.
( cd .. && genesis/build/olic < examples/kernel.oli > genesis/build/kernel.elf 2> genesis/build/kernel.err ) || fail "kernel: olic could not compile examples/kernel.oli: $(head -1 build/kernel.err)"
readelf -h build/kernel.elf | grep -q 'Entry point address: *0x1001' || fail "kernel: the entry is not in the segment loaded at 0x100000"
readelf -l build/kernel.elf | grep -q 'LOAD .*0x0000000000100000 0x0000000000100000' || fail "kernel: the first segment is not loaded at 0x100000"
[ "$(od -An -tx1 -j176 -N24 build/kernel.elf | tr -d ' \n')" = "d65052e800000000180000001200ad17000000000800000000" ] \
    || [ "$(od -An -tx1 -j176 -N24 build/kernel.elf | tr -d ' \n')" = "d65052e8000000001800000012afad170000000008000000" ] \
    || fail "kernel: the Multiboot2 header is not at offset 176: $(od -An -tx1 -j176 -N24 build/kernel.elf | tr -d '\n')"
( cd .. && genesis/build/show_asm < examples/kernel.oli > genesis/build/kernel.asm 2>/dev/null ) || fail "kernel: show_asm"
grep -q '^    ee)' build/kernel.asm || fail "kernel: the COM1 write must be one out dx, al"
grep -q '0f a2' build/kernel.asm || fail "kernel: cpuid must be in its block"
( cd .. && genesis/build/explain < examples/kernel.oli > genesis/build/kernel.explain 2>/dev/null ) || fail "kernel: explain"
[ "$(grep -c 'syscalls=0' build/kernel.explain)" = "$(grep -c '(proc ' build/kernel.explain)" ] || fail "kernel: a procedure of the kernel makes a system call"
echo "ok: examples/kernel.oli is an ELF64 loaded at 0x100000 with its Multiboot2 header at offset 176, port I/O and cpuid in its machine blocks and no system call anywhere (run it: qemu-system-x86_64 -kernel kernel.elf -serial stdio)"

# A program with no trap site carries no trap routine.
printf 'module notrap\nproc start -> s32\n    entry\n    ret 0\nend\n' > build/notrap.oli
( cd .. && genesis/build/olic < genesis/build/notrap.oli > genesis/build/notrap.elf ) || fail "olic could not compile notrap.oli"
[ "$(wc -c < build/notrap.elf)" -lt 200 ] || fail "notrap.elf is $(wc -c < build/notrap.elf) bytes: the trap routine was emitted anyway"
echo "ok: the trap routine and its messages are in the binary only when a trap site is"

# Anything the back end cannot lower is E0900, never approximated: here a
# `choice` wider than a word, whose image would need memory (ABI.md 3).
printf 'module z\nchoice Wide\n    one { a : u64, b : u64 }\nend\nproc f(x : u64) -> u64 or Wide\n    if x > 1 then fail one { a: x, b: x }\n    ret x\nend\nproc start -> s32\n    entry\n    v := f(1) else ret 3\n    if v != 1\n        ret 1\n    end\n    ret 0\nend\n' > build/nolower.oli
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
grep -q 'E0900' build/nolower.err || fail "back end: an unlowered construct must report E0900"
[ ! -s build/nolower.elf ] || fail "back end: a refused program still wrote a binary"
echo "ok: a construct the back end cannot lower is E0900 and writes no file"
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
