//! AST snapshot tests: `--show-ast` output of the reference programs must
//! match `tests/snapshots/<name>.ast`. Run with `UPDATE_SNAPSHOTS=1` to
//! rewrite the snapshots after an intentional change.

// Tests may panic on failure; the no-panic policy applies to the compiler proper.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used, dead_code)]

mod common;

use std::path::Path;

fn check(source: &str) {
    let path = common::repo_root().join(source);
    let (module, diags) = common::diagnostics(&path);
    assert!(diags.is_empty(), "{source} has diagnostics: {diags:?}");
    let actual = oli_ast::print_module(&module);
    let stem = Path::new(source)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("snapshot");
    let snap = common::repo_root()
        .join("tests/snapshots")
        .join(format!("{stem}.ast"));
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&snap, &actual)
            .unwrap_or_else(|e| panic!("cannot write {}: {e}", snap.display()));
        return;
    }
    let expected = std::fs::read_to_string(&snap)
        .unwrap_or_else(|e| {
            panic!(
                "missing snapshot {} ({e}); run with UPDATE_SNAPSHOTS=1",
                snap.display()
            )
        })
        .replace("\r\n", "\n");
    assert_eq!(
        actual, expected,
        "AST of {source} changed; run with UPDATE_SNAPSHOTS=1 if intended"
    );
}

#[test]
fn hello_ast() {
    check("examples/hello.oli");
}

#[test]
fn packet_demo_ast() {
    check("examples/packet_demo.oli");
}

#[test]
fn kernel_sketch_ast() {
    check("tests/parse/ok/kernel_sketch.oli");
}

#[test]
fn statements_ast() {
    check("tests/parse/ok/statements.oli");
}
