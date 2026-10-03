//! How wide each of the bar's words comes out, which is Pango's to say.
//!
//! `showing` places against widths and never asks for one, so that where a
//! slab lands can be asserted with no font in the room. This is the one call
//! that needs a font, and it is a module of its own because two bars are drawn
//! out of `showing` -- the desktop's, and the one over the way in -- and a width
//! asked two ways is two bars that disagree about where a thumb lands.

use console_core_geometry::Size;
use console_core_never::Never;
use console_draw_painting::{Run, measure_text};

use crate::showing::{Bar, Face, Fitting, Layout, Measured, Slot};

pub fn sized(layout: &Layout, fitting: Fitting) -> Result<Bar, Never> {
    let Ok(left) = every(&layout.left, fitting);
    let Ok(middle) = every(&layout.middle, fitting);
    let Ok(right) = every(&layout.right, fitting);

    Ok(Bar { left, middle, right, filling: layout.filling })
}

pub fn every(slots: &[Slot], fitting: Fitting) -> Result<Vec<Measured>, Never> {
    let mut measured = Vec::new();

    for slot in slots {
        let mut runs = Vec::new();

        for span in &slot.spans {
            let Ok(one) = measure(span.text.as_str(), span.face, fitting);

            runs.push(one);
        }

        measured.push(Measured { slot: slot.clone(), runs });
    }

    Ok(measured)
}

fn measure(text: &str, face: Face, fitting: Fitting) -> Result<Size<u32>, Never> {
    let Ok(font) = fitting.font(face);
    let Ok(weight) = face.weight();

    measure_text(Run { said: text, weight, width: u32::MAX }, &font)
}
