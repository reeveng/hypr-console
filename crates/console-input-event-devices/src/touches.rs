//! A finger on the screen, read the way the kernel hands it over.
//!
//! A screen is a node that says its axes are the display itself (`DIRECT`)
//! and reports where one finger is on `ABS_X` and `ABS_Y`, which every
//! multitouch driver also sends for the first finger down. A touchpad has the
//! same axes and not the property, so the pad's own surface is never read as a
//! finger on the screen. One finger is all a pattern is drawn with, so the
//! slots behind it are not read.
//!
//! Where the finger is comes out as a share of each axis rather than in the
//! device's own units, because what those units are is the device's business
//! and the panel it lies over is the caller's. It is the panel's own way up:
//! turning it the way the picture is turned is the caller's too.
//!
//! The kernel says a frame is whole with a report, so a finger that came down
//! and moved in one frame is one touch down where it ended up, and a frame
//! with nothing but pressure in it is not a move.

use console_core_geometry::Point;
use console_core_never::Never;

use crate::codes::{AbsoluteAxisCode, EventType, PropType, SynchronizationCode};
use crate::device::Device;
use crate::event::{InputEvent, events};
use crate::kernel::AbsInfo;
use crate::keys::KeyCode;

const DOWN: i32 = 1;

const UP: i32 = 0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScreenTouch {
    Down(Point<f64>),
    Moved(Point<f64>),
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Contact {
    Pressed,
    Lifted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Went {
    Down,
    Up,
    Nowhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Moved {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    least: i32,
    most: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Touchscreen {
    x: Span,
    y: Span,
    at: Point<i32>,
    contact: Contact,
    went: Went,
    moved: Moved,
}

impl Touchscreen {
    pub fn of(device: &Device) -> Result<Option<Touchscreen>, Never> {
        let direct = device.properties.contains(&PropType::DIRECT);
        let touches = device.keys.contains(&KeyCode::BTN_TOUCH);

        match (direct, touches) {
            (true, true) => {}
            (false, _) | (true, false) => return Ok(None),
        }

        let axes = match device.absolute() {
            Ok(axes) => axes,
            Err(_no_absolute_axes) => return Ok(None),
        };
        let span_of = |wanted: AbsoluteAxisCode| {
            axes.iter().find(|(axis, _)| *axis == wanted).map(|(_, info)| {
                let Ok(span) = span(info);

                span
            })
        };

        Ok(match (span_of(AbsoluteAxisCode::ABS_X), span_of(AbsoluteAxisCode::ABS_Y)) {
            (Some(across), Some(down)) => {
                let Ok(screen) = Touchscreen::new(across, down);

                Some(screen)
            }
            (Some(_), None) | (None, _) => None,
        })
    }

    fn new(across: Span, down: Span) -> Result<Touchscreen, Never> {
        Ok(Touchscreen {
            x: across,
            y: down,
            at: Point { x: across.least, y: down.least },
            contact: Contact::Lifted,
            went: Went::Nowhere,
            moved: Moved::No,
        })
    }

    pub fn decode(&mut self, read: &[u8]) -> Result<Vec<ScreenTouch>, Never> {
        let Ok(read) = events(read);
        let mut touches = Vec::new();

        for event in read {
            let Ok(touch) = self.event(event);

            touches.extend(touch);
        }

        Ok(touches)
    }

    fn event(&mut self, event: InputEvent) -> Result<Option<ScreenTouch>, Never> {
        match (event.kind, event.code) {
            (EventType::ABSOLUTE, code) => {
                match AbsoluteAxisCode(code) {
                    AbsoluteAxisCode::ABS_X => {
                        self.at.x = event.value;
                        self.moved = Moved::Yes;
                    }
                    AbsoluteAxisCode::ABS_Y => {
                        self.at.y = event.value;
                        self.moved = Moved::Yes;
                    }
                    _ => {}
                }

                Ok(None)
            }
            (EventType::KEY, code) => {
                match (KeyCode(code), event.value) {
                    (KeyCode::BTN_TOUCH, DOWN) => self.went = Went::Down,
                    (KeyCode::BTN_TOUCH, UP) => self.went = Went::Up,
                    _ => {}
                }

                Ok(None)
            }
            (EventType::SYNCHRONIZATION, code) => match SynchronizationCode(code) {
                SynchronizationCode::SYN_REPORT => self.take_report(),
                _ => Ok(None),
            },
            _ => Ok(None),
        }
    }

    fn take_report(&mut self) -> Result<Option<ScreenTouch>, Never> {
        let Ok(share) = self.share();
        let touch = match (self.went, self.contact, self.moved) {
            (Went::Down, Contact::Lifted, _) => {
                self.contact = Contact::Pressed;

                Some(ScreenTouch::Down(share))
            }
            (Went::Up, Contact::Pressed, _) => {
                self.contact = Contact::Lifted;

                Some(ScreenTouch::Up)
            }
            (Went::Nowhere | Went::Down, Contact::Pressed, Moved::Yes) => Some(ScreenTouch::Moved(share)),
            (Went::Nowhere | Went::Down, Contact::Pressed, Moved::No)
            | (Went::Up | Went::Nowhere, Contact::Lifted, _) => None,
        };

        self.went = Went::Nowhere;
        self.moved = Moved::No;

        Ok(touch)
    }

    fn share(&self) -> Result<Point<f64>, Never> {
        let Ok(across) = along(self.at.x, self.x);
        let Ok(down) = along(self.at.y, self.y);

        Ok(Point { x: across, y: down })
    }
}

fn span(info: &AbsInfo) -> Result<Span, Never> {
    Ok(Span { least: info.minimum, most: info.maximum })
}

fn along(value: i32, span: Span) -> Result<f64, Never> {
    let wide = f64::from(span.most) - f64::from(span.least);

    Ok(match wide > 0.0 {
        true => ((f64::from(value) - f64::from(span.least)) / wide).clamp(0.0, 1.0),
        false => 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Result<Touchscreen, Never> {
        Touchscreen::new(Span { least: 0, most: 1000 }, Span { least: 0, most: 2000 })
    }

    fn frame(events: &[(EventType, u16, i32)]) -> Result<Vec<u8>, Never> {
        let mut read: Vec<u8> = events
            .iter()
            .flat_map(|(kind, code, value)| {
                let Ok(bytes) = InputEvent { kind: *kind, code: *code, value: *value }.bytes();

                bytes
            })
            .collect();
        let Ok(report) = InputEvent::REPORT.bytes();

        read.extend(report);

        Ok(read)
    }

    fn at(on: Point<i32>) -> Result<Vec<(EventType, u16, i32)>, Never> {
        Ok(vec![(EventType::ABSOLUTE, AbsoluteAxisCode::ABS_X.0, on.x), (EventType::ABSOLUTE, AbsoluteAxisCode::ABS_Y.0, on.y)])
    }

    fn touch_event(value: i32) -> Result<(EventType, u16, i32), Never> {
        Ok((EventType::KEY, KeyCode::BTN_TOUCH.0, value))
    }

    fn finger_down(on: Point<i32>) -> Result<Vec<u8>, Never> {
        let Ok(mut down) = at(on);
        let Ok(touched) = touch_event(DOWN);

        down.push(touched);

        frame(&down)
    }

    #[test]
    fn a_finger_comes_down_moves_and_lifts() {
        let Ok(mut screen) = screen();
        let Ok(mut read) = finger_down(Point { x: 500, y: 500 });
        let Ok(moved) = at(Point { x: 1000, y: 2000 });
        let Ok(moved) = frame(&moved);
        let Ok(lifted) = touch_event(UP);
        let Ok(lifted) = frame(&[lifted]);

        read.extend(moved);
        read.extend(lifted);

        let Ok(touches) = screen.decode(&read);

        assert_eq!(
            touches,
            vec![
                ScreenTouch::Down(Point { x: 0.5, y: 0.25 }),
                ScreenTouch::Moved(Point { x: 1.0, y: 1.0 }),
                ScreenTouch::Up,
            ]
        );
    }

    #[test]
    fn a_frame_that_moved_nothing_is_not_a_move() {
        let Ok(mut screen) = screen();
        let Ok(mut read) = finger_down(Point { x: 0, y: 0 });
        let Ok(pressed) = frame(&[(EventType::ABSOLUTE, AbsoluteAxisCode::ABS_PRESSURE.0, 40)]);

        read.extend(pressed);

        let Ok(touches) = screen.decode(&read);

        assert_eq!(touches, vec![ScreenTouch::Down(Point { x: 0.0, y: 0.0 })]);
    }

    #[test]
    fn a_position_with_no_finger_down_is_nothing() {
        let Ok(mut screen) = screen();
        let Ok(there) = at(Point { x: 10, y: 10 });
        let Ok(read) = frame(&there);
        let Ok(touches) = screen.decode(&read);

        assert_eq!(touches, Vec::new());
    }

    #[test]
    fn a_frame_is_read_only_once_it_is_whole() {
        let Ok(mut screen) = screen();
        let Ok(whole) = finger_down(Point { x: 500, y: 500 });
        let (first, rest) = whole.split_at(whole.len().saturating_sub(24));
        let Ok(before) = screen.decode(first);
        let Ok(after) = screen.decode(rest);

        assert_eq!(before, Vec::new());
        assert_eq!(after, vec![ScreenTouch::Down(Point { x: 0.5, y: 0.25 })]);
    }
}
