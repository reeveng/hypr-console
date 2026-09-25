//! Shapes, put into a buffer someone else owns.
//!
//! **Everything here is in points, and the buffer is in pixels.** A surface is
//! handed out at device resolution and every number in this tree is logical, so
//! the one place the two meet is the transform set on the context before the
//! first shape is drawn. [`Frame`] carries both sizes with their names on, so
//! there is no order to get wrong and no scale passed about as a bare ratio.
//! After that line every coordinate a caller wrote is the coordinate it meant,
//! and how a corner lands on a half pixel is cairo's problem rather than
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
//! Pango how tall a run wraps to, with no surface in the room, and hands back a
//! size. Placement is arithmetic over that answer somewhere else. The order
//! matters: a placement that measured as it went would be a placement that
//! could only be checked where there is a font, and the whole reason the shape
//! list is pure is so that it can be checked where there is not.
//!
//! **A page of a book is lines, so where a run breaks is said as well.**
//! [`wrapped`] is the same question as [`measured`] with the answer kept in
//! pieces: the text of each line Pango broke the run into, at the width it was
//! given. A reader turning pages has to know which line is the last one that
//! fits, and the only thing that knows where a paragraph breaks is the thing
//! that shapes it -- a count of letters is wrong in every proportional face and
//! in every script without spaces.
//!
//! **Both of them shape without hinting, and that is not a preference.**
//! Measuring happens on a context with no surface under it and drawing happens
//! on one scaled by the compositor's own factor, and cairo hints glyph
//! advances to whole *device* pixels. So the same string in the same face is a
//! different width at 1x and at 2x, and a run placed at the width it measured
//! is a run given slightly less room than it turns out to need -- which shows
//! up as a word wrapping onto a second line inside a slab that was made to fit
//! one, which is how it was found. Hint metrics off is the only setting under
//! which a measurement taken with no screen is true on every screen, which is
//! the whole claim the split is making.
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
//! **Every color here is opaque and the buffer agrees.** Cairo's `ARgb32`
//! wants premultiplied alpha and `wl_shm`'s `Argb8888` is the same bytes in the
//! same order on a little-endian machine, so the two agree for free while
//! nothing is translucent. The first shape drawn at less than full alpha is the
//! one that has to multiply, and it will be wrong in a way that looks like a
//! color rather than like a fault.

use console_core_color::{Oklch, fit, oklch_to_rgb};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_i32, whole_u32};
use console_core_shapes::{Clip, Cropped, Edge, Line, Panel, Picture, Pixels, Round, Shape, Weight, Text};

use cairo::{Context, Format, ImageSurface};
use pango::FontDescription;
use pango::prelude::FontMapExt;

pub use console_core_shapes::Font;

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
    Surface(cairo::Error),
    Context(cairo::Error),
    Drawing(cairo::Error),
}

impl std::fmt::Display for Cannot {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cannot::Surface(why) => write!(to, "no surface over the frame: {why}"),
            Cannot::Context(why) => write!(to, "nothing to draw the frame with: {why}"),
            Cannot::Drawing(why) => write!(to, "a shape would not draw: {why}"),
        }
    }
}

impl std::error::Error for Cannot {}

fn unhinted(context: &pango::Context) -> Result<(), Never> {
    let mut options = match cairo::FontOptions::new() {
        Ok(options) => options,
        Err(_cairo_would_not_make_one_and_the_default_shaping_stands) => return Ok(()),
    };

    options.set_hint_metrics(cairo::HintMetrics::Off);
    pangocairo::functions::context_set_font_options(context, Some(&options));

    Ok(())
}

pub fn measured(run: Run<'_>, font: &Font) -> Result<Size<u32>, Never> {
    let context = pangocairo::FontMap::default().create_context();
    let Ok(()) = unhinted(&context);
    let layout = pango::Layout::new(&context);
    let Ok(described) = described(font, run.weight);
    let Ok(wide) = fitted::<u32, i32>(run.width);

    layout.set_font_description(Some(&described));
    layout.set_wrap(pango::WrapMode::WordChar);
    layout.set_width(wide.saturating_mul(pango::SCALE));
    layout.set_text(run.said);

    let (across, down) = layout.pixel_size();
    let Ok(wide) = whole_u32(f64::from(across));
    let Ok(tall) = whole_u32(f64::from(down));

    Ok(Size { width: wide, height: tall })
}

pub fn wrapped(run: Run<'_>, font: &Font) -> Result<Vec<String>, Never> {
    let context = pangocairo::FontMap::default().create_context();
    let Ok(()) = unhinted(&context);
    let layout = pango::Layout::new(&context);
    let Ok(described) = described(font, run.weight);
    let Ok(wide) = fitted::<u32, i32>(run.width);

    layout.set_font_description(Some(&described));
    layout.set_wrap(pango::WrapMode::WordChar);
    layout.set_width(wide.saturating_mul(pango::SCALE));
    layout.set_text(run.said);

    let mut lines: Vec<String> = Vec::new();

    for line in layout.lines_readonly() {
        let Ok(from) = fitted::<i32, u32>(line.start_index());
        let Ok(long) = fitted::<i32, u32>(line.length());
        let Ok(start) = index(from);
        let Ok(past) = index(from.saturating_add(long));

        match run.said.get(start..past) {
            Some(said) => lines.push(said.trim_end().to_string()),
            None => lines.push(String::new()),
        }
    }

    Ok(lines)
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
        (Some((before, _word)), _) => before.trim_end(),
        (None, Some((cut, _))) => match last.get(..cut) {
            Some(kept) => kept,
            None => "",
        },
        (None, None) => "",
    };

    said.push(format!("{shortened}{CUT}"));

    Ok(said.join("\n"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ground {
    Cleared,
    Preserved,
}

pub fn onto(pixels: &mut [u8], frame: Frame, shapes: &[Shape]) -> Result<(), Cannot> {
    painted(pixels, frame, shapes, Ground::Cleared)
}

pub fn over(pixels: &mut [u8], frame: Frame, shapes: &[Shape]) -> Result<(), Cannot> {
    painted(pixels, frame, shapes, Ground::Preserved)
}

fn painted(pixels: &mut [u8], frame: Frame, shapes: &[Shape], ground: Ground) -> Result<(), Cannot> {
    let Ok(stride) = fitted::<u32, i32>(frame.device.width.saturating_mul(4));
    let Ok(wide) = fitted::<u32, i32>(frame.device.width);
    let Ok(tall) = fitted::<u32, i32>(frame.device.height);

    // SAFETY: cairo writes into `pixels` only while this borrow is alive, and
    // the surface is finished before the borrow ends.
    let image = unsafe {
        ImageSurface::create_for_data_unsafe(pixels.as_mut_ptr(), Format::ARgb32, wide, tall, stride)
    }
    .map_err(Cannot::Surface)?;
    let cairo = Context::new(&image).map_err(Cannot::Context)?;
    let Ok(ratio) = ratio(frame);

    match ground {
        Ground::Cleared => {
            cairo.set_operator(cairo::Operator::Source);
            cairo.set_source_rgba(0.0, 0.0, 0.0, 0.0);
            cairo.paint().map_err(Cannot::Drawing)?;
        },
        Ground::Preserved => {},
    }

    cairo.set_operator(cairo::Operator::Over);
    cairo.scale(ratio.width, ratio.height);

    for shape in shapes {
        match shape {
            Shape::Panel(panel) => panelled(&cairo, panel)?,
            Shape::Text(words) => written(&cairo, words)?,
            Shape::Picture(picture) => pictured(&cairo, picture)?,
            Shape::Cropped(picture) => cropped(&cairo, picture)?,
            Shape::Line(line) => drawn(&cairo, line)?,
            Shape::Clip(clip) => {
                let Ok(()) = clipped(&cairo, *clip);
            }
        }
    }

    image.finish();

    Ok(())
}

fn clipped(cairo: &Context, clip: Clip) -> Result<(), Never> {
    cairo.reset_clip();

    match clip {
        Clip::To { at, size } => {
            cairo.rectangle(f64::from(at.x), f64::from(at.y), f64::from(size.width), f64::from(size.height));
            cairo.clip();
        }
        Clip::Lifted => {},
    }

    Ok(())
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

    Ok(Size { width: wide, height: tall })
}

fn described(font: &Font, weight: Weight) -> Result<FontDescription, Never> {
    let mut described = FontDescription::new();

    described.set_family(&font.family);
    let Ok(tall) = fitted::<u32, i32>(font.height);

    described.set_absolute_size(f64::from(tall.saturating_mul(pango::SCALE)));
    described.set_weight(match weight {
        Weight::Plain => pango::Weight::Normal,
        Weight::Bold => pango::Weight::Bold,
    });

    Ok(described)
}

fn drawn(cairo: &Context, line: &Line) -> Result<(), Cannot> {
    let Ok(()) = sourced(cairo, line.color);

    cairo.set_line_width(f64::from(line.width));
    cairo.set_line_cap(cairo::LineCap::Round);
    cairo.move_to(f64::from(line.from.x), f64::from(line.from.y));
    cairo.line_to(f64::from(line.to.x), f64::from(line.to.y));
    cairo.stroke().map_err(Cannot::Drawing)?;

    Ok(())
}

fn panelled(cairo: &Context, panel: &Panel) -> Result<(), Cannot> {
    let Ok(()) = traced(cairo, panel, Inset::None);
    let Ok(()) = sourced(cairo, panel.fill);

    cairo.fill().map_err(Cannot::Drawing)?;

    match panel.edge {
        Edge::None => {}
        Edge::Of { wide, color } => {
            let half = f64::from(wide) / 2.0;
            let Ok(()) = traced(cairo, panel, Inset::By(half));
            let Ok(()) = sourced(cairo, color);

            cairo.set_line_width(f64::from(wide));
            cairo.stroke().map_err(Cannot::Drawing)?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Inset {
    By(f64),
    None,
}

fn traced(cairo: &Context, panel: &Panel, inset: Inset) -> Result<(), Never> {
    let by = match inset {
        Inset::By(by) => by,
        Inset::None => 0.0,
    };
    let left = f64::from(panel.at.x) + by;
    let top = f64::from(panel.at.y) + by;
    let wide = (f64::from(panel.size.width) - by * 2.0).max(0.0);
    let tall = (f64::from(panel.size.height) - by * 2.0).max(0.0);
    let Round(round) = panel.round;
    let most = wide.min(tall) / 2.0;
    let radius = f64::from(round).min(most);

    cairo.new_path();

    match radius <= 0.0 {
        true => {
            cairo.rectangle(left, top, wide, tall);

            return Ok(());
        }
        false => {},
    }

    let half = std::f64::consts::FRAC_PI_2;
    let whole = std::f64::consts::PI;

    cairo.new_sub_path();
    cairo.arc(left + wide - radius, top + radius, radius, -half, 0.0);
    cairo.arc(left + wide - radius, top + tall - radius, radius, 0.0, half);
    cairo.arc(left + radius, top + tall - radius, radius, half, whole);
    cairo.arc(left + radius, top + radius, radius, whole, whole + half);
    cairo.close_path();

    Ok(())
}

fn written(cairo: &Context, words: &Text) -> Result<(), Cannot> {
    let Ok(()) = sourced(cairo, words.ink);
    let Ok(described) = described(&words.font, words.weight);
    let layout = pangocairo::functions::create_layout(cairo);
    let Ok(()) = unhinted(&layout.context());
    let Ok(wide) = fitted::<u32, i32>(words.width);

    layout.set_font_description(Some(&described));
    layout.set_wrap(pango::WrapMode::WordChar);
    layout.set_width(wide.saturating_mul(pango::SCALE));
    layout.set_text(&words.said);

    let Ok(across) = toward_zero_i32(f64::from(words.at.x));
    let Ok(down) = toward_zero_i32(f64::from(words.at.y));

    cairo.move_to(f64::from(across), f64::from(down));
    pangocairo::functions::show_layout(cairo, &layout);

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Alpha(u8);

fn premultiplied(pixels: &Pixels) -> Result<Vec<u8>, Never> {
    let Ok(step) = index(pixels.stride);
    let across = pixels.width;
    let mut argb: Vec<u8> = Vec::new();

    for line in pixels.bytes.chunks(step) {
        let mut many = 0u32;

        'over_pixels: for pixel in line.chunks_exact(4) {
            match many < across {
                true => many = many.saturating_add(1),
                false => break 'over_pixels,
            }

            let ([red, green, blue], alpha) = match pixel {
                [red, green, blue, alpha] => ([*red, *green, *blue], Alpha(*alpha)),
                _shorter_than_a_pixel => break 'over_pixels,
            };

            let Ok(red) = lit(red, alpha);
            let Ok(green) = lit(green, alpha);
            let Ok(blue) = lit(blue, alpha);
            let Alpha(alpha) = alpha;

            argb.extend_from_slice(&u32::from_be_bytes([alpha, red, green, blue]).to_le_bytes());
        }

        for _short_of_the_width in many..across {
            argb.extend_from_slice(&0u32.to_le_bytes());
        }
    }

    Ok(argb)
}

fn lit(channel: u8, alpha: Alpha) -> Result<u8, Never> {
    const OPAQUE: u16 = 255;

    let Alpha(alpha) = alpha;

    let lit = u16::from(channel)
        .saturating_mul(u16::from(alpha))
        .saturating_add(OPAQUE.saturating_div(2))
        .saturating_div(OPAQUE);

    fitted(lit)
}

fn pictured(cairo: &Context, picture: &Picture) -> Result<(), Cannot> {
    cropped(
        cairo,
        &Cropped {
            at: picture.at,
            size: picture.size,
            pixels: picture.pixels.clone(),
            from: Point { x: 0.0, y: 0.0 },
            seen: Size { width: f64::from(picture.pixels.width), height: f64::from(picture.pixels.height) },
        },
    )
}

fn cropped(cairo: &Context, picture: &Cropped) -> Result<(), Cannot> {
    let Ok(wide) = fitted::<u32, i32>(picture.pixels.width);
    let Ok(tall) = fitted::<u32, i32>(picture.pixels.height);

    match wide > 0 && tall > 0 && picture.seen.width > 0.0 && picture.seen.height > 0.0 {
        true => {},
        false => return Ok(()),
    }

    let Ok(mut argb) = premultiplied(&picture.pixels);
    let Ok(stride) = fitted::<u32, i32>(picture.pixels.width.saturating_mul(4));

    let drawn = {
        // SAFETY: cairo reads `argb` only while this borrow is alive, and the
        // surface is finished before the borrow ends.
        let image = unsafe {
            ImageSurface::create_for_data_unsafe(argb.as_mut_ptr(), Format::ARgb32, wide, tall, stride)
        }
        .map_err(Cannot::Surface)?;

        let stretch = Size {
            width: f64::from(picture.size.width) / picture.seen.width,
            height: f64::from(picture.size.height) / picture.seen.height,
        };

        cairo.save().map_err(Cannot::Drawing)?;
        cairo.translate(f64::from(picture.at.x), f64::from(picture.at.y));
        cairo.scale(stretch.width, stretch.height);
        cairo.set_source_surface(&image, -picture.from.x, -picture.from.y).map_err(Cannot::Drawing)?;
        cairo.source().set_extend(cairo::Extend::Pad);
        cairo.rectangle(0.0, 0.0, picture.seen.width, picture.seen.height);
        cairo.clip();

        let drawn = cairo.paint();

        cairo.restore().map_err(Cannot::Drawing)?;
        image.finish();

        drawn
    };

    drawn.map_err(Cannot::Drawing)?;

    Ok(())
}

fn sourced(cairo: &Context, color: Oklch) -> Result<(), Never> {
    let Ok(narrowed) = fit(color);
    let Ok(within) = color.with(narrowed);
    let Ok([red, green, blue]) = oklch_to_rgb(within);

    cairo.set_source_rgb(red.clamp(0.0, 1.0), green.clamp(0.0, 1.0), blue.clamp(0.0, 1.0));

    Ok(())
}
