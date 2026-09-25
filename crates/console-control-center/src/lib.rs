//! Control Center: what a finger pulls down from the top edge of the screen.
//!
//! It is here because a screenshot had no answer for a hand holding nothing.
//! The paddle with L2 and Super+S are the pad's and the keyboard's, and the
//! obvious third -- a row in a panel -- takes a picture of that panel standing
//! in front of whatever was meant to be in it. A panel coming up is also a
//! panel taking the keyboard, and the one that was up before it goes. So what
//! is pulled down is not a panel: it is on the overlay layer, over a game put
//! full screen and over every panel of ours, and it declines the keyboard, so
//! nothing that was open closes because it came.
//!
//! The edge has to be somebody's before a finger can pull it, and Hyprland
//! reads the touchscreen as a pointer on whatever is under it. A swipe that
//! starts over someone else's window would want a plugin -- hyprgrass -- or
//! the digitizer read beside the compositor, and both answer a question the
//! layer shell already answers: a strip `EDGE` deep along the top, on the
//! overlay layer, is where every pull starts, and a touch that starts on a
//! surface keeps reporting to it wherever the finger goes after. What it costs
//! is the top `EDGE` points of whatever is under it, which on this desktop is
//! the top of the bar.
//!
//! The sheet goes the way it came: a swipe up puts it away, and so does a tap
//! anywhere but the button, because a finger that has left the button has
//! said it did not mean it.
//!
//! Close is beside it because a finger has no paddle. A program that draws no
//! close button of its own, or one too small to hit, could otherwise only be
//! left by picking up the controller. It runs what the paddle runs, so it puts
//! away what the paddle would, and it too waits for the sheet to go: the sheet
//! declines the keyboard, so what was in front before it came is still what
//! is in front after.
//!
//! The picture is taken after the sheet has gone. It goes back to the strip,
//! which is drawn in nothing, and a strip drawn in nothing is not in anybody's
//! screenshot.

use console_core_color::palette::Wearing;
use console_core_fonts::TextStyle;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_i32};
use console_core_shapes::{Covers, Edge, Panel, Round, Shape, Text, Weight};

pub const EDGE: u32 = 10;

const PULL: f64 = 48.0;

const MARGIN: u32 = 16;

const PADDING: u32 = 16;

const WIDEST: u32 = 360;

const BUTTON_TALL: u32 = 56;

const CARD_ROUND: u32 = 16;

const BUTTON_ROUND: u32 = 12;

const CARD_EDGE: u32 = 2;

pub const SCREENSHOT: &str = "Screenshot";

pub const CLOSE: &str = "Close";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Swipe {
    Down,
    Up,
    Neither,
}

pub fn swiped(from: (f64, f64), at: (f64, f64)) -> Result<Swipe, Never> {
    let down = at.1 - from.1;
    let across = (at.0 - from.0).abs();

    Ok(match (down.abs() >= PULL && down.abs() > across, down > 0.0) {
        (true, true) => Swipe::Down,
        (true, false) => Swipe::Up,
        (false, _) => Swipe::Neither,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tapped {
    Screenshot,
    Close,
    Outside,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    pub card: Panel,
    pub screenshot: Panel,
    pub close: Panel,
}

pub fn sheet(room: Size<u32>, wearing: &Wearing) -> Result<Sheet, Never> {
    let wide = room.width.saturating_sub(MARGIN.saturating_mul(2)).min(WIDEST);
    let Ok(room_wide) = fitted::<u32, i32>(room.width);
    let Ok(card_wide) = fitted::<u32, i32>(wide);
    let Ok(margin) = fitted::<u32, i32>(MARGIN);
    let Ok(padding) = fitted::<u32, i32>(PADDING);
    let left = room_wide.saturating_sub(card_wide).saturating_div(2);

    let card = Panel {
        at: Point { x: left, y: margin },
        size: Size { width: wide, height: BUTTON_TALL.saturating_add(PADDING.saturating_mul(2)) },
        round: Round(CARD_ROUND),
        fill: wearing.panel,
        edge: Edge::Of { wide: CARD_EDGE, color: wearing.edge },
    };

    let button_wide = wide.saturating_sub(PADDING.saturating_mul(3)).saturating_div(2);
    let Ok(step) = fitted::<u32, i32>(button_wide.saturating_add(PADDING));
    let first = left.saturating_add(padding);
    let down = margin.saturating_add(padding);

    let button = |across: i32| Panel {
        at: Point { x: across, y: down },
        size: Size { width: button_wide, height: BUTTON_TALL },
        round: Round(BUTTON_ROUND),
        fill: wearing.ground,
        edge: Edge::None,
    };

    Ok(Sheet { card, screenshot: button(first), close: button(first.saturating_add(step)) })
}

pub fn tapped(sheet: &Sheet, at: (f64, f64)) -> Result<Tapped, Never> {
    let Ok(across) = toward_zero_i32(at.0.floor());
    let Ok(down) = toward_zero_i32(at.1.floor());
    let Ok(on_screenshot) = sheet.screenshot.covers(Point { x: across, y: down });
    let Ok(on_close) = sheet.close.covers(Point { x: across, y: down });

    Ok(match (on_screenshot, on_close) {
        (Covers::Yes, _) => Tapped::Screenshot,
        (Covers::No, Covers::Yes) => Tapped::Close,
        (Covers::No, Covers::No) => Tapped::Outside,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Labels {
    pub screenshot: Size<u32>,
    pub close: Size<u32>,
}

pub fn drawn(sheet: &Sheet, labels: Labels, wearing: &Wearing) -> Result<Vec<Shape>, Never> {
    let Ok(screenshot) = label(sheet.screenshot, labels.screenshot, SCREENSHOT, wearing);
    let Ok(close) = label(sheet.close, labels.close, CLOSE, wearing);

    Ok(vec![
        Shape::Panel(sheet.card),
        Shape::Panel(sheet.screenshot),
        screenshot,
        Shape::Panel(sheet.close),
        close,
    ])
}

fn label(button: Panel, label: Size<u32>, said: &str, wearing: &Wearing) -> Result<Shape, Never> {
    let Ok(font) = TextStyle::Headline.font();
    let Ok(button_wide) = fitted::<u32, i32>(button.size.width);
    let Ok(button_tall) = fitted::<u32, i32>(button.size.height);
    let Ok(label_wide) = fitted::<u32, i32>(label.width);
    let Ok(label_tall) = fitted::<u32, i32>(label.height);
    let across = button.at.x.saturating_add(button_wide.saturating_sub(label_wide).saturating_div(2));
    let down = button.at.y.saturating_add(button_tall.saturating_sub(label_tall).saturating_div(2));

    Ok(Shape::Text(Text {
        at: Point { x: across, y: down },
        width: label.width,
        said: said.to_string(),
        weight: Weight::Bold,
        font,
        ink: wearing.text,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_core_color::Oklch;

    fn wearing() -> Wearing {
        let one = Oklch { lightness: 0.5, chroma: 0.0, hue: 0.0 };

        Wearing { panel: one, text: one, edge: one, soft: one, coral: one, ground: one, fill: one, night: one, pink: one }
    }

    #[test]
    fn a_finger_dragged_down_off_the_edge_is_a_swipe_down() {
        assert_eq!(swiped((500.0, 2.0), (510.0, 80.0)), Ok(Swipe::Down));
    }

    #[test]
    fn a_finger_dragged_back_up_is_a_swipe_up() {
        assert_eq!(swiped((500.0, 300.0), (490.0, 200.0)), Ok(Swipe::Up));
    }

    #[test]
    fn a_tap_is_no_swipe() {
        assert_eq!(swiped((500.0, 2.0), (503.0, 6.0)), Ok(Swipe::Neither));
    }

    #[test]
    fn a_finger_dragged_along_the_edge_is_no_swipe() {
        assert_eq!(swiped((100.0, 2.0), (400.0, 90.0)), Ok(Swipe::Neither));
    }

    #[test]
    fn the_left_button_takes_the_picture_the_right_one_closes_and_anywhere_else_puts_the_sheet_away() {
        let Ok(sheet) = sheet(Size { width: 1280, height: 800 }, &wearing());
        let down = 60.0;

        assert_eq!(tapped(&sheet, (560.0, down)), Ok(Tapped::Screenshot));
        assert_eq!(tapped(&sheet, (720.0, down)), Ok(Tapped::Close));
        assert_eq!(tapped(&sheet, (640.0, down)), Ok(Tapped::Outside));
        assert_eq!(tapped(&sheet, (640.0, 600.0)), Ok(Tapped::Outside));
        assert_eq!(tapped(&sheet, (20.0, down)), Ok(Tapped::Outside));
    }

    #[test]
    fn the_sheet_is_centred_and_no_wider_than_it_needs_on_a_wide_screen() {
        let Ok(sheet) = sheet(Size { width: 1280, height: 800 }, &wearing());

        assert_eq!(sheet.card.size.width, WIDEST);
        assert_eq!(sheet.card.at.x, 460);
    }
}
