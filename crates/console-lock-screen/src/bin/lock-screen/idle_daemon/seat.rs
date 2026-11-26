use std::os::fd::BorrowedFd;

use console_compositor::lock_notify::{hyprland_lock_notification_v1, hyprland_lock_notifier_v1};
use console_core_never::Never;
use console_lock_screen::idle::policy::{Request, Session, Timer};
use console_wayland::client::globals::GlobalListContents;
use console_wayland::client::protocol::{wl_registry, wl_seat};
use console_wayland::client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};
use console_wayland::protocols::ext::idle_notify::v1::client::{
  ext_idle_notification_v1, ext_idle_notifier_v1,
};
use console_wayland::{registry, waiting};

use super::Result;

pub struct Seat {
  connection: Connection,
  queue: EventQueue<Events>,
  events: Events,
  seat: wl_seat::WlSeat,
  manager: ext_idle_notifier_v1::ExtIdleNotifierV1,
  notifications: Vec<ext_idle_notification_v1::ExtIdleNotificationV1>,
}

impl Seat {
  pub fn connect() -> Result<Self> {
    let connection = console_wayland::connect()?;
    let (globals, mut queue) = registry::queue::<Events>(&connection)?;
    let handle = queue.handle();
    let seat = registry::seat(&globals, &handle, 1..=1)?;
    let manager = globals.bind(&handle, 1..=1, ())?;
    let lock_manager: hyprland_lock_notifier_v1::HyprlandLockNotifierV1 =
      globals.bind(&handle, 1..=1, ())?;
    let _lock_notification = lock_manager.get_lock_notification(&handle, ());
    lock_manager.destroy();
    let mut events = Events::default();
    let _initial = queue.roundtrip(&mut events)?;

    Ok(Self {
      connection,
      queue,
      events,
      seat,
      manager,
      notifications: Vec::new(),
    })
  }

  pub fn reset(&mut self, timers: &[(Timer, u32)]) -> Result<()> {
    let handle = self.queue.handle();

    for old in self.notifications.drain(..) {
      old.destroy();
    }

    self.events.generation = self.events.generation.wrapping_add(1);
    self
      .events
      .requests
      .retain(|request| matches!(request, Request::Session(_)));
    self.notifications.extend(timers.iter().map(|&(timer, ms)| {
      self.manager.get_idle_notification(
        ms,
        &self.seat,
        &handle,
        TimerData {
          timer,
          generation: self.events.generation,
        },
      )
    }));
    let _registered = waiting::flush(&self.connection)?;

    Ok(())
  }

  pub fn wait(&mut self, other: &[BorrowedFd<'_>]) -> Result<()> {
    let worked = waiting::wait(
      &self.connection,
      &mut self.queue,
      &mut self.events,
      other,
      None,
    );
    worked.map_err(std::io::Error::from)?;
    Ok(())
  }

  pub fn take(&mut self) -> core::result::Result<Vec<Request>, Never> {
    Ok(std::mem::take(&mut self.events.requests))
  }
}

struct TimerData {
  timer: Timer,
  generation: u64,
}

#[derive(Default)]
struct Events {
  generation: u64,
  requests: Vec<Request>,
}

impl Dispatch<ext_idle_notification_v1::ExtIdleNotificationV1, TimerData> for Events {
  fn event(
    state: &mut Self,
    _: &ext_idle_notification_v1::ExtIdleNotificationV1,
    event: ext_idle_notification_v1::Event,
    data: &TimerData,
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match (state.generation == data.generation, event.opcode()) {
      (true, ext_idle_notification_v1::EVT_IDLED_OPCODE) => {
        state.requests.push(Request::Idled(data.timer))
      }
      (true, ext_idle_notification_v1::EVT_RESUMED_OPCODE) => {
        state.requests.push(Request::Activity)
      }
      (false, _) | (true, _) => {}
    }
  }
}

impl Dispatch<hyprland_lock_notification_v1::HyprlandLockNotificationV1, ()> for Events {
  fn event(
    state: &mut Self,
    _: &hyprland_lock_notification_v1::HyprlandLockNotificationV1,
    event: hyprland_lock_notification_v1::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match event.opcode() {
      hyprland_lock_notification_v1::EVT_LOCKED_OPCODE => {
        state.requests.push(Request::Session(Session::Locked))
      }
      hyprland_lock_notification_v1::EVT_UNLOCKED_OPCODE => {
        state.requests.push(Request::Session(Session::Unlocked))
      }
      _unknown => {}
    }
  }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Events {
  fn event(
    _: &mut Self,
    _: &wl_registry::WlRegistry,
    _: wl_registry::Event,
    _: &GlobalListContents,
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
  }
}

delegate_noop!(Events: ignore wl_seat::WlSeat);
delegate_noop!(Events: ignore ext_idle_notifier_v1::ExtIdleNotifierV1);
delegate_noop!(Events: ignore hyprland_lock_notifier_v1::HyprlandLockNotifierV1);
