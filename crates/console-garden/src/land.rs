//! The room the garden is in: the sky, the valley, the air between.
//!
//! The picture is taken from up on the near slope, looking down a valley. That
//! is why the horizon sits high and there is so much ground: a camera tilted
//! down puts the horizon up.

use cairo::{Context, LinearGradient, RadialGradient};
use console_random::Random;

use crate::garden::{Garden, HORIZON};
use crate::paint::{Wash, stop, stop_wash};
use crate::way::{road, turf};

/// Darkest overhead, warming down to the last of the light behind the hills.
pub fn sky(ctx: &Context, garden: &Garden) {
    let line = LinearGradient::new(0.0, 0.0, 0.0, garden.down(HORIZON));
    stop(&line, 0.00, &garden.paint, "sky_high", 1.0);
    stop(&line, 0.52, &garden.paint, "sky_high", 1.0);
    stop(&line, 1.00, &garden.paint, "sky_low", 1.0);
    ctx.set_source(&line).expect("a gradient");
    // The whole picture and not down to the horizon. A hill's crest wanders
    // either side of where the hills are said to be, so a sky that stops at
    // that line leaves bare canvas showing wherever a crest happens to sit
    // below it. Everything else is drawn on top of this, so the part of it
    // nobody sees costs nothing.
    ctx.paint().expect("the sky");
}

/// The top of a hill, as a height across the picture.
///
/// Two waves and a lean. One wave is a hill and two are a range: the long one
/// gives the shape of the ridge and the short one keeps it from being a single
/// smooth mound, which is what a hill drawn with one sine always looks like.
pub fn crestline(
    garden: &Garden,
    base: f64,
    tilt: f64,
    swell: f64,
    ripple: f64,
    phase: f64,
) -> impl Fn(f64) -> f64 + use<'_> {
    move |x: f64| {
        let share = x / garden.width;
        garden.height
            * (base
                + tilt * (share - 0.5)
                + swell * (std::f64::consts::TAU * 0.68 * share + phase).sin()
                + ripple * (std::f64::consts::TAU * 2.4 * share + phase * 1.7).sin())
    }
}

/// A hill, filled from its crest to the bottom of the picture.
///
/// Everything is filled all the way down and drawn back to front, so a nearer
/// hill covers a further one without either of them having to know the other
/// is there.
///
/// The fill is a gradient and not a colour. The crest of a ridge is the part
/// of it that is furthest away, so it holds the most air; below the crest the
/// ground is nearer and clearer. Filled flat, a hill is a piece of paper cut
/// to the shape of a hill, which is what they all were for one draft.
pub fn ridge(
    ctx: &Context,
    garden: &Garden,
    crest: &dyn Fn(f64) -> f64,
    top: Wash,
    bottom: Wash,
    fall: f64,
) {
    let high = steps(garden.width, 16.0)
        .map(crest)
        .fold(f64::INFINITY, f64::min);
    let face = LinearGradient::new(0.0, high, 0.0, high + garden.down(fall));
    stop_wash(&face, 0.0, top);
    stop_wash(&face, 1.0, bottom);

    ctx.move_to(0.0, crest(0.0));
    for x in steps(garden.width, 8.0) {
        ctx.line_to(x, crest(x));
    }
    ctx.line_to(garden.width, garden.height);
    ctx.line_to(0.0, garden.height);
    ctx.close_path();
    ctx.set_source(&face).expect("a gradient");
    ctx.fill().expect("a hill");
}

/// Every whole step across the picture, and the far edge as well.
pub fn steps(width: f64, by: f64) -> impl Iterator<Item = f64> {
    (0..=(width as u32)).step_by(by as usize).map(f64::from)
}

/// Each hill is nearer and lower than the one behind it, and holds less of the
/// air between here and it. Distance is the whole of what tells them apart:
/// every one of them is the same ground under the same sky.
struct Hill {
    base: f64,
    tilt: f64,
    swell: f64,
    ripple: f64,
    phase: f64,
    /// How much of the air between here and it it holds.
    mist: f64,
    /// How wide the path is where it crosses this hill.
    spread: f64,
    /// How far to one side the path comes over the crest.
    shift: f64,
}

const HILLS: [Hill; 5] = [
    Hill {
        base: 0.398,
        tilt: -0.034,
        swell: 0.030,
        ripple: 0.009,
        phase: 0.4,
        mist: 0.66,
        spread: 0.0036,
        shift: 0.000,
    },
    Hill {
        base: 0.432,
        tilt: 0.052,
        swell: 0.037,
        ripple: 0.013,
        phase: 2.1,
        mist: 0.48,
        spread: 0.0075,
        shift: 0.014,
    },
    Hill {
        base: 0.472,
        tilt: -0.060,
        swell: 0.031,
        ripple: 0.017,
        phase: 3.7,
        mist: 0.30,
        spread: 0.0150,
        shift: -0.010,
    },
    Hill {
        base: 0.516,
        tilt: 0.034,
        swell: 0.024,
        ripple: 0.012,
        phase: 5.2,
        mist: 0.16,
        spread: 0.0270,
        shift: 0.018,
    },
    Hill {
        base: 0.560,
        tilt: -0.022,
        swell: 0.018,
        ripple: 0.008,
        phase: 1.3,
        mist: 0.07,
        spread: 0.0450,
        shift: 0.002,
    },
];

/// The valley, from the far end of it to the floor, with the path on it.
///
/// The nearest of these is the floor. There is no separate shape for the
/// ground the path lies on, because a floor drawn as its own rectangle puts a
/// straight edge across the whole picture at the height where the hills stop,
/// and a straight edge is the one thing a landscape has none of.
///
/// The path is drawn into this rather than over it. Each hill is laid down and
/// then the path is drawn with nothing above that hill's crest, and the next
/// hill nearer covers everything below its own, so what is left is the piece
/// of path on that hill and nothing else. Each piece is drawn at the width the
/// path has on that hill, which is why it steps rather than tapers.
pub fn hills(ctx: &Context, garden: &Garden, seed: u64) {
    for hill in &HILLS {
        let crest = crestline(
            garden,
            hill.base,
            hill.tilt,
            hill.swell,
            hill.ripple,
            hill.phase,
        );
        ridge(
            ctx,
            garden,
            &crest,
            garden.paint.washed("haze_far", "earth", Some(hill.mist)),
            garden
                .paint
                .washed("haze_far", "earth", Some(hill.mist * 0.45)),
            0.16,
        );
        turf(
            ctx,
            garden,
            &crest,
            hill.spread,
            hill.shift,
            &mut Random::seeded(seed + 5),
        );
        road(
            ctx,
            garden,
            &crest,
            hill.spread,
            hill.shift,
            &mut Random::seeded(seed + 3),
        );
    }

    // The floor falls away towards the bottom of the picture, where it is
    // nearest and holds nothing worth looking at.
    let line = LinearGradient::new(0.0, garden.down(0.60), 0.0, garden.height);
    stop(&line, 0.0, &garden.paint, "shade", 0.0);
    stop(&line, 1.0, &garden.paint, "shade", 0.55);
    ctx.set_source(&line).expect("a gradient");
    ctx.rectangle(
        0.0,
        garden.down(0.60),
        garden.width,
        garden.down(0.40) + 1.0,
    );
    ctx.fill().expect("the floor");
}

/// The slope this is all being looked at from.
///
/// It leaves the left of the picture high and falls away out of the bottom, so
/// the near end of the path goes behind it. A path that runs to the bottom edge
/// is a path you are standing on; a path that goes behind a slope is one you
/// are looking down at, which is the difference asked for.
pub fn brow(garden: &Garden, x: f64) -> f64 {
    let share = x / garden.width;
    garden.height
        * (0.700
            + 0.86 * share.powf(1.35)
            + 0.030 * (std::f64::consts::TAU * 1.7 * share + 1.1).sin())
}

pub fn near_slope(ctx: &Context, garden: &Garden) {
    ridge(
        ctx,
        garden,
        &|x| brow(garden, x),
        garden.paint.washed("shade", "earth", Some(0.30)),
        garden.paint.washed("shade", "earth", Some(0.72)),
        0.34,
    );
}

/// What is left of the day, low behind the far end of the valley.
///
/// Squashed into an ellipse and laid across the horizon rather than up to it.
/// Light in the air does not stop where the ground starts, and the first
/// version of this did, which put a step across the whole picture at exactly
/// the height a step is least forgivable.
pub fn glow(ctx: &Context, garden: &Garden) {
    let (middle, low) = (garden.across(0.54), garden.down(HORIZON + 0.01));
    ctx.save().expect("a brush can be put down");
    ctx.translate(middle, low);
    ctx.scale(1.0, 0.40);
    ctx.translate(-middle, -low);
    let light = RadialGradient::new(middle, low, 0.0, middle, low, garden.across(0.48));
    stop(&light, 0.0, &garden.paint, "glow", 1.0);
    stop(&light, 1.0, &garden.paint, "glow", 0.0);
    ctx.set_source(&light).expect("a gradient");
    ctx.paint().expect("the last of the light");
    ctx.restore().expect("a brush comes back");
}

/// The air in the valley, thickest where the valley is furthest away.
///
/// It is drawn across the join between the sky and the hills rather than up to
/// it. Anything that stops at the horizon puts a line there, and a line across
/// a picture at the height of the far distance is the one thing that stops it
/// being distance.
pub fn haze(ctx: &Context, garden: &Garden) {
    let (top, bottom) = (garden.down(HORIZON - 0.16), garden.down(0.74));
    let line = LinearGradient::new(0.0, top, 0.0, bottom);
    for (offset, alpha) in [
        (0.00, 0.0),
        (0.26, 0.80),
        (0.42, 1.0),
        (0.72, 0.42),
        (1.00, 0.0),
    ] {
        stop(&line, offset, &garden.paint, "haze", alpha);
    }
    ctx.set_source(&line).expect("a gradient");
    ctx.rectangle(0.0, top, garden.width, bottom - top);
    ctx.fill().expect("the air");
}

/// Clip everything after this to below one crest.
pub fn under(ctx: &Context, garden: &Garden, crest: &dyn Fn(f64) -> f64) {
    ctx.move_to(0.0, crest(0.0));
    for x in steps(garden.width, 8.0) {
        ctx.line_to(x, crest(x));
    }
    ctx.line_to(garden.width, garden.height);
    ctx.line_to(0.0, garden.height);
    ctx.close_path();
    ctx.clip();
}
