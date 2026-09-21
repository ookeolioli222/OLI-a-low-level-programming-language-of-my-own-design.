use crate::{lex, Kw, Prim, TokenKind};
use oli_diag::{Diagnostics, SourceFile};

fn lex_all(src: &str) -> (Vec<TokenKind>, Vec<&'static str>) {
    let file = SourceFile::new("t.oli", src);
    let mut diags = Diagnostics::new();
    let toks = lex(&file, &mut diags);
    let kinds = toks.into_iter().map(|t| t.kind).collect();
    let codes = diags.iter().map(|d| d.code).collect();
    (kinds, codes)
}

fn kinds(src: &str) -> Vec<TokenKind> {
    let (k, codes) = lex_all(src);
    assert!(
        codes.is_empty(),
        "unexpected diagnostics {codes:?} for {src:?}"
    );
    k
}

fn ident(s: &str) -> TokenKind {
    TokenKind::Ident(s.to_string())
}

#[test]
fn words_are_classified() {
    assert_eq!(
        kinds("proc u8 foo _bar end zone x64 const"),
        vec![
            TokenKind::Kw(Kw::Proc),
            TokenKind::Prim(Prim::U8),
            ident("foo"),
            ident("_bar"),
            TokenKind::Kw(Kw::End),
            TokenKind::Kw(Kw::Zone),
            ident("x64"),
            ident("const"), // reserved for later: an identifier the parser refuses to declare
            TokenKind::Eof,
        ]
    );
    for (w, k) in [("wrap", Kw::Wrap), ("never", Kw::Never), ("mmio", Kw::Mmio)] {
        assert_eq!(Kw::from_word(w), Some(k));
        assert_eq!(k.as_str(), w);
    }
    assert_eq!(Prim::from_word("physaddr"), Some(Prim::Physaddr));
    assert_eq!(Prim::Uword.as_str(), "uword");
}

#[test]
fn integers() {
    assert_eq!(
        kinds("0 42 1_000 0xFF 0b1010 0o17 64K 2M 1G 0X1f"),
        vec![
            TokenKind::Int(0),
            TokenKind::Int(42),
            TokenKind::Int(1000),
            TokenKind::Int(255),
            TokenKind::Int(10),
            TokenKind::Int(15),
            TokenKind::Int(65536),
            TokenKind::Int(2 << 20),
            TokenKind::Int(1 << 30),
            TokenKind::Int(31),
            TokenKind::Eof,
        ]
    );
    assert_eq!(
        kinds("0xFFFFFFFF80100000"),
        vec![TokenKind::Int(0xFFFF_FFFF_8010_0000), TokenKind::Eof]
    );
}

#[test]
fn integer_errors() {
    let (k, codes) = lex_all("340282366920938463463374607431768211456");
    assert_eq!(codes, vec!["E0003"]);
    assert_eq!(k, vec![TokenKind::Int(u128::MAX), TokenKind::Eof]);

    let (k, codes) = lex_all("0x");
    assert_eq!(codes, vec!["E0004"]);
    assert_eq!(k, vec![TokenKind::Int(0), TokenKind::Eof]);

    let (k, codes) = lex_all("12abc 0x10K 7T");
    assert_eq!(codes, vec!["E0004", "E0004", "E0004"]);
    assert_eq!(
        k,
        vec![
            TokenKind::Int(12),
            TokenKind::Int(16),
            TokenKind::Int(7),
            TokenKind::Eof
        ]
    );

    let (_, codes) = lex_all("99999999999999999999999999999999999999K");
    assert_eq!(codes, vec!["E0003"]);
}

#[test]
fn strings() {
    let src = r#""Hello Oli--\n" "tab\there" "q\"x\x41\\" "" "żółw""#;
    let k = kinds(src);
    assert_eq!(k.first(), Some(&TokenKind::Str(b"Hello Oli--\n".to_vec())));
    assert_eq!(k.get(1), Some(&TokenKind::Str(b"tab\there".to_vec())));
    assert_eq!(
        k.get(2),
        Some(&TokenKind::Str(vec![b'q', b'"', b'x', b'A', b'\\']))
    );
    assert_eq!(k.get(3), Some(&TokenKind::Str(Vec::new())));
    assert_eq!(k.get(4), Some(&TokenKind::Str("żółw".as_bytes().to_vec())));
}

#[test]
fn string_errors() {
    let (k, codes) = lex_all("\"abc\nx");
    assert_eq!(codes, vec!["E0002"]);
    assert_eq!(
        k,
        vec![
            TokenKind::Str(b"abc".to_vec()),
            TokenKind::Newline,
            ident("x"),
            TokenKind::Eof
        ]
    );

    let (k, codes) = lex_all(r#""a\qb""#);
    assert_eq!(codes, vec!["E0006"]);
    assert_eq!(k, vec![TokenKind::Str(b"aqb".to_vec()), TokenKind::Eof]);

    let (_, codes) = lex_all(r#""\x4""#);
    assert_eq!(codes, vec!["E0006"]);

    let (_, codes) = lex_all("\"unterminated at eof");
    assert_eq!(codes, vec!["E0002"]);
}

#[test]
fn chars() {
    assert_eq!(
        kinds(r"'a' '\n' '\x41' '\'' ' '"),
        vec![
            TokenKind::Char(b'a'),
            TokenKind::Char(b'\n'),
            TokenKind::Char(0x41),
            TokenKind::Char(b'\''),
            TokenKind::Char(b' '),
            TokenKind::Eof,
        ]
    );
    let (_, codes) = lex_all("''");
    assert_eq!(codes, vec!["E0005"]);
    let (_, codes) = lex_all("'ab'");
    assert_eq!(codes, vec!["E0005"]);
    let (_, codes) = lex_all("'ż'");
    assert_eq!(codes, vec!["E0005"]);
    let (_, codes) = lex_all("'");
    assert_eq!(codes, vec!["E0005"]);
}

#[test]
fn comments_and_docs() {
    let k = kinds(
        "x -- comment\n--- doc text  \n---- not a doc\ny -- \"not a string\"\n\"--not a comment\"",
    );
    assert_eq!(
        k,
        vec![
            ident("x"),
            TokenKind::Newline,
            TokenKind::Doc("doc text".to_string()),
            TokenKind::Newline,
            ident("y"),
            TokenKind::Newline,
            TokenKind::Str(b"--not a comment".to_vec()),
            TokenKind::Eof,
        ]
    );
    assert_eq!(
        kinds("---"),
        vec![TokenKind::Doc(String::new()), TokenKind::Eof]
    );
}

#[test]
fn operators_maximal_munch() {
    assert_eq!(
        kinds("a <- b < -c x<~y p->q a..b a.b := == != <= >= << >> & | ^ ~ %"),
        vec![
            ident("a"),
            TokenKind::Store,
            ident("b"),
            TokenKind::Lt,
            TokenKind::Minus,
            ident("c"),
            ident("x"),
            TokenKind::Move,
            ident("y"),
            ident("p"),
            TokenKind::Arrow,
            ident("q"),
            ident("a"),
            TokenKind::DotDot,
            ident("b"),
            ident("a"),
            TokenKind::Dot,
            ident("b"),
            TokenKind::Bind,
            TokenKind::EqEq,
            TokenKind::Ne,
            TokenKind::Le,
            TokenKind::Ge,
            TokenKind::Shl,
            TokenKind::Shr,
            TokenKind::Amp,
            TokenKind::Pipe,
            TokenKind::Caret,
            TokenKind::Tilde,
            TokenKind::Percent,
            TokenKind::Eof,
        ]
    );
    assert_eq!(
        kinds("()[]{}:,"),
        vec![
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::Colon,
            TokenKind::Comma,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn newlines_collapse_and_crlf() {
    assert_eq!(
        kinds("\n\n  a\r\n\r\n\n b \n"),
        vec![
            ident("a"),
            TokenKind::Newline,
            ident("b"),
            TokenKind::Newline,
            TokenKind::Eof
        ]
    );
    assert_eq!(kinds(""), vec![TokenKind::Eof]);
    assert_eq!(kinds("   \t\n"), vec![TokenKind::Eof]);
}

#[test]
fn invalid_characters() {
    let (k, codes) = lex_all("x = 1; y ż ! @");
    assert_eq!(codes, vec!["E0001", "E0001", "E0001", "E0001", "E0001"]);
    assert_eq!(
        k,
        vec![ident("x"), TokenKind::Int(1), ident("y"), TokenKind::Eof]
    );
    let file = SourceFile::new("t.oli", "x = 1");
    let mut diags = Diagnostics::new();
    lex(&file, &mut diags);
    let d = diags
        .iter()
        .next()
        .map(|d| (d.span.start, d.span.end, d.notes.len()));
    assert_eq!(d, Some((2, 3, 1)));
}

#[test]
fn spans_are_exact() {
    let file = SourceFile::new("t.oli", "ab := 0x10\n  \"s\"");
    let mut diags = Diagnostics::new();
    let toks = lex(&file, &mut diags);
    let spans: Vec<(u32, u32)> = toks.iter().map(|t| (t.span.start, t.span.end)).collect();
    assert_eq!(
        spans,
        vec![(0, 2), (3, 5), (6, 10), (10, 11), (13, 16), (16, 16)]
    );
}

#[test]
fn hostile_inputs_do_not_panic() {
    for src in [
        r"\",
        r#""\"#,
        r"'\",
        r"'\x",
        "0x",
        "0b_",
        "--",
        "---",
        r#"""#,
        "'",
        "\u{0}",
        "\u{FEFF}x",
        "1K2M",
        "..",
        ". .",
        "\r",
        r#""\x4z""#,
        r"'\'",
    ] {
        let _ = lex_all(src);
    }
}
