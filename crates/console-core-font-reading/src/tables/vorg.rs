//! The [VORG (Vertical Origin)](https://docs.microsoft.com/en-us/typography/opentype/spec/vorg) table.

include!("../../generated/generated_vorg.rs");

impl Vorg<'_> {
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "the published `Vorg::vertical_origin_y` method returns `i16` directly in the existing parser interface"
    )
  )]
  pub fn vertical_origin_y(&self, glyph_id: GlyphId) -> i16 {
    let Ok(gid) = glyph_id.to_u32();
    let Ok(metrics) = self.vert_origin_y_metrics();

    match metrics.binary_search_by(|rec| {
      let Ok(glyph_index) = rec.glyph_index();
      let Ok(value_1) = glyph_index.to_u32();

      value_1.cmp(&gid)
    }) {
      Ok(ix) => match metrics.get(ix) {
        Some(metric) => {
          let Ok(vert_origin_y) = metric.vert_origin_y();

          vert_origin_y
        }
        None => 0,
      },
      Err(_absent) => {
        let Ok(default_vert_origin_y) = self.default_vert_origin_y();

        default_vert_origin_y
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{FontRef, TableProvider};

  #[test]
  fn vertical_origins() -> ::core::result::Result<(), crate::read::ReadError> {
    let font = FontRef::new(font_test_data::VORG)?;
    let vorg = font.vorg()?;

    assert_eq!(
      vorg.vertical_origin_y({
        let Ok(explicit_value) = GlyphId::new(0);
        explicit_value
      }),
      880
    );
    assert_eq!(
      vorg.vertical_origin_y({
        let Ok(explicit_value) = GlyphId::new(1);
        explicit_value
      }),
      867
    );
    assert_eq!(
      vorg.vertical_origin_y({
        let Ok(explicit_value) = GlyphId::new(2);
        explicit_value
      }),
      880
    );
    assert_eq!(
      vorg.vertical_origin_y({
        let Ok(explicit_value) = GlyphId::new(3);
        explicit_value
      }),
      824
    );
    ::core::result::Result::Ok(())
  }
}
