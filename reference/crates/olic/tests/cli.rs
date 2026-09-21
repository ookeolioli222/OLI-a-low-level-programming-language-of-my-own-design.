//! End-to-end tests of the `olic` binary: flags, exit codes and the exact
//! diagnostic format required by `spec/OLI_SYNTAX_V0.md` §8.

// Tests may panic on failure; the no-panic policy applies to the compiler proper.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used, dead_code)]

mod common;

use std::process::{Command, Output};

/// Runs `olic` with `args`. Returns `None` (and prints a loud notice) when the
/// operating system refuses to execute the freshly built binary, as Windows
/// Smart App Control does for unsigned local builds; the in-process tests
/// still cover the same code paths.
fn olic(args: &[&str]) -> Option<Output> {
    let exe = env!("CARGO_BIN_EXE_olic");
    match Command::new(exe)
        .args(args)
        .current_dir(common::repo_root())
        .output()
    {
        Ok(o) => Some(o),
        Err(e) => {
            eprintln!("SKIPPED: cannot execute {exe}: {e} (application control policy?)");
            None
        }
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

#[test]
fn check_syntax_exit_codes() {
    let Some(ok) = olic(&["examples/hello.oli", "--check-syntax"]) else {
        return;
    };
    assert!(ok.status.success(), "stderr: {}", text(&ok.stderr));
    assert!(ok.stdout.is_empty());

    let Some(bad) = olic(&["tests/parse/err/missing_expr.oli", "--check-syntax"]) else {
        return;
    };
    assert_eq!(bad.status.code(), Some(1));
    assert!(text(&bad.stderr).ends_with("error: 1 error in `tests/parse/err/missing_expr.oli`\n"));
}

#[test]
fn diagnostic_format_matches_the_spec() {
    let Some(out) = olic(&["tests/parse/err/missing_expr.oli", "--check-syntax"]) else {
        return;
    };
    let expected = "error[E0012]: expected expression\n \
                    --> tests/parse/err/missing_expr.oli:2:13\n  \
                    |\n\
                    2 |     total <-\n  \
                    |             ^ expression expected here\n";
    let stderr = text(&out.stderr);
    assert!(stderr.starts_with(expected), "got:\n{stderr}");
}

#[test]
fn show_tokens_and_ast() {
    let Some(toks) = olic(&["examples/hello.oli", "--show-tokens"]) else {
        return;
    };
    assert!(toks.status.success());
    let out = text(&toks.stdout);
    assert!(
        out.starts_with("   1:1    Kw       module\n"),
        "got:\n{out}"
    );
    assert!(out.contains(r#"Str      "Hello Oli--\n""#), "got:\n{out}");
    assert!(out.trim_end().ends_with("Eof      <eof>"), "got:\n{out}");

    let Some(ast) = olic(&["examples/hello.oli", "--show-ast"]) else {
        return;
    };
    assert!(ast.status.success());
    let snapshot = std::fs::read_to_string(common::repo_root().join("tests/snapshots/hello.ast"))
        .map(|s| s.replace("\r\n", "\n"))
        .unwrap_or_default();
    assert_eq!(text(&ast.stdout), snapshot);
}

#[test]
fn show_ast_on_a_broken_file_prints_the_partial_tree_and_fails() {
    let Some(out) = olic(&["tests/parse/err/missing_expr.oli", "--show-ast"]) else {
        return;
    };
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stdout).contains("(proc main () -> s32"));
    assert!(text(&out.stderr).contains("error[E0012]"));
}

#[test]
fn compiling_reports_not_implemented_honestly() {
    let Some(out) = olic(&["examples/hello.oli"]) else {
        return;
    };
    assert_eq!(out.status.code(), Some(1));
    let stderr = text(&out.stderr);
    assert!(
        stderr.starts_with("error[E0900]: feature not implemented: code generation"),
        "got:\n{stderr}"
    );
    assert!(out.stdout.is_empty());
}

#[test]
fn check_and_show_sema() {
    let Some(ok) = olic(&["examples/packet_demo.oli", "--check"]) else {
        return;
    };
    assert!(ok.status.success(), "stderr: {}", text(&ok.stderr));
    assert!(ok.stdout.is_empty());

    let Some(sema) = olic(&["examples/hello.oli", "--show-sema"]) else {
        return;
    };
    assert!(sema.status.success());
    let snapshot = std::fs::read_to_string(common::repo_root().join("tests/snapshots/hello.sema"))
        .map(|s| s.replace("\r\n", "\n"))
        .unwrap_or_default();
    assert_eq!(text(&sema.stdout), snapshot);

    let Some(bad) = olic(&["tests/sema/err/unassigned.oli", "--check"]) else {
        return;
    };
    assert_eq!(bad.status.code(), Some(1));
    let stderr = text(&bad.stderr);
    assert!(
        stderr.contains("error[E0220]: `x` is read before it is stored"),
        "got:\n{stderr}"
    );
    assert!(
        stderr.contains(" --> tests/sema/err/unassigned.oli:7:10"),
        "got:\n{stderr}"
    );

    // Diagnostics in imported library modules name the library file.
    let Some(free) = olic(&[
        "tests/sema/ok/freestanding.oli",
        "--freestanding",
        "--check",
    ]) else {
        return;
    };
    assert!(free.status.success(), "stderr: {}", text(&free.stderr));
    let Some(hosted) = olic(&["tests/sema/ok/freestanding.oli", "--check"]) else {
        return;
    };
    assert_eq!(hosted.status.code(), Some(1));
    assert!(text(&hosted.stderr).contains("error[E0601]"));
}

#[test]
fn usage_errors() {
    let Some(out) = olic(&[]) else { return };
    assert!(out.status.success());
    assert!(text(&out.stdout).starts_with("usage: olic <file.oli>"));

    let Some(out) = olic(&["--bogus"]) else {
        return;
    };
    assert_eq!(out.status.code(), Some(2));

    let Some(out) = olic(&["does/not/exist.oli"]) else {
        return;
    };
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).starts_with("olic: cannot read `does/not/exist.oli`"));

    let Some(out) = olic(&["--version"]) else {
        return;
    };
    assert!(text(&out.stdout).starts_with("olic 0."));
}

#[test]
fn non_utf8_input_is_a_diagnostic_not_a_crash() {
    let dir = std::env::temp_dir().join(format!("olic-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("bad.oli");
    std::fs::write(&path, [b'p', b'r', b'o', b'c', b' ', 0xFF, 0xFE, b'\n']).expect("write");
    let Some(out) = olic(&[path.to_str().unwrap_or("bad.oli")]) else {
        return;
    };
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).starts_with("error[E0007]"));
}
