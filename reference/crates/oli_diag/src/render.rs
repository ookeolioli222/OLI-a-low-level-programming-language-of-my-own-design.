use crate::{Diagnostic, Severity, SourceFile};
use std::fmt::Write;

/// Renders a diagnostic in the fixed `spec/OLI_SYNTAX_V0.md` §8 format:
///
/// ```text
/// error[E0012]: expected expression
///  --> test.oli:12:15
///    |
/// 12 | total <-
///    |         ^ expression expected here
/// ```
pub fn render(d: &Diagnostic, file: &SourceFile) -> String {
    let mut out = String::new();
    let sev = match d.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    let _ = writeln!(out, "{sev}[{}]: {}", d.code, d.message);

    let (line, col) = file.line_col(d.span.start);
    let _ = writeln!(out, " --> {}:{}:{}", file.name(), line, col);

    let mut spans: Vec<(u32, u32, &str)> = Vec::new();
    spans.push((d.span.start, d.span.end, d.label.as_str()));
    for (s, l) in &d.secondary {
        spans.push((s.start, s.end, l.as_str()));
    }
    let width = spans
        .iter()
        .map(|&(s, _, _)| file.line_of(s))
        .max()
        .map_or(1, |l| l.to_string().len());
    let pad = " ".repeat(width);
    let _ = writeln!(out, "{pad} |");
    for (start, end, label) in spans {
        snippet(&mut out, file, start, end, label, width);
    }
    for n in &d.notes {
        let _ = writeln!(out, "{pad} = note: {n}");
    }
    out
}

fn snippet(out: &mut String, file: &SourceFile, start: u32, end: u32, label: &str, width: usize) {
    let (line, col) = file.line_col(start);
    let text = file.line_text(line);
    let _ = writeln!(out, "{line:>width$} | {text}");
    // Caret length: the span on this line only, at least one caret.
    let line_end = file.line_start(line) as usize + text.len();
    let end = (end as usize).clamp(start as usize, line_end);
    let caret_len = file
        .slice(crate::Span::new(start, end as u32))
        .chars()
        .count()
        .max(1);
    let pad = " ".repeat(width);
    let indent = " ".repeat(col.saturating_sub(1) as usize);
    let carets = "^".repeat(caret_len);
    if label.is_empty() {
        let _ = writeln!(out, "{pad} | {indent}{carets}");
    } else {
        let _ = writeln!(out, "{pad} | {indent}{carets} {label}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;

    #[test]
    fn spec_format() {
        let src = "proc main -> s32\n    total <-\nend\n";
        let file = SourceFile::new("test.oli", src);
        // `total <-` is on line 2; the missing expression is at end of line.
        let at = src.find("<-").map_or(0, |i| i + 2) as u32;
        let d = Diagnostic::error("E0012", "expected expression", Span::point(at))
            .with_label("expression expected here");
        let text = render(&d, &file);
        let expected = "error[E0012]: expected expression\n --> test.oli:2:13\n  |\n2 |     total <-\n  |             ^ expression expected here\n";
        assert_eq!(text, expected);
    }

    #[test]
    fn multi_char_span_and_note() {
        let file = SourceFile::new("a.oli", "x := 10\n");
        let d = Diagnostic::warning("W0001", "unused", Span::new(0, 1))
            .with_note("prefix with _ to silence");
        let text = render(&d, &file);
        assert!(text.starts_with("warning[W0001]: unused\n --> a.oli:1:1\n"));
        assert!(text.contains("1 | x := 10\n  | ^\n"));
        assert!(text.ends_with("  = note: prefix with _ to silence\n"));
    }
}
