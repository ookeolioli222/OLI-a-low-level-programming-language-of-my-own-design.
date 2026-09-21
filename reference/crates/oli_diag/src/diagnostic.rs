use crate::Span;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One compiler message with a code, a primary span and optional extras.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable code such as `E0012`; the negative test suite matches on it.
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    /// Text printed under the caret; empty means "message only".
    pub label: String,
    pub notes: Vec<String>,
    /// Additional spans with their own labels (e.g. "block opened here").
    pub secondary: Vec<(Span, String)>,
    /// Index of the source file the spans refer to (see [`crate::SourceMap`]).
    pub file: u32,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>, span: Span) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            code,
            message: message.into(),
            span,
            label: String::new(),
            notes: Vec::new(),
            secondary: Vec::new(),
            file: 0,
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>, span: Span) -> Diagnostic {
        Diagnostic {
            severity: Severity::Warning,
            ..Diagnostic::error(code, message, span)
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Diagnostic {
        self.label = label.into();
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Diagnostic {
        self.notes.push(note.into());
        self
    }

    pub fn with_secondary(mut self, span: Span, label: impl Into<String>) -> Diagnostic {
        self.secondary.push((span, label.into()));
        self
    }

    pub fn in_file(mut self, file: u32) -> Diagnostic {
        self.file = file;
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// The sink every stage reports into.
#[derive(Debug, Default, Clone)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
    errors: usize,
}

impl Diagnostics {
    pub fn new() -> Diagnostics {
        Diagnostics::default()
    }

    pub fn push(&mut self, d: Diagnostic) {
        if d.is_error() {
            self.errors += 1;
        }
        self.items.push(d);
    }

    pub fn error(&mut self, code: &'static str, message: impl Into<String>, span: Span) {
        self.push(Diagnostic::error(code, message, span));
    }

    pub fn has_errors(&self) -> bool {
        self.errors > 0
    }

    pub fn error_count(&self) -> usize {
        self.errors
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.items.iter()
    }

    /// Sort by file and position so output is deterministic regardless of the stage order.
    pub fn sorted(&self) -> Vec<&Diagnostic> {
        let mut v: Vec<&Diagnostic> = self.items.iter().collect();
        v.sort_by_key(|d| (d.file, d.span.start, d.span.end, d.code));
        v
    }

    /// Moves every diagnostic of `other` into `self`, tagging it with `file`.
    pub fn absorb(&mut self, other: Diagnostics, file: u32) {
        for d in other.items {
            self.push(d.in_file(file));
        }
    }

    pub fn warning_count(&self) -> usize {
        self.items.len() - self.errors
    }
}
