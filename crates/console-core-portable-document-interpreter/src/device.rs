//! What a page is drawn onto: the calls the interpreter makes, in the order the content stream
//! asks for them, and a device that takes every one of them and draws nothing.
//!
//! The three calls with a body here are the ones a device may leave alone. A rectangle is a path
//! unless the device has a faster way to fill one, and marked content is a bracket around what
//! is drawn rather than a drawing, so a device that does not care about it hears nothing.

use crate::font::Glyph;
use crate::soft_mask::SoftMask;
use crate::{BlendMode, ClipPath, Image};
use crate::{GlyphDrawMode, Paint, PathDrawMode};
use console_core_never::Never;
use kurbo::{Affine, BezPath, Rect, Shape};

pub trait Device<'a> {
  fn set_soft_mask(&mut self, mask: Option<SoftMask<'a>>);

  fn set_blend_mode(&mut self, blend_mode: BlendMode);

  fn draw_path(
    &mut self,
    path: &BezPath,
    transform: Affine,
    paint: &Paint<'a>,
    draw_mode: &PathDrawMode,
  );

  fn push_clip_path(&mut self, clip_path: &ClipPath);

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "device implementations receive the existing normalized PDF opacity through this rendering callback"
    )
  )]
  fn push_transparency_group(
    &mut self,
    opacity: f32,
    mask: Option<SoftMask<'a>>,
    blend_mode: BlendMode,
  );

  fn draw_glyph(
    &mut self,
    glyph: &Glyph<'a>,
    transform: Affine,
    glyph_transform: Affine,
    paint: &Paint<'a>,
    draw_mode: &GlyphDrawMode,
  );

  fn draw_image(&mut self, image: Image<'a, '_>, transform: Affine);

  fn pop_clip_path(&mut self);

  fn pop_transparency_group(&mut self);

  fn draw_rect(
    &mut self,
    rect: &Rect,
    transform: Affine,
    paint: &Paint<'a>,
    draw_mode: &PathDrawMode,
  ) -> Result<(), Never> {
    self.draw_path(&rect.to_path(0.1), transform, paint, draw_mode);

    Ok(())
  }

  fn begin_marked_content(&mut self, _tag: &[u8], _mcid: Option<i32>) -> Result<(), Never> {
    Ok(())
  }

  fn end_marked_content(&mut self) -> Result<(), Never> {
    Ok(())
  }
}

pub struct DummyDevice;

impl Device<'_> for DummyDevice {
  fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}

  fn set_blend_mode(&mut self, _: BlendMode) {}

  fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'_>, _: &PathDrawMode) {}

  fn push_clip_path(&mut self, _: &ClipPath) {}

  fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}

  fn draw_glyph(
    &mut self,
    _: &Glyph<'_>,
    _: Affine,
    _: Affine,
    _: &Paint<'_>,
    _: &GlyphDrawMode,
  ) {
  }

  fn draw_image(&mut self, _: Image<'_, '_>, _: Affine) {}

  fn pop_clip_path(&mut self) {}

  fn pop_transparency_group(&mut self) {}
}
