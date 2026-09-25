//! Every event node, and the ones plugged in after the program started.
//!
//! At boot a pad can be enumerated after the unit that wants it, so
//! `/dev/input` is watched and a node that appears is opened; one that stops
//! answering is dropped. What waits is `poll` over all of them and over one
//! more descriptor the caller hands in, which is how the greeter hears the
//! login window and a press in the same wait. Nothing here has a timeout,
//! because nothing a person does arrives on a schedule. A node that is a
//! touchscreen is asked its axes when it is opened, and a finger on it wakes
//! the wait the way a press does.

use std::collections::HashSet;
use std::ffi::{CString, c_ulong};
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

use crate::device::{Device, INPUT};
use crate::event::LONG;
use crate::kernel::{self, IN, Waiting, checked};
use crate::presses::{ButtonPress, pressed};
use crate::touches::{ScreenTouch, Touchscreen};

const NODE: &str = "event";

const AT_ONCE: u32 = 64;

struct Node {
    path: PathBuf,
    file: File,
    screen: Option<Touchscreen>,
}

pub struct Devices {
    told: File,
    held: Vec<Node>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ready {
    Yes,
    No,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Woke {
    pub presses: Vec<ButtonPress>,
    pub touches: Vec<ScreenTouch>,
    pub also: Ready,
}

impl Devices {
    pub fn watched() -> io::Result<Devices> {
        // SAFETY: flags only; what comes back is a new descriptor or -1.
        let made = unsafe { kernel::inotify_init1(kernel::CLOSE_ON_EXEC | kernel::NOT_BLOCKING) };
        let descriptor = checked(made)?;

        // SAFETY: a descriptor the kernel just handed this process, owned by
        // nothing else, so the file is its only owner from here.
        let told = File::from(unsafe { OwnedFd::from_raw_fd(descriptor) });
        let path = CString::new(Path::new(INPUT).as_os_str().as_bytes());
        let path = match path {
            Ok(path) => path,
            Err(why) => return Err(io::Error::other(why)),
        };

        // SAFETY: a live descriptor and a NUL-ended path the call only reads.
        let added = unsafe { kernel::inotify_add_watch(told.as_raw_fd(), path.as_ptr(), kernel::CREATED | kernel::CHANGED) };

        checked(added)?;

        let mut devices = Devices { told, held: Vec::new() };
        let Ok(()) = devices.opened();

        Ok(devices)
    }

    fn opened(&mut self) -> Result<(), Never> {
        let entries = match fs::read_dir(INPUT) {
            Ok(entries) => entries,
            Err(why) => {
                eprintln!("cannot list {INPUT}: {why}");

                return Ok(());
            }
        };
        let known: HashSet<PathBuf> = self.held.iter().map(|device| device.path.clone()).collect();

        for entry in entries.flatten() {
            let path = entry.path();
            let node = entry.file_name().to_string_lossy().starts_with(NODE);
            let known = known.contains(&path);

            match (node, known) {
                (true, false) => match File::open(&path) {
                    Ok(file) => {
                        let Ok(screen) = screen(&path);

                        self.held.push(Node { path, file, screen });
                    }
                    Err(why) => eprintln!("cannot open {}: {why}", path.display()),
                },
                (true, true) | (false, _) => {}
            }
        }

        Ok(())
    }

    fn drained(&mut self) -> Result<(), Never> {
        let mut buffer = [0_u8; 4096];

        loop {
            match self.told.read(&mut buffer) {
                Ok(0) | Err(_) => return Ok(()),
                Ok(_) => {}
            }
        }
    }

    pub fn waited(&mut self, also: Option<BorrowedFd<'_>>) -> io::Result<Woke> {
        loop {
            let mut waiting = vec![Waiting { descriptor: self.told.as_raw_fd(), events: IN, returned: 0 }];

            waiting.extend(also.map(|also| Waiting { descriptor: also.as_raw_fd(), events: IN, returned: 0 }));
            waiting.extend(self.held.iter().map(|device| Waiting { descriptor: device.file.as_raw_fd(), events: IN, returned: 0 }));

            let Ok(many) = fitted::<_, c_ulong>(waiting.len());

            // SAFETY: `many` entries, all of them live descriptors this
            // struct or the caller holds for the length of the call.
            let polled = unsafe { kernel::poll(waiting.as_mut_ptr(), many, -1) };

            checked(polled)?;

            let stirred: HashSet<RawFd> = waiting
                .iter()
                .filter(|one| one.returned != 0)
                .map(|one| one.descriptor)
                .collect();
            let plugged = match stirred.contains(&self.told.as_raw_fd()) {
                true => Ready::Yes,
                false => Ready::No,
            };
            let also = match also.map(|also| stirred.contains(&also.as_raw_fd())) {
                Some(true) => Ready::Yes,
                Some(false) | None => Ready::No,
            };
            let readable: HashSet<u32> = (0..)
                .zip(self.held.iter())
                .filter(|(_, device)| stirred.contains(&device.file.as_raw_fd()))
                .map(|(at, _)| at)
                .collect();
            let Ok((presses, touches)) = self.read(&readable);

            match plugged {
                Ready::Yes => {
                    let Ok(()) = self.drained();
                    let Ok(()) = self.opened();
                }
                Ready::No => {}
            }

            match (presses.is_empty(), touches.is_empty(), also) {
                (true, true, Ready::No) => {}
                (false, _, _) | (_, false, _) | (true, true, Ready::Yes) => return Ok(Woke { presses, touches, also }),
            }
        }
    }

    fn read(&mut self, readable: &HashSet<u32>) -> Result<(Vec<ButtonPress>, Vec<ScreenTouch>), Never> {
        let mut presses = Vec::new();
        let mut touches = Vec::new();
        let mut kept = Vec::new();
        let Ok(room) = index(LONG.saturating_mul(AT_ONCE));
        let mut buffer = vec![0_u8; room];

        for (at, mut device) in (0..).zip(std::mem::take(&mut self.held)) {
            match readable.contains(&at) {
                false => kept.push(device),
                true => match device.file.read(&mut buffer) {
                    Ok(long) => {
                        let (whole, _) = buffer.split_at(long.min(buffer.len()));
                        let Ok(heard) = pressed(whole);

                        presses.extend(heard);

                        match device.screen.as_mut() {
                            Some(screen) => {
                                let Ok(felt) = screen.heard(whole);

                                touches.extend(felt);
                            }
                            None => {}
                        }

                        kept.push(device);
                    }
                    Err(why) => eprintln!("{} went away: {why}", device.path.display()),
                },
            }
        }

        self.held = kept;

        Ok((presses, touches))
    }
}

fn screen(path: &Path) -> Result<Option<Touchscreen>, Never> {
    match Device::open(path) {
        Ok(device) => Touchscreen::of(&device),
        Err(why) => {
            eprintln!("cannot ask {} what it is: {why}", path.display());

            Ok(None)
        }
    }
}
