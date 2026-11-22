//! The former paged serde shape remains readable when pages are keyed by value.
//!
//! The shape names a page by its number, which is its first value over the page
//! width; reading one back multiplies it out again, and a number whose first
//! value is past the end of the integer domain is a fault rather than a wrap.
//! Each list is bounded by the number of pages in the integer domain.

use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;

use console_core_never::Never;
use console_core_number_conversion::index;

use super::page::{Page as Bitmap, WIDTH};
use super::{PageStart, U32Set};
use crate::Emptiness;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
  storage: [u64; 8],
  length: u32,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PageInfo {
  index: u32,
  major_value: u32,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PagedSet {
  pages: PageList<Page>,
  page_map: PageList<PageInfo>,
  length: u64,
}

const MAX_PAGES: u32 = (u32::MAX / WIDTH.get()).saturating_add(1);

#[derive(serde::Serialize)]
#[serde(transparent)]
struct PageList<T>(Vec<T>);

impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for PageList<T> {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    deserializer.deserialize_seq(PageListVisitor::<T, MAX_PAGES>(PhantomData))
  }
}

struct PageListVisitor<T, const LIMIT: u32>(PhantomData<T>);

impl<'de, T: serde::Deserialize<'de>, const LIMIT: u32> serde::de::Visitor<'de>
  for PageListVisitor<T, LIMIT>
{
  type Value = PageList<T>;

  fn expecting(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(to, "a sequence of at most {LIMIT} page entries")
  }

  fn visit_seq<A: serde::de::SeqAccess<'de>>(
    self,
    mut sequence: A,
  ) -> Result<Self::Value, A::Error> {
    let mut entries = Vec::new();

    for _ in 0..LIMIT {
      let next = sequence.next_element()?;

      match next {
        Some(entry) => entries.push(entry),
        None => return Ok(PageList(entries)),
      }
    }

    let next = sequence.next_element::<T>()?;

    match next {
      Some(_excess) => Err(serde::de::Error::custom(
        "page list exceeds the integer domain",
      )),
      None => Ok(PageList(entries)),
    }
  }
}

impl U32Set {
  fn paged(&self) -> Result<PagedSet, Never> {
    let mut pages = Vec::new();
    let mut page_map = Vec::new();

    for (index, (start, page)) in (0u32..).zip(&self.pages) {
      let Ok(storage) = page.words();
      let Ok(length) = page.len();

      pages.push(Page { storage, length });
      page_map.push(PageInfo {
        index,
        major_value: start.0 / WIDTH,
      });
    }

    let Ok(length) = self.len();

    Ok(PagedSet {
      pages: PageList(pages),
      page_map: PageList(page_map),
      length,
    })
  }
}

impl serde::Serialize for U32Set {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    let Ok(paged) = self.paged();

    serde::Serialize::serialize(&paged, serializer)
  }
}

impl<'de> serde::Deserialize<'de> for U32Set {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let paged = <PagedSet as serde::Deserialize>::deserialize(deserializer)?;
    let mut set = Self::default();

    for page in paged.page_map.0 {
      let start = match page.major_value.checked_mul(WIDTH.get()) {
        Some(first) => PageStart(first),
        None => {
          return Err(serde::de::Error::custom(
            "page position exceeds the integer domain",
          ));
        }
      };
      let Ok(at) = index(page.index);
      let storage = paged
        .pages
        .0
        .get(at)
        .ok_or_else(|| serde::de::Error::custom("page index is out of bounds"))?;
      let Ok(bitmap) = Bitmap::from_words(storage.storage);
      let Ok(emptiness) = bitmap.is_empty();

      match emptiness {
        Emptiness::Empty => {}
        Emptiness::NonEmpty => {
          let Ok(()) = set.pages.entry(start).or_default().union(&bitmap);
        }
      }
    }

    let Ok(()) = set.recount();

    Ok(set)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde::de::Visitor;
  use serde::de::value::{Error, SeqDeserializer};

  #[test]
  fn a_page_list_refuses_entries_beyond_its_limit() -> Result<(), Error> {
    let exact = SeqDeserializer::new(0u32..2);
    let excess = SeqDeserializer::<_, Error>::new(0u32..3);
    let accepted = PageListVisitor::<u32, 2>(PhantomData).visit_seq(exact)?;
    let refused = PageListVisitor::<u32, 2>(PhantomData).visit_seq(excess);

    assert_eq!(accepted.0, [0, 1]);

    match refused {
      Ok(_accepted_excess) => Err(serde::de::Error::custom("accepted an oversized page list")),
      Err(fault) => {
        assert_eq!(fault.to_string(), "page list exceeds the integer domain");

        Ok(())
      }
    }
  }

  #[test]
  fn legacy_pages_can_be_empty_or_stored_out_of_order() -> Result<(), serde_json::Error> {
    let bytes = include_bytes!("../../tests/fixtures/legacy-pages.json");
    let decoded: U32Set = serde_json::from_slice(bytes)?;

    assert_eq!(decoded, U32Set::from_iter([5, 1027]));

    Ok(())
  }

  #[test]
  fn a_page_past_the_end_of_the_domain_is_refused() -> Result<(), crate::TestFailure> {
    let last = r#"{"pages":[{"storage":[1,0,0,0,0,0,0,0],"length":1}],"page_map":[{"index":0,"major_value":8388607}],"length":1}"#;
    let past = r#"{"pages":[{"storage":[1,0,0,0,0,0,0,0],"length":1}],"page_map":[{"index":0,"major_value":8388608}],"length":1}"#;
    let decoded: U32Set = crate::test_support::Required::required(serde_json::from_str(last))?;
    let refused: Result<U32Set, serde_json::Error> = serde_json::from_str(past);

    assert_eq!(decoded, U32Set::from_iter([u32::MAX.saturating_sub(511)]));

    match refused {
      Ok(_accepted) => Err(crate::TestFailure::UnexpectedSuccess),
      Err(_refusal) => Ok(()),
    }
  }
}
