//! Wire operations for a Wayland virtual keyboard.

use std::os::fd::BorrowedFd;

use console_core_never::Never;

use crate::client::globals::{BindError, GlobalList};
use crate::client::protocol::wl_seat::WlSeat;
use crate::client::{Dispatch, QueueHandle};
use crate::protocols_misc::zwp_virtual_keyboard_v1::client::{
  zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
  zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

pub fn manager<State>(
  globals: &GlobalList,
  hand: &QueueHandle<State>,
) -> Result<ZwpVirtualKeyboardManagerV1, BindError>
where
  State: Dispatch<ZwpVirtualKeyboardManagerV1, ()> + 'static,
{
  globals.bind(hand, 1..=1, ())
}

pub struct VirtualKeyboard(ZwpVirtualKeyboardV1);

pub struct Stroke {
  pub milliseconds: u32,
  pub code: u32,
}

impl VirtualKeyboard {
  #[must_use]
  pub fn new<State>(
    manager: &ZwpVirtualKeyboardManagerV1,
    seat: &WlSeat,
    hand: &QueueHandle<State>,
  ) -> Result<Self, Never>
  where
    State: Dispatch<ZwpVirtualKeyboardV1, ()> + 'static,
  {
    Ok(Self(manager.create_virtual_keyboard(seat, hand, ())))
  }

  pub fn keymap(&self, file: BorrowedFd<'_>, length: u32) -> Result<(), Never> {
    const XKB_V1: u32 = 1;
    self.0.keymap(XKB_V1, file, length);
    Ok(())
  }

  pub fn press(&self, Stroke { milliseconds, code }: Stroke) -> Result<(), Never> {
    self.0.key(milliseconds, code, 1);
    Ok(())
  }

  pub fn release(&self, Stroke { milliseconds, code }: Stroke) -> Result<(), Never> {
    self.0.key(milliseconds, code, 0);
    Ok(())
  }

  pub fn set_depressed_modifiers(&self, depressed: u32) -> Result<(), Never> {
    self.0.modifiers(depressed, 0, 0, 0);
    Ok(())
  }
}
