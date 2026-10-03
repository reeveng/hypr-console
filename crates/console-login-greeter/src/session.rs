//! What the ring hears when it is drawn over a session rather than before one.
//!
//! Before a login there is no compositor, so the greeter reads the pad and the
//! touchscreen off the kernel itself. Choosing a pattern in the settings and
//! answering a lock are both drawn on a surface the compositor hands over, so
//! the pad arrives as the keys every panel reads and a finger as the pointer
//! every panel hears. Both are turned into the same presses and touches the
//! greeter decides with here, once, so the two surfaces cannot come to mean
//! different things by the same key.
//!
//! A lock draws the bar over the ring as well, so a finger put down on it is a
//! tap on the bar rather than the start of a pattern: [`heard`] asks the bar
//! that was drawn last before it asks the ring.

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::toward_zero_i32;
use console_draw_surface::{Keysym, PointerEvent};
use console_input_event_devices::presses::ButtonPress;
use console_login_pattern::Touch;
use console_status_bar::lock_screen::LockScreenBarEvent;
use console_status_bar::showing::Rendered;

use crate::greeting::GreeterEvent;
use crate::picture::in_the_room;

pub fn heard(event: PointerEvent, logical: Option<Size<u32>>, bar: &Rendered) -> Result<Option<GreeterEvent>, Never> {
    let tapped = match event {
        PointerEvent::Down { at } => {
            let Ok(across) = toward_zero_i32(at.0);
            let Ok(down) = toward_zero_i32(at.1);
            let Ok(on) = bar.on(Point { x: across, y: down });

            on
        }
        PointerEvent::Moved { .. } | PointerEvent::Up | PointerEvent::Scrolled { .. } | PointerEvent::Pinched { .. } | PointerEvent::Left => None,
    };

    match tapped {
        Some(action) => Ok(Some(GreeterEvent::Bar(LockScreenBarEvent::Tapped(action)))),
        None => {
            let Ok(touched) = touch(event, logical);

            Ok(touched.map(GreeterEvent::Touched))
        }
    }
}

pub fn touch(event: PointerEvent, logical: Option<Size<u32>>) -> Result<Option<Touch>, Never> {
    let room = |at: (f64, f64)| {
        logical.map(|canvas| {
            let Ok(room) = in_the_room(canvas, Point { x: at.0, y: at.1 });

            room
        })
    };

    Ok(match event {
        PointerEvent::Down { at } => room(at).map(Touch::Down),
        PointerEvent::Moved { at } => room(at).map(Touch::Moved),
        PointerEvent::Up => Some(Touch::Up),
        PointerEvent::Scrolled { .. } | PointerEvent::Pinched { .. } | PointerEvent::Left => None,
    })
}

pub fn press(key: Keysym) -> Result<Option<ButtonPress>, Never> {
    Ok(match key {
        Keysym::Up => Some(ButtonPress::Up),
        Keysym::Down => Some(ButtonPress::Down),
        Keysym::Left => Some(ButtonPress::Left),
        Keysym::Right => Some(ButtonPress::Right),
        Keysym::Return | Keysym::KP_Enter | Keysym::space => Some(ButtonPress::Choose),
        Keysym::Escape | Keysym::BackSpace => Some(ButtonPress::Back),
        _ => None,
    })
}
