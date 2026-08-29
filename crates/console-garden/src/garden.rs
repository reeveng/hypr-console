//! The picture's own measurements, and the table it is painted out of.

use serde::Deserialize;

use crate::paint::{Paints, Wash};

/// Where the furthest hills meet the sky.
///
/// It is high because the picture is taken from up on the near slope looking
/// down a valley, and a camera tilted down puts the horizon up.
pub const HORIZON: f64 = 0.408;

/// How hard the encoder is asked to work. Read by the stamp, so a change here
/// is a redraw.
pub const QUALITY: u32 = 78;

/// Everything a shape needs to know about where it is being drawn.
///
/// Passed rather than reached for. The size is the screen's own, read from the
/// compositor's file, and the paints are the palette's; neither is written
/// down in this crate.
#[derive(Debug, Clone)]
pub struct Garden {
    pub width: f64,
    pub height: f64,
    pub paint: Paints,
    pub rest_seconds: f64,
    pub gust_seconds: f64,
    pub frames_per_second: f64,
}

impl Garden {
    /// A fraction across the picture, in pixels.
    pub fn across(&self, share: f64) -> f64 {
        self.width * share
    }

    /// A fraction down the picture, in pixels.
    pub fn down(&self, share: f64) -> f64 {
        self.height * share
    }

    /// How many frames a gust is.
    pub fn gust_frames(&self) -> usize {
        ((self.gust_seconds * self.frames_per_second).round() as usize).max(2)
    }
}

/// What `theme/palette.toml` says about the garden.
#[derive(Debug, Deserialize)]
pub struct Spec {
    pub garden: Said,
}

#[derive(Debug, Deserialize)]
pub struct Said {
    pub rest_seconds: f64,
    pub gust_seconds: f64,
    pub frames_per_second: f64,
    pub paint: indexmap::IndexMap<String, Dipped>,
}

/// One entry of the garden's table: which colour, and how much of it reaches
/// the picture.
///
/// `alpha` lives in the palette rather than in this crate because a wash at a
/// tenth is a decision about colour. The shape of a tree is not.
#[derive(Debug, Deserialize)]
pub struct Dipped {
    pub colour: String,
    pub alpha: f64,
}

impl Said {
    /// The table, as colours a brush can be dipped in.
    pub fn paints(&self, palette: &dyn Fn(&str) -> Option<String>) -> Result<Paints, String> {
        self.paint
            .iter()
            .map(|(name, dipped)| {
                palette(&dipped.colour)
                    .map(|code| (name.clone(), Wash::of(&code, dipped.alpha)))
                    .ok_or_else(|| {
                        format!(
                            "the garden paints {name} with {}, which is not a colour",
                            dipped.colour
                        )
                    })
            })
            .collect::<Result<Vec<_>, String>>()
            .map(Paints::of)
    }
}
