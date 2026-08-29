//! The gust: what leaves the tree, and where it has got to.

use cairo::Context;
use console_random::Random;

use crate::garden::Garden;
use crate::paint::{dip, petal_at};
use crate::tree::Tip;

/// One blossom in the wind, and everything about how it travels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Petal {
    pub x: f64,
    pub y: f64,
    pub wait: f64,
    pub speed: f64,
    pub rise: f64,
    pub sway: f64,
    pub beat: f64,
    pub phase: f64,
    pub size: f64,
    pub spin: f64,
    pub pale: bool,
}

/// Where each petal of a gust starts, and how it goes.
///
/// They do not all leave together. Each waits its own share of the gust before
/// it lifts, which is what makes a wind out of what would otherwise be a
/// single flock moving as one body.
///
/// The wind comes in under the tree rather than over it, so the blossom it
/// takes is the blossom hanging low. That is also what keeps the whole picture
/// from having to be redrawn: every frame of a gust costs whatever band of the
/// picture the petals reach, and a stream is a band while a snowstorm is the
/// whole sky.
pub fn flight(garden: &Garden, tips: &[Tip], rng: &mut Random, count: usize) -> Vec<Petal> {
    let low = {
        let mut hanging = tips.to_vec();
        hanging.sort_by(|left, right| right.1.total_cmp(&left.1));
        hanging.truncate((tips.len() * 2 / 3).max(4));
        hanging
    };
    (0..count)
        .map(|_| {
            let (x, y, reach) = *rng.choice(&low);
            Petal {
                x: x + rng.gauss(0.0, reach * 0.7),
                y: y + rng.gauss(0.0, reach * 0.7),
                wait: rng.uniform(0.0, 0.55),
                speed: rng.uniform(0.80, 1.35),
                rise: rng.uniform(-0.17, -0.01),
                sway: rng.uniform(0.02, 0.06),
                beat: rng.uniform(4.0, 9.0),
                phase: rng.uniform(0.0, std::f64::consts::TAU),
                size: garden.across(rng.uniform(0.0026, 0.0058)),
                spin: rng.uniform(-7.0, 7.0),
                pale: rng.random() < 0.34,
            }
        })
        .collect()
}

/// Where one petal has got to, a part of the way through a gust.
///
/// Nothing before it has left, and nothing once it is off the right of the
/// picture. The band the wind is drawn into is measured by walking this, so
/// this is the only place a petal's position is worked out.
pub fn carried(garden: &Garden, petal: &Petal, part: f64) -> Option<(f64, f64, f64)> {
    let gone = (part - petal.wait) / (1.0 - petal.wait);
    if gone <= 0.0 || gone >= 1.0 {
        return None;
    }
    let travel = garden.across(0.95) * petal.speed * gone.powf(1.25);
    let x = petal.x + travel;
    if x - petal.size > garden.width {
        return None;
    }
    let y = petal.y
        + travel * petal.rise
        + (petal.phase + gone * petal.beat).sin() * garden.across(petal.sway);
    Some((x, y, gone))
}

/// The petals of a gust, drawn where they have got to.
pub fn blown(ctx: &Context, garden: &Garden, petals: &[Petal], part: f64) {
    for petal in petals {
        let Some((x, y, gone)) = carried(garden, petal, part) else {
            continue;
        };
        // The wind dies rather than stopping. Without the second half of this
        // the last frame of a gust is full of blossom and the one after it is
        // the resting picture, and blossom does not leave a garden like that.
        let fade = (gone * 6.0).min(1.0) * ((1.0 - gone) * 3.5).min(1.0);
        dip(
            ctx,
            &garden.paint,
            if petal.pale { "petal_pale" } else { "petal" },
            fade,
        );
        petal_at(ctx, x, y, petal.size, petal.phase + gone * petal.spin);
    }
}

/// The strip of the picture a gust ever reaches.
///
/// Only this is redrawn while the wind blows, so the band is the file size. It
/// is measured by walking every petal through every frame rather than by
/// picking a likely strip: a strip that is too big is paid for in every frame,
/// and a strip that is too small clips a petal against nothing.
pub fn band_of(garden: &Garden, petals: &[Petal], steps: usize) -> (i32, i32) {
    let reached = (0..=steps).flat_map(|step| {
        petals.iter().filter_map(move |petal| {
            carried(garden, petal, step as f64 / steps as f64)
                .map(|(_, y, _)| (y - petal.size * 2.0, y + petal.size * 2.0))
        })
    });
    let (top, bottom) = reached.fold((garden.height, 0.0f64), |(top, bottom), (over, under)| {
        (top.min(over), bottom.max(under))
    });
    let top = (top as i32).max(0) & !1;
    let bottom = (bottom as i32 + 2).min(garden.height as i32);
    (top, bottom - top)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Paints;

    fn garden() -> Garden {
        Garden {
            width: 1000.0,
            height: 500.0,
            paint: Paints::default(),
            rest_seconds: 300.0,
            gust_seconds: 6.0,
            frames_per_second: 12.0,
        }
    }

    fn petal() -> Petal {
        Petal {
            x: 100.0,
            y: 200.0,
            wait: 0.25,
            speed: 1.0,
            rise: -0.1,
            sway: 0.0,
            beat: 0.0,
            phase: 0.0,
            size: 4.0,
            spin: 0.0,
            pale: false,
        }
    }

    #[test]
    fn a_petal_is_nowhere_before_it_leaves() {
        assert_eq!(carried(&garden(), &petal(), 0.25), None);
        assert!(carried(&garden(), &petal(), 0.26).is_some());
    }

    #[test]
    fn a_petal_is_nowhere_once_it_is_off_the_picture() {
        let far = Petal {
            x: 990.0,
            speed: 1.35,
            ..petal()
        };
        assert_eq!(carried(&garden(), &far, 0.99), None);
    }

    #[test]
    fn a_petal_that_never_leaves_is_no_band_at_all() {
        let asleep = Petal {
            wait: 0.999,
            ..petal()
        };
        assert_eq!(band_of(&garden(), &[asleep], 4), (500, -498));
    }

    #[test]
    fn the_band_starts_on_an_even_row_and_holds_every_petal() {
        let (top, tall) = band_of(&garden(), &[petal()], 24);
        assert_eq!(top % 2, 0);
        let lowest = (1..24)
            .filter_map(|step| carried(&garden(), &petal(), step as f64 / 24.0))
            .fold(0.0f64, |low, (_, y, _)| low.max(y));
        assert!(f64::from(top) <= 200.0 - 8.0 && f64::from(top + tall) >= lowest + 8.0);
    }

    #[test]
    fn the_wind_takes_the_blossom_that_hangs_low() {
        let tips = vec![(0.0, 10.0, 1.0), (0.0, 90.0, 1.0), (0.0, 50.0, 1.0)];
        let mut rng = Random::seeded(1);
        let petals = flight(&garden(), &tips, &mut rng, 40);
        // Four are kept out of three tips, so every tip is in reach here; what
        // is being held is that a petal starts near a tip and not elsewhere.
        assert!(
            petals
                .iter()
                .all(|petal| petal.y > -20.0 && petal.y < 120.0)
        );
    }
}
