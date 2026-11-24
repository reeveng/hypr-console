use super::{LexError, Lexer, Tok};

impl<'input> Iterator for Lexer<'input> {
    type Item = std::result::Result<(usize, Tok, usize), LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.pending.pop_front() {
            Some(tok) => {
                return Some(tok);
            },
            ::core::option::Option::None => {},
        }

        if self.pos >= self.input.len() {
            return None;
        }



        match self.lex_arrow_and_label() {
            Some(res) => {
                return Some(res);
            },
            ::core::option::Option::None => {},
        }

        match self.lex_sep() {
            Some(sep) => {
                self.allow_header_direction = false;
                return Some(Ok(sep));
            },
            ::core::option::Option::None => {},
        }

        self.skip_ws();

        if self.pos >= self.input.len() {
            return None;
        }

        match self.lex_sep() {
            Some(sep) => {
                self.allow_header_direction = false;
                return Some(Ok(sep));
            },
            ::core::option::Option::None => {},
        }

        if self.allow_header_direction {
            match self.lex_direction() {
                Some(direction) => {
                    return Some(Ok(direction));
                },
                ::core::option::Option::None => {},
            }

            let start = self.pos;
            let rest = &self.input[start..];
            let value_len = rest
                .find(|ch| ch == ';' || super::is_ecmascript_trim_char(ch))
                .unwrap_or(rest.len());
            self.pos += rest.find(['\n', ';']).unwrap_or(rest.len());
            self.allow_header_direction = false;
            let span = crate::SourceSpan::new(start, start + value_len);
            return Some(Err({
                let error = LexError::with_span("invalid flowchart direction", span);
                #[cfg(feature = "editor-facts")]
                let error = error.expecting(
                    crate::EditorExpectedSyntaxKind::FlowchartDirectionValue,
                    span,
                );
                error
            }));
        }

        let start = self.pos;

        match self.lex_reserved_token_error() {
            Some(error) => {
                return Some(Err(error));
            },
            ::core::option::Option::None => {},
        }

        match self.lex_direction_stmt() {
            Some(tok) => {
                return Some(tok);
            },
            ::core::option::Option::None => {},
        }

        match self.lex_style_stmt() {
            Some(res) => {
                return Some(res);
            },
            ::core::option::Option::None => {},
        }

        match self.lex_classdef_stmt() {
            Some(res) => {
                return Some(res);
            },
            ::core::option::Option::None => {},
        }

        match self.lex_class_assign_stmt() {
            Some(res) => {
                return Some(res);
            },
            ::core::option::Option::None => {},
        }

        match self.lex_click_stmt() {
            Some(res) => {
                return Some(res);
            },
            ::core::option::Option::None => {},
        }

        match self.lex_link_style_stmt() {
            Some(res) => {
                return Some(res);
            },
            ::core::option::Option::None => {},
        }

        match self.lex_shape_data() {
            Some(res) => {
                return Some(res);
            },
            ::core::option::Option::None => {},
        }

        if self.starts_with_kw("flowchart-elk") {
            self.pos += "flowchart-elk".len();
            self.allow_header_direction = true;
            return Some(Ok((start, Tok::KwFlowchartElk, self.pos)));
        }

        if self.starts_with_kw("swimlane-beta") {
            self.pos += "swimlane-beta".len();
            self.allow_header_direction = true;
            return Some(Ok((start, Tok::KwSwimlane, self.pos)));
        }

        if self.starts_with_kw("flowchart") {
            self.pos += "flowchart".len();
            self.allow_header_direction = true;
            return Some(Ok((start, Tok::KwFlowchart, self.pos)));
        }

        if self.starts_with_kw("graph") {
            self.pos += "graph".len();
            self.allow_header_direction = true;
            return Some(Ok((start, Tok::KwGraph, self.pos)));
        }

        if self.starts_with_kw("subgraph") {
            self.pos += "subgraph".len();
            let keyword_end = self.pos;

            match self.lex_subgraph_header_after_keyword(start) {
                Some(header) => {
                    self.pending.push_back(header);
                },
                ::core::option::Option::None => {},
            }

            return Some(Ok((start, Tok::KwSubgraph, keyword_end)));
        }

        if self.starts_with_kw("end") {
            self.pos += "end".len();
            return Some(Ok((start, Tok::KwEnd, self.pos)));
        }

        match self.lex_style_sep() {
            Some(tok) => {
                return Some(Ok(tok));
            },
            ::core::option::Option::None => {},
        }

        match self.lex_node_label() {
            Some(res) => {
                return Some(res);
            },
            ::core::option::Option::None => {},
        }

        match self.lex_edge_id() {
            Some(tok) => {
                return Some(Ok(tok));
            },
            ::core::option::Option::None => {},
        }

        match self.lex_id() {
            Some(tok) => {
                return Some(Ok(tok));
            },
            ::core::option::Option::None => {},
        }

        match self.lex_amp() {
            Some(tok) => {
                return Some(Ok(tok));
            },
            ::core::option::Option::None => {},
        }


        self.pos += self.input[start..].chars().next().unwrap().len_utf8();
        Some(Err(LexError::with_span(
            format!("Unexpected character at {start}"),
            crate::SourceSpan::new(start, self.pos),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_markers_remain_identifier_tokens() {
        let source = "% %A A% A%%B %% %%tail\nC";
        let tokens = Lexer::new(source)
            .collect::<Result<Vec<_>, _>>()
            .expect("percent identifiers must tokenize");
        let ids = tokens
            .iter()
            .filter_map(|(start, token, end)| match token {
                Tok::Id(id) => {
                    assert_eq!(&source[*start..*end], id);
                    Some(id.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(ids, ["%", "%A", "A%", "A%%B", "%%", "%%tail", "C"]);
        assert!(tokens.iter().any(|(_, token, _)| matches!(token, Tok::Sep)));
    }

    #[test]
    fn unsupported_unicode_recovery_keeps_character_boundaries() {
        let mut lexer = Lexer::new("😀A");
        assert!(lexer.next().unwrap().is_err());
        let (start, token, end) = lexer.next().unwrap().unwrap();
        assert!(matches!(token, Tok::Id(ref id) if id == "A"));
        assert_eq!((start, end), (4, 5));
        assert!(lexer.next().is_none());
    }
}
