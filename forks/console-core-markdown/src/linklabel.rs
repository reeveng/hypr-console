// Copyright 2018 Google LLC
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

//! Link label parsing and matching.

use unicase::UniCase;

use crate::scanners::{is_ascii_punctuation, is_ascii_whitespace, scan_eol};
use crate::strings::CowStr;

#[derive(Debug)]
pub(crate) enum ReferenceLabel<'a> {
    Link(CowStr<'a>),
    Footnote(CowStr<'a>),
}

pub(crate) type LinkLabel<'a> = UniCase<CowStr<'a>>;

pub(crate) type FootnoteLabel<'a> = UniCase<CowStr<'a>>;

/// Assumes the opening bracket has already been scanned.
/// The line break handler determines what happens when a linebreak
/// is found. It is passed the bytes following the line break and
/// either returns `Some(k)`, where `k` is the number of bytes to skip,
/// or `None` to abort parsing the label.
/// Returns the number of bytes read (including closing bracket) and label on success.
#[must_use]
pub(crate) fn scan_link_label_rest<'t>(
    text: &'t str,
    linebreak_handler: &dyn Fn(&[u8]) -> Option<usize>,
    is_in_table: bool,
) -> Option<(usize, CowStr<'t>)> {
    let bytes = text.as_bytes();
    let mut ix = 0;
    let mut only_white_space = true;
    let mut codepoints = 0;

    let mut label = String::new();
    let mut mark = 0;

    loop {
        match {
            let _explicit_decision = ::core::primitive::bool::then_some(codepoints >= 1000, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                return None;
            },
            ::core::option::Option::None => {},
        }

        let checked_value = bytes.get(ix)?;

        match *checked_value {
            b'[' => return None,
            b']' => break,




            b'|' if is_in_table && ix != 0 && bytes.get(ix - 1) == Some(&b'\\') => {

                label.push_str(&text[mark..ix - 1]);
                label.push('|');
                ix += 1;
                only_white_space = false;
                mark = ix;
            }
            b'\\' if is_in_table && bytes.get(ix + 1) == Some(&b'|') => {

                label.push_str(&text[mark..ix]);
                label.push('|');
                ix += 2;
                codepoints += 1;
                only_white_space = false;
                mark = ix;
            }
            b'\\' if is_ascii_punctuation(*{ let checked_value_2 = bytes.get(ix + 1)?; checked_value_2 }) => {
                ix += 2;
                codepoints += 2;
                only_white_space = false;
            }
            b if is_ascii_whitespace(b) => {

                let mut whitespaces = 0;
                let mut linebreaks = 0;
                let whitespace_start = ix;

                while match <[_]>::get(&*bytes, ix) { ::core::option::Option::Some(&checked_element) => is_ascii_whitespace(checked_element), ::core::option::Option::None => false } {
                    match scan_eol(&bytes[ix..]) {
                        Some(eol_bytes) => {
                            linebreaks += 1;

                            match {
                                let _explicit_decision = ::core::primitive::bool::then_some(linebreaks > 1, ());
                                _explicit_decision
                            } {
                                ::core::option::Option::Some(()) => {
                                    return None;
                                },
                                ::core::option::Option::None => {},
                            }

                            ix += eol_bytes;
                            ix += linebreak_handler(&bytes[ix..])?;
                            whitespaces += 2;
                        },
                        ::core::option::Option::None => {
                            whitespaces += match {
                                let _explicit_decision = ::core::primitive::bool::then_some(bytes[ix] == b' ', ());
                                _explicit_decision
                            } {
                                ::core::option::Option::Some(()) => { 1 },
                                ::core::option::Option::None => { 2 },
                            };
                            ix += 1;
                        },
                    }
                }

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(whitespaces > 1, ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        label.push_str(&text[mark..whitespace_start]);
                        label.push(' ');
                        mark = ix;
                        codepoints += ix - whitespace_start;
                    },
                    ::core::option::Option::None => {
                        codepoints += 1;
                    },
                }
            }
            b => {
                only_white_space = false;
                ix += 1;

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(b & 0b1000_0000 != 0, ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        codepoints += 1;
                    },
                    ::core::option::Option::None => {},
                }
            }
        }
    }

    match ::core::primitive::bool::then_some(only_white_space, ()) {
        ::core::option::Option::Some(()) => {
            None
        },
        ::core::option::Option::None => {
            let cow = match mark {
                0 => {
                    let asciiws: &[_] = &[' ', '\r', '\n', '\t'];
                    <crate::strings::CowStr<'_> as ::core::convert::From<&::core::primitive::str>>::from(text[..ix].trim_matches(asciiws))
                },
                _ => {
                    label.push_str(&text[mark..ix]);

                    while matches!(
                        label.as_bytes().last(),
                        Some(&b' ' | &b'\r' | &b'\n' | &b'\t')
                    ) {
                        label.pop();
                    }

                    while matches!(
                        label.as_bytes().first(),
                        Some(&b' ' | &b'\r' | &b'\n' | &b'\t')
                    ) {
                        label.remove(0);
                    }

                    <crate::strings::CowStr<'_> as ::core::convert::From<::std::string::String>>::from(label)
                },
            };
            Some((ix + 1, cow))
        },
    }
}

#[cfg(test)]
mod test {
    use super::scan_link_label_rest;

    #[test]
    fn whitespace_normalization() {
        let input = "«\t\tBlurry Eyes\t\t»][blurry_eyes]";
        let expected_output = "« Blurry Eyes »";

        let (_bytes, normalized_label) = scan_link_label_rest(input, &|_| None, false).unwrap();
        assert_eq!(expected_output, normalized_label.as_ref());
    }

    #[test]
    fn return_carriage_linefeed_ok() {
        let input = "hello\r\nworld\r\n]";
        assert!(scan_link_label_rest(input, &|_| Some(0), false).is_some());
    }
}
