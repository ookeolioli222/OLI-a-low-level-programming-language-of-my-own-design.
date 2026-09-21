use crate::Span;

/// One source file: its name, its text and a line table for position lookup.
#[derive(Debug, Clone)]
pub struct SourceFile {
    name: String,
    text: String,
    /// Byte offset of the first character of every line.
    line_starts: Vec<u32>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> SourceFile {
        let text = text.into();
        let mut line_starts = vec![0u32];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        SourceFile {
            name: name.into(),
            text,
            line_starts,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn len(&self) -> u32 {
        self.text.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// 1-based line number of a byte offset.
    pub fn line_of(&self, offset: u32) -> u32 {
        let offset = offset.min(self.len());
        match self.line_starts.binary_search(&offset) {
            Ok(i) => i as u32 + 1,
            Err(i) => i as u32, // i >= 1 because line_starts[0] == 0
        }
    }

    /// 1-based (line, column) of a byte offset. Columns count characters.
    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        let line = self.line_of(offset);
        let start = self.line_start(line);
        let offset = offset.min(self.len());
        let col = self
            .text
            .get(start as usize..offset as usize)
            .map_or(1, |s| s.chars().count() + 1);
        (line, col as u32)
    }

    /// Byte offset where 1-based `line` starts.
    pub fn line_start(&self, line: u32) -> u32 {
        self.line_starts
            .get(line.saturating_sub(1) as usize)
            .copied()
            .unwrap_or(0)
    }

    /// Text of 1-based `line` without its line terminator.
    pub fn line_text(&self, line: u32) -> &str {
        let start = self.line_start(line) as usize;
        let end = self
            .line_starts
            .get(line as usize)
            .map_or(self.text.len(), |&next| next as usize);
        let raw = self.text.get(start..end).unwrap_or("");
        raw.trim_end_matches(['\n', '\r'])
    }

    pub fn line_count(&self) -> u32 {
        self.line_starts.len() as u32
    }

    /// Source text under a span (clamped; never panics).
    pub fn slice(&self, span: Span) -> &str {
        let start = (span.start as usize).min(self.text.len());
        let end = (span.end as usize).clamp(start, self.text.len());
        // Both bounds are clamped, but they might fall inside a multi-byte
        // character if a span was built from bad offsets; fall back to "".
        self.text.get(start..end).unwrap_or("")
    }
}

/// All source files of one compilation; a diagnostic's `file` indexes it.
#[derive(Debug, Default, Clone)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn new() -> SourceMap {
        SourceMap::default()
    }

    /// Adds a file and returns its index.
    pub fn add(&mut self, file: SourceFile) -> u32 {
        self.files.push(file);
        (self.files.len() - 1) as u32
    }

    pub fn get(&self, id: u32) -> Option<&SourceFile> {
        self.files.get(id as usize)
    }

    pub fn files(&self) -> &[SourceFile] {
        &self.files
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions() {
        let f = SourceFile::new("t.oli", "ab\ncd\r\nef");
        assert_eq!(f.line_col(0), (1, 1));
        assert_eq!(f.line_col(1), (1, 2));
        assert_eq!(f.line_col(3), (2, 1));
        assert_eq!(f.line_col(7), (3, 1));
        assert_eq!(f.line_text(2), "cd");
        assert_eq!(f.line_text(3), "ef");
        assert_eq!(f.line_count(), 3);
        assert_eq!(f.line_col(999), (3, 3));
    }

    #[test]
    fn unicode_columns() {
        let f = SourceFile::new("t.oli", "żółw x");
        // "żółw " is 5 chars, 8 bytes.
        assert_eq!(f.line_col(8), (1, 6));
        assert_eq!(f.slice(Span::new(8, 9)), "x");
        assert_eq!(f.slice(Span::new(1, 2)), "");
    }
}
