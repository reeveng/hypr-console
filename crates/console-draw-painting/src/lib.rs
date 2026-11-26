//! Shapes, put into a buffer someone else owns.
//!
//! A painting pass verifies opacity before touching a preserved background.
//!
//! **Everything here is in points, and the buffer is in pixels.** A surface is
//! handed out at device resolution and every number in this tree is logical, so
//! the one place the two meet is the transform set on the context before the
//! first shape is drawn. [`Frame`] carries both sizes with their names on, so
//! there is no order to get wrong and no scale passed about as a bare ratio.
//! After that line every coordinate a caller wrote is the coordinate it meant,
//! and how a corner lands on a half pixel is the rasterizer's problem rather than
//! something worked out again at every rounded rectangle.
//!
//! **It knows about no compositor, and that is the point.** What it is given is
//! a slice of bytes and two sizes. So the drawing can be pressed against a slab
//! of memory on a machine with no screen, and the thing that actually needs a
//! screen -- whether the color that came out is the color the palette spends
//! -- stays a check at the stage that can answer it.
//!
//! **A word says what it is set in, and this asks for no font of its own.**
//! [`Font`] is `console-core-shapes`' rather than this crate's, because a face
//! is part of what a run of words *is*: the bar sets a clock in one family and
//! the icon beside it in another, in one frame, and a font handed to `onto`
//! would have been one font for the whole surface. What is measured still takes
//! one, because a measurement is asked about a run that has not been placed yet.
//!
//! **Measuring is separate from drawing and comes first.** [`measured`] asks
//! the text engine how tall a run wraps to, with no surface in the room, and hands back a
//! size. Placement is arithmetic over that answer somewhere else. The order
//! matters: a placement that measured as it went would be a placement that
//! could only be checked where there is a font, and the whole reason the shape
//! list is pure is so that it can be checked where there is not.
//!
//! **A page of a book is lines, so where a run breaks is said as well.**
//! [`wrapped`] is the same question as [`measured`] with the answer kept in
//! pieces: the text of each line the engine broke the run into, at the width it was
//! given. A reader turning pages has to know which line is the last one that
//! fits, and the only thing that knows where a paragraph breaks is the thing
//! that shapes it -- a count of letters is wrong in every proportional face and
//! in every script without spaces.
//!
//! **The layout is independent of the screen.** Measurement and painting use
//! the same logical coordinates without metrics hinting. The text engine
//! rasterizes glyphs at the device scale only after wrapping has been decided,
//! so a word placed at its measured width stays on the same line at every scale.
//!
//! **A film is drawn over what was already there.** [`onto`] clears the
//! buffer and draws every shape, which is right for a card that changed and
//! wrong thirty times a second for a card where only the picture did. [`over`]
//! keeps what the last frame left and draws the shapes it is handed on top, so
//! a film's next frame costs its own rectangle and nothing else on the card is
//! painted again.
//!
//! **A color arrives as `Oklch` and becomes channels here.** That is the whole
//! reason this crate exists as something separate from the shape list: the
//! palette is decided in a perceptual space and every other file this desktop
//! writes is a conversion out of it, so our own surfaces keep the color in
//! that space until the last inch. `fit` narrows the chroma to what sRGB can
//! actually show, which is the same narrowing `hexcode` does when the palette
//! is spent, so a shape and the stylesheet beside it cannot disagree about what
//! a color was.
//!
//! **The buffer holds premultiplied native ARGB.** The rasterizer borrows it
//! through the raster canvas, which arranges incoming colors for that buffer.
//! Images arrive with straight alpha and are premultiplied once before they
//! are composited.

use console_core_color::{Oklch, fit, oklch_to_rgb};
use console_core_geometry::{Point, Size, Triangle};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, narrow_f32, whole_u8};
use console_core_shapes::{
  Cropped, Edge, Line, Panel, Picture, PictureGeometry, Pixels, Round, Shape, Text, Weight,
};

use console_core_path_rasterizer::{
  Color, ColorU8, FillRule, FilterQuality, FloatRectangle, LineCap, Paint, Path, PathBuilder,
  Pattern, Pixmap, SpreadMode, Stroke, Transform,
};

use raster::Canvas;

mod opaque_frame;
pub mod painted;
pub mod raster;
pub mod text;

pub use console_core_shapes::Font;
pub use painted::{Rendered, painter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
  pub device: Size<u32>,
  pub points: Size<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Run<'a> {
  pub said: &'a str,
  pub weight: Weight,
  pub width: u32,
}

#[derive(Debug)]
pub enum Cannot {
  Buffer {
    device: Size<u32>,
    bytes: u64,
  },
  Transform,
  Mask,
  TextBusy,
  Pixels {
    size: Size<u32>,
    stride: u32,
    bytes: u64,
  },
}

impl std::fmt::Display for Cannot {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Cannot::Buffer { device, bytes } => write!(
        to,
        "{bytes} bytes cannot hold a {} by {} frame",
        device.width, device.height
      ),
      Cannot::Transform => write!(to, "the frame scale must be finite and positive"),
      Cannot::Mask => write!(to, "no clipping mask fits the frame"),
      Cannot::TextBusy => write!(to, "the drawing thread is already using its text cache"),
      Cannot::Pixels {
        size,
        stride,
        bytes,
      } => write!(
        to,
        "{bytes} bytes at stride {stride} cannot hold a {} by {} picture",
        size.width, size.height
      ),
    }
  }
}

impl std::error::Error for Cannot {}

pub fn measure_text(run: Run<'_>, font: &Font) -> Result<Size<u32>, Never> {
  let Ok(layout) = text::layout(run.said, font, run.weight, run.width);

  layout.measured()
}

pub fn wrapped(run: Run<'_>, font: &Font) -> Result<Vec<String>, Never> {
  let Ok(layout) = text::layout(run.said, font, run.weight, run.width);

  layout.wrapped()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lines(pub u32);

pub const CUT: char = '\u{2026}';

pub fn at_most(run: Run<'_>, font: &Font, most: Lines) -> Result<String, Never> {
  let Ok(lines) = wrapped(run, font);
  let Ok(many) = fitted::<_, u32>(lines.len());

  let before = match (many > most.0, most.0.checked_sub(1)) {
    (true, Some(before)) => before,
    (false, _) | (true, None) => return Ok(run.said.to_string()),
  };

  let Ok(at) = index(before);
  let mut said: Vec<String> = lines.iter().take(at).cloned().collect();

  let last = match lines.get(at) {
    Some(last) => last.trim_end(),
    None => "",
  };

  let shortened = match (last.rsplit_once(' '), last.char_indices().rev().nth(1)) {
    (Some((before_2, _word)), _) => before_2.trim_end(),
    (None, Some((cut, _))) => match last.get(..cut) {
      Some(kept) => kept,
      None => "",
    },
    (None, None) => "",
  };

  said.push(format!("{shortened}{CUT}"));

  Ok(said.join("\n"))
}

enum Ground<'a> {
  Cleared,
  Preserved,
  Picture(Picture<&'a Pixels>),
}

pub fn onto(pixels: &mut [u8], frame: Frame, shapes: &[Shape]) -> Result<(), Cannot> {
  paint(pixels, frame, shapes, Ground::Cleared, Point::default())
}

pub fn onto_region(
  pixels: &mut [u8],
  frame: Frame,
  shapes: &[Shape],
  origin: Point<i32>,
) -> Result<(), Cannot> {
  paint(pixels, frame, shapes, Ground::Cleared, origin)
}

pub fn over(pixels: &mut [u8], frame: Frame, shapes: &[Shape]) -> Result<(), Cannot> {
  paint(pixels, frame, shapes, Ground::Preserved, Point::default())
}

pub fn over_picture(
  pixels: &mut [u8],
  frame: Frame,
  picture: Picture<&Pixels>,
  above: &[Shape],
) -> Result<(), Cannot> {
  paint(
    pixels,
    frame,
    above,
    Ground::Picture(picture),
    Point::default(),
  )
}

fn paint(
  pixels: &mut [u8],
  frame: Frame,
  shapes: &[Shape],
  ground: Ground<'_>,
  origin: Point<i32>,
) -> Result<(), Cannot> {
  let Ok(ratio) = ratio(frame);
  let Ok(across) = narrow_f32(ratio.width);
  let Ok(down) = narrow_f32(ratio.height);
  let Ok(scale) =
    Transform::from_scale_factors(console_core_paths::ScaleFactors { x: across, y: down });
  let Ok(left) = narrow_f32(-f64::from(origin.x));
  let Ok(top) = narrow_f32(-f64::from(origin.y));
  let Ok(transform) = scale.pre_translate_by(console_core_paths::Point { x: left, y: top });
  let mut canvas = Canvas::new(pixels, frame.device, transform)?;

  let remaining_shapes = match ground {
    Ground::Cleared => {
      let Ok(remaining) = clear_ground(&mut canvas, frame, shapes, origin);
      remaining
    }
    Ground::Preserved => shapes,
    Ground::Picture(picture) => {
      let Ok(painted) = opaque_frame::paint(&mut canvas, frame, &picture);

      match painted {
        opaque_frame::OpaquePaint::Painted => {}
        opaque_frame::OpaquePaint::Fallback => draw_picture(&mut canvas, picture)?,
      }

      shapes
    }
  };

  for shape in remaining_shapes {
    match shape {
      Shape::Panel(panel) => draw_panel(&mut canvas, panel)?,
      Shape::Text(words) => draw_text(&mut canvas, words)?,
      Shape::Picture(picture) => {
        let Ok(borrowed) = picture.borrow_pixels();
        draw_picture(&mut canvas, borrowed)?;
      }
      Shape::Cropped(crop) => {
        let Ok(picture) = crop.picture.borrow_pixels();
        cropped(
          &mut canvas,
          Cropped {
            picture,
            from: crop.from,
            seen: crop.seen,
          },
        )?;
      }
      Shape::Line(line) => {
        let Ok(()) = draw_line(&mut canvas, line);
      }
      Shape::Clip(clip) => canvas.set_clip(*clip)?,
    }
  }

  Ok(())
}

fn clear_ground<'a>(
  canvas: &mut Canvas<'_>,
  frame: Frame,
  shapes: &'a [Shape],
  origin: Point<i32>,
) -> Result<&'a [Shape], Never> {
  let (fill, remaining) = match shapes.split_first() {
    Some((
      Shape::Panel(Panel {
        at,
        size,
        round: Round(0),
        fill,
        edge: Edge::None,
      }),
      rest,
    )) => {
      match (origin == Point::default()
        && *at == origin
        && *size == frame.points
        && size.width != 0
        && size.height != 0)
        .then_some(())
      {
        Some(()) => {
          let Ok(ink) = color(*fill);
          let Ok(pixel) = raster::buffer_color(ink);
          (pixel, rest)
        }
        None => (Color::TRANSPARENT, shapes),
      }
    }
    Some(_) | None => (Color::TRANSPARENT, shapes),
  };
  let Ok(()) = canvas.pixels.fill(fill);

  Ok(remaining)
}

fn ratio(frame: Frame) -> Result<Size<f64>, Never> {
  let wide = match frame.points.width {
    0 => 1.0,
    many => f64::from(frame.device.width) / f64::from(many),
  };
  let tall = match frame.points.height {
    0 => 1.0,
    many => f64::from(frame.device.height) / f64::from(many),
  };

  Ok(Size {
    width: wide,
    height: tall,
  })
}

fn draw_line(canvas: &mut Canvas<'_>, line: &Line) -> Result<(), Never> {
  let Ok(color) = color(line.color);
  let Ok(width) = narrow_f32(f64::from(line.width));
  let Ok(from_x) = narrow_f32(f64::from(line.from.x));
  let Ok(from_y) = narrow_f32(f64::from(line.from.y));
  let Ok(to_x) = narrow_f32(f64::from(line.to.x));
  let Ok(to_y) = narrow_f32(f64::from(line.to.y));
  let Ok(mut path) = PathBuilder::new();

  let ::core::result::Result::Ok(()) = ::console_core_paths::PathBuilder::move_to_pt(
    &mut path,
    ::console_core_paths::Point {
      x: from_x,
      y: from_y,
    },
  );
  let ::core::result::Result::Ok(()) = ::console_core_paths::PathBuilder::line_to_pt(
    &mut path,
    ::console_core_paths::Point { x: to_x, y: to_y },
  );

  let Ok(finish) = path.finish();

  match finish {
    Some(path_2) => {
      let mut paint = Paint::default();
      let Ok(color_2) = raster::buffer_color(color);
      let ::core::result::Result::Ok(()) = paint.set_color(color_2);
      let ::core::result::Result::Ok(()) = canvas.pixels.stroke_path(
        &path_2,
        &paint,
        &Stroke {
          width,
          line_cap: LineCap::Round,
          ..Stroke::default()
        },
        canvas.transform,
        canvas.clip.as_ref().map(|clip| {
          let Ok(mask) = clip.mask();

          mask
        }),
      );
    }
    None => {}
  }

  Ok(())
}

fn draw_panel(canvas: &mut Canvas<'_>, panel: &Panel) -> Result<(), Cannot> {
  let Ok(fill) = color(panel.fill);
  let Ok(()) = panel_path(canvas, panel, fill, None);

  match panel.edge {
    Edge::None => {}
    Edge::Of { wide, color: ink } => {
      let Ok(ink_2) = color(ink);
      let Ok(width) = narrow_f32(f64::from(wide));
      let Ok(()) = panel_path(canvas, panel, ink_2, Some(width));
    }
  }

  Ok(())
}

fn panel_path(
  canvas: &mut Canvas<'_>,
  panel: &Panel,
  color: Color,
  stroke: Option<f32>,
) -> Result<(), Never> {
  let inset = stroke.map_or(0.0, |width| width / 2.0);
  let Ok(left) = narrow_f32(f64::from(panel.at.x));
  let Ok(top) = narrow_f32(f64::from(panel.at.y));
  let Ok(width) = narrow_f32(f64::from(panel.size.width));
  let Ok(height) = narrow_f32(f64::from(panel.size.height));
  let Round(round) = panel.round;
  let Ok(radius) = narrow_f32(f64::from(round));
  let rect = match {
    let Ok(explicit_value) = FloatRectangle::from_xywh(
      left + inset,
      top + inset,
      width - inset * 2.0,
      height - inset * 2.0,
    );
    explicit_value
  } {
    Some(rect) => rect,
    None => return Ok(()),
  };
  let Ok(path) = raster::rounded(rect, radius);
  let path_3 = match path {
    Some(path_2) => path_2,
    None => return Ok(()),
  };
  let mut paint = Paint::default();

  let Ok(color_2) = raster::buffer_color(color);
  let ::core::result::Result::Ok(()) = paint.set_color(color_2);

  let Ok(()) = match stroke {
    Some(width_2) => canvas.pixels.stroke_path(
      &path_3,
      &paint,
      &Stroke {
        width: width_2,
        ..Stroke::default()
      },
      canvas.transform,
      canvas.clip.as_ref().map(|clip| {
        let Ok(mask) = clip.mask();

        mask
      }),
    ),
    None => canvas.fill_path(&path_3, &paint, FillRule::Winding),
  };

  Ok(())
}

fn draw_text(canvas: &mut Canvas<'_>, words: &Text) -> Result<(), Cannot> {
  let Ok(ink) = color(words.ink);

  text::paint(canvas, words, ink)
}

fn premultiplied(pixels: &Pixels) -> Result<Pixmap, Cannot> {
  let Ok(size) = pixels.size();
  let Ok(bytes) = fitted::<_, u64>(pixels.bytes.len());
  let fault = || Cannot::Pixels {
    size,
    stride: pixels.stride,
    bytes,
  };
  let Ok(rows) = pixels.rows();
  let rows_2 = rows.ok_or_else(fault)?;
  let Ok(pixmap) = Pixmap::new(pixels.width, pixels.height);
  let mut image = pixmap.ok_or_else(fault)?;
  let Ok(width) = index(pixels.width);
  let Ok(target) = image.data_mut();

  for (source, target_2) in rows_2.zip(target.as_chunks_mut::<4>().0.chunks_mut(width.max(1))) {
    for (&[red, green, blue, alpha], target_3) in source.iter().zip(target_2) {
      let Ok(color_u8) = ColorU8::from_rgba(red, green, blue, alpha);
      let Ok(pixel) = raster::buffer_pixel(color_u8);

      *target_3 = pixel;
    }
  }

  Ok(image)
}

fn draw_picture(canvas: &mut Canvas<'_>, picture: Picture<&Pixels>) -> Result<(), Cannot> {
  let Ok(seen) = picture.pixels.size();
  let Ok(seen_2) = seen.map(f64::from);
  cropped(
    canvas,
    Cropped {
      picture,
      from: Point::default(),
      seen: seen_2,
    },
  )
}

fn cropped(canvas: &mut Canvas<'_>, crop: Cropped<&Pixels>) -> Result<(), Cannot> {
  let Cropped {
    picture,
    from,
    seen,
  } = crop;

  match ::core::primitive::bool::then_some(
    picture.pixels.width > 0
      && picture.pixels.height > 0
      && seen.width > 0.0
      && seen.height > 0.0,
    (),
  ) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(()),
  }

  let Ok(bounds) = picture.bounds();
  let Ok(size) = bounds.size();

  match (bounds.x0.is_finite()
    && bounds.y0.is_finite()
    && size.width.is_finite()
    && size.height.is_finite())
  .then_some(())
  {
    Some(()) => {}
    None => return Ok(()),
  }

  let Ok(x) = narrow_f32(bounds.x0);
  let Ok(y) = narrow_f32(bounds.y0);
  let Ok(width) = narrow_f32(size.width);
  let Ok(height) = narrow_f32(size.height);
  let Ok(destination) = FloatRectangle::from_xywh(x, y, width, height);
  let rect = match destination {
    Some(rect) => rect,
    None => return Ok(()),
  };
  let image = premultiplied(picture.pixels)?;
  let Ok(across) = narrow_f32(size.width / seen.width);
  let Ok(down) = narrow_f32(size.height / seen.height);
  let Ok(from_x) = narrow_f32(from.x);
  let Ok(from_y) = narrow_f32(from.y);
  let Ok(translation) = Transform::from_translation(console_core_paths::Point { x, y });
  let Ok(scaled) =
    translation.pre_scale_by(console_core_paths::ScaleFactors { x: across, y: down });
  let Ok(transform) = scaled.pre_translate_by(console_core_paths::Point {
    x: -from_x,
    y: -from_y,
  });
  let Ok(shader) = Pattern::new(
    {
      let Ok(explicit_value) = image.as_ref();
      explicit_value
    },
    SpreadMode::Pad,
    FilterQuality::Bilinear,
    1.0,
    transform,
  );
  let paint = Paint {
    shader,
    ..Paint::default()
  };

  let Ok(()) = fill_picture(canvas, picture.geometry, rect, &paint);
  Ok(())
}

fn fill_picture(
  canvas: &mut Canvas<'_>,
  geometry: PictureGeometry,
  rect: FloatRectangle,
  paint: &Paint<'_>,
) -> Result<(), Never> {
  let Ok(path) = match geometry {
    PictureGeometry::Rectangle(_) => {
      return canvas.pixels.fill_rect(
        rect,
        paint,
        canvas.transform,
        canvas.clip.as_ref().map(|clip| {
          let Ok(mask) = clip.mask();

          mask
        }),
      );
    }
    PictureGeometry::Circle(_) => PathBuilder::from_oval(rect),
    PictureGeometry::Triangle(triangle) => triangle_path(triangle),
  };

  match path {
    Some(path_2) => canvas.fill_path(&path_2, paint, FillRule::Winding),
    None => Ok(()),
  }
}

fn triangle_path(triangle: Triangle<f64>) -> Result<Option<Path>, Never> {
  match triangle
    .vertices
    .iter()
    .all(|point| point.x.is_finite() && point.y.is_finite())
    .then_some(())
  {
    Some(()) => {}
    None => return Ok(None),
  }

  let [start, second, third] = triangle.vertices.map(|point| {
    let Ok(x) = narrow_f32(point.x);
    let Ok(y) = narrow_f32(point.y);
    console_core_paths::Point { x, y }
  });
  let Ok(mut builder) = PathBuilder::new();
  let Ok(()) = builder.move_to_pt(start);
  let Ok(()) = builder.line_to_pt(second);
  let Ok(()) = builder.line_to_pt(third);
  let Ok(()) = builder.close();
  builder.finish()
}

fn color(color: Oklch) -> Result<Color, Never> {
  let Ok(narrowed) = fit(color);
  let Ok(within) = color.with(narrowed);
  let Ok([red, green, blue]) = oklch_to_rgb(within);
  let Ok(red_2) = whole_u8(red.clamp(0.0, 1.0) * 255.0);
  let Ok(green_2) = whole_u8(green.clamp(0.0, 1.0) * 255.0);
  let Ok(blue_2) = whole_u8(blue.clamp(0.0, 1.0) * 255.0);

  Ok({
    let Ok(explicit_value) = Color::from_rgba8(red_2, green_2, blue_2, 255);
    explicit_value
  })
}
