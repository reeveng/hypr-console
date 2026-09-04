//! The path through the valley, and the grass either side of it.

use cairo::{Context, LinearGradient};
use console_random::Random;

use crate::fault::Drawing;
use crate::garden::Garden;
use crate::land::under;
use crate::paint::{dip, petal_at, stop};

/// Where the path is furthest and where it is nearest, down the picture.
///
/// `FAR` has to sit above the crest of the furthest hill where the path
/// crosses it, or the path simply begins below that hill and the hill has no
/// path on it, which is not the same thing as being too far away to see one.
pub const FAR: f64 = 0.392;
pub const NEAR: f64 = 0.960;

/// The middle of the path and half its width, at a height down the picture.
///
/// One route over rolling ground, so where it runs is a question about how far
/// down the picture you are looking. How wide it is there is a question about
/// which hill you are looking at, and those are not the same question.
///
/// Going down the picture across a crest does not take you a little nearer. It
/// takes you from the furthest of one hill that can still be seen to the top
/// of the next hill along, which is a good deal nearer, so the path steps
/// wider at every crest rather than growing smoothly. Drawn at one width down
/// the whole picture it climbed over every ridge like something painted on.
pub fn lane(garden: &Garden, down: f64, spread: f64, shift: f64) -> (f64, f64) {
    let part = ((down - FAR) / (NEAR - FAR)).clamp(0.0, 1.0);
    let x = garden.across(
        0.60 - 0.475 * part.powf(1.15) + 0.075 * (std::f64::consts::PI * part).sin() + shift,
    );
    let half = garden.across(spread) * (0.62 + 0.38 * part);
    (x, half)
}

/// As much of the path as can be seen from over one particular crest.
///
/// A path lying on rolling ground is not one ribbon. It comes over a crest,
/// runs down the near face of that hill towards you, and goes behind the next
/// hill along; the dip between them is the far side of something and the far
/// side of something cannot be seen. So it is drawn once for every hill, each
/// time with nothing above that hill's crest, and the next hill nearer paints
/// over everything below its own. What is left is a piece of path on each
/// face, each one wider than the last because each one is nearer.
///
/// Drawn as a single ribbon it climbed straight over every ridge in the
/// picture, which is the sort of thing you cannot stop seeing.
pub fn road(
    ctx: &Context,
    garden: &Garden,
    over: &dyn Fn(f64) -> f64,
    spread: f64,
    shift: f64,
    rng: &mut Random,
) -> Drawing<()> {
    ctx.save()?;
    under(ctx, garden, over);

    let steps = 160;
    let sides: Vec<((f64, f64), (f64, f64))> = (0..=steps)
        .map(|i| {
            let down = FAR + (NEAR - FAR) * f64::from(i) / f64::from(steps);
            let (x, half) = lane(garden, down, spread, shift);
            ((x - half, garden.down(down)), (x + half, garden.down(down)))
        })
        .collect();
    let left: Vec<(f64, f64)> = sides.iter().map(|(one, _)| *one).collect();
    let right: Vec<(f64, f64)> = sides.iter().map(|(_, other)| *other).collect();

    ctx.move_to(left[0].0, left[0].1);

    for point in &left[1..] {
        ctx.line_to(point.0, point.1);
    }

    for point in right.iter().rev() {
        ctx.line_to(point.0, point.1);
    }

    ctx.close_path();

    dip(ctx, &garden.paint, "road", 1.0)?;
    ctx.fill_preserve()?;
    ctx.save()?;
    ctx.clip();

    // Worn lighter towards the near end, where feet have been.
    let line = LinearGradient::new(0.0, garden.down(FAR), 0.0, garden.down(NEAR));

    for (offset, alpha) in [(0.0, 0.0), (0.55, 0.45), (1.0, 0.75)] {
        stop(&line, offset, &garden.paint, "road_light", alpha)?;
    }

    ctx.set_source(&line)?;
    ctx.paint()?;
    ctx.restore()?;

    // Grass does not stop at a line. The edge is laid down as a few strokes
    // getting wider and fainter, which is the nearest a vector drawing comes
    // to something being out of focus.
    for width in [1.0, 3.0, 7.0] {
        dip(ctx, &garden.paint, "verge", 0.45 / width)?;
        ctx.set_line_width(garden.across(0.0010) * width);

        for edge in [&left, &right] {
            ctx.move_to(edge[0].0, edge[0].1);

            for point in &edge[1..] {
                ctx.line_to(point.0, point.1);
            }

            ctx.stroke()?;
        }
    }

    fallen(ctx, garden, spread, shift, rng)?;
    ctx.restore()?;
    Ok(())
}

/// The ground either side of the path, which is grass and not paper.
///
/// Nothing here is meant to be looked at. It is here because a field of one
/// flat colour reads as a hole, and a field with something in it reads as a
/// field, even when what is in it is too faint to name.
pub fn turf(
    ctx: &Context,
    garden: &Garden,
    over: &dyn Fn(f64) -> f64,
    spread: f64,
    shift: f64,
    rng: &mut Random,
) -> Drawing<()> {
    ctx.save()?;
    under(ctx, garden, over);

    for _ in 0..900 {
        let down = rng.uniform(FAR, 1.0);
        let x = rng.uniform(0.0, garden.width);
        let (middle, half) = lane(garden, down, spread, shift);

        if (x - middle).abs() < half {
            continue;
        }

        let part = (down - FAR) / (1.0 - FAR);
        let blade = garden.across(0.0008 + 0.0060 * part);
        dip(
            ctx,
            &garden.paint,
            "verge",
            rng.uniform(0.06, 0.20) * (0.3 + part),
        )?;
        ctx.set_line_width(garden.across(0.0005 + 0.0016 * part));
        ctx.move_to(x, garden.down(down));
        ctx.line_to(x + rng.gauss(0.0, blade * 0.35), garden.down(down) - blade);
        ctx.stroke()?;
    }

    ctx.restore()?;
    Ok(())
}

/// Blossom already down, lying on the path where it fell.
pub fn fallen(ctx: &Context, garden: &Garden, spread: f64, shift: f64, rng: &mut Random) -> Drawing<()> {
    for _ in 0..200 {
        let down = FAR + (NEAR - FAR) * rng.random().powf(0.55);
        let (middle, half) = lane(garden, down, spread, shift);
        let x = middle + rng.uniform(-1.0, 1.0) * half * 0.92;
        let near = (down - FAR) / (NEAR - FAR);
        let size = (garden.across(0.0004) + garden.across(0.0028) * near) * rng.uniform(0.75, 1.25);
        dip(ctx, &garden.paint, "fallen", 0.25 + 0.75 * near)?;
        petal_at(
            ctx,
            x,
            garden.down(down),
            size,
            rng.uniform(0.0, std::f64::consts::PI),
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{Paints, Wash};

    fn garden() -> Garden {
        Garden {
            width: 2560.0,
            height: 1600.0,
            paint: Paints::of([("road".to_string(), Wash::of("ffffff", 1.0).expect("a colour"))]),
            rest_seconds: 420.0,
            gust_seconds: 3.6,
            frames_per_second: 12.0,
        }
    }

    #[test]
    fn the_path_is_wider_the_nearer_it_gets() {
        let garden = garden();
        let (_, far) = lane(&garden, FAR, 0.02, 0.0);
        let (_, near) = lane(&garden, NEAR, 0.02, 0.0);
        assert!(near > far, "{near} is no wider than {far}");
    }

    #[test]
    fn the_path_is_the_same_width_above_and_below_where_it_runs() {
        // Beyond either end it is clamped, so a hill whose crest sits above
        // FAR does not get a path of negative width.
        let garden = garden();
        assert_eq!(lane(&garden, 0.0, 0.02, 0.0), lane(&garden, FAR, 0.02, 0.0));
        assert_eq!(
            lane(&garden, 1.0, 0.02, 0.0),
            lane(&garden, NEAR, 0.02, 0.0)
        );
    }

    #[test]
    fn the_path_crosses_the_picture_from_the_middle_to_the_left() {
        let garden = garden();
        let (far, _) = lane(&garden, FAR, 0.02, 0.0);
        let (near, _) = lane(&garden, NEAR, 0.02, 0.0);
        assert!(
            far > near,
            "the path should come towards the left, not away"
        );
        assert!(near > 0.0 && far < garden.width);
    }

    #[test]
    fn shifting_a_hills_path_moves_it_and_leaves_its_width_alone() {
        let garden = garden();
        let (straight, wide) = lane(&garden, 0.6, 0.02, 0.0);
        let (aside, still) = lane(&garden, 0.6, 0.02, 0.05);
        assert_eq!(wide, still);
        assert!((aside - straight - garden.across(0.05)).abs() < 1e-9);
    }
}
