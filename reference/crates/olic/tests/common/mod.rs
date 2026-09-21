//! Shared helpers for the integration tests.
#![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used, dead_code)]

use std::path::{Path, PathBuf};

/// Repository root: three levels above this crate's manifest directory
/// (`<root>/reference/crates/olic`). Language assets (`examples/`, `lib/`,
/// `tests/`) live at the root; the Rust reference lives under `reference/`.
pub fn repo_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| manifest.to_path_buf())
}

/// All `*.oli` files directly inside `dir` (relative to the repository root), sorted.
pub fn oli_files(dir: &str) -> Vec<PathBuf> {
    let dir = repo_root().join(dir);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "oli"))
        // Fixtures for features the reference does not implement (design 0015
        // hardware commands) are checked by the Oli-- compiler only.
        .filter(|p| {
            !std::fs::read_to_string(p)
                .unwrap_or_default()
                .lines()
                .any(|l| l.trim() == "-- reference: skip")
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .oli files in {}", dir.display());
    files
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Diagnostics of a file as `(code, line, col)`, sorted.
pub fn diagnostics(path: &Path) -> (oli_ast::Module, Vec<(String, u32, u32)>) {
    let text = read(path);
    let file = oli_diag::SourceFile::new(path.display().to_string(), text);
    let mut diags = oli_diag::Diagnostics::new();
    let module = oli_parser::parse_source(&file, &mut diags);
    let mut out: Vec<(String, u32, u32)> = diags
        .iter()
        .map(|d| {
            let (l, c) = file.line_col(d.span.start);
            (d.code.to_string(), l, c)
        })
        .collect();
    out.sort();
    (module, out)
}

/// Full analysis of a file (parse + semantics) with the repository `lib/`.
/// A first-line comment `-- target: freestanding` selects the target.
pub fn analyze_file(path: &Path) -> (oli_sema::Program, Vec<(String, u32, u32)>) {
    let text = read(path);
    let freestanding = text
        .lines()
        .next()
        .is_some_and(|l| l.trim() == "-- target: freestanding");
    let target = if freestanding {
        oli_sema::Target::Freestanding
    } else {
        oli_sema::Target::Hosted
    };
    let root_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let source = oli_sema::FsModuleSource {
        root_dir,
        lib_dirs: vec![repo_root().join("lib")],
    };
    let file = oli_diag::SourceFile::new(path.display().to_string(), text);
    let mut sources = oli_diag::SourceMap::new();
    let mut diags = oli_diag::Diagnostics::new();
    let program = oli_sema::analyze(
        file,
        &source,
        &oli_sema::Options { target },
        &mut sources,
        &mut diags,
    );
    let mut out: Vec<(String, u32, u32)> = diags
        .iter()
        .map(|d| {
            let (l, c) = sources
                .get(d.file)
                .map_or((0, 0), |f| f.line_col(d.span.start));
            (d.code.to_string(), l, c)
        })
        .collect();
    out.sort();
    (program, out)
}

// reference build marker 1
