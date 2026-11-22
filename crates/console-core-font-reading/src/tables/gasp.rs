//! The [gasp](https://learn.microsoft.com/en-us/typography/opentype/spec/gasp) table

include!("../../generated/generated_gasp.rs");

#[cfg(test)]
mod tests {

  use super::*;

  #[test]
  fn smoke_test() -> ::core::result::Result<(), crate::read::ReadError> {
    let buf = font_test_data::be_buffer! { (1u16), (2u16), (404u16), (GaspRangeBehavior::GASP_GRIDFIT | GaspRangeBehavior::GASP_DOGRAY), (u16::MAX), (GaspRangeBehavior::GASP_SYMMETRIC_GRIDFIT
    | GaspRangeBehavior::GASP_SYMMETRIC_SMOOTHING) };

    let gasp = Gasp::read(<crate::font_data::FontData<'_> as ::core::convert::From<
      &[::core::primitive::u8],
    >>::from(&*buf))?;
    assert_eq!(gasp.version(), Ok(1));
    let Ok(gasp_ranges) = gasp.gasp_ranges();
    assert_eq!(
            *match <[_]>::get(&*gasp_ranges, 0) { ::core::option::Option::Some(indexed_value) => indexed_value, ::core::option::Option::None => return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds) },
            GaspRange {
                range_max_ppem: <::font_types::BigEndian<::core::primitive::u16> as ::core::convert::From<::core::primitive::u16>>::from(404),
                range_gasp_behavior: <::font_types::BigEndian<crate::tables::gasp::GaspRangeBehavior> as ::core::convert::From<crate::tables::gasp::GaspRangeBehavior>>::from(GaspRangeBehavior::GASP_GRIDFIT
                    | GaspRangeBehavior::GASP_DOGRAY),
            }
        );
    let Ok(gasp_gasp_ranges) = gasp.gasp_ranges();
    assert_eq!(
            *match <[_]>::get(&*gasp_gasp_ranges, 1) { ::core::option::Option::Some(indexed_value) => indexed_value, ::core::option::Option::None => return ::core::result::Result::Err(crate::read::ReadError::OutOfBounds) },
            GaspRange {
                range_max_ppem: <::font_types::BigEndian<::core::primitive::u16> as ::core::convert::From<::core::primitive::u16>>::from(u16::MAX),
                range_gasp_behavior: <::font_types::BigEndian<crate::tables::gasp::GaspRangeBehavior> as ::core::convert::From<crate::tables::gasp::GaspRangeBehavior>>::from(GaspRangeBehavior::GASP_SYMMETRIC_GRIDFIT
                    | GaspRangeBehavior::GASP_SYMMETRIC_SMOOTHING),
            }
        );
    ::core::result::Result::Ok(())
  }
}
