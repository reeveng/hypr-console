//! What comes out of a shape, read back out of the bytes.
//!
//! There is no compositor here and no screen. A frame is a `Vec<u8>` the right
//! length, the shapes go into it, and the pixel that was asked about is read
//! out of the slice -- which is the whole claim this crate makes about itself,
//! that the drawing can be pressed on a machine with nothing to draw on.

use std::error::Error;
use std::sync::Arc;

use console_core_color::{Oklch, Rgba};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_shapes::{Edge, Font, Panel, Picture, Pixels, Round, Shape, Weight, Text};
use console_draw_painting::{Frame, Run, measure_text, onto, over};

const WIDE: u32 = 40;

const TALL: u32 = 20;

const WHOLE: Frame = Frame { device: Size { width: WIDE, height: TALL }, points: Size { width: WIDE, height: TALL } };

const NOTHING: [u8; 4] = [0, 0, 0, 0];

fn font() -> Result<Font, Never> {
    Ok(Font { family: "Noto Sans".to_string(), height: 10 })
}

fn inked_pixels(pixels: &[u8]) -> Result<u32, Never> {
    fitted(pixels.chunks_exact(4).filter(|pixel| *pixel != NOTHING).count())
}

fn slab() -> Result<Vec<u8>, Never> {
    let long = WIDE.saturating_mul(TALL).saturating_mul(4);
    let Ok(long) = index(long);

    Ok(vec![0; long])
}

fn at(pixels: &[u8], point: Point<u32>) -> Result<Option<[u8; 4]>, Never> {
    let Ok(position) = index(point.y.saturating_mul(WIDE).saturating_add(point.x));

    Ok(pixels.chunks_exact(4).nth(position).and_then(<[u8]>::first_chunk::<4>).copied())
}

fn color(six: &str) -> Result<Oklch, Box<dyn Error>> {
    let channels = Rgba::of(six).map_err(|why| format!("{six} should read: {why}"))?;
    let Ok(oklch) = Oklch::of(channels);

    Ok(oklch)
}

#[test]
fn a_panel_puts_its_color_where_it_was_put_and_nowhere_else() -> Result<(), Box<dyn Error>> {
    let red = color("ff0000")?;
    let Ok(mut pixels) = slab();
    let shapes = vec![Shape::Panel(Panel {
        at: Point { x: 10, y: 5 },
        size: Size { width: 10, height: 10 },
        round: Round(0),
        fill: red,
        edge: Edge::None,
    })];

    onto(&mut pixels, WHOLE, &shapes)?;

    assert_eq!(at(&pixels, Point { x: 15, y: 10 }), Ok(Some([0x00, 0x00, 0xff, 0xff])), "the middle of the panel");
    assert_eq!(at(&pixels, Point { x: 2, y: 2 }), Ok(Some([0, 0, 0, 0])), "outside it");

    Ok(())
}

#[test]
fn a_frame_of_device_pixels_puts_a_panel_where_the_points_said() -> Result<(), Box<dyn Error>> {
    let green = color("00ff00")?;
    let Ok(mut pixels) = slab();
    let frame = Frame {
        device: Size { width: WIDE, height: TALL },
        points: Size { width: WIDE.div_euclid(2), height: TALL.div_euclid(2) },
    };
    let shapes = vec![Shape::Panel(Panel {
        at: Point { x: 5, y: 0 },
        size: Size { width: 5, height: 5 },
        round: Round(0),
        fill: green,
        edge: Edge::None,
    })];

    onto(&mut pixels, frame, &shapes)?;

    assert_eq!(at(&pixels, Point { x: 12, y: 4 }), Ok(Some([0x00, 0xff, 0x00, 0xff])), "twice as far across and down");
    assert_eq!(at(&pixels, Point { x: 4, y: 4 }), Ok(Some([0, 0, 0, 0])), "before where the panel starts");

    Ok(())
}

#[test]
fn an_edge_is_drawn_in_its_own_color_over_the_fill() -> Result<(), Box<dyn Error>> {
    let white = color("ffffff")?;
    let black = color("000000")?;
    let Ok(mut pixels) = slab();
    let shapes = vec![Shape::Panel(Panel {
        at: Point { x: 0, y: 0 },
        size: Size { width: WIDE, height: TALL },
        round: Round(0),
        fill: black,
        edge: Edge::Of { wide: 2, color: white },
    })];

    onto(&mut pixels, WHOLE, &shapes)?;

    assert_eq!(at(&pixels, Point { x: 0, y: 10 }), Ok(Some([0xff, 0xff, 0xff, 0xff])), "on the edge");
    assert_eq!(at(&pixels, Point { x: 20, y: 10 }), Ok(Some([0x00, 0x00, 0x00, 0xff])), "well inside it");

    Ok(())
}

#[test]
fn a_run_of_words_wraps_taller_the_narrower_it_is_given() {
    let Ok(font) = font();
    let said = "the desktop has said something worth reading twice";
    let Ok(roomy) = measure_text(Run { said, weight: Weight::Plain, width: 400 }, &font);
    let Ok(tight) = measure_text(Run { said, weight: Weight::Plain, width: 80 }, &font);

    assert!(tight.height > roomy.height, "{tight:?} should be taller than {roomy:?}");
    assert!(tight.width <= 80, "a wrapped run should stay inside its width: {tight:?}");
}

#[test]
fn a_face_asked_for_in_pixels_is_drawn_that_many_pixels_tall() {
    let Ok(font) = font();
    let run = |tall| {
        let Ok(size) =
            measure_text(Run { said: "Hg", weight: Weight::Plain, width: 400 }, &Font { height: tall, ..font.clone() });

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
    let Ok(font) = font();
    let said = "notification";
    let Ok(plain) = measure_text(Run { said, weight: Weight::Plain, width: 400 }, &font);
    let Ok(bold) = measure_text(Run { said, weight: Weight::Bold, width: 400 }, &font);

    assert!(bold.width > plain.width, "bold {bold:?} should be wider than plain {plain:?}");
}

#[test]
fn words_put_ink_on_the_frame_where_nothing_was() -> Result<(), Box<dyn Error>> {
    let white = color("ffffff")?;
    let Ok(font) = font();
    let Ok(mut pixels) = slab();
    let shapes = vec![Shape::Text(Text {
        at: Point { x: 0, y: 0 },
        width: WIDE,
        said: "HHHH".to_string(),
        weight: Weight::Bold,
        font,
        ink: white,
    })];

    onto(&mut pixels, WHOLE, &shapes)?;

    let Ok(inked) = inked_pixels(&pixels);

    assert!(inked > 0, "nothing was drawn");
    assert!(inked < 400, "the whole frame was filled rather than some letters: {inked}");

    Ok(())
}

#[test]
fn two_runs_in_one_frame_are_set_in_the_two_faces_they_each_asked_for() -> Result<(), Box<dyn Error>> {
    let white = color("ffffff")?;
    let Ok(font) = font();
    let run = |font: Font, down| {
        Shape::Text(Text {
            at: Point { x: 0, y: down },
            width: WIDE,
            said: "HH".to_string(),
            weight: Weight::Plain,
            font,
            ink: white,
        })
    };
    let small = Font { height: 5, ..font.clone() };
    let large = Font { height: 12, ..font.clone() };
    let Ok(mut both) = slab();

    onto(&mut both, WHOLE, &[run(small.clone(), 0), run(large, 8)])?;

    let Ok(mut twice) = slab();

    onto(&mut twice, WHOLE, &[run(small.clone(), 0), run(small, 8)])?;

    let Ok(both) = inked_pixels(&both);
    let Ok(twice) = inked_pixels(&twice);

    assert!(both > twice, "the second run was drawn in the first one's face: {both} against {twice}");

    Ok(())
}

#[test]
fn a_run_drawn_in_the_width_it_measured_stays_on_one_line() -> Result<(), Box<dyn Error>> {
    let font = Font { family: "Noto Sans".to_string(), height: 11 };
    let said = "100%";
    let Ok(one) = measure_text(Run { said, weight: Weight::Plain, width: u32::MAX }, &font);
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
        let Ok(long) = index(long);
        let mut pixels = vec![0_u8; long];

        onto(&mut pixels, Frame { device, points }, &shapes)?;

        let stride = device.width.saturating_mul(4);
        let under = one.height.saturating_mul(scale);
        let Ok(start) = index(under.saturating_mul(stride));
        let below = pixels.get(start..).ok_or("the run drew nothing below where it measure_text")?;

        assert_eq!(
            inked_pixels(below),
            Ok(0),
            "at {scale}x, {said} measure_text {one:?} and drew a second line under it, \
             so the width it was placed at is not the width it takes"
        );
    }

    Ok(())
}

#[test]
fn a_picture_lands_where_it_was_put_in_the_colors_it_was_handed() -> Result<(), Box<dyn Error>> {
    let Ok(mut pixels) = slab();
    let red = [255u8, 0, 0, 255];
    let green = [0u8, 255, 0, 255];
    let bytes: Vec<u8> = [red, green, green, red].concat();
    let shapes = vec![Shape::Picture(Picture {
        at: Point { x: 4, y: 2 },
        size: Size { width: 2, height: 2 },
        pixels: Pixels { width: 2, height: 2, stride: 8, bytes: Arc::new(bytes) },
    })];

    onto(&mut pixels, WHOLE, &shapes)?;

    assert_eq!(at(&pixels, Point { x: 4, y: 2 }), Ok(Some([0, 0, 255, 255])), "its first pixel is where it was put");
    assert_eq!(at(&pixels, Point { x: 5, y: 2 }), Ok(Some([0, 255, 0, 255])), "and the one beside it is the next one");
    assert_eq!(at(&pixels, Point { x: 3, y: 2 }), Ok(Some([0, 0, 0, 0])), "and nothing is drawn before it");
    assert_eq!(at(&pixels, Point { x: 6, y: 2 }), Ok(Some([0, 0, 0, 0])), "or past the size it was given");

    Ok(())
}

#[test]
fn a_picture_that_is_partly_see_through_is_drawn_over_what_was_under_it() -> Result<(), Box<dyn Error>> {
    let white = color("ffffff")?;
    let Ok(mut pixels) = slab();
    let shapes = vec![
        Shape::Panel(Panel {
            at: Point { x: 0, y: 0 },
            size: Size { width: WIDE, height: TALL },
            round: Round(0),
            fill: white,
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

    onto(&mut pixels, WHOLE, &shapes)?;

    let Ok(corner) = at(&pixels, Point { x: 0, y: 0 });
    let [blue, green, red, alpha] = corner.ok_or("the corner is outside the frame")?;

    assert_eq!(alpha, 255, "what was under it is still opaque");
    assert!(blue > 200 && green > 200 && red > 200, "and still the color it was");

    Ok(())
}

#[test]
fn a_picture_given_a_bigger_box_than_itself_fills_the_box() -> Result<(), Box<dyn Error>> {
    let Ok(mut pixels) = slab();
    let red = [255u8, 0, 0, 255];
    let shapes = vec![Shape::Picture(Picture {
        at: Point { x: 0, y: 0 },
        size: Size { width: 8, height: 8 },
        pixels: Pixels { width: 2, height: 2, stride: 8, bytes: Arc::new([red; 4].concat()) },
    })];

    onto(&mut pixels, WHOLE, &shapes)?;

    let Ok(corner) = at(&pixels, Point { x: 6, y: 6 });
    let [_blue, _green, red, alpha] = corner.ok_or("the far corner is outside the frame")?;

    assert_eq!(alpha, 255, "the far corner of the box was drawn on");
    assert!(red > 200, "in the color the picture is");
    assert_eq!(at(&pixels, Point { x: 9, y: 9 }), Ok(Some([0, 0, 0, 0])), "and nothing past the box was");

    Ok(())
}

fn red_square(across: i32) -> Result<Shape, Box<dyn Error>> {
    let red = color("ff0000")?;

    Ok(Shape::Panel(Panel {
        at: Point { x: across, y: 5 },
        size: Size { width: 10, height: 10 },
        round: Round(0),
        fill: red,
        edge: Edge::None,
    }))
}

#[test]
fn a_frame_drawn_over_keeps_what_was_there_and_one_drawn_onto_does_not() -> Result<(), Box<dyn Error>> {
    let Ok(mut pixels) = slab();
    let first = red_square(0)?;
    let second = red_square(25)?;

    onto(&mut pixels, WHOLE, std::slice::from_ref(&first))?;
    over(&mut pixels, WHOLE, std::slice::from_ref(&second))?;

    assert_eq!(at(&pixels, Point { x: 5, y: 10 }), Ok(Some([0x00, 0x00, 0xff, 0xff])), "what was there is kept");
    assert_eq!(at(&pixels, Point { x: 30, y: 10 }), Ok(Some([0x00, 0x00, 0xff, 0xff])), "what was drawn over it is there");

    onto(&mut pixels, WHOLE, std::slice::from_ref(&second))?;

    assert_eq!(at(&pixels, Point { x: 5, y: 10 }), Ok(Some([0, 0, 0, 0])), "a whole frame starts from nothing");

    Ok(())
}
