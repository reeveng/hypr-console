// Copyright 2015 Google Inc. All rights reserved.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

//! Scanners for fragments of CommonMark syntax

use std::char;

use crate::parse::HtmlScanGuard;
pub(crate) use crate::puncttable::{is_ascii_punctuation, is_punctuation};
use crate::strings::CowStr;
use crate::{entities, BlockQuoteKind, HeadingLevel};
use crate::{Alignment, LinkType};

use memchr::memchr;


const HTML_TAGS: [&str; 62] = [
    "address",
    "article",
    "aside",
    "base",
    "basefont",
    "blockquote",
    "body",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "iframe",
    "legend",
    "li",
    "link",
    "main",
    "menu",
    "menuitem",
    "nav",
    "noframes",
    "ol",
    "optgroup",
    "option",
    "p",
    "param",
    "search",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
];

/// Analysis of the beginning of a line, including indentation and container
/// markers.
#[derive(Clone)]
pub(crate) struct LineStart<'a> {
    bytes: &'a [u8],
    ix: usize,











    tab_start: usize,








    spaces_remaining: usize,



    min_hrule_offset: usize,
}

impl<'a> LineStart<'a> {
    #[must_use]
    pub(crate) fn new(bytes: &[u8]) -> LineStart<'_> {
        LineStart {
            bytes,
            tab_start: 0,
            ix: 0,
            spaces_remaining: 0,
            min_hrule_offset: 0,
        }
    }

    /// Try to scan a number of spaces.
    ///
    /// Returns true if all spaces were consumed.
    ///
    /// Note: consumes some spaces even if not successful.
    pub(crate) fn scan_space(&mut self, n_space: usize) -> bool {
        self.scan_space_inner(n_space) == 0
    }

    /// Scan a number of spaces up to a maximum.
    ///
    /// Returns number of spaces scanned.
    pub(crate) fn scan_space_upto(&mut self, n_space: usize) -> usize {
        n_space - self.scan_space_inner(n_space)
    }

    /// Returns unused remainder of spaces.
    fn scan_space_inner(&mut self, mut n_space: usize) -> usize {


        let n_from_remaining = self.spaces_remaining.min(n_space);
        self.spaces_remaining -= n_from_remaining;
        n_space -= n_from_remaining;

        while n_space > 0 && self.ix < self.bytes.len() {
            match self.bytes[self.ix] {
                b' ' => {
                    self.ix += 1;
                    n_space -= 1;
                }
                b'\t' => {
                    let spaces = (::core::num::Wrapping(4) - ::core::num::Wrapping((self.ix - self.tab_start).wrapping_rem(4))).0;
                    self.ix += 1;
                    self.tab_start = self.ix;
                    let n = spaces.min(n_space);
                    n_space -= n;


                    self.spaces_remaining = spaces - n;
                }
                _ => break,
            }
        }

        n_space
    }

    /// Scan all available ASCII whitespace (not including eol).
    pub(crate) fn scan_all_space(&mut self) {
        self.spaces_remaining = 0;
        self.ix += self.bytes[self.ix..]
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();
    }

    /// Determine whether we're at end of line (includes end of file).
    #[must_use]
    pub(crate) fn is_at_eol(&self) -> bool {
        match self.bytes
            .get(self.ix)
            .map(|&c| c == b'\r' || c == b'\n') {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => true,
        }
    }

    fn scan_ch(&mut self, c: u8) -> bool {
        match {
            let _explicit_decision = ::core::primitive::bool::then_some(match <[_]>::get(&*self.bytes, self.ix) { ::core::option::Option::Some(&checked_element) => checked_element == c, ::core::option::Option::None => false }, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                self.ix += 1;
                true
            },
            ::core::option::Option::None => {
                false
            },
        }
    }

    fn scan_case_insensitive(&mut self, tag: &[u8]) -> bool {
        match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.bytes.len() - self.ix < tag.len(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return false;
            },
            ::core::option::Option::None => {},
        }

        let prefix = &self.bytes[self.ix..self.ix + tag.len()];
        let ok = prefix.eq_ignore_ascii_case(tag);

        match ::core::primitive::bool::then_some(ok, ()) {
            ::core::option::Option::Some(()) => {
                self.ix += tag.len();
            },
            ::core::option::Option::None => {},
        }

        ok
    }

    pub(crate) fn scan_blockquote_tag(&mut self) -> Option<BlockQuoteKind> {
        let saved_ix = self.ix;
        let tag = match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.scan_ch(b'[') && self.scan_ch(b'!'), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                let tag = match {
                    let _explicit_decision = ::core::primitive::bool::then_some(self.scan_case_insensitive(b"note"), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        Some(BlockQuoteKind::Note)
                    },
                    ::core::option::Option::None => match {
                        let _explicit_decision = ::core::primitive::bool::then_some(self.scan_case_insensitive(b"tip"), ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            Some(BlockQuoteKind::Tip)
                        },
                        ::core::option::Option::None => match {
                            let _explicit_decision = ::core::primitive::bool::then_some(self.scan_case_insensitive(b"important"), ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                Some(BlockQuoteKind::Important)
                            },
                            ::core::option::Option::None => match {
                                let _explicit_decision = ::core::primitive::bool::then_some(self.scan_case_insensitive(b"warning"), ());
                                _explicit_decision
                            } {
                                ::core::option::Option::Some(()) => {
                                    Some(BlockQuoteKind::Warning)
                                },
                                ::core::option::Option::None => match {
                                    let _explicit_decision = ::core::primitive::bool::then_some(self.scan_case_insensitive(b"caution"), ());
                                    _explicit_decision
                                } {
                                    ::core::option::Option::Some(()) => {
                                        Some(BlockQuoteKind::Caution)
                                    },
                                    ::core::option::Option::None => {
                                        None
                                    },
                                },
                            },
                        },
                    },
                };

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(tag.is_some() && self.scan_ch(b']'), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        match scan_blank_line(&self.bytes[self.ix..]) {
                            Some(nl) => {
                                self.ix += nl;
                                tag
                            },
                            ::core::option::Option::None => {
                                None
                            },
                        }
                    },
                    ::core::option::Option::None => {
                        None
                    },
                }
            },
            ::core::option::Option::None => {
                None
            },
        };

        match tag {
            ::core::option::Option::None => {
                self.ix = saved_ix;
            },
            ::core::option::Option::Some(_) => {},
        }

        tag
    }

    pub(crate) fn scan_blockquote_marker(&mut self) -> bool {
        match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.scan_ch(b'>'), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                let _ = self.scan_space(1);
                true
            },
            ::core::option::Option::None => {
                false
            },
        }
    }

    /// Scan a definition marker.
    ///
    /// Definition markers are single colons, preceded by at most three spaces
    /// and followed by at most three spaces. The indentation of following
    /// lines is equal to the whole size of the marker, including the colon.
    ///
    /// If one is found, it will make the preceding paragraph into a definition
    /// list title.
    ///
    /// Return value is the amount of indentation, or `None` if it's not a
    /// definition list marker.
    pub(crate) fn scan_definition_list_definition_marker_with_indent(
        &mut self,
        indent: usize,
    ) -> Option<usize> {
        let save = self.clone();

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.scan_ch(b':'), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                let save_2 = self.clone();

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(self.scan_space(5), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        *self = save_2;
                        Some(indent + 1 + self.scan_space_upto(1))
                    },
                    ::core::option::Option::None => {
                        *self = save_2;
                        Some(indent + 1 + self.scan_space_upto(5))
                    },
                }
            },
            ::core::option::Option::None => {
                *self = save;
                None
            },
        }
    }

    /// Scan a list marker.
    ///
    /// Return value is the character, the start index, and the indent in spaces.
    /// For ordered list markers, the character will be one of b'.' or b')'. For
    /// bullet list markers, it will be one of b'-', b'+', or b'*'.
    pub(crate) fn scan_list_marker_with_indent(
        &mut self,
        indent: usize,
    ) -> Option<(u8, u64, usize)> {
        let save = self.clone();

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.ix < self.bytes.len(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                let c = self.bytes[self.ix];

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(c == b'-' || c == b'+' || c == b'*', ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        match {
                            let _explicit_decision = ::core::primitive::bool::then_some(self.ix >= self.min_hrule_offset, ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {

                                match scan_hrule(&self.bytes[self.ix..]) {
                                    Err(min_offset) => {
                                        self.min_hrule_offset = min_offset;
                                    },
                                    ::core::result::Result::Ok(_) => {
                                        *self = save;
                                        return None;
                                    },
                                }
                            },
                            ::core::option::Option::None => {},
                        }

                        self.ix += 1;

                        match {
                            let _explicit_decision = ::core::primitive::bool::then_some(self.scan_space(1) || self.is_at_eol(), ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                return self.finish_list_marker(c, 0, indent + 2);
                            },
                            ::core::option::Option::None => {},
                        }
                    },
                    ::core::option::Option::None => match {
                        let _explicit_decision = ::core::primitive::bool::then_some(c.is_ascii_digit(), ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            let start_ix = self.ix;
                            let mut ix = self.ix + 1;
                            let mut val = u64::from(c - b'0');

                            while ix < self.bytes.len() && ix - start_ix < 10 {
                                let c_2 = self.bytes[ix];
                                ix += 1;

                                match {
                                    let _explicit_decision = ::core::primitive::bool::then_some(c_2.is_ascii_digit(), ());
                                    _explicit_decision
                                } {
                                    ::core::option::Option::Some(()) => {
                                        val = val * 10 + u64::from(c_2 - b'0');
                                    },
                                    ::core::option::Option::None => match {
                                        let _explicit_decision = ::core::primitive::bool::then_some(c_2 == b')' || c_2 == b'.', ());
                                        _explicit_decision
                                    } {
                                        ::core::option::Option::Some(()) => {
                                            self.ix = ix;

                                            match {
                                                let _explicit_decision = ::core::primitive::bool::then_some(self.scan_space(1) || self.is_at_eol(), ());
                                                _explicit_decision
                                            } {
                                                ::core::option::Option::Some(()) => {
                                                    return self.finish_list_marker(c_2, val, indent + 1 + ix - start_ix);
                                                },
                                                ::core::option::Option::None => {
                                                    break;
                                                },
                                            }
                                        },
                                        ::core::option::Option::None => {
                                            break;
                                        },
                                    },
                                }
                            }
                        },
                        ::core::option::Option::None => {},
                    },
                }
            },
            ::core::option::Option::None => {},
        }

        *self = save;
        None
    }

    fn finish_list_marker(
        &mut self,
        c: u8,
        start: u64,
        mut indent: usize,
    ) -> Option<(u8, u64, usize)> {
        let save = self.clone();


        match {
            let _explicit_decision = ::core::primitive::bool::then_some(scan_blank_line(&self.bytes[self.ix..]).is_some(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return Some((c, start, indent));
            },
            ::core::option::Option::None => {},
        }

        let post_indent = self.scan_space_upto(4);

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(post_indent < 4, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                indent += post_indent;
            },
            ::core::option::Option::None => {
                *self = save;
            },
        }

        Some((c, start, indent))
    }

    /// Returns Some(is_checked) when a task list marker was found. Resets itself
    /// to original state otherwise.
    pub(crate) fn scan_task_list_marker(&mut self) -> Option<bool> {
        let save = self.clone();
        self.scan_space_upto(3);

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(!self.scan_ch(b'['), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                *self = save;
                return None;
            },
            ::core::option::Option::None => {},
        }

        let is_checked = match self.bytes.get(self.ix) {
            Some(&c) if is_ascii_whitespace_no_nl(c) => {
                self.ix += 1;
                false
            }
            Some(b'x') | Some(b'X') => {
                self.ix += 1;
                true
            }
            _ => {
                *self = save;
                return None;
            }
        };

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(!self.scan_ch(b']'), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                *self = save;
                return None;
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(!(match self
            .bytes
            .get(self.ix)
            .map(|&b| is_ascii_whitespace(b)) {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => false,
            }), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                *self = save;
                return None;
            },
            ::core::option::Option::None => {},
        }

        Some(is_checked)
    }

    #[must_use]
    pub(crate) fn bytes_scanned(&self) -> usize {
        self.ix
    }

    #[must_use]
    pub(crate) fn remaining_space(&self) -> usize {
        self.spaces_remaining
    }
}

#[must_use]
pub(crate) fn is_ascii_whitespace(c: u8) -> bool {
    (0x09..=0x0d).contains(&c) || c == b' '
}

#[must_use]
pub(crate) fn is_ascii_whitespace_no_nl(c: u8) -> bool {
    c == b'\t' || c == 0x0b || c == 0x0c || c == b' '
}

#[must_use]
fn is_ascii_alpha(c: u8) -> bool {
    c.is_ascii_alphabetic()
}

#[must_use]
fn is_ascii_alphanumeric(c: u8) -> bool {
    matches!(c, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z')
}

#[must_use]
fn is_ascii_letterdigitdash(c: u8) -> bool {
    c == b'-' || is_ascii_alphanumeric(c)
}

#[must_use]
fn is_digit(c: u8) -> bool {
    c.is_ascii_digit()
}

#[must_use]
fn is_valid_unquoted_attr_value_char(c: u8) -> bool {
    !matches!(
        c,
        b'\'' | b'"' | b' ' | b'=' | b'>' | b'<' | b'`' | b'\n' | b'\r'
    )
}


#[must_use]
pub(crate) fn scan_ch(data: &[u8], c: u8) -> usize {
    match {
        let _explicit_decision = ::core::primitive::bool::then_some(match <[_]>::get(&*data, 0) { ::core::option::Option::Some(&checked_element) => checked_element == c, ::core::option::Option::None => false }, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            1
        },
        ::core::option::Option::None => {
            0
        },
    }
}

#[must_use]
pub(crate) fn scan_while<F>(data: &[u8], mut f: F) -> usize
where
    F: FnMut(u8) -> bool,
{
    data.iter().take_while(|&&c| f(c)).count()
}

#[must_use]
pub(crate) fn scan_rev_while<F>(data: &[u8], mut f: F) -> usize
where
    F: FnMut(u8) -> bool,
{
    data.iter().rev().take_while(|&&c| f(c)).count()
}

#[must_use]
pub(crate) fn scan_ch_repeat(data: &[u8], c: u8) -> usize {
    scan_while(data, |x| x == c)
}



#[must_use]
pub(crate) fn scan_whitespace_no_nl(data: &[u8]) -> usize {
    scan_while(data, is_ascii_whitespace_no_nl)
}

#[must_use]
fn scan_attr_value_chars(data: &[u8]) -> usize {
    scan_while(data, is_valid_unquoted_attr_value_char)
}

#[must_use]
pub(crate) fn scan_eol(bytes: &[u8]) -> Option<usize> {
    match bytes {
        &[] => Some(0),
        &[b'\n', ..] => Some(1),
        &[b'\r', b'\n', ..] => Some(2),
        &[b'\r', ..] => Some(1),
        _ => None,
    }
}

#[must_use]
pub(crate) fn scan_blank_line(bytes: &[u8]) -> Option<usize> {
    let i = scan_whitespace_no_nl(bytes);
    scan_eol(&bytes[i..]).map(|n| i + n)
}

#[must_use]
pub(crate) fn scan_nextline(bytes: &[u8]) -> usize {
    memchr(b'\n', bytes).map_or(bytes.len(), |x| x + 1)
}



#[must_use]
pub(crate) fn scan_closing_code_fence(
    bytes: &[u8],
    fence_char: u8,
    n_fence_char: usize,
) -> Option<usize> {
    match {
        let _explicit_decision = ::core::primitive::bool::then_some(bytes.is_empty(), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return Some(0);
        },
        ::core::option::Option::None => {},
    }

    let mut i = 0;
    let num_fence_chars_found = scan_ch_repeat(&bytes[i..], fence_char);

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(num_fence_chars_found < n_fence_char, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return None;
        },
        ::core::option::Option::None => {},
    }

    i += num_fence_chars_found;
    let num_trailing_spaces = scan_ch_repeat(&bytes[i..], b' ');
    i += num_trailing_spaces;
    scan_eol(&bytes[i..]).map(|_| i)
}



#[must_use]
pub(crate) fn scan_closing_metadata_block(bytes: &[u8], fence_char: u8) -> Option<usize> {
    let mut i = 0;
    let mut num_fence_chars_found = scan_ch_repeat(&bytes[i..], fence_char);

    match num_fence_chars_found {
        3 => {},
        _ => {

            match fence_char {
                b'-' => {
                    num_fence_chars_found = scan_ch_repeat(&bytes[i..], b'.');

                    match num_fence_chars_found {
                        3 => {},
                        _ => {
                            return None;
                        },
                    }
                },
                _ => {
                    return None;
                },
            }
        },
    }

    i += num_fence_chars_found;
    let num_trailing_spaces = scan_ch_repeat(&bytes[i..], b' ');
    i += num_trailing_spaces;
    scan_eol(&bytes[i..]).map(|_| i)
}


#[must_use]
pub(crate) fn calc_indent(text: &[u8], max: usize) -> (usize, usize) {
    let mut spaces = 0;
    let mut offset = 0;

    for (i, &b) in text.iter().enumerate() {
        offset = i;

        match b {
            b' ' => {
                spaces += 1;

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(spaces == max, ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        break;
                    },
                    ::core::option::Option::None => {},
                }
            }
            b'\t' => {
                let new_spaces = spaces + 4 - (spaces & 3);

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(new_spaces > max, ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        break;
                    },
                    ::core::option::Option::None => {},
                }

                spaces = new_spaces;
            }
            _ => break,
        }
    }

    (offset, spaces)
}

/// Scan hrule opening sequence.
///
/// Returns Ok(x) when it finds an hrule, where x is the
/// size of line containing the hrule, including the trailing newline.
///
/// Returns Err(x) when it does not find an hrule and x is
/// the offset in data before no hrule can appear.
pub(crate) fn scan_hrule(bytes: &[u8]) -> Result<usize, usize> {
    match {
        let _explicit_decision = ::core::primitive::bool::then_some(bytes.len() < 3, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return Err(0);
        },
        ::core::option::Option::None => {},
    }

    let c = bytes[0];

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(!(c == b'*' || c == b'-' || c == b'_'), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return Err(0);
        },
        ::core::option::Option::None => {},
    }

    let mut n = 0;
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'\n' | b'\r' => {
                i += match scan_eol(&bytes[i..]) {
                    ::core::option::Option::Some(present) => present,
                    ::core::option::Option::None => 0,
                };
                break;
            }
            c2 if c2 == c => {
                n += 1;
            }
            b' ' | b'\t' => (),
            _ => return Err(i),
        }

        i += 1;
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(n >= 3, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            Ok(i)
        },
        ::core::option::Option::None => {
            Err(i)
        },
    }
}

/// Scan an ATX heading opening sequence.
///
/// Returns number of bytes in prefix and level.
#[must_use]
pub(crate) fn scan_atx_heading(data: &[u8]) -> Option<HeadingLevel> {
    let level = scan_ch_repeat(data, b'#');

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(data.get(level).copied().map_or(true, is_ascii_whitespace), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            HeadingLevel::try_from(level).ok()
        },
        ::core::option::Option::None => {
            None
        },
    }
}

/// Scan a setext heading underline.
///
/// Returns number of bytes in line (including trailing newline) and level.
#[must_use]
pub(crate) fn scan_setext_heading(data: &[u8]) -> Option<(usize, HeadingLevel)> {
    let c = ::core::option::Option::<&_>::copied(data.first())?;
    let level = match c {
        b'=' => {
            HeadingLevel::H1
        },
        _ => match c {
            b'-' => {
                HeadingLevel::H2
            },
            _ => {
                return None;
            },
        },
    };
    let mut i = 1 + scan_ch_repeat(&data[1..], c);
    i += scan_blank_line(&data[i..])?;
    Some((i, level))
}



#[must_use]
pub(crate) fn scan_table_head(data: &[u8]) -> (usize, Vec<Alignment>) {
    let (mut i, spaces) = calc_indent(data, 4);

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(spaces > 3 || i == data.len(), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return (0, vec![]);
        },
        ::core::option::Option::None => {},
    }

    let mut cols = vec![];
    let mut active_col = Alignment::None;
    let mut start_col = true;
    let mut found_pipe = false;
    let mut found_hyphen = false;
    let mut found_hyphen_in_col = false;

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(data[i] == b'|', ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            i += 1;
            found_pipe = true;
        },
        ::core::option::Option::None => {},
    }

    for c in &data[i..] {
        match scan_eol(&data[i..]) {
            Some(n) => {
                i += n;
                break;
            },
            ::core::option::Option::None => {},
        }

        match *c {
            b' ' => (),
            b':' => {
                active_col = match (start_col, active_col) {
                    (true, Alignment::None) => Alignment::Left,
                    (false, Alignment::Left) => Alignment::Center,
                    (false, Alignment::None) => Alignment::Right,
                    _ => active_col,
                };
                start_col = false;
            }
            b'-' => {
                start_col = false;
                found_hyphen = true;
                found_hyphen_in_col = true;
            }
            b'|' => {
                start_col = true;
                found_pipe = true;
                cols.push(active_col);
                active_col = Alignment::None;

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(!found_hyphen_in_col, ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {

                        return (0, vec![]);
                    },
                    ::core::option::Option::None => {},
                }

                found_hyphen_in_col = false;
            }
            _ => {

                return (0, vec![]);
            }
        }

        i += 1;
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(!start_col, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            cols.push(active_col);
        },
        ::core::option::Option::None => {},
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(!found_pipe || !found_hyphen, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {


            return (0, vec![]);
        },
        ::core::option::Option::None => {},
    }

    (i, cols)
}

/// Scan code fence.
///
/// Returns number of bytes scanned and the char that is repeated to make the code fence.
#[must_use]
pub(crate) fn scan_code_fence(data: &[u8]) -> Option<(usize, u8)> {
    let c = ::core::option::Option::<&_>::copied(data.first())?;

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(!(c == b'`' || c == b'~'), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return None;
        },
        ::core::option::Option::None => {},
    }

    let i = 1 + scan_ch_repeat(&data[1..], c);

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(i >= 3, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            match c {
                b'`' => {
                    let suffix = &data[i..];
                    let next_line = i + scan_nextline(suffix);

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some(suffix[..(next_line - i)].iter().any(|&b| b == b'`'), ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            return None;
                        },
                        ::core::option::Option::None => {},
                    }
                },
                _ => {},
            }

            Some((i, c))
        },
        ::core::option::Option::None => {
            None
        },
    }
}

/// Scan metadata block, returning the number of delimiter bytes
/// (always 3 for now) and the delimiter character.
///
/// Differently to code blocks, metadata blocks must be closed with the closing
/// sequence not being a valid terminator the end of the file.
///
/// In addition, they cannot be empty (closing sequence in the next line) and
/// the next line cannot be an empty line.
#[must_use]
pub(crate) fn scan_metadata_block(
    data: &[u8],
    yaml_style_enabled: bool,
    pluses_style_enabled: bool,
) -> Option<(usize, u8)> {

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(yaml_style_enabled || pluses_style_enabled, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            let c = ::core::option::Option::<&_>::copied(data.first())?;

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(!((c == b'-' && yaml_style_enabled) || (c == b'+' && pluses_style_enabled)), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    return None;
                },
                ::core::option::Option::None => {},
            }

            let i = 1 + scan_ch_repeat(&data[1..], c);

            let next_line = scan_nextline(&data[i..]);

            for c_2 in &data[i..i + next_line] {
                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(!c_2.is_ascii_whitespace(), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        return None;
                    },
                    ::core::option::Option::None => {},
                }
            }

            match i {
                3 => {

                    let mut j = i;
                    let mut first_line = true;

                    while j < data.len() {
                        j += scan_nextline(&data[j..]);
                        let closed = scan_closing_metadata_block(&data[j..], c).is_some();


                        match ::core::primitive::bool::then_some(first_line, ()) {
                            ::core::option::Option::Some(()) => {
                                match {
                                    let _explicit_decision = ::core::primitive::bool::then_some(closed || scan_blank_line(&data[j..]).is_some(), ());
                                    _explicit_decision
                                } {
                                    ::core::option::Option::Some(()) => {
                                        return None;
                                    },
                                    ::core::option::Option::None => {},
                                }

                                first_line = false;
                            },
                            ::core::option::Option::None => {},
                        }

                        match ::core::primitive::bool::then_some(closed, ()) {
                            ::core::option::Option::Some(()) => {
                                return Some((i, c));
                            },
                            ::core::option::Option::None => {},
                        }
                    }

                    None
                },
                _ => {
                    None
                },
            }
        },
        ::core::option::Option::None => {
            None
        },
    }
}

#[must_use]
pub(crate) fn scan_blockquote_start(data: &[u8]) -> Option<usize> {
    match {
        let _explicit_decision = ::core::primitive::bool::then_some(data.first().copied() == Some(b'>'), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            let space = match {
                let _explicit_decision = ::core::primitive::bool::then_some(data.get(1).copied() == Some(b' '), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    1
                },
                ::core::option::Option::None => {
                    0
                },
            };
            Some(1 + space)
        },
        ::core::option::Option::None => {
            None
        },
    }
}

/// return number of bytes scanned, delimiter, start index, and indent
#[must_use]
pub(crate) fn scan_listitem(bytes: &[u8]) -> Option<(usize, u8, usize, usize)> {
    let checked_value = bytes.first()?;
    let mut c = *checked_value;
    let (w, start) = match c {
        b'-' | b'+' | b'*' => (1, 0),
        b'0'..=b'9' => {
            let (length, start) = parse_decimal(bytes, 9);
            let checked_value_2 = bytes.get(length)?;
            c = *checked_value_2;

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(!(c == b'.' || c == b')'), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    return None;
                },
                ::core::option::Option::None => {},
            }

            (length + 1, start)
        }
        _ => {
            return None;
        }
    };

    let (mut postn, mut postindent) = calc_indent(&bytes[w..], 5);

    match postindent {
        0 => {
            scan_eol(&bytes[w..])?;
            postindent += 1;
        },
        _ => match {
            let _explicit_decision = ::core::primitive::bool::then_some(postindent > 4, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                postn = 1;
                postindent = 1;
            },
            ::core::option::Option::None => {},
        },
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(scan_blank_line(&bytes[w..]).is_some(), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            postn = 0;
            postindent = 1;
        },
        ::core::option::Option::None => {},
    }

    Some((w + postn, c, start, w + postindent))
}


#[must_use]
fn parse_decimal(bytes: &[u8], limit: usize) -> (usize, usize) {
    match bytes
        .iter()
        .take(limit)
        .take_while(|&&b| is_digit(b))
        .try_fold((0, 0usize), |(count, acc), c| {
            let digit = usize::from(c - b'0');

            match acc
                .checked_mul(10)
                .and_then(|ten_acc| ten_acc.checked_add(digit))
            {
                Some(number) => Ok((count + 1, number)),

                None => Err((count, acc)),
            }
        }) {
        Ok(p) | Err(p) => p,
    }
}


#[must_use]
fn parse_hex(bytes: &[u8], limit: usize) -> (usize, usize) {
    match bytes
        .iter()
        .take(limit)
        .try_fold((0, 0usize), |(count, acc), c| {
            let mut c_2 = *c;
            let digit = match {
                let _explicit_decision = ::core::primitive::bool::then_some(c_2.is_ascii_digit(), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    usize::from(c_2 - b'0')
                },
                ::core::option::Option::None => {

                    c_2 |= 0x20;

                    match {
                        let _explicit_decision = ::core::primitive::bool::then_some((b'a'..=b'f').contains(&c_2), ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            usize::from(c_2 - b'a' + 10)
                        },
                        ::core::option::Option::None => {
                            return Err((count, acc));
                        },
                    }
                },
            };

            match acc
                .checked_mul(16)
                .and_then(|sixteen_acc| sixteen_acc.checked_add(digit))
            {
                Some(number) => Ok((count + 1, number)),

                None => Err((count, acc)),
            }
        }) {
        Ok(p) | Err(p) => p,
    }
}

#[must_use]
fn char_from_codepoint(input: usize) -> Option<char> {
    let codepoint = match input.try_into() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None }?;

    match codepoint {
        0 => {
            return None;
        },
        _ => {},
    }

    char::from_u32(codepoint)
}


#[must_use]
pub(crate) fn scan_entity(bytes: &[u8]) -> (usize, Option<CowStr<'static>>) {
    let mut end = 1;

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(bytes.get(end) == Some(&b'#'), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            end += 1;
            let (bytecount, codepoint) = match {
                let _explicit_decision = ::core::primitive::bool::then_some(match <[_]>::get(&*bytes, end) { ::core::option::Option::Some(&checked_element) => checked_element | 0x20 == b'x', ::core::option::Option::None => false }, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    end += 1;
                    parse_hex(&bytes[end..], 6)
                },
                ::core::option::Option::None => {
                    parse_decimal(&bytes[end..], 7)
                },
            };
            end += bytecount;
            return match {
                let _explicit_decision = ::core::primitive::bool::then_some(bytecount == 0 || bytes.get(end) != Some(&b';'), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    (0, None)
                },
                ::core::option::Option::None => {
                    (
                        end + 1,
                        Some(<crate::strings::CowStr<'_> as ::core::convert::From<::core::primitive::char>>::from(match char_from_codepoint(codepoint) {
                            ::core::option::Option::Some(present) => present,
                            ::core::option::Option::None => '\u{FFFD}',
                        })),
                    )
                },
            };
        },
        ::core::option::Option::None => {},
    }

    end += scan_while(&bytes[end..], is_ascii_alphanumeric);

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(bytes.get(end) == Some(&b';'), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            match entities::get_entity(&bytes[1..end]) {
                Some(value) => {
                    return (end + 1, Some(<crate::strings::CowStr<'_> as ::core::convert::From<&::core::primitive::str>>::from(value)));
                },
                ::core::option::Option::None => {},
            }
        },
        ::core::option::Option::None => {},
    }

    (0, None)
}

#[must_use]
pub(crate) fn scan_wikilink_pipe(data: &str, start_ix: usize, len: usize) -> Option<(usize, &str)> {
    let bytes = data.as_bytes();
    let end_ix = std::cmp::min(start_ix + len, bytes.len());
    let mut i = start_ix;

    for _ in (::core::ops::Range { start: i, end: end_ix }) {
        match {
            let _explicit_decision = ::core::primitive::bool::then_some(bytes[i] == b'|', ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return Some((i + 1, &data[start_ix..i]));
            },
            ::core::option::Option::None => {},
        }

        i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
    }

    None
}




#[must_use]
pub(crate) fn scan_link_dest(
    data: &str,
    start_ix: usize,
    max_next: usize,
) -> Option<(usize, &str)> {
    let bytes = &data.as_bytes()[start_ix..];
    let mut i = scan_ch(bytes, b'<');

    match i {
        0 => {

            let mut nest = 0;

            while i < bytes.len() {
                match bytes[i] {
                    0x0..=0x20 => {
                        break;
                    }
                    b'(' => {
                        match {
                            let _explicit_decision = ::core::primitive::bool::then_some(nest > max_next, ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                return None;
                            },
                            ::core::option::Option::None => {},
                        }

                        nest += 1;
                    }
                    b')' => {
                        match nest {
                            0 => {
                                break;
                            },
                            _ => {},
                        }

                        nest -= 1;
                    }
                    b'\\' if match <[_]>::get(&*bytes, i + 1) { ::core::option::Option::Some(&checked_element) => is_ascii_punctuation(checked_element), ::core::option::Option::None => false } => {
                        i += 1;
                    }
                    _ => {}
                }

                i += 1;
            }

            match nest {
                0 => {},
                _ => {
                    return None;
                },
            }

            Some((i, &data[start_ix..(start_ix + i)]))
        },
        _ => {

            while i < bytes.len() {
                match bytes[i] {
                    b'\n' | b'\r' | b'<' => return None,
                    b'>' => return Some((i + 1, &data[(start_ix + 1)..(start_ix + i)])),
                    b'\\' if match <[_]>::get(&*bytes, i + 1) { ::core::option::Option::Some(&checked_element) => is_ascii_punctuation(checked_element), ::core::option::Option::None => false } => {
                        i += 1;
                    }
                    _ => {}
                }

                i += 1;
            }

            None
        },
    }
}

/// Returns bytes scanned
#[must_use]
fn scan_attribute_name(data: &[u8]) -> Option<usize> {
    let (&c, tail) = data.split_first()?;

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(is_ascii_alpha(c) || c == b'_' || c == b':', ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            Some(
                1 + scan_while(tail, |c_2| {
                    is_ascii_alphanumeric(c_2) || c_2 == b'_' || c_2 == b'.' || c_2 == b':' || c_2 == b'-'
                }),
            )
        },
        ::core::option::Option::None => {
            None
        },
    }
}

/// Returns the index immediately following the attribute on success.
/// The argument `buffer_ix` refers to the index into `data` from which we
/// should copy into `buffer` when we find bytes to skip.
fn scan_attribute(
    data: &[u8],
    mut ix: usize,
    newline_handler: Option<&dyn Fn(&[u8]) -> usize>,
    buffer: &mut Vec<u8>,
    buffer_ix: &mut usize,
) -> Option<usize> {
    ix += scan_attribute_name(&data[ix..])?;
    let ix_after_attribute = ix;
    ix = scan_whitespace_with_newline_handler_without_buffer(data, ix, newline_handler)?;

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(data.get(ix) == Some(&b'='), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            ix = scan_whitespace_with_newline_handler(
                data,
                ix_after_attribute,
                newline_handler,
                buffer,
                buffer_ix,
            )?;
            ix += 1;
            ix = scan_whitespace_with_newline_handler(data, ix, newline_handler, buffer, buffer_ix)?;
            ix = scan_attribute_value(data, ix, newline_handler, buffer, buffer_ix)?;
            Some(ix)
        },
        ::core::option::Option::None => {

            Some(ix_after_attribute)
        },
    }
}

/// Scans whitespace and possibly newlines according to the
/// behavior defined by the newline handler. When bytes are skipped,
/// all preceding non-skipped bytes are pushed to the buffer.
fn scan_whitespace_with_newline_handler(
    data: &[u8],
    mut i: usize,
    newline_handler: Option<&dyn Fn(&[u8]) -> usize>,
    buffer: &mut Vec<u8>,
    buffer_ix: &mut usize,
) -> Option<usize> {
    while i < data.len() {
        match {
            let _explicit_decision = ::core::primitive::bool::then_some(!is_ascii_whitespace(data[i]), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return Some(i);
            },
            ::core::option::Option::None => {},
        }

        match scan_eol(&data[i..]) {
            Some(eol_bytes) => {
                let handler = newline_handler?;
                i += eol_bytes;
                let skipped_bytes = handler(&data[i..]);

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(skipped_bytes > 0, ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        buffer.extend(&data[*buffer_ix..i]);
                        *buffer_ix = i + skipped_bytes;
                    },
                    ::core::option::Option::None => {},
                }

                i += skipped_bytes;
            },
            ::core::option::Option::None => {
                i += 1;
            },
        }
    }

    Some(i)
}

/// Scans whitespace and possible newlines according to the behavior defined
/// by the newline handler.
///
/// Unlike [`scan_whitespace_with_newline_handler`], this function doesn't
/// copy skipped data into a buffer. Typically, if this function
/// returns `Some`, a call to `scan_whitespace_with_newline_handler` will
/// soon follow.
#[must_use]
fn scan_whitespace_with_newline_handler_without_buffer(
    data: &[u8],
    mut i: usize,
    newline_handler: Option<&dyn Fn(&[u8]) -> usize>,
) -> Option<usize> {
    while i < data.len() {
        match {
            let _explicit_decision = ::core::primitive::bool::then_some(!is_ascii_whitespace(data[i]), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return Some(i);
            },
            ::core::option::Option::None => {},
        }

        match scan_eol(&data[i..]) {
            Some(eol_bytes) => {
                let handler = newline_handler?;
                i += eol_bytes;
                let skipped_bytes = handler(&data[i..]);
                i += skipped_bytes;
            },
            ::core::option::Option::None => {
                i += 1;
            },
        }
    }

    Some(i)
}

/// Returns the index immediately following the attribute value on success.
fn scan_attribute_value(
    data: &[u8],
    mut i: usize,
    newline_handler: Option<&dyn Fn(&[u8]) -> usize>,
    buffer: &mut Vec<u8>,
    buffer_ix: &mut usize,
) -> Option<usize> {
    let checked_value = data.get(i)?;

    match *checked_value {
        b @ b'"' | b @ b'\'' => {
            i += 1;

            while i < data.len() {
                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(data[i] == b, ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        return Some(i + 1);
                    },
                    ::core::option::Option::None => {},
                }

                match scan_eol(&data[i..]) {
                    Some(eol_bytes) => {
                        let handler = newline_handler?;
                        i += eol_bytes;
                        let skipped_bytes = handler(&data[i..]);

                        match {
                            let _explicit_decision = ::core::primitive::bool::then_some(skipped_bytes > 0, ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                buffer.extend(&data[*buffer_ix..i]);
                                *buffer_ix = i + skipped_bytes;
                            },
                            ::core::option::Option::None => {},
                        }

                        i += skipped_bytes;
                    },
                    ::core::option::Option::None => {
                        i += 1;
                    },
                }
            }

            return None;
        }
        b' ' | b'=' | b'>' | b'<' | b'`' | b'\n' | b'\r' => {
            return None;
        }
        _ => {

            i += scan_attr_value_chars(&data[i..]);
        }
    }

    Some(i)
}


#[must_use]
pub(crate) fn unescape<'a, I: Into<CowStr<'a>>>(input: I, is_in_table: bool) -> CowStr<'a> {
    let input_2 = <_ as ::core::convert::Into<crate::strings::CowStr<'_>>>::into(input);
    let mut result = String::new();
    let mut mark = 0;
    let mut i = 0;
    let bytes = input_2.as_bytes();

    while i < bytes.len() {
        match bytes[i..] {



            [b'\\', b'\\', b'|', ..] if is_in_table => {


                result.push_str(&input_2[mark..i]);
                mark = i + 2;
                i += 3;
            }
            [b'\\', cx, ..] if is_ascii_punctuation(cx) => {
                result.push_str(&input_2[mark..i]);
                mark = i + 1;
                i += 2;
            }
            [b'&', ..] => match scan_entity(&bytes[i..]) {
                (n, Some(value)) => {
                    result.push_str(&input_2[mark..i]);
                    result.push_str(&value);
                    i += n;
                    mark = i;
                }
                _ => i += 1,
            },
            [b'\r', ..] => {
                result.push_str(&input_2[mark..i]);
                i += 1;
                mark = i;
            }
            _ => i += 1,
        }
    }

    match mark {
        0 => {
            input_2
        },
        _ => {
            result.push_str(&input_2[mark..]);
            <crate::strings::CowStr<'_> as ::core::convert::From<::std::string::String>>::from(result)
        },
    }
}

/// Assumes `data` is preceded by `<`.
#[must_use]
pub(crate) fn starts_html_block_type_6(data: &[u8]) -> bool {
    let i = scan_ch(data, b'/');
    let tail = &data[i..];
    let n = scan_while(tail, is_ascii_alphanumeric);

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(!is_html_tag(&tail[..n]), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return false;
        },
        ::core::option::Option::None => {},
    }



    let tail_2 = &tail[n..];
    (match <[_]>::get(&*tail_2, 0) { ::core::option::Option::Some(&checked_element) => checked_element == b' ' || checked_element == b'\t' || checked_element == b'\r' || checked_element == b'\n' || checked_element == b'>', ::core::option::Option::None => true })
        || tail_2.len() >= 2 && &tail_2[..2] == b"/>"
}

#[must_use]
fn is_html_tag(tag: &[u8]) -> bool {
    match &(HTML_TAGS
        .binary_search_by(|probe| {
            let probe_bytes_iter = probe.as_bytes().iter();
            let tag_bytes_iter = tag.iter();

            probe_bytes_iter
                .zip(tag_bytes_iter)
                .find_map(|(&a, &b)| {


                    match a.cmp(&(b | 0x20)) {
                        std::cmp::Ordering::Equal => None,
                        inequality => Some(inequality),
                    }
                })
                .unwrap_or_else(|| probe.len().cmp(&tag.len()))
        })) { ::core::result::Result::Ok(_) => true, ::core::result::Result::Err(_insertion_position) => false }
}

/// Assumes that `data` starts with `<`.
/// Returns the index into data directly after the html tag on success.
#[must_use]
pub(crate) fn scan_html_type_7(data: &[u8]) -> Option<usize> {


    let (_span, i) = scan_html_block_inner(data, None)?;
    scan_blank_line(&data[i..])?;
    Some(i)
}

/// Assumes that `data` starts with `<`.
/// Returns the number of bytes scanned and the html in case of
/// success.
/// When some bytes were skipped, because the html was split over
/// multiple leafs (e.g. over multiple lines in a blockquote),
/// the html is returned as a vector of bytes.
/// If no bytes were skipped, the buffer will be empty.
#[must_use]
pub(crate) fn scan_html_block_inner(
    data: &[u8],
    newline_handler: Option<&dyn Fn(&[u8]) -> usize>,
) -> Option<(Vec<u8>, usize)> {
    let mut buffer = Vec::new();
    let mut last_buf_index = 0;

    let close_tag_bytes = scan_ch(&data[1..], b'/');
    let l = scan_while(&data[(1 + close_tag_bytes)..], is_ascii_alpha);

    match l {
        0 => {
            return None;
        },
        _ => {},
    }

    let mut i = 1 + close_tag_bytes + l;
    i += scan_while(&data[i..], is_ascii_letterdigitdash);

    match close_tag_bytes {
        0 => {
            loop {
                let old_i = i;

                'inner_loop_2: loop {
                    i += scan_whitespace_no_nl(&data[i..]);

                    match scan_eol(&data[i..]) {
                        Some(eol_bytes) => {
                            match eol_bytes {
                                0 => {
                                    return None;
                                },
                                _ => {},
                            }

                            let handler = newline_handler?;
                            i += eol_bytes;
                            let skipped_bytes = handler(&data[i..]);

                            let data_len = data.len() - i;

                            debug_assert!(
                                skipped_bytes <= data_len,
                                "Handler tried to skip too many bytes, fed {}, skipped {}",
                                data_len,
                                skipped_bytes
                            );

                            match {
                                let _explicit_decision = ::core::primitive::bool::then_some(skipped_bytes > 0, ());
                                _explicit_decision
                            } {
                                ::core::option::Option::Some(()) => {
                                    buffer.extend(&data[last_buf_index..i]);
                                    i += skipped_bytes;
                                    last_buf_index = i;
                                },
                                ::core::option::Option::None => {},
                            }
                        },
                        ::core::option::Option::None => {
                            break 'inner_loop_2;
                        },
                    }
                }

                if let Some(b'/') | Some(b'>') = data.get(i) {
                    break;
                }

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(old_i == i, ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {

                        return None;
                    },
                    ::core::option::Option::None => {},
                }

                i = scan_attribute(data, i, newline_handler, &mut buffer, &mut last_buf_index)?;
            }
        },
        _ => {},
    }

    i += scan_whitespace_no_nl(&data[i..]);

    match close_tag_bytes {
        0 => {
            i += scan_ch(&data[i..], b'/');
        },
        _ => {},
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(data.get(i) != Some(&b'>'), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            None
        },
        ::core::option::Option::None => {
            i += 1;

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(!buffer.is_empty(), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    buffer.extend(&data[last_buf_index..i]);
                },
                ::core::option::Option::None => {},
            }

            Some((buffer, i))
        },
    }
}

/// Returns (next_byte_offset, uri, type)
#[must_use]
pub(crate) fn scan_autolink(text: &str, start_ix: usize) -> Option<(usize, CowStr<'_>, LinkType)> {
    scan_uri(text, start_ix)
        .map(|(bytes, uri)| (bytes, uri, LinkType::Autolink))
        .or_else(|| scan_email(text, start_ix).map(|(bytes, uri)| (bytes, uri, LinkType::Email)))
}

/// Returns (next_byte_offset, uri)
#[must_use]
fn scan_uri(text: &str, start_ix: usize) -> Option<(usize, CowStr<'_>)> {
    let bytes = &text.as_bytes()[start_ix..];


    match {
        let _explicit_decision = ::core::primitive::bool::then_some(match <[_]>::get(&*bytes, 0) { ::core::option::Option::Some(&checked_element) => !is_ascii_alpha(checked_element), ::core::option::Option::None => true }, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return None;
        },
        ::core::option::Option::None => {},
    }

    let mut i = 1;

    while i < bytes.len() {
        let c = bytes[i];
        i += 1;

        match c {
            c_2 if is_ascii_alphanumeric(c_2) => (),
            b'.' | b'-' | b'+' => (),
            b':' => break,
            _ => return None,
        }
    }



    match {
        let _explicit_decision = ::core::primitive::bool::then_some(!(3..=33).contains(&i), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return None;
        },
        ::core::option::Option::None => {},
    }

    for _ in (::core::ops::Range { start: i, end: bytes.len() }) {
        match bytes[i] {
            b'>' => return Some((start_ix + i + 1, text[start_ix..(start_ix + i)].into())),
            b'\0'..=b' ' | b'<' => return None,
            _ => (),
        }

        i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
    }

    None
}

/// Returns (next_byte_offset, email)
#[must_use]
fn scan_email(text: &str, start_ix: usize) -> Option<(usize, CowStr<'_>)> {

    let bytes = &text.as_bytes()[start_ix..];
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i];
        i += 1;

        match c {
            c_2 if is_ascii_alphanumeric(c_2) => (),
            b'.' | b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'/' | b'=' | b'?'
            | b'^' | b'_' | b'`' | b'{' | b'|' | b'}' | b'~' | b'-' => (),
            b'@' if i > 1 => break,
            _ => return None,
        }
    }

    loop {
        let label_start_ix = i;
        let mut fresh_label = true;

        'inner_for_2: for _ in (::core::ops::Range { start: i, end: bytes.len() }) {
            match bytes[i] {
                c if is_ascii_alphanumeric(c) => (),
                b'-' if fresh_label => {
                    return None;
                }
                b'-' => (),
                _ => break 'inner_for_2,
            }

            fresh_label = false;
            i = (::core::num::Wrapping(i) + ::core::num::Wrapping(1)).0;
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(i == label_start_ix || i - label_start_ix > 63 || bytes[i - 1] == b'-', ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return None;
            },
            ::core::option::Option::None => {},
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(bytes.get(i) != Some(&b'.'), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                break;
            },
            ::core::option::Option::None => {},
        }

        i += 1;
    }

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(bytes.get(i) != Some(&b'>'), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return None;
        },
        ::core::option::Option::None => {},
    }

    Some((start_ix + i + 1, text[start_ix..(start_ix + i)].into()))
}

/// Scan comment, declaration, or CDATA section, with initial "<!" already consumed.
/// Returns byte offset on match.
pub(crate) fn scan_inline_html_comment(
    bytes: &[u8],
    mut ix: usize,
    scan_guard: &mut HtmlScanGuard,
) -> Option<usize> {
    let c = ::core::option::Option::<&_>::copied(bytes.get(ix))?;
    ix += 1;

    match c {


        b'-' if ix > scan_guard.comment => {

            match {
                let checked_value = bytes.get(ix)?;
                let _explicit_decision = ::core::primitive::bool::then_some(*checked_value != b'-', ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    return None;
                },
                ::core::option::Option::None => {},
            }








            ix -= 1;

            while let Some(x) = memchr(b'-', &bytes[ix..]) {
                ix += x + 1;
                scan_guard.comment = ix;

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(bytes.get(ix) == Some(&b'-') && bytes.get(ix + 1) == Some(&b'>'), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        return Some(ix + 2);
                    },
                    ::core::option::Option::None => {},
                }
            }

            None
        }


        b'[' if bytes[ix..].starts_with(b"CDATA[") && ix > scan_guard.cdata => {
            ix += b"CDATA[".len();
            ix = memchr(b']', &bytes[ix..]).map_or(bytes.len(), |x| ix + x);
            let close_brackets = scan_ch_repeat(&bytes[ix..], b']');
            ix += close_brackets;

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(close_brackets == 0 || bytes.get(ix) != Some(&b'>'), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    scan_guard.cdata = ix;
                    None
                },
                ::core::option::Option::None => {
                    Some(ix + 1)
                },
            }
        }


        _ if c.is_ascii_alphabetic() && ix > scan_guard.declaration => {
            ix = memchr(b'>', &bytes[ix..]).map_or(bytes.len(), |x| ix + x);

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(bytes.get(ix) != Some(&b'>'), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    scan_guard.declaration = ix;
                    None
                },
                ::core::option::Option::None => {
                    Some(ix + 1)
                },
            }
        }
        _ => None,
    }
}

/// Scan processing directive, with initial "<?" already consumed.
/// Returns the next byte offset on success.
pub(crate) fn scan_inline_html_processing(
    bytes: &[u8],
    mut ix: usize,
    scan_guard: &mut HtmlScanGuard,
) -> Option<usize> {
    match {
        let _explicit_decision = ::core::primitive::bool::then_some(ix <= scan_guard.processing, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return None;
        },
        ::core::option::Option::None => {},
    }

    while let Some(offset) = memchr(b'?', &bytes[ix..]) {
        ix += offset + 1;

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(bytes.get(ix) == Some(&b'>'), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return Some(ix + 1);
            },
            ::core::option::Option::None => {},
        }
    }

    scan_guard.processing = ix;
    None
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn overflow_list() {
        assert!(
            scan_listitem(b"4444444444444444444444444444444444444444444444444444444444!").is_none()
        );
    }

    #[test]
    fn overflow_by_addition() {
        assert!(scan_listitem(b"1844674407370955161615!").is_none());
    }

    #[test]
    fn good_emails() {
        const EMAILS: &[&str] = &[
            "<a@b.c>",
            "<a@b>",
            "<a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-@example.com>",
            "<a@sixty-three-letters-in-this-identifier-----------------------63>",
        ];
        for email in EMAILS {
            assert!(scan_email(email, 1).is_some());
        }
    }

    #[test]
    fn bad_emails() {
        const EMAILS: &[&str] = &[
            "<@b.c>",
            "<foo@-example.com>",
            "<foo@example-.com>",
            "<a@notrailingperiod.>",
            "<a(noparens)@example.com>",
            "<\"noquotes\"@example.com>",
            "<a@sixty-four-letters-in-this-identifier-------------------------64>",
        ];
        for email in EMAILS {
            assert!(scan_email(email, 1).is_none());
        }
    }
}
