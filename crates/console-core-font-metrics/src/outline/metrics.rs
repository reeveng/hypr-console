//! Helper for loading (possibly variable) horizontal glyph metrics.

use raw::{
  FontRef, TableProvider,
  tables::{hmtx::Hmtx, hvar::Hvar},
  types::{F2Dot14, GlyphId},
};

/// Access to horizontal glyph metrics.
#[derive(Clone)]
pub(crate) struct GlyphHMetrics<'a> {
  pub hmtx: Hmtx<'a>,
  pub hvar: Option<Hvar<'a>>,
}

impl<'a> GlyphHMetrics<'a> {
  pub fn new(font: &FontRef<'a>) -> Option<Self> {
    let hmtx = font.hmtx().ok()?;
    let hvar = font.hvar().ok();
    Some(Self { hmtx, hvar })
  }

  /// Returns the advance width (in font units) for the given glyph and
  /// the location in variation space represented by the set of normalized
  /// coordinates in 2.14 fixed point.
  pub fn advance_width(&self, gid: GlyphId, coords: &'a [F2Dot14]) -> i32 {
    let mut advance = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
        self
          .hmtx
          .advance(gid)
          .unwrap_or_default()
          .unwrap_or_default(),
      );
      migrated_value
    });
    if let (false, Some(hvar)) = (coords.is_empty(), &self.hvar) {
      advance += hvar
        .advance_width_delta(gid, coords)
        .map(|delta| {
          ({
            let Ok(value) = delta.to_i32();
            value
          })
        })
        .unwrap_or(0);
    }
    advance
  }

  /// Returns the left side bearing (in font units) for the given glyph and
  /// the location in variation space represented by the set of normalized
  /// coordinates in 2.14 fixed point.    
  pub fn lsb(&self, gid: GlyphId, coords: &'a [F2Dot14]) -> i32 {
    let mut lsb = ({
      let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i32>(
        self
          .hmtx
          .side_bearing(gid)
          .unwrap_or_default()
          .unwrap_or_default(),
      );
      migrated_value
    });
    if let (false, Some(hvar)) = (coords.is_empty(), &self.hvar) {
      lsb += hvar
        .lsb_delta(gid, coords)
        .map(|delta| {
          ({
            let Ok(value) = delta.to_i32();
            value
          })
        })
        .unwrap_or(0);
    }
    lsb
  }
}
