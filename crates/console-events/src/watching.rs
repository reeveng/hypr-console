//! A folder, and everything that is written, moved or thrown away under it.
//!
//! What a `Path` topic is. The machine already says when a file changes --
//! inotify is the kernel's own answer and rustix carries it -- so a folder is a
//! source like the sound and the network are, rather than something each of
//! our programs has to remember to announce. The downloads used to say what
//! they had written, which told an open library about a download and about
//! nothing else: a screenshot, a book unzipped in the files, a song thrown
//! away, a film the browser saved -- all of them went unheard until the
//! library was closed and opened again. Asking the kernel hears every one of
//! them, from every program, including the ones this desktop did not write.
//!
//! What is said is the path of the thing that changed, whole, on the topic of
//! the folder that was asked for. Whoever listens decides what it means; most
//! of them read their folder again, which is what they did when they opened.
//!
//! **A folder is watched with every folder under it.** inotify watches one
//! directory and not what is inside its children, so each one is added as the
//! walk meets it and each one made later is added as it arrives. A folder
//! whose name starts with a dot is not walked into: it is somebody's cache or
//! somebody's repository, and neither is a thing a library lists.
//! [`MOST`] is where the walk stops, because a folder anyone may ask for could
//! be one with a hundred thousand folders in it, and a pool that spent the
//! kernel's watches on it would be spending them for every other program on
//! the machine too. Past it, what is deeper is not heard and the journal says
//! so once.
//!
//! **A folder that is not there yet is asked for again.** Books is made by the
//! first book fetched into it, and a watch cannot be put on a folder that does
//! not exist. So a round that finds nothing to watch is a round that ends, and
//! `keep` makes it again; a folder that is thrown away ends its round the same
//! way. What lands in a folder in the seconds before it is watched is not
//! heard, which is a library read when it is next opened -- the way every one
//! of them was before there was anything to hear.
//!
//! **An overflow is said as the folder itself.** The kernel keeps a queue, and a
//! burst longer than the queue loses what did not fit and says so. Whoever was
//! listening is told the folder changed, which is true and is enough for
//! anything that reads the folder again.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_reconnect::{Round, keep};
use console_program_contract::{Change, Topic};
use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};

pub const MOST: u32 = 4096;

const ROOM: u32 = 16 << 10;

pub fn watch(folder: PathBuf, say: Sender<Change>) -> Result<(), Never> {
    keep(move || {
        let Ok(round) = round(&folder, &say);

        round
    })
}

fn asked() -> Result<WatchFlags, Never> {
    Ok(WatchFlags::CLOSE_WRITE
        | WatchFlags::CREATE
        | WatchFlags::DELETE
        | WatchFlags::MOVED_FROM
        | WatchFlags::MOVED_TO
        | WatchFlags::DONT_FOLLOW)
}

fn round(folder: &Path, say: &Sender<Change>) -> Result<Round, Never> {
    let listening = match inotify::init(CreateFlags::CLOEXEC) {
        Ok(listening) => listening,
        Err(fault) => {
            eprintln!("console-events: {} cannot be watched: {fault}", folder.display());

            return Ok(Round::Another);
        }
    };

    let mut watched: BTreeMap<i32, PathBuf> = BTreeMap::new();
    let Ok(()) = added(&listening, folder, &mut watched);

    match watched.is_empty() {
        true => return Ok(Round::Another),
        false => {},
    }

    let Ok(roomy) = index(ROOM);
    let mut room = vec![MaybeUninit::<u8>::uninit(); roomy];
    let mut reader = inotify::Reader::new(&listening, &mut room);

    loop {
        let (flags, within, name) = match reader.next() {
            Ok(event) => (
                event.events(),
                event.wd(),
                event.file_name().map(|name| OsStr::from_bytes(name.to_bytes()).to_os_string()),
            ),
            Err(_unreadable) => return Ok(Round::Another),
        };

        let at = match (watched.get(&within), name) {
            (Some(inside), Some(name)) => inside.join(name),
            (Some(inside), None) => inside.clone(),
            (None, Some(_) | None) => folder.to_path_buf(),
        };

        let said = match (
            flags.contains(ReadFlags::QUEUE_OVERFLOW),
            flags.contains(ReadFlags::IGNORED),
            flags.contains(ReadFlags::ISDIR),
            flags.intersects(ReadFlags::CREATE | ReadFlags::MOVED_TO),
        ) {
            (true, _, _, _) => Some(folder.to_path_buf()),
            (false, true, _, _) => {
                let _ = watched.remove(&within);

                match watched.is_empty() {
                    true => return Ok(Round::Another),
                    false => None,
                }
            }
            (false, false, true, true) => {
                let Ok(()) = added(&listening, &at, &mut watched);

                Some(at)
            }
            (false, false, false, true) => match flags.contains(ReadFlags::CREATE) {
                true => None,
                false => Some(at),
            },
            (false, false, _, false) => Some(at),
        };

        let at = match said {
            Some(at) => at,
            None => continue,
        };

        let sent = say.send(Change { topic: Topic::Path(folder.to_path_buf()), text: at.display().to_string() });

        match sent {
            Ok(()) => {},
            Err(_nobody_is_left_to_tell) => return Ok(Round::Finished),
        }
    }
}

fn added(listening: &OwnedFd, from: &Path, watched: &mut BTreeMap<i32, PathBuf>) -> Result<(), Never> {
    let Ok(flags) = asked();
    let mut walking = vec![from.to_path_buf()];

    while let Some(at) = walking.pop() {
        let Ok(many) = fitted::<_, u32>(watched.len());

        match many >= MOST {
            true => {
                eprintln!(
                    "console-events: {} has more than {MOST} folders under it, and what is deeper \
                     than that is not watched",
                    from.display()
                );

                return Ok(());
            }
            false => {},
        }

        let within = match inotify::add_watch(listening, &at, flags) {
            Ok(within) => within,
            Err(_gone_or_not_a_folder) => continue,
        };

        let _ = watched.insert(within, at.clone());

        let reading = match std::fs::read_dir(&at) {
            Ok(reading) => reading,
            Err(_unreadable) => continue,
        };

        for entry in reading.flatten() {
            let hidden = entry.file_name().as_bytes().first() == Some(&b'.');

            match (hidden, entry.file_type().map(|kind| kind.is_dir())) {
                (false, Ok(true)) => walking.push(entry.path()),
                (false, Ok(false)) | (true, Ok(_)) => {},
                (false | true, Err(_the_kind_is_unknown)) => {},
            }
        }
    }

    Ok(())
}
