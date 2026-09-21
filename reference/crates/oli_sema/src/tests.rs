#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

use crate::{analyze, FsModuleSource, Options, Target};
use oli_diag::{Diagnostics, SourceFile, SourceMap};
use std::path::PathBuf;

fn lib_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../lib")
}

/// Analyzes `src` as `t.oli`; returns diagnostic codes in source order.
fn run(src: &str, target: Target) -> (crate::Program, Vec<String>) {
    let file = SourceFile::new("t.oli", src);
    let source = FsModuleSource {
        root_dir: PathBuf::from("."),
        lib_dirs: vec![lib_dir()],
    };
    let mut sources = SourceMap::new();
    let mut diags = Diagnostics::new();
    let program = analyze(file, &source, &Options { target }, &mut sources, &mut diags);
    let codes = diags
        .sorted()
        .into_iter()
        .map(|d| d.code.to_string())
        .collect();
    (program, codes)
}

fn codes(src: &str) -> Vec<String> {
    run(src, Target::Hosted).1
}

fn ok(src: &str) {
    let (_, c) = run(src, Target::Hosted);
    assert!(
        c.is_empty(),
        "expected no diagnostics, got {c:?}\n---\n{src}"
    );
}

/// Wraps statements in `proc main -> s32 ... ret 0 end`.
fn main_body(stmts: &str) -> String {
    format!("proc main -> s32\n    entry\n{stmts}\n    ret 0\nend\n")
}

fn body_codes(stmts: &str) -> Vec<String> {
    codes(&main_body(stmts))
}

#[test]
fn hello_and_packet_demo_are_valid() {
    let hello = std::fs::read_to_string(lib_dir().join("../examples/hello.oli")).unwrap();
    ok(&hello);
    let demo = std::fs::read_to_string(lib_dir().join("../examples/packet_demo.oli")).unwrap();
    ok(&demo);
}

#[test]
fn hosted_needs_entry() {
    assert_eq!(codes("proc f\nend\n"), vec!["E0600"]);
    assert_eq!(codes("proc main -> s32\n    ret 0\nend\n"), vec!["E0600"]);
    assert_eq!(
        codes("proc f(_a : u8) -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0600"]
    );
    assert_eq!(codes("proc f\n    entry\nend\n"), vec!["E0600"]);
    assert_eq!(
        codes(
            "proc a -> s32\n    entry\n    ret 0\nend\nproc b -> s32\n    entry\n    ret 0\nend\n"
        ),
        vec!["E0602"]
    );
    assert_eq!(
        codes("proc a -> s32\n    entry\n    traps\n    ret 0\nend\n"),
        vec!["E0601"]
    );
}

#[test]
fn names_and_scopes() {
    assert_eq!(body_codes("    _x := y"), vec!["E0100"]);
    assert_eq!(
        body_codes("    x : u8 <- 1\n    x : u8 <- 2\n    _y := x"),
        vec!["W0002", "E0101"]
    );
    assert_eq!(
        body_codes("    main : u8 <- 1\n    _y := main"),
        vec!["E0101"]
    );
    assert_eq!(
        body_codes("    mem : u8 <- 1\n    _y := mem"),
        vec!["E0101"]
    );
    assert_eq!(
        codes("proc f\nend\nproc f\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0102"]
    );
    assert_eq!(
        codes("proc os\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0103"]
    );
    assert_eq!(
        codes("import nope.missing\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0104"]
    );
    assert_eq!(
        codes("import std.os\nproc main -> s32\n    entry\n    _x := os.NOPE\n    ret 0\nend\n"),
        vec!["E0105"]
    );
    assert_eq!(body_codes("    x : u8 <- 1\n    if true\n        x2 : u8 <- x\n        _z := x2\n    end\n    y : u8 <- 1\n    _w := y"), Vec::<String>::new());
}

#[test]
fn unused_warnings() {
    assert_eq!(body_codes("    x := u8(1)"), vec!["W0002"]);
    assert_eq!(body_codes("    _x := u8(1)"), Vec::<String>::new());
    assert_eq!(
        codes("proc f(a : u8, _b : u8)\nend\nproc main -> s32\n    entry\n    f(1, 2)\n    ret 0\nend\n"),
        vec!["W0002"]
    );
}

#[test]
fn integer_literals_need_context() {
    assert_eq!(body_codes("    x := 10\n    _y := x"), vec!["E0201"]);
    assert_eq!(body_codes("    x := 1 + 2\n    _y := x"), vec!["E0201"]);
    assert_eq!(
        body_codes("    x : u32 := 3 * 4\n    _y : u32 := x"),
        Vec::<String>::new()
    );
    assert_eq!(
        body_codes("    x : u32 := 3\n    _y : s32 := x"),
        vec!["E0202"]
    );
    assert_eq!(body_codes("    _x : u8 := 300"), vec!["E0202"]);
    assert_eq!(
        body_codes("    x : s8 := -128\n    y : u8 := 255\n    _z := x\n    _w := y"),
        Vec::<String>::new()
    );
    assert_eq!(body_codes("    _x : u32 := 40000000000"), vec!["E0202"]);
    assert_eq!(
        body_codes("    each _i in 0..10\n    end"),
        Vec::<String>::new()
    );
    assert_eq!(
        body_codes("    n : u16 := 5\n    each i in 0..n\n        _y : u16 := i\n    end"),
        Vec::<String>::new()
    );
    // A shift count beyond the width is masked (defined), never an error.
    assert_eq!(body_codes("    _x : u32 := 1 << 40"), Vec::<String>::new());
}

#[test]
fn arithmetic_typing() {
    ok(&main_body("    a : u8 := 1\n    b : u32 := 2\n    _c : u32 := a + b\n    _d : u32 := b * 2\n    _e := wrap(b - 3)\n    _f : u64 := sat(u64(b) + 1)"));
    assert_eq!(
        body_codes("    a : u8 := 1\n    b : s32 := 2\n    _c := a + b"),
        vec!["E0200"]
    );
    assert_eq!(
        body_codes("    a : u8 := 1\n    b : u32 := 2\n    _c := a & b"),
        vec!["E0200"]
    );
    assert_eq!(body_codes("    a : u32 := 1\n    _c := -a"), vec!["E0200"]);
    assert_eq!(
        body_codes("    a : u32 := 1\n    _c := a < true"),
        vec!["E0200"]
    );
    assert_eq!(
        body_codes("    a : s8 := 1\n    _c := a << 3\n    _d := a >> u8(1)"),
        Vec::<String>::new()
    );
    assert_eq!(
        body_codes("    a : u32 := 1\n    b : s32 := 1\n    _c := a << b"),
        vec!["E0200"]
    );
    assert_eq!(
        body_codes("    r := checked(u8(200) + u8(100)) else 0\n    _q : u8 := r"),
        Vec::<String>::new()
    );
    assert_eq!(body_codes("    _r := wrap(1 + 2)"), vec!["E0201"]);
    assert_eq!(body_codes("    if 3 < 4\n    end"), Vec::<String>::new());
}

#[test]
fn address_spaces() {
    ok(&main_body(
        "    p := physaddr(0x1000)\n    _q := p + 8\n    _n := uword(p)",
    ));
    assert_eq!(
        body_codes("    p := physaddr(0x1000)\n    a := addr u8 (0x2000)\n    _q := p + a"),
        vec!["E0401", "E0203"]
    );
    assert_eq!(codes("proc main -> s32\n    entry\npermit memory.raw\n    p := physaddr(0x1000)\n    a := addr u8 (0x2000)\n    _q := p + a\n    ret 0\nend\n"), vec!["E0203"]);
    assert_eq!(body_codes("    _a := addr u8 (0x2000)"), vec!["E0401"]);
    assert_eq!(
        body_codes("    a : u8 <- 1\n    _p := addr a\n    _n := uword(addr a)"),
        vec!["E0401"]
    );
    assert_eq!(
        body_codes("    p := physaddr(1)\n    _q := addr u8 (p)"),
        vec!["E0200"]
    );
}

#[test]
fn places_bindings_and_stores() {
    assert_eq!(body_codes("    x := u8(1)\n    x <- 2"), vec!["E0110"]);
    assert_eq!(body_codes("    s := \"abc\"\n    s[0] <- 1"), vec!["E0111"]);
    assert_eq!(body_codes("    x : u8\n    _y := x"), vec!["E0220"]);
    assert_eq!(
        body_codes("    x : u8\n    if true\n        x <- 1\n    end\n    _y := x"),
        vec!["E0220"]
    );
    assert_eq!(body_codes("    x : u8\n    if true\n        x <- 1\n    else\n        x <- 2\n    end\n    _y := x"), Vec::<String>::new());
    assert_eq!(
        body_codes("    x : u8\n    while true\n        x <- 1\n    end\n    _y := x"),
        vec!["E0220"]
    );
    assert_eq!(
        body_codes("    x : u8\n    loop\n        x <- 1\n        break\n    end\n    _y := x"),
        Vec::<String>::new()
    );
    assert_eq!(
        body_codes("    buf : [4]u8\n    buf[0] <- 1\n    _y := buf[0]"),
        Vec::<String>::new()
    );
    assert_eq!(body_codes("    f() <- 1"), vec!["E0017"]);
    assert_eq!(
        body_codes("    x : u8\n    x <- 1\n    _y := x"),
        Vec::<String>::new()
    );
}

#[test]
fn control_flow() {
    assert_eq!(
        codes("proc f -> u8\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0230"]
    );
    assert_eq!(codes("proc f -> u8\n    if true\n        ret 1\n    end\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"), vec!["E0230"]);
    assert_eq!(codes("proc f -> u8\n    if true\n        ret 1\n    else\n        ret 2\n    end\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"), Vec::<String>::new());
    assert_eq!(
        codes(
            "proc f -> u8\n    loop\n    end\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        codes("proc f -> never\n    ret\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0200"]
    );
    assert_eq!(
        body_codes("    ret 1\n    x : u8 <- 2\n    _y := x"),
        vec!["E0231"]
    );
    assert_eq!(body_codes("    break"), vec!["E0232", "E0231"]);
    assert_eq!(
        body_codes("    loop\n        break\n        continue\n    end"),
        vec!["E0231"]
    );
    assert_eq!(
        codes("proc f\n    ret 1\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0200"]
    );
    assert_eq!(
        codes("proc f -> u8\n    ret\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0200"]
    );
    assert_eq!(body_codes("    if u8(1)\n    end"), vec!["E0200"]);
}

#[test]
fn fallible_values() {
    let pre = "choice E\n    a\n    b { code : u32 }\nend\nproc f(x : u8) -> u8 or E\n    if x == 0 then fail a\n    if x == 1 then fail b { code: 1 }\n    ret x\nend\n";
    assert_eq!(
        codes(&format!(
            "{pre}proc main -> s32\n    entry\n    f(1)\n    ret 0\nend\n"
        )),
        vec!["E0310"]
    );
    assert_eq!(codes(&format!("{pre}proc main -> s32\n    entry\n    _v := f(1) else 0\n    _w := f(2) else ret 3\n    ret 0\nend\n")), Vec::<String>::new());
    assert_eq!(
        codes(&format!(
            "{pre}proc main -> s32\n    entry\n    _v := f(1) else fail\n    ret 0\nend\n"
        )),
        vec!["E0200"]
    );
    assert_eq!(codes(&format!("{pre}proc g -> u8 or E\n    v := f(1) else fail\n    ret v\nend\nproc main -> s32\n    entry\n    _x := g() else 0\n    ret 0\nend\n")), Vec::<String>::new());
    assert_eq!(codes(&format!("{pre}proc main -> s32\n    entry\n    case f(1)\n    when ok v\n        _q := v\n    end\n    ret 0\nend\n")), vec!["E0311"]);
    assert_eq!(codes(&format!("{pre}proc main -> s32\n    entry\n    case f(1)\n    when ok v\n        _q := v\n    when fail a\n    end\n    ret 0\nend\n")), vec!["E0311"]);
    assert_eq!(codes(&format!("{pre}proc main -> s32\n    entry\n    case f(1)\n    when ok v\n        _q := v\n    when fail a\n    when fail b {{ code }}\n        _c := code\n    end\n    ret 0\nend\n")), Vec::<String>::new());
    assert_eq!(codes(&format!("{pre}proc main -> s32\n    entry\n    case f(1)\n    when ok\n    when fail e\n        _x := e\n    when fail a\n    end\n    ret 0\nend\n")), vec!["W0003"]);
    assert_eq!(
        codes(&format!(
            "{pre}proc main -> s32\n    entry\n    if true then fail a\n    ret 0\nend\n"
        )),
        vec!["E0200", "E0100"]
    );
    assert_eq!(
        body_codes("    x := u8(3)\n    case x\n    when 1\n    when 2\n    end"),
        vec!["E0311"]
    );
    assert_eq!(
        body_codes("    x := u8(3)\n    case x\n    when 1\n    else\n    end"),
        Vec::<String>::new()
    );
    assert_eq!(
        body_codes("    x := true\n    case x\n    when true\n    when false\n    end"),
        Vec::<String>::new()
    );
    assert_eq!(body_codes("    _x := \"a\" else 0"), vec!["E0200"]);
}

#[test]
fn choices_and_layouts() {
    let pre = "layout P\n    x : s32\n    y : s32\nend\nchoice S\n    dot\n    line { a : P, b : P }\nend\n";
    assert_eq!(
        codes(&format!(
            "{pre}proc main -> s32\n    entry\n    p := P {{ x: 1 }}\n    _q := p\n    ret 0\nend\n"
        )),
        vec!["E0208"]
    );
    assert_eq!(codes(&format!("{pre}proc main -> s32\n    entry\n    p := P {{ x: 1, y: 2, z: 3 }}\n    _q := p\n    ret 0\nend\n")), vec!["E0208"]);
    assert_eq!(codes(&format!("{pre}proc main -> s32\n    entry\n    p : P <- P {{ x: 1, y: 2 }}\n    p.x <- 5\n    _s := S.line {{ a: p, b: p }}\n    _d := S.dot\n    ret p.y\nend\n")), Vec::<String>::new());
    assert_eq!(codes(&format!("{pre}proc main -> s32\n    entry\n    p : P <- P {{ x: 1, y: 2 }}\n    _z := p.z\n    ret 0\nend\n")), vec!["E0105"]);
    assert_eq!(
        codes(&format!(
            "{pre}proc main -> s32\n    entry\n    _n := P.size + P.align\n    ret 0\nend\n"
        )),
        Vec::<String>::new()
    );
    assert_eq!(
        codes("layout R\n    r : R\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0204"]
    );
    assert_eq!(
        codes(
            "layout L\n    a : u8\n    a : u8\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"
        ),
        vec!["E0102"]
    );
}

#[test]
fn layout_sizes() {
    let (p, c) = run("layout A\n    a : u8\n    b : u32\n    c : u16\nend\nlayout B packed\n    a : u8\n    b : u32\nend\nlayout C align 16\n    a : u8\nend\nchoice D\n    x\n    y { v : u64 }\nend\nproc main -> s32\n    entry\n    ret 0\nend\n", Target::Hosted);
    assert!(c.is_empty(), "{c:?}");
    let by = |n: &str| p.layouts.iter().find(|l| l.name == n).unwrap();
    let a = by("A");
    assert_eq!((a.size, a.align), (12, 4));
    assert_eq!(
        a.fields.iter().map(|f| f.offset).collect::<Vec<_>>(),
        vec![0, 4, 8]
    );
    let b = by("B");
    assert_eq!((b.size, b.align), (5, 1));
    let cl = by("C");
    assert_eq!((cl.size, cl.align), (16, 16));
    let d = p.choices.iter().find(|c| c.name == "D").unwrap();
    assert_eq!(
        (d.size, d.align, d.tag_bytes, d.payload_offset),
        (16, 8, 1, 8)
    );
}

#[test]
fn constants() {
    ok("N := 4\nM : u32 := N * 2\nS := \"hi\"\nB := N < 5\nT : [2]u8 := { 1, N }\nproc main -> s32\n    entry\n    _a : u32 := M\n    _b : u8 := T[1]\n    _c := S.len\n    if B then ret 1\n    ret s32(N)\nend\n");
    assert_eq!(
        codes("A := B\nB := A\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0106"]
    );
    assert_eq!(
        codes("N : u8 := 300\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0202"]
    );
    assert_eq!(
        codes("N : u8 := 200 + 100\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0212"]
    );
    assert_eq!(
        codes("N := 1 / 0\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0213"]
    );
    assert_eq!(
        codes("proc f -> u8\n    ret 1\nend\nN : u8 := f()\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0107"]
    );
    assert_eq!(
        codes("x : u8 <- 5\ny : u8 <- x\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0107"]
    );
    assert_eq!(
        codes("x : u8 <- 5\nproc main -> s32\n    entry\n    x <- 6\n    ret s32(x)\nend\n"),
        Vec::<String>::new()
    );
    assert_eq!(
        codes(
            "T : [2]u8 := { 1, 2 }\nproc main -> s32\n    entry\n    T[0] <- 3\n    ret 0\nend\n"
        ),
        vec!["E0111"]
    );
    assert_eq!(
        codes("N := 4\n    section \".x\"\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0018"]
    );
}

#[test]
fn capabilities() {
    assert_eq!(
        body_codes("    a := addr u8 (16)\n    _v := [a]"),
        vec!["E0401", "E0401"]
    );
    assert_eq!(
        codes("proc f(a : addr u8) -> u8\n    ret [a]\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0401"]
    );
    assert_eq!(codes("proc f(a : addr u8) -> u8\npermit memory.raw\n    ret [a]\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"), Vec::<String>::new());
    assert_eq!(
        body_codes("    machine x64\n        nop\n    end"),
        vec!["E0401"]
    );
    assert_eq!(body_codes("    cpu.halt()"), vec!["E0401"]);
    assert_eq!(
        codes("proc main -> s32\n    entry\npermit cpu.halt\n    cpu.halt()\n    ret 0\nend\n"),
        vec!["W0100"]
    );
    assert_eq!(
        codes("proc main -> s32\n    entry\npermit cpu.fly\n    ret 0\nend\n"),
        vec!["E0402"]
    );
    assert_eq!(body_codes("    os.syscall(60, 0)"), vec!["E0401"]);
    assert_eq!(
        body_codes("    zone z 4K\n        _b := z.bytes(16)\n    end"),
        vec!["E0401"]
    );
}

#[test]
fn freestanding_rules() {
    let (_, c) = run("proc start -> never\n    entry\n    calls none\n    permit cpu.asm\n    machine x64\n        hlt\n    end\nend\n", Target::Freestanding);
    assert!(c.is_empty(), "{c:?}");
    let (_, c) = run("proc f\nend\n", Target::Freestanding);
    assert_eq!(c, vec!["E0602"]);
    let (_, c) = run("proc a -> never\n    entry\n    loop\n    end\nend\nproc b -> never\n    entry\n    loop\n    end\nend\n", Target::Freestanding);
    assert_eq!(c, vec!["E0602"]);
    let (_, c) = run(
        "proc a -> never\n    entry\n    permit os.syscall\n    loop\n    end\nend\n",
        Target::Freestanding,
    );
    assert_eq!(c, vec!["E0400"]);
    let (_, c) = run(
        "proc a -> never\n    entry\n    zone z 4K\n    end\n    loop\n    end\nend\n",
        Target::Freestanding,
    );
    assert_eq!(c, vec!["E0330"]);
    let (_, c) = run("proc t(_kind : core.TrapKind, _site : core.Site) -> never\n    traps\n    loop\n    end\nend\nproc a -> never\n    entry\n    loop\n    end\nend\n", Target::Freestanding);
    assert!(c.is_empty(), "{c:?}");
    let (_, c) = run("proc t(_kind : u8) -> never\n    traps\n    loop\n    end\nend\nproc a -> never\n    entry\n    loop\n    end\nend\n", Target::Freestanding);
    assert_eq!(c, vec!["E0603"]);
    let (_, c) = run(
        "proc a -> never\n    entry\n    calls none\n    _x : u8 <- 1\nend\n",
        Target::Freestanding,
    );
    assert_eq!(c, vec!["E0603"]);
}

#[test]
fn machine_blocks() {
    let src = "b : u32 <- 0\nproc main -> s32\n    entry\npermit cpu.asm\n    v : u32\n    machine x64\n        in eax <- 7\n        cpuid\n        out ebx -> v\n        mov rax, [b + 4]\n        clobber ecx, memory\n    end\n    ret s32.bits(v)\nend\n";
    assert_eq!(codes(src), Vec::<String>::new());
    assert_eq!(codes("proc main -> s32\n    entry\npermit cpu.asm\n    machine x64\n        in zzz <- 1\n    end\n    ret 0\nend\n"), vec!["E0500"]);
    assert_eq!(codes("proc main -> s32\n    entry\npermit cpu.asm\n    v : u8\n    machine x64\n        out eax -> v\n    end\n    ret s32(v)\nend\n"), vec!["E0200"]);
    assert_eq!(codes("proc main -> s32\n    entry\npermit cpu.asm\n    machine x64\n        ret\n    end\n    ret 0\nend\n"), vec!["E0500"]);
    assert_eq!(codes("proc main -> s32\n    entry\npermit cpu.asm\n    x : u8 <- 1\n    machine x64\n        mov al, x\n    end\n    ret s32(x)\nend\n"), vec!["E0500"]);
}

#[test]
fn escape_analysis_accepts_valid_programs() {
    ok("layout H\n    a : u32\nend\nproc s -> view u8\n    ret \"static\"\nend\nproc sub(v : view u8) -> view u8\n    ret v[1..]\nend\nproc hdr(v : view u8) -> ref H\n    ret H.at(v)\nend\nproc alloc(z : zone) -> rw view u8\n    ret z.bytes(16)\nend\nproc main -> s32\n    entry\npermit os.syscall\n    zone a 4K\n        zone b 1K\n            _x := b.bytes(8)\n        end\n        each _i in 0..3\n            zone c 1K\n                _y := c.bytes(8)\n                break\n            end\n        end\n        buf := a.bytes(64)\n        _h := hdr(buf)\n        _s := sub(buf)\n    end\n    ret 0\nend\n");
}

#[test]
fn escape_analysis_rejects_escapes() {
    assert_eq!(
        codes(
            "proc f -> view u8\n    a : [4]u8\n    ret a\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"
        ),
        vec!["E0300"]
    );
    assert_eq!(codes("proc f -> rw view u8\npermit os.syscall\n    zone z 4K\n        ret z.bytes(8)\n    end\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"), vec!["E0300"]);
    assert_eq!(codes("g : view u8 <- \"x\"\nproc main -> s32\n    entry\npermit os.syscall\n    zone z 4K\n        g <- z.bytes(8)\n    end\n    ret 0\nend\n"), vec!["E0300"]);
    assert_eq!(codes("proc main -> s32\n    entry\npermit os.syscall\n    zone a 4K\n        p : view u8\n        zone b 1K\n            p <- b.bytes(8)\n        end\n        _q := p\n    end\n    ret 0\nend\n"), vec!["E0300"]);
    assert_eq!(codes("proc main -> s32\n    entry\npermit os.syscall\n    zone a 4K\n        p : view u8\n        zone b 1K\n            p <- a.bytes(8)\n        end\n        _q := p\n    end\n    ret 0\nend\n"), Vec::<String>::new());
    assert_eq!(codes("proc main -> s32\n    entry\npermit os.syscall\n    h : zone\n    zone z 4K\n        h <- z\n    end\n    _b := h.bytes(1)\n    ret 0\nend\n"), vec!["E0300"]);
    assert_eq!(codes("proc f -> ref u32\n    x : u32 <- 1\n    ret ref x\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"), vec!["E0300"]);
    assert_eq!(codes("proc f(z : zone) -> rw view u8\npermit os.syscall\n    zone inner 1K from z\n        ret inner.bytes(4)\n    end\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"), vec!["E0300"]);
}

#[test]
fn not_implemented_features() {
    assert_eq!(
        codes("proc f(_x : own u8)\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0900"]
    );
    assert_eq!(
        body_codes("    x : u8 <- 1\n    x <~ 2\n    _y := x"),
        vec!["E0900"]
    );
    assert_eq!(
        codes("proc f\n    calls interrupt\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"),
        vec!["E0900"]
    );
    assert_eq!(body_codes("    _p := port u8 (0x3F8)"), vec!["E0900"]);
    assert_eq!(body_codes("    _f : f32 := 1"), vec!["E0900"]);
    assert_eq!(
        body_codes("    _v := mem.mmio(u16, 0xB8000, 10)"),
        vec!["E0900"]
    );
}

#[test]
fn endian_fields_and_case_on_refs() {
    let pre =
        "layout H\n    len : be u16\n    n : le u32\nend\nchoice S\n    a\n    b { v : u8 }\nend\n";
    let src = format!("{pre}proc f(h : rw ref H, s : ref S) -> u8\n    h.len <- 5\n    x : u16 := h.len\n    h.n <- u32(x)\n    case s\n    when a\n        ret 1\n    when b {{ v }}\n        ret v\n    end\nend\nproc main -> s32\n    entry\n    ret 0\nend\n");
    assert_eq!(codes(&src), Vec::<String>::new());
    assert_eq!(
        codes(&format!(
            "{pre}proc main -> s32\n    entry\n    _x : be u16 := 1\n    ret 0\nend\n"
        )),
        vec!["E0200"]
    );
    assert_eq!(
        codes(&format!(
            "{pre}proc f(_x : le u32)\nend\nproc main -> s32\n    entry\n    ret 0\nend\n"
        )),
        vec!["E0200"]
    );
}
