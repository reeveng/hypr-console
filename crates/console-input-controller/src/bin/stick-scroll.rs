//! The desktop half of the controller: scrolling, and the buttons that ask the
//! compositor for something.  Everything that decides anything is in
//! `console_input_controller`, where it can be asked the same question twice.
//! What is here is a machine's real devices, offered to that as somewhere the
//! devices are plugged in.  It is also where the daemon's own share of a wait
//! is written down. Every other stopwatch on this device starts when a program
//! is exec'd, so the stretch between the pad saying something and this deciding
//! to act on it was the one nobody could see -- and it is the stretch that says
//! whether a slow opening is the daemon or the toolkit.  A turn happens twenty
//! times a second and almost all of them decide nothing, so almost all of them
//! write nothing. A turn that starts a program always writes, because that is a
//! press somebody is waiting on the far end of. A turn that only scrolled or
//! told the home screen something writes only if it took longer than a frame,
//! which is the only version of it worth reading.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use evdev::uinput::VirtualDevice;
use evdev::{
    AbsoluteAxisCode, AttributeSet, Device, InputEvent, KeyCode, RelativeAxisCode,
};
use console_program_lifetime::{LetGo, Still, let_go};
use console_cpu_boost::Hurrying;
use console_input_controller::clock::since_boot;
use console_input_controller::doing::Doing;
use console_input_controller::finding::{Says, says};
use console_input_controller::means::{self, Table};
use console_input_gamepad::jobs::Rebound;
use console_core_never::Never;
use console_input_controller::mode::{Awake, Mode};
use console_input_controller::reading::{From, Ranges};
use console_input_controller::turning::{Gone, Plugged, READ, Took, Turning};

fn main() -> std::process::ExitCode {
    let mut out = match published() {
        Ok(out) => out,
        Err(fault) => {
            eprintln!("stick-scroll: {fault}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let mut machine = Machine::default();
    let Ok(told) = told();
    let Ok(mut turning) = Turning::pointed_at(told);

    let home = match std::env::var("HOME") {
        Ok(home) => home,
        Err(std::env::VarError::NotPresent) => String::new(),
        Err(fault) => {
            eprintln!("stick-scroll: HOME: {fault}; running on the table this was built with");

            String::new()
        }
    };

    let Ok(mut bound) = Bound::of(&home);

    match bound.look(&mut turning) {
        Ok(()) => {},
        Err(fault) => eprintln!("stick-scroll: {fault}"),
    }

    let mut holding: BTreeMap<From, String> = BTreeMap::new();
    let mut running: Vec<LetGo> = Vec::new();

    let Ok(changed) = watching();

    match look(&mut turning) {
        Ok(_) => {}
        Err(fault) => eprintln!("stick-scroll: {fault}"),
    }

    let mut hurrying = Hurrying::default();
    let mut said_the_watcher_went = false;

    let Ok(mut was_awake) = Awake::asked();

    loop {
        let word = match changed.try_recv() {
            Ok(()) => Word::Came,
            Err(TryRecvError::Empty) => Word::Nothing,
            Err(TryRecvError::Disconnected) => Word::Gone,
        };

        match word == Word::Gone && !said_the_watcher_went {
            true => {
                eprintln!("stick-scroll: the watcher on the compositor has ended; what is in front of you will not be asked again");
                said_the_watcher_went = true;
            }
            false => {},
        }

        let Ok(awake) = Awake::asked();

        let woke = awake != was_awake;
        was_awake = awake;

        match word == Word::Came || woke {
            true => {
                while let Ok(()) = changed.try_recv() {}

                match look(&mut turning) {
                    Ok(letting_go) => {
                        for what in &letting_go {
                            let Ok(started) = done(what, &mut out);

                            running.extend(started);
                        }
                    }
                    Err(fault) => eprintln!("stick-scroll: {fault}"),
                }
            }
            false => {},
        }

        match bound.look(&mut turning) {
            Ok(()) => {},
            Err(fault) => eprintln!("stick-scroll: {fault}"),
        }

        let Ok(mut waiting) = console_response_times::Waiting::here("controller", "press");

        let Ok(now) = since_boot();
        let Ok(decided) = turning.turn(&mut machine, now);
        let mut what_for = Decided::Nothing;

        let Ok(()) = waiting.mark("deciding");

        for what in decided {
            match &what {
                Doing::Run(argv) => {
                    let Ok(()) = hurrying.asked(Instant::now());

                    match (what_for, argv.split_first()) {
                        (Decided::Nothing | Decided::Something, Some((program, _))) => {
                            let Ok(()) = waiting.named("starting", program);
                        }
                        (Decided::ToStart, _) | (_, None) => {},
                    }

                    what_for = Decided::ToStart;
                }
                Doing::Frame(_) | Doing::Tell(_) => {
                    what_for = match what_for {
                        Decided::Nothing => Decided::Something,
                        Decided::Something | Decided::ToStart => what_for,
                    }
                }
            }

            let Ok(started) = done(&what, &mut out);

            running.extend(started);
        }

        let Ok(()) = waiting.mark("doing");

        match what_for {
            Decided::ToStart => {
                let Ok(()) = waiting.done();
            }
            Decided::Something => {
                let Ok(()) = waiting.done_if_felt();
            }
            Decided::Nothing => {},
        }

        let Ok(()) = hurrying.settle(Instant::now());
        let Ok(still) = reaped(running);

        running = still;
        let Ok(()) = say_what_changed(&mut holding, &turning);
        let Ok(poll) = turning.poll();

        #[cfg_attr(
            dylint_lib = "explicit021_no_sleeping",
            allow(
                explicit021_no_sleeping,
                reason = "a stick is a position rather than an event: what it is doing now is only knowable by looking, and how often to look is what `turning.poll()` decides"
            )
        )]
        std::thread::sleep(Duration::from_secs_f64(poll));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decided {
    Nothing,
    Something,
    ToStart,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Word {
    Came,
    Nothing,
    Gone,
}

struct Bound {
    at: PathBuf,
    written: Option<std::time::SystemTime>,
    read: bool,
}

impl Bound {
    fn of(home: &str) -> Result<Self, Never> {
        let Ok(at) = console_input_gamepad::jobs::path_in(home);

        Ok(Self { at, written: None, read: false })
    }

    fn look(&mut self, turning: &mut Turning) -> Result<(), String> {
        let at = self.at.clone();

        let written = match std::fs::metadata(&at).and_then(|held| held.modified()) {
            Ok(when) => Some(when),
            Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => None,
            Err(fault) => {
                self.written = None;
                self.read = true;

                return Err(format!("{}: asking when it was last written: {fault}", at.display()));
            }
        };

        match self.read && written == self.written {
            true => return Ok(()),
            false => {},
        }

        self.written = written;
        self.read = true;

        let said = match std::fs::read_to_string(&at) {
            Ok(said) => said,
            Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(fault) => return Err(format!("{}: reading it: {fault}", at.display())),
        };

        match console_input_gamepad::jobs::Jobs::read(&said) {
            Ok(jobs) => {
                let Ok(moved) = jobs.moved();

                match moved {
                    Rebound::Something => {
                        eprintln!(
                            "stick-scroll: {} moves {} of them",
                            at.display(),
                            jobs.moved.len()
                        );
                    }
                    Rebound::Nothing => {},
                }

                let Ok(table) = Table::of(&jobs);
                let Ok(()) = turning.bound_by(table);
            }
            Err(fault) => eprintln!("stick-scroll: {}: {fault}", at.display()),
        }

        Ok(())
    }
}

fn watching() -> Result<Receiver<()>, Never> {
    let (say, heard) = channel();
    let Ok(()) = console_events::again::layers(say);

    Ok(heard)
}

fn look(turning: &mut Turning) -> Result<Vec<Doing>, String> {
    let screens = console_onscreen::screens()?;
    let Ok(awake) = Awake::asked();

    let Ok(mode) = Mode::seen(&screens, awake);
    let Ok(now_in) = turning.held.now_in(mode);

    Ok(now_in)
}

#[derive(Default)]
struct Machine {
    open: BTreeMap<String, Device>,
}

impl Plugged for Machine {
    fn every(&self) -> Vec<Says> {
        evdev::enumerate()
            .map(|(path, device)| {
                let Ok(says) = says(&path.display().to_string(), &device);

                says
            })
            .collect()
    }

    fn open(&mut self, path: &str) -> Took {
        match self.open.contains_key(path) {
            true => return Took::Held,
            false => {},
        }

        let opened = Device::open(path).and_then(|device| {
            device.set_nonblocking(true)?;
            Ok(device)
        });

        match opened {
            Ok(device) => {
                self.open.insert(path.to_string(), device);
                Took::Held
            }
            Err(fault) => {
                eprintln!("stick-scroll: {path}: {fault}");
                Took::Refused
            }
        }
    }

    fn ranges(&self, path: &str) -> Ranges {
        let Some(device) = self.open.get(path) else { return Ranges::default() };

        let mut told: BTreeMap<u16, (i32, i32)> = BTreeMap::new();

        match device.get_absinfo() {
            Ok(states) => {
                for (axis, info) in states {
                    told.insert(axis.0, (info.minimum(), info.maximum()));
                }
            }
            Err(_the_device_went_away) => {},
        }

        let stick = [AbsoluteAxisCode::ABS_RX, AbsoluteAxisCode::ABS_RY]
            .iter()
            .filter_map(|axis| told.get(&axis.0))
            .map(|(low, high)| low.abs().max(high.abs()))
            .max()
            .filter(|span| *span > 0)
            .unwrap_or(Ranges::default().stick);
        let trigger =
            told.get(&AbsoluteAxisCode::ABS_Z.0).copied().unwrap_or(Ranges::default().trigger);
        Ranges { stick, trigger }
    }

    fn drain(&mut self, path: &str) -> Result<Vec<InputEvent>, Gone> {
        let Some(device) = self.open.get_mut(path) else { return Err(Gone) };

        let arrived = match device.fetch_events() {
            Ok(arrived) => Ok(arrived.collect()),
            Err(fault) if fault.kind() == std::io::ErrorKind::WouldBlock => Ok(Vec::new()),
            Err(_) => Err(Gone),
        };

        match arrived.is_err() {
            true => {
                self.open.remove(path);
            }
            false => {
                {};
            }
        }

        arrived
    }
}

fn told() -> Result<BTreeMap<From, String>, Never> {
    let named = |which| match which {
        From::Pad => "CONSOLE_PAD",
        From::Keys => "CONSOLE_KEYS",
        From::Touch => "CONSOLE_TOUCHPAD",
    };
    let mut out = BTreeMap::new();

    for which in READ {
        let name = named(which);

        let path = match std::env::var(name) {
            Ok(path) => path,
            Err(std::env::VarError::NotPresent) => continue,
            Err(fault) => {
                eprintln!("stick-scroll: {name}: {fault}; finding that device instead");
                continue;
            }
        };

        match path.is_empty() {
            true => {},
            false => {
                out.insert(which, path);
            }
        }
    }

    Ok(out)
}

fn say_what_changed(
    holding: &mut BTreeMap<From, String>,
    turning: &Turning,
) -> Result<(), Never> {
    let Ok(now) = turning.holding();
    let name = |which| match which {
        From::Pad => "pad",
        From::Keys => "keyboard",
        From::Touch => "touchpad",
    };

    for which in READ {
        match (holding.get(&which), now.get(&which)) {
            (None, Some(path)) => eprintln!("stick-scroll: reading the {} at {path}", name(which)),
            (Some(_), None) => eprintln!("stick-scroll: the {} has gone", name(which)),
            _ => (),
        }
    }

    *holding = now.clone();

    Ok(())
}

fn published() -> Result<VirtualDevice, String> {
    let mut keys = AttributeSet::<KeyCode>::new();

    let Ok(sends) = means::sends();

    for key in sends {
        keys.insert(key);
    }

    let mut axes = AttributeSet::<RelativeAxisCode>::new();

    for axis in [
        RelativeAxisCode::REL_HWHEEL,
        RelativeAxisCode::REL_WHEEL,
        RelativeAxisCode::REL_X,
        RelativeAxisCode::REL_Y,
    ] {
        axes.insert(axis);
    }

    let opened = VirtualDevice::builder()
        .map_err(|fault| format!("no way in to /dev/uinput: {fault}"))?;
    let keyed = opened
        .name("stick-scroll")
        .with_keys(&keys)
        .map_err(|fault| format!("the button: {fault}"))?;
    let wheeled = keyed
        .with_relative_axes(&axes)
        .map_err(|fault| format!("the wheel: {fault}"))?;

    wheeled.build().map_err(|fault| format!("the device would not build: {fault}"))
}

fn done(what: &Doing, out: &mut VirtualDevice) -> Result<Option<LetGo>, Never> {
    match what {
        Doing::Frame(frame) => {
            let events: Vec<InputEvent> = frame
                .iter()
                .map(|written| InputEvent::new(written.kind.0, written.code, written.value))
                .collect();

            match out.emit(&events) {
                Ok(()) => {},
                Err(fault) => eprintln!("stick-scroll: nothing came out: {fault}"),
            }

            Ok(None)
        }
        Doing::Run(argv) => {
            eprintln!("stick-scroll: {}", argv.join(" "));

            run(argv)
        }
        Doing::Tell(said) => {
            match console_onscreen::telling(*said) {
                Ok(()) => {},
                Err(fault) => eprintln!("stick-scroll: the home screen was not told: {fault}"),
            }

            Ok(None)
        }
    }
}

fn reaped(running: Vec<LetGo>) -> Result<Vec<LetGo>, Never> {
    Ok(running
        .into_iter()
        .filter_map(|mut child| {
            let Ok(still) = child.still();

            match still {
                Still::Running => Some(child),
                Still::Ended => None,
            }
        })
        .collect())
}

fn run(argv: &[String]) -> Result<Option<LetGo>, Never> {
    let Some((program, rest)) = argv.split_first() else {
        return Ok(None);
    };

    let mut starting = Command::new(program);
    starting.args(rest).stdout(Stdio::null()).stderr(Stdio::inherit());
    let Ok(()) = console_response_times::pressed(&mut starting, "pad");

    Ok(match let_go(&mut starting) {
        Ok(child) => Some(child),
        Err(fault) => {
            eprintln!("stick-scroll: cannot run {program}: {fault}");
            None
        }
    })
}
