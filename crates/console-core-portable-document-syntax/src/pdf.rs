//! The starting point for reading PDF files.

use crate::PdfData;
use crate::crypto::Password;
use crate::object::Object;
use crate::page::Pages;
use crate::page::cached::CachedPages;
use crate::util::find_needle;
use crate::xref::{XRef, XRefError, fallback, root_xref};
use console_core_never::Never;
use console_core_number_conversion::index;

pub use crate::crypto::DecryptionError;
use crate::metadata::Metadata;

pub struct Pdf {
  header_version: PdfVersion,
  pages: CachedPages,
  data: PdfData,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LoadPdfError {
  Decryption(DecryptionError),

  Invalid,
}

#[allow(
  clippy::len_without_is_empty,
  reason = "a document is asked how many objects it holds, never whether it holds none"
)]
impl Pdf {
  pub fn new(data: impl Into<PdfData>) -> Result<Self, LoadPdfError> {
    Self::new_with_password(data, "")
  }

  #[cfg_attr(
    dylint_lib = "explicit094_no_plain_secret",
    expect(
      explicit094_no_plain_secret,
      reason = "the existing public API borrows the caller-owned password and immediately hands it to the typed decryption path"
    )
  )]
  pub fn new_with_password(
    data: impl Into<PdfData>,
    password: &str,
  ) -> Result<Self, LoadPdfError> {
    let data_2 = <_ as ::core::convert::Into<crate::data::PdfData>>::into(data);
    let password_2 = Password(password.as_bytes());
    let Ok(header_version) = find_version(data_2.as_ref());
    let version = match header_version {
      Some(version) => version,
      None => PdfVersion::Pdf10,
    };
    let xref = match root_xref(data_2.clone(), password_2) {
      Ok(x) => x,
      Err(e) => match e {
        XRefError::Unknown => ({
          let Ok(explicit_value) = fallback(data_2.clone(), password_2);
          explicit_value
        })
        .ok_or(LoadPdfError::Invalid)?,
        XRefError::Encryption(e_2) => return Err(LoadPdfError::Decryption(e_2)),
      },
    };
    let Ok(cached_pages) = CachedPages::new(xref);
    let pages = cached_pages.ok_or(LoadPdfError::Invalid)?;

    Ok(Self {
      header_version: version,
      pages,
      data: data_2,
    })
  }

  pub fn len(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    let Ok(xref) = self.xref();
    let Ok(length) = xref.len();
    console_core_number_conversion::index(length)
  }

  pub fn objects(
    &self,
  ) -> core::result::Result<impl IntoIterator<Item = Object<'_>>, console_core_never::Never> {
    let Ok(xref) = self.xref();
    let Ok(objects) = xref.objects();

    Ok(objects)
  }

  pub fn version(&self) -> core::result::Result<PdfVersion, console_core_never::Never> {
    let Ok(xref) = self.xref();
    let Ok(trailer_data) = xref.trailer_data();
    Ok(match trailer_data.version {
      Some(catalog_version) => catalog_version,
      None => self.header_version,
    })
  }

  pub fn data(&self) -> core::result::Result<&PdfData, console_core_never::Never> {
    Ok(&self.data)
  }

  pub fn pages(&self) -> core::result::Result<&Pages<'_>, console_core_never::Never> {
    let Ok(page) = self.pages.get();

    Ok(page)
  }

  pub fn xref(&self) -> core::result::Result<&XRef, console_core_never::Never> {
    self.pages.xref()
  }

  pub fn metadata(&self) -> core::result::Result<&Metadata, console_core_never::Never> {
    let Ok(xref) = self.xref();
    let Ok(metadata) = xref.metadata();

    Ok(metadata)
  }
}

fn find_version(data: &[u8]) -> Result<Option<PdfVersion>, Never> {
  const HEADER: &[u8; 5] = b"%PDF-";
  let prefix = match data.get(..data.len().min(2000)) {
    Some(bytes) => bytes,
    None => return Ok(None),
  };
  let Ok(offset) = find_needle(prefix, HEADER);
  let found = match offset {
    Some(found) => found,
    None => return Ok(None),
  };
  let Ok(position) = index(found.wrapping_add(5));

  match prefix.get(position..) {
    Some(after_header) => PdfVersion::from_bytes(after_header),
    None => Ok(None),
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PdfVersion {
  Pdf10,

  Pdf11,

  Pdf12,

  Pdf13,

  Pdf14,

  Pdf15,

  Pdf16,

  Pdf17,

  Pdf20,
}

impl PdfVersion {
  pub(crate) fn from_bytes(
    bytes: &[u8],
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(
      match match bytes.get(..3) {
        Some(value) => value,
        None => return Ok(None),
      } {
        b"1.0" => Some(Self::Pdf10),
        b"1.1" => Some(Self::Pdf11),
        b"1.2" => Some(Self::Pdf12),
        b"1.3" => Some(Self::Pdf13),
        b"1.4" => Some(Self::Pdf14),
        b"1.5" => Some(Self::Pdf15),
        b"1.6" => Some(Self::Pdf16),
        b"1.7" => Some(Self::Pdf17),
        b"2.0" => Some(Self::Pdf20),
        _ => None,
      },
    )
  }
}

#[cfg(test)]
mod tests {
  use crate::pdf::{LoadPdfError, Pdf, PdfVersion};

  #[test]
  fn issue_49() {
    assert!(matches!(
      Pdf::new(alloc::vec::Vec::new()),
      Err(LoadPdfError::Invalid)
    ));
  }

  #[test]
  fn pdf_version_header() -> Result<(), LoadPdfError> {
    let data = include_bytes!("../tests/fixtures/version-header.pdf").to_vec();
    let pdf = Pdf::new(data)?;

    let Ok(version) = pdf.version();
    assert_eq!(version, PdfVersion::Pdf17);

    Ok(())
  }

  #[test]
  fn pdf_version_catalog() -> Result<(), LoadPdfError> {
    let data = include_bytes!("../tests/fixtures/version-catalog.pdf").to_vec();
    let pdf = Pdf::new(data)?;

    let Ok(version) = pdf.version();
    assert_eq!(version, PdfVersion::Pdf14);

    Ok(())
  }
}
