//! Apply a complete color-transform batch after a clock or output change.
//!
//! A commit resets omitted outputs to identity, so hotplug also resends the
//! current transform to the outputs that were already present.

use std::collections::{BTreeMap, btree_map::Entry};
use std::io;
use std::os::fd::AsFd;

use console_compositor::color_transform::hyprland_ctm_control_manager_v1::{
  self as ctm, HyprlandCtmControlManagerV1,
};
use console_core_color::temperature::white_balance;
use console_core_iteration::{Step as Turn, iterate};
use console_core_never::Never;
use console_core_number_conversion::Float;
use console_location::sun::UnixSeconds;
use console_location::timezone;
use console_night_light::{Step, Temperature, curve, schedule};
use console_wayland::client::globals::{Global, GlobalListContents};
use console_wayland::client::protocol::{wl_output, wl_registry};
use console_wayland::client::{
  Connection, Dispatch, EventQueue, Proxy, QueueHandle, delegate_noop,
};
use console_wayland::{registry, waiting};

use super::clock::Clock;

pub enum Look {
  Scheduled,
  Fixed(Temperature),
}

pub struct GammaPercent(pub u32);

#[derive(Default)]
struct Outputs {
  active: BTreeMap<u32, wl_output::WlOutput>,
  control: Control,
}

#[derive(Default)]
enum Control {
  Blocked,
  #[default]
  NeedsCommit,
  Current,
}

impl Outputs {
  fn changed(&mut self) -> Result<(), Never> {
    self.control = match self.control {
      Control::Blocked => Control::Blocked,
      Control::NeedsCommit | Control::Current => Control::NeedsCommit,
    };

    Ok(())
  }

  fn add(
    &mut self,
    registry: &wl_registry::WlRegistry,
    hand: &QueueHandle<Self>,
    global: &Global,
  ) -> Result<(), Never> {
    match (global.interface == wl_output::WlOutput::interface().name).then_some(()) {
      Some(()) => match self.active.entry(global.name) {
        Entry::Vacant(slot) => {
          let output = registry.bind(global.name, global.version.min(3), hand, ());
          let _bound = slot.insert(output);
          let Ok(()) = self.changed();
        }
        Entry::Occupied(_) => {}
      },
      None => {}
    }

    Ok(())
  }

  fn apply(
    &mut self,
    manager: &HyprlandCtmControlManagerV1,
    matrix: [f64; 3],
  ) -> Result<(), Never> {
    let [red, green, blue] = matrix;

    for output in self.active.values() {
      manager.set_ctm_for_output(output, red, 0.0, 0.0, 0.0, green, 0.0, 0.0, 0.0, blue);
    }

    manager.commit();
    self.control = Control::Current;

    Ok(())
  }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Outputs {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    expect(
      explicit016_no_wildcard_arm,
      reason = "Wayland registry events are non-exhaustive and other events do not alter the output list"
    )
  )]
  fn event(
    state: &mut Self,
    registry: &wl_registry::WlRegistry,
    event: wl_registry::Event,
    _globals: &GlobalListContents,
    _connection: &Connection,
    hand: &QueueHandle<Self>,
  ) {
    match event {
      wl_registry::Event::Global {
        name,
        interface,
        version,
      } => {
        let Ok(()) = state.add(
          registry,
          hand,
          &Global {
            name,
            interface,
            version,
          },
        );
      }
      wl_registry::Event::GlobalRemove { name } => match state.active.remove(&name) {
        Some(output) => {
          let Ok(()) = state.changed();

          match (output.version() >= 3).then_some(()) {
            Some(()) => output.release(),
            None => {}
          }
        }
        None => {}
      },
      _ => {}
    }
  }
}

impl Dispatch<HyprlandCtmControlManagerV1, ()> for Outputs {
  fn event(
    state: &mut Self,
    _manager: &HyprlandCtmControlManagerV1,
    event: ctm::Event,
    _data: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match matches!(event, ctm::Event::Blocked).then_some(()) {
      Some(()) => state.control = Control::Blocked,
      None => {}
    }
  }
}

delegate_noop!(Outputs: ignore wl_output::WlOutput);

fn matrix(temperature: Temperature, gamma: &GammaPercent) -> Result<[f64; 3], Never> {
  let gains = match temperature {
    Temperature::Neutral => [1.0; 3],
    Temperature::Kelvin(kelvin) => {
      let Ok(white) = white_balance(kelvin);

      white
    }
  };

  let factor = f64::from(gamma.0) / 100.0;

  Ok(gains.map(|gain| gain * factor))
}

fn scheduled(fallback: &[Step], unix_seconds: i64) -> Result<Temperature, Never> {
  let Ok(location) = timezone::known_location();

  match location {
    Some(place) => {
      let Ok(seconds) = unix_seconds.float();

      schedule::at_location(&place, UnixSeconds(seconds))
    }
    None => schedule::at_utc(fallback, unix_seconds),
  }
}

pub fn run(look: Look, gamma: GammaPercent) -> Result<(), Box<dyn std::error::Error>> {
  let connection = console_wayland::connect()?;
  let (globals, mut queue) = registry::queue::<Outputs>(&connection)?;
  let hand = queue.handle();
  let manager = globals
    .bind::<HyprlandCtmControlManagerV1, _, _>(&hand, 1..=2, ())
    .map_err(|fault| {
      io::Error::other(format!(
        "Hyprland color transforms are unavailable: {fault}"
      ))
    })?;
  let mut outputs = Outputs::default();
  let Ok(fallback) = curve();
  let (temperature, clock) = match look {
    Look::Fixed(temperature) => (temperature, None),
    Look::Scheduled => {
      let clock = Clock::new()?;
      let now = clock.arm()?;
      let Ok(temperature) = scheduled(&fallback, now);

      (temperature, Some(clock))
    }
  };

  for global in globals.contents().clone_list() {
    let Ok(()) = outputs.add(globals.registry(), &hand, &global);
  }

  let _events = queue.roundtrip(&mut outputs)?;

  let mut display = Display {
    connection,
    queue,
    outputs,
    manager,
    fallback,
    temperature,
    gamma,
    clock,
  };
  display.refresh()?;
  let _applied = display.queue.roundtrip(&mut display.outputs)?;
  console_readiness::ready()?;
  let finished: io::Result<()> = iterate(display, |mut current| {
    Ok(match current.next() {
      Ok(()) => Turn::Again(current),
      Err(fault) => Turn::Halt(Err(fault)),
    })
  })?;
  finished?;

  Ok(())
}

struct Display {
  connection: Connection,
  queue: EventQueue<Outputs>,
  outputs: Outputs,
  manager: HyprlandCtmControlManagerV1,
  fallback: Vec<Step>,
  temperature: Temperature,
  gamma: GammaPercent,
  clock: Option<Clock>,
}

impl Display {
  fn refresh(&mut self) -> io::Result<()> {
    let tick = self.clock.as_ref().map(Clock::tick).transpose()?;

    match tick.flatten() {
      Some(now) => {
        let Ok(next) = scheduled(&self.fallback, now);

        match (self.temperature != next).then_some(()) {
          Some(()) => {
            self.temperature = next;
            let Ok(()) = self.outputs.changed();
          }
          None => {}
        }
      }
      None => {}
    }

    match self.outputs.control {
      Control::NeedsCommit => {
        let Ok(gains) = matrix(self.temperature, &self.gamma);
        let Ok(()) = self.outputs.apply(&self.manager, gains);
      }
      Control::Current => {}
      Control::Blocked => {
        return Err(io::Error::other(
          "another client controls Hyprland's color transforms",
        ));
      }
    }

    Ok(())
  }

  fn next(&mut self) -> io::Result<()> {
    self.refresh()?;
    let timer = self.clock.as_ref().map(AsFd::as_fd);

    waiting::wait(
      &self.connection,
      &mut self.queue,
      &mut self.outputs,
      timer.as_slice(),
      None,
    )
    .map_err(io::Error::from)
  }
}
