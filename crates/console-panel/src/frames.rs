//! The one picture on a card that changes while nobody presses anything.
//!
//! A row's square comes out of the picture store, at the size a row draws it,
//! made by another program for the next opening. A photograph being looked at
//! is drawn across most of the card, and a store of every one of those at that
//! size would be a store the size of the pictures folder -- so it is not kept
//! there. What is held here is the one picture on the screen: decoded on a
//! thread of its own the first time a card asks for it, and let go when a card
//! asks for another.
//!
//! A film is the same picture changing. Whoever plays it hands each frame to
//! [`put`], and the frame replaces the last one. The decode that was started
//! for the still is what a film shows before it starts, and it cannot land on
//! top of a frame the player has already put.
//!
//! **The draw loop is told rather than asked.** The loop sleeps in `poll` until
//! the compositor says something, and a picture that finished decoding while it
//! slept would stay undrawn until the next press. So there is a socket here,
//! and every change says which of three things it is: a frame, which repaints
//! the picture's own rectangle and nothing else; the card, which draws the
//! whole card again from the rows it already has; and the rows, which asks the
//! page for them again, because a film's clock under the picture has moved on.
//! A frame thirty times a second is thirty rectangles, and the words, the bar
//! and the buttons are painted when they change and not when the film does.
//! During that paint the slot lends the current pixels and captions together.
//! The paint runs synchronously, so their borrow ends before the slot unlocks.
//!
//! The same socket is how a card is shut. The host sets a flag and waits for
//! the loop to end, and a flag is only read when the loop next wakes -- which,
//! with nothing loading, was up to a whole resting poll later. Measured, a
//! panel took as long as a tenth of a second to close, all of it spent asleep,
//! and the host could open nothing else meanwhile. So shutting says so here
//! too, and the loop wakes to read the flag at once.
//!
//! The incoming and outgoing panels can overlap during handoff. Each drawing
//! thread has its own queue so either can drain notices while the other sleeps.

use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_shapes::Pixels;

#[derive(Debug, Clone, PartialEq, Eq)]
enum FrameState {
  Decoding,
  Decoded(Pixels),
  Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Looked {
  of: PathBuf,
  room: Size<u32>,
  held: FrameState,
  asked: Size<u32>,
  said: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
  Frame,
  Card,
  Rows,
  Closed,
  Accessibility,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WakeState {
  pub frame: FrameReceived,
  pub card: FrameReceived,
  pub rows: FrameReceived,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FrameReceived {
  Yes,
  #[default]
  No,
}

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  allow(
    explicit044_no_ambient_value,
    reason = "the one picture on the screen, handed from the thread that decoded or played it to the loop that draws it; the loop builds its rows from a page it did not write, so there is no value of its own this could be carried in"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "the wake dispatcher stores the latest compositor observation shared by all process-local surfaces"
  )
)]
static LOOKING_AT: Mutex<Option<Looked>> = Mutex::new(None);

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  allow(
    explicit044_no_ambient_value,
    reason = "panel producers publish notices without owning the active surfaces, which can overlap during handoff"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "each drawing thread registers its own wake socket and producers notify every live listener"
  )
)]
static LISTENERS: Mutex<Vec<Weak<Wake>>> = Mutex::new(Vec::new());

struct Wake {
  receiver: UnixStream,
  sender: UnixStream,
}

impl Wake {
  fn open() -> io::Result<Arc<Self>> {
    let (receiver, sender) = UnixStream::pair()?;
    receiver.set_nonblocking(true)?;
    sender.set_nonblocking(true)?;
    let wake = Arc::new(Self { receiver, sender });
    let mut listeners = LISTENERS
      .lock()
      .map_err(|_| io::Error::other("the wake queue registry is unavailable"))?;
    listeners.retain(|listener| listener.strong_count() != 0);
    listeners.push(Arc::downgrade(&wake));
    Ok(wake)
  }
}

std::thread_local! {
  static WAKE: Option<Arc<Wake>> = match Wake::open() {
    Ok(wake) => Some(wake),
    Err(fault) => {
      console_core_logging::warn!("console-panel: cannot create a wake queue: {fault}");
      None
    }
  };
}

pub(crate) fn with_waking<T>(wait: impl FnOnce(Option<BorrowedFd<'_>>) -> T) -> T {
  WAKE.with(|wake| wait(wake.as_ref().map(|wake| wake.receiver.as_fd())))
}

pub fn tell(what: Notice) -> Result<(), Never> {
  let byte = match what {
    Notice::Frame => b'f',
    Notice::Card => b'c',
    Notice::Rows => b'r',
    Notice::Closed => b's',
    Notice::Accessibility => b'a',
  };

  signal_ready(byte)
}

pub(crate) fn subscription_ready() -> Result<(), Never> {
  signal_ready(b'e')
}

fn signal_ready(byte: u8) -> Result<(), Never> {
  if let Ok(mut listeners) = LISTENERS.lock() {
    listeners.retain(|listener| {
      let Some(wake) = listener.upgrade() else {
        return false;
      };
      match rustix::io::write(&wake.sender, &[byte]) {
        Ok(_) | Err(rustix::io::Errno::AGAIN) => {}
        Err(fault) => console_core_logging::warn!("console-panel: waking the card: {fault}"),
      }
      true
    });
  }

  Ok(())
}

pub fn wake_state() -> Result<WakeState, Never> {
  with_waking(read_wakes)
}

fn read_wakes(told: Option<BorrowedFd<'_>>) -> Result<WakeState, Never> {
  let told = match told {
    Some(told) => told,
    None => return Ok(WakeState::default()),
  };

  let read = (0..1024).map_while(|_batch| {
    let mut read = [0u8; 64];

    match rustix::io::read(told, read.as_mut_slice()) {
      Ok(0) | Err(rustix::io::Errno::AGAIN) => None,
      Ok(many) => Some(read.into_iter().take(many)),
      Err(fault) => {
        console_core_logging::warn!("console-panel: reading the wake queue: {fault}");
        None
      }
    }
  });

  Ok(
    read
      .flatten()
      .fold(WakeState::default(), |heard, byte| match byte {
        b'f' => WakeState {
          frame: FrameReceived::Yes,
          ..heard
        },
        b'c' => WakeState {
          card: FrameReceived::Yes,
          ..heard
        },
        b'r' => WakeState {
          rows: FrameReceived::Yes,
          ..heard
        },
        _somebody_else => heard,
      }),
  )
}

pub fn current(of: &Path, room: Size<u32>) -> Result<Option<Pixels>, Never> {
  with_current(of, room, (), |(), pixels, _said| Ok(pixels.clone()))
}

pub(crate) fn snapshot(
  of: &Path,
  room: Size<u32>,
) -> Result<Option<(Pixels, Vec<String>)>, Never> {
  with_current(of, room, (), |(), pixels, said| {
    Ok((pixels.clone(), said.to_vec()))
  })
}

fn held_frame() -> Result<Option<MutexGuard<'static, Option<Looked>>>, Never> {
  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the resident frame lock covers only bounded memory transfers and synchronous rendering views; image decoding and filesystem I/O run after releasing it"
    )
  )]
  let holding = LOOKING_AT.lock();

  Ok(match holding {
    Ok(frame) => Some(frame),
    Err(_resident_frame_is_inconsistent) => None,
  })
}

pub(crate) fn with_current<Context, T>(
  of: &Path,
  room: Size<u32>,
  context: Context,
  view: impl FnOnce(Context, &Pixels, &[String]) -> Result<T, Never>,
) -> Result<Option<T>, Never> {
  let Ok(resident) = held_frame();
  let mut holding = match resident {
    Some(frame) => frame,
    None => return Ok(None),
  };

  match holding.as_mut() {
    Some(looked) => match ::core::primitive::bool::then_some(looked.of.as_path() == of, ()) {
      ::core::option::Option::Some(()) => {
        looked.room = room;

        return match &looked.held {
          FrameState::Decoded(pixels) => view(context, pixels, &looked.said).map(Some),
          FrameState::Decoding | FrameState::Failed => Ok(None),
        };
      }
      ::core::option::Option::None => {}
    },
    None => {}
  }

  *holding = Some(Looked {
    of: of.to_path_buf(),
    room,
    held: FrameState::Decoding,
    asked: room,
    said: Vec::new(),
  });

  drop(holding);

  let at = of.to_path_buf();
  let decoding = std::thread::spawn(move || {
    let read = match console_pictures::decoded(&at, room) {
      Ok(Some(pixels)) => FrameState::Decoded(pixels),
      Ok(None) => FrameState::Failed,
      Err(why) => {
        console_core_logging::warn!("console-panel: no picture: {why}");

        FrameState::Failed
      }
    };

    let Ok(()) = store_frame(&at, read);
  });

  let Ok(()) = console_program_lifetime::threads::let_go(decoding);

  Ok(None)
}

fn store_frame(at: &Path, read: FrameState) -> Result<(), Never> {
  let Ok(resident) = held_frame();
  let mut holding = match resident {
    Some(frame) => frame,
    None => return Ok(()),
  };

  let still_wanted = holding
    .as_ref()
    .is_some_and(|looked| looked.of.as_path() == at && looked.held == FrameState::Decoding);

  match ::core::primitive::bool::then_some(still_wanted, ()) {
    ::core::option::Option::Some(()) => {
      match holding.as_mut() {
        Some(looked) => looked.held = read,
        None => {}
      }

      drop(holding);

      tell(Notice::Card)
    }
    ::core::option::Option::None => Ok(()),
  }
}

pub fn sharpened(of: &Path, within: Size<u32>) -> Result<(), Never> {
  let Ok(resident) = held_frame();
  let mut holding = match resident {
    Some(frame) => frame,
    None => return Ok(()),
  };

  let looked = match holding.as_mut() {
    Some(looked) => looked,
    None => return Ok(()),
  };

  let larger = within.width > looked.asked.width || within.height > looked.asked.height;

  match (looked.of.as_path() == of, &looked.held, larger) {
    (true, FrameState::Decoded(_), true) => {}
    (false, _, _)
    | (true, FrameState::Decoding | FrameState::Failed, _)
    | (true, FrameState::Decoded(_), false) => return Ok(()),
  }

  looked.asked = within;

  drop(holding);

  let at = of.to_path_buf();
  let decoding = std::thread::spawn(move || {
    let read = match console_pictures::decoded(&at, within) {
      Ok(Some(pixels)) => pixels,
      Ok(None) => return,
      Err(why) => {
        console_core_logging::warn!("console-panel: no sharper picture: {why}");

        return;
      }
    };

    let Ok(()) = sharper(&at, within, read);
  });

  console_program_lifetime::threads::let_go(decoding)
}

fn sharper(at: &Path, within: Size<u32>, read: Pixels) -> Result<(), Never> {
  let Ok(resident) = held_frame();
  let mut holding = match resident {
    Some(frame) => frame,
    None => return Ok(()),
  };

  let looked = match holding.as_mut() {
    Some(looked) => looked,
    None => return Ok(()),
  };

  match ::core::primitive::bool::then_some(
    looked.of.as_path() == at && looked.asked == within,
    (),
  ) {
    ::core::option::Option::Some(()) => looked.held = FrameState::Decoded(read),
    ::core::option::Option::None => return Ok(()),
  }

  drop(holding);

  tell(Notice::Card)
}

pub fn room(of: &Path) -> Result<Option<Size<u32>>, Never> {
  let Ok(resident) = held_frame();
  let holding = match resident {
    Some(frame) => frame,
    None => return Ok(None),
  };

  Ok(
    holding
      .as_ref()
      .filter(|looked| looked.of.as_path() == of)
      .map(|looked| looked.room),
  )
}

pub fn said(of: &Path) -> Result<Vec<String>, Never> {
  let Ok(resident) = held_frame();
  let holding = match resident {
    Some(frame) => frame,
    None => return Ok(Vec::new()),
  };

  Ok(match holding.as_ref() {
    Some(looked) => match ::core::primitive::bool::then_some(looked.of.as_path() == of, ()) {
      ::core::option::Option::Some(()) => looked.said.clone(),
      ::core::option::Option::None => Vec::new(),
    },
    None => Vec::new(),
  })
}

pub fn insert(of: &Path, pixels: Pixels, said: Vec<String>) -> Result<(), Never> {
  let Ok(resident) = held_frame();
  let mut holding = match resident {
    Some(frame) => frame,
    None => return Ok(()),
  };

  let room = match holding.as_ref() {
    Some(looked) => match ::core::primitive::bool::then_some(looked.of.as_path() == of, ()) {
      ::core::option::Option::Some(()) => looked.room,
      ::core::option::Option::None => return Ok(()),
    },
    None => return Ok(()),
  };

  *holding = Some(Looked {
    of: of.to_path_buf(),
    room,
    held: FrameState::Decoded(pixels),
    asked: room,
    said,
  });

  drop(holding);

  tell(Notice::Frame)
}

#[cfg(test)]
#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  allow(
    explicit044_no_ambient_value,
    reason = "the picture on the screen is one per process and the tests share the process, so the tests that look at it take turns"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "these tests share the process wake endpoints and must serialize changes to that fixture"
  )
)]
static ONE_TEST_AT_A_TIME: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn serial_fixture() -> Result<MutexGuard<'static, ()>, Never> {
  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "these finite tests must take turns using the single process wake channel and release the guard before returning"
    )
  )]
  let turn = ONE_TEST_AT_A_TIME.lock();

  Ok(match turn {
    Ok(occupied) => occupied,
    Err(previous_case_panicked) => previous_case_panicked.into_inner(),
  })
}

#[cfg(test)]
pub(crate) fn a_red_picture(
  at: &Path,
  size: Size<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
  let area = u64::from(size.width).saturating_mul(u64::from(size.height));
  let Ok(area_2) = console_core_number_conversion::index(area);
  let rgba = [255, 0, 0, 255].repeat(area_2);
  let written =
    console_core_png_files::encoded(&console_core_png_files::Picture { size, bytes: rgba })?;

  console_core_atomic_writes::whole(at, &written)?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::cell::Cell;
  use std::time::Duration;

  type Failure = Box<dyn std::error::Error>;

  #[test]
  fn overlapping_panels_do_not_drain_each_others_wakes() -> Result<(), Failure> {
    let Ok(_turn) = serial_fixture();
    let outgoing = Wake::open()?;
    let incoming = Wake::open()?;
    let Ok(()) = tell(Notice::Card);
    let Ok(outgoing_wake) = read_wakes(Some(outgoing.receiver.as_fd()));
    let Ok(incoming_wake) = read_wakes(Some(incoming.receiver.as_fd()));

    assert_eq!(outgoing_wake.card, FrameReceived::Yes);
    assert_eq!(incoming_wake.card, FrameReceived::Yes);
    Ok(())
  }

  fn a_frame(red: u8) -> Result<Pixels, Never> {
    Ok(Pixels {
      width: 2,
      height: 1,
      stride: 8,
      bytes: vec![red, 0, 0, 255, red, 0, 0, 255],
    })
  }

  fn fresh_picture() -> Result<std::path::PathBuf, Failure> {
    let folder = console_core_temporary_directories::fresh("panel-frames")?;
    let at = folder.join("panel-frames.png");
    a_red_picture(
      &at,
      Size {
        width: 40,
        height: 30,
      },
    )?;

    Ok(at)
  }

  #[test]
  fn a_picture_asked_for_is_decoded_away_from_the_loop_and_the_loop_is_told()
  -> Result<(), Failure> {
    let Ok(_turn) = serial_fixture();
    let Ok(_before) = wake_state();
    let at = fresh_picture()?;
    let space = Size {
      width: 80,
      height: 80,
    };

    assert_eq!(
      current(&at, space),
      Ok(None),
      "the first ask is answered at once, with nothing yet"
    );

    let told = Cell::new(FrameReceived::No);
    let Ok(patience) = console_waiting::Schedule::of(Duration::from_secs(20));
    let Ok(drawn) = console_waiting::until_some(patience, || {
      let Ok(heard) = wake_state();

      match heard.card {
        FrameReceived::Yes => told.set(FrameReceived::Yes),
        FrameReceived::No => {}
      }

      current(&at, space)
    });
    std::fs::remove_file(&at)?;

    let drawn_2 = drawn.ok_or("the picture was never decoded")?;

    assert_eq!((drawn_2.width, drawn_2.height), (80, 60));

    let Ok(heard) = wake_state();

    match heard.card {
      FrameReceived::Yes => told.set(FrameReceived::Yes),
      FrameReceived::No => {}
    }

    assert_eq!(
      told.get(),
      FrameReceived::Yes,
      "the loop was never told the card had changed"
    );

    let film = Path::new("/nowhere/a-film.mkv");
    let Ok(bright) = a_frame(200);
    let Ok(dim) = a_frame(9);

    assert_eq!(current(film, space), Ok(None));
    assert_eq!(room(film), Ok(Some(space)));

    let Ok(()) = insert(film, bright.clone(), vec![String::from("Hello")]);
    let Ok(heard_2) = wake_state();

    assert_eq!(
      heard_2.frame,
      FrameReceived::Yes,
      "a frame put is a frame the loop hears about"
    );
    assert_eq!(current(film, space), Ok(Some(bright.clone())));
    assert_eq!(
      said(film),
      Ok(vec![String::from("Hello")]),
      "what is said over a frame comes with it"
    );
    assert_eq!(said(Path::new("/nowhere/another.mkv")), Ok(Vec::new()));

    let Ok(()) = store_frame(film, FrameState::Failed);

    assert_eq!(
      current(film, space),
      Ok(Some(bright.clone())),
      "a late still does not cover a frame"
    );

    let Ok(()) = insert(Path::new("/nowhere/another.mkv"), dim, Vec::new());

    assert_eq!(
      current(film, space),
      Ok(Some(bright)),
      "a frame of a film nobody is looking at is dropped"
    );

    Ok(())
  }
}
