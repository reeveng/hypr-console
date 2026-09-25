//! The ways an apply does not happen, named rather than said.
//!
//! An apply walks a manifest: packages, then what is built, then the files,
//! then the services. Every step of it can fail, and for a long time each of
//! them failed by handing back a sentence -- so the one caller that might have
//! done something about a fault, the one that puts back what it had already
//! laid down, could only print it. A file whose directory will not be made and
//! a machine that is out of disk are the same string.
//!
//! One enum for the whole engine, because one apply walks all of it, and every
//! sentence it used to print is the `Display` arm for the variant that replaced
//! it: the journal reads as it did, and `put_back` can now ask what kind of
//! fault it is looking at.

use std::fmt;
use std::path::PathBuf;

use crate::manifest::{MARK, ONCE};
use console_manifest_migrations::sweeping::Moment;

#[derive(Debug)]
pub enum Unapplied {
    NoKernelName(String, std::io::Error),
    AlreadyRunning,
    NotEnough(String),
    NoRoom(String),
    WentBack(Box<Unapplied>),
    Staging(String, &'static str, std::io::Error),
    NoUserFor(String, String),
    NothingBeside(String, &'static str),
    Unwritten(String, console_core_atomic_writes::Unwritten),
    Wrote(console_core_atomic_writes::Unwritten),
    NothingKept(String),
    NoUser(String),
    Owner(PathBuf, std::io::Error),
    Directory(PathBuf, std::io::Error),
    NoMatches(String),
    Read(PathBuf, std::io::Error),
    Unsaid(PathBuf, String),
    AsRoot(&'static str),
    PacmanRefused,
    PacmanUntold,
    Restore(Vec<String>),
    CargoRefused,
    NoProgram,
    CargoUnrun(std::io::Error),
    CargoUnwaited(std::io::Error),
    Making(PathBuf, std::io::Error),
    At(String, std::io::Error),
    Saving(String, console_core_atomic_writes::Unwritten),
    TwiceSaid(String, String, String),
    NoSuchSection(String, String),
    BeforeAnySection(String, String),
    OnceIsForFiles(String, String, String),
    OnlyOnce(String, String, String),
    WhatTimeItIs(std::io::Error),
    MigrationStopped(Moment, Box<Unapplied>),
    Undone(console_manifest_migrations::Undone),
    Pruning(PathBuf, String),
}

impl fmt::Display for Unapplied {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unapplied::NoKernelName(name, fault) => {
                write!(to, "{name} is not a name this kernel will hold: {fault}")
            }
            Unapplied::AlreadyRunning => {
                write!(to, "another console apply is running on this machine.")
            }
            Unapplied::NotEnough(said) | Unapplied::NoRoom(said) => write!(to, "{said}"),
            Unapplied::WentBack(fault) => {
                write!(to, "{fault} (and what was already down went back)")
            }
            Unapplied::Staging(live, what, fault) => {
                write!(to, "{live}: {what}: {fault}")
            }
            Unapplied::NoUserFor(live, owner) => {
                write!(to, "{live}: no user called {owner}")
            }
            Unapplied::NothingBeside(live, what) => {
                write!(to, "{live}: there is no file here to {what}")
            }
            Unapplied::Unwritten(live, fault) => write!(to, "{live}: {fault}"),
            Unapplied::Wrote(fault) => write!(to, "{fault}"),
            Unapplied::NothingKept(at) => {
                write!(to, "{at}: there is nothing kept here to put back")
            }
            Unapplied::NoUser(whoever) => write!(to, "no user called {whoever}"),
            Unapplied::Owner(at, fault) => {
                write!(to, "{}: its owner: {fault}", at.display())
            }
            Unapplied::Directory(at, fault) => {
                write!(to, "{}: its directory: {fault}", at.display())
            }
            Unapplied::NoMatches(named) => write!(
                to,
                "machines.conf has a machine called [{named}] that says no {}, so \
                 nothing could ever be it",
                crate::machines::MATCHES
            ),
            Unapplied::Read(at, fault) => {
                write!(to, "{} could not be read: {fault}", at.display())
            }
            Unapplied::Unsaid(at, fault) => {
                write!(to, "{} could not be read: {fault}", at.display())
            }
            Unapplied::AsRoot(what) => write!(to, "console {what} has to run as root."),
            Unapplied::PacmanRefused => {
                write!(to, "pacman could not install what the manifest asks for.")
            }
            Unapplied::PacmanUntold => {
                write!(to, "pacman would not be told the desktop asks for these.")
            }
            Unapplied::Restore(fell) => write!(
                to,
                "put back: {} would not run what this was about to install.",
                fell.join(", ")
            ),
            Unapplied::CargoRefused => {
                write!(to, "cargo could not build what the manifest asks for.")
            }
            Unapplied::NoProgram => {
                write!(to, "cargo was asked for with no program to run")
            }
            Unapplied::CargoUnrun(fault) => write!(to, "cargo could not be run: {fault}"),
            Unapplied::CargoUnwaited(fault) => {
                write!(to, "cargo could not be waited for: {fault}")
            }
            Unapplied::Making(at, fault) => write!(to, "{}: {fault}", at.display()),
            Unapplied::At(path, fault) => write!(to, "{path}: {fault}"),
            Unapplied::Saving(path, fault) => write!(to, "{path}: {fault}"),
            Unapplied::TwiceSaid(file, entry, under) => write!(
                to,
                "{file} says {entry:?} under [{under}] and so does {MARK}: a \
                 machine's own block is what is true there and not everywhere, so a \
                 line in both of them is one of them being wrong"
            ),
            Unapplied::NoSuchSection(file, name) => write!(
                to,
                "{file} has a section called [{name}], which is not one this reads"
            ),
            Unapplied::BeforeAnySection(file, line) => {
                write!(to, "{file} has {line:?} before any section")
            }
            Unapplied::OnceIsForFiles(file, entry, under) => write!(
                to,
                "{file} says {entry:?} under [{under}], and {ONCE} is a word only a \
                 file takes: a package or a unit has no inside for anyone to own"
            ),
            Unapplied::OnlyOnce(file, entry, under) => write!(
                to,
                "{file} says {entry:?} under [{under}], and the only word an entry takes \
                 after it is {ONCE}"
            ),
            Unapplied::WhatTimeItIs(fault) => write!(to, "what time it is: {fault}"),
            Unapplied::MigrationStopped(moment, fault) => write!(
                to,
                "migration {moment} stopped at {fault}, so nothing after it has run and \
                 nothing has been installed over it"
            ),
            Unapplied::Undone(fault) => write!(to, "{fault}"),
            Unapplied::Pruning(at, said) => write!(
                to,
                "{} left the manifest and could not be moved to the attic: {said}",
                at.display()
            ),
        }
    }
}

impl std::error::Error for Unapplied {}

impl From<console_manifest_migrations::Undone> for Unapplied {
    fn from(fault: console_manifest_migrations::Undone) -> Self {
        Unapplied::Undone(fault)
    }
}

impl From<console_core_atomic_writes::Unwritten> for Unapplied {
    fn from(fault: console_core_atomic_writes::Unwritten) -> Self {
        Unapplied::Wrote(fault)
    }
}
