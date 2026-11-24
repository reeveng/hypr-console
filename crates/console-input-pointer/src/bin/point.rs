//! Put the pointer somewhere on the screen, at a place on the picture.
//!
//!     console-point [--in NAMESPACE] ACROSS DOWN [--click] [--scroll NOTCHES] [--drag ACROSS DOWN ...]
//!  Where ACROSS and DOWN are a place on the picture, in the size the desktop
//! is laid out in -- the numbers `hyprctl layers` answers in and the numbers
//! anything drawing a surface thinks in. How big that is, is asked of the
//! compositor rather than read out of the tree: the density is a setting
//! someone can change while the desktop is running, and a place measured
//! against the size the screen used to be lands somewhere else entirely.
//! `--in` says the place is inside a surface instead, measured from its corner:
//! `--in settings-panel 40 60` is forty across and sixty down from wherever the
//! compositor has put that panel, once it has put it anywhere: a surface that
//! is not up yet is waited for rather than guessed at, which is what the
//! seconds of sleep in front of every press in the nested desktop used to be. The corner is asked for here, in the session
//! being pointed at, at the moment of the press -- which is the whole reason it
//! is a word on this command line rather than arithmetic in a check. A check
//! that measured the corner first would be pointing at where the panel was, and
//! a check that guessed it would be pressing a place no one drew. Both of those
//! go green just as readily as the right answer does.  The two questions are
//! asked of the same walk `console_onscreen` already makes for "is this up", so
//! a surface this can be measured against is exactly a surface the rest of the
//! desktop agrees is on the screen.  It asks the compositor for a pointer of
//! its own rather than making one out of uinput, and `console_input_pointer`
//! says why. What that buys is the nested desktop: the emulator is a compositor
//! inside a window on this machine, a uinput mouse would move the laptop's
//! pointer and never the one in the picture, and a pointer asked for over the
//! socket is inside the picture where the checks are looking. So the same
//! command presses the device and presses the emulator, and a check that could
//! only be written for one of them can now be written for both.  The pointer
//! stays where it is put. The virtual pointer goes when this program does, and
//! the compositor keeps the cursor where the last motion left it, so a
//! screenshot taken seconds later still has the pointer standing on whatever
//! this stood it on.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_input_pointer::{
  COMMAND, Measured, PointerAction, Request, Unsaid, Where, approach, from_the_corner,
  on_the_screen, request,
};
use console_waiting::{Schedule, until_some};
use console_wayland::pointer::{self, Button};

const LOGICAL_PIXELS_PER_NOTCH: u32 = 15;

const STEP: Duration = Duration::from_millis(60);

const PRESSED: Duration = Duration::from_millis(80);

const HELD: Duration = Duration::from_millis(400);

const APPEARING: Duration = Duration::from_secs(10);

fn main() -> ExitCode {
  let words: Vec<String> = std::env::args().skip(1).collect();
  let asked = match console_core_arguments::read(&COMMAND, &words).and_then(|line| request(&line))
  {
    Ok(asked) => asked,
    Err(refusal) => {
      let Ok(code) = refusal.print();

      return ExitCode::from(code);
    }
  };

  match run(asked) {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      eprintln!("console-point: {fault}");

      ExitCode::FAILURE
    }
  }
}

#[derive(Debug)]
enum Unpointed {
  Unsaid(Unsaid),
  Undeclared(console_screen::Undeclared),
  Onscreen(console_onscreen::Error),
  NotUp(String),
  PastTheSurface(Point<u32>, Size<u32>, String),
  OffTheScreen(Point<u32>, Size<u32>),
  Wayland(pointer::Error),
}

impl std::fmt::Display for Unpointed {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Unpointed::Unsaid(fault) => write!(to, "{fault}"),
      Unpointed::Undeclared(fault) => write!(to, "{fault}"),
      Unpointed::Onscreen(fault) => write!(to, "{fault}"),
      Unpointed::NotUp(namespace) => write!(
        to,
        "nothing called {namespace} is on the screen, so there is no corner to measure from"
      ),
      Unpointed::PastTheSurface(at, room, namespace) => write!(
        to,
        "({}, {}) is past the {}x{} of {namespace}",
        at.x, at.y, room.width, room.height
      ),
      Unpointed::OffTheScreen(at, room) => write!(
        to,
        "({}, {}) is not on a {}x{} screen",
        at.x, at.y, room.width, room.height
      ),
      Unpointed::Wayland(fault) => write!(to, "{fault}"),
    }
  }
}

impl std::error::Error for Unpointed {}

impl From<Unsaid> for Unpointed {
  fn from(fault: Unsaid) -> Self {
    Unpointed::Unsaid(fault)
  }
}

impl From<console_screen::Undeclared> for Unpointed {
  fn from(fault: console_screen::Undeclared) -> Self {
    Unpointed::Undeclared(fault)
  }
}

impl From<console_onscreen::Error> for Unpointed {
  fn from(fault: console_onscreen::Error) -> Self {
    Unpointed::Onscreen(fault)
  }
}

impl From<pointer::Error> for Unpointed {
  fn from(fault: pointer::Error) -> Self {
    Self::Wayland(fault)
  }
}

fn run(asked: Request) -> Result<(), Unpointed> {
  let at = to_screen_point(asked.at, &asked.measured)?;

  let pointer = Pointer::new()?;
  let room = pointer.room;
  let Ok(standing) = on_the_screen(at, room);

  match standing {
    Where::OnTheScreen => {}
    Where::OffIt => {
      return Err(Unpointed::OffTheScreen(at, room));
    }
  }

  let Ok(from) = approach(at, room);

  pointer.to(from)?;
  #[cfg_attr(
    dylint_lib = "explicit021_no_sleeping",
    allow(
      explicit021_no_sleeping,
      reason = "a warp sends no motion: the pointer is put down beside the target and moved onto it a frame later, and the gap between the two is what makes it a movement the compositor reports"
    )
  )]
  std::thread::sleep(STEP);
  pointer.to(at)?;

  match asked.does {
    PointerAction::None => {}
    PointerAction::Click => pointer.click()?,
    PointerAction::Scroll(notches) => pointer.scroll(notches)?,
    PointerAction::Drag(through) => {
      let mut places = Vec::new();

      for place in through {
        let screen_point = to_screen_point(place, &asked.measured)?;

        places.push(screen_point);
      }

      pointer.drag(&places)?;
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit021_no_sleeping",
    allow(
      explicit021_no_sleeping,
      reason = "the virtual device is destroyed when this process ends, and an event written into a device that is gone before the compositor read it never happened"
    )
  )]
  std::thread::sleep(HELD);

  Ok(())
}

fn to_screen_point(at: Point<u32>, measured: &Measured) -> Result<Point<u32>, Unpointed> {
  match measured {
    Measured::FromTheScreen => Ok(at),
    Measured::FromTheCorner(namespace) => in_the_surface(at, namespace),
  }
}

fn in_the_surface(at: Point<u32>, namespace: &str) -> Result<Point<u32>, Unpointed> {
  let Ok(patience) = Schedule::of(APPEARING);
  let Ok(_up) = until_some(patience, || {
    Ok(match console_onscreen::screens() {
      Ok(screens) => {
        let Ok(drawn) = console_onscreen::layer_state(&screens, namespace);

        drawn
      }
      Err(_not_answering_yet) => None,
    })
  });

  let screens = console_onscreen::screens()?;
  let Ok(drawn) = console_onscreen::layer_state(&screens, namespace);

  let standing = drawn.ok_or_else(|| Unpointed::NotUp(namespace.to_string()))?;

  let Ok(wide) = fitted::<i64, u32>(standing.width);
  let Ok(tall) = fitted::<i64, u32>(standing.height);
  let Ok(inside) = on_the_screen(
    at,
    Size {
      width: wide,
      height: tall,
    },
  );

  match inside {
    Where::OnTheScreen => {}
    Where::OffIt => {
      return Err(Unpointed::PastTheSurface(
        at,
        Size {
          width: wide,
          height: tall,
        },
        namespace.to_string(),
      ));
    }
  }

  from_the_corner(
    at,
    Point {
      x: standing.x,
      y: standing.y,
    },
  )
  .map_err(Unpointed::Unsaid)
}

struct Pointer {
  device: pointer::Pointer,
  room: Size<u32>,
  since: Instant,
}

impl Pointer {
  fn new() -> Result<Pointer, Unpointed> {
    let device = pointer::Pointer::connect()?;
    let Ok(logical_size) = device.logical_size();
    let room = match logical_size {
      Some(room) => room,
      None => {
        let screen = console_screen::from_hyprland_config()?;
        let Ok(logical) = screen.logical();

        logical
      }
    };

    Ok(Pointer {
      device,
      room,
      since: Instant::now(),
    })
  }

  fn now(&self) -> Result<u32, Never> {
    fitted(self.since.elapsed().as_millis())
  }

  fn to(&self, at: Point<u32>) -> Result<(), Unpointed> {
    let Ok(now) = self.now();

    self.device.move_to(now, at, self.room)?;
    Ok(())
  }

  fn click(&self) -> Result<(), Unpointed> {
    self.button(Button::Pressed)?;

    #[cfg_attr(
      dylint_lib = "explicit021_no_sleeping",
      allow(
        explicit021_no_sleeping,
        reason = "a click is a press and a release with a person-shaped gap between them; released in the same frame it is a press nothing downstream counts"
      )
    )]
    std::thread::sleep(PRESSED);

    self.button(Button::Released)
  }

  fn button(&self, state: Button) -> Result<(), Unpointed> {
    let Ok(now) = self.now();

    self.device.button(now, state)?;
    Ok(())
  }

  fn drag(&self, through: &[Point<u32>]) -> Result<(), Unpointed> {
    self.button(Button::Pressed)?;

    for place in through {
      #[cfg_attr(
        dylint_lib = "explicit021_no_sleeping",
        allow(
          explicit021_no_sleeping,
          reason = "a line is drawn at a hand's pace: every place is a motion of its own, in a frame of its own, and motions written in one frame arrive as one jump"
        )
      )]
      std::thread::sleep(STEP);

      self.to(*place)?;
    }

    #[cfg_attr(
      dylint_lib = "explicit021_no_sleeping",
      allow(
        explicit021_no_sleeping,
        reason = "the last place is reached before the button lets go, and released in the same frame as the motion the lift would land before the place did"
      )
    )]
    std::thread::sleep(PRESSED);

    self.button(Button::Released)
  }

  fn scroll(&self, notches: i32) -> Result<(), Unpointed> {
    let Ok(now) = self.now();
    self.device.scroll(
      now,
      pointer::ScrollMotion {
        steps: notches,
        logical_pixels_per_step: LOGICAL_PIXELS_PER_NOTCH,
      },
    )?;
    Ok(())
  }
}
