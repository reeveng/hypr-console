//! The kernel, booted -- or, on this machine, the same steps run as a program.
//!
//! Built for `x86_64-unknown-uefi` this is a UEFI program: the firmware starts
//! it, `serial` says its lines on COM1 and `display` draws the same lines on
//! the panel, which is the only place a handheld has, `firmware` lends the pool until the
//! memory map is read and boot services are exited, and `allocator` moves onto
//! the kernel's own heap from then on. `exceptions` loads the kernel's own
//! segments and interrupt gates as soon as there is a heap to keep them in,
//! so a fault from there on is a line and a failure rather than a reset, and
//! `timer` counts a few interrupts to show they arrive. In QEMU an attached
//! USB boot keyboard can feed the diagnostic input loop after the demo.
//! Escape exits the QEMU preview. On a real machine the kernel halts with
//! its output on the panel.
//!
//! Built for this machine it prints the same lines and answers with the same
//! status. That is what lets the kernel be a member of the workspace like any
//! other crate -- tested, linted and gated by `just ready` -- rather than a
//! tree of its own that the rules never reach.

#![cfg_attr(target_os = "uefi", no_std, no_main)]

#[cfg(target_os = "uefi")]
extern crate alloc;

#[cfg(target_os = "uefi")]
mod acpi;
#[cfg(target_os = "uefi")]
mod allocator;

#[cfg(target_os = "uefi")]
mod amd_vi;

#[cfg(target_os = "uefi")]
mod boot;

#[cfg(target_os = "uefi")]
mod display;

#[cfg(target_os = "uefi")]
mod exceptions;

#[cfg(target_os = "uefi")]
mod firmware;
#[cfg(target_os = "uefi")]
mod generation;
#[cfg(target_os = "uefi")]
mod input;

#[cfg(target_os = "uefi")]
mod ivrs;

#[cfg(target_os = "uefi")]
mod serial;

#[cfg(target_os = "uefi")]
mod timer;

#[cfg(target_os = "uefi")]
mod userspace;
#[cfg(target_os = "uefi")]
mod xhci;

#[cfg(not(target_os = "uefi"))]
fn main() -> std::process::ExitCode {
  let Ok(run) = console_kernel::run(console_kernel::HOSTED_MEMORY);

  for line in &run.log {
    println!("console-kernel: {line}");
  }

  match run.exit {
    console_kernel::Exit::Success => {
      println!("console-kernel: every step held");

      std::process::ExitCode::SUCCESS
    }
    console_kernel::Exit::Failure(error) => {
      eprintln!("console-kernel: stopped: {error}");

      std::process::ExitCode::FAILURE
    }
  }
}
