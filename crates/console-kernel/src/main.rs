//! The kernel, booted -- or, on this machine, the same steps run as a program.
//!
//! Built for `x86_64-unknown-uefi` this is a UEFI program: the firmware starts
//! it, `serial` says its lines on COM1, `firmware` lends the pool until the
//! memory map is read and boot services are exited, and `allocator` moves onto
//! the kernel's own heap from then on. `boot` says each line of
//! `console_kernel::run` and then writes the answer to the exit port QEMU is
//! started with, so a run ends with a status rather than a window somebody has
//! to close. On a real machine that port is nobody's and the kernel halts.
//!
//! Built for this machine it prints the same lines and answers with the same
//! status. That is what lets the kernel be a member of the workspace like any
//! other crate -- tested, linted and gated by `just ready` -- rather than a
//! tree of its own that the rules never reach.

#![cfg_attr(target_os = "uefi", no_std, no_main)]

#[cfg(target_os = "uefi")]
extern crate alloc;

#[cfg(target_os = "uefi")]
mod allocator;

#[cfg(target_os = "uefi")]
mod boot;

#[cfg(target_os = "uefi")]
mod firmware;

#[cfg(target_os = "uefi")]
mod serial;

#[cfg(not(target_os = "uefi"))]
fn main() -> std::process::ExitCode {
    let Ok(run) = console_kernel::run();

    for line in &run.log {
        println!("console-kernel: {line}");
    }

    match run.exit {
        console_kernel::Exit::Success => {
            println!("console-kernel: every step held");

            std::process::ExitCode::SUCCESS
        },
        console_kernel::Exit::Failure(error) => {
            eprintln!("console-kernel: stopped: {error}");

            std::process::ExitCode::FAILURE
        },
    }
}
