//! Shapes, put into a buffer somebody else owns.
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
//! screen -- whether the colour that came out is the colour the palette spends
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
//! **A colour arrives as `Oklch` and becomes channels here.** That is the whole
//! reason this crate exists as something separate from the shape list: the
//! palette is decided in a perceptual space and every other file this desktop
//! writes is a conversion out of it, so our own surfaces keep the colour in
//! that space until the last inch. `fit` narrows the chroma to what sRGB can
//! actually show, which is the same narrowing `hexcode` does when the palette
//! is spent, so a shape and the stylesheet beside it cannot disagree about what
//! a colour was.
//!
//! **Every colour here is opaque and the buffer agrees.** Cairo's `ARgb32`
//! wants premultiplied alpha and `wl_shm`'s `Argb8888` is the same bytes in the
//! same order on a little-endian machine, so the two agree for free while
//! nothing is translucent. The first shape drawn at less than full alpha is the
//! one that has to multiply, and it will be wrong in a way that looks like a
//! colour rather than like a fault.

use console_core_colour::{Oklch, fit, oklch_to_rgb};
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_i32, whole_u32};
use console_core_shapes::{Edge, Panel, Round, Shape, Weight, Words};

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
    pub wide: u32,
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
    let Ok(wide) = fitted::<u32, i32>(run.wide);

    layout.set_font_description(Some(&described));
    layout.set_wrap(pango::WrapMode::WordChar);
    layout.set_width(wide.saturating_mul(pango::SCALE));
    layout.set_text(run.said);

    let (across, down) = layout.pixel_size();
    let Ok(wide) = whole_u32(f64::from(across));
    let Ok(tall) = whole_u32(f64::from(down));

    Ok(Size { wide, tall })
}

pub fn onto(pixels: &mut [u8], frame: Frame, shapes: &[Shape]) -> Result<(), Cannot> {
    let Ok(stride) = fitted::<u32, i32>(frame.device.wide.saturating_mul(4));
    let Ok(wide) = fitted::<u32, i32>(frame.device.wide);
    let Ok(tall) = fitted::<u32, i32>(frame.device.tall);

    // SAFETY: cairo writes into `pixels` only while this borrow is alive, and
    // the surface is finished before the borrow ends.
    let image = unsafe {
        ImageSurface::create_for_data_unsafe(pixels.as_mut_ptr(), Format::ARgb32, wide, tall, stride)
    }
    .map_err(Cannot::Surface)?;
    let cairo = Context::new(&image).map_err(Cannot::Context)?;
    let Ok(ratio) = ratio(frame);

    cairo.set_operator(cairo::Operator::Source);
    cairo.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    cairo.paint().map_err(Cannot::Drawing)?;
    cairo.set_operator(cairo::Operator::Over);
    cairo.scale(ratio.wide, ratio.tall);

    for shape in shapes {
        match shape {
            Shape::Panel(panel) => panelled(&cairo, panel)?,
            Shape::Words(words) => written(&cairo, words)?,
        }
    }

    image.finish();

    Ok(())
}

fn ratio(frame: Frame) -> Result<Size<f64>, Never> {
    let wide = match frame.points.wide {
        0 => 1.0,
        many => f64::from(frame.device.wide) / f64::from(many),
    };
    let tall = match frame.points.tall {
        0 => 1.0,
        many => f64::from(frame.device.tall) / f64::from(many),
    };

    Ok(Size { wide, tall })
}

fn described(font: &Font, weight: Weight) -> Result<FontDescription, Never> {
    let mut described = FontDescription::new();

    described.set_family(&font.family);
    let Ok(tall) = fitted::<u32, i32>(font.tall);

    described.set_absolute_size(f64::from(tall.saturating_mul(pango::SCALE)));
    described.set_weight(match weight {
        Weight::Plain => pango::Weight::Normal,
        Weight::Bold => pango::Weight::Bold,
    });

    Ok(described)
}

fn panelled(cairo: &Context, panel: &Panel) -> Result<(), Cannot> {
    let Ok(()) = traced(cairo, panel, Inset::None);
    let Ok(()) = sourced(cairo, panel.fill);

    cairo.fill().map_err(Cannot::Drawing)?;

    match panel.edge {
        Edge::None => {}
        Edge::Of { wide, colour } => {
            let half = f64::from(wide) / 2.0;
            let Ok(()) = traced(cairo, panel, Inset::By(half));
            let Ok(()) = sourced(cairo, colour);

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
    let left = f64::from(panel.at.across) + by;
    let top = f64::from(panel.at.down) + by;
    let wide = (f64::from(panel.size.wide) - by * 2.0).max(0.0);
    let tall = (f64::from(panel.size.tall) - by * 2.0).max(0.0);
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

fn written(cairo: &Context, words: &Words) -> Result<(), Cannot> {
    let Ok(()) = sourced(cairo, words.ink);
    let Ok(described) = described(&words.font, words.weight);
    let layout = pangocairo::functions::create_layout(cairo);
    let Ok(()) = unhinted(&layout.context());
    let Ok(wide) = fitted::<u32, i32>(words.wide);

    layout.set_font_description(Some(&described));
    layout.set_wrap(pango::WrapMode::WordChar);
    layout.set_width(wide.saturating_mul(pango::SCALE));
    layout.set_text(&words.said);

    let Ok(across) = toward_zero_i32(f64::from(words.at.across));
    let Ok(down) = toward_zero_i32(f64::from(words.at.down));

    cairo.move_to(f64::from(across), f64::from(down));
    pangocairo::functions::show_layout(cairo, &layout);

    Ok(())
}

fn sourced(cairo: &Context, colour: Oklch) -> Result<(), Never> {
    let Ok(narrowed) = fit(colour);
    let Ok(within) = colour.with(narrowed);
    let Ok([red, green, blue]) = oklch_to_rgb(within);

    cairo.set_source_rgb(red.clamp(0.0, 1.0), green.clamp(0.0, 1.0), blue.clamp(0.0, 1.0));

    Ok(())
}
