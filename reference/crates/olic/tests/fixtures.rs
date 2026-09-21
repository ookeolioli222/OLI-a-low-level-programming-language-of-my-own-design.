//! Fixture-driven parser tests.
//!
//! - `tests/parse/ok/*.oli` and `examples/*.oli` must parse without diagnostics.
//! - `tests/parse/err/*.oli` must produce exactly the diagnostics listed in
//!   their `-- expect: CODE @ LINE:COL` comment lines.

// Tests may panic on failure; the no-panic policy applies to the compiler proper.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used, dead_code)]

mod common;

#[test]
fn ok_fixtures_parse_cleanly() {
    let mut files = common::oli_files("tests/parse/ok");
    files.extend(common::oli_files("examples"));
    for f in files {
        let (_, diags) = common::diagnostics(&f);
        assert!(
            diags.is_empty(),
            "{} produced diagnostics: {diags:?}",
            f.display()
        );
    }
}

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
        let l: u32 = l.parse().unwrap_or_else(|_| panic!("bad line in: {line}"));
        let c: u32 = c
            .parse()
            .unwrap_or_else(|_| panic!("bad column in: {line}"));
        out.push((code, l, c));
    }
    out.sort();
    assert!(
        !out.is_empty(),
        "an error fixture needs at least one `-- expect:` line"
    );
    out
}

#[test]
fn err_fixtures_report_exactly_the_expected_diagnostics() {
    for f in common::oli_files("tests/parse/err") {
        let expected = expectations(&common::read(&f));
        let (_, actual) = common::diagnostics(&f);
        assert_eq!(
            actual,
            expected,
            "diagnostics of {} differ from its `-- expect:` lines",
            f.display()
        );
    }
}

#[test]
fn err_fixtures_have_no_error_free_files() {
    for f in common::oli_files("tests/parse/err") {
        let (_, diags) = common::diagnostics(&f);
        assert!(
            !diags.is_empty(),
            "{} is in err/ but parses cleanly",
            f.display()
        );
    }
}
