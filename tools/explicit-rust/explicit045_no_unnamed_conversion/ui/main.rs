// run-rustfix
// edition:2024

// UI test for EXPLICIT045 — a conversion names the type it becomes.

use std::path::PathBuf;

// BAD EXPLICIT045 — the line says neither end of this.
fn owns(text: &str) -> String {
    //~v EXPLICIT045_NO_UNNAMED_CONVERSION
    text.into()
}

// BAD EXPLICIT045 — and the destination here is a type with a shape.
fn a_path(text: &str) -> PathBuf {
    //~v EXPLICIT045_NO_UNNAMED_CONVERSION
    text.into()
}

mod r#type {
    pub struct View<'a>(pub &'a [u8]);

    impl<'a> From<&'a [u8]> for View<'a> {
        fn from(bytes: &'a [u8]) -> Self {
            Self(bytes)
        }
    }
}

fn held(bytes: &[u8], use_first: bool) -> r#type::View<'_> {
    match use_first {
        true => bytes.into(),
        //~^ EXPLICIT045_NO_UNNAMED_CONVERSION
        false => (&bytes[1..]).into(),
        //~^ EXPLICIT045_NO_UNNAMED_CONVERSION
    }
}

struct Pixels([u16; 3]);

impl From<[u16; 3]> for Pixels {
    fn from(values: [u16; 3]) -> Self {
        Self(values)
    }
}

fn pixels(values: [u16; 3]) -> Pixels {
    values.into()
}

fn boxed(values: Vec<u16>) -> Box<[u16]> {
    values.into()
}

fn named_bound(value: impl Into<String>) -> String {
    value.into()
}

// GOOD — the destination is on the line and moves when the type does.
fn says_it(text: &str) -> String {
    String::from(text)
}

// GOOD — two numbers are EXPLICIT010's, which can name both widths.
fn widens(small: u8) -> u32 {
    small.into()
}

// GOOD — nothing is converted.
fn the_same(text: String) -> String {
    text
}

fn main() {
    assert_eq!(held(&[1, 2], true).0, &[1, 2]);
    assert_eq!(held(&[1, 2], false).0, &[2]);
    assert_eq!(owns("text"), says_it("text"));
    assert_eq!(the_same(owns("text")), "text");
    assert_eq!(a_path("file"), PathBuf::from("file"));
    assert_eq!(widens(4), 4);
    assert_eq!(pixels([1, 2, 3]).0, [1, 2, 3]);
    assert_eq!(&*boxed(vec![1, 2]), &[1, 2]);
    assert_eq!(named_bound("text"), "text");
}
