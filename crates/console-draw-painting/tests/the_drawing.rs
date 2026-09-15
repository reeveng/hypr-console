//! What comes out of a shape, read back out of the bytes.
//!
//! There is no compositor here and no screen. A frame is a `Vec<u8>` the right
//! length, the shapes go into it, and the pixel that was asked about is read
//! out of the slice -- which is the whole claim this crate makes about itself,
//! that the drawing can be pressed on a machine with nothing to draw on.

use console_core_colour::{Oklch, Rgba};
use console_core_geometry::{Point, Size};
use console_core_shapes::{Edge, Font, Panel, Round, Shape, Weight, Words};
use console_draw_painting::{Frame, Run, measured, onto};

const WIDE: u32 = 40;

const TALL: u32 = 20;

fn font() -> Font {
    Font { family: "Noto Sans".to_string(), tall: 10 }
}

fn inked(pixels: &[u8]) -> usize {
    pixels.chunks_exact(4).filter(|pixel| pixel.iter().any(|byte| *byte > 0)).count()
}

fn slab() -> Vec<u8> {
    let long = WIDE.saturating_mul(TALL).saturating_mul(4);

    vec![0; usize::try_from(long).unwrap_or(0)]
}

fn at(pixels: &[u8], across: u32, down: u32) -> [u8; 4] {
    let stride = WIDE.saturating_mul(4);
    let start = down.saturating_mul(stride).saturating_add(across.saturating_mul(4));
    let start = usize::try_from(start).unwrap_or(0);
    let held = pixels.get(start..start.saturating_add(4)).unwrap_or(&[]);

    match held {
        [blue, green, red, alpha] => [*blue, *green, *red, *alpha],
        _ => [0, 0, 0, 0],
    }
}

fn colour(six: &str) -> Oklch {
    let channels = match Rgba::of(six) {
        Ok(channels) => channels,
        Err(why) => panic!("{six} should read: {why}"),
    };
    let Ok(oklch) = Oklch::of(channels);

    oklch
}

fn whole() -> Frame {
    Frame { device: Size { wide: WIDE, tall: TALL }, points: Size { wide: WIDE, tall: TALL } }
}

#[test]
fn a_panel_puts_its_colour_where_it_was_put_and_nowhere_else() {
    let mut pixels = slab();
    let shapes = vec![Shape::Panel(Panel {
        at: Point { across: 10, down: 5 },
        size: Size { wide: 10, tall: 10 },
        round: Round(0),
        fill: colour("ff0000"),
        edge: Edge::None,
    })];

    match onto(&mut pixels, whole(), &shapes) {
        Ok(()) => {}
        Err(why) => panic!("the panel should draw: {why}"),
    }

    assert_eq!(at(&pixels, 15, 10), [0x00, 0x00, 0xff, 0xff], "the middle of the panel");
    assert_eq!(at(&pixels, 2, 2), [0, 0, 0, 0], "outside it");
}

#[test]
fn a_frame_of_device_pixels_puts_a_panel_where_the_points_said() {
    let mut pixels = slab();
    let frame = Frame {
        device: Size { wide: WIDE, tall: TALL },
        points: Size { wide: WIDE / 2, tall: TALL / 2 },
    };
    let shapes = vec![Shape::Panel(Panel {
        at: Point { across: 5, down: 0 },
        size: Size { wide: 5, tall: 5 },
        round: Round(0),
        fill: colour("00ff00"),
        edge: Edge::None,
    })];

    match onto(&mut pixels, frame, &shapes) {
        Ok(()) => {}
        Err(why) => panic!("the panel should draw: {why}"),
    }

    assert_eq!(at(&pixels, 12, 4), [0x00, 0xff, 0x00, 0xff], "twice as far across and down");
    assert_eq!(at(&pixels, 4, 4), [0, 0, 0, 0], "before where the panel starts");
}

#[test]
fn an_edge_is_drawn_in_its_own_colour_over_the_fill() {
    let mut pixels = slab();
    let shapes = vec![Shape::Panel(Panel {
        at: Point { across: 0, down: 0 },
        size: Size { wide: WIDE, tall: TALL },
        round: Round(0),
        fill: colour("000000"),
        edge: Edge::Of { wide: 2, colour: colour("ffffff") },
    })];

    match onto(&mut pixels, whole(), &shapes) {
        Ok(()) => {}
        Err(why) => panic!("the panel should draw: {why}"),
    }

    assert_eq!(at(&pixels, 0, 10), [0xff, 0xff, 0xff, 0xff], "on the edge");
    assert_eq!(at(&pixels, 20, 10), [0x00, 0x00, 0x00, 0xff], "well inside it");
}

#[test]
fn a_run_of_words_wraps_taller_the_narrower_it_is_given() {
    let said = "the desktop has said something worth reading twice";
    let Ok(roomy) = measured(Run { said, weight: Weight::Plain, wide: 400 }, &font());
    let Ok(tight) = measured(Run { said, weight: Weight::Plain, wide: 80 }, &font());

    assert!(tight.tall > roomy.tall, "{tight:?} should be taller than {roomy:?}");
    assert!(tight.wide <= 80, "a wrapped run should stay inside its width: {tight:?}");
}

#[test]
fn a_face_asked_for_in_pixels_is_drawn_that_many_pixels_tall() {
    let run = |tall| {
        let Ok(size) =
            measured(Run { said: "Hg", weight: Weight::Plain, wide: 400 }, &Font { tall, ..font() });

        size
    };
    let one = run(20);
    let twice = run(40);

    assert!(one.tall >= 20, "a line set at twenty pixels came out {one:?}");
    assert!(one.tall < 34, "a line set at twenty pixels came out {one:?}, which is points");
    assert!(
        twice.wide.abs_diff(one.wide.saturating_mul(2)) <= 2,
        "twice the height should be about twice the width: {one:?} against {twice:?}"
    );
}

#[test]
fn bold_is_not_the_same_run_as_plain() {
    let said = "notification";
    let Ok(plain) = measured(Run { said, weight: Weight::Plain, wide: 400 }, &font());
    let Ok(bold) = measured(Run { said, weight: Weight::Bold, wide: 400 }, &font());

    assert!(bold.wide > plain.wide, "bold {bold:?} should be wider than plain {plain:?}");
}

#[test]
fn words_put_ink_on_the_frame_where_nothing_was() {
    let mut pixels = slab();
    let shapes = vec![Shape::Words(Words {
        at: Point { across: 0, down: 0 },
        wide: WIDE,
        said: "HHHH".to_string(),
        weight: Weight::Bold,
        font: font(),
        ink: colour("ffffff"),
    })];

    match onto(&mut pixels, whole(), &shapes) {
        Ok(()) => {}
        Err(why) => panic!("the words should draw: {why}"),
    }

    let inked = inked(&pixels);

    assert!(inked > 0, "nothing was drawn");
    assert!(inked < 400, "the whole frame was filled rather than some letters: {inked}");
}

#[test]
fn two_runs_in_one_frame_are_set_in_the_two_faces_they_each_asked_for() {
    let run = |font: Font, down| {
        Shape::Words(Words {
            at: Point { across: 0, down },
            wide: WIDE,
            said: "HH".to_string(),
            weight: Weight::Plain,
            font,
            ink: colour("ffffff"),
        })
    };
    let small = Font { tall: 5, ..font() };
    let large = Font { tall: 12, ..font() };

    let mut both = slab();

    match onto(&mut both, whole(), &[run(small.clone(), 0), run(large, 8)]) {
        Ok(()) => {}
        Err(why) => panic!("both runs should draw: {why}"),
    }

    let mut twice = slab();

    match onto(&mut twice, whole(), &[run(small.clone(), 0), run(small, 8)]) {
        Ok(()) => {}
        Err(why) => panic!("both runs should draw: {why}"),
    }

    assert!(
        inked(&both) > inked(&twice),
        "the second run was drawn in the first one's face: {} against {}",
        inked(&both),
        inked(&twice)
    );
}

#[test]
fn a_run_drawn_in_the_width_it_measured_stays_on_one_line() {
    let font = Font { family: "Noto Sans".to_string(), tall: 11 };
    let said = "100%";
    let Ok(one) = measured(Run { said, weight: Weight::Plain, wide: u32::MAX }, &font);
    let points = Size { wide: one.wide, tall: one.tall.saturating_mul(3) };
    let shapes = [Shape::Words(Words {
        at: Point { across: 0, down: 0 },
        wide: one.wide,
        said: said.to_string(),
        weight: Weight::Plain,
        font,
        ink: Oklch { lightness: 1.0, chroma: 0.0, hue: 0.0 },
    })];

    for scale in [1_u32, 2, 3] {
        let device = Size {
            wide: points.wide.saturating_mul(scale),
            tall: points.tall.saturating_mul(scale),
        };
        let long = device.wide.saturating_mul(device.tall).saturating_mul(4);
        let mut pixels = vec![0_u8; usize::try_from(long).unwrap_or(0)];

        match onto(&mut pixels, Frame { device, points }, &shapes) {
            Ok(()) => {}
            Err(why) => panic!("the run should draw: {why}"),
        }

        let stride = usize::try_from(device.wide.saturating_mul(4)).unwrap_or(0);
        let under = usize::try_from(one.tall.saturating_mul(scale)).unwrap_or(0);
        let below = pixels.get(under.saturating_mul(stride)..).unwrap_or(&[]);

        assert_eq!(
            inked(below),
            0,
            "at {scale}x, {said} measured {one:?} and drew a second line under it, \
             so the width it was placed at is not the width it takes"
        );
    }
}
