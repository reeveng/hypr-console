//! The panel the firmware lent, and the lines said on it.
//!
//! `console_kernel::screen` says where a character's dots go; this is the
//! edge that turns the address the firmware named into the pixels it draws
//! in, and keeps them where every line can reach them. They are kept in one
//! place behind the kernel's lock rather than handed along, because the last
//! thing to speak is a fault or a panic, and the processor hands those a
//! stack frame and nothing else.
//!
//! A fault does not wait for the lock. It may have interrupted the line that
//! holds it, and a kernel that hangs waiting to say why it stopped has said
//! nothing; the serial port, which has no lock, has the line either way.
//!
//! The pixels are written through volatile stores. The firmware's identity map is
//! still what the processor walks, and it covers the panel because the
//! firmware drew on it through the same map; when the kernel has page tables
//! of its own the panel is one of the things they will have to name.

use core::fmt::{self, Write};
use core::ops::Range;

use console_core_geometry::Size;
use console_core_never::Never;
use console_kernel::locking::SpinLock;
use console_kernel::screen::{
  FramebufferRegion, Panel, Pixels, Presentation, Screen, ScreenError,
};

use crate::firmware::Graphics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
  Rendered(Panel),
  NothingLent,
  Rejected(ScreenError),
}

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  allow(
    explicit044_no_ambient_value,
    reason = "a fault and a panic are entered with a stack frame and nothing else, so the panel their line is drawn on can only be where the whole kernel can reach it; the lock serializes it"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "fault entry receives no driver context; the display lock serializes access to the installed boot framebuffer"
  )
)]
static SCREEN: SpinLock<Option<Screen<'static>>> = match SpinLock::new(None) {
  Ok(lock) => lock,
};

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the validated GOP framebuffer is transferred to its exclusive display owner here"
  )
)]
pub fn open(graphics: Option<Graphics>) -> Result<Opened, Never> {
  Ok(match graphics {
    Some(Graphics {
      base,
      pixels,
      panel,
    }) => {
      let region = FramebufferRegion {
        base,
        pixels: Pixels(pixels),
      };

      // SAFETY: GOP supplied the mapped framebuffer and its 32-bit pixel count.
      // This runs once and gives the display lock sole access to that range.
      match unsafe { Screen::from_framebuffer(region, panel) } {
        Ok(screen) => {
          let Ok(mut held) = SCREEN.lock();
          *held = Some(screen);

          Opened::Rendered(panel)
        }
        Err(error) => Opened::Rejected(error),
      }
    }
    None => Opened::NothingLent,
  })
}

pub fn print(line: fmt::Arguments<'_>) -> Result<(), Never> {
  let Ok(mut held) = SCREEN.lock();

  match held.as_mut() {
    Some(screen) => match writeln!(screen, "{line}") {
      Ok(()) | Err(fmt::Error) => Ok(()),
    },
    None => Ok(()),
  }
}

pub(crate) fn reset() -> Result<(), Never> {
  let Ok(mut held) = SCREEN.lock();

  match held.as_mut() {
    Some(screen) => screen.reset(),
    None => Ok(()),
  }
}

pub fn draw_frame<const SIDE: usize>(rows: &[[u32; SIDE]]) -> Result<Presentation, Never> {
  let Ok(mut held) = SCREEN.lock();

  match held.as_mut() {
    Some(screen) => screen.draw_frame(rows),
    None => Ok(Presentation::Skipped),
  }
}

pub fn draw_monochrome_frame(size: Size<u32>, pixels: &[[u8; 4]]) -> Result<Presentation, Never> {
  let Ok(mut held) = SCREEN.lock();

  match held.as_mut() {
    Some(screen) => screen.draw_monochrome_frame(size, pixels),
    None => Ok(Presentation::Skipped),
  }
}

pub(crate) fn draw_bitmap_rows<const WORDS: usize>(
  bitmap: &[[u32; WORDS]],
  rows: Range<u64>,
) -> Result<Presentation, Never> {
  let Ok(mut held) = SCREEN.lock();

  match held.as_mut() {
    Some(screen) => screen.draw_bitmap_rows(bitmap, rows),
    None => Ok(Presentation::Skipped),
  }
}

pub fn write_without_waiting(what: fmt::Arguments<'_>) -> Result<(), Never> {
  let Ok(held) = SCREEN.try_lock();

  match held {
    Some(mut held_2) => match held_2.as_mut() {
      Some(screen) => match screen.write_fmt(what) {
        Ok(()) | Err(fmt::Error) => Ok(()),
      },
      None => Ok(()),
    },
    None => Ok(()),
  }
}
