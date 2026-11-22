use crate::xkb::Keymap;
use std::fmt::Debug;
use std::fmt::Display;

pub struct Formatter<'a> {
  pub(crate) keymap: &'a Keymap,
}

impl Debug for Formatter<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    Display::fmt(self, f)
  }
}
