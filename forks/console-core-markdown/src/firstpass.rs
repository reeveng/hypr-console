//! The first pass resolves all block structure, generating an AST. Within a block, items
//! are in a linear chain with potential inline markup identified.

use std::cmp::max;
use std::ops::Range;

use crate::parse::{
    scan_containers, Allocations, FootnoteDef, HeadingAttributes, Item, ItemBody, LinkDef,
    LINK_MAX_NESTED_PARENS,
};
use crate::strings::CowStr;
use crate::tree::{Tree, TreeIndex};
use crate::Options;
use crate::{
    linklabel::{scan_link_label_rest, LinkLabel},
    HeadingLevel,
};
use crate::{scanners::*, MetadataBlockKind};
use core::cell::Cell;

use unicase::UniCase;

use crate::parse::ListFlags;

/// Runs the first pass, which resolves the block structure of the document,
/// and returns the resulting tree.
#[must_use]
pub(crate) fn run_first_pass(text: &str, options: Options) -> (Tree<Item>, Allocations<'_>) {


    let start_capacity = max(128, text.len().wrapping_div(32));
    let lookup_table = &special_bytes(&options);
    let first_pass = FirstPass {
        text,
        tree: Tree::with_capacity(start_capacity),
        begin_list_item: None,
        last_line_blank: false,
        allocs: Allocations::new(),
        options,
        lookup_table,
        brace_context_next: 0,
        brace_context_stack: Vec::new(),
    };
    first_pass.run()
}










const MATH_BRACE_CONTEXT_MAX_NESTING: usize = 25;

/// State for the first parsing pass.
struct FirstPass<'a, 'b> {
    text: &'a str,
    tree: Tree<Item>,
    begin_list_item: Option<usize>,
    last_line_blank: bool,
    allocs: Allocations<'a>,
    options: Options,
    lookup_table: &'b LookupTable,
    /// Math environment brace nesting.
    brace_context_stack: Vec<u8>,
    brace_context_next: usize,
}

impl<'a, 'b> FirstPass<'a, 'b> {
    #[must_use]
    fn run(mut self) -> (Tree<Item>, Allocations<'a>) {
        let mut ix = 0;

        while ix < self.text.len() {
            ix = self.parse_block(ix);
        }

        while self.tree.spine_len() > 0 {
            self.pop(ix);
        }

        (self.tree, self.allocs)
    }

    /// Returns offset after block.
    fn parse_block(&mut self, mut start_ix: usize) -> usize {
        let bytes = self.text.as_bytes();
        let mut line_start = LineStart::new(&bytes[start_ix..]);


        self.brace_context_stack.clear();
        self.brace_context_next = 0;

        let i = scan_containers(&self.tree, &mut line_start, self.options, true);

        for _ in i..self.tree.spine_len() {
            self.pop(start_ix);
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.options.contains(Options::ENABLE_OLD_FOOTNOTES), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {

                match self.tree.peek_up() {
                    Some(node_ix) => {
                        if let ItemBody::FootnoteDefinition(..) = self.tree[node_ix].item.body {
                            match {
                                let _explicit_decision = ::core::primitive::bool::then_some(self.last_line_blank, ());
                                _explicit_decision
                            } {
                                ::core::option::Option::Some(()) => {
                                    self.pop(start_ix);
                                },
                                ::core::option::Option::None => {},
                            }
                        }
                    },
                    ::core::option::Option::None => {},
                }
            },
            ::core::option::Option::None => {},
        }


        loop {
            let save = line_start.clone();
            let outer_indent = line_start.scan_space_upto(4);

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(outer_indent >= 4, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    line_start = save;
                    break;
                },
                ::core::option::Option::None => {},
            }

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(self.options.contains(Options::ENABLE_FOOTNOTES), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {

                    let container_start = start_ix + line_start.bytes_scanned();

                    match self.parse_footnote(container_start) {
                        Some(bytecount) => {
                            start_ix = container_start + bytecount;

                            match {
                                let _explicit_decision = ::core::primitive::bool::then_some(self.options.contains(Options::ENABLE_OLD_FOOTNOTES), ());
                                _explicit_decision
                            } {
                                ::core::option::Option::Some(()) => {


                                    start_ix += match scan_blank_line(&bytes[start_ix..]) {
                                        ::core::option::Option::Some(option_value) => option_value,
                                        ::core::option::Option::None => 0,
                                    };
                                },
                                ::core::option::Option::None => {},
                            }

                            line_start = LineStart::new(&bytes[start_ix..]);
                            continue;
                        },
                        ::core::option::Option::None => {},
                    }
                },
                ::core::option::Option::None => {},
            }

            let container_start = start_ix + line_start.bytes_scanned();

            match line_start.scan_list_marker_with_indent(outer_indent) {
                Some((ch, index, indent)) => {
                    let after_marker_index = start_ix + line_start.bytes_scanned();
                    self.continue_list(container_start - outer_indent, ch, index);
                    self.tree.append(Item {
                        start: container_start - outer_indent,
                        end: after_marker_index,
                        body: ItemBody::ListItem(indent),
                    });
                    self.tree.push();

                    match scan_blank_line(&bytes[after_marker_index..]) {
                        Some(n) => {
                            self.begin_list_item = Some(after_marker_index + n);
                            return after_marker_index + n;
                        },
                        ::core::option::Option::None => {},
                    }

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(self.options.contains(Options::ENABLE_TASKLISTS), ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            let task_list_marker =
                                line_start.scan_task_list_marker().map(|is_checked| Item {
                                    start: after_marker_index,
                                    end: start_ix + line_start.bytes_scanned(),
                                    body: ItemBody::TaskListMarker(is_checked),
                                });

                            match task_list_marker {
                                Some(task_list_marker_2) => {
                                    match scan_blank_line(&bytes[task_list_marker_2.end..]) {
                                        Some(n) => {
                                            let end = task_list_marker_2.end;
                                            self.tree.append(task_list_marker_2);
                                            self.begin_list_item = Some(end + n);
                                            return end + n;
                                        },
                                        ::core::option::Option::None => {
                                            line_start.scan_all_space();
                                            let ix = start_ix + line_start.bytes_scanned();
                                            return self.parse_paragraph(ix, Some(task_list_marker_2));
                                        },
                                    }
                                },
                                ::core::option::Option::None => {},
                            }
                        },
                        ::core::option::Option::None => {},
                    }
                },
                ::core::option::Option::None => match self
                    .options
                    .contains(Options::ENABLE_DEFINITION_LIST)
                    .then(|| {
                        self.tree
                            .cur()
                            .map(|cur| (self.tree[cur].child, &mut self.tree[cur].item))
                    })
                    .flatten()
                    .filter(|(_, item)| {
                        matches!(
                            item,
                            Item {
                                body: ItemBody::Paragraph
                                    | ItemBody::TightParagraph
                                    | ItemBody::MaybeDefinitionListTitle
                                    | ItemBody::DefinitionListDefinition(_),
                                ..
                            }
                        )
                    })
                    .and_then(|item| {
                        Some((
                            line_start
                                .scan_definition_list_definition_marker_with_indent(outer_indent)?,
                            item.0,
                            item.1,
                        ))
                    }) {
                    Some((indent, child, item)) => {
                        match item.body {
                            ItemBody::Paragraph | ItemBody::TightParagraph => {
                                item.body = ItemBody::DefinitionList(Cell::new(ListFlags::IS_TIGHT));
                                let Item { start, end, .. } = *item;
                                let list_idx = self.tree.cur().unwrap();
                                let title_idx = self.tree.create_node(Item {
                                    start,
                                    end,
                                    body: ItemBody::DefinitionListTitle,
                                });
                                self.tree[title_idx].child = child;
                                self.tree[list_idx].child = Some(title_idx);
                                self.tree.push();
                            }
                            ItemBody::MaybeDefinitionListTitle => {
                                item.body = ItemBody::DefinitionListTitle;
                            }
                            ItemBody::DefinitionListDefinition(_) => {}
                            _ => unreachable!(),
                        }

                        let after_marker_index = start_ix + line_start.bytes_scanned();


                        self.tree.append(Item {
                            start: start_ix + save.bytes_scanned(),
                            end: after_marker_index,
                            body: ItemBody::DefinitionListDefinition(indent),
                        });

                        if let Some(ItemBody::DefinitionList(ref list_flags)) =
                            self.tree.peek_up().map(|cur| &mut self.tree[cur].item.body)
                        {
                            match {
                                let _explicit_decision = ::core::primitive::bool::then_some(self.last_line_blank, ());
                                _explicit_decision
                            } {
                                ::core::option::Option::Some(()) => {
                                    list_flags.set(list_flags.get() & !ListFlags::IS_TIGHT);
                                    self.last_line_blank = false;
                                },
                                ::core::option::Option::None => {},
                            }
                        }

                        self.tree.push();

                        match scan_blank_line(&bytes[after_marker_index..]) {
                            Some(n) => {
                                self.begin_list_item = Some(after_marker_index + n);
                                return after_marker_index + n;
                            },
                            ::core::option::Option::None => {},
                        }
                    },
                    ::core::option::Option::None => match {
                        let _explicit_decision = ::core::primitive::bool::then_some(line_start.scan_blockquote_marker(), ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            let kind = match {
                                let _explicit_decision = ::core::primitive::bool::then_some(self.options.contains(Options::ENABLE_GFM), ());
                                _explicit_decision
                            } {
                                ::core::option::Option::Some(()) => {
                                    line_start.scan_blockquote_tag()
                                },
                                ::core::option::Option::None => {
                                    None
                                },
                            };
                            self.finish_list(start_ix);
                            self.tree.append(Item {
                                start: container_start,
                                end: 0,
                                body: ItemBody::BlockQuote(kind),
                            });
                            self.tree.push();

                            match kind {
                                ::core::option::Option::Some(_) => {



                                    let ix = start_ix + line_start.bytes_scanned();
                                    let mut lazy_line_start = LineStart::new(&bytes[ix..]);
                                    let tree_position =
                                        scan_containers(&self.tree, &mut lazy_line_start, self.options, false);
                                    let current_container = tree_position == self.tree.spine_len();

                                    match {
                                        let _explicit_decision = ::core::primitive::bool::then_some(!lazy_line_start.scan_space(4)
                                        && self.scan_paragraph_interrupt(
                                            &self.text[ix + lazy_line_start.bytes_scanned()..],
                                            current_container,
                                            tree_position,
                                        ), ());
                                        _explicit_decision
                                    } {
                                        ::core::option::Option::Some(()) => {
                                            return ix;
                                        },
                                        ::core::option::Option::None => {


                                            line_start = lazy_line_start;
                                            line_start.scan_all_space();
                                            start_ix = ix;
                                            break;
                                        },
                                    }
                                },
                                ::core::option::Option::None => {},
                            }
                        },
                        ::core::option::Option::None => {
                            line_start = save;
                            break;
                        },
                    },
                },
            }
        }

        let ix = start_ix + line_start.bytes_scanned();

        match scan_blank_line(&bytes[ix..]) {
            Some(n) => {
                match self.tree.peek_up() {
                    Some(node_ix) => {
                        match &mut self.tree[node_ix].item.body {
                            ItemBody::BlockQuote(..) => (),
                            ItemBody::ListItem(indent) | ItemBody::DefinitionListDefinition(indent)
                                if self.begin_list_item.is_some() =>
                            {
                                self.last_line_blank = true;




                                *indent = 0;
                            }
                            _ => {
                                self.last_line_blank = true;
                            }
                        }
                    },
                    ::core::option::Option::None => {
                        self.last_line_blank = true;
                    },
                }

                return ix + n;
            },
            ::core::option::Option::None => {},
        }


        let remaining_space = line_start.remaining_space();

        let indent = line_start.scan_space_upto(4);

        match indent {
            4 => {
                self.finish_list(start_ix);
                let ix_2 = start_ix + line_start.bytes_scanned();
                let remaining_space_2 = line_start.remaining_space();
                return self.parse_indented_code_block(ix_2, remaining_space_2);
            },
            _ => {},
        }

        let ix_3 = start_ix + line_start.bytes_scanned();


        match indent {
            0 => {
                match scan_metadata_block(
                        &bytes[ix_3..],
                        self.options
                            .contains(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS),
                        self.options
                            .contains(Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS),
                    ) {
                    Some((_n, metadata_block_ch)) => {
                        self.finish_list(start_ix);
                        return self.parse_metadata_block(ix_3, metadata_block_ch);
                    },
                    ::core::option::Option::None => {},
                }
            },
            _ => {},
        }


        match {
            let _explicit_decision = ::core::primitive::bool::then_some(bytes[ix_3] == b'<', ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {


                match get_html_end_tag(&bytes[(ix_3 + 1)..]) {
                    Some(html_end_tag) => {
                        self.finish_list(start_ix);
                        return self.parse_html_block_type_1_to_5(
                            ix_3,
                            html_end_tag,
                            remaining_space,
                            indent,
                        );
                    },
                    ::core::option::Option::None => {},
                }


                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(starts_html_block_type_6(&bytes[(ix_3 + 1)..]), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        self.finish_list(start_ix);
                        return self.parse_html_block_type_6_or_7(ix_3, remaining_space, indent);
                    },
                    ::core::option::Option::None => {},
                }


                match scan_html_type_7(&bytes[ix_3..]) {
                    Some(_html_bytes) => {
                        self.finish_list(start_ix);
                        return self.parse_html_block_type_6_or_7(ix_3, remaining_space, indent);
                    },
                    ::core::option::Option::None => {},
                }
            },
            ::core::option::Option::None => {},
        }

        match scan_hrule(&bytes[ix_3..]) {
            Ok(n) => {
                self.finish_list(start_ix);
                return self.parse_hrule(n, ix_3);
            },
            ::core::result::Result::Err(_) => {},
        }

        match scan_atx_heading(&bytes[ix_3..]) {
            Some(atx_size) => {
                self.finish_list(start_ix);
                return self.parse_atx_heading(ix_3, atx_size);
            },
            ::core::option::Option::None => {},
        }

        match scan_code_fence(&bytes[ix_3..]) {
            Some((n, fence_ch)) => {
                self.finish_list(start_ix);
                return self.parse_fenced_code_block(ix_3, indent, fence_ch, n);
            },
            ::core::option::Option::None => {},
        }


        while let Some((bytecount, label, link_def)) =
            self.parse_refdef_total(start_ix + line_start.bytes_scanned())
        {
            self.allocs.refdefs.0.entry(label).or_insert(link_def);
            let container_start = start_ix + line_start.bytes_scanned();
            let mut ix_4 = container_start + bytecount;















            match scan_blank_line(&bytes[ix_4..]) {
                Some(nl) => {
                    ix_4 += nl;
                },
                ::core::option::Option::None => {
                    self.finish_list(start_ix);
                    return ix_4;
                },
            }

            match self.scan_next_line_or_lazy_continuation(&bytes[ix_4..]) {
                Some(lazy_line_start) => {
                    line_start = lazy_line_start;
                    start_ix = ix_4;
                },
                ::core::option::Option::None => {
                    self.finish_list(start_ix);
                    return ix_4;
                },
            }
        }

        let ix_5 = start_ix + line_start.bytes_scanned();

        self.parse_paragraph(ix_5, None)
    }

    /// footnote definitions and GFM quote markers can be "interrupted"
    /// like paragraphs, but otherwise can't have other blocks after them.
    ///
    /// Call this at the end of the line to parse that. If it succeeeds,
    /// this returns the LineStart for the new line.
    fn scan_next_line_or_lazy_continuation<'input>(
        &mut self,
        bytes: &'input [u8],
    ) -> Option<LineStart<'input>> {
        let mut line_start = LineStart::new(bytes);
        let tree_position = scan_containers(&self.tree, &mut line_start, self.options, false);
        let current_container = tree_position == self.tree.spine_len();

        match {
            let _explicit_decision = ::core::primitive::bool::then_some((!line_start.scan_space(4)
            && self.scan_paragraph_interrupt(
                &self.text[self.text.len() - bytes.len() + line_start.bytes_scanned()..],
                current_container,
                tree_position,
            ))
            || scan_blank_line(&bytes[line_start.bytes_scanned()..]).is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                None
            },
            ::core::option::Option::None => {
                line_start.scan_all_space();
                Some(line_start)
            },
        }
    }

    /// Returns the offset of the first line after the table.
    /// Assumptions: current focus is a table element and the table header
    /// matches the separator line (same number of columns).
    fn parse_table(
        &mut self,
        table_cols: usize,
        head_start: usize,
        body_start: usize,
    ) -> Option<usize> {


        let mut missing_empty_cells = 0;

        let (_sep_start, thead_ix) =
            self.parse_table_row_inner(head_start, table_cols, &mut missing_empty_cells)?;
        self.tree[thead_ix].item.body = ItemBody::TableHead;


        let mut ix = body_start;

        while let Some((next_ix, _row_ix)) =
            self.parse_table_row(ix, table_cols, &mut missing_empty_cells)
        {
            ix = next_ix;
        }

        self.pop(ix);
        Some(ix)
    }

    /// Call this when containers are taken care of.
    /// Returns bytes scanned, row_ix
    fn parse_table_row_inner(
        &mut self,
        mut ix: usize,
        row_cells: usize,
        missing_empty_cells: &mut usize,
    ) -> Option<(usize, TreeIndex)> {

        const MAX_AUTOCOMPLETED_CELLS: usize = 1 << 18;

        let bytes = self.text.as_bytes();
        let mut cells = 0;
        let mut final_cell_ix = None;

        let old_cur = self.tree.cur();
        let row_ix = self.tree.append(Item {
            start: ix,
            end: 0,
            body: ItemBody::TableRow,
        });
        self.tree.push();

        loop {
            ix += scan_ch(&bytes[ix..], b'|');
            let start_ix = ix;
            ix += scan_whitespace_no_nl(&bytes[ix..]);

            match scan_eol(&bytes[ix..]) {
                Some(eol_bytes) => {
                    ix += eol_bytes;
                    break;
                },
                ::core::option::Option::None => {},
            }

            let cell_ix = self.tree.append(Item {
                start: start_ix,
                end: ix,
                body: ItemBody::TableCell,
            });
            self.tree.push();
            let (next_ix, _brk) = self.parse_line(ix, None, TableParseMode::Active);

            self.tree[cell_ix].item.end = next_ix;
            self.tree.pop();

            ix = next_ix;
            cells += 1;

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(cells == row_cells, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    final_cell_ix = Some(cell_ix);
                },
                ::core::option::Option::None => {},
            }
        }

        if let (Some(cur), 0) = (old_cur, cells) {
            self.pop(ix);
            self.tree[cur].next = None;
            return None;
        }




        for _ in cells..row_cells {
            match {
                let _explicit_decision = ::core::primitive::bool::then_some(*missing_empty_cells >= MAX_AUTOCOMPLETED_CELLS, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    return None;
                },
                ::core::option::Option::None => {},
            }

            *missing_empty_cells += 1;
            self.tree.append(Item {
                start: ix,
                end: ix,
                body: ItemBody::TableCell,
            });
        }


        match final_cell_ix {
            Some(cell_ix) => {
                self.tree[cell_ix].next = None;
            },
            ::core::option::Option::None => {},
        }

        self.pop(ix);

        Some((ix, row_ix))
    }

    /// Returns first offset after the row and the tree index of the row.
    fn parse_table_row(
        &mut self,
        mut ix: usize,
        row_cells: usize,
        missing_empty_cells: &mut usize,
    ) -> Option<(usize, TreeIndex)> {
        let bytes = self.text.as_bytes();
        let mut line_start = LineStart::new(&bytes[ix..]);
        let tree_position = scan_containers(&self.tree, &mut line_start, self.options, false);
        let current_container = tree_position == self.tree.spine_len();

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(!current_container, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return None;
            },
            ::core::option::Option::None => {},
        }

        line_start.scan_all_space();
        ix += line_start.bytes_scanned();

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(scan_paragraph_interrupt_no_table(
                &self.text[ix..],
                current_container,
                self.options.contains(Options::ENABLE_FOOTNOTES),
                self.options.contains(Options::ENABLE_DEFINITION_LIST),
                &self.tree,
                tree_position,
            ), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return None;
            },
            ::core::option::Option::None => {},
        }

        let (ix_2, row_ix) = self.parse_table_row_inner(ix, row_cells, missing_empty_cells)?;
        Some((ix_2, row_ix))
    }

    /// Returns offset of line start after paragraph.
    fn parse_paragraph(&mut self, start_ix: usize, tasklist_marker: Option<Item>) -> usize {
        let body = if let Some(ItemBody::DefinitionList(_)) =
            self.tree.peek_up().map(|idx| &self.tree[idx].item.body)
        {
            match {
                let _explicit_decision = ::core::primitive::bool::then_some(self.tree.cur().map_or(true, |idx| {
                    matches!(
                        &self.tree[idx].item.body,
                        ItemBody::DefinitionListDefinition(..)
                    )
                }), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {

                    self.last_line_blank = false;
                    ItemBody::MaybeDefinitionListTitle
                },
                ::core::option::Option::None => {
                    self.finish_list(start_ix);
                    ItemBody::Paragraph
                },
            }
        } else {
            self.finish_list(start_ix);
            ItemBody::Paragraph
        };
        let node_ix = self.tree.append(Item {
            start: start_ix,
            end: 0,
            body: body.clone(),
        });
        self.tree.push();

        match tasklist_marker {
            Some(item) => {
                self.tree.append(item);
            },
            ::core::option::Option::None => {},
        }

        let bytes = self.text.as_bytes();
        let mut ix = start_ix;

        loop {
            let scan_mode = match {
                let _explicit_decision = ::core::primitive::bool::then_some(self.options.contains(Options::ENABLE_TABLES) && ix == start_ix, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    TableParseMode::Scan
                },
                ::core::option::Option::None => {
                    TableParseMode::Disabled
                },
            };
            let (next_ix, brk) = self.parse_line(ix, None, scan_mode);


            if let Some(Item {
                body: ItemBody::Table(alignment_ix),
                ..
            }) = brk
            {
                let table_cols = self.allocs[alignment_ix].len();
                self.tree[node_ix].item.body = ItemBody::Table(alignment_ix);


                self.tree[node_ix].child = None;
                self.tree.pop();

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(body == ItemBody::MaybeDefinitionListTitle, ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        self.finish_list(ix);
                    },
                    ::core::option::Option::None => {},
                }

                self.tree.push();

                match self.parse_table(table_cols, ix, next_ix) {
                    Some(ix_2) => {
                        return ix_2;
                    },
                    ::core::option::Option::None => {},
                }
            }

            ix = next_ix;
            let mut line_start = LineStart::new(&bytes[ix..]);
            let tree_position = scan_containers(&self.tree, &mut line_start, self.options, false);
            let current_container = tree_position == self.tree.spine_len();
            let trailing_backslash_pos = match brk {
                Some(Item {
                    start,
                    body: ItemBody::HardBreak(true),
                    ..
                }) if bytes[start] == b'\\' => Some(start),
                ::core::option::Option::None | ::core::option::Option::Some(_) => None,
            };

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(!line_start.scan_space(4), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    let ix_new = ix + line_start.bytes_scanned();

                    match ::core::primitive::bool::then_some(current_container, ()) {
                        ::core::option::Option::Some(()) => {
                            match self.parse_setext_heading(ix_new, node_ix, trailing_backslash_pos.is_some()) {
                                Some(ix_setext) => {
                                    match trailing_backslash_pos {
                                        Some(pos) => {
                                            self.tree.append_text(pos, pos + 1, false);
                                        },
                                        ::core::option::Option::None => {},
                                    }

                                    self.pop(ix_setext);

                                    match {
                                        let _explicit_decision = ::core::primitive::bool::then_some(body == ItemBody::MaybeDefinitionListTitle, ());
                                        _explicit_decision
                                    } {
                                        ::core::option::Option::Some(()) => {
                                            self.finish_list(ix);
                                        },
                                        ::core::option::Option::None => {},
                                    }

                                    return ix_setext;
                                },
                                ::core::option::Option::None => {},
                            }
                        },
                        ::core::option::Option::None => {},
                    }


                    let suffix = &self.text[ix_new..];

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(self.scan_paragraph_interrupt(suffix, current_container, tree_position), ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            match trailing_backslash_pos {
                                Some(pos) => {
                                    self.tree.append_text(pos, pos + 1, false);
                                },
                                ::core::option::Option::None => {},
                            }

                            break;
                        },
                        ::core::option::Option::None => {},
                    }
                },
                ::core::option::Option::None => {},
            }

            line_start.scan_all_space();

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(line_start.is_at_eol(), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    match trailing_backslash_pos {
                        Some(pos) => {
                            self.tree.append_text(pos, pos + 1, false);
                        },
                        ::core::option::Option::None => {},
                    }

                    break;
                },
                ::core::option::Option::None => {},
            }

            ix = next_ix + line_start.bytes_scanned();

            match brk {
                Some(item) => {
                    self.tree.append(item);
                },
                ::core::option::Option::None => {},
            }
        }

        self.pop(ix);
        ix
    }

    /// Returns end ix of setext_heading on success.
    fn parse_setext_heading(
        &mut self,
        ix: usize,
        node_ix: TreeIndex,
        has_trailing_content: bool,
    ) -> Option<usize> {
        let bytes = self.text.as_bytes();
        let (n, level) = scan_setext_heading(&bytes[ix..])?;
        let mut attrs = None;

        match self.tree.cur() {
            Some(cur_ix) => {
                let parent_ix = self.tree.peek_up().unwrap();
                let header_start = self.tree[parent_ix].item.start;



                let header_end = self.tree[cur_ix].item.end;


                let (content_end, attrs_) =
                    self.extract_and_parse_heading_attribute_block(header_start, header_end);
                attrs = attrs_;


                let new_end = match ::core::primitive::bool::then_some(has_trailing_content, ()) {
                    ::core::option::Option::Some(()) => {
                        content_end
                    },
                    ::core::option::Option::None => {
                        let mut last_line_start = header_start;

                        match attrs {
                            ::core::option::Option::Some(_) => {
                                loop {
                                    let next_line_start =
                                        last_line_start + scan_nextline(&bytes[last_line_start..content_end]);

                                    match {
                                        let _explicit_decision = ::core::primitive::bool::then_some(next_line_start >= content_end, ());
                                        _explicit_decision
                                    } {
                                        ::core::option::Option::Some(()) => {
                                            break;
                                        },
                                        ::core::option::Option::None => {},
                                    }

                                    let mut line_start = LineStart::new(&bytes[next_line_start..content_end]);

                                    match {
                                        let _explicit_decision = ::core::primitive::bool::then_some(scan_containers(&self.tree, &mut line_start, self.options, false)
                                        != self.tree.spine_len(), ());
                                        _explicit_decision
                                    } {
                                        ::core::option::Option::Some(()) => {
                                            break;
                                        },
                                        ::core::option::Option::None => {},
                                    }

                                    last_line_start = next_line_start + line_start.bytes_scanned();
                                }
                            },
                            ::core::option::Option::None => {},
                        }

                        let trailing_ws = scan_rev_while(
                            &bytes[last_line_start..content_end],
                            is_ascii_whitespace_no_nl,
                        );
                        content_end - trailing_ws
                    },
                };

                match attrs {
                    ::core::option::Option::Some(_) => {

                        self.tree.truncate_siblings(new_end);
                    },
                    ::core::option::Option::None => {},
                }

                match self.tree.cur() {
                    Some(cur_ix_2) => {
                        self.tree[cur_ix_2].item.end = new_end;
                    },
                    ::core::option::Option::None => {},
                }
            },
            ::core::option::Option::None => {},
        }

        self.tree[node_ix].item.body = ItemBody::Heading(
            level,
            attrs.map(|attrs_2| self.allocs.allocate_heading(attrs_2)),
        );

        Some(ix + n)
    }

    /// Parse a line of input, appending text and items to tree.
    ///
    /// Returns: index after line and an item representing the break.
    fn parse_line(
        &mut self,
        start: usize,
        end: Option<usize>,
        mode: TableParseMode,
    ) -> (usize, Option<Item>) {
        let bytes = self.text.as_bytes();
        let bytes_2 = match end {
            Some(end_2) => &bytes[..end_2],
            None => bytes,
        };
        let bytes_len = bytes_2.len();
        let mut pipes = 0;
        let mut last_pipe_ix = start;
        let mut begin_text = start;
        let mut backslash_escaped = false;

        let (final_ix, brk) = iterate_special_bytes(self.lookup_table, bytes_2, start, |ix, byte| {
            match byte {
                b'\n' | b'\r' => {
                    match mode {
                        TableParseMode::Active => {
                            return LoopInstruction::BreakAtWith(ix, None);
                        },
                        TableParseMode::Scan | TableParseMode::Disabled => {},
                    }

                    let mut i = ix;
                    let eol_bytes = scan_eol(&bytes_2[ix..]).unwrap();

                    let end_ix = ix + eol_bytes;
                    let trailing_backslashes = scan_rev_while(&bytes_2[..ix], |b| b == b'\\');

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(trailing_backslashes.wrapping_rem(2) == 1 && end_ix < bytes_len, ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            i -= 1;
                            self.tree.append_text(begin_text, i, backslash_escaped);
                            backslash_escaped = false;
                            return LoopInstruction::BreakAtWith(
                                end_ix,
                                Some(Item {
                                    start: i,
                                    end: end_ix,
                                    body: ItemBody::HardBreak(true),
                                }),
                            );
                        },
                        ::core::option::Option::None => {},
                    }

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(mode == TableParseMode::Scan && pipes > 0, ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {

                            let next_line_ix = ix + eol_bytes;
                            let mut line_start = LineStart::new(&bytes_2[next_line_ix..]);

                            match {
                                let _explicit_decision = ::core::primitive::bool::then_some(scan_containers(&self.tree, &mut line_start, self.options, false)
                                == self.tree.spine_len(), ());
                                _explicit_decision
                            } {
                                ::core::option::Option::Some(()) => {
                                    let table_head_ix = next_line_ix + line_start.bytes_scanned();
                                    let (table_head_bytes, alignment) =
                                        scan_table_head(&bytes_2[table_head_ix..]);

                                    match {
                                        let _explicit_decision = ::core::primitive::bool::then_some(table_head_bytes > 0, ());
                                        _explicit_decision
                                    } {
                                        ::core::option::Option::Some(()) => {

                                            let ::core::result::Result::Ok(header_count) =
                                                count_header_cols(bytes_2, self::CountHeaderColsArguments { pipes, start, last_pipe_ix });


                                            match {
                                                let _explicit_decision = ::core::primitive::bool::then_some(alignment.len() == header_count, ());
                                                _explicit_decision
                                            } {
                                                ::core::option::Option::Some(()) => {
                                                    let alignment_ix = self.allocs.allocate_alignment(alignment);
                                                    let end_ix_2 = table_head_ix + table_head_bytes;
                                                    return LoopInstruction::BreakAtWith(
                                                        end_ix_2,
                                                        Some(Item {
                                                            start: i,
                                                            end: end_ix_2,
                                                            body: ItemBody::Table(alignment_ix),
                                                        }),
                                                    );
                                                },
                                                ::core::option::Option::None => {},
                                            }
                                        },
                                        ::core::option::Option::None => {},
                                    }
                                },
                                ::core::option::Option::None => {},
                            }
                        },
                        ::core::option::Option::None => {},
                    }

                    let trailing_whitespace =
                        scan_rev_while(&bytes_2[..ix], is_ascii_whitespace_no_nl);

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(trailing_whitespace >= 2, ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            i -= trailing_whitespace;
                            self.tree.append_text(begin_text, i, backslash_escaped);
                            backslash_escaped = false;
                            return LoopInstruction::BreakAtWith(
                                end_ix,
                                Some(Item {
                                    start: i,
                                    end: end_ix,
                                    body: ItemBody::HardBreak(false),
                                }),
                            );
                        },
                        ::core::option::Option::None => {},
                    }

                    self.tree
                        .append_text(begin_text, ix - trailing_whitespace, backslash_escaped);
                    backslash_escaped = false;

                    LoopInstruction::BreakAtWith(
                        end_ix,
                        Some(Item {
                            start: i,
                            end: end_ix,
                            body: ItemBody::SoftBreak,
                        }),
                    )
                }
                b'\\' => {
                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(ix + 1 < bytes_len && is_ascii_punctuation(bytes_2[ix + 1]), ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            self.tree.append_text(begin_text, ix, backslash_escaped);

                            match {
                                let _explicit_decision = ::core::primitive::bool::then_some(bytes_2[ix + 1] == b'`', ());
                                _explicit_decision
                            } {
                                ::core::option::Option::Some(()) => {
                                    let count = 1 + scan_ch_repeat(&bytes_2[(ix + 2)..], b'`');
                                    self.tree.append(Item {
                                        start: ix + 1,
                                        end: ix + count + 1,
                                        body: ItemBody::MaybeCode(count, true),
                                    });
                                    begin_text = ix + 1 + count;
                                    backslash_escaped = false;
                                    LoopInstruction::ContinueAndSkip(count)
                                },
                                ::core::option::Option::None => match {
                                    let _explicit_decision = ::core::primitive::bool::then_some(bytes_2[ix + 1] == b'|' && TableParseMode::Active == mode, ());
                                    _explicit_decision
                                } {
                                    ::core::option::Option::Some(()) => {






                                        begin_text = ix + 1;
                                        backslash_escaped = false;
                                        LoopInstruction::ContinueAndSkip(1)
                                    },
                                    ::core::option::Option::None => match {
                                        let _explicit_decision = ::core::primitive::bool::then_some(ix + 2 < bytes_len
                                        && bytes_2[ix + 1] == b'\\'
                                        && bytes_2[ix + 2] == b'|'
                                        && TableParseMode::Active == mode, ());
                                        _explicit_decision
                                    } {
                                        ::core::option::Option::Some(()) => {

                                            begin_text = ix + 2;
                                            backslash_escaped = true;
                                            LoopInstruction::ContinueAndSkip(2)
                                        },
                                        ::core::option::Option::None => {
                                            begin_text = ix + 1;
                                            backslash_escaped = true;
                                            LoopInstruction::ContinueAndSkip(1)
                                        },
                                    },
                                },
                            }
                        },
                        ::core::option::Option::None => {
                            LoopInstruction::ContinueAndSkip(0)
                        },
                    }
                }
                c @ b'*' | c @ b'_' | c @ b'~' | c @ b'^' => {
                    let string_suffix = &self.text[ix..];
                    let count = 1 + scan_ch_repeat(&string_suffix.as_bytes()[1..], c);
                    let can_open = delim_run_can_open(
                        &self.text[start..],
                        string_suffix,
                        self::DelimRunCanOpenArguments { run_len: count,
                        ix: ix - start },
                        mode,
                    );
                    let can_close = delim_run_can_close(
                        &self.text[start..],
                        string_suffix,
                        self::DelimRunCanCloseArguments { run_len: count,
                        ix: ix - start },
                        mode,
                    );
                    let is_valid_seq = (c != b'~' || count <= 2) || (c == b'~' && count == 2);

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some((can_open || can_close) && is_valid_seq, ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            self.tree.append_text(begin_text, ix, backslash_escaped);
                            backslash_escaped = false;

                            for i in 0..count {
                                self.tree.append(Item {
                                    start: ix + i,
                                    end: ix + i + 1,
                                    body: ItemBody::MaybeEmphasis(count - i, can_open, can_close),
                                });
                            }

                            begin_text = ix + count;
                        },
                        ::core::option::Option::None => {},
                    }

                    LoopInstruction::ContinueAndSkip(count - 1)
                }
                b'$' => {
                    let byte_suffix = &bytes_2[ix..];
                    let can_open = !byte_suffix[1..]
                        .first()
                        .copied()
                        .map_or(true, is_ascii_whitespace);
                    let can_close = ix > start
                        && !bytes_2[..ix]
                            .last()
                            .copied()
                            .map_or(true, is_ascii_whitespace);









                    let brace_context =
                        match {
                            let _explicit_decision = ::core::primitive::bool::then_some(self.brace_context_stack.len() > MATH_BRACE_CONTEXT_MAX_NESTING, ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                self.brace_context_next as u8
                            },
                            ::core::option::Option::None => {
                                self.brace_context_stack.last().copied().unwrap_or_else(|| {
                                    self.brace_context_stack.push(!0);
                                    !0
                                })
                            },
                        };

                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    self.tree.append(Item {
                        start: ix,
                        end: ix + 1,
                        body: ItemBody::MaybeMath(can_open, can_close, brace_context),
                    });
                    begin_text = ix + 1;
                    LoopInstruction::ContinueAndSkip(0)
                }
                b'{' => {
                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(self.brace_context_stack.len() == MATH_BRACE_CONTEXT_MAX_NESTING, ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            self.brace_context_stack.push(self.brace_context_next as u8);
                            self.brace_context_next = MATH_BRACE_CONTEXT_MAX_NESTING;
                        },
                        ::core::option::Option::None => match {
                            let _explicit_decision = ::core::primitive::bool::then_some(self.brace_context_stack.len() > MATH_BRACE_CONTEXT_MAX_NESTING, ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {


                                self.brace_context_next += 1;
                            },
                            ::core::option::Option::None => match {
                                let _explicit_decision = ::core::primitive::bool::then_some(!self.brace_context_stack.is_empty(), ());
                                _explicit_decision
                            } {
                                ::core::option::Option::Some(()) => {

                                    self.brace_context_stack.push(self.brace_context_next as u8);
                                    self.brace_context_next += 1;
                                },
                                ::core::option::Option::None => {},
                            },
                        },
                    }

                    LoopInstruction::ContinueAndSkip(0)
                }
                b'}' => {
                    if let &mut [ref mut top_level_context] = ::core::convert::AsMut::<[_]>::as_mut(&mut self.brace_context_stack) {















                        *top_level_context = top_level_context.wrapping_sub(1);
                    } else if self.brace_context_stack.len() > MATH_BRACE_CONTEXT_MAX_NESTING {


                        match {
                            let _explicit_decision = ::core::primitive::bool::then_some(self.brace_context_next <= MATH_BRACE_CONTEXT_MAX_NESTING, ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                self.brace_context_stack.pop();
                            },
                            ::core::option::Option::None => {
                                self.brace_context_next -= 1;
                            },
                        }
                    } else {
                        self.brace_context_stack.pop();
                    }

                    LoopInstruction::ContinueAndSkip(0)
                }
                b'`' => {
                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    backslash_escaped = false;
                    let count = 1 + scan_ch_repeat(&bytes_2[(ix + 1)..], b'`');
                    self.tree.append(Item {
                        start: ix,
                        end: ix + count,
                        body: ItemBody::MaybeCode(count, false),
                    });
                    begin_text = ix + count;
                    LoopInstruction::ContinueAndSkip(count - 1)
                }
                b'<' if bytes_2.get(ix + 1) != Some(&b'\\') => {


                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    backslash_escaped = false;
                    self.tree.append(Item {
                        start: ix,
                        end: ix + 1,
                        body: ItemBody::MaybeHtml,
                    });
                    begin_text = ix + 1;
                    LoopInstruction::ContinueAndSkip(0)
                }
                b'!' => {
                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(ix + 1 < bytes_len && bytes_2[ix + 1] == b'[', ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            self.tree.append_text(begin_text, ix, backslash_escaped);
                            backslash_escaped = false;
                            self.tree.append(Item {
                                start: ix,
                                end: ix + 2,
                                body: ItemBody::MaybeImage,
                            });
                            begin_text = ix + 2;
                            LoopInstruction::ContinueAndSkip(1)
                        },
                        ::core::option::Option::None => {
                            LoopInstruction::ContinueAndSkip(0)
                        },
                    }
                }
                b'[' => {
                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    backslash_escaped = false;
                    self.tree.append(Item {
                        start: ix,
                        end: ix + 1,
                        body: ItemBody::MaybeLinkOpen,
                    });
                    begin_text = ix + 1;
                    LoopInstruction::ContinueAndSkip(0)
                }
                b']' => {
                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    backslash_escaped = false;
                    self.tree.append(Item {
                        start: ix,
                        end: ix + 1,
                        body: ItemBody::MaybeLinkClose(true),
                    });
                    begin_text = ix + 1;
                    LoopInstruction::ContinueAndSkip(0)
                }
                b'&' => match scan_entity(&bytes_2[ix..]) {
                    (n, Some(value)) => {
                        self.tree.append_text(begin_text, ix, backslash_escaped);
                        backslash_escaped = false;
                        self.tree.append(Item {
                            start: ix,
                            end: ix + n,
                            body: ItemBody::SynthesizeText(self.allocs.allocate_cow(value)),
                        });
                        begin_text = ix + n;
                        LoopInstruction::ContinueAndSkip(n - 1)
                    }
                    _ => LoopInstruction::ContinueAndSkip(0),
                },
                b'|' => {
                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(ix != 0 && bytes_2[ix - 1] == b'\\', ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            LoopInstruction::ContinueAndSkip(0)
                        },
                        ::core::option::Option::None => match mode {
                            TableParseMode::Active => {
                                LoopInstruction::BreakAtWith(ix, None)
                            },
                            TableParseMode::Scan | TableParseMode::Disabled => {
                                last_pipe_ix = ix;
                                pipes += 1;
                                LoopInstruction::ContinueAndSkip(0)
                            },
                        },
                    }
                }
                b'.' => {
                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(ix + 2 < bytes_2.len() && bytes_2[ix + 1] == b'.' && bytes_2[ix + 2] == b'.', ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            self.tree.append_text(begin_text, ix, backslash_escaped);
                            backslash_escaped = false;
                            self.tree.append(Item {
                                start: ix,
                                end: ix + 3,
                                body: ItemBody::SynthesizeChar('…'),
                            });
                            begin_text = ix + 3;
                            LoopInstruction::ContinueAndSkip(2)
                        },
                        ::core::option::Option::None => {
                            LoopInstruction::ContinueAndSkip(0)
                        },
                    }
                }
                b'-' => {
                    let count = 1 + scan_ch_repeat(&bytes_2[(ix + 1)..], b'-');

                    match count {
                        1 => {
                            LoopInstruction::ContinueAndSkip(0)
                        },
                        _ => {
                            let itembody = match count {
                                2 => {
                                    ItemBody::SynthesizeChar('–')
                                },
                                _ => match count {
                                    3 => {
                                        ItemBody::SynthesizeChar('—')
                                    },
                                    _ => {
                                        let (ems, ens) = match count.wrapping_rem(6) {
                                            0 | 3 => (count.wrapping_div(3), 0),
                                            2 | 4 => (0, count.wrapping_div(2)),
                                            1 => (count.wrapping_div(3) - 1, 2),
                                            _ => (count.wrapping_div(3), 1),
                                        };

                                        let mut buf = String::with_capacity(3 * (ems + ens));

                                        for _ in 0..ems {
                                            buf.push('—');
                                        }

                                        for _ in 0..ens {
                                            buf.push('–');
                                        }

                                        ItemBody::SynthesizeText(self.allocs.allocate_cow(<crate::strings::CowStr<'_> as ::core::convert::From<::std::string::String>>::from(buf)))
                                    },
                                },
                            };

                            self.tree.append_text(begin_text, ix, backslash_escaped);
                            backslash_escaped = false;
                            self.tree.append(Item {
                                start: ix,
                                end: ix + count,
                                body: itembody,
                            });
                            begin_text = ix + count;
                            LoopInstruction::ContinueAndSkip(count - 1)
                        },
                    }
                }
                c @ b'\'' | c @ b'"' => {
                    let string_suffix = &self.text[ix..];
                    let can_open =
                        delim_run_can_open(&self.text[start..], string_suffix, self::DelimRunCanOpenArguments { run_len: 1, ix: ix - start }, mode);
                    let can_close = delim_run_can_close(
                        &self.text[start..],
                        string_suffix,
                        self::DelimRunCanCloseArguments { run_len: 1,
                        ix: ix - start },
                        mode,
                    );

                    self.tree.append_text(begin_text, ix, backslash_escaped);
                    backslash_escaped = false;
                    self.tree.append(Item {
                        start: ix,
                        end: ix + 1,
                        body: ItemBody::MaybeSmartQuote(c, can_open, can_close),
                    });
                    begin_text = ix + 1;

                    LoopInstruction::ContinueAndSkip(0)
                }
                _ => LoopInstruction::ContinueAndSkip(0),
            }
        });

        match brk {
            ::core::option::Option::None => {
                let trailing_whitespace =
                    scan_rev_while(&bytes_2[begin_text..final_ix], is_ascii_whitespace_no_nl);

                self.tree.append_text(
                    begin_text,
                    final_ix - trailing_whitespace,
                    backslash_escaped,
                );
            },
            ::core::option::Option::Some(_) => {},
        }

        (final_ix, brk)
    }

    /// When start_ix is at the beginning of an HTML block of type 1 to 5,
    /// this will find the end of the block, adding the block itself to the
    /// tree and also keeping track of the lines of HTML within the block.
    ///
    /// The html_end_tag is the tag that must be found on a line to end the block.
    fn parse_html_block_type_1_to_5(
        &mut self,
        start_ix: usize,
        html_end_tag: &str,
        mut remaining_space: usize,
        mut indent: usize,
    ) -> usize {
        self.tree.append(Item {
            start: start_ix,
            end: 0,
            body: ItemBody::HtmlBlock,
        });
        self.tree.push();

        let bytes = self.text.as_bytes();
        let mut ix = start_ix;
        let end_ix;

        loop {
            let line_start_ix = ix;
            ix += scan_nextline(&bytes[ix..]);
            self.append_html_line(remaining_space.max(indent), line_start_ix, ix);

            let mut line_start = LineStart::new(&bytes[ix..]);
            let n_containers = scan_containers(&self.tree, &mut line_start, self.options, false);

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(n_containers < self.tree.spine_len(), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    end_ix = ix;
                    break;
                },
                ::core::option::Option::None => {},
            }

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(self.text[line_start_ix..ix].contains(html_end_tag), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    end_ix = ix;
                    break;
                },
                ::core::option::Option::None => {},
            }

            let next_line_ix = ix + line_start.bytes_scanned();

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(next_line_ix == self.text.len(), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    end_ix = next_line_ix;
                    break;
                },
                ::core::option::Option::None => {},
            }

            ix = next_line_ix;
            remaining_space = line_start.remaining_space();
            indent = 0;
        }

        self.pop(end_ix);
        ix
    }

    /// When start_ix is at the beginning of an HTML block of type 6 or 7,
    /// this will consume lines until there is a blank line and keep track of
    /// the HTML within the block.
    fn parse_html_block_type_6_or_7(
        &mut self,
        start_ix: usize,
        mut remaining_space: usize,
        mut indent: usize,
    ) -> usize {
        self.tree.append(Item {
            start: start_ix,
            end: 0,
            body: ItemBody::HtmlBlock,
        });
        self.tree.push();

        let bytes = self.text.as_bytes();
        let mut ix = start_ix;
        let end_ix;

        loop {
            let line_start_ix = ix;
            ix += scan_nextline(&bytes[ix..]);
            self.append_html_line(remaining_space.max(indent), line_start_ix, ix);

            let mut line_start = LineStart::new(&bytes[ix..]);
            let n_containers = scan_containers(&self.tree, &mut line_start, self.options, false);

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(n_containers < self.tree.spine_len() || line_start.is_at_eol(), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    end_ix = ix;
                    break;
                },
                ::core::option::Option::None => {},
            }

            let next_line_ix = ix + line_start.bytes_scanned();

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(next_line_ix == self.text.len() || scan_blank_line(&bytes[next_line_ix..]).is_some(), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    end_ix = next_line_ix;
                    break;
                },
                ::core::option::Option::None => {},
            }

            ix = next_line_ix;
            remaining_space = line_start.remaining_space();
            indent = 0;
        }

        self.pop(end_ix);
        ix
    }

    fn parse_indented_code_block(&mut self, start_ix: usize, mut remaining_space: usize) -> usize {
        self.tree.append(Item {
            start: start_ix,
            end: 0,
            body: ItemBody::IndentCodeBlock,
        });
        self.tree.push();
        let bytes = self.text.as_bytes();
        let mut last_nonblank_child = None;
        let mut last_nonblank_ix = 0;
        let mut end_ix = 0;
        self.last_line_blank = false;

        let mut ix = start_ix;

        loop {
            let line_start_ix = ix;
            ix += scan_nextline(&bytes[ix..]);
            self.append_code_text(remaining_space, line_start_ix, ix);


            match {
                let _explicit_decision = ::core::primitive::bool::then_some(!self.last_line_blank, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    last_nonblank_child = self.tree.cur();
                    last_nonblank_ix = ix;
                    end_ix = ix;
                },
                ::core::option::Option::None => {},
            }

            let mut line_start = LineStart::new(&bytes[ix..]);
            let n_containers = scan_containers(&self.tree, &mut line_start, self.options, false);

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(n_containers < self.tree.spine_len()
                || !(line_start.scan_space(4) || line_start.is_at_eol()), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    break;
                },
                ::core::option::Option::None => {},
            }

            let next_line_ix = ix + line_start.bytes_scanned();

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(next_line_ix == self.text.len(), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    break;
                },
                ::core::option::Option::None => {},
            }

            ix = next_line_ix;
            remaining_space = line_start.remaining_space();
            self.last_line_blank = scan_blank_line(&bytes[ix..]).is_some();
        }


        match last_nonblank_child {
            Some(child) => {
                self.tree[child].next = None;
                self.tree[child].item.end = last_nonblank_ix;
            },
            ::core::option::Option::None => {},
        }

        self.pop(end_ix);
        ix
    }

    fn parse_fenced_code_block(
        &mut self,
        start_ix: usize,
        indent: usize,
        fence_ch: u8,
        n_fence_char: usize,
    ) -> usize {
        let bytes = self.text.as_bytes();
        let mut info_start = start_ix + n_fence_char;
        info_start += scan_whitespace_no_nl(&bytes[info_start..]);


        let mut ix = info_start + scan_nextline(&bytes[info_start..]);
        let info_end = ix - scan_rev_while(&bytes[info_start..ix], is_ascii_whitespace);
        let info_string = unescape(&self.text[info_start..info_end], self.tree.is_in_table());
        self.tree.append(Item {
            start: start_ix,
            end: 0,
            body: ItemBody::FencedCodeBlock(self.allocs.allocate_cow(info_string)),
        });
        self.tree.push();

        loop {
            let mut line_start = LineStart::new(&bytes[ix..]);
            let n_containers = scan_containers(&self.tree, &mut line_start, self.options, false);

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(n_containers < self.tree.spine_len(), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {


                    self.pop(ix);
                    return ix;
                },
                ::core::option::Option::None => {},
            }

            line_start.scan_space(indent);
            let mut close_line_start = line_start.clone();

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(!close_line_start.scan_space(4 - indent), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    let close_ix = ix + close_line_start.bytes_scanned();

                    match scan_closing_code_fence(&bytes[close_ix..], fence_ch, n_fence_char) {
                        Some(n) => {
                            ix = close_ix + n;
                            self.pop(ix);

                            return ix + match scan_blank_line(&bytes[ix..]) {
                                ::core::option::Option::Some(option_value) => option_value,
                                ::core::option::Option::None => 0,
                            };
                        },
                        ::core::option::Option::None => {},
                    }
                },
                ::core::option::Option::None => {},
            }

            let remaining_space = line_start.remaining_space();
            ix += line_start.bytes_scanned();
            let next_ix = ix + scan_nextline(&bytes[ix..]);
            self.append_code_text(remaining_space, ix, next_ix);
            ix = next_ix;
        }
    }

    fn parse_metadata_block(&mut self, start_ix: usize, metadata_block_ch: u8) -> usize {
        let bytes = self.text.as_bytes();
        let metadata_block_kind = match metadata_block_ch {
            b'-' => MetadataBlockKind::YamlStyle,
            b'+' => MetadataBlockKind::PlusesStyle,
            _ => panic!("Erroneous metadata block character when parsing metadata block"),
        };

        let mut ix = start_ix + 3 + scan_nextline(&bytes[start_ix + 3..]);
        self.tree.append(Item {
            start: start_ix,
            end: 0,
            body: ItemBody::MetadataBlock(metadata_block_kind),
        });
        self.tree.push();

        loop {
            let mut line_start = LineStart::new(&bytes[ix..]);
            let n_containers = scan_containers(&self.tree, &mut line_start, self.options, false);

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(n_containers < self.tree.spine_len(), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    break;
                },
                ::core::option::Option::None => {},
            }

            if let (_, 0) = calc_indent(&bytes[ix..], 4) {
                match scan_closing_metadata_block(&bytes[ix..], metadata_block_ch) {
                    Some(n) => {
                        ix += n;
                        break;
                    },
                    ::core::option::Option::None => {},
                }
            }

            let remaining_space = line_start.remaining_space();
            ix += line_start.bytes_scanned();
            let next_ix = ix + scan_nextline(&bytes[ix..]);
            self.append_code_text(remaining_space, ix, next_ix);
            ix = next_ix;
        }

        self.pop(ix);


        ix + match scan_blank_line(&bytes[ix..]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 0,
        }
    }

    fn append_code_text(&mut self, remaining_space: usize, start: usize, end: usize) {
        match {
            let _explicit_decision = ::core::primitive::bool::then_some(remaining_space > 0, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                let cow_ix = self.allocs.allocate_cow("   "[..remaining_space].into());
                self.tree.append(Item {
                    start,
                    end: start,
                    body: ItemBody::SynthesizeText(cow_ix),
                });
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.text.as_bytes()[end - 2] == b'\r', ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {

                self.tree.append_text(start, end - 2, false);
                self.tree.append_text(end - 1, end, false);
            },
            ::core::option::Option::None => {
                self.tree.append_text(start, end, false);
            },
        }
    }

    /// Appends a line of HTML to the tree.
    fn append_html_line(&mut self, remaining_space: usize, start: usize, end: usize) {
        match {
            let _explicit_decision = ::core::primitive::bool::then_some(remaining_space > 0, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                let cow_ix = self.allocs.allocate_cow("   "[..remaining_space].into());
                self.tree.append(Item {
                    start,
                    end: start,
                    body: ItemBody::SynthesizeText(cow_ix),
                });
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.text.as_bytes()[end - 2] == b'\r', ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {

                self.tree.append(Item {
                    start,
                    end: end - 2,
                    body: ItemBody::Html,
                });
                self.tree.append(Item {
                    start: end - 1,
                    end,
                    body: ItemBody::Html,
                });
            },
            ::core::option::Option::None => {
                self.tree.append(Item {
                    start,
                    end,
                    body: ItemBody::Html,
                });
            },
        }
    }

    /// Pop a container, setting its end.
    fn pop(&mut self, ix: usize) {
        let cur_ix = self.tree.pop().unwrap();
        self.tree[cur_ix].item.end = ix;

        if let ItemBody::DefinitionList(_) = self.tree[cur_ix].item.body {
            let ::core::result::Result::Ok(()) = fixup_end_of_definition_list(&mut self.tree, cur_ix);
            self.begin_list_item = None;
        }

        if let ItemBody::List(ref list_flags, _, _) | ItemBody::DefinitionList(ref list_flags) =
            self.tree[cur_ix].item.body
        {
            match {
                let _explicit_decision = ::core::primitive::bool::then_some(list_flags.get().contains(ListFlags::IS_TIGHT), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    let ::core::result::Result::Ok(()) = surgerize_tight_list(&mut self.tree, cur_ix);
                    self.begin_list_item = None;
                },
                ::core::option::Option::None => {},
            }
        }
    }

    /// Close a list if it's open. Also set loose if last line was blank
    /// and end current list if it's a lone, empty item
    fn finish_list(&mut self, ix: usize) {
        self.finish_empty_list_item();

        match self.tree.peek_up() {
            Some(node_ix) => {
                if let ItemBody::List(_, _, _) | ItemBody::DefinitionList(_) =
                    self.tree[node_ix].item.body
                {
                    self.pop(ix);
                }
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.last_line_blank, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                match self.tree.peek_grandparent() {
                    Some(node_ix) => {
                        if let ItemBody::List(ref list_flags, _, _)
                        | ItemBody::DefinitionList(ref list_flags) = self.tree[node_ix].item.body
                        {
                            list_flags.set(list_flags.get() & !ListFlags::IS_TIGHT);
                        }
                    },
                    ::core::option::Option::None => {},
                }

                self.last_line_blank = false;
            },
            ::core::option::Option::None => {},
        }
    }

    fn finish_empty_list_item(&mut self) {
        match self.begin_list_item {
            Some(begin_list_item) => {
                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(self.last_line_blank, ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {

                        match self.tree.peek_up() {
                            Some(node_ix) => {
                                if let ItemBody::ListItem(_) | ItemBody::DefinitionListDefinition(_) =
                                    self.tree[node_ix].item.body
                                {
                                    self.pop(begin_list_item);
                                }
                            },
                            ::core::option::Option::None => {},
                        }
                    },
                    ::core::option::Option::None => {},
                }
            },
            ::core::option::Option::None => {},
        }

        self.begin_list_item = None;
    }

    /// Continue an existing list or start a new one if there's not an open
    /// list that matches.
    fn continue_list(&mut self, start: usize, ch: u8, index: u64) {
        self.finish_empty_list_item();

        match self.tree.peek_up() {
            Some(node_ix) => {
                if let ItemBody::List(ref list_flags, existing_ch, _) = self.tree[node_ix].item.body {
                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(existing_ch == ch, ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            match {
                                let _explicit_decision = ::core::primitive::bool::then_some(self.last_line_blank, ());
                                _explicit_decision
                            } {
                                ::core::option::Option::Some(()) => {
                                    list_flags.set(list_flags.get() & !ListFlags::IS_TIGHT);
                                    self.last_line_blank = false;
                                },
                                ::core::option::Option::None => {},
                            }

                            return;
                        },
                        ::core::option::Option::None => {},
                    }
                }


                self.finish_list(start);
            },
            ::core::option::Option::None => {},
        }

        self.tree.append(Item {
            start,
            end: 0,
            body: ItemBody::List(Cell::new(ListFlags::IS_TIGHT), ch, index),
        });
        self.tree.push();
        self.last_line_blank = false;
    }

    /// Parse a thematic break.
    ///
    /// Returns index of start of next line.
    fn parse_hrule(&mut self, hrule_size: usize, ix: usize) -> usize {
        self.tree.append(Item {
            start: ix,
            end: ix + hrule_size,
            body: ItemBody::Rule,
        });
        ix + hrule_size
    }

    /// Parse an ATX heading.
    ///
    /// Returns index of start of next line.
    fn parse_atx_heading(&mut self, start: usize, atx_level: HeadingLevel) -> usize {
        let mut ix = start;
        let heading_ix = self.tree.append(Item {
            start,
            end: 0,
            body: ItemBody::default(),
        });
        ix += atx_level as usize;

        let bytes = self.text.as_bytes();

        match scan_eol(&bytes[ix..]) {
            Some(eol_bytes) => {
                self.tree[heading_ix].item.end = ix + eol_bytes;
                self.tree[heading_ix].item.body = ItemBody::Heading(atx_level, None);
                return ix + eol_bytes;
            },
            ::core::option::Option::None => {},
        }


        let skip_spaces = scan_whitespace_no_nl(&bytes[ix..]);
        ix += skip_spaces;


        let header_start = ix;
        let header_node_idx = self.tree.push();


        let (end, content_end, attrs) = match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.options.contains(Options::ENABLE_HEADING_ATTRIBUTES), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {


                let header_end = header_start + scan_nextline(&bytes[header_start..]);
                let (content_end, attrs) =
                    self.extract_and_parse_heading_attribute_block(header_start, header_end);
                self.parse_line(ix, Some(content_end), TableParseMode::Disabled);
                (header_end, content_end, attrs)
            },
            ::core::option::Option::None => {
                let (line_ix, line_brk) = self.parse_line(ix, None, TableParseMode::Disabled);
                ix = line_ix;

                if let Some(Item {
                    start: start_2,
                    end,
                    body: ItemBody::HardBreak(true),
                }) = line_brk
                {
                    self.tree.append_text(start_2, end, false);
                }

                (ix, ix, None)
            },
        };
        self.tree[header_node_idx].item.end = end;


        let mut empty_text_node = false;

        match self.tree.cur() {
            Some(cur_ix) => {

                let header_text = &bytes[header_start..content_end];
                let mut limit = header_text
                    .iter()
                    .rposition(|&b| !(b == b'\n' || b == b'\r' || b == b' '))
                    .map_or(0, |i| i + 1);
                let closer = header_text[..limit]
                    .iter()
                    .rposition(|&b| b != b'#')
                    .map_or(0, |i| i + 1);

                match closer {
                    0 => {
                        limit = closer;
                    },
                    _ => {
                        let spaces = scan_rev_while(&header_text[..closer], |b| b == b' ');

                        match {
                            let _explicit_decision = ::core::primitive::bool::then_some(spaces > 0, ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                limit = closer - spaces;
                            },
                            ::core::option::Option::None => {},
                        }
                    },
                }


                self.tree[cur_ix].item.end = limit + header_start;


                match limit {
                    0 => {
                        empty_text_node = true;
                    },
                    _ => {},
                }
            },
            ::core::option::Option::None => {},
        }

        match ::core::primitive::bool::then_some(empty_text_node, ()) {
            ::core::option::Option::Some(()) => {
                self.tree.remove_node();
            },
            ::core::option::Option::None => {
                self.tree.pop();
            },
        }

        self.tree[heading_ix].item.body = ItemBody::Heading(
            atx_level,
            attrs.map(|attrs_2| self.allocs.allocate_heading(attrs_2)),
        );

        end
    }

    /// Returns the number of bytes scanned on success.
    fn parse_footnote(&mut self, start: usize) -> Option<usize> {
        let bytes = &self.text.as_bytes()[start..];

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(!bytes.starts_with(b"[^"), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return None;
            },
            ::core::option::Option::None => {},
        }

        let (mut i, label) = match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.options.has_gfm_footnotes(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {



                scan_link_label_rest(&self.text[start + 2..], &|_| None, self.tree.is_in_table())?
            },
            ::core::option::Option::None => {
                self.parse_refdef_label(start + 2)?
            },
        };

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.options.has_gfm_footnotes() && label.bytes().any(|b| b == b'\r' || b == b'\n'), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {


                return None;
            },
            ::core::option::Option::None => {},
        }

        i += 2;

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(bytes.get(i) != Some(&b':'), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return None;
            },
            ::core::option::Option::None => {},
        }

        i += 1;
        self.finish_list(start);

        match self.tree.peek_up() {
            Some(node_ix) => {
                if let ItemBody::FootnoteDefinition(..) = self.tree[node_ix].item.body {

                    self.pop(start);
                }
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.options.has_gfm_footnotes(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                i += scan_whitespace_no_nl(&bytes[i..]);
            },
            ::core::option::Option::None => {},
        }

        self.allocs
            .footdefs
            .0
            .insert(UniCase::new(label.clone()), FootnoteDef { use_count: 0 });
        self.tree.append(Item {
            start,
            end: 0,

            body: ItemBody::FootnoteDefinition(self.allocs.allocate_cow(label)),
        });
        self.tree.push();
        Some(i)
    }

    /// Tries to parse a reference label, which can be interrupted by new blocks.
    /// On success, returns the number of bytes of the label and the label itself.
    #[must_use]
    fn parse_refdef_label(&self, start: usize) -> Option<(usize, CowStr<'a>)> {
        scan_link_label_rest(
            &self.text[start..],
            &|bytes| {
                let mut line_start = LineStart::new(bytes);
                let tree_position =
                    scan_containers(&self.tree, &mut line_start, self.options, false);
                let current_container = tree_position == self.tree.spine_len();

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(line_start.scan_space(4), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        return Some(line_start.bytes_scanned());
                    },
                    ::core::option::Option::None => {},
                }

                let bytes_scanned = line_start.bytes_scanned();
                let off = self.text.len() - bytes.len() + bytes_scanned;
                let suffix = &self.text[off..];

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(self.scan_paragraph_interrupt(suffix, current_container, tree_position)
                    || (current_container && scan_setext_heading(suffix.as_bytes()).is_some()), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        None
                    },
                    ::core::option::Option::None => {
                        Some(bytes_scanned)
                    },
                }
            },
            self.tree.is_in_table(),
        )
    }

    /// Returns number of bytes scanned, label and definition on success.
    fn parse_refdef_total(&mut self, start: usize) -> Option<(usize, LinkLabel<'a>, LinkDef<'a>)> {
        let bytes = &self.text.as_bytes()[start..];

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(bytes.get(0) != Some(&b'['), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return None;
            },
            ::core::option::Option::None => {},
        }

        let (mut i, label) = self.parse_refdef_label(start + 1)?;
        i += 1;

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(bytes.get(i) != Some(&b':'), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return None;
            },
            ::core::option::Option::None => {},
        }

        i += 1;
        let (bytecount, link_def) = self.scan_refdef(start, start + i)?;
        Some((bytecount + i, UniCase::new(label), link_def))
    }

    /// Returns number of bytes and number of newlines
    #[must_use]
    fn scan_refdef_space(&self, bytes: &[u8], mut i: usize) -> Option<(usize, usize)> {
        let mut newlines = 0;

        loop {
            let whitespaces = scan_whitespace_no_nl(&bytes[i..]);
            i += whitespaces;

            match scan_eol(&bytes[i..]) {
                Some(eol_bytes) => {
                    i += eol_bytes;
                    newlines += 1;

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(newlines > 1, ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            return None;
                        },
                        ::core::option::Option::None => {},
                    }
                },
                ::core::option::Option::None => {
                    break;
                },
            }

            let mut line_start = LineStart::new(&bytes[i..]);
            let tree_position = scan_containers(&self.tree, &mut line_start, self.options, false);
            let current_container = tree_position == self.tree.spine_len();

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(!line_start.scan_space(4), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    let suffix = &self.text[i + line_start.bytes_scanned()..];

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(self.scan_paragraph_interrupt(suffix, current_container, tree_position)
                        || scan_setext_heading(suffix.as_bytes()).is_some(), ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            return None;
                        },
                        ::core::option::Option::None => {},
                    }
                },
                ::core::option::Option::None => {},
            }

            i += line_start.bytes_scanned();
        }

        Some((i, newlines))
    }


    #[must_use]
    fn scan_refdef_title<'t>(&self, text: &'t str) -> Option<(usize, CowStr<'t>)> {
        let bytes = text.as_bytes();
        let checked_value = bytes.first()?;
        let closing_delim = match checked_value {
            b'\'' => b'\'',
            b'"' => b'"',
            b'(' => b')',
            _ => return None,
        };
        let mut bytecount = 1;
        let mut linestart = 1;

        let mut linebuf = None;

        while let Some(&c) = bytes.get(bytecount) {
            match c {
                b'(' if closing_delim == b')' => {



                    return None;
                }
                b'\n' | b'\r' => {







                    let linebuf_3 = match &mut linebuf {
                        Some(linebuf_2) => {
                            linebuf_2
                        },
                        ::core::option::Option::None => {
                            linebuf = Some(String::new());
                            linebuf.as_mut().unwrap()
                        },
                    };
                    linebuf_3.push_str(&text[linestart..bytecount]);
                    linebuf_3.push('\n');

                    bytecount += 1;

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(c == b'\r' && bytes.get(bytecount) == Some(&b'\n'), ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            bytecount += 1;
                        },
                        ::core::option::Option::None => {},
                    }

                    let mut line_start = LineStart::new(&bytes[bytecount..]);
                    let tree_position =
                        scan_containers(&self.tree, &mut line_start, self.options, false);
                    let current_container = tree_position == self.tree.spine_len();

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(!line_start.scan_space(4), ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            let suffix = &text[bytecount + line_start.bytes_scanned()..];

                            match {
                                let _explicit_decision = ::core::primitive::bool::then_some(self.scan_paragraph_interrupt(suffix, current_container, tree_position)
                                || scan_setext_heading(suffix.as_bytes()).is_some(), ());
                                _explicit_decision
                            } {
                                ::core::option::Option::Some(()) => {
                                    return None;
                                },
                                ::core::option::Option::None => {},
                            }
                        },
                        ::core::option::Option::None => {},
                    }

                    line_start.scan_all_space();
                    bytecount += line_start.bytes_scanned();
                    linestart = bytecount;

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(scan_blank_line(&bytes[bytecount..]).is_some(), ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {

                            return None;
                        },
                        ::core::option::Option::None => {},
                    }
                }
                b'\\' => {
                    bytecount += 1;

                    match bytes.get(bytecount) {
                        Some(c_2) => {
                            match {
                                let _explicit_decision = ::core::primitive::bool::then_some(c_2 != &b'\r' && c_2 != &b'\n', ());
                                _explicit_decision
                            } {
                                ::core::option::Option::Some(()) => {
                                    bytecount += 1;
                                },
                                ::core::option::Option::None => {},
                            }
                        },
                        ::core::option::Option::None => {},
                    }
                }
                c_3 if c_3 == closing_delim => {
                    let cow = match linebuf {
                        Some(mut linebuf_4) => {
                            linebuf_4.push_str(&text[linestart..bytecount]);
                            CowStr::from(linebuf_4)
                        },
                        ::core::option::Option::None => {
                            CowStr::from(&text[linestart..bytecount])
                        },
                    };
                    return Some((bytecount + 1, cow));
                }
                _ => {
                    bytecount += 1;
                }
            }
        }

        None
    }

    /// Returns # of bytes and definition.
    /// Assumes the label of the reference including colon has already been scanned.
    #[must_use]
    fn scan_refdef(&self, span_start: usize, start: usize) -> Option<(usize, LinkDef<'a>)> {
        let bytes = self.text.as_bytes();


        let (mut i, _newlines) = self.scan_refdef_space(bytes, start)?;


        let (dest_length, dest) = scan_link_dest(self.text, i, LINK_MAX_NESTED_PARENS)?;

        match dest_length {
            0 => {
                return None;
            },
            _ => {},
        }

        let dest_2 = unescape(dest, self.tree.is_in_table());
        i += dest_length;


        let mut backup = (
            i - start,
            LinkDef {
                dest: dest_2,
                title: None,
                span: span_start..i,
            },
        );


        let (mut i_2, newlines) =
            match self.scan_refdef_space(bytes, i) {
                Some((new_i, mut newlines)) => {
                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(i == self.text.len(), ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            newlines += 1;
                        },
                        ::core::option::Option::None => {},
                    }

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(new_i == i && newlines == 0, ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            return None;
                        },
                        ::core::option::Option::None => {},
                    }

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(newlines > 1, ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            return Some(backup);
                        },
                        ::core::option::Option::None => {},
                    };

                    (new_i, newlines)
                },
                ::core::option::Option::None => {
                    return Some(backup);
                },
            };



        match self.scan_refdef_title(&self.text[i_2..]) {
            Some((title_length, title)) => {
                i_2 += title_length;

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(scan_blank_line(&bytes[i_2..]).is_some(), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        backup.0 = i_2 - start;
                        backup.1.span = span_start..i_2;
                        backup.1.title = Some(unescape(title, self.tree.is_in_table()));
                        return Some(backup);
                    },
                    ::core::option::Option::None => {},
                }
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(newlines > 0, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                Some(backup)
            },
            ::core::option::Option::None => {
                None
            },
        }
    }

    /// Checks whether we should break a paragraph on the given input.
    #[must_use]
    fn scan_paragraph_interrupt(
        &self,
        text: &str,
        current_container: bool,
        tree_position: usize,
    ) -> bool {
        let bytes = text.as_bytes();

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(scan_paragraph_interrupt_no_table(
                text,
                current_container,
                self.options.contains(Options::ENABLE_FOOTNOTES),
                self.options.contains(Options::ENABLE_DEFINITION_LIST),
                &self.tree,
                tree_position,
            ), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return true;
            },
            ::core::option::Option::None => {},
        }















        match {
            let _explicit_decision = ::core::primitive::bool::then_some(!self.options.contains(Options::ENABLE_TABLES) || !bytes.starts_with(b"|"), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return false;
            },
            ::core::option::Option::None => {},
        }



        let mut pipes = 0;
        let mut next_line_ix = 0;
        let mut bsesc = false;
        let mut last_pipe_ix = 0;

        for (i, &byte) in bytes.iter().enumerate() {
            match byte {
                b'\\' => {
                    bsesc = true;
                    continue;
                }
                b'|' if !bsesc => {
                    pipes += 1;
                    last_pipe_ix = i;
                }
                b'\r' | b'\n' => {
                    next_line_ix = i + scan_eol(&bytes[i..]).unwrap();
                    break;
                }
                _ => {}
            }

            bsesc = false;
        }


        match next_line_ix {
            0 => {
                return false;
            },
            _ => {},
        }













        let mut line_start = LineStart::new(&bytes[next_line_ix..]);

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(scan_containers(&self.tree, &mut line_start, self.options, false)
            != self.tree.spine_len(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return false;
            },
            ::core::option::Option::None => {},
        }

        let table_head_ix = next_line_ix + line_start.bytes_scanned();
        let (table_head_bytes, alignment) = scan_table_head(&bytes[table_head_ix..]);

        match table_head_bytes {
            0 => {
                return false;
            },
            _ => {},
        }


        let ::core::result::Result::Ok(header_count) = count_header_cols(bytes, self::CountHeaderColsArguments { pipes, start: 0, last_pipe_ix });


        alignment.len() == header_count
    }

    /// Extracts and parses a heading attribute block if exists.
    ///
    /// Returns `(end_offset_of_heading_content, (id, classes))`.
    ///
    /// If `header_end` is less than or equal to `header_start`, the given
    /// input is considered as empty.
    fn extract_and_parse_heading_attribute_block(
        &mut self,
        header_start: usize,
        header_end: usize,
    ) -> (usize, Option<HeadingAttributes<'a>>) {
        match {
            let _explicit_decision = ::core::primitive::bool::then_some(!self.options.contains(Options::ENABLE_HEADING_ATTRIBUTES), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return (header_end, None);
            },
            ::core::option::Option::None => {},
        }


        let header_bytes = &self.text.as_bytes()[header_start..header_end];
        let (content_len, attr_block_range_rel) =
            extract_attribute_block_content_from_header_text(header_bytes);
        let content_end = header_start + content_len;
        let attrs = attr_block_range_rel.and_then(|r| {
            parse_inside_attribute_block(
                &self.text[(header_start + r.start)..(header_start + r.end)],
            )
        });
        (content_end, attrs)
    }
}

/// Scanning modes for `Parser`'s `parse_line` method.
#[derive(PartialEq, Eq, Copy, Clone)]
enum TableParseMode {
    /// Inside a paragraph, scanning for table headers.
    Scan,
    /// Inside a table.
    Active,
    /// Inside a paragraph, not scanning for table headers.
    Disabled,
}

/// Computes the number of header columns in a table line by computing the number of dividing pipes
/// that aren't followed or preceded by whitespace.
fn count_header_cols(
    bytes: &[u8],
    CountHeaderColsArguments { mut pipes, mut start, last_pipe_ix }: CountHeaderColsArguments,
) -> ::core::result::Result<usize, ::core::convert::Infallible> {

    start += scan_whitespace_no_nl(&bytes[start..]);

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(bytes[start] == b'|', ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            pipes -= 1;
        },
        ::core::option::Option::None => {},
    }


    ::core::result::Result::Ok(match {
        let _explicit_decision = ::core::primitive::bool::then_some(scan_blank_line(&bytes[(last_pipe_ix + 1)..]).is_some(), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            pipes
        },
        ::core::option::Option::None => {
            pipes + 1
        },
    })
}

struct CountHeaderColsArguments {
    pipes: usize,
    start: usize,
    last_pipe_ix: usize,
}

/// Checks whether we should break a paragraph on the given input.
///
/// Use `FirstPass::scan_paragraph_interrupt` in any context that allows
/// tables to interrupt the paragraph.
#[must_use]
fn scan_paragraph_interrupt_no_table(
    text: &str,
    current_container: bool,
    has_footnote: bool,
    definition_list: bool,
    tree: &Tree<Item>,
    tree_position: usize,
) -> bool {
    let bytes = text.as_bytes();
    scan_eol(bytes).is_some()
        || scan_hrule(bytes).is_ok()
        || scan_atx_heading(bytes).is_some()
        || scan_code_fence(bytes).is_some()
        || scan_blockquote_start(bytes).is_some()
        || scan_listitem(bytes).map_or(false, |(ix, delim, index, _)| {
            ! current_container ||
            tree.is_in_table() ||


            (delim == b'*' || delim == b'-' || delim == b'+' || index == 1)
                && (scan_blank_line(&bytes[ix..]).is_none())
        })
        || bytes.starts_with(b"<")
            && (get_html_end_tag(&bytes[1..]).is_some() || starts_html_block_type_6(&bytes[1..]))
        || definition_list
            && ((current_container
                && tree.peek_up().map_or(false, |cur| {
                    matches!(
                        tree[cur].item.body,
                        ItemBody::Paragraph
                            | ItemBody::TightParagraph
                            | ItemBody::MaybeDefinitionListTitle
                    )
                }))
                || tree.walk_spine().nth(tree_position).map_or(false, |cur| {
                    matches!(tree[*cur].item.body, ItemBody::DefinitionListDefinition(_))
                }))
            && bytes.starts_with(b":")
        || (has_footnote
            && text.starts_with("[^")
            && scan_link_label_rest(&text[2..], &|_| None, tree.is_in_table())
                .map_or(false, |(len, _)| bytes.get(2 + len) == Some(&b':')))
}

/// Assumes `text_bytes` is preceded by `<`.
#[must_use]
fn get_html_end_tag(text_bytes: &[u8]) -> Option<&'static str> {
    static BEGIN_TAGS: &[&[u8]; 4] = &[b"pre", b"style", b"script", b"textarea"];

    static ST_BEGIN_TAGS: &[&[u8]; 3] = &[b"!--", b"?", b"![CDATA["];

    for (beg_tag, end_tag) in BEGIN_TAGS
        .iter()
        .zip(["</pre>", "</style>", "</script>", "</textarea>"].iter())
    {
        let tag_len = beg_tag.len();

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(text_bytes.len() < tag_len, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {

                break;
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(!text_bytes[..tag_len].eq_ignore_ascii_case(beg_tag), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                continue;
            },
            ::core::option::Option::None => {},
        }


        match {
            let _explicit_decision = ::core::primitive::bool::then_some(text_bytes.len() == tag_len, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return Some(end_tag);
            },
            ::core::option::Option::None => {},
        }


        let s = text_bytes[tag_len];

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(is_ascii_whitespace(s) || s == b'>', ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return Some(end_tag);
            },
            ::core::option::Option::None => {},
        }
    }

    for (beg_tag, end_tag) in ST_BEGIN_TAGS.iter().zip(["-->", "?>", "]]>"].iter()) {
        match {
            let _explicit_decision = ::core::primitive::bool::then_some(text_bytes.starts_with(beg_tag), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return Some(end_tag);
            },
            ::core::option::Option::None => {},
        }
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(match <[_]>::first_chunk::<2>(&*text_bytes) { ::core::option::Option::Some(&[checked_element_0, checked_element_1]) => checked_element_0 == b'!' && checked_element_1.is_ascii_alphabetic(), ::core::option::Option::None => false }, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            Some(">")
        },
        ::core::option::Option::None => {
            None
        },
    }
}

fn surgerize_tight_list(tree: &mut Tree<Item>, list_ix: TreeIndex) -> ::core::result::Result<(), ::core::convert::Infallible> {
    let mut list_item = tree[list_ix].child;
    ::core::result::Result::Ok(while let Some(listitem_ix) = list_item {
        let mut node_ix = tree[listitem_ix].child;

        while let Some(node) = node_ix {
            if let ItemBody::Paragraph = tree[node].item.body {
                tree[node].item.body = ItemBody::TightParagraph;
            }

            node_ix = tree[node].next;
        }

        list_item = tree[listitem_ix].next;
    })
}

fn fixup_end_of_definition_list(tree: &mut Tree<Item>, list_ix: TreeIndex) -> ::core::result::Result<(), ::core::convert::Infallible> {
    let mut list_item = tree[list_ix].child;
    let mut previous_list_item = None;

    while let Some(listitem_ix) = list_item {
        match &mut tree[listitem_ix].item.body {
            ItemBody::DefinitionListTitle | ItemBody::DefinitionListDefinition(_) => {
                previous_list_item = list_item;
                list_item = tree[listitem_ix].next;
            }
            body @ ItemBody::MaybeDefinitionListTitle => {
                *body = ItemBody::Paragraph;
                break;
            }
            _ => break,
        }
    }

    ::core::result::Result::Ok(match previous_list_item {
        Some(previous_list_item_2) => {
            tree.truncate_to_parent(previous_list_item_2);
        },
        ::core::option::Option::None => {},
    })
}

/// Determines whether the delimiter run starting at given index is
/// left-flanking, as defined by the commonmark spec (and isn't intraword
/// for _ delims).
/// suffix is &s[ix..], which is passed in as an optimization, since taking
/// a string subslice is O(n).
#[must_use]
fn delim_run_can_open(
    s: &str,
    suffix: &str,
    DelimRunCanOpenArguments { run_len, ix }: DelimRunCanOpenArguments,
    mode: TableParseMode,
) -> bool {
    let next_char = match suffix[run_len..].chars().next() {
        Some(c) => {
            c
        },
        ::core::option::Option::None => {
            return false;
        },
    };

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(next_char.is_whitespace(), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return false;
        },
        ::core::option::Option::None => {},
    }

    match ix {
        0 => {
            return true;
        },
        _ => {},
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(mode == TableParseMode::Active, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            match {
                let _explicit_decision = ::core::primitive::bool::then_some(s.as_bytes()[..ix].ends_with(b"|") && !s.as_bytes()[..ix].ends_with(br"\|"), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    return true;
                },
                ::core::option::Option::None => {},
            }

            match next_char {
                '|' => {
                    return false;
                },
                _ => {},
            }
        },
        ::core::option::Option::None => {},
    }

    let delim = suffix.bytes().next().unwrap();

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(delim == b'*' && !is_punctuation(next_char), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return true;
        },
        ::core::option::Option::None => {},
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(delim == b'~' && run_len > 1, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return true;
        },
        ::core::option::Option::None => {},
    }

    let prev_char = s[..ix].chars().last().unwrap();

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(delim == b'~' && prev_char == '~' && !is_punctuation(next_char), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return true;
        },
        ::core::option::Option::None => {},
    }

    prev_char.is_whitespace()
        || is_punctuation(prev_char) && (delim != b'\'' || ![']', ')'].contains(&prev_char))
}

struct DelimRunCanOpenArguments {
    run_len: usize,
    ix: usize,
}

/// Determines whether the delimiter run starting at given index is
/// right-flanking, as defined by the commonmark spec (and isn't intraword
/// for _ delims)
#[must_use]
fn delim_run_can_close(
    s: &str,
    suffix: &str,
    DelimRunCanCloseArguments { run_len, ix }: DelimRunCanCloseArguments,
    mode: TableParseMode,
) -> bool {
    match ix {
        0 => {
            return false;
        },
        _ => {},
    }

    let prev_char = s[..ix].chars().last().unwrap();

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(prev_char.is_whitespace(), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return false;
        },
        ::core::option::Option::None => {},
    }

    let next_char = match suffix[run_len..].chars().next() {
        Some(c) => {
            c
        },
        ::core::option::Option::None => {
            return true;
        },
    };

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(mode == TableParseMode::Active, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            match {
                let _explicit_decision = ::core::primitive::bool::then_some(s.as_bytes()[..ix].ends_with(b"|") && !s.as_bytes()[..ix].ends_with(br"\|"), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    return false;
                },
                ::core::option::Option::None => {},
            }

            match next_char {
                '|' => {
                    return true;
                },
                _ => {},
            }
        },
        ::core::option::Option::None => {},
    }

    let delim = suffix.bytes().next().unwrap();

    match {
        let _explicit_decision = ::core::primitive::bool::then_some((delim == b'*' || (delim == b'~' && run_len > 1)) && !is_punctuation(prev_char), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return true;
        },
        ::core::option::Option::None => {},
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(delim == b'~' && prev_char == '~', ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return true;
        },
        ::core::option::Option::None => {},
    }

    next_char.is_whitespace() || is_punctuation(next_char)
}

struct DelimRunCanCloseArguments {
    run_len: usize,
    ix: usize,
}

#[must_use]
fn special_bytes(options: &Options) -> [bool; 256] {
    let mut bytes = [false; 256];
    let standard_bytes = [
        b'\n', b'\r', b'*', b'_', b'&', b'\\', b'[', b']', b'<', b'!', b'`',
    ];

    for &byte in &standard_bytes {
        bytes[<usize as ::core::convert::From<::core::primitive::u8>>::from(byte)] = true;
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(options.contains(Options::ENABLE_TABLES), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            bytes[<usize as ::core::convert::From<::core::primitive::u8>>::from(b'|')] = true;
        },
        ::core::option::Option::None => {},
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(options.contains(Options::ENABLE_STRIKETHROUGH)
        || options.contains(Options::ENABLE_SUBSCRIPT), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            bytes[<usize as ::core::convert::From<::core::primitive::u8>>::from(b'~')] = true;
        },
        ::core::option::Option::None => {},
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(options.contains(Options::ENABLE_SUPERSCRIPT), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            bytes[<usize as ::core::convert::From<::core::primitive::u8>>::from(b'^')] = true;
        },
        ::core::option::Option::None => {},
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(options.contains(Options::ENABLE_MATH), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            bytes[<usize as ::core::convert::From<::core::primitive::u8>>::from(b'$')] = true;
            bytes[<usize as ::core::convert::From<::core::primitive::u8>>::from(b'{')] = true;
            bytes[<usize as ::core::convert::From<::core::primitive::u8>>::from(b'}')] = true;
        },
        ::core::option::Option::None => {},
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(options.contains(Options::ENABLE_SMART_PUNCTUATION), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            for &byte in &[b'.', b'-', b'"', b'\''] {
                bytes[<usize as ::core::convert::From<::core::primitive::u8>>::from(byte)] = true;
            }
        },
        ::core::option::Option::None => {},
    }

    bytes
}

enum LoopInstruction<T> {
    /// Continue looking for more special bytes, but skip next few bytes.
    ContinueAndSkip(usize),
    /// Break looping immediately, returning with the given index and value.
    BreakAtWith(usize, T),
}

type LookupTable = [bool; 256];

/// This function walks the byte slices from the given index and
/// calls the callback function on all bytes (and their indices) that are in the following set:
/// `` ` ``, `\`, `&`, `*`, `_`, `~`, `!`, `<`, `[`, `]`, `|`, `\r`, `\n`
/// It is guaranteed not call the callback on other bytes.
/// Whenever `callback(ix, byte)` returns a `ContinueAndSkip(n)` value, the callback
/// will not be called with an index that is less than `ix + n + 1`.
/// When the callback returns a `BreakAtWith(end_ix, opt+val)`, no more callbacks will be
/// called and the function returns immediately with the return value `(end_ix, opt_val)`.
/// If `BreakAtWith(..)` is never returned, this function will return the first
/// index that is outside the byteslice bound and a `None` value.
#[must_use]
fn iterate_special_bytes<F, T>(
    lut: &[bool; 256],
    bytes: &[u8],
    mut ix: usize,
    mut callback: F,
) -> (usize, Option<T>)
where
    F: FnMut(usize, u8) -> LoopInstruction<Option<T>>,
{
    while ix < bytes.len() {
        let b = bytes[ix];

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(lut[<usize as ::core::convert::From<::core::primitive::u8>>::from(b)], ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                match callback(ix, b) {
                    LoopInstruction::ContinueAndSkip(skip) => {
                        ix += skip;
                    }
                    LoopInstruction::BreakAtWith(ix_2, val) => {
                        return (ix_2, val);
                    }
                }
            },
            ::core::option::Option::None => {},
        }

        ix += 1;
    }

    (ix, None)
}

/// Split the usual heading content range and the content inside the trailing attribute block.
///
/// Returns `(leading_content_len, Option<trailing_attr_block_range>)`.
///
/// Note that `trailing_attr_block_range` will be empty range when the block
/// is `{}`, since the range is content inside the wrapping `{` and `}`.
///
/// The closing `}` of an attribute block can have trailing whitespaces.
/// They are automatically trimmed when the attribute block is being searched.
///
/// However, this method does not trim the trailing whitespaces of heading content.
/// It is callers' responsibility to trim them if necessary.
#[must_use]
fn extract_attribute_block_content_from_header_text(
    heading: &[u8],
) -> (usize, Option<Range<usize>>) {
    let heading_len = heading.len();
    let mut ix = heading_len;
    ix -= scan_rev_while(heading, |b| {
        b == b'\n' || b == b'\r' || b == b' ' || b == b'\t'
    });

    match ix {
        0 => {
            return (heading_len, None);
        },
        _ => {},
    }

    let attr_block_close = ix - 1;

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(heading.get(attr_block_close) != Some(&b'}'), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {

            return (heading_len, None);
        },
        ::core::option::Option::None => {},
    }


    ix -= 1;

    ix -= scan_rev_while(&heading[..ix], |b| {














        !matches!(b, b'{' | b'}' | b'<' | b'>' | b'\\' | b'\n' | b'\r')
    });

    match ix {
        0 => {

            return (heading_len, None);
        },
        _ => {},
    }

    let attr_block_open = ix - 1;

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(heading[attr_block_open] != b'{', ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {

            return (heading_len, None);
        },
        ::core::option::Option::None => {},
    }

    (attr_block_open, Some(ix..attr_block_close))
}

/// Parses an attribute block content, such as `.class1 #id .class2`.
///
/// Returns `(id, classes)`.
///
/// It is callers' responsibility to find opening and closing characters of the attribute
/// block. Usually [`extract_attribute_block_content_from_header_text`] function does it for you.
///
/// Note that this parsing requires explicit whitespace separators between
/// attributes. This is intentional design with the reasons below:
///
/// * to keep conversion simple and easy to understand for any possible input,
/// * to avoid adding less obvious conversion rule that can reduce compatibility
///   with other implementations more, and
/// * to follow the major design of implementations with the support for the
///   attribute blocks extension (as of this writing).
///
/// See also: [`Options::ENABLE_HEADING_ATTRIBUTES`].
///
/// [`Options::ENABLE_HEADING_ATTRIBUTES`]: `crate::Options::ENABLE_HEADING_ATTRIBUTES`
#[must_use]
fn parse_inside_attribute_block(inside_attr_block: &str) -> Option<HeadingAttributes<'_>> {
    let mut id = None;
    let mut classes = Vec::new();
    let mut attrs = Vec::new();

    for attr in inside_attr_block.split_ascii_whitespace() {


        match {
            let _explicit_decision = ::core::primitive::bool::then_some(attr.len() > 1, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                let first_byte = attr.as_bytes()[0];

                match first_byte {
                    b'#' => {
                        id = Some(attr[1..].into());
                    },
                    _ => match first_byte {
                        b'.' => {
                            classes.push(attr[1..].into());
                        },
                        _ => {
                            let split = attr.split_once('=');

                            match split {
                                Some((key, value)) => {
                                    attrs.push((<crate::strings::CowStr<'_> as ::core::convert::From<&::core::primitive::str>>::from(key), Some(<crate::strings::CowStr<'_> as ::core::convert::From<&::core::primitive::str>>::from(value))));
                                },
                                ::core::option::Option::None => {
                                    attrs.push((<crate::strings::CowStr<'_> as ::core::convert::From<&::core::primitive::str>>::from(attr), None));
                                },
                            }
                        },
                    },
                }
            },
            ::core::option::Option::None => {},
        }
    }

    Some(HeadingAttributes { id, classes, attrs })
}
