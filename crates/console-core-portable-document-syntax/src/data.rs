use crate::object::ObjectIdentifier;
use crate::object::Stream;
use crate::sync::HashMap;
use crate::sync::{Arc, Mutex, MutexExt};
use crate::util::SegmentList;
use alloc::borrow::Cow;
use alloc::vec::Vec;
use console_core_never::Never;
use console_core_number_conversion::fixed_index;
use core::fmt::{Debug, Formatter};

#[derive(Clone)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The public PDF byte owner is cloned by readers of the same immutable input"
  )
)]
pub struct PdfData {
  #[cfg(feature = "std")]
  inner: Arc<dyn AsRef<[u8]> + Send + Sync>,
  #[cfg(not(feature = "std"))]
  inner: Arc<dyn AsRef<[u8]>>,
}

impl Debug for PdfData {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "PdfData {{ ... }}")
  }
}

impl AsRef<[u8]> for PdfData {
  fn as_ref(&self) -> &[u8] {
    (*self.inner).as_ref()
  }
}

#[cfg(feature = "std")]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The public conversion retains the caller's immutable shared PDF bytes"
  )
)]
impl<T: AsRef<[u8]> + Send + Sync + 'static> From<Arc<T>> for PdfData {
  fn from(data: Arc<T>) -> Self {
    Self { inner: data }
  }
}

#[cfg(not(feature = "std"))]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The public conversion retains the caller's immutable single-threaded PDF bytes"
  )
)]
impl<T: AsRef<[u8]> + 'static> From<Arc<T>> for PdfData {
  fn from(data: Arc<T>) -> Self {
    Self { inner: data }
  }
}

impl From<Vec<u8>> for PdfData {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Multiple document objects borrow slices from this immutable byte owner"
    )
  )]
  fn from(data: Vec<u8>) -> Self {
    Self {
      inner: Arc::new(data),
    }
  }
}

pub(crate) struct Data {
  data: PdfData,

  decoded: SegmentList<Option<Vec<u8>>, 32>,
  map: Mutex<HashMap<ObjectIdentifier, u64>>,
}

impl Debug for Data {
  fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
    write!(f, "Data {{ ... }}")
  }
}

impl Data {
  pub(crate) fn new(data: PdfData) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(decoded) = SegmentList::new();

    Ok(Self {
      data,
      decoded,
      map: Mutex::new(HashMap::new()),
    })
  }

  pub(crate) fn get(&self) -> core::result::Result<&PdfData, console_core_never::Never> {
    Ok(&self.data)
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "The cache marks a stream's slot before decoding it and indirect references are depth-limited"
    )
  )]
  pub(crate) fn decoded_stream(
    &self,
    id: ObjectIdentifier,
    stream: &Stream<'_>,
  ) -> Result<Option<&[u8]>, Never> {
    let slot = {
      let Ok(acquired) = self.map.get();
      let mut map = match acquired {
        Some(map) => map,
        None => return Ok(None),
      };

      match map.get(&id).copied() {
        Some(index) => StreamSlot::Existing(index),
        None => {
          let Ok(index) = fixed_index(map.len());
          map.insert(id, index);
          StreamSlot::New(index)
        }
      }
    };

    match slot {
      StreamSlot::Existing(index) => {
        let Ok(decoded) = self.decoded.get(index);
        Ok(decoded.and_then(Option::as_deref))
      }
      StreamSlot::New(index) => {
        let Ok(decoded) = self.decoded.get_or_init(index, || match stream.decoded() {
          Ok(bytes) => Some(Cow::into_owned(bytes)),
          Err(_invalid_stream_encoding) => None,
        });
        Ok(decoded.and_then(Option::as_deref))
      }
    }
  }
}

enum StreamSlot {
  Existing(u64),
  New(u64),
}
