//! The greeter as shapes: the rows of dots, the lines between the dots
//! drawn so far, Login under them, and one line saying what is happening.
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

use std::collections::BTreeSet;

use console_core_color::palette::Wearing;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_i32, whole_u32};
use console_core_shapes::{Edge, Font, Line, Panel, Round, Shape, Text, Weight};
use console_draw_painting::{Run, measured};
use console_login_pattern::{DOTS, LOGIN, LOGIN_SIZE, ROOM, Target, dot};

use crate::greeting::{Greeting, Status};

pub const FONT: &str = "Noto Sans";

const FILLS: f64 = 0.8;

const LINE: f64 = 6.0;

const EDGE: f64 = 2.0;

const CURSOR: f64 = 5.0;

const WORDS: f64 = 22.0;

const LOGGING_IN: &str = "Login";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cursor {
    On,
    Off,
}

fn cursor(on: Target, here: Target) -> Result<Cursor, Never> {
    Ok(match on == here {
        true => Cursor::On,
        false => Cursor::Off,
    })
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

pub fn picture(greeting: &Greeting, wearing: &Wearing, canvas: Size<u32>) -> Result<Vec<Shape>, Never> {
    labelled(greeting, wearing, canvas, LOGGING_IN)
}

pub fn labelled(greeting: &Greeting, wearing: &Wearing, canvas: Size<u32>, button: &str) -> Result<Vec<Shape>, Never> {
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

    for (at, dot) in (0..).zip(DOTS.iter()) {
        let drawn = walked.contains(&at);
        let fill = match drawn {
            true => wearing.coral,
            false => wearing.panel,
        };
        let Ok(on) = cursor(greeting.pattern.at, Target::Dot(at));
        let Ok(edge) = edge(on, wearing, &placed);
        let Ok(corner) = placed.corner(dot.centre, dot.size);
        let Ok(size) = placed.size(dot.size);

        shapes.push(Shape::Panel(Panel { at: corner, size, round: Round(size.width.min(size.height).saturating_div(2)), fill, edge }));
    }

    let Ok(on) = cursor(greeting.pattern.at, Target::Login);
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
            let Ok(bottom) = fitted::<u32, i32>(ROOM.height);
            let Ok(half) = fitted::<u32, i32>(ROOM.width.saturating_div(2));
            let Ok(below) = placed.point(Point { x: half, y: bottom });
            let wide = size.width.saturating_mul(2);
            let Ok(at) = centred(Run { said: &message, weight: Weight::Plain, width: wide }, &font, below);
            let at = Point { x: at.x, y: below.y };

            shapes.push(Shape::Text(Text {
                at,
                width: wide,
                said: message,
                weight: Weight::Plain,
                font,
                ink: wearing.soft,
            }));
        }
        None => {}
    }

    Ok(shapes)
}

fn centred(run: Run<'_>, font: &Font, middle: Point<i32>) -> Result<Point<i32>, Never> {
    let Ok(ink) = measured(run, font);
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

    fn wearing() -> Wearing {
        let color = |lightness| Oklch { lightness, chroma: 0.0, hue: 0.0 };

        Wearing {
            panel: color(0.3),
            text: color(0.9),
            edge: color(0.5),
            soft: color(0.7),
            coral: color(0.6),
            ground: color(0.1),
            fill: color(0.4),
            night: color(0.05),
            pink: color(0.85),
        }
    }

    fn panels(shapes: &[Shape]) -> Vec<Panel> {
        let Ok(panels) = console_core_shapes::panels(shapes);

        panels
    }

    #[test]
    fn every_dot_and_login_are_on_the_screen() {
        let canvas = Size { width: 2560, height: 1600 };
        let Ok(pattern) = Pattern::new();
        let Ok(shapes) = picture(&Greeting { pattern, status: Status::Waiting }, &wearing(), canvas);
        let inside = |panel: &Panel| {
            panel.at.x >= 0
                && panel.at.y >= 0
                && i64::from(panel.at.x) + i64::from(panel.size.width) <= i64::from(canvas.width)
                && i64::from(panel.at.y) + i64::from(panel.size.height) <= i64::from(canvas.height)
        };

        assert_eq!(panels(&shapes).len(), DOTS.len() + 2);
        assert!(panels(&shapes).iter().all(inside));
    }

    fn texts(shapes: &[Shape]) -> Vec<Text> {
        let Ok(texts) = console_core_shapes::texts(shapes);

        texts
    }

    fn middle_of(text: &Text) -> i64 {
        let Ok(ink) = measured(Run { said: &text.said, weight: text.weight, width: text.width }, &text.font);

        i64::from(text.at.x) + i64::from(ink.width) / 2
    }

    #[test]
    fn the_button_and_the_line_under_it_are_centred() {
        let canvas = Size { width: 1280, height: 800 };
        let Ok(pattern) = Pattern::new();

        for button in ["Login", "Next", "Save"] {
            let greeting = Greeting { pattern: pattern.clone(), status: Status::Message("Draw a new pattern, then Next.".to_string()) };
            let Ok(shapes) = labelled(&greeting, &wearing(), canvas, button);
            let Ok(placed) = Placed::of(canvas);
            let Ok(centre) = placed.point(LOGIN);
            let said = texts(&shapes);
            let apart = |text: &Text, from: i64| (middle_of(text) - from).abs();

            assert_eq!(said.len(), 2);
            assert!(said.first().is_some_and(|label| apart(label, i64::from(centre.x)) <= 2), "{button} is off the middle of its button");
            assert!(said.get(1).is_some_and(|line| apart(line, i64::from(canvas.width) / 2) <= 2), "the line under {button} is off the middle of the screen");
        }
    }

    #[test]
    fn a_drawn_pattern_is_one_line_fewer_than_its_dots() {
        let pattern = Pattern { at: Target::Login, path: vec![0, 2, 5], finger: None };
        let Ok(shapes) = picture(&Greeting { pattern, status: Status::Waiting }, &wearing(), Size { width: 1280, height: 800 });
        assert_eq!(shapes.iter().filter(|shape| matches!(shape, Shape::Line(_))).count(), 2);
    }

    #[test]
    fn a_finger_down_draws_the_line_on_from_the_last_dot_to_it() {
        let finger = Point { x: 300, y: 200 };
        let pattern = Pattern { at: Target::Dot(5), path: vec![0, 2, 5], finger: Some(finger) };
        let canvas = Size { width: 1280, height: 800 };
        let Ok(shapes) = picture(&Greeting { pattern, status: Status::Waiting }, &wearing(), canvas);
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

            assert!((back.x - dot.centre.x).abs() <= 1 && (back.y - dot.centre.y).abs() <= 1, "{} came back at {back:?}", dot.key);
        }
    }

    #[test]
    fn the_cursor_is_the_one_edge_in_the_text_color() {
        let Ok(pattern) = Pattern::new();
        let Ok(shapes) = picture(&Greeting { pattern, status: Status::Waiting }, &wearing(), Size { width: 1280, height: 800 });
        assert_eq!(
            panels(&shapes)
                .iter()
                .filter(|panel| match panel.edge {
                    Edge::Of { color, .. } => color == wearing().text,
                    Edge::None => false,
                })
                .count(),
            1
        );
    }
}
