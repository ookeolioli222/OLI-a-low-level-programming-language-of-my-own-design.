//! `olic` — the Oli-- compiler driver.
//!
//! Phase 1 implements the front end only: `--show-tokens`, `--show-ast` and
//! `--check-syntax`. Anything further reports `E0900 feature not implemented`
//! instead of pretending.

use oli_diag::{Diagnostics, SourceFile, SourceMap};
use oli_lexer::TokenKind;
use oli_sema::{FsModuleSource, Target};
use std::io::{self, Write};
use std::process::ExitCode;

const USAGE: &str = "\
usage: olic <file.oli> [options]

options:
  --show-tokens     print the token stream and stop
  --show-ast        print the syntax tree and stop
  --show-sema       print the semantic graph (typed, resolved program) and stop
  --check-syntax    parse only; exit 0 when there are no syntax errors
  --check           parse and analyze; exit 0 when the program is valid
  --freestanding    target x86_64-freestanding (no OS, own `entry`)
  --lib <dir>       add a library directory (also: the OLI_LIB variable)
  --help            show this help
  --version         show the version

Phase 1b (front end + semantic analysis). Code generation is not implemented yet.";

#[derive(Default, Debug)]
struct Options {
    file: Option<String>,
    show_tokens: bool,
    show_ast: bool,
    show_sema: bool,
    check_syntax: bool,
    check: bool,
    freestanding: bool,
    lib_dirs: Vec<String>,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut o = Options::default();
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--show-tokens" => o.show_tokens = true,
            "--show-ast" => o.show_ast = true,
            "--show-sema" => o.show_sema = true,
            "--check-syntax" => o.check_syntax = true,
            "--check" => o.check = true,
            "--freestanding" => o.freestanding = true,
            "--lib" => match iter.next() {
                Some(d) => o.lib_dirs.push(d.clone()),
                None => return Err("`--lib` needs a directory".to_string()),
            },
            "--help" | "-h" => return Err(USAGE.to_string()),
            "--version" | "-V" => return Err(format!("olic {}", env!("CARGO_PKG_VERSION"))),
            s if s.starts_with('-') => return Err(format!("unknown option `{s}`\n\n{USAGE}")),
            s => {
                if o.file.is_some() {
                    return Err(format!("only one input file is accepted (got `{s}` too)"));
                }
                o.file = Some(s.to_string());
            }
        }
    }
    if o.file.is_none() {
        return Err(USAGE.to_string());
    }
    Ok(o)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // A generous stack keeps deep (but bounded) recursive descent safe on
    // platforms with a small main-thread stack.
    let handle = std::thread::Builder::new()
        .name("olic".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || run(&args));
    match handle.map(|h| h.join()) {
        Ok(Ok(code)) => code,
        _ => {
            eprintln!("olic: internal error: compiler thread failed");
            ExitCode::from(101)
        }
    }
}

fn run(args: &[String]) -> ExitCode {
    let opts = match parse_args(args) {
        Ok(o) => o,
        Err(msg) => {
            let is_help = msg == USAGE || msg.starts_with("olic ");
            if is_help {
                println!("{msg}");
                return ExitCode::SUCCESS;
            }
            eprintln!("olic: {msg}");
            return ExitCode::from(2);
        }
    };
    let Some(path) = opts.file.as_deref() else {
        return ExitCode::from(2);
    };
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("olic: cannot read `{path}`: {e}");
            return ExitCode::from(2);
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => {
            let at = e.utf8_error().valid_up_to();
            eprintln!("error[E0007]: `{path}` is not valid UTF-8 (first bad byte at offset {at})");
            return ExitCode::FAILURE;
        }
    };
    let file = SourceFile::new(path, text);
    let mut diags = Diagnostics::new();

    if opts.show_tokens {
        let tokens = oli_lexer::lex(&file, &mut diags);
        let mut out = String::new();
        for t in &tokens {
            let (line, col) = file.line_col(t.span.start);
            let text = match &t.kind {
                TokenKind::Newline => "<newline>".to_string(),
                TokenKind::Eof => "<eof>".to_string(),
                _ => file.slice(t.span).to_string(),
            };
            out.push_str(&format!(
                "{line:>4}:{col:<4} {:<8} {text}\n",
                t.kind.kind_name()
            ));
        }
        let _ = io::stdout().write_all(out.as_bytes());
        return report(&diags, &file);
    }

    if opts.show_ast || opts.check_syntax {
        let module = oli_parser::parse_source(&file, &mut diags);
        if opts.show_ast {
            let _ = io::stdout().write_all(oli_ast::print_module(&module).as_bytes());
        }
        return report(&diags, &file);
    }

    // Semantic analysis over the whole program (root + imported modules).
    let mut sources = SourceMap::new();
    let root_dir = std::path::Path::new(path)
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let mut lib_dirs: Vec<std::path::PathBuf> =
        opts.lib_dirs.iter().map(std::path::PathBuf::from).collect();
    lib_dirs.extend(FsModuleSource::default_lib_dirs());
    let source = FsModuleSource { root_dir, lib_dirs };
    let target = if opts.freestanding {
        Target::Freestanding
    } else {
        Target::Hosted
    };
    let program = oli_sema::analyze(
        file,
        &source,
        &oli_sema::Options { target },
        &mut sources,
        &mut diags,
    );
    if opts.show_sema {
        let _ = io::stdout().write_all(oli_sema::printer::print_program(&program).as_bytes());
        return report_all(&diags, &sources);
    }
    let code = report_all(&diags, &sources);
    if code != ExitCode::SUCCESS || opts.check {
        return code;
    }
    eprintln!(
        "error[E0900]: feature not implemented: code generation
         = note: `{path}` parsed and type-checked without errors; this build of olic stops after semantic analysis
         = note: use --show-tokens, --show-ast, --show-sema or --check"
    );
    ExitCode::FAILURE
}

/// Prints the diagnostics of a multi-file compilation, sorted by file and position.
fn report_all(diags: &Diagnostics, sources: &SourceMap) -> ExitCode {
    let mut err = io::stderr().lock();
    for d in diags.sorted() {
        if let Some(file) = sources.get(d.file) {
            let _ = err.write_all(oli_diag::render(d, file).as_bytes());
            let _ = err.write_all(
                b"
",
            );
        }
    }
    let n = diags.error_count();
    if n > 0 {
        let root = sources.get(0).map_or("<input>", |f| f.name());
        let _ = writeln!(
            err,
            "error: {n} error{} in `{root}`",
            if n == 1 { "" } else { "s" }
        );
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Prints all diagnostics to stderr, sorted by position; returns the exit code.
fn report(diags: &Diagnostics, file: &SourceFile) -> ExitCode {
    let mut err = io::stderr().lock();
    for d in diags.sorted() {
        let _ = err.write_all(oli_diag::render(d, file).as_bytes());
        let _ = err.write_all(b"\n");
    }
    let n = diags.error_count();
    if n > 0 {
        let _ = writeln!(
            err,
            "error: {n} error{} in `{}`",
            if n == 1 { "" } else { "s" },
            file.name()
        );
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
