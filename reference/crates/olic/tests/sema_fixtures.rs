//! Fixture-driven semantic tests.
//!
//! - `tests/sema/ok/*.oli` and `examples/*.oli` must analyze without diagnostics.
//! - `tests/sema/err/*.oli` must produce exactly the diagnostics listed in
//!   their `-- expect: CODE @ LINE:COL` comment lines.
//! - `tests/snapshots/*.sema` hold the `--show-sema` output of the reference programs.

// Tests may panic on failure; the no-panic policy applies to the compiler proper.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used, dead_code)]

mod common;

use std::path::Path;

fn expectations(text: &str) -> Vec<(String, u32, u32)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("-- expect:") else {
            continue;
        };
        let mut parts = rest.split_whitespace();
        let code = parts.next().unwrap_or("").to_string();
        assert_eq!(parts.next(), Some("@"), "bad expect line: {line}");
        let pos = parts.next().unwrap_or("");
        let (l, c) = pos
            .split_once(':')
            .unwrap_or_else(|| panic!("bad position in: {line}"));
        out.push((code, l.parse().unwrap(), c.parse().unwrap()));
    }
    out.sort();
    assert!(
        !out.is_empty(),
        "an error fixture needs at least one `-- expect:` line"
    );
    out
}

#[test]
fn ok_fixtures_analyze_cleanly() {
    let mut files = common::oli_files("tests/sema/ok");
    files.extend(common::oli_files("examples"));
    for f in files {
        let (_, diags) = common::analyze_file(&f);
        assert!(
            diags.is_empty(),
            "{} produced diagnostics: {diags:?}",
            f.display()
        );
    }
}

#[test]
fn err_fixtures_report_exactly_the_expected_diagnostics() {
    for f in common::oli_files("tests/sema/err") {
        let expected = expectations(&common::read(&f));
        let (_, actual) = common::analyze_file(&f);
        assert_eq!(
            actual,
            expected,
            "diagnostics of {} differ from its `-- expect:` lines",
            f.display()
        );
    }
}

fn snapshot(source: &str) {
    let path = common::repo_root().join(source);
    let (program, diags) = common::analyze_file(&path);
    assert!(diags.is_empty(), "{source} has diagnostics: {diags:?}");
    let actual = oli_sema::printer::print_program(&program);
    let stem = Path::new(source)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("snapshot");
    let snap = common::repo_root()
        .join("tests/snapshots")
        .join(format!("{stem}.sema"));
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&snap, &actual).unwrap();
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
        "semantic graph of {source} changed; run with UPDATE_SNAPSHOTS=1 if intended"
    );
}

#[test]
fn hello_sema_snapshot() {
    snapshot("examples/hello.oli");
}

#[test]
fn packet_demo_sema_snapshot() {
    snapshot("examples/packet_demo.oli");
}

#[test]
fn freestanding_sema_snapshot() {
    snapshot("tests/sema/ok/freestanding.oli");
}
