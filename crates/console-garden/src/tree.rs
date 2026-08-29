//! The two trees: what they are made of, and what they throw.

use cairo::{
    Context, Filter, Format, ImageSurface, LinearGradient, Matrix, RadialGradient, SurfacePattern,
};
use console_random::Random;

use crate::garden::Garden;
use crate::paint::{Wash, curve, dip, petal_at, stop, stop_wash};

/// Where a branch ends, and how far it reached to get there. Blossom goes here.
pub type Tip = (f64, f64, f64);

/// One reach of a branch: where it starts, where it is going, and how thick
/// and how bent it is on the way.
///
/// A branch said as a thing rather than as six numbers in a row, so that the
/// recursion reads as one branch making the next one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reach {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub length: f64,
    pub width: f64,
    pub bow: f64,
}

/// A whole tree, said the same way: where it stands and what it grew into.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Standing {
    pub x: f64,
    pub base: f64,
    pub height: f64,
    pub lean: f64,
    pub seed: u64,
    pub depth: u32,
    pub size: f64,
}

/// One branch, and the branches off it, laid down as a filled shape.
///
/// A stroke would give every branch the same thickness from the trunk to the
/// tip. A tree does not do that, so each limb is drawn as its own tapering
/// quadrilateral and the next one starts narrower.
pub fn limb(ctx: &Context, tips: &mut Vec<Tip>, reach: Reach, rng: &mut Random, depth: u32) {
    let Reach {
        x,
        y,
        angle,
        length,
        width,
        bow,
    } = reach;
    let end_x = x + angle.cos() * length;
    let end_y = y + angle.sin() * length;
    let across = angle + std::f64::consts::FRAC_PI_2;
    let tip_width = width * 0.62;

    // Both sides of the limb are bent the same way by the same amount, so the
    // limb curves rather than swelling. A branch that leaves the trunk dead
    // straight is a mast.
    let mid_x = (x + end_x) / 2.0 + across.cos() * bow;
    let mid_y = (y + end_y) / 2.0 + across.sin() * bow;
    let (out, back) = (across.cos(), across.sin());

    ctx.move_to(x + out * width, y + back * width);
    curve(
        ctx,
        (x + out * width, y + back * width),
        (mid_x + out * width * 0.8, mid_y + back * width * 0.8),
        (end_x + out * tip_width, end_y + back * tip_width),
    );
    ctx.line_to(end_x - out * tip_width, end_y - back * tip_width);
    curve(
        ctx,
        (end_x - out * tip_width, end_y - back * tip_width),
        (mid_x - out * width * 0.8, mid_y - back * width * 0.8),
        (x - out * width, y - back * width),
    );
    ctx.close_path();
    ctx.fill().expect("a limb");

    if depth == 0 {
        tips.push((end_x, end_y, length));
        return;
    }

    // A cherry does not reach for the sky, it reaches sideways and then leans
    // down under its own weight, so a branch is turned away from its parent by
    // a good deal and then pulled back towards level.
    for turn in [-1.0, 1.0] {
        for _ in 0..*rng.choice(&[1, 1, 2]) {
            let next = Reach {
                x: end_x,
                y: end_y,
                angle: (angle + turn * rng.uniform(0.30, 0.80) + rng.uniform(0.04, 0.20))
                    .clamp(-2.5, 0.55),
                length: length * rng.uniform(0.54, 0.72),
                width: tip_width * rng.uniform(0.58, 0.76),
                bow: length * rng.uniform(-0.10, 0.10),
            };
            limb(ctx, tips, next, rng, depth - 1);
        }
    }
}

/// The one limb the rest of the tree comes off, in a trunk's proportions.
///
/// Its own function because a tree is drawn twice: once standing, and once
/// lying on the ground as its own shadow. Written down in both places the two
/// could be different trees, and the day somebody makes this one lean further
/// is the day its shadow does not.
pub fn trunk(ctx: &Context, tips: &mut Vec<Tip>, standing: &Standing, rng: &mut Random) {
    let reach = Reach {
        x: standing.x,
        y: standing.base,
        angle: -std::f64::consts::FRAC_PI_2 + standing.lean,
        length: standing.height * 0.42,
        width: standing.height * 0.036,
        bow: standing.height * 0.070,
    };
    limb(ctx, tips, reach, rng, standing.depth);
}

/// A trunk from the ground and everything that comes off it.
///
/// The light is off to the right, so the bark is laid down as a gradient
/// across the whole tree rather than a flat fill. A branch on the near side of
/// the trunk then comes out lighter than one behind it without anything having
/// to know which side it is on.
pub fn tree(ctx: &Context, garden: &Garden, standing: &Standing, rng: &mut Random) -> Vec<Tip> {
    let mut tips = Vec::new();
    let bark = garden.paint.get("bark");
    let lit = LinearGradient::new(
        standing.x - standing.height * 0.10,
        0.0,
        standing.x + standing.height * 0.10,
        0.0,
    );
    stop_wash(&lit, 0.0, Wash { alpha: 1.0, ..bark });
    stop_wash(&lit, 1.0, garden.paint.washed("bark_lit", "bark", None));
    ctx.set_source(&lit).expect("a gradient");

    trunk(ctx, &mut tips, standing, rng);
    tips
}

/// The mass of it first, then the few that catch the light on top.
pub fn blossom(ctx: &Context, garden: &Garden, tips: &[Tip], rng: &mut Random, size: f64) {
    for (x, y, reach) in tips.iter().copied() {
        for _ in 0..4 {
            let radius = reach * rng.uniform(0.5, 1.15) * size;
            let (from_x, from_y) = (
                x + rng.gauss(0.0, reach * 0.8),
                y + rng.gauss(0.0, reach * 0.8),
            );
            let cloud = RadialGradient::new(from_x, from_y, 0.0, x, y, radius);
            stop(&cloud, 0.0, &garden.paint, "bloom", 1.0);
            stop(&cloud, 0.55, &garden.paint, "bloom_deep", 1.0);
            stop(&cloud, 1.0, &garden.paint, "bloom", 0.0);
            ctx.set_source(&cloud).expect("a gradient");
            ctx.arc(x, y, radius, 0.0, std::f64::consts::TAU);
            ctx.fill().expect("a cloud of blossom");
        }
    }
    for (x, y, reach) in tips.iter().copied() {
        for petal in 0..7 {
            let px = x + rng.gauss(0.0, reach * 0.62);
            let py = y + rng.gauss(0.0, reach * 0.62);
            let which = match petal % 3 {
                0 => "petal_pale",
                _ => "petal",
            };
            dip(ctx, &garden.paint, which, rng.uniform(0.55, 1.0));
            let wide = (reach * 0.085 * size + garden.across(0.0013)) * rng.uniform(0.68, 1.38);
            petal_at(ctx, px, py, wide, rng.uniform(0.0, std::f64::consts::PI));
        }
    }
}

/// Where the light comes from, said as the direction of a shadow.
///
/// It is the same light the bark is lit by: low, behind the far end of the
/// valley, and off to the right. A tree this tall under a sun that low throws
/// a shadow the length of a field, and the ground takes nearly all of that back
/// by lying almost edge-on to us, so what reaches the picture is a long reach
/// across it and a short one down it. Both per height of whatever is casting.
const THROW: f64 = -0.85;
const SETTLE: f64 = 0.36;

/// A shadow is sharp where the thing touches the ground and soft at its far
/// end, because the light has a width and that end is further from the ground.
/// So it is laid down three times, each reaching further than the last and
/// fainter for it. At the foot the three lie on top of each other and at the
/// tip they lie apart, and that is the spreading.
const SPREADS: [(f64, f64); 3] = [(0.90, 0.42), (1.00, 0.32), (1.14, 0.20)];

/// How much smaller than the picture the shadow is drawn before it is laid
/// back on it, which is to say how blurred it is: stretching a small picture up
/// is the blur cairo has not got.
///
/// It has to be blurred at all because there is no sun here to cast it. What
/// light is left is the whole of the sky behind the hills, and a light that
/// broad carries no outline as far as the ground: the shadow of a twig is not
/// a thin shadow, it is no shadow. Drawn sharp and faint instead, the tree came
/// out as scratches on a field.
const SOFTEN: i32 = 8;

/// The tree lying on the ground, as a small picture of where it is solid.
///
/// The ground is put under the brush rather than a second set of shapes being
/// worked out: the same tree, from the same seed, through a transform that tips
/// it away from the light and flattens it into the picture. A shadow cannot
/// come out of a different tree than the one standing in it.
///
/// The blossom is thrown with it, as a mass at every branch tip. What stops the
/// light is the flower and not the twig, and a tree that threw only its
/// branches would be a tree in winter.
fn thrown(garden: &Garden, standing: &Standing, stretch: f64) -> ImageSurface {
    let mask = ImageSurface::create(
        Format::A8,
        garden.width as i32 / SOFTEN,
        garden.height as i32 / SOFTEN,
    )
    .expect("a mask");
    let ctx = Context::new(&mask).expect("a brush");
    ctx.scale(1.0 / f64::from(SOFTEN), 1.0 / f64::from(SOFTEN));
    ctx.translate(standing.x, standing.base);
    ctx.transform(Matrix::new(
        1.0,
        0.0,
        -THROW * stretch,
        -SETTLE * stretch,
        0.0,
        0.0,
    ));
    ctx.translate(-standing.x, -standing.base);
    ctx.set_source_rgba(0.0, 0.0, 0.0, 1.0);

    let mut tips = Vec::new();
    trunk(
        &ctx,
        &mut tips,
        standing,
        &mut Random::seeded(standing.seed),
    );
    for (tip_x, tip_y, reach) in tips {
        ctx.arc(
            tip_x,
            tip_y,
            reach * 0.80 * standing.size,
            0.0,
            std::f64::consts::TAU,
        );
        ctx.fill().expect("a mass of blossom");
    }
    mask
}

/// What a tree throws.
///
/// Nothing is clipped. A shadow leaves the foot of what casts it going down
/// the picture, and down the picture is nearer, so there is no ground behind
/// it for it to climb.
pub fn shadow(ctx: &Context, garden: &Garden, standing: &Standing) {
    let cast = garden.paint.get("shadow");
    for (stretch, weight) in SPREADS {
        let laid = SurfacePattern::create(thrown(garden, standing, stretch));
        laid.set_filter(Filter::Good);
        ctx.save().expect("a brush can be put down");
        ctx.scale(f64::from(SOFTEN), f64::from(SOFTEN));
        ctx.set_source_rgba(cast.red, cast.green, cast.blue, cast.alpha * weight);
        ctx.mask(&laid).expect("a shadow");
        ctx.restore().expect("a brush comes back");
    }
}

/// One tree, everything it is made of, in the order the ground sees it: what
/// it throws, then what throws it, then what is in flower on it.
pub fn planted(ctx: &Context, garden: &Garden, standing: &Standing) -> Vec<Tip> {
    shadow(ctx, garden, standing);
    let mut rng = Random::seeded(standing.seed);
    let tips = tree(ctx, garden, standing, &mut rng);
    blossom(ctx, garden, &tips, &mut rng, standing.size);
    tips
}
