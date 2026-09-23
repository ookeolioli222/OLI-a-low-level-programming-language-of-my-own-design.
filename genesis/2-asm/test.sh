#!/bin/sh
# G2 encoder tests; source and expected byte fixtures are independent.
set -eu
cd "$(dirname "$0")/.."
fail() { echo "FAIL: asm: $*" >&2; exit 1; }
asm=./build/asm.bin
$asm < 2-asm/tests/registers.oli > build/registers.elf
./build/hex0.bin < 2-asm/tests/registers.hex > build/registers.want
tail -c +121 build/registers.elf > build/registers.got
cmp build/registers.want build/registers.got || fail 'register encodings'
$asm < 2-asm/tests/registers.oli > build/registers.again
cmp build/registers.elf build/registers.again || fail determinism
echo 'ok: exact bytes for all 16 r64 registers and supported instruction forms'
$asm < 2-asm/tests/mov32.oli > build/mov32.elf
./build/hex0.bin < 2-asm/tests/mov32.hex > build/mov32.want
tail -c +121 build/mov32.elf > build/mov32.got
cmp build/mov32.want build/mov32.got || fail '32-bit MOV encodings'
echo 'ok: exact bytes for all 16 r32 registers, immediates and memory MOV'
$asm < 2-asm/tests/memory.oli > build/memory.elf
./build/hex0.bin < 2-asm/tests/memory.hex > build/memory.want
tail -c +121 build/memory.elf > build/memory.got
cmp build/memory.want build/memory.got || fail 'memory encodings'
echo 'ok: exact ModRM/SIB/REX bytes, addressing edge cases and displacement boundaries'
$asm < 2-asm/tests/branches.oli > build/branches.elf
./build/hex0.bin < 2-asm/tests/branches.hex > build/branches.want
tail -c +121 build/branches.elf > build/branches.got
cmp build/branches.want build/branches.got || fail 'symbol encodings'
echo 'ok: forward/backward rel32, absolute procedure addresses and local label scopes'
$asm < 2-asm/tests/conditions.oli > build/conditions.elf
./build/hex0.bin < 2-asm/tests/conditions.hex > build/conditions.want
tail -c +121 build/conditions.elf > build/conditions.got
cmp build/conditions.want build/conditions.got || fail 'condition codes'
$asm < 2-asm/tests/data.oli > build/data.elf
./build/hex0.bin < 2-asm/tests/data.hex > build/data.want
tail -c +121 build/data.elf > build/data.got
cmp build/data.want build/data.got || fail 'data encodings'
$asm < ../examples/genesis/hello.oli > build/hello-machine.elf
chmod +x build/hello-machine.elf
./build/hello-machine.elf > build/hello-machine.got
printf 'Hello Oli--\n' > build/hello-machine.want
cmp build/hello-machine.want build/hello-machine.got || fail 'hello stdout'
echo 'ok: read-only data, string escapes, absolute address data and Hello Oli-- stdout'
$asm < 2-asm/tests/arithmetic.oli > build/arithmetic.elf
chmod +x build/arithmetic.elf
status=0
./build/arithmetic.elf || status=$?
[ "$status" = 42 ] || fail "arithmetic exit $status"
echo 'ok: real mnemonic program executes; entry selects the second procedure'
for fixture in signed memory_run branches_run mov32_run memory_extra_run; do
    $asm < "2-asm/tests/$fixture.oli" > "build/$fixture.elf"
    chmod +x "build/$fixture.elf"
    status=0
    "./build/$fixture.elf" || status=$?
    [ "$status" = 42 ] || fail "$fixture exit $status"
done
echo 'ok: signed division, multiplication, shifts, stack and ALU execute'

wrap() {
    printf 'module test\nproc main\n calls none\n entry\n machine x64\n'
    printf '%s\n' "$1"
    printf ' end\nend\n'
}
reject() {
    want=$1
    status=0
    $asm < build/reject.oli > build/reject.elf 2> build/reject.err || status=$?
    [ "$status" = "$want" ] || fail "expected exit $want, got $status: $(cat build/reject.oli)"
    [ ! -s build/reject.elf ] || fail 'error emitted a partial executable'
    grep -q '^asm:' build/reject.err || fail 'missing diagnostic'
}
for instruction in 'unknown rax' 'mov ax, 42' 'mov r16, 1' 'mov rax rbx' \
    'mov rax,' 'mov rax, 0x' 'mov rax, -' 'mov rax, 12z' 'mov rax, 1 extra' \
    'syscall rax' 'ret extra' 'shl rax, rcx' 'shl rax, clx' 'pop 42' \
    'bytes' 'bytes a' 'bytes gg' 'bytes abcd' 'bytes ab z0' 'bytesx ff' \
    'mov rax, [rbx+]' 'mov [rax], [rbx]' \
    'mov rax, [rbx+rsp*2]' 'mov rax, [rbx+rcx*3]' \
    'mov rax, [rbx' 'mov rax, [rbx++rcx]' 'mov rax, [rbx+rax+rcx]' \
    'lea rax, rbx' 'lea [rax], rbx' 'imul [rax], rbx' \
    'mov eax, rax' 'mov eax, ax' 'mov eax, [ebx]' 'mov eax, unknown'; do
    wrap "$instruction" > build/reject.oli
    reject 1
done
for instruction in 'call unknown' 'jmp .missing' 'mov rax, missing' 'je missing'; do
    wrap "$instruction" > build/reject.oli
    reject 2
    grep -q 'undefined symbol' build/reject.err || fail 'missing symbol diagnostic'
done
for instruction in '.same:
.same:' '.:' '.name' '.1bad:'; do
    wrap "$instruction" > build/reject.oli
    reject 1
done
for instruction in 'mov rax, 18446744073709551616' 'mov rax, 0x10000000000000000' \
    'mov rax, -9223372036854775809' 'add rax, 2147483648' 'sub rax, -2147483649' \
    'test rax, 0xffffffff' 'push 2147483648' 'imul rax, rbx, -2147483649' \
    'shl rax, 256' 'shr rax, -1' 'mov rax, [rbx+2147483648]' \
    'mov rax, [rbx-2147483649]' 'mov [rbx], 2147483648' \
    'add rax, 18446744073709551615' 'push 0xffffffffffffffff' \
    'imul rax, rbx, 0xffffffffffffffff' 'mov rax, [rbx+0xffffffffffffffff]' \
    'mov eax, 4294967296' 'mov r15d, -2147483649' 'mov eax, 0xffffffffffffffff'; do
    wrap "$instruction" > build/reject.oli
    reject 4
done
for source in '' 'module test' 'module test
proc main
calls none
machine x64
ret
end
end'; do
    printf '%s\n' "$source" > build/reject.oli
    case "$source" in '') reject 1 ;; *) reject 5 ;; esac
done
wrap 'entry' > build/reject.oli
reject 1
wrap 'ret' | sed '/ entry/a\ entry' > build/reject.oli
reject 5
for source in 'ret' 'module test
ret' 'module test
proc main
entry
machine x64
ret
end
end' 'module test
proc main
calls none
entry
machine x64
ret
end' 'module test
proc main
calls sysv' 'module 1bad' 'module x..y' 'module x.'; do
    printf '%s\n' "$source" > build/reject.oli
    reject 1
done
echo 'ok: malformed source, unsupported operands, overflow and entry errors reject without output'

# CRLF, tabs, comments, no trailing newline, split input writes.
wrap 'mov rdi, 42
mov rax, 60
syscall' > build/format.oli
$asm < build/format.oli > build/format.want
sed 's/$/\r/;s/^ /\t/' build/format.oli > build/format-crlf.oli
$asm < build/format-crlf.oli > build/format.got
cmp build/format.want build/format.got || fail CRLF
size=$(wc -c < build/format.oli)
head -c "$((size - 1))" build/format.oli | $asm > build/format.got
cmp build/format.want build/format.got || fail 'final newline'
{ printf 'module test\n'; sleep 0.05; tail -n +2 build/format.oli; } | $asm > build/format.got
cmp build/format.want build/format.got || fail 'short reads'
# Input capacity is deliberate and errors instead of overwriting output.
head -c 1048576 /dev/zero > build/reject.oli
reject 6
status=0
$asm < build/format.oli > /dev/full 2> build/reject.err || status=$?
[ "$status" = 7 ] || fail 'write failure not reported'
echo 'ok: CRLF, tabs, EOF, short reads, input limit and write failures'

# More emitted bytes than input bytes: validate output capacity separately.
{
    printf 'module test\nproc main\n calls none\n entry\n machine x64\n'
    yes 'mov r8,0' | head -n 104846
    printf ' end\nend\n'
} > build/reject.oli
reject 6
status=0
$asm < . > build/reject.elf 2> build/reject.err || status=$?
[ "$status" = 7 ] || fail 'read failure not reported'
[ ! -s build/reject.elf ] || fail 'read failure emitted output'
echo 'ok: output capacity and read failures'

# Data syntax and bounds, including errors encountered only in pass two.
for instruction in 'byte 256' 'byte -1' 'word 18446744073709551616'; do
    printf 'module test\nrodata value\n%s\nend\n' "$instruction" > build/reject.oli
    reject 4
done
for instruction in 'ascii "unterminated' 'ascii "\q"' 'ascii "\x0g"' \
    'byte 1,' 'addr .local' 'res' 'ret'; do
    printf 'module test\nrodata value\n%s\nend\n' "$instruction" > build/reject.oli
    reject 1
done
{
    printf 'module test\nrodata value\naddr missing\nend\n'
    tail -n +2 build/format.oli
} > build/reject.oli
reject 2
grep -q 'undefined symbol missing' build/reject.err || fail 'undefined name not reported'
{
    cat build/format.oli
    printf 'proc main\ncalls none\nmachine x64\nret\nend\nend\n'
} > build/reject.oli
reject 1
{
    cat build/format.oli
    i=0
    while [ "$i" -lt 819 ]; do
        printf 'rodata item%s\nres 0\nend\n' "$i"
        i=$((i + 1))
    done
} > build/reject.oli
reject 6
echo 'ok: data validation, duplicate/undefined symbols and symbol-table capacity'

# Header fields must describe the actual output, not a stale hand-written size.
for elf in build/asm.bin build/registers.elf build/hello-machine.elf; do
    size=$(wc -c < "$elf")
    filesz=$(od -A n -j 96 -N 8 -t u8 "$elf" | tr -d ' \n')
    [ "$filesz" = "$size" ] || fail "stale ELF file size in $elf"
done
flags=$(od -A n -j 68 -N 4 -t u4 build/hello-machine.elf | tr -d ' \n')
[ "$flags" = 5 ] || fail 'output segment must be read/execute only'
echo 'ok: ELF file sizes and output segment permissions'
