//! Everything laid down in the order the ground sees it, and the sheet it
//! goes onto.

use cairo::{Context, Format, ImageSurface};
use console_random::Random;

use crate::air::{Petal, band_of, blown, flight};
use crate::garden::Garden;
use crate::land::{brow, glow, haze, hills, near_slope, sky};
use crate::tree::{Standing, Tip, planted};

/// How many petals a gust carries.
const GUST: usize = 170;

/// Everything that does not move, back to front.
///
/// The order matters more than any one shape in it. Everything beyond reach is
/// drawn first and the air is put in front of all of it together, so that the
/// mist crosses the horizon instead of stopping at it. The slope this is being
/// looked at from is drawn after the path, so the path goes behind it, and the
/// tree that is close is drawn on top of the slope, standing on it.
///
/// Two trees, and no more. There is very little here on purpose: what a
/// wallpaper is for is to be behind something.
pub fn scene(ctx: &Context, garden: &Garden, seed: u64) -> Vec<Tip> {
    sky(ctx, garden);
    hills(ctx, garden, seed);

    planted(
        ctx,
        garden,
        &Standing {
            x: garden.across(0.665),
            base: garden.down(0.655),
            height: garden.down(0.26),
            lean: 0.12,
            seed: seed + 7,
            depth: 4,
            size: 0.76,
        },
    );

    glow(ctx, garden);
    haze(ctx, garden);
    near_slope(ctx, garden);

    let near = garden.across(0.205);
    planted(
        ctx,
        garden,
        &Standing {
            x: near,
            base: brow(garden, near),
            height: garden.down(0.655),
            lean: 0.13,
            seed,
            depth: 5,
            size: 1.0,
        },
    )
}

/// A blank sheet, and a brush that draws onto the band of the picture starting
/// at `offset`.
pub fn sheet(width: i32, height: i32, offset: f64) -> (ImageSurface, Context) {
    let surface = ImageSurface::create(Format::Rgb24, width, height).expect("a sheet");
    let ctx = Context::new(&surface).expect("a brush");
    ctx.translate(0.0, -offset);
    (surface, ctx)
}

/// One frame of the wallpaper: where it goes, how long it lasts, and it.
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub milliseconds: u32,
    pub picture: Vec<u8>,
}

/// What the drawing came out as: the resting picture, the frames the file is
/// made of, and the band the wind was found to cross.
pub struct Drawn {
    pub still: ImageSurface,
    pub frames: Vec<Frame>,
    pub top: i32,
    pub tall: i32,
    pub count: usize,
}

/// The whole wallpaper, still and moving, from one seed.
pub fn draw(garden: &Garden, seed: u64, encode: &dyn Fn(&ImageSurface) -> Vec<u8>) -> Drawn {
    let (width, height) = (garden.width as i32, garden.height as i32);
    let (still, tips) = {
        let (surface, ctx) = sheet(width, height, 0.0);
        let tips = scene(&ctx, garden, seed);
        drop(ctx);
        (surface, tips)
    };

    let petals: Vec<Petal> = flight(garden, &tips, &mut Random::seeded(seed + 11), GUST);
    let count = garden.gust_frames();
    let (top, tall) = band_of(garden, &petals, count);
    let each = (garden.gust_seconds * 1000.0 / count as f64).round() as u32;

    let resting = Frame {
        x: 0,
        y: 0,
        width,
        height,
        milliseconds: (garden.rest_seconds * 1000.0).round() as u32,
        picture: encode(&still),
    };
    let gust = (1..=count).map(|step| {
        let (strip, ctx) = sheet(width, tall, f64::from(top));
        scene(&ctx, garden, seed);
        if step < count {
            blown(&ctx, garden, &petals, step as f64 / count as f64);
        }
        drop(ctx);
        Frame {
            x: 0,
            y: top,
            width,
            height: tall,
            milliseconds: each,
            picture: encode(&strip),
        }
    });

    Drawn {
        frames: std::iter::once(resting).chain(gust).collect(),
        still,
        top,
        tall,
        count,
    }
}
