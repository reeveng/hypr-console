//! A tag as the rest of the desktop reads it: which format it was, the items
//! it says in order, and its pictures.
//!
//! Every format's own tag is turned into one of these as it is read, in the
//! order the format kept its fields, and a field that holds several values is
//! as many items. So two items can share a key, and reading one back means
//! choosing which, which is the caller's to do rather than this crate's.

use console_core_never::Never;

use crate::keys::ItemKey;
use crate::picture::Picture;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TagType {
  Ape,
  Id3v1,
  Id3v2,
  Mp4Ilst,
  VorbisComments,
  RiffInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TagItem {
  pub key: ItemKey,
  pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
  pub tag_type: TagType,
  pub items: Vec<TagItem>,
  pub pictures: Vec<Picture>,
}

impl Tag {
  pub fn new(tag_type: TagType) -> Result<Tag, Never> {
    Ok(Tag {
      tag_type,
      items: Vec::new(),
      pictures: Vec::new(),
    })
  }

  pub fn insert_text(&mut self, key: ItemKey, text: String) -> Result<(), Never> {
    self.items.retain(|item| item.key != key);
    self.items.push(TagItem { key, text });

    Ok(())
  }

  pub(crate) fn push_text(&mut self, key: ItemKey, text: String) -> Result<(), Never> {
    self.items.push(TagItem { key, text });

    Ok(())
  }
}
