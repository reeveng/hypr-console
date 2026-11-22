use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;

// An expression forwarded by a macro can retain its authored span even when
// another macro arm also stringifies that expression. Check its written context.
pub fn outside_macro_input(cx: &LateContext<'_>, span: Span) -> bool {
    let file = cx.sess().source_map().lookup_source_file(span.lo());
    let Some(prefix) = file
        .src
        .as_ref()
        .and_then(|source| source.get(..(span.lo() - file.start_pos).0 as usize))
    else {
        return false;
    };
    let mut delimiters = Vec::new();
    let mut after_name = false;
    let mut after_macro = false;
    for token in tokenize(prefix, FrontmatterAllowed::No) {
        use TokenKind::*;
        match token.kind {
            Whitespace | LineComment { .. } | BlockComment { .. } => continue,
            OpenBrace | OpenBracket | OpenParen => delimiters.push(after_macro),
            CloseBrace | CloseBracket | CloseParen => {
                delimiters.pop();
            }
            _ => {}
        }
        after_macro = after_name && token.kind == Bang;
        after_name = matches!(token.kind, Ident | RawIdent);
    }
    !delimiters.contains(&true)
}
