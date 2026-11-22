//! Showing a string: each character code read off it by the font, drawn as the text rendering
//! mode asks, and the text matrix moved on by its advance.
//!
//! A string with no font set, or with one that could not be read, is shown in Helvetica only
//! when its bytes are ASCII, because a fallback reading anything else would draw the wrong
//! letters where a reader draws none. Invisible text is still handed to the device, so a device
//! that extracts text hears it.

use crate::context::Context;
use crate::device::Device;
use crate::font::Glyph;
use crate::interpret::path::get_paint;
use crate::interpret::state::TextStateFont;
use crate::{GlyphDrawMode, PaintOperation, Visibility};
use console_core_never::Never;
use console_core_portable_document_syntax::object;
use console_core_portable_document_syntax::page::Resources;
use kurbo::{Affine, Vec2};

enum Shown {
  Glyphs,
  Advances,
}

pub(crate) fn show_text_string<'a>(
  ctx: &mut Context<'a>,
  device: &mut impl Device<'a>,
  resources: &Resources<'a>,
  text: &object::String<'_>,
) -> Result<(), Never> {
  let Ok(state) = ctx.get();
  let font = match state.text_state.font.clone() {
    Some(font) => font,
    None => {
      warn!("tried to show text without active font");

      return Ok(());
    }
  };
  let Ok(bytes) = text.as_bytes();
  let shown = match (&font, bytes.is_ascii()) {
    (TextStateFont::Font(_), _) | (TextStateFont::Fallback(_), true) => Shown::Glyphs,
    (TextStateFont::Fallback(_), false) => Shown::Advances,
  };

  let Ok(codes) = font.codes(bytes);

  for code in codes {
    match shown {
      Shown::Glyphs => {
        let Ok(glyph_id) = font.map_code(code.value);
        let Ok(displacement) = font.origin_displacement(code.value);
        let Ok((glyph, glyph_transform)) =
          font.get_glyph(glyph_id, code.value, ctx, resources, displacement);
        let Ok(()) = show_glyph(ctx, device, &glyph, glyph_transform);
      }
      Shown::Advances => {}
    }

    let Ok(state_2) = ctx.get_mut();
    let Ok(()) = state_2.text_state.apply_code_advance(code);
  }

  Ok(())
}

pub(crate) fn next_line(ctx: &mut Context<'_>, offset: Vec2) -> Result<(), Never> {
  let Ok(state) = ctx.get_mut();
  let moved = state.text_state.text_line_matrix * Affine::translate(offset);

  state.text_state.text_line_matrix = moved;
  state.text_state.text_matrix = moved;

  Ok(())
}

#[derive(Clone, Copy)]
enum Painted {
  Fill,
  Stroke,
  FillAndStroke,
  Invisible,
  Nothing,
}

#[derive(Clone, Copy)]
enum Clipped {
  Clipped,
  Unclipped,
}

pub(crate) fn show_glyph<'a>(
  ctx: &mut Context<'a>,
  device: &mut impl Device<'a>,
  glyph: &Glyph<'a>,
  glyph_transform: Affine,
) -> Result<(), Never> {
  let Ok(visibility) = ctx.ocg_state.is_visible();

  match visibility {
    Visibility::Visible => {}
    Visibility::Hidden => return Ok(()),
  }

  let Ok(state) = ctx.get();

  device.set_soft_mask(state.graphics_state.soft_mask.clone());
  device.set_blend_mode(state.graphics_state.blend_mode);

  let Ok((painted, clipped)) = state.text_state.render_mode.parts();

  match clipped {
    Clipped::Clipped => {
      let Ok(()) = clip_glyph(ctx, glyph, glyph_transform);
    }
    Clipped::Unclipped => {}
  }

  let Ok(stroke_props) = ctx.stroke_props();
  let Ok(state_2) = ctx.get();
  let transform = state_2.ctm;
  let Ok(fill) = get_paint(ctx, PaintOperation::Fill);
  let Ok(stroke) = get_paint(ctx, PaintOperation::Stroke);

  match painted {
    Painted::Fill => device.draw_glyph(
      glyph,
      transform,
      glyph_transform,
      &fill,
      &GlyphDrawMode::Fill,
    ),
    Painted::Stroke => device.draw_glyph(
      glyph,
      transform,
      glyph_transform,
      &stroke,
      &GlyphDrawMode::Stroke(stroke_props),
    ),
    Painted::FillAndStroke => {
      device.draw_glyph(
        glyph,
        transform,
        glyph_transform,
        &fill,
        &GlyphDrawMode::Fill,
      );
      device.draw_glyph(
        glyph,
        transform,
        glyph_transform,
        &stroke,
        &GlyphDrawMode::Stroke(stroke_props),
      );
    }
    Painted::Invisible => device.draw_glyph(
      glyph,
      transform,
      glyph_transform,
      &fill,
      &GlyphDrawMode::Invisible,
    ),
    Painted::Nothing => {}
  }

  Ok(())
}

pub(crate) fn clip_glyph(
  context: &mut Context<'_>,
  glyph: &Glyph<'_>,
  transform: Affine,
) -> Result<(), Never> {
  match glyph {
    Glyph::Outline(outline_glyph) => {
      let Ok(outline) = outline_glyph.outline();
      let outline_2 = transform * outline;

      let first_segment = outline_2.segments().next();

      match first_segment {
        Some(_) => {
          let Ok(state) = context.get_mut();

          state.text_state.clip_paths.extend(outline_2);
        }
        None => {}
      }
    }
    Glyph::Type3(_) => {
      warn!("text rendering mode clip not implemented for shape glyphs");
    }
  }

  Ok(())
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) enum TextRenderingMode {
  #[default]
  Fill,
  Stroke,
  FillStroke,
  Invisible,
  FillAndClip,
  StrokeAndClip,
  FillAndStrokeAndClip,
  Clip,
}

impl TextRenderingMode {
  fn parts(self) -> Result<(Painted, Clipped), Never> {
    Ok(match self {
      Self::Fill => (Painted::Fill, Clipped::Unclipped),
      Self::Stroke => (Painted::Stroke, Clipped::Unclipped),
      Self::FillStroke => (Painted::FillAndStroke, Clipped::Unclipped),
      Self::Invisible => (Painted::Invisible, Clipped::Unclipped),
      Self::FillAndClip => (Painted::Fill, Clipped::Clipped),
      Self::StrokeAndClip => (Painted::Stroke, Clipped::Clipped),
      Self::FillAndStrokeAndClip => (Painted::FillAndStroke, Clipped::Clipped),
      Self::Clip => (Painted::Nothing, Clipped::Clipped),
    })
  }
}
