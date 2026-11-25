use std::io;
use std::os::fd::AsFd;
use std::os::unix::net::UnixDatagram;

use console_battery::power_source;
use console_concurrency::HostWorkers;
use console_core_iteration::{Step, iterate};
use console_core_logging::{Severity, Sink};
use console_core_never::Never;
use console_core_parallel_execution::{Worker, Workers};
use console_core_state_machine::{Machine, Transition};
use console_events::subscription::{self, Subscriptions};
use console_lock_screen::LockScreen;
use console_lock_screen::idle::policy::{
  Effect, Idle, Input, Request, Session, Sleep, State, Timer,
};
use console_lock_screen::idle::{self, Action, Profile};
use console_login_window::stored_pattern::{self, StoredPattern};
use console_program_contract::EventGroup;

mod actions;
mod bus;
mod seat;

use actions::{Actions, SleepDelay};
use bus::{Login, ScreenSaver};
use seat::Seat;

type Failure = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Failure>;

static JOURNAL: Sink = Sink {
  minimum: Severity::Info,
  write: |record| eprintln!("{}", record.message),
};

pub fn run() -> Result<()> {
  console_core_logging::install(&JOURNAL)
    .map_err(|_installed| io::Error::other("a log sink is already installed"))?;

  let mut seat = Seat::connect()?;
  let (login, logind_idle) = Login::connect()?;
  let screen_saver = ScreenSaver::connect()?;
  let power = PowerWatch::start()?;
  let Ok(source) = power_source();
  let Ok(config) = Configuration::read();
  seat.reset(&config.timers)?;
  let Ok(initial) = Idle::initial_transition(
    &Input {
      power: Profile::from(source),
      lock_before_sleep: config.suspend_lock,
      logind_idle,
    },
    None,
  );
  let actions = Actions::start()?;
  let mut driver = Driver {
    seat,
    login,
    screen_saver,
    delay: None,
    actions,
    config,
  };
  let done = driver.run(initial, &power);
  let Ok(()) = driver.actions.close();

  done
}

struct Configuration {
  timers: Vec<(Timer, u32)>,
  suspend_lock: LockScreen,
}

impl Configuration {
  fn read() -> core::result::Result<Self, Never> {
    let mut timers = Vec::new();

    for action in Action::ALL {
      let Ok(delay) = action.delay();
      let Ok(millis) = delay.milliseconds();
      let timer = match action {
        Action::Dim => Timer::Dim,
        Action::Blank => Timer::Blank,
        Action::Suspend => Timer::Suspend,
      };
      timers.extend(millis.map(|ms| (timer, ms)));
    }

    let Ok(available) = lock_available();
    let suspend_lock = match available {
      LockScreen::On => {
        for profile in [Profile::General, Profile::OnBattery, Profile::PluggedIn] {
          let Ok(delay) = idle::delay(profile);
          let Ok(millis) = delay.milliseconds();
          timers.extend(millis.map(|ms| (Timer::Lock(profile), ms)));
        }

        let Ok(choice) = idle::on_suspend();
        choice
      }
      LockScreen::Off => LockScreen::Off,
    };

    Ok(Self {
      timers,
      suspend_lock,
    })
  }
}

fn lock_available() -> core::result::Result<LockScreen, Never> {
  let Ok(enabled) = console_lock_screen::current();
  let Ok(home) = console_core_places::home();

  Ok(match (enabled, home) {
    (LockScreen::On, Some(directory)) => match stored_pattern::stored(&directory) {
      Ok(StoredPattern::Hash(_)) => LockScreen::On,
      Ok(StoredPattern::Absent) => LockScreen::Off,
      Err(fault) => {
        console_core_logging::warn!("Reading the login pattern for automatic locking: {fault}");
        LockScreen::On
      }
    },
    (LockScreen::Off, _) | (LockScreen::On, None) => LockScreen::Off,
  })
}

struct Driver {
  seat: Seat,
  login: Login,
  screen_saver: Option<ScreenSaver>,
  actions: Actions,
  delay: Option<SleepDelay>,
  config: Configuration,
}

impl Driver {
  fn run(&mut self, initial: Transition<State, Effect>, power: &PowerWatch) -> Result<()> {
    let initialized = self.perform(initial)?;
    let state = self.seat_events(initialized)?;
    let Ok(session) = state.session();

    match session {
      Session::Locked => {}
      Session::Unlocked => self.login.report(session)?,
    }

    console_readiness::ready()?;
    let finished = iterate((self, state), |(driver, current)| {
      Ok(match driver.next(current, power) {
        Ok(next) => Step::Again((driver, next)),
        Err(fault) => Step::Halt(Err(fault)),
      })
    })?;

    finished
  }

  fn next(&mut self, state: State, power: &PowerWatch) -> Result<State> {
    let mut descriptors = vec![self.login.incoming.as_fd(), power.reader.as_fd()];
    descriptors.extend(self.screen_saver.as_ref().map(|bus| bus.incoming.as_fd()));
    self.seat.wait(&descriptors)?;
    let lock_or_sleep = self.login.next()?;
    let after_login = self.observed(state, lock_or_sleep)?;
    let Ok(session) = after_login.session();
    let inhibition = self
      .screen_saver
      .as_mut()
      .map(|bus| bus.next(session))
      .transpose()?;
    let after_inhibitors = self.observed(after_login, inhibition.flatten())?;
    let power_change = power.next()?;
    let after_power = self.observed(after_inhibitors, power_change)?;

    self.seat_events(after_power)
  }

  fn seat_events(&mut self, mut state: State) -> Result<State> {
    let Ok(requests) = self.seat.take();

    for request in requests {
      let next = self.observed(state, Some(request))?;
      state = next;
    }

    Ok(state)
  }

  fn observed(&mut self, state: State, request: Option<Request>) -> Result<State> {
    match request {
      Some(event) => {
        let Ok(next) = Idle::transition(state, event);
        self.perform(next)
      }
      None => Ok(state),
    }
  }

  fn perform(&mut self, turn: Transition<State, Effect>) -> Result<State> {
    for effect in turn.effects {
      match effect {
        Effect::Display(next) => self.actions.display(next)?,
        Effect::StartLock => self.actions.lock()?,
        Effect::Suspend => self.actions.suspend()?,
        Effect::HoldSleepDelay => match self.delay.as_ref() {
          Some(_held) => {}
          None => match SleepDelay::hold() {
            Ok(delay) => self.delay = Some(delay),
            Err(fault) => console_core_logging::warn!(
              "Could not delay suspend until the session locks: {fault}"
            ),
          },
        },
        Effect::ReleaseSleepDelay => self.delay = None,
        Effect::ResetTimers => self.seat.reset(&self.config.timers)?,
        Effect::ReportSession(session) => {
          match session {
            Session::Locked => console_core_logging::info!("Session locked"),
            Session::Unlocked => console_core_logging::info!("Session unlocked"),
          }

          self.login.report(session)?;
          let _sent = self
            .screen_saver
            .as_ref()
            .map(|bus| bus.report(session))
            .transpose()?;
        }
        Effect::ReportSleep(Sleep::Preparing | Sleep::AwaitingLock) => {
          console_core_logging::info!("System preparing to sleep");
        }
        Effect::ReportSleep(Sleep::Awake) => {
          console_core_logging::info!("System resumed from sleep");
        }
      }
    }

    Ok(turn.state)
  }
}

struct PowerWatch {
  reader: UnixDatagram,
  worker: Option<(Subscriptions, Worker)>,
}

impl PowerWatch {
  #[cfg_attr(
    dylint_lib = "explicit055_no_unbounded_loop",
    expect(
      explicit055_no_unbounded_loop,
      reason = "dropping the subscriptions owner closes the battery receiver and ends this worker"
    )
  )]
  pub fn start() -> io::Result<Self> {
    let (reader, writer) = UnixDatagram::pair()?;
    reader.set_nonblocking(true)?;
    writer.set_nonblocking(true)?;
    let Ok(subscription) = subscription::connect(&[EventGroup::Battery]);
    let Ok((subscriptions, events)) = subscription.split();
    let worker = HostWorkers.start("idle-power".to_owned(), move || {
      for _event in events {
        match writer.send(b"p") {
          Ok(_written) => {}
          Err(fault) => match (fault.kind() == io::ErrorKind::WouldBlock).then_some(()) {
            Some(()) => continue,
            None => {
              console_core_logging::warn!("Could not forward a power-source change: {fault}");
              break;
            }
          },
        }
      }
    })?;

    Ok(Self {
      reader,
      worker: Some((subscriptions, worker)),
    })
  }

  pub fn next(&self) -> io::Result<Option<Request>> {
    match self.reader.recv(&mut [0]) {
      Ok(_size) => {
        let Ok(source) = power_source();
        Ok(Some(Request::Power(Profile::from(source))))
      }
      Err(fault) => match (fault.kind() == io::ErrorKind::WouldBlock).then_some(()) {
        Some(()) => Ok(None),
        None => Err(fault),
      },
    }
  }
}

impl Drop for PowerWatch {
  fn drop(&mut self) {
    match self.worker.take() {
      Some((subscriptions, worker)) => {
        drop(subscriptions);

        match worker.join() {
          Ok(()) => {}
          Err(_panicked) => {
            console_core_logging::warn!("The idle power watcher stopped unexpectedly")
          }
        }
      }
      None => {}
    }
  }
}
