// edition:2024

fn main() {
    let value = Some(1);
    match value {
        #[cfg(all())]
        Some(number) => assert_eq!(number, 1),
        _ => (),
    }
}

enum Conditional {
    Visible,
    Other,
    #[cfg(any())]
    Alternate,
}

fn covered(value: Conditional) -> u8 {
    match value {
        Conditional::Visible => 1,
        _ => 0,
    }
}

fn collectively_covered(value: Result<Option<bool>, ()>) -> usize {
    match value {
        Ok(Some(true)) => 1,
        Ok(Some(false)) => 2,
        Ok(None) => 3,
        _ => 0,
    }
}

#[cfg(target_pointer_width = "64")]
use core::cmp::Ordering::Less as First;
#[cfg(not(target_pointer_width = "64"))]
use core::cmp::Ordering::Greater as First;

fn imported_directly(value: core::cmp::Ordering) -> usize {
    match value {
        First => 1,
        _ => 0,
    }
}

fn imported_payload(value: Option<core::cmp::Ordering>) -> usize {
    match value {
        ::core::option::Option::Some(First) => 1,
        _ => 0,
    }
}

#[cfg(alternate_core)]
extern crate self as core;

#[cfg(alternate_core)]
pub use std::option;

#[cfg(alternate_core)]
pub mod cmp {
    pub enum Ordering {
        Less,
        Equal,
        Greater,
        Extra,
    }
}

fn absolute_from_core(value: ::core::cmp::Ordering) -> usize {
    match value {
        ::core::cmp::Ordering::Less => 1,
        _ => 0,
    }
}
