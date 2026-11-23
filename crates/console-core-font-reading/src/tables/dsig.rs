//! The [DSIG](https://learn.microsoft.com/en-us/typography/opentype/spec/dsig) table

include!("../../generated/generated_dsig.rs");

impl SignatureRecord {
  pub fn signature_block<'a>(
    &self,
    data: FontData<'a>,
  ) -> Result<SignatureBlockFormat1<'a>, ReadError> {
    let Ok(read_format) = self.format();

    match read_format {
      1 => {
        let Ok(signature_block_offset) = self.signature_block_offset();
        let signature = signature_block_offset.resolve::<SignatureBlockFormat1>(data)?;

        let Ok(compute_len) = signature.compute_len();
        let actual_len = u32::try_from(compute_len).map_err(|_| ReadError::OutOfBounds)?;

        let Ok(length) = self.length();

        match ::core::primitive::bool::then_some(length == actual_len, ()) {
          ::core::option::Option::Some(()) => Ok(signature),
          ::core::option::Option::None => {
            Err(ReadError::MalformedData("ambiguous DSIG signature length"))
          }
        }
      }
      unknown => Err(ReadError::InvalidFormat(
        <::core::primitive::i64 as ::core::convert::From<::core::primitive::u32>>::from(unknown),
      )),
    }
  }
}

impl SignatureBlockFormat1<'_> {
  #[must_use]
  fn compute_len(&self) -> core::result::Result<u64, console_core_never::Never> {
    let Ok(value_1) = self.min_byte_range();

    Ok(value_1.end.saturating_sub(value_1.start))
  }
}

#[cfg(test)]
mod tests {

  use super::{Dsig, PermissionFlags};
  use crate::{FontData, FontRead, ReadError};

  #[test]
  fn test_empty() -> ::core::result::Result<(), crate::read::ReadError> {
    let buf = font_test_data::be_buffer! { (0x1u32), (0x0u16), (0x0u16) };

    let dsig = Dsig::read({
      let Ok(explicit_value) = FontData::new(&buf);
      explicit_value
    })?;
    assert_eq!(dsig.version(), Ok(1));
    let Ok(signature_records) = dsig.signature_records();
    assert_eq!(signature_records.len(), 0);
    assert_eq!(dsig.flags(), PermissionFlags::empty());
    ::core::result::Result::Ok(())
  }

  #[test]
  fn test_beef() -> ::core::result::Result<(), crate::read::ReadError> {
    let buf = font_test_data::be_buffer! { (0x1_u32), (0x1_u16), (0x1_u16), (0x1_u32), (0xC_u32), (0x14_u32), (0x0_u16), (0x0_u16), (0x4_u32), (0xDEADBEEF_u32) };

    let Ok(data) = FontData::new(&buf);

    let dsig = Dsig::read(data)?;

    assert_eq!(dsig.version(), Ok(1));
    let Ok(signature_records) = dsig.signature_records();
    assert_eq!(signature_records.len(), 1);
    assert_eq!(dsig.flags(), Ok(PermissionFlags::CANNOT_BE_RESIGNED));

    let Ok(dsig_signature_records) = dsig.signature_records();
    let record = *match <[_]>::get(&*dsig_signature_records, 0) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
      }
    };
    assert_eq!(record.format(), Ok(1));

    let block = record.signature_block(data)?;
    let signature: &[u8] = &[0xDE, 0xAD, 0xBE, 0xEF];
    assert_eq!(block.signature(), Ok(signature));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn test_unknown_format() -> ::core::result::Result<(), crate::read::ReadError> {
    let buf = font_test_data::be_buffer! { (0x1_u32), (0x1_u16), (0x1_u16), (0x2_u32), (0xC_u32), (0x14_u32), (0x0_u16), (0x0_u16), (0x4_u32), (0xDEADBEEF_u32) };

    let Ok(data) = FontData::new(&buf);

    let dsig = Dsig::read(data)?;

    let Ok(signature_records) = dsig.signature_records();
    assert_eq!(signature_records.len(), 1);
    let Ok(dsig_signature_records) = dsig.signature_records();
    let record = *match <[_]>::get(&*dsig_signature_records, 0) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
      }
    };

    assert_eq!(record.format(), Ok(2));

    let block_attempt = record.signature_block(data);
    assert!(matches!(block_attempt, Err(ReadError::InvalidFormat(2))));
    ::core::result::Result::Ok(())
  }

  #[test]
  fn test_lying_length() -> ::core::result::Result<(), crate::read::ReadError> {
    let buf = font_test_data::be_buffer! { (0x1_u32), (0x1_u16), (0x1_u16), (0x1_u32), (0xB_u32), (0x14_u32), (0x0_u16), (0x0_u16), (0x4_u32), (0xDEADBEEF_u32) };

    let Ok(data) = FontData::new(&buf);

    let dsig = Dsig::read(data)?;

    let Ok(signature_records) = dsig.signature_records();
    assert_eq!(signature_records.len(), 1);
    let Ok(dsig_signature_records) = dsig.signature_records();
    let record = *match <[_]>::get(&*dsig_signature_records, 0) {
      ::core::option::Option::Some(indexed_value) => indexed_value,
      ::core::option::Option::None => {
        return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds);
      }
    };

    let block_attempt = record.signature_block(data);
    assert!(matches!(
      block_attempt,
      Err(ReadError::MalformedData("ambiguous DSIG signature length"))
    ));
    ::core::result::Result::Ok(())
  }
}
