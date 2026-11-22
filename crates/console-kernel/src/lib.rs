//! What the kernel does once it is up, log a line at a time.
//!
//! Today that is the handle model walked end to end: a root process, a
//! browser spawned from it, and a site renderer drawing a plain-text data URL
//! into a memory object. The browser receives the frame through its site
//! channel. The kernel enforces execution policy, channel bounds and memory
//! budgets, revokes a permission and reclaims the job when its last tab closes.
//!
//! It is here rather than beside the firmware because nothing in it touches a
//! machine. The same steps run in QEMU under `just kernel`, and here as a test
//! and under `cargo run`, and the lines they say are the same lines in all
//! three, so a step that stops holding is red on this machine before anything
//! is booted.

#![no_std]

extern crate alloc;

pub mod compute;
pub mod events;
pub mod frame_mappings;
pub mod interrupts;
pub mod locking;
pub mod memory;
pub mod scheduling;
pub mod screen;
pub mod usb_keyboard;
pub mod user_buffers;

#[cfg(any(test, kani))]
#[path = "../proofs/revocation_budget.rs"]
mod revocation_budget;

#[cfg(kani)]
#[path = "../proofs/file_recovery.rs"]
mod file_recovery;

#[cfg(kani)]
#[path = "../proofs/fusion.rs"]
mod fusion;

#[cfg(kani)]
#[path = "../proofs/protocols.rs"]
mod protocols;

#[cfg(not(target_os = "uefi"))]
use alloc::{string::String, vec::Vec};
use core::fmt;

use console_core_geometry::Size;
use console_core_never::Never;
pub use console_kernel_handles::Bytes;
use console_kernel_handles::HandleError;

mod browser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
  Success,
  Failure(HandleError),
}

#[cfg(not(target_os = "uefi"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
  pub log: Vec<String>,
  pub exit: Exit,
}

pub const BROWSER_SHARE: Bytes = Bytes(1_048_576);

pub const HOSTED_MEMORY: Bytes = Bytes(268_435_456);

#[cfg(not(target_os = "uefi"))]
#[cfg_attr(
  dylint_lib = "explicit066_no_heap_after_init",
  expect(
    explicit066_no_heap_after_init,
    reason = "the hosted transcript adapter runs in userspace and stores the lines for tests; firmware streams them directly"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit047_no_captured_permission",
  expect(
    explicit047_no_captured_permission,
    reason = "the hosted transcript callback borrows only a local buffer synchronously for the duration of run_logging"
  )
)]
pub fn run(memory: Bytes) -> Result<Run, Never> {
  let mut log = Vec::new();
  let Ok(exit) = run_logging(memory, |line| log.push(alloc::fmt::format(line)));
  Ok(Run { log, exit })
}

pub fn run_logging(memory: Bytes, log: impl FnMut(fmt::Arguments<'_>)) -> Result<Exit, Never> {
  run_with_frame(memory, log, |_, _| {})
}

pub fn run_with_frame(
  memory: Bytes,
  mut log: impl FnMut(fmt::Arguments<'_>),
  mut present: impl FnMut(Size<u32>, &[[u8; 4]]),
) -> Result<Exit, Never> {
  Ok(match browser::run(memory, &mut log, &mut present) {
    Ok(()) => Exit::Success,
    Err(error) => Exit::Failure(error),
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn every_step_holds_and_says_so_in_order() -> Result<(), Never> {
    let Ok(run) = run(HOSTED_MEMORY);

    assert_eq!(run.exit, Exit::Success);
    assert_eq!(
      run.log,
      [
        "the browser opened a site in a job with 983040 bytes",
        "a 640 by 360 plain-text page reached the browser as read-only memory",
        "the site's job refused to make memory executable even with the resource",
        "the renderer read nothing, and the browser was stopped after 64 messages: the channel holds no more",
        "the browser asked for as much memory as its whole job was given, and was refused",
        "the root took back the executable resource from the renderer",
        "the replacement renderer reopened the page in the same tab",
        "closing the last tab ended the renderer and returned the site's share",
      ]
    );

    Ok(())
  }

  #[test]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the test compares a fixed run using the slice API and the expected array length"
    )
  )]
  fn the_walkthrough_displays_the_decoded_text() -> Result<(), Never> {
    let frames = core::cell::Cell::new(0_u32);
    let Ok(exit) = run_with_frame(
      HOSTED_MEMORY,
      |_| {},
      |size, pixels| {
        frames.set(frames.get().saturating_add(1));
        assert_eq!(
          size,
          Size {
            width: 640,
            height: 360
          }
        );
        let paper = [255; 4];
        let ink = [0, 0, 0, 255];
        assert_eq!(pixels.first(), Some(&paper));

        let start = 10_256_usize;
        let scanline = [
          paper, paper, paper, paper, ink, ink, ink, ink, ink, ink, ink, ink,
        ];
        assert_eq!(
          pixels.get(start..start.saturating_add(scanline.len())),
          Some(scanline.as_slice())
        );
        let blank = pixels.get(20_480..30_720);
        assert!(blank.is_some_and(|rows| rows.iter().all(|pixel| *pixel == paper)));
        assert_eq!(pixels.last(), Some(&paper));
      },
    );

    assert_eq!(exit, Exit::Success);
    assert_eq!(frames.get(), 2);

    Ok(())
  }
}
