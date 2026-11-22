use crate::{Color, DecorationSpan, LayoutRun, PhysicalGlyph, UnderlineStyle};
use crate::{FontSystem, GlyphCache};

pub trait Renderer {
  fn rectangle(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color);

  fn glyph(&mut self, physical_glyph: PhysicalGlyph, color: Color);
}

pub fn render_decoration<R: Renderer>(
  renderer: &mut R,
  run: &LayoutRun,
  default_color: Color,
) -> Result<(), console_core_never::Never> {
  {
    for span in run.decorations {
      let Ok(()) = draw_decoration_span(renderer, run, span, default_color);
    }

    Ok(())
  }
}

fn draw_decoration_span<R: Renderer>(
  renderer: &mut R,
  run: &LayoutRun,
  span: &DecorationSpan,
  default_color: Color,
) -> Result<(), console_core_never::Never> {
  {
    let glyphs = match (run.glyphs).get({
      let core::ops::Range { start, end } = span.glyph_range.clone();
      (match console_core_number_conversion::index(start) {
        Ok(position) => position,
      })..(match console_core_number_conversion::index(end) {
        Ok(position) => position,
      })
    }) {
      Some(value) => value,
      None => return Ok(()),
    };

    match ::core::primitive::bool::then_some(glyphs.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let deco = &span.data;
    let td = &deco.text_decoration;
    let font_size = span.font_size;

    let mut x_min = f32::INFINITY;
    let mut x_max = f32::NEG_INFINITY;

    for g in glyphs {
      x_min = x_min.min(g.x);
      x_max = x_max.max(g.x + g.w);
    }

    let width = x_max - x_min;

    match ::core::primitive::bool::then_some(width <= 0.0, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let Ok(w) = console_core_number_conversion::toward_zero_u32(f64::from(width));

    match ::core::primitive::bool::then_some(w == 0, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    let x_start = x_min;

    match td.underline {
      UnderlineStyle::None => {}
      UnderlineStyle::Single => {
        let color = match td.underline_color_opt.or(span.color_opt) {
          Some(present) => present,
          None => default_color,
        };
        let thickness = (deco.underline_metrics.thickness * font_size)
          .max(1.0)
          .ceil();
        let y = run.line_y - deco.underline_metrics.offset * font_size;
        renderer.rectangle(
          match console_core_number_conversion::toward_zero_i32(f64::from(x_start)) {
            Ok(value) => value,
          },
          match console_core_number_conversion::toward_zero_i32(f64::from(y)) {
            Ok(value) => value,
          },
          w,
          match console_core_number_conversion::toward_zero_u32(f64::from(thickness)) {
            Ok(value) => value,
          },
          color,
        );
      }
      UnderlineStyle::Double => {
        let color = match td.underline_color_opt.or(span.color_opt) {
          Some(present) => present,
          None => default_color,
        };
        let thickness = (deco.underline_metrics.thickness * font_size)
          .max(1.0)
          .ceil();
        let gap = thickness;
        let y = run.line_y - deco.underline_metrics.offset * font_size;
        renderer.rectangle(
          match console_core_number_conversion::toward_zero_i32(f64::from(x_start)) {
            Ok(value) => value,
          },
          match console_core_number_conversion::toward_zero_i32(f64::from(y)) {
            Ok(value) => value,
          },
          w,
          match console_core_number_conversion::toward_zero_u32(f64::from(thickness)) {
            Ok(value) => value,
          },
          color,
        );
        renderer.rectangle(
          match console_core_number_conversion::toward_zero_i32(f64::from(x_start)) {
            Ok(value) => value,
          },
          match console_core_number_conversion::toward_zero_i32(f64::from(y + thickness + gap)) {
            Ok(value) => value,
          },
          w,
          match console_core_number_conversion::toward_zero_u32(f64::from(thickness)) {
            Ok(value) => value,
          },
          color,
        );
      }
    }

    match td.strikethrough {
      crate::DecorationLine::Visible => {
        let color = match td.strikethrough_color_opt.or(span.color_opt) {
          Some(present) => present,
          None => default_color,
        };
        let thickness = (deco.strikethrough_metrics.thickness * font_size)
          .max(1.0)
          .ceil();
        let y = run.line_y - deco.strikethrough_metrics.offset * font_size;
        renderer.rectangle(
          match console_core_number_conversion::toward_zero_i32(f64::from(x_start)) {
            Ok(value) => value,
          },
          match console_core_number_conversion::toward_zero_i32(f64::from(y)) {
            Ok(value) => value,
          },
          w,
          match console_core_number_conversion::toward_zero_u32(f64::from(thickness)) {
            Ok(value) => value,
          },
          color,
        );
      }
      crate::DecorationLine::Hidden => {}
    }

    match td.overline {
      crate::DecorationLine::Visible => {
        let color = match td.overline_color_opt.or(span.color_opt) {
          Some(present) => present,
          None => default_color,
        };

        let thickness = (deco.underline_metrics.thickness * font_size)
          .max(1.0)
          .ceil();

        let y = (run.line_y - deco.ascent * font_size).max(run.line_top);
        renderer.rectangle(
          match console_core_number_conversion::toward_zero_i32(f64::from(x_start)) {
            Ok(value) => value,
          },
          match console_core_number_conversion::toward_zero_i32(f64::from(y)) {
            Ok(value) => value,
          },
          w,
          match console_core_number_conversion::toward_zero_u32(f64::from(thickness)) {
            Ok(value) => value,
          },
          color,
        );
      }
      crate::DecorationLine::Hidden => {}
    };

    Ok(())
  }
}

#[derive(Debug)]
pub struct LegacyRenderer<'a, State, F: FnMut(&mut State, i32, i32, u32, u32, Color)> {
  pub font_system: &'a mut FontSystem,
  pub cache: &'a mut GlyphCache,
  pub state: &'a mut State,
  pub callback: F,
}

impl<'a, State, F: FnMut(&mut State, i32, i32, u32, u32, Color)> Renderer
  for LegacyRenderer<'a, State, F>
{
  fn rectangle(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
    (self.callback)(self.state, x, y, w, h, color);
  }

  fn glyph(&mut self, physical_glyph: PhysicalGlyph, color: Color) {
    let Ok(()) = self.cache.with_pixels(
      self.font_system,
      physical_glyph.cache_key,
      color,
      &mut (&mut self.callback, &mut *self.state),
      |state, x, y, pixel_color| {
        (state.0)(
          state.1,
          i32::saturating_add(physical_glyph.x, x),
          i32::saturating_add(physical_glyph.y, y),
          1,
          1,
          pixel_color,
        );
      },
    );
  }
}
