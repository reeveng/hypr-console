//! The greeter as shapes: the rows of dots, the lines between the dots
//! drawn so far, Login under them, and what is happening above them, each
//! line of it centred on its own.
//!
//! The rows are laid out in its own room and scaled whole into the middle
//! of the screen, so the dots keep the shape they were drawn in on any panel.
//! A dot in the pattern is filled with the accent and joined to the dot before
//! it, and while a finger is down the last dot is joined to the finger too, so
//! the line being drawn is under the hand drawing it. The dot the cursor is on
//! wears a heavier edge in the text color, which is the one thing on the
//! screen that has to be found at a glance.
//!
//! A touch lands in the screen's own points and the pattern is decided in the
//! room's, so [`in_the_room`] is the same placing walked backwards, and
//! [`from_the_room`] is where a hand has to go to press a dot.
//!
//! The bar is drawn over all of it, at the top and at the size it is on the
//! desktop: the desktop is drawn [`DRAWN_AT`] points across whatever the panel's
//! pixels, so a screen with no compositor to scale it draws the bar into a room
//! that wide and paints it up to the pixels, and a finger on it is walked back
//! down by [`on_the_bar`]. Over a session the compositor has done that already
//! and the room is the surface. While the pad is on the bar the ring wears no
//! cursor.

use std::collections::BTreeSet;

use console_core_color::palette::Wearing;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_i32, whole_u32};
use console_core_shapes::{Edge, Font, Line, Panel, Round, Shape, Text, Weight};
use console_draw_painting::{Cannot, Frame, Run, measure_text, onto, over, wrapped};
use console_login_pattern::{DOTS, LOGIN, LOGIN_SIZE, ROOM, Target, dot};
use console_screen::DRAWN_AT;
use console_status_bar::showing::{self, Rendered};

use crate::greeting::{GreeterState, Greeting, Status};

pub const FONT: &str = "Noto Sans";

const FILLS: f64 = 0.7;

const LINE: f64 = 6.0;

const EDGE: f64 = 2.0;

const CURSOR: f64 = 5.0;

const WORDS: f64 = 22.0;

const ABOVE: i32 = -12;

const LOGGING_IN: &str = "Login";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cursor {
    On,
    Off,
}

fn cursor(on: Option<Target>, here: Target) -> Result<Cursor, Never> {
    Ok(match on == Some(here) {
        true => Cursor::On,
        false => Cursor::Off,
    })
}

#[derive(Debug, Clone, Copy)]
pub struct Worn<'a> {
    pub ring: &'a Wearing,
    pub bar: &'a showing::Wearing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderedFrame {
    pub pixels: Vec<u8>,
    pub bar: Rendered,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Placed {
    origin: Point<f64>,
    scale: f64,
}

impl Placed {
    fn of(canvas: Size<u32>) -> Result<Placed, Never> {
        let wide = f64::from(canvas.width);
        let tall = f64::from(canvas.height);
        let scale = (wide / f64::from(ROOM.width)).min(tall / f64::from(ROOM.height)) * FILLS;
        let origin = Point {
            x: (wide - f64::from(ROOM.width) * scale) / 2.0,
            y: (tall - f64::from(ROOM.height) * scale) / 2.0,
        };

        Ok(Placed { origin, scale })
    }

    fn room(&self, at: Point<f64>) -> Result<Point<i32>, Never> {
        let Ok(across) = toward_zero_i32((at.x - self.origin.x) / self.scale);
        let Ok(down) = toward_zero_i32((at.y - self.origin.y) / self.scale);

        Ok(Point { x: across, y: down })
    }

    fn point(&self, at: Point<i32>) -> Result<Point<i32>, Never> {
        let Ok(across) = toward_zero_i32(self.origin.x + f64::from(at.x) * self.scale);
        let Ok(down) = toward_zero_i32(self.origin.y + f64::from(at.y) * self.scale);

        Ok(Point { x: across, y: down })
    }

    fn length(&self, many: f64) -> Result<u32, Never> {
        whole_u32(many * self.scale)
    }

    fn size(&self, size: Size<u32>) -> Result<Size<u32>, Never> {
        let Ok(wide) = self.length(f64::from(size.width));
        let Ok(tall) = self.length(f64::from(size.height));

        Ok(Size { width: wide, height: tall })
    }

    fn corner(&self, centre: Point<i32>, size: Size<u32>) -> Result<Point<i32>, Never> {
        let Ok(middle) = self.point(centre);
        let Ok(scaled) = self.size(size);
        let Ok(half_wide) = toward_zero_i32(f64::from(scaled.width) / 2.0);
        let Ok(half_tall) = toward_zero_i32(f64::from(scaled.height) / 2.0);

        Ok(Point { x: middle.x.saturating_sub(half_wide), y: middle.y.saturating_sub(half_tall) })
    }
}

pub fn in_the_room(canvas: Size<u32>, at: Point<f64>) -> Result<Point<i32>, Never> {
    let Ok(placed) = Placed::of(canvas);

    placed.room(at)
}

pub fn from_the_room(canvas: Size<u32>, at: Point<i32>) -> Result<Point<i32>, Never> {
    let Ok(placed) = Placed::of(canvas);

    placed.point(at)
}

fn scale(canvas: Size<u32>) -> Result<f64, Never> {
    Ok(f64::from(canvas.width) / f64::from(DRAWN_AT.0.max(1)))
}

pub fn bar_room(canvas: Size<u32>) -> Result<Size<u32>, Never> {
    let Ok(scale) = scale(canvas);
    let Ok(tall) = whole_u32(f64::from(canvas.height) / scale);

    Ok(Size { width: DRAWN_AT.0, height: tall })
}

pub fn on_the_bar(canvas: Size<u32>, at: Point<f64>) -> Result<Point<i32>, Never> {
    let Ok(scale) = scale(canvas);
    let Ok(across) = toward_zero_i32(at.x / scale);
    let Ok(down) = toward_zero_i32(at.y / scale);

    Ok(Point { x: across, y: down })
}

pub fn painted(into: Vec<u8>, state: &GreeterState, worn: Worn<'_>, canvas: Size<u32>) -> Result<Result<RenderedFrame, Cannot>, Never> {
    let Ok(shapes) = under_the_bar(state, worn.ring, canvas, LOGGING_IN);
    let Ok(room) = bar_room(canvas);
    let Ok(bar) = state.bar.pictured(worn.bar, room);
    let Ok(long) = console_core_number_conversion::index(u64::from(canvas.width).saturating_mul(u64::from(canvas.height)).saturating_mul(4));
    let mut pixels = into;

    pixels.resize(long, 0);

    let ring = onto(&mut pixels, Frame { device: canvas, points: canvas }, &shapes);
    let both = match ring {
        Ok(()) => over(&mut pixels, Frame { device: canvas, points: room }, &bar.shapes),
        Err(why) => Err(why),
    };

    Ok(both.map(|()| RenderedFrame { pixels, bar }))
}

pub fn over_a_session(state: &GreeterState, worn: Worn<'_>, logical: Size<u32>, button: &str) -> Result<(Vec<Shape>, Rendered), Never> {
    let Ok(mut shapes) = under_the_bar(state, worn.ring, logical, button);
    let Ok(bar) = state.bar.pictured(worn.bar, logical);

    shapes.extend(bar.shapes.iter().cloned());

    Ok((shapes, bar))
}

pub fn picture(greeting: &Greeting, wearing: &Wearing, canvas: Size<u32>) -> Result<Vec<Shape>, Never> {
    render(greeting, wearing, canvas, LOGGING_IN)
}

pub fn under_the_bar(state: &GreeterState, wearing: &Wearing, canvas: Size<u32>, button: &str) -> Result<Vec<Shape>, Never> {
    let at = match state.bar.focus {
        Some(_) => None,
        None => Some(state.greeting.pattern.at),
    };

    ring(&state.greeting, wearing, canvas, Looks { button, cursor: at })
}

pub fn render(greeting: &Greeting, wearing: &Wearing, canvas: Size<u32>, button: &str) -> Result<Vec<Shape>, Never> {
    ring(greeting, wearing, canvas, Looks { button, cursor: Some(greeting.pattern.at) })
}

#[derive(Debug, Clone, Copy)]
struct Looks<'a> {
    button: &'a str,
    cursor: Option<Target>,
}

fn ring(greeting: &Greeting, wearing: &Wearing, canvas: Size<u32>, looks: Looks<'_>) -> Result<Vec<Shape>, Never> {
    let Looks { button, cursor: at } = looks;
    let Ok(placed) = Placed::of(canvas);
    let mut shapes = vec![Shape::Panel(Panel {
        at: Point { x: 0, y: 0 },
        size: canvas,
        round: Round(0),
        fill: wearing.ground,
        edge: Edge::None,
    })];

    let dot_at = |at: &u32| {
        let Ok(dot) = dot(*at);

        dot
    };

    for pair in greeting.pattern.path.windows(2) {
        let ends = (pair.first().and_then(dot_at), pair.get(1).and_then(dot_at));

        match ends {
            (Some(from), Some(to)) => {
                let Ok(from) = placed.point(from.centre);
                let Ok(to) = placed.point(to.centre);
                let Ok(wide) = placed.length(LINE);

                shapes.push(Shape::Line(Line { from, to, width: wide, color: wearing.coral }));
            }
            (Some(_), None) | (None, _) => {}
        }
    }

    let last = greeting.pattern.path.last().and_then(dot_at);

    match (last, greeting.pattern.finger) {
        (Some(last), Some(finger)) => {
            let Ok(from) = placed.point(last.centre);
            let Ok(to) = placed.point(finger);
            let Ok(wide) = placed.length(LINE);

            shapes.push(Shape::Line(Line { from, to, width: wide, color: wearing.coral }));
        }
        (Some(_), None) | (None, _) => {}
    }

    let walked: BTreeSet<u32> = greeting.pattern.path.iter().copied().collect();

    for (number, dot) in (0..).zip(DOTS.iter()) {
        let drawn = walked.contains(&number);
        let fill = match drawn {
            true => wearing.coral,
            false => wearing.panel,
        };
        let Ok(on) = cursor(at, Target::Dot(number));
        let Ok(edge) = edge(on, wearing, &placed);
        let Ok(corner) = placed.corner(dot.centre, dot.size);
        let Ok(size) = placed.size(dot.size);

        shapes.push(Shape::Panel(Panel { at: corner, size, round: Round(size.width.min(size.height).saturating_div(2)), fill, edge }));
    }

    let Ok(on) = cursor(at, Target::Login);
    let Ok(edge) = edge(on, wearing, &placed);
    let Ok(corner) = placed.corner(LOGIN, LOGIN_SIZE);
    let Ok(size) = placed.size(LOGIN_SIZE);
    let Ok(tall) = placed.length(WORDS);
    let font = Font { family: FONT.to_string(), height: tall };
    let Ok(middle) = placed.point(LOGIN);
    let Ok(label) = centred(Run { said: button, weight: Weight::Bold, width: size.width }, &font, middle);

    shapes.push(Shape::Panel(Panel { at: corner, size, round: Round(size.height.saturating_div(2)), fill: wearing.fill, edge }));
    shapes.push(Shape::Text(Text {
        at: label,
        width: size.width,
        said: button.to_string(),
        weight: Weight::Bold,
        font: font.clone(),
        ink: wearing.text,
    }));

    let Ok(message) = message(&greeting.status);

    match message {
        Some(message) => {
            let Ok(half) = fitted::<u32, i32>(ROOM.width.saturating_div(2));
            let Ok(above) = placed.point(Point { x: half, y: ABOVE });
            let Ok(wide) = whole_u32(f64::from(canvas.width) * FILLS);
            let Ok(lines) = wrapped(Run { said: &message, weight: Weight::Plain, width: wide }, &font);
            let mut bottom = above.y;

            for line in lines.iter().rev() {
                let Ok(ink) = measure_text(Run { said: line, weight: Weight::Plain, width: wide }, &font);
                let Ok(tall) = fitted::<u32, i32>(ink.height);
                let Ok(half_wide) = fitted::<u32, i32>(ink.width.saturating_div(2));

                bottom = bottom.saturating_sub(tall);
                shapes.push(Shape::Text(Text {
                    at: Point { x: above.x.saturating_sub(half_wide), y: bottom },
                    width: wide,
                    said: line.clone(),
                    weight: Weight::Plain,
                    font: font.clone(),
                    ink: wearing.soft,
                }));
            }
        }
        None => {}
    }

    Ok(shapes)
}

fn centred(run: Run<'_>, font: &Font, middle: Point<i32>) -> Result<Point<i32>, Never> {
    let Ok(ink) = measure_text(run, font);
    let Ok(half_wide) = fitted::<u32, i32>(ink.width.saturating_div(2));
    let Ok(half_tall) = fitted::<u32, i32>(ink.height.saturating_div(2));

    Ok(Point { x: middle.x.saturating_sub(half_wide), y: middle.y.saturating_sub(half_tall) })
}

fn edge(on: Cursor, wearing: &Wearing, placed: &Placed) -> Result<Edge, Never> {
    let (wide, color) = match on {
        Cursor::On => (CURSOR, wearing.text),
        Cursor::Off => (EDGE, wearing.edge),
    };
    let Ok(wide) = placed.length(wide);

    Ok(Edge::Of { wide: wide.max(1), color })
}

fn message(status: &Status) -> Result<Option<String>, Never> {
    Ok(match status {
        Status::Waiting => None,
        Status::Fell => Some("The desktop stopped. Draw your pattern to start it again.".to_string()),
        Status::Checking => Some("Checking...".to_string()),
        Status::Failed(why) => Some(format!("That did not work: {why}.")),
        Status::Message(text) => Some(text.clone()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use console_core_color::Oklch;
    use console_login_pattern::Pattern;
    use console_status_bar::lock_screen::{LockScreenBar, Menu};
    use console_status_bar::showing::BarAction;

    use crate::turn::{Rows, changed, drawn_at};

    fn flat_palette() -> Result<Wearing, Never> {
        let color = |lightness| Oklch { lightness, chroma: 0.0, hue: 0.0 };

        Ok(Wearing {
            panel: color(0.3),
            text: color(0.9),
            edge: color(0.5),
            soft: color(0.7),
            coral: color(0.6),
            ground: color(0.1),
            fill: color(0.4),
            night: color(0.05),
            pink: color(0.85),
        })
    }

    fn flat_bar_palette() -> Result<showing::Wearing, Never> {
        let color = |lightness| Oklch { lightness, chroma: 0.0, hue: 0.0 };

        Ok(showing::Wearing {
            ground: color(0.1),
            text: color(0.9),
            soft: color(0.7),
            pink: color(0.85),
            night: color(0.05),
            butter: color(0.8),
            coral: color(0.6),
            leaf: color(0.75),
            fill: color(0.4),
        })
    }

    fn standing(greeting: Greeting) -> Result<GreeterState, Never> {
        Ok(GreeterState { greeting, bar: LockScreenBar { clock: String::from("14:30"), ..LockScreenBar::default() } })
    }

    #[test]
    fn every_dot_and_login_are_on_the_screen() {
        let Ok(palette) = flat_palette();
        let canvas = Size { width: 2560, height: 1600 };
        let Ok(pattern) = Pattern::new();
        let Ok(shapes) = picture(&Greeting { pattern, status: Status::Waiting }, &palette, canvas);
        let inside = |panel: &Panel| {
            panel.at.x >= 0
                && panel.at.y >= 0
                && i64::from(panel.at.x).saturating_add(i64::from(panel.size.width)) <= i64::from(canvas.width)
                && i64::from(panel.at.y).saturating_add(i64::from(panel.size.height)) <= i64::from(canvas.height)
        };

        let Ok(drawn) = console_core_shapes::panels(&shapes);

        assert_eq!(drawn.len(), DOTS.len().saturating_add(2));
        assert!(drawn.iter().all(inside));
    }

    fn middle_of(text: &Text) -> Result<i64, Never> {
        let Ok(ink) = measure_text(Run { said: &text.said, weight: text.weight, width: text.width }, &text.font);

        Ok(i64::from(text.at.x).saturating_add(i64::from(ink.width).div_euclid(2)))
    }

    #[test]
    fn the_button_and_every_line_above_the_ring_are_centred() {
        let Ok(palette) = flat_palette();
        let canvas = Size { width: 1280, height: 800 };
        let Ok(pattern) = Pattern::new();
        let long = "That did not work: the pattern was drawn, the session was asked for, and nothing answered before the screen went dark again.";

        for (button, said) in [("Login", "Checking..."), ("Next", "Draw a new pattern, then Next."), ("Save", long)] {
            let greeting = Greeting { pattern: pattern.clone(), status: Status::Message(String::from(said)) };
            let Ok(shapes) = render(&greeting, &palette, canvas, button);
            let Ok(placed) = Placed::of(canvas);
            let Ok(centre) = placed.point(LOGIN);
            let Ok(ring) = placed.point(Point { x: 0, y: 0 });
            let Ok(texts) = console_core_shapes::texts(&shapes);
            let apart = |text: &Text, from: i64| {
                let Ok(middle) = middle_of(text);

                middle.saturating_sub(from).abs()
            };
            let lines: Vec<&Text> = texts.iter().skip(1).collect();

            assert!(texts.first().is_some_and(|label| apart(label, i64::from(centre.x)) <= 2), "{button} is off the middle of its button");
            assert!(!lines.is_empty(), "nothing says {said:?}");
            assert!(lines.iter().all(|line| apart(line, i64::from(canvas.width).div_euclid(2)) <= 2), "a line of {said:?} is off the middle of the screen");
            assert!(lines.iter().all(|line| line.at.y >= 0 && line.at.y < ring.y), "a line of {said:?} is not above the ring");
        }

        let greeting = Greeting { pattern: pattern.clone(), status: Status::Message(String::from(long)) };
        let Ok(shapes) = render(&greeting, &palette, canvas, "Save");
        let Ok(texts) = console_core_shapes::texts(&shapes);

        assert!(texts.len() > 2, "the long message did not wrap, so it proves nothing about wrapped lines");
    }

    #[test]
    fn a_drawn_pattern_is_one_line_fewer_than_its_dots() {
        let Ok(palette) = flat_palette();
        let pattern = Pattern { at: Target::Login, path: vec![0, 2, 5], finger: None };
        let Ok(shapes) = picture(&Greeting { pattern, status: Status::Waiting }, &palette, Size { width: 1280, height: 800 });
        assert_eq!(shapes.iter().filter(|shape| matches!(shape, Shape::Line(_))).count(), 2);
    }

    #[test]
    fn a_finger_down_draws_the_line_on_from_the_last_dot_to_it() {
        let Ok(palette) = flat_palette();
        let finger = Point { x: 300, y: 200 };
        let pattern = Pattern { at: Target::Dot(5), path: vec![0, 2, 5], finger: Some(finger) };
        let canvas = Size { width: 1280, height: 800 };
        let Ok(shapes) = picture(&Greeting { pattern, status: Status::Waiting }, &palette, canvas);
        let Ok(placed) = Placed::of(canvas);
        let Ok(under) = placed.point(finger);
        let lines: Vec<&Line> = shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Line(line) => Some(line),
                Shape::Panel(_) | Shape::Text(_) | Shape::Picture(_) | Shape::Cropped(_) | Shape::Clip(_) => None,
            })
            .collect();

        assert_eq!(lines.len(), 3);
        assert!(lines.last().is_some_and(|line| line.to == under));
    }

    #[test]
    fn a_point_on_the_screen_is_found_again_in_the_room() {
        let canvas = Size { width: 2560, height: 1600 };
        let Ok(placed) = Placed::of(canvas);

        for dot in DOTS {
            let Ok(on_screen) = placed.point(dot.centre);
            let Ok(back) = in_the_room(canvas, Point { x: f64::from(on_screen.x), y: f64::from(on_screen.y) });

            assert!(back.x.abs_diff(dot.centre.x) <= 1 && back.y.abs_diff(dot.centre.y) <= 1, "{} came back at {back:?}", dot.key);
        }
    }

    #[test]
    fn the_cursor_is_the_one_edge_in_the_text_color() {
        let Ok(palette) = flat_palette();
        let Ok(pattern) = Pattern::new();
        let Ok(shapes) = picture(&Greeting { pattern, status: Status::Waiting }, &palette, Size { width: 1280, height: 800 });
        let Ok(drawn) = console_core_shapes::panels(&shapes);

        assert_eq!(
            drawn
                .iter()
                .filter(|panel| match panel.edge {
                    Edge::Of { color, .. } => color == palette.text,
                    Edge::None => false,
                })
                .count(),
            1
        );
    }

    const PANEL: Size<u32> = Size { width: 1600, height: 2560 };

    fn dragging(nudged: i32) -> Result<Option<Greeting>, Never> {
        let Ok(first) = dot(0);
        let Ok(second) = dot(1);
        let Ok(pattern) = Pattern::new();

        Ok(first.zip(second).map(|(first, second)| {
            let middle = Point {
                x: first.centre.x.saturating_add(second.centre.x).saturating_div(2).saturating_add(nudged),
                y: first.centre.y.saturating_add(second.centre.y).saturating_div(2),
            };

            Greeting { pattern: Pattern { path: vec![0], finger: Some(middle), ..pattern }, status: Status::Waiting }
        }))
    }

    #[test]
    fn a_finger_moving_between_two_dots_changes_a_narrow_band_of_rows() -> Result<(), &'static str> {
        let Ok(wearing) = flat_palette();
        let Ok(bar) = flat_bar_palette();
        let worn = Worn { ring: &wearing, bar: &bar };
        let Ok(canvas) = drawn_at(PANEL);
        let Ok(before) = dragging(0);
        let Ok(after) = dragging(6);
        let before = before.ok_or("the pattern has fewer than two dots")?;
        let after = after.ok_or("the pattern has fewer than two dots")?;
        let Ok(before) = standing(before);
        let Ok(after) = standing(after);
        let Ok(was) = painted(Vec::new(), &before, worn, canvas);
        let Ok(is) = painted(Vec::new(), &after, worn, canvas);
        let was = was.map_err(|_cannot| "the first frame would not paint")?;
        let is = is.map_err(|_cannot| "the second frame would not paint")?;
        let Ok(band) = changed(&was.pixels, &is.pixels, canvas);
        let Rows { from, to } = band.ok_or("moving the finger changed nothing on the screen")?;

        assert!(to.saturating_sub(from) <= canvas.height.saturating_div(8), "a move changed rows {from} to {to} of {}", canvas.height);

        Ok(())
    }

    #[test]
    fn the_ring_wears_no_cursor_while_the_pad_is_on_the_bar() -> Result<(), &'static str> {
        let Ok(palette) = flat_palette();
        let Ok(pattern) = Pattern::new();
        let Ok(state) = standing(Greeting { pattern, status: Status::Waiting });
        let on_the_bar = GreeterState { bar: LockScreenBar { focus: Some(BarAction::Menu(Menu::Power)), ..state.bar.clone() }, ..state.clone() };
        let cursors = |state: &GreeterState| {
            let Ok(shapes) = under_the_bar(state, &palette, Size { width: 1280, height: 800 }, LOGGING_IN);
            let Ok(drawn) = console_core_shapes::panels(&shapes);

            drawn
                .iter()
                .filter(|panel| match panel.edge {
                    Edge::Of { color, .. } => color == palette.text,
                    Edge::None => false,
                })
                .count()
        };

        assert_eq!(cursors(&state), 1);
        assert_eq!(cursors(&on_the_bar), 0, "the ring kept a cursor with the pad on the bar");

        Ok(())
    }

    #[test]
    fn the_bar_is_drawn_the_desktop_s_width_in_points_and_a_finger_on_it_lands_on_what_is_under_it() -> Result<(), &'static str> {
        let Ok(wearing) = flat_palette();
        let Ok(bar) = flat_bar_palette();
        let Ok(canvas) = drawn_at(PANEL);
        let Ok(pattern) = Pattern::new();
        let Ok(state) = standing(Greeting { pattern, status: Status::Waiting });
        let opened = GreeterState { bar: LockScreenBar { menu: Some(Menu::Power), ..state.bar.clone() }, ..state };
        let Ok(room) = bar_room(canvas);
        let Ok(painted) = painted(Vec::new(), &opened, Worn { ring: &wearing, bar: &bar }, canvas);
        let painted = painted.map_err(|_cannot| "the frame would not paint")?;
        let Ok(scale) = scale(canvas);

        assert_eq!(room.width, DRAWN_AT.0);
        assert_eq!(room.height, 640, "a 2560 by 1600 panel is not 1024 by 640 points");
        assert!(!painted.bar.touching.is_empty(), "nothing on the bar can be pressed");

        for region in &painted.bar.touching {
            let middle = Point {
                x: (f64::from(region.panel.at.x) + f64::from(region.panel.size.width) / 2.0) * scale,
                y: (f64::from(region.panel.at.y) + f64::from(region.panel.size.height) / 2.0) * scale,
            };
            let Ok(point) = on_the_bar(canvas, middle);
            let Ok(landed) = painted.bar.on(point);

            assert_eq!(landed, Some(region.action), "a finger on {:?} landed on something else", region.action);
        }

        Ok(())
    }
}
