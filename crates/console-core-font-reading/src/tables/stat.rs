//! The [STAT](https://learn.microsoft.com/en-us/typography/opentype/spec/stat) table

include!("../../generated/generated_stat.rs");

impl AxisValue<'_> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `AxisValue::value` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn value(&self) -> Option<Fixed> {
    match self {
      Self::Format1(item) => Some({
        let Ok(explicit_value) = item.value();
        explicit_value
      }),
      Self::Format2(item) => {
        let Ok(nominal_value) = item.nominal_value();

        Some(nominal_value)
      }
      Self::Format3(item) => Some({
        let Ok(explicit_value) = item.value();
        explicit_value
      }),
      Self::Format4(_) => None,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `AxisValue::linked_value` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn linked_value(&self) -> Option<Fixed> {
    match self {
      Self::Format3(item) => {
        let Ok(linked_value) = item.linked_value();

        Some(linked_value)
      }
      Self::Format1(_) | Self::Format2(_) | Self::Format4(_) => None,
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `AxisValue::axis_index` method returns the optional value directly to callers that match its presence"
    )
  )]
  pub fn axis_index(&self) -> Option<u16> {
    match self {
      Self::Format1(item) => {
        let Ok(axis_index) = item.axis_index();

        Some(axis_index)
      }
      Self::Format2(item) => {
        let Ok(axis_index) = item.axis_index();

        Some(axis_index)
      }
      Self::Format3(item) => {
        let Ok(axis_index) = item.axis_index();

        Some(axis_index)
      }
      Self::Format4(_) => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use types::{Fixed, NameId};

  use crate::{FontRef, table_provider::TableProvider};

  use super::*;

  #[test]
  fn smoke_test() -> ::core::result::Result<(), crate::test_support::Failure> {
    let font = crate::test_support::fixture_value(
      FontRef::new(font_test_data::VAZIRMATN_VAR),
      "fixture value",
    )?;
    let table = crate::test_support::fixture_value(font.stat(), "fixture value")?;
    assert_eq!(table.design_axis_count(), Ok(1));
    let fixture_value = crate::test_support::fixture_value(table.design_axes(), "fixture value")?;
    let axis_record = crate::test_support::fixture_value(fixture_value.first(), "design axis")?;
    assert_eq!(
      axis_record.axis_tag(),
      Ok({
        let Ok(value) = Tag::new(b"wght");
        value
      })
    );
    assert_eq!(
      axis_record.axis_name_id(),
      Ok({
        let Ok(explicit_value) = NameId::new(257);
        explicit_value
      })
    );
    assert_eq!(axis_record.axis_ordering(), Ok(0));
    let axis_values = crate::test_support::fixture_value(
      {
        let axis_values_offset =
          crate::test_support::fixture_value(table.offset_to_axis_values(), "fixture value")?;
        axis_values_offset
      },
      "fixture value",
    )?;
    let Ok(axis_values_axis_values) = axis_values.axis_values();
    let Ok(iter) = axis_values_axis_values.iter();
    let axis_values_2 = iter.collect::<Result<Vec<_>, _>>()?;

    assert_eq!(axis_values_2.len(), 3);
    let last = crate::test_support::fixture_value(axis_values_2.get(2), "third axis value")?;

    match last {
      AxisValue::Format1(table_2) => {
        assert_eq!(table_2.axis_index(), Ok(0));
        assert_eq!(
          table_2.value_name_id(),
          Ok({
            let Ok(explicit_value) = NameId::new(264);
            explicit_value
          })
        );
        assert_eq!(
          table_2.value(),
          Ok({
            let Ok(explicit_value) = Fixed::from_f64(700.0);
            explicit_value
          })
        );
      }
      AxisValue::Format2(_) | AxisValue::Format3(_) | AxisValue::Format4(_) => {}
    };

    Ok(())
  }
}
