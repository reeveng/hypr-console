//! A folder, and everything that is written, moved or thrown away under it.
//!
//! What a `Path` event group is. The machine already says when a file changes
//! -- inotify is the kernel's own answer and rustix carries it -- so a folder
//! is a source like the sound and the network are, rather than something each
//! of our programs has to remember to announce. The downloads used to say what
//! they had written, which told an open library about a download and about
//! nothing else: a screenshot, a book unzipped in the files, a song thrown
//! away, a film the browser saved -- all of them went unheard until the library
//! was closed and opened again. Asking the kernel hears every one of them, from
//! every program, including the ones this desktop did not write.
//!
//! What is said is the path of the thing that changed, whole, on the event
//! group of the folder that was asked for. Whoever listens decides what it
//! means; most of them read their folder again, which is what they did when
//! they opened.
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
use std::ops::ControlFlow;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::SyncSender;

use console_core_directory_listing::Descend;
use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_reconnect::{Round, keep};
use console_program_contract::{Change, EventGroup};
use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};

pub const MOST: u32 = 4096;

pub fn watch(folder: PathBuf, say: SyncSender<Change>) -> Result<(), Never> {
  keep(move || {
    let Ok(round) = round(&folder, &say);

    round
  })
}

fn watch_flags() -> Result<WatchFlags, Never> {
  Ok(
    WatchFlags::CLOSE_WRITE
      | WatchFlags::CREATE
      | WatchFlags::DELETE
      | WatchFlags::MOVED_FROM
      | WatchFlags::MOVED_TO
      | WatchFlags::ONLYDIR
      | WatchFlags::DONT_FOLLOW,
  )
}

fn round(folder: &Path, say: &SyncSender<Change>) -> Result<Round, Never> {
  let listening = match inotify::init(CreateFlags::CLOEXEC) {
    Ok(listening) => listening,
    Err(fault) => {
      eprintln!(
        "console-events: {} cannot be watched: {fault}",
        folder.display()
      );

      return Ok(Round::Another);
    }
  };

  let mut watched: BTreeMap<i32, PathBuf> = BTreeMap::new();
  let Ok(()) = added(&listening, folder, &mut watched);

  match ::core::primitive::bool::then_some(watched.is_empty(), ()) {
    ::core::option::Option::Some(()) => return Ok(Round::Another),
    ::core::option::Option::None => {}
  }

  let mut room = [MaybeUninit::<u8>::uninit(); 16 << 10];
  let reader = inotify::Reader::new(&listening, &mut room);

  let watching = iterate((reader, watched), |(mut reader_2, mut watched_2)| {
    let (flags, within, name) = match reader_2.next() {
      Ok(event) => (
        event.events(),
        event.wd(),
        event
          .file_name()
          .map(|name| OsStr::from_bytes(name.to_bytes()).to_os_string()),
      ),
      Err(_unreadable) => return Ok(Step::Halt(Round::Another)),
    };

    #[cfg_attr(
      dylint_lib = "explicit086_no_climbing_join",
      expect(
        explicit086_no_climbing_join,
        reason = "the inotify kernel event supplies one directory entry name relative to the watched directory"
      )
    )]
    let at = match (watched_2.get(&within), name) {
      (Some(inside), Some(name_2)) => inside.join(name_2),
      (Some(inside), None) => inside.clone(),
      (None, Some(_) | None) => folder.to_path_buf(),
    };

    let said = match (
      flags.contains(ReadFlags::QUEUE_OVERFLOW),
      flags.contains(ReadFlags::IGNORED),
      flags.contains(ReadFlags::ISDIR),
      flags.intersects(ReadFlags::CREATE | ReadFlags::MOVED_TO),
    ) {
      (true, _, _, _) => {
        console_core_logging::warn!(
          "the inotify queue overflowed for {}; asking listeners to rescan",
          folder.display()
        );

        Some(folder.to_path_buf())
      }
      (false, true, _, _) => {
        let _ = watched_2.remove(&within);

        match ::core::primitive::bool::then_some(watched_2.is_empty(), ()) {
          ::core::option::Option::Some(()) => return Ok(Step::Halt(Round::Another)),
          ::core::option::Option::None => None,
        }
      }
      (false, false, true, true) => {
        let Ok(()) = added(&listening, &at, &mut watched_2);

        Some(at)
      }
      (false, false, false, true) => {
        match ::core::primitive::bool::then_some(flags.contains(ReadFlags::CREATE), ()) {
          ::core::option::Option::Some(()) => None,
          ::core::option::Option::None => Some(at),
        }
      }
      (false, false, _, false) => Some(at),
    };

    let at_3 = match said {
      Some(at_2) => at_2,
      None => return Ok(Step::Again((reader_2, watched_2))),
    };

    let sent = say.send(Change {
      event_group: EventGroup::Path(folder.to_path_buf()),
      text: at_3.display().to_string(),
    });

    match sent {
      Ok(()) => {}
      Err(_nobody_is_left_to_tell) => return Ok(Step::Halt(Round::Finished)),
    }

    Ok(Step::Again((reader_2, watched_2)))
  });

  Ok(match watching {
    Ok(round) => round,
    Err(Endless) => Round::Another,
  })
}

fn added(
  listening: &OwnedFd,
  from: &Path,
  watched: &mut BTreeMap<i32, PathBuf>,
) -> Result<(), Never> {
  let Ok(flags) = watch_flags();
  let Ok(listing) = console_core_directory_listing::recursive(from, |path| {
    let Ok(shown) = shown(path);

    shown
  });
  let mut folders =
    std::iter::once(from.to_path_buf()).chain(listing.filter_map(|entry| match entry {
      Ok(path) => {
        let Ok(shown) = shown(&path);
        let metadata = std::fs::symlink_metadata(&path);

        match (metadata, shown) {
          (Ok(details), Descend::Into) => {
            match ::core::primitive::bool::then_some(details.is_dir(), ()) {
              ::core::option::Option::Some(()) => Some(path),
              ::core::option::Option::None => None,
            }
          }
          (Ok(_), Descend::Past) => None,
          (Err(_disappeared_or_unreadable), _) => None,
        }
      }
      Err(_unreadable) => None,
    }));

  let walked = folders.try_fold(watched, |watched_2, at| {
    let Ok(many) = fitted::<_, u32>(watched_2.len());

    match ::core::primitive::bool::then_some(many >= MOST, ()) {
      ::core::option::Option::Some(()) => return ControlFlow::Break(watched_2),
      ::core::option::Option::None => {}
    }

    match inotify::add_watch(listening, &at, flags) {
      Ok(within) => {
        let _ = watched_2.insert(within, at);
      }
      Err(rustix::io::Errno::NOENT | rustix::io::Errno::NOTDIR) => {
        return ControlFlow::Continue(watched_2);
      }
      Err(fault) => {
        console_core_logging::warn!("console-events: could not watch {}: {fault}", at.display());
      }
    }

    ControlFlow::Continue(watched_2)
  });

  match walked {
    ControlFlow::Break(_full) => eprintln!(
      "console-events: {} has more than {MOST} folders under it, and what is deeper \
             than that is not watched",
      from.display()
    ),
    ControlFlow::Continue(_every_folder) => {}
  }

  Ok(())
}

fn shown(path: &Path) -> Result<Descend, Never> {
  Ok(
    match path
      .file_name()
      .map(|name| name.as_bytes().first() == Some(&b'.'))
    {
      Some(true) => Descend::Past,
      Some(false) | None => Descend::Into,
    },
  )
}
