//! What comes out of a shape, read back out of the bytes.
//!
//! There is no compositor here and no screen. A frame is a `Vec<u8>` the right
//! length, the shapes go into it, and the pixel that was asked about is read
//! out of the slice -- which is the whole claim this crate makes about itself,
//! that the drawing can be pressed on a machine with nothing to draw on.

use std::error::Error;

use console_core_color::{Oklch, Rgba};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_path_rasterizer::Pixmap;
use console_core_shapes::{Edge, Font, Panel, Picture, Pixels, Round, Shape, Text, Weight};
use console_draw_painting::{Frame, Run, measure_text, onto, over, over_picture};

const WIDE: u32 = 40;

const TALL: u32 = 20;

const WHOLE: Frame = Frame {
  device: Size {
    width: WIDE,
    height: TALL,
  },
  points: Size {
    width: WIDE,
    height: TALL,
  },
};

const NOTHING: [u8; 4] = [0, 0, 0, 0];

fn font() -> Result<Font, Never> {
  Ok(Font {
    family: "Noto Sans".to_string(),
    height: 10,
  })
}

fn inked_pixels(pixels: &[u8]) -> Result<u32, Never> {
  fitted(
    pixels
      .as_chunks::<4>()
      .0
      .iter()
      .filter(|pixel| **pixel != NOTHING)
      .count(),
  )
}

#[cfg_attr(
  dylint_lib = "explicit088_no_arriving_length",
  expect(
    explicit088_no_arriving_length,
    reason = "the slab allocation is sized by the fixed WIDE and TALL test dimensions"
  )
)]
fn slab() -> Result<Vec<u8>, Never> {
  let long = WIDE.saturating_mul(TALL).saturating_mul(4);
  let Ok(long_2) = index(long);

  Ok(vec![0; long_2])
}

fn at(pixels: &[u8], point: Point<u32>) -> Result<Option<[u8; 4]>, Never> {
  let Ok(position) = index(point.y.saturating_mul(WIDE).saturating_add(point.x));

  Ok(pixels.as_chunks::<4>().0.get(position).copied())
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
    size: Size {
      width: 10,
      height: 10,
    },
    round: Round(0),
    fill: red,
    edge: Edge::None,
  })];

  onto(&mut pixels, WHOLE, &shapes)?;

  assert_eq!(
    at(&pixels, Point { x: 15, y: 10 }),
    Ok(Some([0x00, 0x00, 0xff, 0xff])),
    "the middle of the panel"
  );
  assert_eq!(
    at(&pixels, Point { x: 2, y: 2 }),
    Ok(Some([0, 0, 0, 0])),
    "outside it"
  );

  Ok(())
}

#[test]
fn a_frame_of_device_pixels_puts_a_panel_where_the_points_said() -> Result<(), Box<dyn Error>> {
  let green = color("00ff00")?;
  let Ok(mut pixels) = slab();
  let frame = Frame {
    device: Size {
      width: WIDE,
      height: TALL,
    },
    points: Size {
      width: WIDE.div_euclid(2),
      height: TALL.div_euclid(2),
    },
  };
  let shapes = vec![Shape::Panel(Panel {
    at: Point { x: 5, y: 0 },
    size: Size {
      width: 5,
      height: 5,
    },
    round: Round(0),
    fill: green,
    edge: Edge::None,
  })];

  onto(&mut pixels, frame, &shapes)?;

  assert_eq!(
    at(&pixels, Point { x: 12, y: 4 }),
    Ok(Some([0x00, 0xff, 0x00, 0xff])),
    "twice as far across and down"
  );
  assert_eq!(
    at(&pixels, Point { x: 4, y: 4 }),
    Ok(Some([0, 0, 0, 0])),
    "before where the panel starts"
  );

  Ok(())
}

#[test]
fn an_edge_is_drawn_in_its_own_color_over_the_fill() -> Result<(), Box<dyn Error>> {
  let white = color("ffffff")?;
  let black = color("000000")?;
  let Ok(mut pixels) = slab();
  let shapes = vec![Shape::Panel(Panel {
    at: Point { x: 0, y: 0 },
    size: Size {
      width: WIDE,
      height: TALL,
    },
    round: Round(0),
    fill: black,
    edge: Edge::Of {
      wide: 2,
      color: white,
    },
  })];

  onto(&mut pixels, WHOLE, &shapes)?;

  assert_eq!(
    at(&pixels, Point { x: 0, y: 10 }),
    Ok(Some([0xff, 0xff, 0xff, 0xff])),
    "on the edge"
  );
  assert_eq!(
    at(&pixels, Point { x: 20, y: 10 }),
    Ok(Some([0x00, 0x00, 0x00, 0xff])),
    "well inside it"
  );

  Ok(())
}

#[test]
fn a_run_of_words_wraps_taller_the_narrower_it_is_given() {
  let Ok(font) = font();
  let said = "the desktop has said something worth reading twice";
  let Ok(roomy) = measure_text(
    Run {
      said,
      weight: Weight::Plain,
      width: 400,
    },
    &font,
  );
  let Ok(tight) = measure_text(
    Run {
      said,
      weight: Weight::Plain,
      width: 80,
    },
    &font,
  );

  assert!(
    tight.height > roomy.height,
    "{tight:?} should be taller than {roomy:?}"
  );

  assert!(
    tight.width <= 80,
    "a wrapped run should stay inside its width: {tight:?}"
  )
}

#[test]
fn wrapping_a_long_word_keeps_every_unicode_character_in_reading_order() {
  let Ok(font) = font();

  for said in [
    "abcdefghijklmnopqrstuv",
    "العربيةالعربيةالعربية",
    "aé日🙂bcé日🙂def",
  ] {
    let Ok(lines) = console_draw_painting::wrapped(
      Run {
        said,
        weight: Weight::Plain,
        width: 25,
      },
      &font,
    );

    assert!(lines.len() > 1, "the word should wrap: {lines:?}");
    assert_eq!(
      lines.concat(),
      said,
      "wrapping lost or reordered text: {lines:?}"
    );
  }
}

#[test]
fn explicit_blank_lines_survive_wrapping() {
  let Ok(font) = font();
  let Ok(lines) = console_draw_painting::wrapped(
    Run {
      said: "first\n\nlast",
      weight: Weight::Plain,
      width: 400,
    },
    &font,
  );

  assert_eq!(lines, ["first", "", "last"])
}

#[test]
fn text_respects_the_same_clip_as_the_shapes_around_it() -> Result<(), Box<dyn Error>> {
  let Ok(mut pixels) = slab();
  let Ok(font) = font();
  let shapes = [
    Shape::Clip(console_core_shapes::Clip::To {
      at: Point { x: 10, y: 0 },
      size: Size {
        width: 10,
        height: TALL,
      },
    }),
    Shape::Text(Text {
      at: Point { x: 0, y: 0 },
      width: WIDE,
      said: "HHHHHHHH".to_string(),
      weight: Weight::Bold,
      font,
      ink: Oklch::WHITE,
    }),
  ];

  onto(&mut pixels, WHOLE, &shapes)?;

  let Ok(inked) = inked_pixels(&pixels);

  assert!(inked > 0, "the clipped text still draws");

  for x in (0..10).chain(20..WIDE) {
    for y in 0..TALL {
      assert_eq!(
        at(&pixels, Point { x, y }),
        Ok(Some(NOTHING)),
        "text escaped its clip at {x},{y}"
      );
    }
  }

  Ok(())
}

#[test]
fn a_face_asked_for_in_pixels_is_drawn_that_many_pixels_tall() {
  let Ok(font) = font();
  let run = |tall| {
    let Ok(size) = measure_text(
      Run {
        said: "Hg",
        weight: Weight::Plain,
        width: 400,
      },
      &Font {
        height: tall,
        ..font.clone()
      },
    );

    size
  };
  let one = run(20);
  let twice = run(40);

  assert!(
    one.height >= 20,
    "a line set at twenty pixels came out {one:?}"
  );
  assert!(
    one.height < 34,
    "a line set at twenty pixels came out {one:?}, which is points"
  );

  assert!(
    twice.width.abs_diff(one.width.saturating_mul(2)) <= 2,
    "twice the height should be about twice the width: {one:?} against {twice:?}"
  )
}

#[test]
fn bold_is_not_the_same_run_as_plain() {
  let Ok(font) = font();
  let said = "notification";
  let Ok(plain) = measure_text(
    Run {
      said,
      weight: Weight::Plain,
      width: 400,
    },
    &font,
  );
  let Ok(bold) = measure_text(
    Run {
      said,
      weight: Weight::Bold,
      width: 400,
    },
    &font,
  );

  assert!(
    bold.width > plain.width,
    "bold {bold:?} should be wider than plain {plain:?}"
  )
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
  assert!(
    inked < 400,
    "the whole frame was filled rather than some letters: {inked}"
  );

  Ok(())
}

#[test]
fn two_runs_in_one_frame_are_set_in_the_two_faces_they_each_asked_for()
-> Result<(), Box<dyn Error>> {
  let white = color("ffffff")?;
  let Ok(font) = font();
  let run = |font_2: Font, down| {
    Shape::Text(Text {
      at: Point { x: 0, y: down },
      width: WIDE,
      said: "HH".to_string(),
      weight: Weight::Plain,
      font: font_2,
      ink: white,
    })
  };
  let small = Font {
    height: 5,
    ..font.clone()
  };
  let large = Font {
    height: 12,
    ..font.clone()
  };
  let Ok(mut both) = slab();

  onto(&mut both, WHOLE, &[run(small.clone(), 0), run(large, 8)])?;

  let Ok(mut twice) = slab();

  onto(&mut twice, WHOLE, &[run(small.clone(), 0), run(small, 8)])?;

  let Ok(mixed_count) = inked_pixels(&both);
  let Ok(small_count) = inked_pixels(&twice);

  assert!(
    mixed_count > small_count,
    "the second run was drawn in the first one's face: {mixed_count} against {small_count}"
  );

  Ok(())
}

#[test]
fn a_run_drawn_in_the_width_it_measured_stays_on_one_line() -> Result<(), Box<dyn Error>> {
  let font = Font {
    family: "Noto Sans".to_string(),
    height: 11,
  };
  let said = "100%";
  let Ok(one) = measure_text(
    Run {
      said,
      weight: Weight::Plain,
      width: u32::MAX,
    },
    &font,
  );
  let points = Size {
    width: one.width,
    height: one.height.saturating_mul(3),
  };
  let shapes = [Shape::Text(Text {
    at: Point { x: 0, y: 0 },
    width: one.width,
    said: said.to_string(),
    weight: Weight::Plain,
    font,
    ink: Oklch::WHITE,
  })];

  for scale in [1_u32, 2, 3] {
    let device = Size {
      width: points.width.saturating_mul(scale),
      height: points.height.saturating_mul(scale),
    };
    let Ok(pixmap) = Pixmap::new(device.width, device.height);
    let frame = pixmap.ok_or("the test frame dimensions do not fit a pixmap")?;
    let Ok(mut pixels) = frame.take();

    onto(&mut pixels, Frame { device, points }, &shapes)?;

    let stride = device.width.saturating_mul(4);
    let under = one.height.saturating_mul(scale);
    let Ok(start) = index(under.saturating_mul(stride));
    let below = pixels
      .get(start..)
      .ok_or("the run drew nothing below where it measure_text")?;

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
  let Ok(picture) = Picture::rectangular(
    Point { x: 4, y: 2 },
    Size {
      width: 2,
      height: 2,
    },
    Pixels {
      width: 2,
      height: 2,
      stride: 8,
      bytes,
    },
  );
  let Ok(mut borrowed) = slab();
  let Ok(borrowed_picture) = picture.borrow_pixels();

  over_picture(&mut borrowed, WHOLE, borrowed_picture, &[])?;
  onto(&mut pixels, WHOLE, &[Shape::Picture(picture)])?;

  assert_eq!(borrowed, pixels);
  assert_eq!(
    at(&pixels, Point { x: 4, y: 2 }),
    Ok(Some([0, 0, 255, 255])),
    "its first pixel is where it was put"
  );
  assert_eq!(
    at(&pixels, Point { x: 5, y: 2 }),
    Ok(Some([0, 255, 0, 255])),
    "and the one beside it is the next one"
  );
  assert_eq!(
    at(&pixels, Point { x: 3, y: 2 }),
    Ok(Some([0, 0, 0, 0])),
    "and nothing is drawn before it"
  );
  assert_eq!(
    at(&pixels, Point { x: 6, y: 2 }),
    Ok(Some([0, 0, 0, 0])),
    "or past the size it was given"
  );

  Ok(())
}

#[test]
fn a_round_picture_preserves_the_background_outside_its_circle() -> Result<(), Box<dyn Error>> {
  use console_core_geometry::Circle;
  use console_core_shapes::PictureGeometry;

  let background = [40, 50, 60, 255];
  let mut pixels = background.repeat(800);
  let frame = Frame {
    device: WHOLE.device,
    points: Size {
      width: TALL,
      height: TALL,
    },
  };
  let source = Pixels {
    width: 1,
    height: 1,
    stride: 4,
    bytes: vec![255, 0, 0, 255],
  };
  let picture = Picture {
    geometry: PictureGeometry::Circle(Circle {
      center: Point { x: 10.0, y: 10.0 },
      radius: 10.0,
    }),
    pixels: &source,
  };

  over_picture(&mut pixels, frame, picture, &[])?;

  assert_eq!(at(&pixels, Point { x: 1, y: 1 }), Ok(Some(background)));
  assert_eq!(
    at(&pixels, Point { x: 20, y: 10 }),
    Ok(Some([0, 0, 255, 255]))
  );
  Ok(())
}

#[test]
fn a_cropped_triangular_picture_respects_its_outline_and_the_active_clip()
-> Result<(), Box<dyn Error>> {
  use console_core_geometry::Triangle;
  use console_core_shapes::{Clip, Cropped, PictureGeometry};

  let Ok(mut pixels) = slab();
  let picture = Picture {
    geometry: PictureGeometry::Triangle(Triangle {
      vertices: [
        Point { x: 4.0, y: 2.0 },
        Point { x: 14.0, y: 2.0 },
        Point { x: 4.0, y: 12.0 },
      ],
    }),
    pixels: Pixels {
      width: 2,
      height: 1,
      stride: 8,
      bytes: vec![255, 0, 0, 255, 0, 255, 0, 255],
    },
  };
  let shapes = [
    Shape::Clip(Clip::To {
      at: Point { x: 9, y: 2 },
      size: Size {
        width: 5,
        height: 10,
      },
    }),
    Shape::Cropped(Cropped {
      picture,
      from: Point { x: 1.0, y: 0.0 },
      seen: Size {
        width: 1.0,
        height: 1.0,
      },
    }),
  ];

  onto(&mut pixels, WHOLE, &shapes)?;

  assert_eq!(
    at(&pixels, Point { x: 9, y: 3 }),
    Ok(Some([0, 255, 0, 255]))
  );
  assert_eq!(at(&pixels, Point { x: 7, y: 3 }), Ok(Some(NOTHING)));
  assert_eq!(at(&pixels, Point { x: 12, y: 9 }), Ok(Some(NOTHING)));
  Ok(())
}

#[test]
fn a_full_frame_picture_matches_the_rasterizer_before_drawing_the_overlay()
-> Result<(), Box<dyn Error>> {
  use console_core_shapes::Cropped;

  let overlay = red_square(10)?;

  for alpha in [255, 128] {
    let mut bytes = [200_u8, 100, 50, 255].repeat(800);

    match bytes.last_mut() {
      Some(last) => {
        *last = alpha;
      }
      ::core::option::Option::None => {}
    }

    let pixels = Pixels {
      width: WIDE,
      height: TALL,
      stride: WIDE.saturating_mul(4),
      bytes,
    };
    let origin = Point { x: 0, y: 0 };
    let mut actual = [50_u8, 100, 200, 255].repeat(801);
    let mut expected = actual.clone();
    let Ok(picture) = Picture::rectangular(origin, WHOLE.points, pixels);
    let Ok(borrowed) = picture.borrow_pixels();
    over_picture(&mut actual, WHOLE, borrowed, std::slice::from_ref(&overlay))?;
    over(
      &mut expected,
      WHOLE,
      &[
        Shape::Cropped(Cropped {
          picture,
          from: Point { x: 0.0, y: 0.0 },
          seen: Size {
            width: f64::from(WIDE),
            height: f64::from(TALL),
          },
        }),
        overlay.clone(),
      ],
    )?;
    assert_eq!(actual, expected, "final source pixel alpha {alpha}");
  }

  Ok(())
}

#[test]
fn a_scaled_full_frame_matches_the_bilinear_rasterizer() -> Result<(), Box<dyn Error>> {
  for (from, to) in [
    ((7, 5), (11, 8)),
    ((7, 5), (4, 3)),
    ((1, 5), (7, 9)),
    ((7, 1), (11, 8)),
    ((11, 8), (11, 8)),
  ] {
    let source_size = Size {
      width: from.0,
      height: from.1,
    };
    let frame = Frame {
      points: Size {
        width: 7,
        height: 5,
      },
      device: Size {
        width: to.0,
        height: to.1,
      },
    };

    for alpha in [255, 128] {
      compare_scaled_frame(source_size, frame, alpha)?;
    }
  }

  Ok(())
}

fn checkerboard(size: Size<u32>, edge_alpha: u8) -> Result<Pixels, Box<dyn Error>> {
  let mut bytes = Vec::new();

  for y in 0..size.height {
    for x in 0..size.width {
      let Ok(red) = fitted(x.saturating_mul(31));
      let Ok(green) = fitted(y.saturating_mul(43));
      let blue = match x.wrapping_add(y) & 1 {
        0 => 255,
        _ => 0,
      };
      bytes.extend_from_slice(&[red, green, blue, 255]);
    }
  }

  let edge = bytes.last_mut().ok_or("empty source fixture")?;
  *edge = edge_alpha;

  Ok(Pixels {
    width: size.width,
    height: size.height,
    stride: size.width.saturating_mul(4),
    bytes,
  })
}

fn compare_scaled_frame(from: Size<u32>, frame: Frame, alpha: u8) -> Result<(), Box<dyn Error>> {
  use console_core_shapes::Cropped;

  let origin = Point { x: 0, y: 0 };
  let source = checkerboard(from, alpha)?;
  let area = frame.device.width.saturating_mul(frame.device.height);
  let Ok(pixels) = index(area);
  let mut actual = [20_u8, 30, 40, 255].repeat(pixels);
  let mut expected = actual.clone();
  let Ok(picture) = Picture::rectangular(origin, frame.points, source);
  let Ok(borrowed) = picture.borrow_pixels();
  over_picture(&mut actual, frame, borrowed, &[])?;
  over(
    &mut expected,
    frame,
    &[Shape::Cropped(Cropped {
      picture,
      from: Point { x: 0.0, y: 0.0 },
      seen: Size {
        width: f64::from(from.width),
        height: f64::from(from.height),
      },
    })],
  )?;

  match alpha {
    255 => {
      for (painted, reference) in actual
        .as_chunks::<4>()
        .0
        .iter()
        .zip(expected.as_chunks::<4>().0)
      {
        assert!(
          painted
            .iter()
            .zip(reference)
            .all(|(&a, &b)| a.abs_diff(b) <= 1),
          "{from:?} -> {:?}: {painted:?} differed from {reference:?}",
          frame.device
        );
      }
    }
    _ => assert_eq!(
      actual, expected,
      "{from:?} -> {:?} with partial alpha",
      frame.device
    ),
  }

  Ok(())
}

#[test]
fn a_picture_that_is_partly_see_through_is_drawn_over_what_was_under_it()
-> Result<(), Box<dyn Error>> {
  let white = color("ffffff")?;
  let Ok(mut pixels) = slab();
  let Ok(picture) = Picture::rectangular(
    Point::default(),
    Size {
      width: 2,
      height: 2,
    },
    Pixels {
      width: 2,
      height: 2,
      stride: 8,
      bytes: [[0u8, 0, 0, 0]; 4].concat(),
    },
  );
  let shapes = vec![
    Shape::Panel(Panel {
      at: Point { x: 0, y: 0 },
      size: Size {
        width: WIDE,
        height: TALL,
      },
      round: Round(0),
      fill: white,
      edge: Edge::None,
    }),
    Shape::Picture(picture),
  ];

  onto(&mut pixels, WHOLE, &shapes)?;

  let Ok(corner) = at(&pixels, Point { x: 0, y: 0 });
  let [blue, green, red, alpha] = corner.ok_or("the corner is outside the frame")?;

  assert_eq!(alpha, 255, "what was under it is still opaque");
  assert!(
    blue > 200 && green > 200 && red > 200,
    "and still the color it was"
  );

  Ok(())
}

#[test]
fn a_picture_given_a_bigger_box_than_itself_fills_the_box() -> Result<(), Box<dyn Error>> {
  let Ok(mut pixels) = slab();
  let red = [255u8, 0, 0, 255];
  let Ok(picture) = Picture::rectangular(
    Point::default(),
    Size {
      width: 8,
      height: 8,
    },
    Pixels {
      width: 2,
      height: 2,
      stride: 8,
      bytes: [red; 4].concat(),
    },
  );
  let shapes = [Shape::Picture(picture)];

  onto(&mut pixels, WHOLE, &shapes)?;

  let Ok(corner) = at(&pixels, Point { x: 6, y: 6 });
  let [_blue, _green, corner_red, alpha] = corner.ok_or("the far corner is outside the frame")?;

  assert_eq!(alpha, 255, "the far corner of the box was drawn on");
  assert!(corner_red > 200, "in the color the picture is");
  assert_eq!(
    at(&pixels, Point { x: 9, y: 9 }),
    Ok(Some([0, 0, 0, 0])),
    "and nothing past the box was"
  );

  Ok(())
}

fn red_square(across: i32) -> Result<Shape, Box<dyn Error>> {
  let red = color("ff0000")?;

  Ok(Shape::Panel(Panel {
    at: Point { x: across, y: 5 },
    size: Size {
      width: 10,
      height: 10,
    },
    round: Round(0),
    fill: red,
    edge: Edge::None,
  }))
}

#[test]
fn a_frame_drawn_over_keeps_what_was_there_and_one_drawn_onto_does_not()
-> Result<(), Box<dyn Error>> {
  let Ok(mut pixels) = slab();
  let first = red_square(0)?;
  let second = red_square(25)?;

  onto(&mut pixels, WHOLE, std::slice::from_ref(&first))?;
  over(&mut pixels, WHOLE, std::slice::from_ref(&second))?;

  assert_eq!(
    at(&pixels, Point { x: 5, y: 10 }),
    Ok(Some([0x00, 0x00, 0xff, 0xff])),
    "what was there is kept"
  );
  assert_eq!(
    at(&pixels, Point { x: 30, y: 10 }),
    Ok(Some([0x00, 0x00, 0xff, 0xff])),
    "what was drawn over it is there"
  );

  onto(&mut pixels, WHOLE, std::slice::from_ref(&second))?;

  assert_eq!(
    at(&pixels, Point { x: 5, y: 10 }),
    Ok(Some([0, 0, 0, 0])),
    "a whole frame starts from nothing"
  );

  Ok(())
}

#[test]
fn a_short_frame_is_rejected_before_any_pixel_changes() {
  let mut pixels = [17_u8; 8];
  let result = onto(&mut pixels, WHOLE, &[]);

  assert!(matches!(
    result,
    Err(console_draw_painting::Cannot::Buffer { .. })
  ));

  assert_eq!(pixels, [17; 8])
}

#[test]
fn a_failed_picture_keeps_the_preserved_buffers_channel_order() {
  let pixel = [50_u8, 100, 200, 255];
  let mut pixels = pixel.repeat(800);
  let before = pixels.clone();
  let Ok(picture) = Picture::rectangular(
    Point::default(),
    Size {
      width: 2,
      height: 2,
    },
    Pixels {
      width: 2,
      height: 2,
      stride: 8,
      bytes: vec![0; 3],
    },
  );
  let shapes = [Shape::Picture(picture)];

  assert!(matches!(
    over(&mut pixels, WHOLE, &shapes),
    Err(console_draw_painting::Cannot::Pixels { .. })
  ));

  assert_eq!(pixels, before)
}

#[test]
fn pictures_keep_partial_alpha_and_skip_row_padding() -> Result<(), Box<dyn Error>> {
  let Ok(mut pixels) = slab();
  let Ok(picture) = Picture::rectangular(
    Point { x: 3, y: 2 },
    Size {
      width: 1,
      height: 2,
    },
    Pixels {
      width: 1,
      height: 2,
      stride: 8,
      bytes: vec![200, 100, 50, 128, 255, 255, 255, 255, 0, 200, 0, 128],
    },
  );
  let shapes = [Shape::Picture(picture)];

  onto(&mut pixels, WHOLE, &shapes)?;

  assert_eq!(
    at(&pixels, Point { x: 3, y: 2 }),
    Ok(Some([25, 50, 100, 128]))
  );
  assert_eq!(
    at(&pixels, Point { x: 3, y: 3 }),
    Ok(Some([0, 100, 0, 128]))
  );
  assert_eq!(at(&pixels, Point { x: 4, y: 2 }), Ok(Some(NOTHING)));

  Ok(())
}

#[test]
fn cropped_images_follow_the_clip_and_lifting_it_restores_the_frame() -> Result<(), Box<dyn Error>>
{
  use console_core_shapes::{Clip, Cropped};

  let Ok(mut pixels) = slab();
  let Ok(picture) = Picture::rectangular(
    Point { x: 4, y: 2 },
    Size {
      width: 4,
      height: 4,
    },
    Pixels {
      width: 2,
      height: 1,
      stride: 8,
      bytes: vec![255, 0, 0, 255, 0, 255, 0, 255],
    },
  );
  let image = Shape::Cropped(Cropped {
    picture,
    from: Point { x: 1.0, y: 0.0 },
    seen: Size {
      width: 1.0,
      height: 1.0,
    },
  });
  let square = red_square(25)?;
  let shapes = [
    Shape::Clip(Clip::To {
      at: Point { x: 6, y: 2 },
      size: Size {
        width: 1,
        height: 4,
      },
    }),
    image,
    Shape::Clip(Clip::Lifted),
    square,
  ];

  onto(&mut pixels, WHOLE, &shapes)?;

  assert_eq!(
    at(&pixels, Point { x: 6, y: 4 }),
    Ok(Some([0, 255, 0, 255]))
  );
  assert_eq!(at(&pixels, Point { x: 5, y: 4 }), Ok(Some(NOTHING)));
  assert_eq!(at(&pixels, Point { x: 7, y: 4 }), Ok(Some(NOTHING)));
  assert_eq!(
    at(&pixels, Point { x: 30, y: 10 }),
    Ok(Some([0, 0, 255, 255]))
  );

  Ok(())
}

#[test]
fn rounded_panels_leave_the_corner_clear_and_lines_have_round_caps() -> Result<(), Box<dyn Error>>
{
  use console_core_shapes::Line;

  let white = color("ffffff")?;
  let Ok(mut pixels) = slab();
  let shapes = [
    Shape::Panel(Panel {
      at: Point { x: 0, y: 0 },
      size: Size {
        width: 20,
        height: 20,
      },
      round: Round(10),
      fill: white,
      edge: Edge::None,
    }),
    Shape::Line(Line {
      from: Point { x: 25, y: 10 },
      to: Point { x: 35, y: 10 },
      width: 4,
      color: white,
    }),
  ];

  onto(&mut pixels, WHOLE, &shapes)?;

  assert_eq!(at(&pixels, Point { x: 0, y: 0 }), Ok(Some(NOTHING)));
  assert_eq!(at(&pixels, Point { x: 10, y: 10 }), Ok(Some([255; 4])));
  assert_eq!(at(&pixels, Point { x: 24, y: 10 }), Ok(Some([255; 4])));
  assert_eq!(at(&pixels, Point { x: 22, y: 10 }), Ok(Some(NOTHING)));

  Ok(())
}

#[test]
fn colored_glyphs_keep_red_and_blue_in_waylands_channel_order() -> Result<(), Box<dyn Error>> {
  let Ok(font) = font();
  let Ok(mut pixels) = slab();
  let red = color("ff0000")?;
  let shapes = [Shape::Text(Text {
    at: Point { x: 0, y: 0 },
    width: WIDE,
    said: "HH".to_string(),
    weight: Weight::Bold,
    font,
    ink: red,
  })];

  onto(&mut pixels, WHOLE, &shapes)?;

  assert!(
    pixels
      .as_chunks::<4>()
      .0
      .iter()
      .any(|pixel| matches!(pixel, [0, 0, 1..=255, 1..=255]))
  );
  assert!(
    pixels
      .as_chunks::<4>()
      .0
      .iter()
      .all(|pixel| matches!(pixel, [0, 0, _, _]))
  );

  Ok(())
}
