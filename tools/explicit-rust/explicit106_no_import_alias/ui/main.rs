#![expect(unused_imports, reason = "the fixture checks import declarations")]

extern crate alloc as memory;
extern crate core;

use std::fmt::Debug as RenderDebug;
use std::fmt::{Display, Write as _};
use std::io::{
  self, Error as ReadError,
  prelude::{self as prelude, BufRead as Buffered},
};
#[rustfmt::skip]
use std::path::Path as Path;

pub use std::time::Duration as Elapsed;

mod without_exception {
  pub struct Error;
  use std::fmt::Error as FormattingError;
}

mod with_exception {
  pub struct Error;

  #[cfg_attr(
    dylint_lib = "explicit106_no_import_alias",
    allow(
      explicit106_no_import_alias,
      reason = "this module uses both its own Error and std::fmt::Error"
    )
  )]
  use std::fmt::Error as FormattingError;

  pub fn errors() -> (Error, FormattingError) {
    (Error, FormattingError)
  }
}

mod with_anonymous_exception {
  use std::io::Write;

  #[cfg_attr(
    dylint_lib = "explicit106_no_import_alias",
    allow(
      explicit106_no_import_alias,
      reason = "this module uses both std::io::Write and std::fmt::Write"
    )
  )]
  use std::fmt::Write as _;

  pub fn write_both() {
    let mut bytes = Vec::new();
    let mut text = String::new();
    let _ = bytes.write_all(b"fixture");
    let _ = text.write_str("fixture");
  }
}

mod qualified_paths {
  use std::{collections, net::*};

  pub fn errors() -> (std::fmt::Error, std::io::Error) {
    (std::fmt::Error, std::io::Error::other("fixture"))
  }
}

macro_rules! generated_import {
  () => {
    use std::sync::Arc as GeneratedArc;
  };
}

generated_import!();

fn main() {
  use std::cell::{self as cells};
  let _ = without_exception::Error;
  let _ = qualified_paths::errors();
  let _ = with_exception::errors();
  with_anonymous_exception::write_both();
  let _ = 1_u8 as u16;
}
