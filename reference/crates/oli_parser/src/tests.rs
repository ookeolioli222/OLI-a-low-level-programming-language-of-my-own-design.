use crate::parse_source;
use oli_ast::print_module;
use oli_diag::{Diagnostics, SourceFile};

/// Parses `src`; returns the printed AST and the diagnostics as (code, line, col).
fn parse(src: &str) -> (String, Vec<(&'static str, u32, u32)>) {
    let file = SourceFile::new("t.oli", src);
    let mut diags = Diagnostics::new();
    let m = parse_source(&file, &mut diags);
    let codes = diags
        .sorted()
        .into_iter()
        .map(|d| {
            let (l, c) = file.line_col(d.span.start);
            (d.code, l, c)
        })
        .collect();
    (print_module(&m), codes)
}

fn ast(src: &str) -> String {
    let (out, codes) = parse(src);
    assert!(codes.is_empty(), "unexpected diagnostics {codes:?}\n{out}");
    out
}

fn codes(src: &str) -> Vec<&'static str> {
    parse(src).1.into_iter().map(|(c, _, _)| c).collect()
}

/// Wraps statements in a procedure and returns the printed body, trimmed.
fn body(stmts: &str) -> String {
    let out = ast(&format!("proc t\n{stmts}\nend\n"));
    let start = out.find("(body").map_or(0, |i| i + 5);
    // Strip exactly the three closers of body, proc and module.
    let inner = out.get(start..).unwrap_or("").trim();
    inner
        .strip_suffix(")))")
        .unwrap_or(inner)
        .trim()
        .to_string()
}

/// Prints one expression as its S-expression.
fn expr(e: &str) -> String {
    let b = body(&format!("x := {e}"));
    let inner = b.trim_start_matches("(bind x ");
    inner.strip_suffix(')').unwrap_or(inner).to_string()
}

#[test]
fn precedence() {
    assert_eq!(expr("a + b * c"), "(+ (name a) (* (name b) (name c)))");
    assert_eq!(expr("a * b + c"), "(+ (* (name a) (name b)) (name c))");
    assert_eq!(
        expr("a == b and c or d"),
        "(or (and (== (name a) (name b)) (name c)) (name d))"
    );
    assert_eq!(expr("not a == b"), "(== (not (name a)) (name b))");
    assert_eq!(expr("-a + b"), "(+ (neg (name a)) (name b))");
    assert_eq!(
        expr("a & b | c ^ d"),
        "(| (& (name a) (name b)) (^ (name c) (name d)))"
    );
    assert_eq!(expr("a << 1 + 2"), "(<< (name a) (+ (int 1) (int 2)))");
    assert_eq!(expr("(a + b) * c"), "(* (+ (name a) (name b)) (name c))");
    assert_eq!(expr("a - b - c"), "(- (- (name a) (name b)) (name c))");
    assert_eq!(expr("~a % 2"), "(% (bitnot (name a)) (int 2))");
    assert_eq!(codes("proc t\nx := a < b < c\nend"), vec!["E0031"]);
}

#[test]
fn fallback_is_loosest() {
    assert_eq!(
        expr("f() else 0 + 1"),
        "(else (call (name f)) (default (+ (int 0) (int 1))))"
    );
    assert_eq!(
        expr("a + f() else 0"),
        "(else (+ (name a) (call (name f))) (default (int 0)))"
    );
    assert_eq!(expr("f() else fail"), "(else (call (name f)) (fail))");
    assert_eq!(expr("f() else ret"), "(else (call (name f)) (ret))");
    assert_eq!(
        expr("f() else ret 1"),
        "(else (call (name f)) (ret (int 1)))"
    );
}

#[test]
fn store_versus_less_than_negative() {
    assert_eq!(
        body("x <- y < -1"),
        "(store (name x) (< (name y) (neg (int 1))))"
    );
    assert_eq!(codes("proc t\nif n <-1\nend\nend"), vec!["E0020"]);
}

#[test]
fn bindings_places_stores() {
    assert_eq!(body("x := 1"), "(bind x (int 1))");
    assert_eq!(body("x : u8 := 1"), "(bind x : u8 (int 1))");
    assert_eq!(body("x : u8"), "(place x : u8)");
    assert_eq!(body("x : u8 <- 1"), "(place x : u8 (int 1))");
    assert_eq!(body("[a] <- 1"), "(store (raw-load (name a)) (int 1))");
    assert_eq!(
        body("a.b[1] <- 2"),
        "(store (index (field (name a) b) (int 1)) (int 2))"
    );
    assert_eq!(body("a <~ b"), "(move (name a) (name b))");
    assert_eq!(codes("proc t\nf() <- 1\nend"), vec!["E0017"]);
    assert_eq!(codes("proc t\na.b := 1\nend"), vec!["E0011"]);
}

#[test]
fn addr_forms() {
    assert_eq!(expr("addr x"), "(addr-of (name x))");
    assert_eq!(
        expr("addr x.y[0]"),
        "(addr-of (index (field (name x) y) (int 0)))"
    );
    assert_eq!(expr("addr u8 (1)"), "(convert addr u8 (int 1))");
    assert_eq!(expr("addr (1)"), "(convert addr (int 1))");
    assert_eq!(expr("addr Regs (1)"), "(convert addr Regs (int 1))");
    assert_eq!(expr("addr a + 8"), "(+ (addr-of (name a)) (int 8))");
    assert_eq!(expr("ref p"), "(ref-of (name p))");
    assert_eq!(expr("rw ref p"), "(rw-ref-of (name p))");
}

#[test]
fn conversions_and_modes() {
    assert_eq!(expr("u8(x)"), "(convert u8 (name x))");
    assert_eq!(expr("u8.wrap(x)"), "(convert u8 wrap (name x))");
    assert_eq!(expr("uword.bits(n)"), "(convert uword bits (name n))");
    assert_eq!(expr("u8.size"), "(field (type u8) size)");
    assert_eq!(expr("port u8 (1)"), "(convert port u8 (int 1))");
    assert_eq!(expr("wrap(a + b)"), "(wrap (+ (name a) (name b)))");
    assert_eq!(expr("checked(a * b)"), "(checked (* (name a) (name b)))");
    assert_eq!(expr("z.make(u8)"), "(call (field (name z) make) (type u8))");
    assert_eq!(expr("v.addr"), "(field (name v) addr)");
    assert_eq!(
        expr("Header.at(v)"),
        "(call (field (name Header) at) (name v))"
    );
}

#[test]
fn one_line_if() {
    assert_eq!(
        body("if a then ret 1"),
        "(if-then (name a)\n        (ret (int 1)))"
    );
    assert_eq!(codes("proc t\nif a then f() else 0\nend"), vec!["E0016"]);
    assert_eq!(codes("proc t\nif a then while b\nend\nend"), vec!["E0021"]);
    assert_eq!(codes("proc t\nif a then\nend"), vec!["E0012"]);
}

#[test]
fn block_if_elif_else() {
    let out = body("if a\n  x := 1\nelif b\n  x := 2\nelse\n  x := 3\nend");
    assert!(out.contains("(if (name a)"), "{out}");
    assert!(out.contains("(elif (name b)"), "{out}");
    assert!(out.contains("(else\n"), "{out}");
    assert_eq!(codes("proc t\nelse\nend"), vec!["E0011"]);
    assert_eq!(codes("proc t\nwhile a\nelif b\nend\nend"), vec!["E0011"]);
}

#[test]
fn case_and_patterns() {
    let src = "case x\nwhen ok v\n  f()\nwhen fail e { a, b }\n  g()\nwhen -1\n  h()\nwhen 'c'\n  i()\nwhen none\n  j()\nelse\n  k()\nend";
    let out = body(src);
    for needle in [
        "(when ok v",
        "(when fail e {a b}",
        "(when (int -1)",
        "(when (char \"c\")",
        "(when none",
        "(else\n",
    ] {
        assert!(out.contains(needle), "missing {needle:?} in\n{out}");
    }
    assert_eq!(
        codes("proc t\ncase x\n  f()\nwhen 1\n  g()\nend\nend"),
        vec!["E0011"]
    );
}

#[test]
fn loops_and_zones() {
    assert!(body("each i in 0..n\nend").starts_with("(each i (range (int 0) (name n))"));
    assert!(body("each b in v\nend").starts_with("(each b (name v)"));
    assert!(body("while a\n  break\n  continue\nend").contains("(break)"));
    assert!(body("loop\nend").starts_with("(loop"));
    assert!(body("zone z 4K\nend").starts_with("(zone z (int 4096)"));
    assert!(body("zone z 4K at p\nend").starts_with("(zone z (int 4096) at (name p)"));
    assert!(body("zone z 4K from q\nend").starts_with("(zone z (int 4096) from (name q)"));
    assert_eq!(expr("v[a..]"), "(index (name v) (range (name a) end))");
    assert_eq!(
        expr("v[a..b]"),
        "(index (name v) (range (name a) (name b)))"
    );
}

#[test]
fn machine_blocks() {
    let src = "machine x64\n  in eax <- 0\n  in al, dx\n  cpuid\n  out ebx -> b\n  out dx, al\n  .top:\n  jmp .top\n  mov qword [rbp - 8], rax\n  lea rax, [rip + rcx*8]\n  and rax, -1\n  clobber ecx, memory\nend";
    let out = body(src);
    for needle in [
        "(in eax (int 0))",
        "(instr in al dx)",
        "(instr cpuid)",
        "(out ebx (name b))",
        "(instr out dx al)",
        "(label top)",
        "(instr jmp .top)",
        "(instr mov (mem qword +rbp -8) rax)",
        "(instr lea rax (mem +rip +rcx*8))",
        "(instr and rax -1)",
        "(clobber ecx memory)",
    ] {
        assert!(out.contains(needle), "missing {needle:?} in\n{out}");
    }
    assert_eq!(
        codes("proc t\nmachine x64\n  out ebx -> f()\nend\nend"),
        vec!["E0017"]
    );
    assert_eq!(
        codes("proc t\nmachine x64\n  lea rax, [a.b*8]\nend\nend"),
        vec!["E0019"]
    );
    assert_eq!(
        codes("proc t\nmachine x64\n  mov rax, 340282366920938463463374607431768211455\nend\nend"),
        vec!["E0019"]
    );
}

#[test]
fn declarations() {
    let src = "module a.b\nimport x.y as z\nimport core.mem\n\nN := 4\nM : u32 := 5\ns : [4]u8\n    section \".bss\"\n    align 16\nt : u64 <- 1\nlayout L packed align 4\n  a : u8\n  b : be u16 align 2\nend\nchoice C\n  x\n  y { p : u8, q : ref L }\nend\npub proc f(a : u8, b : view u8) -> u8 or C\n  permit memory.raw, cpu.asm\n  calls none\n  section \".text\"\n  align 64\n  export \"sym\"\nend\n";
    let out = ast(src);
    for needle in [
        "(module a.b",
        "(import x.y as z)",
        "(const N (int 4))",
        "(const M : u32 (int 5))",
        "(static s : [4]u8\n    (section \".bss\")\n    (align 16))",
        "(static t : u64 (int 1))",
        "(layout L packed align 4\n    (field a u8)\n    (field b be u16 align 2))",
        "(variant y\n      (field p u8)\n      (field q ref L))",
        "(pub proc f ((a u8) (b view u8)) -> u8 or C\n    (permit memory.raw cpu.asm)\n    (calls none)\n    (section \".text\")\n    (align 64)\n    (export \"sym\")",
    ] {
        assert!(out.contains(needle), "missing {needle:?} in\n{out}");
    }
}

#[test]
fn declaration_errors() {
    assert_eq!(codes("proc f\nend\nimport a\n"), vec!["E0023"]);
    assert_eq!(codes("module a\nmodule b\n"), vec!["E0024"]);
    assert_eq!(codes("end\n"), vec!["E0015"]);
    assert_eq!(codes("pub 5\n"), vec!["E0015"]);
    assert_eq!(codes("x\n"), vec!["E0015"]);
    assert_eq!(codes("const := 1\n"), vec!["E0010"]);
    assert_eq!(codes("proc static\nend\n"), vec!["E0010"]);
    assert_eq!(codes("proc f\n  align 3\nend\n"), vec!["E0018"]);
    assert_eq!(codes("proc f\n  calls fast\nend\n"), vec!["E0018"]);
    assert_eq!(codes("proc f\n  entry\n  entry\nend\n"), vec!["E0018"]);
    assert_eq!(codes("s : u8\n  permit cpu.asm\n"), vec!["E0018"]);
    assert_eq!(codes("proc f(a : u8 or b or c)\nend\n"), vec!["E0013"]);
    assert_eq!(codes("proc f(a : [x y]u8)\nend\n"), vec!["E0011"]);
}

#[test]
fn doc_comments() {
    let out = ast("--- first\n--- second\nproc f\n  --- inner is dropped\n  x := 1\nend\n");
    assert!(
        out.contains("(doc \"first\")\n    (doc \"second\")"),
        "{out}"
    );
    assert!(!out.contains("inner"), "{out}");
    assert_eq!(codes("--- dangling\n"), vec!["W0001"]);
    assert_eq!(codes("--- dangling\nimport a\n"), vec!["W0001"]);
}

#[test]
fn missing_end_and_recovery() {
    let (out, c) = parse("proc f\n  x := 1\nproc g\n  y := 2\nend\n");
    assert_eq!(c.iter().map(|x| x.0).collect::<Vec<_>>(), vec!["E0014"]);
    assert!(out.contains("(proc f") && out.contains("(proc g"), "{out}");

    let (out, c) = parse("proc f\n  if a\n    x := 1\n");
    assert_eq!(
        c.iter().map(|x| x.0).collect::<Vec<_>>(),
        vec!["E0014", "E0014"]
    );
    assert!(out.contains("(bind x (int 1))"), "{out}");

    // One bad statement does not lose the following ones.
    let (out, c) = parse("proc f\n  a := 1\n  b := 2 2\n  c := 3\nend\n");
    assert_eq!(c.len(), 1, "{c:?}");
    assert!(
        out.contains("(bind a (int 1))") && out.contains("(bind c (int 3))"),
        "{out}"
    );

    // A bad block header keeps its body attached.
    let (out, c) = parse("proc f\n  while a b\n    b := 1\n  end\n  c := 2\nend\n");
    assert_eq!(c.len(), 1, "{c:?}");
    assert!(out.contains("(while (name a)"), "{out}");
    assert!(
        out.contains("(bind b (int 1))") && out.contains("(bind c (int 2))"),
        "{out}"
    );
    // An unclosed `(` swallows the following line (newlines are trivia inside
    // brackets); the body after it is still parsed and `end` still matches.
    let (out, c) = parse("proc f\n  while (a\n    b := 1\n  end\n  c := 2\nend\n");
    assert_eq!(c.len(), 1, "{c:?}");
    assert!(
        out.contains("(while (error)") && out.contains("(bind c (int 2))"),
        "{out}"
    );
}

#[test]
fn newline_continuation() {
    assert_eq!(body("x := a +\n  b"), "(bind x (+ (name a) (name b)))");
    assert_eq!(
        body("x := f(1,\n  2)"),
        "(bind x (call (name f) (int 1) (int 2)))"
    );
    assert_eq!(
        body("x := {\n  1,\n  2,\n}"),
        "(bind x (array (int 1) (int 2)))"
    );
    assert_eq!(body("x :=\n  1"), "(bind x (int 1))");
    assert_eq!(
        body("x := P {\n  a: 1,\n  b: 2 }"),
        "(bind x (lit (name P) (a: (int 1)) (b: (int 2))))"
    );
    // A line *starting* with an operator does not continue the previous one.
    assert_eq!(codes("proc t\nx := a\n  .b\nend"), vec!["E0012"]);
}

#[test]
fn brace_block_hint() {
    let c = codes("proc f\n  if a == b {\n    x := 1\n  }\nend\n");
    assert!(c.contains(&"E0032"), "{c:?}");
}

#[test]
fn named_arguments() {
    assert_eq!(
        expr("f(x: 1, y: 2)"),
        "(call (name f) (x: (int 1)) (y: (int 2)))"
    );
    assert_eq!(codes("proc t\nx := f(1, y: 2)\nend"), vec!["E0022"]);
}

#[test]
fn nesting_limit_does_not_crash() {
    let deep = format!(
        "proc t\nx := {}1{}\nend\n",
        "(".repeat(300),
        ")".repeat(300)
    );
    assert_eq!(codes(&deep), vec!["E0030"]);
    let deep_blocks = format!(
        "proc t\n{}x := 1\n{}end\n",
        "if a\n".repeat(300),
        "end\n".repeat(300)
    );
    assert!(codes(&deep_blocks).contains(&"E0030"));
}

#[test]
fn truncated_inputs_never_panic() {
    let src = include_str!("../../../../examples/packet_demo.oli");
    for cut in 0..src.len() {
        if src.is_char_boundary(cut) {
            let _ = parse(src.get(..cut).unwrap_or(""));
        }
    }
    let kernel = include_str!("../../../../tests/parse/ok/kernel_sketch.oli");
    for cut in (0..kernel.len()).step_by(7) {
        if kernel.is_char_boundary(cut) {
            let _ = parse(kernel.get(..cut).unwrap_or(""));
        }
    }
}

#[test]
fn garbage_inputs_never_panic() {
    for src in [
        "",
        "\n",
        "end end end",
        "proc",
        "proc (",
        "proc f(",
        "proc f( a :",
        "proc f -> ",
        "if",
        "zone",
        "machine",
        "machine x64\n in",
        "machine x64\n out",
        "machine x64\n [",
        "layout",
        "layout L\n a :",
        "choice C\n a {",
        "case",
        "case x\nwhen",
        "case x\nwhen fail",
        "x := ",
        "x : ",
        "[",
        "{",
        "(",
        "..",
        "a..",
        "-",
        "not",
        "addr",
        "ref",
        "rw",
        "u8.",
        "u8.wrap",
        "port",
        "wrap",
        "else",
        "when",
        "then",
        "'",
        "0x",
        "--- ",
        "import",
        "import a as",
        "pub",
        "pub pub proc",
        "proc f\n permit\nend",
        "proc f\n calls\nend",
        "proc f\n section\nend",
        "proc f\n export 1\nend",
        "x := a else",
        "x := a else ret",
    ] {
        let _ = parse(src);
    }
}
