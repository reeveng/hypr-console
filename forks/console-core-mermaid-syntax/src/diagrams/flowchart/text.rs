use super::{LexError, TitleKind};
use crate::{OperationControl, OperationControlResult, SourceSpan};
use std::collections::{BTreeMap, HashMap};

#[derive(Default)]
pub(super) struct LabelStringState {
    stack: Vec<TitleKind>,
}

impl LabelStringState {
    #[must_use]
    pub(super) fn is_plain(&self) -> bool {
        self.stack.is_empty()
    }

    pub(super) fn consume_delimiter(&mut self, rest: &str) -> Option<usize> {
        if self.stack.last() == Some(&TitleKind::Markdown) && rest.starts_with("`\"") {
            self.stack.pop();
            return Some(2);
        }

        if rest.starts_with("\"`") {
            self.stack.push(TitleKind::Markdown);
            return Some(2);
        }

        if rest.starts_with('"') {
            if self.stack.last() == Some(&TitleKind::String) {
                self.stack.pop();
            } else {
                self.stack.push(TitleKind::String);
            }

            return Some(1);
        }

        None
    }
}

#[derive(Default)]
pub(super) struct LabelScanner {
    quote_ends: HashMap<usize, Option<usize>>,

    delim_ends: HashMap<&'static str, BTreeMap<usize, PlainDelimiterRange>>,
    control: Option<OperationControl>,
    steps_since_checkpoint: usize,
    #[cfg(test)]
    pub(super) work: usize,
}

struct PlainDelimiterRange {
    end: usize,
    found: Option<usize>,
}

impl LabelScanner {
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(super) fn set_control(&mut self, control: &OperationControl) {
        self.control = Some(control.clone());
    }

    fn scanned(&mut self) -> OperationControlResult<()> {
        #[cfg(test)]
        {
            self.work += 1;
        }
        self.checkpoint()
    }

    fn checkpoint(&mut self) -> OperationControlResult<()> {
        self.steps_since_checkpoint += 1;

        match self.steps_since_checkpoint {
            4096 => {
                self.steps_since_checkpoint = 0;

                match &self.control {
                    Some(control) => {
                        control.checkpoint()?;
                    },
                    ::core::option::Option::None => {},
                }
            },
            _ => {},
        }

        Ok(())
    }

    pub(super) fn find_unquoted_delim(
        &mut self,
        input: &str,
        start: usize,
        delim: &'static str,
    ) -> OperationControlResult<Option<usize>> {
        let mut ranges = self.delim_ends.remove(delim).unwrap_or_default();

        while ranges
            .first_key_value()
            .is_some_and(|(_, range)| range.end <= start)
        {
            self.checkpoint()?;
            ranges.pop_first();
        }

        let mut pos = start;
        let mut plain_ranges = Vec::new();
        let (found, final_range) = 'search: loop {
            if pos + delim.len() > input.len() {
                break (None, None);
            }

            if let Some((_, range)) = ranges.range(..=pos).next_back()
                && pos < range.end
            {
                let found = range.found;
                self.scanned()?;
                break (found, None);
            }

            let boundary = ranges
                .range(pos..)
                .next()
                .map_or(input.len(), |(start_2, _)| *start_2);
            let range_start = pos;

            while pos < boundary && pos + delim.len() <= input.len() {
                self.scanned()?;
                let rest = &input[pos..];

                if rest.starts_with(delim) {
                    break 'search (Some(pos), Some((range_start, pos + 1)));
                }

                if rest.starts_with('"') {


                    plain_ranges.push((range_start, pos + 1));

                    let end = {
                        let optional_end = self.quoted_end(input, pos)?;

                        match optional_end {
                            Some(matched_end) => matched_end,
                            ::core::option::Option::None => break 'search (None, None),
                        }
                    };

                    pos = end;
                    continue 'search;
                }

                pos += rest
                    .chars()
                    .next()
                    .expect("label scan character")
                    .len_utf8();
            }

            let range = (range_start < pos).then_some((range_start, pos));

            if pos + delim.len() > input.len() {
                break (None, range);
            }

            match range {
                Some(range_2) => {
                    plain_ranges.push(range_2);
                },
                ::core::option::Option::None => {},
            }
        };



        for (start_3, end) in plain_ranges.into_iter().chain(final_range) {
            self.checkpoint()?;
            ranges.insert(start_3, PlainDelimiterRange { end, found });
        }

        self.delim_ends.insert(delim, ranges);
        Ok(found)
    }

    fn quoted_end(&mut self, input: &str, start: usize) -> OperationControlResult<Option<usize>> {
        let mut state = LabelStringState::default();
        let mut frames = Vec::new();
        let mut pos = start;

        while pos < input.len() {
            self.scanned()?;
            let rest = &input[pos..];
            let depth = state.stack.len();

            if let Some(consumed) = state.consume_delimiter(rest) {
                if state.stack.len() > depth {
                    match self.quote_ends.get(&pos).copied() {
                        Some(Some(end)) => {
                            state.stack.pop();
                            pos = end;

                            if state.is_plain() {
                                return Ok(Some(end));
                            }

                            continue;
                        }
                        Some(None) => break,
                        None => frames.push(pos),
                    }
                } else {
                    let opener = frames.pop().expect("closing label string frame");
                    self.quote_ends.insert(opener, Some(pos + consumed));

                    if state.is_plain() {
                        return Ok(Some(pos + consumed));
                    }
                }

                pos += consumed;
            } else {
                pos += rest
                    .chars()
                    .next()
                    .expect("quoted label character")
                    .len_utf8();
            }
        }

        for opener in frames {
            self.scanned()?;
            self.quote_ends.insert(opener, None);
        }

        Ok(None)
    }
}

#[must_use]
pub(super) fn borrowed_span(source: &str, slice: &str) -> SourceSpan {
    let start = slice.as_ptr() as usize - source.as_ptr() as usize;
    debug_assert!(start <= source.len() && slice.len() <= source.len() - start);
    SourceSpan::new(start, start + slice.len())
}

pub(super) fn parse_quoted_label(raw: &str) -> Result<(&str, TitleKind), LexError> {
    let invalid = || LexError::new("Invalid string token sequence in flowchart label");
    let mut state = LabelStringState::default();
    let mut pos = 0;
    let mut token = None;

    while pos < raw.len() {
        let rest = &raw[pos..];

        match state.consume_delimiter(rest) {
            Some(consumed) => {
                pos += consumed;
                continue;
            },
            ::core::option::Option::None => {},
        }

        let kind = state.stack.last().ok_or_else(invalid)?;
        let len = match kind {
            TitleKind::Markdown => rest.find(['`', '"']).unwrap_or(rest.len()),
            TitleKind::String => rest.find('"').unwrap_or(rest.len()),
            TitleKind::Text => unreachable!(),
        };

        if len == 0 || token.is_some() {
            return Err(invalid());
        }

        token = Some((&rest[..len], kind.clone()));
        pos += len;
    }

    if !state.is_plain() {
        return Err(invalid());
    }

    token.ok_or_else(invalid)
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
#[must_use]
pub(super) fn title_kind_str(kind: &TitleKind) -> &'static str {
    match kind {
        TitleKind::Text => "text",
        TitleKind::String => "string",
        TitleKind::Markdown => "markdown",
    }
}

#[must_use]
pub(super) fn unquote(s: &str) -> String {
    if s.starts_with('"')
        && let Ok((payload, kind)) = parse_quoted_label(s)
    {
        return match kind {
            TitleKind::Markdown => format!("`{payload}`"),
            TitleKind::Text | TitleKind::String => payload.to_string(),
        };
    }

    let bytes = s.as_bytes();

    if (match <[_]>::first_chunk::<2>(&*bytes) { ::core::option::Option::Some(&[checked_element_0, _]) => checked_element_0 == b'"', ::core::option::Option::None => false }) && bytes[bytes.len() - 1] == b'"' {
        return s[1..s.len() - 1].to_string();
    }

    s.to_string()
}

#[must_use]
pub(super) fn is_ecmascript_trim_char(ch: char) -> bool {
    crate::diagrams::scan::is_ecmascript_whitespace(ch)
}

#[must_use]
pub(super) fn trim_flowdb_label_text(text: &str) -> &str {




    text.trim_matches(is_ecmascript_trim_char)
}

#[must_use]
pub(super) fn parse_label_text(raw: &str) -> (String, TitleKind) {
    if raw.starts_with('"')
        && let Ok((payload, kind)) = parse_quoted_label(raw)
    {
        return (trim_flowdb_label_text(payload).to_string(), kind);
    }




    let quoted = raw.starts_with('"') && raw.ends_with('"');
    let unquoted = if quoted {



        unquote(raw)
    } else {
        raw.to_string()
    };

    if quoted {


        let (no_backticks, is_markdown) = strip_wrapping_backticks(&unquoted);

        if is_markdown {
            return (
                trim_flowdb_label_text(&no_backticks).to_string(),
                TitleKind::Markdown,
            );
        }

        return (
            trim_flowdb_label_text(&unquoted).to_string(),
            TitleKind::String,
        );
    }

    (
        trim_flowdb_label_text(&unquoted).to_string(),
        TitleKind::Text,
    )
}

#[must_use]
pub(super) fn parse_edge_label_text(raw: &str) -> (String, TitleKind) {
    let quoted = raw.starts_with('"') && raw.ends_with('"');

    if quoted {
        return parse_label_text(raw);
    }






    (trim_flowdb_label_text(raw).to_string(), TitleKind::Text)
}

#[must_use]
pub(super) fn strip_wrapping_backticks(s: &str) -> (String, bool) {
    if s.len() >= 2 && s.starts_with('`') && s.ends_with('`') {
        return (s[1..s.len() - 1].to_string(), true);
    }

    (s.to_string(), false)
}

#[cfg(test)]
mod tests {
    use super::{LabelScanner, parse_edge_label_text, parse_label_text};
    use crate::diagrams::flowchart::TitleKind;

    #[test]
    fn ordinary_label_scans_reuse_distant_successful_results() {
        for delim in [
            "/]", "\\]", "]]", "]", ")", "))", ")))", "-)", "])", ")]", "}", "}}", "|",
        ] {
            let prefix = "x\n".repeat(256);
            let source = format!("{prefix}{delim}");
            let mut scanner = LabelScanner::default();
            for start in 0..=prefix.len() {
                assert_eq!(
                    scanner.find_unquoted_delim(&source, start, delim).unwrap(),
                    Some(prefix.len()),
                );
            }
            assert!(
                scanner.work <= 3 * source.len(),
                "successful {delim:?} searches must reuse the scanned prefix: {} steps for {} bytes",
                scanner.work,
                source.len(),
            );
        }
    }

    #[test]
    fn ordinary_label_scans_keep_short_label_cache_bounded() {
        for (statement, offset, delims) in [
            ("A[x]\n", 2, &["]"][..]),
            ("A[ordinary label]\n", 2, &["]"][..]),
            ("A[/x/]\n", 3, &["/]", "\\]"][..]),
        ] {
            let source = statement.repeat(256);
            let mut scanner = LabelScanner::default();
            for index in 0..256 {
                let start = index * statement.len() + offset;
                for delim in delims {
                    scanner.find_unquoted_delim(&source, start, delim).unwrap();
                }
                assert_eq!(
                    scanner
                        .delim_ends
                        .values()
                        .map(|ranges| ranges.len())
                        .sum::<usize>(),
                    delims.len(),
                    "ordinary short labels retain only the current range per delimiter",
                );
            }
        }
    }

    #[test]
    fn ordinary_label_scans_do_not_reuse_outer_results_inside_quotes() {
        for quoted in [
            "\"雪😀 /] \\]\r\nlast\"",
            "\"`雪😀 /] \\]\r\nlast`\"",
            "\"`雪😀 /] \\]\r\nlast",
        ] {
            for suffix in ["", " /] \\]"] {
                let source = format!("first {quoted}{suffix}");
                for delim in ["/]", "\\]"] {
                    let mut scanner = LabelScanner::default();
                    let expected = if quoted.ends_with('"') && !suffix.is_empty() {
                        source.rfind(delim)
                    } else {
                        None
                    };
                    assert_eq!(
                        scanner.find_unquoted_delim(&source, 0, delim).unwrap(),
                        expected,
                        "{source:?}",
                    );
                    let interior = source.find(delim).unwrap();
                    assert_eq!(
                        scanner
                            .find_unquoted_delim(&source, interior, delim)
                            .unwrap(),
                        Some(interior),
                        "recovery starts in plain state inside the earlier quote: {source:?}",
                    );
                }
            }
        }
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    #[test]
    fn ordinary_label_scans_observe_cancellation_on_cached_results() {
        let source = format!("{}]", "x".repeat(256));
        let mut scanner = LabelScanner::default();
        assert_eq!(
            scanner.find_unquoted_delim(&source, 0, "]").unwrap(),
            Some(256),
        );
        let control = crate::OperationControl::new().for_phase(crate::OperationPhase::Parse);
        control.cancel_after_checkpoints(0);
        scanner.set_control(&control);
        assert!((0..4096).any(|_| scanner.find_unquoted_delim(&source, 0, "]").is_err()));
        assert!(control.is_cancelled());
        assert!(scanner.work <= 4096);
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    #[test]
    fn ordinary_label_scans_observe_cancellation_while_recording_ranges() {
        let source = format!("{}]", "\"x\"".repeat(900));
        let mut scanner = LabelScanner::default();
        let control = crate::OperationControl::new().for_phase(crate::OperationPhase::Parse);
        control.cancel_after_checkpoints(0);
        scanner.set_control(&control);
        assert!(scanner.find_unquoted_delim(&source, 0, "]").is_err());
        assert!(control.is_cancelled());
        assert_eq!(scanner.work, 3601);
    }

    #[test]
    fn parse_label_text_keeps_backslashes_in_string_labels() {
        let (text, kind) = parse_label_text(r#""Path: C:\\Temp\\merman\\out.svg (Windows-style)""#);
        assert_eq!(kind, TitleKind::String);
        assert_eq!(text, r#"Path: C:\\Temp\\merman\\out.svg (Windows-style)"#);
    }

    #[test]
    fn parse_label_text_does_not_treat_tex_commands_as_escapes() {
        let (text, kind) = parse_label_text(r#""$$\nabla\therefore\alpha$$""#);
        assert_eq!(kind, TitleKind::String);
        assert_eq!(text, r#"$$\nabla\therefore\alpha$$"#);
    }

    #[test]
    fn parse_label_text_keeps_single_quotes_as_text() {
        let (text, kind) = parse_label_text("'Literal quotes'");
        assert_eq!(kind, TitleKind::Text);
        assert_eq!(text, "'Literal quotes'");
    }

    #[test]
    fn parse_label_text_keeps_bare_backticks_as_text() {
        let (text, kind) = parse_label_text("bare `tick` text");
        assert_eq!(kind, TitleKind::Text);
        assert_eq!(text, "bare `tick` text");
    }

    #[test]
    fn parse_edge_label_text_keeps_unquoted_backticks_literal() {
        let (text, kind) =
            parse_edge_label_text(r#"`This is **bold** </br>and <strong>strong</strong>`"#);
        assert_eq!(kind, TitleKind::Text);
        assert_eq!(
            text,
            r#"`This is **bold** </br>and <strong>strong</strong>`"#
        );
    }

    #[test]
    fn parse_edge_label_text_keeps_unquoted_partial_markdown_literal() {
        let (text, kind) = parse_edge_label_text(r#"`**bold*`"#);
        assert_eq!(kind, TitleKind::Text);
        assert_eq!(text, r#"`**bold*`"#);
    }

    #[test]
    fn parse_edge_label_text_supports_quoted_markdown_strings() {
        let (text, kind) = parse_edge_label_text(r#""`Bold **edge label**`""#);
        assert_eq!(kind, TitleKind::Markdown);
        assert_eq!(text, r#"Bold **edge label**"#);
    }

    #[test]
    fn flowdb_label_trim_preserves_entity_provenance_for_nodes_and_edges() {
        let cases = [
            ("\u{00a0}A\u{00a0}", "A", TitleKind::Text),
            ("\"\u{00a0}A\u{00a0}\"", "A", TitleKind::String),
            ("\"`\u{00a0}A\u{00a0}`\"", "A", TitleKind::Markdown),
            (
                "\"\u{00a0}&nbsp;A&nbsp;\u{00a0}\"",
                "&nbsp;A&nbsp;",
                TitleKind::String,
            ),
            (
                "\"`\u{00a0}&nbsp;A&nbsp;\u{00a0}`\"",
                "&nbsp;A&nbsp;",
                TitleKind::Markdown,
            ),
            ("\"\u{feff}A\u{feff}\"", "A", TitleKind::String),
            (
                "\"\u{0085}A\u{0085}\"",
                "\u{0085}A\u{0085}",
                TitleKind::String,
            ),
            ("\" `A` \"", "`A`", TitleKind::String),
            ("\"\u{00a0}`A`\u{00a0}\"", "`A`", TitleKind::String),
            (
                "\"\u{0085}`A`\u{0085}\"",
                "\u{0085}`A`\u{0085}",
                TitleKind::String,
            ),
            ("\"` A `\"", "A", TitleKind::Markdown),
        ];

        for (raw, expected, expected_kind) in cases {
            let (node_text, node_kind) = parse_label_text(raw);
            assert_eq!(node_text, expected, "node label {raw:?}");
            assert_eq!(node_kind, expected_kind, "node label {raw:?}");

            let (edge_text, edge_kind) = parse_edge_label_text(raw);
            assert_eq!(edge_text, expected, "edge label {raw:?}");
            assert_eq!(edge_kind, expected_kind, "edge label {raw:?}");
        }
    }
}
