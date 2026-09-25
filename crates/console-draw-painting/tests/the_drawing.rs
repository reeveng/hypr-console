//! What comes out of a shape, read back out of the bytes.
//!
//! There is no compositor here and no screen. A frame is a `Vec<u8>` the right
//! length, the shapes go into it, and the pixel that was asked about is read
//! out of the slice -- which is the whole claim this crate makes about itself,
//! that the drawing can be pressed on a machine with nothing to draw on.

use console_core_color::{Oklch, Rgba};
use console_core_geometry::{Point, Size};
use std::sync::Arc;

use console_core_shapes::{Edge, Font, Panel, Picture, Pixels, Round, Shape, Weight, Text};
use console_draw_painting::{Frame, Run, measured, onto, over};

const WIDE: u32 = 40;

const TALL: u32 = 20;

fn font() -> Font {
    Font { family: "Noto Sans".to_string(), height: 10 }
}

fn inked(pixels: &[u8]) -> u32 {
    u32::try_from(pixels.chunks_exact(4).filter(|pixel| pixel.iter().any(|byte| *byte > 0)).count()).unwrap()
}

fn slab() -> Vec<u8> {
    let long = WIDE.saturating_mul(TALL).saturating_mul(4);

    vec![0; long.try_into().unwrap()]
}

fn at(pixels: &[u8], across: u32, down: u32) -> [u8; 4] {
    let stride = WIDE.saturating_mul(4);
    let start = down.saturating_mul(stride).saturating_add(across.saturating_mul(4));
    let held = pixels.get(start.try_into().unwrap()..start.saturating_add(4).try_into().unwrap()).unwrap_or(&[]);

    match held {
        [blue, green, red, alpha] => [*blue, *green, *red, *alpha],
        _ => [0, 0, 0, 0],
    }
}

fn color(six: &str) -> Oklch {
    let channels = match Rgba::of(six) {
        Ok(channels) => channels,
        Err(why) => panic!("{six} should read: {why}"),
    };
    let Ok(oklch) = Oklch::of(channels);

    oklch
}

fn whole() -> Frame {
    Frame { device: Size { width: WIDE, height: TALL }, points: Size { width: WIDE, height: TALL } }
}

#[test]
fn a_panel_puts_its_color_where_it_was_put_and_nowhere_else() {
    let mut pixels = slab();
    let shapes = vec![Shape::Panel(Panel {
        at: Point { x: 10, y: 5 },
        size: Size { width: 10, height: 10 },
        round: Round(0),
        fill: color("ff0000"),
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
        device: Size { width: WIDE, height: TALL },
        points: Size { width: WIDE / 2, height: TALL / 2 },
    };
    let shapes = vec![Shape::Panel(Panel {
        at: Point { x: 5, y: 0 },
        size: Size { width: 5, height: 5 },
        round: Round(0),
        fill: color("00ff00"),
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
fn an_edge_is_drawn_in_its_own_color_over_the_fill() {
    let mut pixels = slab();
    let shapes = vec![Shape::Panel(Panel {
        at: Point { x: 0, y: 0 },
        size: Size { width: WIDE, height: TALL },
        round: Round(0),
        fill: color("000000"),
        edge: Edge::Of { wide: 2, color: color("ffffff") },
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
    let Ok(roomy) = measured(Run { said, weight: Weight::Plain, width: 400 }, &font());
    let Ok(tight) = measured(Run { said, weight: Weight::Plain, width: 80 }, &font());

    assert!(tight.height > roomy.height, "{tight:?} should be taller than {roomy:?}");
    assert!(tight.width <= 80, "a wrapped run should stay inside its width: {tight:?}");
}

#[test]
fn a_face_asked_for_in_pixels_is_drawn_that_many_pixels_tall() {
    let run = |tall| {
        let Ok(size) =
            measured(Run { said: "Hg", weight: Weight::Plain, width: 400 }, &Font { height: tall, ..font() });

        size
    };
    let one = run(20);
    let twice = run(40);

    assert!(one.height >= 20, "a line set at twenty pixels came out {one:?}");
    assert!(one.height < 34, "a line set at twenty pixels came out {one:?}, which is points");
    assert!(
        twice.width.abs_diff(one.width.saturating_mul(2)) <= 2,
        "twice the height should be about twice the width: {one:?} against {twice:?}"
    );
}

#[test]
fn bold_is_not_the_same_run_as_plain() {
    let said = "notification";
    let Ok(plain) = measured(Run { said, weight: Weight::Plain, width: 400 }, &font());
    let Ok(bold) = measured(Run { said, weight: Weight::Bold, width: 400 }, &font());

    assert!(bold.width > plain.width, "bold {bold:?} should be wider than plain {plain:?}");
}

#[test]
fn words_put_ink_on_the_frame_where_nothing_was() {
    let mut pixels = slab();
    let shapes = vec![Shape::Text(Text {
        at: Point { x: 0, y: 0 },
        width: WIDE,
        said: "HHHH".to_string(),
        weight: Weight::Bold,
        font: font(),
        ink: color("ffffff"),
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
        Shape::Text(Text {
            at: Point { x: 0, y: down },
            width: WIDE,
            said: "HH".to_string(),
            weight: Weight::Plain,
            font,
            ink: color("ffffff"),
        })
    };
    let small = Font { height: 5, ..font() };
    let large = Font { height: 12, ..font() };

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
    let font = Font { family: "Noto Sans".to_string(), height: 11 };
    let said = "100%";
    let Ok(one) = measured(Run { said, weight: Weight::Plain, width: u32::MAX }, &font);
    let points = Size { width: one.width, height: one.height.saturating_mul(3) };
    let shapes = [Shape::Text(Text {
        at: Point { x: 0, y: 0 },
        width: one.width,
        said: said.to_string(),
        weight: Weight::Plain,
        font,
        ink: Oklch { lightness: 1.0, chroma: 0.0, hue: 0.0 },
    })];

    for scale in [1_u32, 2, 3] {
        let device = Size {
            width: points.width.saturating_mul(scale),
            height: points.height.saturating_mul(scale),
        };
        let long = device.width.saturating_mul(device.height).saturating_mul(4);
        let mut pixels = vec![0_u8; long.try_into().unwrap()];

        match onto(&mut pixels, Frame { device, points }, &shapes) {
            Ok(()) => {}
            Err(why) => panic!("the run should draw: {why}"),
        }

        let stride = device.width.saturating_mul(4);
        let under = one.height.saturating_mul(scale);
        let below = pixels.get(under.saturating_mul(stride).try_into().unwrap()..).unwrap_or(&[]);

        assert_eq!(
            inked(below),
            0,
            "at {scale}x, {said} measured {one:?} and drew a second line under it, \
             so the width it was placed at is not the width it takes"
        );
    }
}

#[test]
fn a_picture_lands_where_it_was_put_in_the_colors_it_was_handed() {
    let mut pixels = slab();
    let red = [255u8, 0, 0, 255];
    let green = [0u8, 255, 0, 255];
    let bytes: Vec<u8> = [red, green, green, red].concat();
    let shapes = vec![Shape::Picture(Picture {
        at: Point { x: 4, y: 2 },
        size: Size { width: 2, height: 2 },
        pixels: Pixels { width: 2, height: 2, stride: 8, bytes: Arc::new(bytes) },
    })];

    match onto(&mut pixels, whole(), &shapes) {
        Ok(()) => {},
        Err(why) => panic!("the picture should draw: {why}"),
    }

    assert_eq!(at(&pixels, 4, 2), [0, 0, 255, 255], "its first pixel is where it was put");
    assert_eq!(at(&pixels, 5, 2), [0, 255, 0, 255], "and the one beside it is the next one");
    assert_eq!(at(&pixels, 3, 2), [0, 0, 0, 0], "and nothing is drawn before it");
    assert_eq!(at(&pixels, 6, 2), [0, 0, 0, 0], "or past the size it was given");
}

#[test]
fn a_picture_that_is_partly_see_through_is_drawn_over_what_was_under_it() {
    let mut pixels = slab();
    let shapes = vec![
        Shape::Panel(Panel {
            at: Point { x: 0, y: 0 },
            size: Size { width: WIDE, height: TALL },
            round: Round(0),
            fill: color("ffffff"),
            edge: Edge::None,
        }),
        Shape::Picture(Picture {
            at: Point { x: 0, y: 0 },
            size: Size { width: 2, height: 2 },
            pixels: Pixels {
                width: 2,
                height: 2,
                stride: 8,
                bytes: Arc::new([[0u8, 0, 0, 0]; 4].concat()),
            },
        }),
    ];

    match onto(&mut pixels, whole(), &shapes) {
        Ok(()) => {},
        Err(why) => panic!("the picture should draw: {why}"),
    }

    let [blue, green, red, alpha] = at(&pixels, 0, 0);

    assert_eq!(alpha, 255, "what was under it is still opaque");
    assert!(blue > 200 && green > 200 && red > 200, "and still the color it was");
}

#[test]
fn a_picture_given_a_bigger_box_than_itself_fills_the_box() {
    let mut pixels = slab();
    let red = [255u8, 0, 0, 255];
    let shapes = vec![Shape::Picture(Picture {
        at: Point { x: 0, y: 0 },
        size: Size { width: 8, height: 8 },
        pixels: Pixels { width: 2, height: 2, stride: 8, bytes: Arc::new([red; 4].concat()) },
    })];

    match onto(&mut pixels, whole(), &shapes) {
        Ok(()) => {},
        Err(why) => panic!("the picture should draw: {why}"),
    }

    let [_blue, _green, red, alpha] = at(&pixels, 6, 6);

    assert_eq!(alpha, 255, "the far corner of the box was drawn on");
    assert!(red > 200, "in the color the picture is");
    assert_eq!(at(&pixels, 9, 9), [0, 0, 0, 0], "and nothing past the box was");
}

fn red_square(across: i32) -> Shape {
    Shape::Panel(Panel {
        at: Point { x: across, y: 5 },
        size: Size { width: 10, height: 10 },
        round: Round(0),
        fill: color("ff0000"),
        edge: Edge::None,
    })
}

#[test]
fn a_frame_drawn_over_keeps_what_was_there_and_one_drawn_onto_does_not() {
    let mut pixels = slab();

    match onto(&mut pixels, whole(), &[red_square(0)]) {
        Ok(()) => {}
        Err(why) => panic!("the first square should draw: {why}"),
    }

    match over(&mut pixels, whole(), &[red_square(25)]) {
        Ok(()) => {}
        Err(why) => panic!("the second square should draw: {why}"),
    }

    assert_eq!(at(&pixels, 5, 10), [0x00, 0x00, 0xff, 0xff], "what was there is kept");
    assert_eq!(at(&pixels, 30, 10), [0x00, 0x00, 0xff, 0xff], "what was drawn over it is there");

    match onto(&mut pixels, whole(), &[red_square(25)]) {
        Ok(()) => {}
        Err(why) => panic!("the square should draw again: {why}"),
    }

    assert_eq!(at(&pixels, 5, 10), [0, 0, 0, 0], "a whole frame starts from nothing");
}
