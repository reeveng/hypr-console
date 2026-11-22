//! Byte offsets over one normalized source text: lines, `proc_macro2` positions and literals.
//!
//! Every pass that reads syntax with `syn` and edits text meets the same question: where in the
//! bytes is the thing a span names. `proc_macro2` answers in lines and character columns, the
//! compiler in bytes. This is the one conversion, and the one list of where literals are, so no
//! edit lands inside a string.

use proc_macro2::{LineColumn, TokenStream, TokenTree};
use std::str::FromStr;

pub struct Text<'a> {
    pub text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Text<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            text.bytes()
                .enumerate()
                .filter_map(|(at, byte)| (byte == b'\n').then_some(at + 1)),
        );
        Self { text, starts }
    }

    pub fn offset(&self, position: LineColumn) -> usize {
        let line = position.line.saturating_sub(1);
        let start = self.starts.get(line).copied().unwrap_or(self.text.len());
        let rest = &self.text[start..];
        let end = rest.find('\n').unwrap_or(rest.len());
        rest[..end]
            .char_indices()
            .map(|(at, _)| at)
            .chain([end])
            .nth(position.column)
            .map_or(start + end, |at| start + at)
    }

    pub fn span(&self, span: proc_macro2::Span) -> (usize, usize) {
        (self.offset(span.start()), self.offset(span.end()))
    }

    pub fn line_of(&self, offset: usize) -> usize {
        match self.starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next.saturating_sub(1),
        }
    }

    pub fn lines(&self) -> usize {
        self.starts.len()
    }

    pub fn line_start(&self, line: usize) -> usize {
        self.starts.get(line).copied().unwrap_or(self.text.len())
    }

    pub fn line_end(&self, line: usize) -> usize {
        match self.starts.get(line + 1) {
            Some(next) => next - 1,
            None => self.text.len(),
        }
    }

    pub fn line(&self, line: usize) -> &'a str {
        let start = self.line_start(line);
        let end = self.line_end(line).max(start);
        &self.text[start..end]
    }

    pub fn indent_of_line(&self, line: usize) -> &'a str {
        let text = self.line(line);
        &text[..text.len() - text.trim_start().len()]
    }

    pub fn indent_at(&self, offset: usize) -> &'a str {
        self.indent_of_line(self.line_of(offset))
    }

    pub fn starts_line(&self, offset: usize) -> bool {
        let line = self.line_of(offset);
        self.text[self.line_start(line)..offset].trim().is_empty()
    }

    pub fn ends_line(&self, offset: usize) -> bool {
        let line = self.line_of(offset);
        self.text[offset..self.line_end(line).max(offset)].trim().is_empty()
    }
}

pub fn literals(text: &str) -> Vec<(usize, usize)> {
    fn walk(tokens: TokenStream, text: &Text<'_>, found: &mut Vec<(usize, usize)>) {
        for token in tokens {
            match token {
                TokenTree::Literal(literal) => found.push(text.span(literal.span())),
                TokenTree::Group(group) => walk(group.stream(), text, found),
                TokenTree::Ident(_) | TokenTree::Punct(_) => {}
            }
        }
    }
    let mut found = vec![];
    if let Ok(tokens) = TokenStream::from_str(text) {
        walk(tokens, &Text::new(text), &mut found);
    }
    found
}

pub fn inside_literal(literals: &[(usize, usize)], offset: usize) -> bool {
    literals.iter().any(|(lo, hi)| *lo < offset && offset < *hi)
}

pub fn reindent(text: &str, by: &str, literals: &[(usize, usize)], base: usize) -> String {
    let mut output = String::with_capacity(text.len());
    let mut at = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let inside = inside_literal(literals, base + at);
        if index > 0 && !inside && !line.trim().is_empty() {
            output.push_str(by);
        }
        output.push_str(line);
        at += line.len();
    }
    output
}

pub fn moved(text: &str, from: &str, to: &str) -> String {
    match from == to || !text.contains('\n') {
        true => text.to_owned(),
        false => {
            let literals = literals(text);
            let mut output = String::with_capacity(text.len());
            let mut at = 0;
            for (index, line) in text.split_inclusive('\n').enumerate() {
                let inside = inside_literal(&literals, at);
                match (index > 0 && !inside, line.strip_prefix(from)) {
                    (true, Some(rest)) => {
                        output.push_str(to);
                        output.push_str(rest);
                    }
                    _ => output.push_str(line),
                }
                at += line.len();
            }
            output
        }
    }
}

pub fn spans_literal_lines(text: &str, literals: &[(usize, usize)], lo: usize, hi: usize) -> bool {
    literals
        .iter()
        .filter(|(start, end)| lo <= *start && *end <= hi)
        .any(|(start, end)| text[*start..*end].contains('\n'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_characters_and_offsets_are_bytes() {
        let text = Text::new("é = 1;\nfn café() {}\n");
        let at = text.offset(LineColumn { line: 2, column: 7 });
        assert_eq!(&text.text[at..at + 1], "(");
        assert_eq!(text.line_of(at), 1);
        assert_eq!(text.line(1), "fn café() {}");
    }

    #[test]
    fn a_moved_value_keeps_its_shape_at_the_new_depth() {
        let text = "f(\n            a,\n            \"x\n            y\",\n        )";
        assert_eq!(moved(text, "        ", "    "), "f(\n        a,\n        \"x\n            y\",\n    )");
    }

    #[test]
    fn literal_lines_are_not_reindented() {
        let text = "f(\"a\nb\",\nc)";
        let found = literals(text);
        assert_eq!(reindent(text, "  ", &found, 0), "f(\"a\nb\",\n  c)");
    }
}
