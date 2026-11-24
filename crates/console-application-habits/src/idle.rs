//! Seat idle notifications avoid reading individual input events.

use std::io;
use std::net::Shutdown;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;

use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_wayland::client::globals::GlobalListContents;
use console_wayland::client::protocol::{wl_registry, wl_seat};
use console_wayland::client::{Dispatch, QueueHandle, delegate_noop};
use console_wayland::protocols::ext::idle_notify::v1::client::{
  ext_idle_notification_v1 as notification, ext_idle_notifier_v1 as notifier,
};
use console_wayland::waiting;
use rustix::event::{PollFd, PollFlags, Timespec, poll};

use crate::Result;

const IDLE_MILLISECONDS: u32 = 300_000;
const RETRY: Timespec = Timespec {
  tv_sec: 30,
  tv_nsec: 0,
};

#[derive(Clone, Copy)]
pub enum SeatState {
  Unavailable,
  Active,
  Idle,
}

impl SeatState {
  #[must_use]
  fn code(self) -> core::result::Result<u8, Never> {
    Ok(match self {
      Self::Unavailable => 0,
      Self::Active => 1,
      Self::Idle => 2,
    })
  }
}

pub struct Idle<'a> {
  state: &'a AtomicU8,
  stop: UnixStream,
}

pub fn with_monitor<T>(record: impl FnOnce(&Idle<'_>) -> Result<T>) -> Result<T> {
  let Ok(initial) = SeatState::Unavailable.code();
  let state = AtomicU8::new(initial);

  thread::scope(|scope| {
    let (stop, control) = UnixStream::pair()?;
    let shared = &state;
    let _worker = thread::Builder::new()
      .name(String::from("habits-idle"))
      .spawn_scoped(scope, move || {
        let ended = std::panic::catch_unwind(|| {
          let Ok(()) = Watcher::run(shared, &control);
        });

        match ended {
          Ok(()) => {}
          Err(_panic) => {
            shared.store(initial, Ordering::SeqCst);
            console_core_logging::warn!("Habits idle monitor stopped unexpectedly");
          }
        }
      })?;
    let idle = Idle {
      state: shared,
      stop,
    };
    record(&idle)
  })
}

impl Idle<'_> {
  #[must_use]
  pub fn status(&self) -> core::result::Result<SeatState, Never> {
    Ok(match self.state.load(Ordering::SeqCst) {
      1 => SeatState::Active,
      2 => SeatState::Idle,
      _unavailable => SeatState::Unavailable,
    })
  }
}

impl Drop for Idle<'_> {
  fn drop(&mut self) {
    match self.stop.shutdown(Shutdown::Both) {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::warn!("Closing the Wayland idle monitor control socket: {fault}");
      }
    }
  }
}

enum Control {
  Running,
  Stopped,
}

fn control_status(control: &UnixStream, delay: &Timespec) -> io::Result<Control> {
  let mut stopping = [PollFd::new(control, PollFlags::IN)];

  match poll(&mut stopping, Some(delay)) {
    Ok(0) | Err(rustix::io::Errno::INTR) => Ok(Control::Running),
    Ok(_ready) => Ok(Control::Stopped),
    Err(fault) => Err(io::Error::from(fault)),
  }
}

struct Watcher(SeatState);

impl Watcher {
  fn publish(&self, shared: &AtomicU8) -> core::result::Result<(), Never> {
    let Ok(code) = self.0.code();
    shared.store(code, Ordering::SeqCst);
    Ok(())
  }

  fn run(shared: &AtomicU8, control: &UnixStream) -> core::result::Result<(), Never> {
    let ended = iterate((), |()| {
      match Self::listen(shared, control) {
        Ok(()) => return Ok(Step::Halt(())),
        Err(fault) => {
          let Ok(()) = Self(SeatState::Unavailable).publish(shared);
          console_core_logging::warn!("Wayland idle monitor: {fault}");
        }
      }

      Ok(match control_status(control, &RETRY) {
        Ok(Control::Running) => Step::Again(()),
        Ok(Control::Stopped) => Step::Halt(()),
        Err(fault) => {
          console_core_logging::warn!("Waiting to reconnect the Wayland idle monitor: {fault}");
          Step::Halt(())
        }
      })
    });

    match ended {
      Ok(()) => {}
      Err(fault) => {
        console_core_logging::warn!("The Wayland idle monitor lost its event source: {fault}");
      }
    }

    Ok(())
  }

  fn listen(shared: &AtomicU8, control: &UnixStream) -> Result<()> {
    let connection = console_wayland::connect()?;
    let (globals, mut queue) = console_wayland::registry::queue(&connection)?;
    let handle = queue.handle();
    let seat = console_wayland::registry::seat(&globals, &handle, 1..=1)?;
    let manager: notifier::ExtIdleNotifierV1 = globals.bind(&handle, 1..=1, ())?;
    let _subscription = manager.get_idle_notification(IDLE_MILLISECONDS, &seat, &handle, ());
    let mut watch = Self(SeatState::Active);
    let Ok(()) = watch.publish(shared);
    let wake = [control.as_fd()];

    let waited = iterate((&mut watch, &mut queue), |(watcher, events)| {
      match control_status(control, &Timespec::default()) {
        Ok(Control::Running) => {}
        Ok(Control::Stopped) => return Ok(Step::Halt(Ok(()))),
        Err(fault) => return Ok(Step::Halt(Err(fault))),
      }

      Ok(
        match waiting::wait(&connection, events, watcher, &wake, None) {
          Ok(()) => {
            let Ok(()) = watcher.publish(shared);
            Step::Again((watcher, events))
          }
          Err(fault) => Step::Halt(Err(io::Error::from(fault))),
        },
      )
    })?;
    waited.map_err(Box::from)
  }
}

impl Dispatch<notification::ExtIdleNotificationV1, ()> for Watcher {
  fn event(
    state: &mut Self,
    _: &notification::ExtIdleNotificationV1,
    event: notification::Event,
    _: &(),
    _: &console_wayland::client::Connection,
    _: &QueueHandle<Self>,
  ) {
    let seat = match event.opcode() {
      notification::EVT_IDLED_OPCODE => SeatState::Idle,
      notification::EVT_RESUMED_OPCODE => SeatState::Active,
      _unknown_opcode => return,
    };
    state.0 = seat;
  }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Watcher {
  fn event(
    _: &mut Self,
    _: &wl_registry::WlRegistry,
    _: wl_registry::Event,
    _: &GlobalListContents,
    _: &console_wayland::client::Connection,
    _: &QueueHandle<Self>,
  ) {
  }
}

delegate_noop!(Watcher: ignore wl_seat::WlSeat);
delegate_noop!(Watcher: ignore notifier::ExtIdleNotifierV1);
