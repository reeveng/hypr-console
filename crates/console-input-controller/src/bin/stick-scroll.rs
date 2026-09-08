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
use std::path::Path;
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
use console_input_bindings::moved::Rebound;
use console_input_controller::binds::{self, Bind};
use console_core_atomic_writes::Held;
use console_core_never::Never;
use console_program_contract::Topic;
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

    let Ok(home) = console_core_places::home();
    let Ok(mut bound) = Bound::of(home.as_deref());
    let Ok(saying) = Saying::of(home.as_deref());

    let Ok(was) = match home.as_deref() {
        Some(home) => console_input_bindings::active::read(home),
        None => Ok(console_input_bindings::active::FIRST),
    };

    let Ok(()) = turning.held.using(was);
    let Ok(()) = saying.using(was);
    let Ok(()) = settled();

    match bound.look(&mut turning) {
        Ok(()) => {},
        Err(fault) => eprintln!("stick-scroll: {fault}"),
    }

    let mut holding: Vec<(From, String)> = Vec::new();
    let mut running: Vec<LetGo> = Vec::new();

    let Ok(changed) = watching();
    let Ok(reloaded) = reloading();

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
                            let Ok(started) = done(what, &mut out, &saying);

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

        let read = match reloaded.try_recv() {
            Ok(()) => Word::Came,
            Err(TryRecvError::Empty) => Word::Nothing,
            Err(TryRecvError::Disconnected) => Word::Gone,
        };

        match read {
            Word::Came => {
                while let Ok(()) = reloaded.try_recv() {}

                let Ok(()) = bound.again();
            }
            Word::Nothing | Word::Gone => {},
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
                Doing::Frame(_) | Doing::Tell(_) | Doing::Using(_) => {
                    what_for = match what_for {
                        Decided::Nothing => Decided::Something,
                        Decided::Something | Decided::ToStart => what_for,
                    }
                }
            }

            let Ok(started) = done(&what, &mut out, &saying);

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
    at: Option<PathBuf>,
    written: Option<std::time::SystemTime>,
    read: bool,
    handed: Vec<Bind>,
}

impl Bound {
    fn of(home: Option<&Path>) -> Result<Self, Never> {
        let at = match home {
            Some(home) => {
                let Ok(at) = console_input_bindings::moved::path_in(home);

                Some(at)
            },
            None => None,
        };

        Ok(Self { at, written: None, read: false, handed: Vec::new() })
    }

    fn hand_over(&mut self, table: &Table) -> Result<(), Never> {
        let Ok(wanted) = binds::wanted(table);

        self.give(wanted)
    }

    fn again(&mut self) -> Result<(), Never> {
        let wanted = self.handed.clone();

        self.give(wanted)
    }

    fn give(&mut self, wanted: Vec<Bind>) -> Result<(), Never> {
        let Ok(holding) = binds::holding();

        let before = match holding {
            binds::Holding::These(held) => {
                let Ok(still) = binds::kept(&self.handed, &held);

                match still.len() == self.handed.len() {
                    true => {},
                    false => eprintln!(
                        "stick-scroll: the compositor is holding {} of the {} keys it was handed; \
                         putting the rest back",
                        still.len(),
                        self.handed.len()
                    ),
                }

                still
            }
            binds::Holding::Unanswered(fault) => {
                eprintln!("stick-scroll: what keys it is holding: {fault}");

                self.handed.clone()
            }
        };

        let Ok(went) = binds::told(&wanted, &before);

        match went {
            binds::Went::Through => {},
            binds::Went::Nowhere => {
                eprintln!("stick-scroll: some of the keys were refused, and those do nothing");
            }
        }

        self.handed = wanted;

        Ok(())
    }

    fn look(&mut self, turning: &mut Turning) -> Result<(), String> {
        let at = match &self.at {
            Some(at) => at.clone(),
            None => {
                match self.read {
                    true => return Ok(()),
                    false => {},
                }

                self.read = true;

                let Ok(ours) = Table::ours();
                let Ok(()) = self.hand_over(&ours);

                return Ok(());
            }
        };

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

        let Ok(held) = console_core_atomic_writes::read(&at);

        let said = match held {
            Held::Said(said) => said,
            Held::Nothing => String::new(),
            Held::Unreadable(fault) => {
                return Err(format!("{}: reading it: {fault}", at.display()));
            }
        };

        match console_input_bindings::moved::Jobs::read(&said) {
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
                let Ok(()) = self.hand_over(&table);
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

fn reloading() -> Result<Receiver<()>, Never> {
    let (say, heard) = channel();
    let Ok(listening) = console_events::listening::listen(&[Topic::Compositor]);

    let _ = std::thread::spawn(move || {
        let Ok(heard) = listening.heard();

        for heard in heard.iter() {
            let worth = match &heard {
                console_events::listening::Heard::GotIn => binds::Worth::Asking,
                console_events::listening::Heard::Said(changed) => {
                    let Ok(worth) = binds::worth_asking_after(&changed.said);

                    worth
                }
            };

            match worth {
                binds::Worth::Asking => {
                    match say.send(()) {
                        Ok(()) => {},
                        Err(_nobody_is_listening) => return,
                    }
                }
                binds::Worth::Ignoring => {},
            }
        }
    });

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
        let device = match self.open.get(path) {
            Some(device) => device,
            None => return Ranges::default(),
        };

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
        let device = match self.open.get_mut(path) {
            Some(device) => device,
            None => return Err(Gone),
        };

        let arrived = match device.fetch_events() {
            Ok(arrived) => Ok(arrived.collect()),
            Err(fault) if fault.kind() == std::io::ErrorKind::WouldBlock => Ok(Vec::new()),
            Err(_) => Err(Gone),
        };

        match &arrived {
            Err(Gone) => {
                self.open.remove(path);
            }
            Ok(_still_reading) => {},
        }

        arrived
    }
}

fn told() -> Result<BTreeMap<From, String>, Never> {
    let named = |which| match which {
        From::Pad => "CONSOLE_PAD",
        From::Keys => "CONSOLE_KEYS",
        From::Touch => "CONSOLE_TOUCHPAD",
        From::Typing => "CONSOLE_TYPING",
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
    holding: &mut Vec<(From, String)>,
    turning: &Turning,
) -> Result<(), Never> {
    let Ok(now) = turning.holding();

    for (which, path) in now {
        match holding.iter().any(|(_, at)| at == path) {
            true => {},
            false => {
                let Ok(name) = which.said();

                eprintln!("stick-scroll: reading the {name} at {path}");
            }
        }
    }

    for (which, path) in holding.iter() {
        match now.iter().any(|(_, at)| at == path) {
            true => {},
            false => {
                let Ok(name) = which.said();

                eprintln!("stick-scroll: the {name} at {path} has gone");
            }
        }
    }

    *holding = now.to_vec();

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

fn done(what: &Doing, out: &mut VirtualDevice, saying: &Saying) -> Result<Option<LetGo>, Never> {
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
        Doing::Using(on) => {
            let Ok(()) = saying.using(*on);

            Ok(None)
        }
    }
}

struct Saying {
    home: Option<PathBuf>,
}

impl Saying {
    fn of(home: Option<&Path>) -> Result<Self, Never> {
        Ok(Saying { home: home.map(Path::to_path_buf) })
    }

    fn using(&self, on: console_input_bindings::bound::Input) -> Result<(), Never> {
        let home = match &self.home {
            Some(home) => home,
            None => return Ok(()),
        };

        match console_input_bindings::active::remember(home, on) {
            Ok(()) => {},
            Err(fault) => eprintln!("stick-scroll: {fault}"),
        }

        Ok(())
    }
}

fn settled() -> Result<(), Never> {
    let named = console_input_language::NAMED;
    let mut starting = Command::new(named);

    starting.arg(console_input_language::SETTLE).stdout(Stdio::null()).stderr(Stdio::inherit());

    match let_go(&mut starting) {
        Ok(_child) => {},
        Err(fault) => eprintln!("stick-scroll: cannot run {named}: {fault}"),
    }

    Ok(())
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
    let (program, rest) = match argv.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(None),
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
