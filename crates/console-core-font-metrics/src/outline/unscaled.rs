//! Compact representation of an unscaled, unhinted outline.

#![allow(dead_code)]

use alloc::vec::Vec;

use super::DrawError;
use core::ops::Range;
use raw::{
  tables::glyf::PointFlags,
  types::{F26Dot6, Point},
};

#[derive(Copy, Clone, Default, Debug)]
pub(super) struct UnscaledPoint {
  pub x: i16,
  pub y: i16,
  pub flags: PointFlags,
  pub is_contour_start: bool,
}

impl UnscaledPoint {
  pub fn from_glyf_point(
    point: Point<F26Dot6>,
    flags: PointFlags,
    is_contour_start: bool,
  ) -> Self {
    let point = point.map(|x| {
      ({
        let Ok(migrated_value) = console_core_number_conversion::wrapped::<_, i16>(
          ({
            let Ok(value) = x.to_bits();
            value
          } >> 6),
        );
        migrated_value
      })
    });
    Self {
      x: ({
        let Ok(value) = point;
        value
      })
      .x,
      y: ({
        let Ok(value) = point;
        value
      })
      .y,
      flags: flags.without_markers(),
      is_contour_start,
    }
  }

  pub fn is_on_curve(self) -> bool {
    matches!(self.flags.curve_membership(), read_fonts::tables::glyf::CurveMembership::OnCurve)
  }
}

pub(super) trait UnscaledOutlineSink {
  fn try_reserve(&mut self, additional: usize) -> Result<(), DrawError>;
  fn push(&mut self, point: UnscaledPoint) -> Result<(), DrawError>;
  fn extend(&mut self, points: impl IntoIterator<Item = UnscaledPoint>) -> Result<(), DrawError> {
    for point in points.into_iter() {
      self.push(point)?;
    }
    Ok(())
  }
}

pub(super) struct UnscaledOutlineBuf(Vec<UnscaledPoint>);

impl UnscaledOutlineBuf {
  pub fn new() -> Self {
    Self(Vec::new())
  }

  pub fn clear(&mut self) {
    self.0.clear();
  }

  pub fn as_ref(&self) -> UnscaledOutlineRef<'_> {
    UnscaledOutlineRef {
      points: self.0.as_slice(),
    }
  }
}

impl UnscaledOutlineSink for UnscaledOutlineBuf {
  fn try_reserve(&mut self, additional: usize) -> Result<(), DrawError> {
    self
      .0
      .try_reserve(additional)
      .map_err(|_| DrawError::InsufficientMemory)
  }

  fn push(&mut self, point: UnscaledPoint) -> Result<(), DrawError> {
    self.0.push(point);
    Ok(())
  }
}

#[derive(Copy, Clone, Debug)]
pub(super) struct UnscaledOutlineRef<'a> {
  pub points: &'a [UnscaledPoint],
}

impl UnscaledOutlineRef<'_> {
  /// Returns the range of contour points and the index of the point within
  /// that contour for the last point where `f` returns true.
  ///
  /// This is common code used for finding extrema when materializing blue
  /// zones.
  ///
  /// For example: <https://gitlab.freedesktop.org/freetype/freetype/-/blob/57617782464411201ce7bbc93b086c1b4d7d84a5/src/autofit/aflatin.c#L509>
  pub fn find_last_contour(
    &self,
    mut f: impl FnMut(&UnscaledPoint) -> bool,
  ) -> Option<(Range<usize>, usize)> {
    match self.points.is_empty() {
      true => {
        return None;
      }
      false => {}
    }
    let mut best_contour = 0..0;

    let mut best_point = 0;
    let mut cur_contour = 0..0;
    let mut found_best_in_cur_contour = false;
    for (point_ix, point) in self.points.iter().enumerate() {
      match point.is_contour_start {
        true => {
          match found_best_in_cur_contour {
            true => {
              best_contour = cur_contour;
            }
            false => {}
          }
          cur_contour = point_ix..point_ix;
          found_best_in_cur_contour = false;

          match (self.points.get(point_ix + 1)) {
            Some(next_point) if next_point.is_contour_start => continue,
            None => continue,
            _ => {}
          }
        }
        false => {}
      }
      cur_contour.end += 1;
      match f(point) {
        true => {
          best_point = point_ix - cur_contour.start;
          found_best_in_cur_contour = true;
        }
        false => {}
      }
    }
    match found_best_in_cur_contour {
      true => {
        best_contour = cur_contour;
      }
      false => {}
    }
    match !best_contour.is_empty() {
      true => Some((best_contour, best_point)),
      false => None,
    }
  }
}

#[derive(Copy, Clone)]
enum PendingElement {
  Line([f32; 2]),
  Cubic([f32; 6]),
}

/// Adapts an UnscaledOutlineSink to be fed from a pen while tracking
/// memory allocation errors.
pub(super) struct UnscaledPenAdapter<'a, T> {
  sink: &'a mut T,
  failed: bool,
  last_start: Option<(f32, f32)>,
  pending: Option<PendingElement>,
}

impl<'a, T> UnscaledPenAdapter<'a, T> {
  pub fn new(sink: &'a mut T) -> Self {
    Self {
      sink,
      failed: false,
      last_start: None,
      pending: None,
    }
  }
}

impl<T> UnscaledPenAdapter<'_, T>
where
  T: UnscaledOutlineSink,
{
  fn push(&mut self, x: f32, y: f32, flags: PointFlags, is_contour_start: bool) {
    match self
      .sink
      .push(UnscaledPoint {
        x: ({
          let Ok(migrated_value) = console_core_number_conversion::toward_zero_i16(f64::from(x));
          migrated_value
        }),
        y: ({
          let Ok(migrated_value) = console_core_number_conversion::toward_zero_i16(f64::from(y));
          migrated_value
        }),
        flags,
        is_contour_start,
      })
      .is_err()
    {
      true => {
        self.failed = true;
      }
      false => {}
    }
  }

  fn flush_pending(&mut self, for_close: bool) {
    match self.pending.take() {
      Some(element) => {
        let [x, y] = match element {
          PendingElement::Line([x, y]) => [x, y],
          PendingElement::Cubic([x0, y0, x1, y1, x, y]) => {
            self.push(x0, y0, PointFlags::off_curve_cubic(), false);
            self.push(x1, y1, PointFlags::off_curve_cubic(), false);
            [x, y]
          }
        };
        match !for_close || self.last_start != Some((x, y)) {
          true => {
            self.push(x, y, PointFlags::on_curve(), false);
          }
          false => {}
        }
      }
      None => {}
    }
  }

  pub fn finish(mut self) -> Result<(), DrawError> {
    self.flush_pending(true);
    match self.failed {
      true => Err(DrawError::InsufficientMemory),
      false => Ok(()),
    }
  }
}

impl<T: UnscaledOutlineSink> super::OutlinePen for UnscaledPenAdapter<'_, T> {
  fn move_to(&mut self, x: f32, y: f32) {
    self.push(x, y, PointFlags::on_curve(), true);
    self.last_start = Some((x, y));
  }

  fn line_to(&mut self, x: f32, y: f32) {
    self.flush_pending(false);
    self.pending = Some(PendingElement::Line([x, y]));
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    self.flush_pending(false);
    self.push(cx0, cy0, PointFlags::off_curve_quad(), false);
    self.push(x, y, PointFlags::on_curve(), false);
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    self.flush_pending(false);
    self.pending = Some(PendingElement::Cubic([cx0, cy0, cx1, cy1, x, y]));
  }

  fn close(&mut self) {
    self.flush_pending(true);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{MetadataProvider, outline::OutlinePen, prelude::LocationRef};
  use raw::{FontRef, types::GlyphId};

  #[test]
  fn read_glyf_outline() {
    let font = FontRef::new(font_test_data::MATERIAL_SYMBOLS_SUBSET).unwrap();
    let glyph = font
      .outline_glyphs()
      .get(
        ({
          let Ok(value) = GlyphId::new(5);
          value
        }),
      )
      .unwrap();
    let mut outline = UnscaledOutlineBuf::new();
    glyph
      .draw_unscaled(LocationRef::default(), None, &mut outline)
      .unwrap();
    let outline = outline.as_ref();
    let expected = [
      (400, 80, 1),
      (400, 360, 1),
      (320, 360, 1),
      (320, 600, 1),
      (320, 633, 0),
      (367, 680, 0),
      (400, 680, 1),
      (560, 680, 1),
      (593, 680, 0),
      (640, 633, 0),
      (640, 600, 1),
      (640, 360, 1),
      (560, 360, 1),
      (560, 80, 1),
      (480, 720, 1),
      (447, 720, 0),
      (400, 767, 0),
      (400, 800, 1),
      (400, 833, 0),
      (447, 880, 0),
      (480, 880, 1),
      (513, 880, 0),
      (560, 833, 0),
      (560, 800, 1),
      (560, 767, 0),
      (513, 720, 0),
    ];
    let points = outline
      .points
      .iter()
      .map(|point| (point.x, point.y, point.flags.to_bits()))
      .collect::<Vec<_>>();
    assert_eq!(points, expected);
  }

  #[test]
  #[cfg(feature = "spec_next")]
  fn read_cubic_glyf_outline() {
    let font = FontRef::new(font_test_data::CUBIC_GLYF).unwrap();
    let glyph = font
      .outline_glyphs()
      .get(
        ({
          let Ok(value) = GlyphId::new(2);
          value
        }),
      )
      .unwrap();
    let mut outline = UnscaledOutlineBuf::new();
    glyph
      .draw_unscaled(LocationRef::default(), None, &mut outline)
      .unwrap();
    let outline = outline.as_ref();
    let expected = [
      (278, 710, 1),
      (278, 470, 1),
      (300, 500, 128),
      (800, 500, 128),
      (998, 470, 1),
      (998, 710, 1),
    ];
    let points = outline
      .points
      .iter()
      .map(|point| (point.x, point.y, point.flags.to_bits()))
      .collect::<Vec<_>>();
    assert_eq!(points, expected);
  }

  #[test]
  fn read_cff_outline() {
    let font = FontRef::new(font_test_data::CANTARELL_VF_TRIMMED).unwrap();
    let glyph = font
      .outline_glyphs()
      .get(
        ({
          let Ok(value) = GlyphId::new(2);
          value
        }),
      )
      .unwrap();
    let mut outline = UnscaledOutlineBuf::new();
    glyph
      .draw_unscaled(LocationRef::default(), None, &mut outline)
      .unwrap();
    let outline = outline.as_ref();
    let expected = [
      (83, 0, 1),
      (163, 0, 1),
      (163, 482, 1),
      (83, 482, 1),
      (124, 595, 1),
      (160, 595, 128),
      (181, 616, 128),
      (181, 652, 1),
      (181, 688, 128),
      (160, 709, 128),
      (124, 709, 1),
      (88, 709, 128),
      (67, 688, 128),
      (67, 652, 1),
      (67, 616, 128),
      (88, 595, 128),
    ];
    let points = outline
      .points
      .iter()
      .map(|point| (point.x, point.y, point.flags.to_bits()))
      .collect::<Vec<_>>();
    assert_eq!(points, expected);
  }

  #[test]
  fn find_vertical_extrema() {
    let font = FontRef::new(font_test_data::MATERIAL_SYMBOLS_SUBSET).unwrap();
    let glyph = font
      .outline_glyphs()
      .get(
        ({
          let Ok(value) = GlyphId::new(5);
          value
        }),
      )
      .unwrap();
    let mut outline = UnscaledOutlineBuf::new();
    glyph
      .draw_unscaled(LocationRef::default(), None, &mut outline)
      .unwrap();
    let outline = outline.as_ref();

    let mut top_y = None;
    let (top_contour, top_point_ix) = outline
      .find_last_contour(|point| match top_y.is_none() || Some(point.y) > top_y {
        true => {
          top_y = Some(point.y);
          true
        }
        false => false,
      })
      .unwrap();
    assert_eq!(top_contour, 14..26);
    assert_eq!(top_point_ix, 5);
    assert_eq!(top_y, Some(880));

    let mut bottom_y = None;
    let (bottom_contour, bottom_point_ix) = outline
      .find_last_contour(
        |point| match bottom_y.is_none() || Some(point.y) < bottom_y {
          true => {
            bottom_y = Some(point.y);
            true
          }
          false => false,
        },
      )
      .unwrap();
    assert_eq!(bottom_contour, 0..14);
    assert_eq!(bottom_point_ix, 0);
    assert_eq!(bottom_y, Some(80));
  }

  /// When a contour ends with a line or cubic whose end matches the start
  /// point, omit the last on curve. This matches FreeType behavior when
  /// constructing a TrueType style outline from a CFF font.
  #[test]
  fn omit_unnecessary_trailing_oncurves() {
    let mut outline = UnscaledOutlineBuf::new();
    let mut pen = UnscaledPenAdapter::new(&mut outline);
    pen.move_to(0.5, 1.5);
    pen.line_to(1.0, 2.0);

    pen.line_to(0.5, 1.5);
    pen.close();
    pen.move_to(5.0, 6.0);
    pen.curve_to(1.0, 1.0, 2.0, 2.0, 2.0, 3.0);

    pen.curve_to(1.0, 1.0, 2.0, 2.0, 5.0, 6.0);
    pen.close();
    pen.move_to(5.0, 6.0);

    pen.curve_to(1.0, 1.0, 2.0, 2.0, 2.0, 3.0);
    pen.close();
    pen.finish().unwrap();

    let on_curves = outline
      .0
      .iter()
      .map(|point| matches!(point.flags.curve_membership(), read_fonts::tables::glyf::CurveMembership::OnCurve))
      .collect::<Vec<_>>();
    #[rustfmt::skip]
        let expected_on_curves = [
            true,
            true,

            true,
            false,
            false,
            true,
            false,
            false,

            true,
            false,
            false,
            true,
        ];
    assert_eq!(on_curves, expected_on_curves);
  }
}
