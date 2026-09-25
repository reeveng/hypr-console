//! The desktop half of the controller: scrolling, and the buttons that ask the
//! compositor for something.  Everything that decides anything is in
//! `console_input_controller`, where it can be asked the same question twice.
//! What is here is a machine's real devices, offered to that as somewhere the
//! devices are plugged in.  It is also where the daemon's own share of a wait
//! is written down. Every other stopwatch on this device starts when a program
//! is exec'd, so the stretch between the pad saying something and this deciding
//! to act on it was the one no one could see -- and it is the stretch that says
//! whether a slow opening is the daemon or the toolkit.  A turn happens when
//! something arrives, and many of them decide nothing, so many of them write
//! nothing. A turn that starts a program always writes, because that is a
//! press someone is waiting on the far end of. A turn that only scrolled or
//! told the home screen something writes only if it took longer than a frame,
//! which is the only version of it worth reading.
//!
//! A question to the compositor that is the same as one still waiting on its
//! answer is not asked again. A shoulder pressed faster than `hyprctl` could
//! answer used to start one process per press, each one the compositor and
//! every listener on its events had to get through before the screen caught
//! up, so a few presses were a second of the desktop chugging behind a thumb
//! that had already stopped. The one in flight answers for the presses behind it.
//!
//! Between turns it waits on the devices themselves, and on the two things
//! that change without a press: a device plugged in or a bindings file
//! written, which inotify says, and a word from the compositor, which the
//! threads listening for it say down a pipe as well as a channel. It used to
//! sleep a fiftieth of a second instead, whatever was happening, which was a
//! machine with its screen dark waking fifty times a second to find nothing.
//! How long it waits is `turning.wake()`'s to say, and the only thing added
//! here is the processors being hurried, which has an end to be looked at.
//! Whether the machine is awake is not waited on: it is asked at the top of
//! every turn, before the press that turn carries is decided, so a turn that
//! comes late to it has lost nothing.

use std::collections::BTreeSet;
use std::collections::BTreeMap;
use std::os::fd::OwnedFd;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::path::Path;
use std::time::{Duration, Instant};

use rustix::event::{Nsecs, PollFd, PollFlags, Secs, Timespec, poll};
use rustix::fs::inotify::{self, CreateFlags, WatchFlags};
use rustix::pipe::{PipeFlags, pipe_with};

use console_input_event_devices::{
    AbsoluteAxisCode, BusType, Device, InputEvent, InputId, RelativeAxisCode, Setup, Unmade,
    VirtualDevice,
};
use console_program_lifetime::{Detached, let_go, threads};
use console_response_times::{Note, Wait};
use console_cpu_boost::{Backoff, Boost};
use console_input_controller::clock;
use console_input_controller::effect::{Effect, Reconnected};
use console_input_controller::finding::{DeviceInfo, describe};
use console_input_controller::actions::{self, Table};
use console_input_bindings::moved::Rebound;
use console_input_controller::binds::{self, KeyBinding};
use console_core_atomic_writes::Stored;
use console_core_never::Never;
use console_program_contract::Topic;
use console_compositor::events::CompositorEvent;
use console_input_controller::mode::{Woken, Mode};
use console_input_controller::reading::{From, POLL, Ranges, Wake};
use console_input_controller::turning::{Closed, Plugged, READ, Took, Turning};

#[derive(Debug)]
enum Unscrolled {
    Read(PathBuf, std::io::Error),
    Parse(PathBuf, String),
    Onscreen(console_onscreen::Error),
    Unbuilt(Unmade),
    Unrung(rustix::io::Errno),
    Unwaited(rustix::io::Errno),
}

impl std::fmt::Display for Unscrolled {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unscrolled::Read(at, fault) => write!(
                to,
                "{}: asking when it was last written: {fault}",
                at.display()
            ),
            Unscrolled::Parse(at, fault) => {
                write!(to, "{}: reading it: {fault}", at.display())
            }
            Unscrolled::Onscreen(fault) => write!(to, "{fault}"),
            Unscrolled::Unbuilt(fault) => write!(to, "the device would not build: {fault}"),
            Unscrolled::Unrung(fault) => {
                write!(to, "nothing to hear the compositor on between presses: {fault}")
            }
            Unscrolled::Unwaited(fault) => write!(to, "waiting for the next press: {fault}"),
        }
    }
}

impl std::error::Error for Unscrolled {}

impl std::convert::From<console_onscreen::Error> for Unscrolled {
    fn from(fault: console_onscreen::Error) -> Self {
        Unscrolled::Onscreen(fault)
    }
}

fn main() -> std::process::ExitCode {
    let mut out = match published() {
        Ok(out) => out,
        Err(fault) => {
            eprintln!("controller-desktop: {fault}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let (rung, ringing) = match pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK) {
        Ok(ends) => ends,
        Err(fault) => {
            eprintln!("controller-desktop: {}", Unscrolled::Unrung(fault));
            return std::process::ExitCode::FAILURE;
        }
    };

    let ringing = Arc::new(ringing);

    let mut machine = Machine::default();
    let Ok(told) = told();
    let Ok(mut turning) = Turning::pointed_at(told);

    let Ok(home) = console_core_places::home();
    let Ok(mut bound) = Bound::of(home.as_deref());
    let Ok(saying) = Sender::of(home.as_deref());

    let Ok(was) = match home.as_deref() {
        Some(home) => console_input_bindings::active::read(home),
        None => Ok(console_input_bindings::active::FIRST),
    };

    let Ok(()) = turning.held.using(was);
    let Ok(()) = saying.using(was);
    let Ok(()) = settled();

    match bound.look(&mut turning) {
        Ok(()) => {},
        Err(fault) => eprintln!("controller-desktop: {fault}"),
    }

    let mut holding: Vec<(From, String)> = Vec::new();
    let mut running: Vec<Detached> = Vec::new();

    let Ok(watched) = watching();
    let Ok(changed) = rung_on(watched, &ringing);
    let Ok(reloads) = reloading();
    let Ok(reloaded) = rung_on(reloads, &ringing);
    let Ok(()) = closing();
    let Ok(plugging) = plugging(bound.at.as_deref());
    let listening = Subscription { rung, plugging };

    match look(&mut turning) {
        Ok(_) => {}
        Err(fault) => eprintln!("controller-desktop: {fault}"),
    }

    let mut hurrying = Backoff::default();
    let mut said_the_watcher_went = false;

    let Ok(mut was_awake) = Woken::asked();

    loop {
        let word = match changed.try_recv() {
            Ok(()) => Event::Came,
            Err(TryRecvError::Empty) => Event::None,
            Err(TryRecvError::Disconnected) => Event::Closed,
        };

        match word == Event::Closed && !said_the_watcher_went {
            true => {
                eprintln!("controller-desktop: the watcher on the compositor has ended; what is in front of you will not be asked again");
                said_the_watcher_went = true;
            }
            false => {},
        }

        let Ok(awake) = Woken::asked();

        let woke = awake != was_awake;
        was_awake = awake;

        match word == Event::Came || woke {
            true => {
                while let Ok(()) = changed.try_recv() {}

                match look(&mut turning) {
                    Ok(letting_go) => {
                        for what in &letting_go {
                            let Ok(started) = done(what, &mut out, &saying);

                            running.extend(started);
                        }
                    }
                    Err(fault) => eprintln!("controller-desktop: {fault}"),
                }
            }
            false => {},
        }

        match bound.look(&mut turning) {
            Ok(()) => {},
            Err(fault) => eprintln!("controller-desktop: {fault}"),
        }

        let read = match reloaded.try_recv() {
            Ok(()) => Event::Came,
            Err(TryRecvError::Empty) => Event::None,
            Err(TryRecvError::Disconnected) => Event::Closed,
        };

        match read {
            Event::Came => {
                while let Ok(()) = reloaded.try_recv() {}

                let Ok(()) = bound.again();
            }
            Event::None | Event::Closed => {},
        }

        let Ok(()) = turned(Turn {
            turning: &mut turning,
            machine: &mut machine,
            hurrying: &mut hurrying,
            out: &mut out,
            saying: &saying,
            running: &mut running,
        });

        let Ok(()) = hurrying.settle(Instant::now());
        let Ok(still) = console_program_lifetime::reaped(running);

        running = still;
        let Ok(()) = say_what_changed(&mut holding, &turning);
        let Ok(wake) = turning.wake();
        let Ok(hurried) = hurrying.on();

        let Ok(wake) = match hurried {
            Boost::On => wake.sooner(Wake::Within(POLL)),
            Boost::Off => Ok(wake),
        };

        match waited(wake, &machine, &listening) {
            Ok(Plugging::Some) => {
                let Ok(()) = turning.hunt_now();
            }
            Ok(Plugging::None) => {},
            Err(fault) => {
                eprintln!("controller-desktop: {fault}");

                return std::process::ExitCode::FAILURE;
            }
        }
    }
}

struct Subscription {
    rung: OwnedFd,
    plugging: Option<OwnedFd>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Plugging {
    Some,
    None,
}

fn waited(wake: Wake, machine: &Machine, listening: &Subscription) -> Result<Plugging, Unscrolled> {
    let patience = match wake {
        Wake::OnInput => None,
        Wake::Within(seconds) => {
            let long = match Duration::try_from_secs_f64(seconds) {
                Ok(long) => long,
                Err(_not_a_length) => Duration::ZERO,
            };
            let Ok(tv_sec) = console_core_number_conversion::fitted::<u64, Secs>(long.as_secs());
            let Ok(tv_nsec) = console_core_number_conversion::fitted::<u32, Nsecs>(long.subsec_nanos());

            Some(Timespec { tv_sec, tv_nsec })
        }
    };

    let mut watch = vec![PollFd::new(&listening.rung, PollFlags::IN)];

    watch.extend(listening.plugging.iter().map(|plugging| PollFd::new(plugging, PollFlags::IN)));
    watch.extend(machine.open.values().map(|device| PollFd::new(device, PollFlags::IN)));

    match poll(&mut watch, patience.as_ref()) {
        Ok(_) | Err(rustix::io::Errno::INTR) => {},
        Err(fault) => return Err(Unscrolled::Unwaited(fault)),
    }

    let Ok(_rung) = emptied(&listening.rung);

    Ok(match &listening.plugging {
        Some(plugging) => {
            let Ok(heard) = emptied(plugging);

            heard
        }
        None => Plugging::None,
    })
}

fn emptied(from: &OwnedFd) -> Result<Plugging, Never> {
    let mut room = [0_u8; 4096];
    let mut heard = Plugging::None;

    loop {
        match rustix::io::read(from, &mut room) {
            Ok(0) => return Ok(heard),
            Err(_the_read_failed) => return Ok(heard),
            Ok(_) => heard = Plugging::Some,
        }
    }
}

fn plugging(bindings: Option<&Path>) -> Result<Option<OwnedFd>, Never> {
    let listening = match inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK) {
        Ok(listening) => listening,
        Err(fault) => {
            eprintln!(
                "controller-desktop: nothing plugged in will be looked for until something is pressed: {fault}"
            );

            return Ok(None);
        }
    };

    let input = console_input_event_devices::device::INPUT;

    match inotify::add_watch(&listening, input, WatchFlags::CREATE | WatchFlags::ATTRIB) {
        Ok(_) => {},
        Err(fault) => eprintln!("controller-desktop: {input}: {fault}"),
    }

    let folder = bindings.and_then(Path::parent);
    let written = WatchFlags::CLOSE_WRITE | WatchFlags::MOVED_TO | WatchFlags::CREATE | WatchFlags::DELETE;

    match folder.map(|folder| (folder, inotify::add_watch(&listening, folder, written))) {
        Some((_, Ok(_))) | None => {},
        Some((folder, Err(fault))) => eprintln!(
            "controller-desktop: {}: {fault}; the bindings are read again at the next press",
            folder.display()
        ),
    }

    Ok(Some(listening))
}

fn rung_on(heard: Receiver<()>, ringing: &Arc<OwnedFd>) -> Result<Receiver<()>, Never> {
    let (say, relayed) = channel();
    let ringing = Arc::clone(ringing);

    let Ok(()) = threads::let_go(std::thread::spawn(move || {
        for () in heard.iter() {
            match say.send(()) {
                Ok(()) => {},
                Err(_no_one_is_listening) => return,
            }

            let _already_ringing = rustix::io::write(&*ringing, &[1]);
        }
    }));

    Ok(relayed)
}

struct Turn<'a> {
    turning: &'a mut Turning,
    machine: &'a mut Machine,
    hurrying: &'a mut Backoff,
    out: &'a mut VirtualDevice,
    saying: &'a Sender,
    running: &'a mut Vec<Detached>,
}

fn turned(turn: Turn<'_>) -> Result<(), Never> {
    let Ok(mut waiting) = console_response_times::Waiting::here(Wait {
        who: "controller",
        what: "press",
    });

    let Ok(now) = clock::now();
    let Ok(decided) = turn.turning.turn(turn.machine, now);
    let mut what_for = Decided::None;

    let Ok(()) = waiting.mark("deciding");

    for what in decided {
        match &what {
            Effect::Run(arguments) => {
                let Ok(()) = turn.hurrying.asked(Instant::now());

                match (what_for, arguments.split_first()) {
                    (Decided::None | Decided::Some, Some((program, _))) => {
                        let Ok(()) = waiting.named(Note { name: "starting", said: program });
                    }
                    (Decided::ToStart, _) | (_, None) => {},
                }

                what_for = Decided::ToStart;
            }
            Effect::Reconnected(_) => {}
            Effect::Frame(_) | Effect::Tell(_) | Effect::Using(_) => {
                what_for = match what_for {
                    Decided::None => Decided::Some,
                    Decided::Some | Decided::ToStart => what_for,
                }
            }
        }

        let Ok(started) = done(&what, turn.out, turn.saying);

        turn.running.extend(started);
    }

    let Ok(()) = waiting.mark("doing");

    match what_for {
        Decided::ToStart => {
            let Ok(()) = waiting.done();
        }
        Decided::Some => {
            let Ok(()) = waiting.done_if_felt();
        }
        Decided::None => {},
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Decided {
    None,
    Some,
    ToStart,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Came,
    None,
    Closed,
}

#[cfg_attr(
    dylint_lib = "explicit048_no_unreal_state",
    allow(
        explicit048_no_unreal_state,
        reason = "`read` is whether the bindings have ever been looked for and `written` is when the file last changed; a home with no bindings file is read and has no time, which is the pair saying two different things rather than one twice"
    )
)]
struct Bound {
    at: Option<PathBuf>,
    written: Option<std::time::SystemTime>,
    read: bool,
    handed: Vec<KeyBinding>,
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

    fn give(&mut self, wanted: Vec<KeyBinding>) -> Result<(), Never> {
        let Ok(holding) = binds::holding();

        match &holding {
            binds::Holding::These(_) => {},
            binds::Holding::HyprctlError(fault) => {
                eprintln!("controller-desktop: what keys it is holding: {fault}");
            }
        }

        let Ok(standing) = binds::standing(&wanted, &holding);

        match standing.missing {
            0 => {},
            many => eprintln!(
                "controller-desktop: handing over {many} of {} keys the compositor is not holding",
                wanted.len()
            ),
        }

        match standing.over {
            0 => {},
            many => eprintln!(
                "controller-desktop: the compositor was holding {many} of these keys more than once, so \
                 one press was running the job that many times; they are back to one each"
            ),
        }

        let Ok(sent) = binds::sent(&wanted, &self.handed, &holding);
        let Ok(went) = binds::told(&sent);

        match went {
            binds::Went::Through => {},
            binds::Went::Nowhere => {
                eprintln!("controller-desktop: some of the keys were refused, and those do nothing");
            }
        }

        self.handed = wanted;

        Ok(())
    }

    fn look(&mut self, turning: &mut Turning) -> Result<(), Unscrolled> {
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
            Err(fault) => match fault.kind() == std::io::ErrorKind::NotFound {
                true => None,
                false => {
                    self.written = None;
                    self.read = true;

                    return Err(Unscrolled::Read(at, fault));
                }
            },
        };

        match self.read && written == self.written {
            true => return Ok(()),
            false => {},
        }

        self.written = written;
        self.read = true;

        let Ok(held) = console_core_atomic_writes::read(&at);

        let said = match held {
            Stored::Text(said) => said,
            Stored::Absent => String::new(),
            Stored::Failed(fault) => return Err(Unscrolled::Parse(at, fault)),
        };

        match console_input_bindings::moved::Tasks::read(&said) {
            Ok(jobs) => {
                let Ok(moved) = jobs.moved();

                match moved {
                    Rebound::Some => {
                        eprintln!(
                            "controller-desktop: {} moves {} of them",
                            at.display(),
                            jobs.moved.len()
                        );
                    }
                    Rebound::None => {},
                }

                let Ok(table) = Table::of(&jobs);
                let Ok(()) = self.hand_over(&table);
                let Ok(()) = turning.bound_by(table);
            }
            Err(fault) => eprintln!("controller-desktop: {}: {fault}", at.display()),
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
    let Ok(subscriber) = console_events::subscription::connect(&[Topic::Compositor]);

    let Ok(()) = threads::let_go(std::thread::spawn(move || {
        let Ok(received) = subscriber.received();

        for event in received.iter() {
            let worth = match &event {
                console_events::subscription::Received::Connected => binds::Worth::Querying,
                console_events::subscription::Received::Event(change) => {
                    let Ok(worth) = binds::worth_asking_after(&change.text);

                    worth
                }
            };

            match worth {
                binds::Worth::Querying => {
                    match say.send(()) {
                        Ok(()) => {},
                        Err(_no_one_is_listening) => return,
                    }
                }
                binds::Worth::Ignoring => {},
            }
        }
    }));

    Ok(heard)
}

fn closing() -> Result<(), Never> {
    let Ok(subscriber) = console_events::subscription::connect(&[Topic::Compositor]);

    threads::let_go(std::thread::spawn(move || {
        let Ok(received) = subscriber.received();
        let Ok(open) = open_now();
        let Ok(mut placed) = console_input_controller::closing::placed(&open);

        for event in received.iter() {
            let stirred = match &event {
                console_events::subscription::Received::Connected => CompositorEvent::WindowMoved,
                console_events::subscription::Received::Event(change) => {
                    let Ok(stirred) = console_compositor::events::read(&change.text);

                    stirred
                }
            };

            match stirred {
                CompositorEvent::WindowClosed(address) => {
                    let Ok(open) = open_now();
                    let Ok(front) = front_now();
                    let Ok(to) = console_input_controller::closing::goes_to(
                        placed.get(&address).copied(),
                        front,
                        &open,
                        &address,
                    );

                    match to {
                        Some(id) => {
                            let Ok(lua) = console_compositor::onto(&id.to_string(), console_compositor::Carrying::None);
                            let Ok(_done) = console_compositor::request(console_compositor::Request::Dispatch, &lua);
                        }
                        None => {},
                    }

                    let Ok(now) = console_input_controller::closing::placed(&open);

                    placed = now;
                }
                CompositorEvent::WindowOpened(_) | CompositorEvent::WindowMoved => {
                    let Ok(open) = open_now();
                    let Ok(now) = console_input_controller::closing::placed(&open);

                    placed = now;
                }
                CompositorEvent::WindowRenamed(_)
                | CompositorEvent::WindowFloated
                | CompositorEvent::WindowPinned
                | CompositorEvent::WindowFilled
                | CompositorEvent::LayerOpened
                | CompositorEvent::LayerClosed
                | CompositorEvent::WorkspaceChanged
                | CompositorEvent::ScreenFocused
                | CompositorEvent::ConfigurationReloaded
                | CompositorEvent::Ignored => {},
            }
        }
    }))
}

fn open_now() -> Result<Vec<console_compositor::Window>, Never> {
    Ok(match console_compositor::query(console_compositor::Query::Clients) {
        Ok(console_compositor::Answer::Clients(open)) => open,
        Ok(other) => {
            eprintln!("controller-desktop: hyprctl answered {other:?} when asked for windows");

            Vec::new()
        }
        Err(fault) => {
            eprintln!("controller-desktop: {fault}");

            Vec::new()
        }
    })
}

fn front_now() -> Result<Option<i64>, Never> {
    Ok(match console_compositor::query(console_compositor::Query::ActiveWorkspace) {
        Ok(console_compositor::Answer::ActiveWorkspace(front)) => front.map(|workspace| workspace.id),
        Ok(_not_what_was_asked) => None,
        Err(_the_compositor_would_not_say_which_one_is_in_front) => None,
    })
}

fn look(turning: &mut Turning) -> Result<Vec<Effect>, Unscrolled> {
    let screens = console_onscreen::screens()?;
    let Ok(awake) = Woken::asked();

    let Ok(mode) = Mode::seen(&screens, awake);
    let Ok(now_in) = turning.held.now_in(mode);

    Ok(now_in)
}

#[derive(Default)]
struct Machine {
    open: BTreeMap<String, Device>,
}

impl Plugged for Machine {
    fn every(&self) -> Vec<DeviceInfo> {
        let Ok(every) = Device::every();

        every
            .iter()
            .map(|device| {
                let Ok(information) = describe(&device.path.display().to_string(), device);

                information
            })
            .collect()
    }

    fn open(&mut self, path: &str) -> Took {
        match self.open.contains_key(path) {
            true => return Took::Acquired,
            false => {},
        }

        let opened = Device::open(Path::new(path)).and_then(|device| {
            device.nonblocking()?;
            Ok(device)
        });

        match opened {
            Ok(device) => {
                self.open.insert(path.to_string(), device);
                Took::Acquired
            }
            Err(fault) => {
                eprintln!("controller-desktop: {path}: {fault}");
                Took::Denied
            }
        }
    }

    fn ranges(&self, path: &str) -> Ranges {
        let device = match self.open.get(path) {
            Some(device) => device,
            None => return Ranges::default(),
        };

        let mut told: BTreeMap<u16, (i32, i32)> = BTreeMap::new();

        match device.absolute() {
            Ok(states) => {
                for (axis, information) in states {
                    told.insert(axis.0, (information.minimum, information.maximum));
                }
            }
            Err(_the_device_went_away) => {},
        }

        let widest = [AbsoluteAxisCode::ABS_RX, AbsoluteAxisCode::ABS_RY]
            .iter()
            .filter_map(|axis| told.get(&axis.0))
            .map(|(low, high)| low.abs().max(high.abs()))
            .max()
            .filter(|span| *span > 0);

        let stick = match widest {
            Some(stick) => stick,
            None => Ranges::default().stick,
        };

        let trigger = match told.get(&AbsoluteAxisCode::ABS_Z.0).copied() {
            Some(trigger) => trigger,
            None => Ranges::default().trigger,
        };

        Ranges { stick, trigger }
    }

    fn drain(&mut self, path: &str) -> Result<Vec<InputEvent>, Closed> {
        let device = match self.open.get_mut(path) {
            Some(device) => device,
            None => return Err(Closed),
        };

        let arrived = match device.read_events() {
            Ok(arrived) => Ok(arrived),
            Err(fault) => match fault.kind() == std::io::ErrorKind::WouldBlock {
                true => Ok(Vec::new()),
                false => Err(Closed),
            },
        };

        match &arrived {
            Err(Closed) => {
                self.open.remove(path);
            }
            Ok(_still_reading) => {},
        }

        arrived
    }
}

fn told() -> Result<BTreeMap<From, String>, Never> {
    let mut out = BTreeMap::new();

    for which in READ {
        let Ok(told) = which.told();

        match told {
            Some(path) => {
                out.insert(which, path);
            },
            None => {},
        }
    }

    Ok(out)
}

fn say_what_changed(
    holding: &mut Vec<(From, String)>,
    turning: &Turning,
) -> Result<(), Never> {
    let Ok(now) = turning.holding();

    let was: BTreeSet<&str> = holding.iter().map(|(_, at)| at.as_str()).collect();
    let is: BTreeSet<&str> = now.iter().map(|(_, at)| at.as_str()).collect();

    for (which, path) in &now {
        match was.contains(path.as_str()) {
            true => {},
            false => {
                let Ok(name) = which.said();

                eprintln!("controller-desktop: reading the {name} at {path}");
            }
        }
    }

    for (which, path) in holding.iter() {
        match is.contains(path.as_str()) {
            true => {},
            false => {
                let Ok(name) = which.said();

                eprintln!("controller-desktop: the {name} at {path} has gone");
            }
        }
    }

    *holding = now;

    Ok(())
}

fn published() -> Result<VirtualDevice, Unscrolled> {
    let Ok(sends) = actions::sends();
    let setup = Setup {
        name: "controller-desktop".to_string(),
        id: InputId { bus: BusType::BUS_USB, vendor: 0x1234, product: 0x5678, version: 0x111 },
        keys: sends.into_iter().collect(),
        relative_axes: vec![
            RelativeAxisCode::REL_HWHEEL,
            RelativeAxisCode::REL_WHEEL,
            RelativeAxisCode::REL_X,
            RelativeAxisCode::REL_Y,
        ],
        ..Setup::default()
    };

    VirtualDevice::create(&setup).map_err(Unscrolled::Unbuilt)
}

fn done(
    what: &Effect,
    out: &mut VirtualDevice,
    saying: &Sender,
) -> Result<Option<Detached>, Never> {
    match what {
        Effect::Frame(frame) => {
            let events: Vec<InputEvent> = frame
                .iter()
                .map(|written| InputEvent { kind: written.kind, code: written.code, value: written.value })
                .collect();

            match out.emit(&events) {
                Ok(()) => {},
                Err(fault) => eprintln!("controller-desktop: nothing came out: {fault}"),
            }

            Ok(None)
        }
        Effect::Run(arguments) => {
            let Ok(dispatched) = what.dispatched();

            match dispatched {
                Some(lua) => {
                    let Ok(taken) = console_compositor::request(console_compositor::Request::Dispatch, lua);

                    match taken {
                        console_compositor::DispatchResult::Success => {},
                        console_compositor::DispatchResult::Failure(why) => {
                            eprintln!("controller-desktop: the compositor would not {lua}: {why}");
                        }
                    }

                    Ok(None)
                }
                None => {
                    eprintln!("controller-desktop: {}", arguments.join(" "));

                    let Ok(started) = run(arguments);

                    Ok(started)
                }
            }
        }
        Effect::Tell(said) => {
            match console_onscreen::telling(*said) {
                Ok(()) => {},
                Err(fault) => eprintln!("controller-desktop: the home screen was not told: {fault}"),
            }

            Ok(None)
        }
        Effect::Using(on) => {
            let Ok(()) = saying.using(*on);

            Ok(None)
        }
        Effect::Reconnected(back) => {
            let Ok(()) = reconnected(*back);

            Ok(None)
        }
    }
}

fn reconnected(back: Reconnected) -> Result<(), Never> {
    let Ok(device) = back.device.said();

    eprintln!("controller-desktop: the {device} came back after {:.1?}", back.gone);

    let Ok(mut waiting) = console_response_times::Waiting::here(Wait { who: "controller", what: "reconnected" });
    let Ok(()) = waiting.taking("gone", back.gone);
    let Ok(()) = waiting.named(Note { name: "device", said: device });

    waiting.done()
}

struct Sender {
    home: Option<PathBuf>,
}

impl Sender {
    fn of(home: Option<&Path>) -> Result<Self, Never> {
        Ok(Sender { home: home.map(Path::to_path_buf) })
    }

    fn using(&self, on: console_input_bindings::bound::Input) -> Result<(), Never> {
        let home = match &self.home {
            Some(home) => home,
            None => return Ok(()),
        };

        match console_input_bindings::active::remember(home, on) {
            Ok(()) => {},
            Err(fault) => eprintln!("controller-desktop: {fault}"),
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
        Err(fault) => eprintln!("controller-desktop: cannot run {named}: {fault}"),
    }

    Ok(())
}

fn run(arguments: &[String]) -> Result<Option<Detached>, Never> {
    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(None),
    };

    let mut starting = Command::new(program);
    starting.args(rest).stdout(Stdio::null()).stderr(Stdio::inherit());
    let Ok(()) = console_response_times::pressed(&mut starting, "pad");

    Ok(match let_go(&mut starting) {
        Ok(child) => Some(child),
        Err(fault) => {
            eprintln!("controller-desktop: cannot run {program}: {fault}");
            None
        }
    })
}
