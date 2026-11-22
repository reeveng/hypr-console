//! Reading document metadata.

use crate::object::DateTime;
use alloc::vec::Vec;

#[derive(Clone, Default, Debug, PartialEq, Eq)]

pub struct Metadata {
  pub creation_date: Option<DateTime>,

  pub modification_date: Option<DateTime>,

  pub title: Option<Vec<u8>>,

  pub author: Option<Vec<u8>>,

  pub subject: Option<Vec<u8>>,

  pub keywords: Option<Vec<u8>>,

  pub creator: Option<Vec<u8>>,

  pub producer: Option<Vec<u8>>,
}
