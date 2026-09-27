//! What the ring hears when it is drawn over a session rather than before one.
//!
//! Before a login there is no compositor, so the greeter reads the pad and the
//! touchscreen off the kernel itself. Choosing a pattern in the settings and
//! answering a lock are both drawn on a surface the compositor hands over, so
//! the pad arrives as the keys every panel reads and a finger as the pointer
//! every panel hears. Both are turned into the same presses and touches the
//! greeter decides with here, once, so the two surfaces cannot come to mean
//! different things by the same key.

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_draw_surface::{Keysym, PointerEvent};
use console_input_event_devices::presses::ButtonPress;
use console_login_pattern::Touch;

use crate::picture::in_the_room;

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
