//! What the kernel does once it is up, log a line at a time.
//!
//! Today that is the handle model walked end to end: a root process, a
//! browser spawned from it, a page and a piece of memory sent down the
//! channel, the browser refused the right to run what it was given, the root
//! allowed it, and the browser ended with everything it held going with it.
//!
//! It is here rather than beside the firmware because nothing in it touches a
//! machine. The same steps run in QEMU under `just kernel`, and here as a test
//! and under `cargo run`, and the lines they say are the same lines in all
//! three, so a step that stops holding is red on this machine before anything
//! is booted.

#![no_std]

extern crate alloc;

pub mod memory;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use console_core_never::Never;
use console_kernel_handles::{Boot, Condition, HandleError, HandleValue, Kernel, ProcessId, Right, Rights};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Success,
    Failure(HandleError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub log: Vec<String>,
    pub exit: Exit,
}

struct Browser {
    job: HandleValue,
    process: ProcessId,
    bootstrap: HandleValue,
    channel: HandleValue,
}

struct Lent {
    memory: HandleValue,
    resource: HandleValue,
    job: HandleValue,
}

pub fn run() -> Result<Run, Never> {
    let mut log = Vec::new();

    let exit = match steps(&mut log) {
        Ok(()) => Exit::Success,
        Err(error) => Exit::Failure(error),
    };

    Ok(Run { log, exit })
}

fn steps(log: &mut Vec<String>) -> Result<(), HandleError> {
    let Boot { mut kernel, root, root_job, executable_resource } = Kernel::boot()?;
    let browser = spawn_browser(&mut kernel, root, root_job, log)?;
    let lent = lend(&mut kernel, root, &browser, executable_resource, log)?;
    let renderer = start_renderer(&mut kernel, &browser, &lent, log)?;

    refused(&mut kernel, &browser, &lent, log)?;
    kernel.revoke(root, executable_resource)?;

    match kernel.handle_rights(browser.process, lent.resource) {
        Err(HandleError::BadHandle) => log.push(String::from("the root took back the resource it lent, and the browser no longer holds it")),
        Err(error) => return Err(error),
        Ok(_) => return Err(HandleError::BadState),
    }

    kernel.kill(root, browser.job)?;

    match (kernel.handles(browser.process), kernel.handles(renderer)) {
        (Err(HandleError::ProcessNotFound), Err(HandleError::ProcessNotFound)) => {
            log.push(String::from("the root ended the browser's job, and the browser and its renderer went with it"));
        },
        (Err(error), _) | (_, Err(error)) => return Err(error),
        (Ok(_), Ok(_)) => return Err(HandleError::BadState),
    }

    Ok(())
}

fn spawn_browser(kernel: &mut Kernel, root: ProcessId, root_job: HandleValue, log: &mut Vec<String>) -> Result<Browser, HandleError> {
    let job = kernel.create_job(root, root_job)?;

    kernel.set_policy(root, job, &[Condition::ReplaceAsExecutable])?;
    log.push(String::from("a job for the browser, which refuses to make memory executable"));

    let created = kernel.create_process(root, job)?;
    let held = kernel.handles(created.id)?;

    log.push(format!("a browser is born in it holding {} handle", held.len()));

    let bootstrap = match held.first() {
        Some(bootstrap) => *bootstrap,
        None => return Err(HandleError::BadHandle),
    };

    Ok(Browser { job, process: created.id, bootstrap, channel: created.channel })
}

fn lend(
    kernel: &mut Kernel,
    root: ProcessId,
    browser: &Browser,
    executable_resource: HandleValue,
    log: &mut Vec<String>,
) -> Result<Lent, HandleError> {
    let Ok(managing) = Rights::from_rights(&[Right::ManageJob, Right::Transfer]);
    let job = kernel.duplicate(root, browser.job, managing)?;
    let Ok(lendable) = Rights::from_rights(&[Right::Transfer]);
    let resource = kernel.duplicate(root, executable_resource, lendable)?;
    let memory = kernel.create_memory(root)?;

    kernel.write(root, browser.channel, b"a page", &[memory, resource, job])?;

    let message = kernel.read(browser.process, browser.bootstrap)?;

    let page = match core::str::from_utf8(&message.bytes) {
        Ok(page) => page,
        Err(_not_text) => "something that is not text",
    };

    log.push(format!("the browser read \"{page}\" and {} handles: memory, the executable resource and its own job", message.handles.len()));

    match message.handles.as_slice() {
        [memory, resource, job] => Ok(Lent { memory: *memory, resource: *resource, job: *job }),
        _ => Err(HandleError::BadHandle),
    }
}

fn start_renderer(kernel: &mut Kernel, browser: &Browser, lent: &Lent, log: &mut Vec<String>) -> Result<ProcessId, HandleError> {
    let renderer = kernel.create_process(browser.process, lent.job)?;

    log.push(String::from("the browser started a renderer inside its own job"));

    Ok(renderer.id)
}

fn refused(kernel: &mut Kernel, browser: &Browser, lent: &Lent, log: &mut Vec<String>) -> Result<(), HandleError> {
    match kernel.replace_as_executable(browser.process, lent.memory, lent.resource) {
        Err(error) => log.push(format!("the browser holds the executable resource and was still refused: {error}")),
        Ok(_) => return Err(HandleError::DeniedByPolicy(Condition::ReplaceAsExecutable)),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_step_holds_and_says_so_in_order() -> Result<(), Never> {
        let Ok(run) = run();

        assert_eq!(run.exit, Exit::Success);
        assert_eq!(
            run.log,
            [
                "a job for the browser, which refuses to make memory executable",
                "a browser is born in it holding 1 handle",
                "the browser read \"a page\" and 3 handles: memory, the executable resource and its own job",
                "the browser started a renderer inside its own job",
                "the browser holds the executable resource and was still refused: its job does not let it make memory executable",
                "the root took back the resource it lent, and the browser no longer holds it",
                "the root ended the browser's job, and the browser and its renderer went with it",
            ]
        );

        Ok(())
    }
}
