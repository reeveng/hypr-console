//! Find the written function when an attribute expansion gives it a generated span.

use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Span};

pub fn authored_function(cx: &LateContext<'_>, signature: Span, required: &[Span]) -> Option<Span> {
    let attribute = signature.source_callsite();
    let start = attribute.hi();
    let first = required.iter().map(|span| span.lo()).min()?;
    let map = cx.sess().source_map();
    if start >= first
        || !map
            .span_to_snippet(attribute)
            .ok()?
            .trim_start()
            .starts_with("#[")
    {
        return None;
    }
    let file = map.lookup_source_file(start);
    if first > file.end_position() {
        return None;
    }
    let text = file
        .src
        .as_ref()?
        .get((start - file.start_pos).0 as usize..)?;
    let mut position = start;
    let mut depth = 0usize;
    let mut saw_function = false;
    for token in tokenize(text, FrontmatterAllowed::No) {
        let from = (position - start).0 as usize;
        position = position + BytePos(token.len);
        match token.kind {
            TokenKind::Ident
                if depth == 0 && text.get(from..(position - start).0 as usize) == Some("fn") =>
            {
                saw_function = true;
            }
            TokenKind::OpenBrace => depth += 1,
            TokenKind::CloseBrace => {
                depth = depth.checked_sub(1)?;
                if depth == 0 && position > first && saw_function {
                    let written = attribute.with_lo(start).with_hi(position);
                    return required
                        .iter()
                        .all(|&span| written.contains(span))
                        .then_some(written);
                }
            }
            _ => {}
        }
    }
    None
}
