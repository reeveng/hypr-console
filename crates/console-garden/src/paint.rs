//! What the brush is dipped in, and the one shape everything is drawn with.
//!
//! There is not one colour written down in this crate. Every one of them comes
//! out of `theme/palette.toml` like every other surface on the machine, and
//! what is here is shapes.

use std::collections::HashMap;

use cairo::{Context, Gradient};

use crate::fault::{Drawing, Fault};

/// A colour and how much of it reaches the picture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wash {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl Wash {
    /// Six hex digits and an alpha, as a brush takes them.
    ///
    /// A code that is not six hex digits is a palette written wrong, and it
    /// says so rather than painting the channel black. Black is a colour
    /// somebody could have meant, so swallowing it here would come out as a
    /// picture that looks nearly right and nothing saying why.
    pub fn of(code: &str, alpha: f64) -> Result<Self, String> {
        let channel = |at: usize| -> Result<f64, String> {
            let pair = code
                .get(at..at + 2)
                .ok_or_else(|| format!("{code} is not six hex digits"))?;
            let value = u8::from_str_radix(pair, 16)
                .map_err(|_| format!("{code} is not six hex digits"))?;

            Ok(f64::from(value) / 255.0)
        };

        Ok(Wash {
            red: channel(0)?,
            green: channel(2)?,
            blue: channel(4)?,
            alpha,
        })
    }

    /// This laid over that, worked out rather than laid down twice.
    ///
    /// A gradient stop cannot be half-transparent and still be the end of a
    /// solid shape: it would show what the shape is standing in front of. So
    /// where a wash has to appear inside a gradient it is resolved into the
    /// colour it would have become.
    pub fn over(self, under: Wash, share: f64) -> Self {
        let mix = |top: f64, bottom: f64| top * share + bottom * (1.0 - share);
        Wash {
            red: mix(self.red, under.red),
            green: mix(self.green, under.green),
            blue: mix(self.blue, under.blue),
            alpha: 1.0,
        }
    }
}

/// The garden's table, as colours a brush can be dipped in.
#[derive(Debug, Clone, Default)]
pub struct Paints(HashMap<String, Wash>);

impl Paints {
    pub fn of(said: impl IntoIterator<Item = (String, Wash)>) -> Self {
        Paints(said.into_iter().collect())
    }

    pub fn get(&self, name: &str) -> Drawing<Wash> {
        self.0
            .get(name)
            .copied()
            .ok_or_else(|| Fault::Paint(name.to_string()))
    }

    /// One paint laid over another, by the top one's own alpha unless told.
    pub fn washed(&self, over: &str, under: &str, share: Option<f64>) -> Drawing<Wash> {
        let top = self.get(over)?;
        Ok(top.over(self.get(under)?, share.unwrap_or(top.alpha)))
    }
}

/// Dip the brush.
pub fn dip(ctx: &Context, paint: &Paints, name: &str, alpha: f64) -> Drawing<()> {
    let wash = paint.get(name)?;
    ctx.set_source_rgba(wash.red, wash.green, wash.blue, wash.alpha * alpha);
    Ok(())
}

/// One stop of a gradient, in a paint from the table.
pub fn stop(gradient: &Gradient, offset: f64, paint: &Paints, name: &str, alpha: f64) -> Drawing<()> {
    let wash = paint.get(name)?;
    gradient.add_color_stop_rgba(offset, wash.red, wash.green, wash.blue, wash.alpha * alpha);
    Ok(())
}

/// A stop in a colour already worked out.
pub fn stop_wash(gradient: &Gradient, offset: f64, wash: Wash) {
    gradient.add_color_stop_rgba(offset, wash.red, wash.green, wash.blue, wash.alpha);
}

/// A curve, said as the three points it is made of.
///
/// Cairo takes six numbers. A curve is not six numbers, it is two handles and
/// somewhere to end up, and written flat the three points come apart on the
/// first line that is too long.
pub fn curve(ctx: &Context, one: (f64, f64), two: (f64, f64), three: (f64, f64)) {
    ctx.curve_to(one.0, one.1, two.0, two.1, three.0, three.1);
}

/// One blossom, wherever it is: on the tree, in the wind, or on the path.
///
/// Round at the open end and pointed where it was joined on. A blossom drawn
/// as a circle reads as a speck and a blossom drawn as an ellipse reads as a
/// grain of rice, and at this size the difference between those and a petal is
/// two curves.
pub fn petal_at(ctx: &Context, x: f64, y: f64, size: f64, turn: f64) -> Drawing<()> {
    ctx.save()?;
    ctx.translate(x, y);
    ctx.rotate(turn);
    ctx.move_to(0.0, -size);
    curve(ctx, (size * 0.88, -size * 0.34), (size * 0.80, size * 0.78), (0.0, size));
    curve(ctx, (-size * 0.80, size * 0.78), (-size * 0.88, -size * 0.34), (0.0, -size));
    ctx.close_path();
    ctx.fill()?;
    ctx.restore()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_is_read_as_six_hex_digits() {
        let wash = Wash::of("ff8040", 0.5).expect("a colour");
        assert_eq!(wash.red, 1.0);
        assert!((wash.green - 128.0 / 255.0).abs() < 1e-12);
        assert!((wash.blue - 64.0 / 255.0).abs() < 1e-12);
        assert_eq!(wash.alpha, 0.5);
    }

    #[test]
    fn a_wash_worked_out_is_opaque() {
        // The whole reason it is worked out: a gradient stop that is partly
        // transparent shows what the shape is standing in front of.
        let top = Wash::of("ffffff", 0.25).expect("a colour");
        let over = top.over(Wash::of("000000", 1.0).expect("a colour"), 0.25);
        assert_eq!(over.alpha, 1.0);
        assert_eq!(over.red, 0.25);
    }

    #[test]
    fn a_wash_takes_its_own_alpha_unless_told_otherwise() {
        let paints = Paints::of([
            ("mist".to_string(), Wash::of("ffffff", 0.5).expect("a colour")),
            ("earth".to_string(), Wash::of("000000", 1.0).expect("a colour")),
        ]);
        assert_eq!(paints.washed("mist", "earth", None).expect("both named").red, 0.5);
        assert_eq!(
            paints.washed("mist", "earth", Some(0.1)).expect("both named").red,
            0.1
        );
    }

    #[test]
    fn dipping_in_a_colour_the_palette_does_not_name_says_which_one() {
        // The name has to reach the reader. A drawing that stops without
        // saying which colour was missing sends somebody through the whole
        // crate looking for it.
        let paints = Paints::default();
        let fault = paints.get("nothing").expect_err("no such paint");
        assert!(matches!(&fault, Fault::Paint(name) if name == "nothing"), "{fault:?}");
        assert!(fault.to_string().contains("nothing"), "{fault}");
    }
}
