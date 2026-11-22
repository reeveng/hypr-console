//! Resize a little-endian integer representation during constant evaluation.
//!
//! Widening sign-extends signed integers and zero-extends unsigned integers.
//! Narrowing retains the low bytes for either kind of integer.
//! The byte and word conversions used in const table loops avoid slice copies.

use console_core_never::Never;

#[doc = "Convert integers in const expressions while retaining their low destination bits."]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one call names the two primitive types and lets their arrays infer both byte lengths, replacing two endian conversions and an infallible result extraction at each const call site"
  )
)]
#[macro_export]
macro_rules! const_wrapped_integer {
    (unsigned ::core::primitive::u8 => ::core::primitive::u16, $value:expr) => {
        ::core::primitive::u16::from_le_bytes([$value, 0])
    };
    (unsigned ::core::primitive::u16 => ::core::primitive::u8, $value:expr) => {
        match ::core::primitive::u16::to_le_bytes($value) {
            [low_byte, _] => low_byte,
        }
    };
    (unsigned $source:ty => $destination:ty, $value:expr) => {
        $crate::const_wrapped_integer!(@ resize_unsigned_le_bytes, $source, $destination, $value)
    };
    (signed $source:ty => $destination:ty, $value:expr) => {
        $crate::const_wrapped_integer!(@ resize_signed_le_bytes, $source, $destination, $value)
    };
    (@ $resize:ident, $source:ty, $destination:ty, $value:expr) => {
        <$destination>::from_le_bytes(
            match $crate::$resize(<$source>::to_le_bytes($value)) {
                ::core::result::Result::Ok(resized_bytes) => resized_bytes,
            },
        )
    };
}

#[inline]
#[doc(hidden)]
pub const fn resize_unsigned_le_bytes<const FROM: usize, const TO: usize>(
  bytes: [u8; FROM],
) -> Result<[u8; TO], Never> {
  resize_le_bytes(bytes, 0)
}

#[inline]
#[doc(hidden)]
pub const fn resize_signed_le_bytes<const FROM: usize, const TO: usize>(
  bytes: [u8; FROM],
) -> Result<[u8; TO], Never> {
  let padding = match bytes.last() {
    Some(128..=u8::MAX) => u8::MAX,
    Some(0..=127) | None => 0,
  };

  resize_le_bytes(bytes, padding)
}

#[inline]
#[cfg_attr(
  dylint_lib = "explicit056_no_hidden_panic_api",
  expect(
    explicit056_no_hidden_panic_api,
    reason = "each checked split uses the length of the other array, so the two slices passed to the copy have equal lengths"
  )
)]
const fn resize_le_bytes<const FROM: usize, const TO: usize>(
  source: [u8; FROM],
  padding: u8,
) -> Result<[u8; TO], Never> {
  let mut destination = [padding; TO];

  match destination.split_at_mut_checked(FROM) {
    Some((low_bytes, _)) => low_bytes.copy_from_slice(&source),
    None => match source.split_at_checked(TO) {
      Some((low_bytes, _)) => destination.copy_from_slice(low_bytes),
      None => {}
    },
  }

  Ok(destination)
}
