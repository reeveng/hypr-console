//! The desktop half of the controller: scrolling, and the buttons that ask the
//! compositor for something.
//!
//! Everything that decides anything is in `console_controller`, where it can be
//! asked the same question twice. What is here is a machine's real devices,
//! offered to that as somewhere the devices are plugged in.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use evdev::uinput::VirtualDevice;
use evdev::{
    AbsoluteAxisCode, AttributeSet, Device, InputEvent, KeyCode, RelativeAxisCode,
};
use console_haste::Hurrying;
use console_controller::clock::since_boot;
use console_controller::doing::Doing;
use console_controller::finding::{Says, says};
use console_controller::means::{self, Table};
use console_pad::jobs::Rebound;
use console_controller::mode::{Awake, Mode};
use console_controller::profile::{Asked, Loading, wanted};
use console_controller::reading::{From, Ranges};
use console_controller::turning::{Gone, Plugged, READ, Took, Turning};

fn main() -> std::process::ExitCode {
    let mut out = match published() {
        Ok(out) => out,
        Err(fault) => {
            eprintln!("stick-scroll: {fault}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let mut machine = Machine::default();
    let mut turning = Turning::pointed_at(told());

    // Where the table of jobs lives, worked out once. HOME cannot change under
    // a running daemon, so asking for it every turn would turn one unset
    // variable into a line in the journal fifty times a second.
    //
    // Unset comes to the empty string, which is what this read as before unset
    // was told apart from a home set to something that is not text. The second
    // is somebody's table being looked for in the wrong place and is worth a
    // line; the first is a unit file that names no home, and the daemon has
    // always run on the table it was built with in that case.
    let home = match std::env::var("HOME") {
        Ok(home) => home,
        Err(std::env::VarError::NotPresent) => String::new(),
        Err(fault) => {
            eprintln!("stick-scroll: HOME: {fault}; running on the table this was built with");

            String::new()
        }
    };

    // What every job is bound to on this machine, and when the file it comes
    // out of was last written. Read here rather than in the library, which
    // opens nothing.
    let mut bound = Bound::of(&home);

    if let Err(fault) = bound.look(&mut turning) {
        eprintln!("stick-scroll: {fault}");
    }

    let mut holding: BTreeMap<From, String> = BTreeMap::new();
    let mut running: Vec<Child> = Vec::new();

    // What is in front of you, which is what the buttons are for. Asked once
    // at the start and again whenever the compositor says a layer opened or
    // closed, which is the only thing that can change the answer.
    //
    // A compositor this program was not started under is not a reason to stop:
    // scrolling and every button that does not depend on what is in front go on
    // working. It is said once, and what is in front is never asked again.
    let changed = match watching() {
        Ok(changed) => changed,
        Err(fault) => {
            eprintln!("stick-scroll: nothing will say when a layer opens: {fault}");

            // A channel with nobody left to speak into it, which is exactly
            // what this had before the failure was told apart from silence.
            // Scrolling and every button that does not depend on what is in
            // front go on working, and the loop below says once that they are
            // working without it.
            let (_say, heard) = channel();

            heard
        }
    };

    let mut wearing = Wearing::default();

    if let Err(fault) = look(&mut turning, &mut wearing) {
        eprintln!("stick-scroll: {fault}");
    }

    // The processors, which are asleep between presses and have to be told
    // otherwise before there is anything for them to notice. See
    // `console_haste`.
    let mut hurrying = Hurrying::default();
    let mut said_the_watcher_went = false;

    // What the home screen last said about itself. Everything else about where
    // you are arrives on the compositor's socket; this one cannot, because
    // waking a highlight opens no layer and closes none.
    let mut was_awake = Awake::asked();

    loop {
        // Asked again when the compositor says a layer opened or closed, and
        // again when the load this started lands. The second is not the same
        // question answered twice: what should be worn when a load finishes is
        // whatever is in front then, which may be neither what was in front
        // when it started nor what it loaded.
        let landed = wearing.loading.is_some() && wearing.in_flight() == Loading::Settled;

        // Nothing waiting is ordinary and means no layer opened or closed this
        // turn. The channel having no sender left is not: the thread watching
        // the compositor has ended, and from here that reads as the same quiet
        // as an idle desktop -- for ever. Said once, because this loop runs
        // fifty times a second.
        let word = match changed.try_recv() {
            Ok(()) => Word::Came,
            Err(TryRecvError::Empty) => Word::Nothing,
            Err(TryRecvError::Disconnected) => Word::Gone,
        };

        if word == Word::Gone && !said_the_watcher_went {
            eprintln!("stick-scroll: the watcher on the compositor has ended; what is in front of you will not be asked again");
            said_the_watcher_went = true;
        }

        // The home screen raising or dropping its highlight, which decides
        // whose A is whose and is the one thing about where you are that the
        // compositor cannot say -- the surface is drawn either way. Asked at
        // the rate the loop runs, because the answer is a file being there or
        // not; the expensive question behind it is only asked when this one
        // has changed its mind.
        let awake = Awake::asked();
        let woke = awake != was_awake;
        was_awake = awake;

        if word == Word::Came || landed || woke {
            // Everything queued behind it says the same thing: ask again.
            while let Ok(()) = changed.try_recv() {}

            if let Err(fault) = look(&mut turning, &mut wearing) {
                eprintln!("stick-scroll: {fault}");
            }
        }

        // Somebody may have moved a button while this was running. The setup
        // screen writes the file and nothing else; watching when it was last
        // written is the whole of the telling, and it is asked at the rate the
        // loop already runs at because a `stat` is cheaper than deciding how
        // often to do one.
        if let Err(fault) = bound.look(&mut turning) {
            eprintln!("stick-scroll: {fault}");
        }

        // Counting the time the machine spent asleep, which is the whole of
        // why `turning::AWAY_SECONDS` can tell a suspend from a slow turn. See
        // `console_controller::clock`.
        for what in turning.turn(&mut machine, since_boot()) {
            // Before the fork rather than after it. Most of what a panel costs
            // is spent before it has a line of its own running -- the loader
            // alone is a third of it -- so a processor told to hurry by the
            // program that was started has already missed the part of the
            // opening it would have helped most.
            if matches!(what, Doing::Run(_)) {
                hurrying.asked(Instant::now());
            }

            running.extend(done(&what, &mut out));
        }

        hurrying.settle(Instant::now());
        running = reaped(running);
        say_what_changed(&mut holding, &turning);
        std::thread::sleep(Duration::from_secs_f64(turning.poll()));
    }
}

/// What the channel from the compositor had to say this turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Word {
    /// A layer opened or closed, so what is in front may have moved.
    Came,
    /// Nothing this turn, which is what almost every turn is.
    Nothing,
    /// Nobody is left to say anything, and nobody ever will be.
    Gone,
}

/// The table of jobs, and when the file it came out of was last written.
///
/// Kept rather than read every turn, and re-read only when the file has
/// changed underneath. A file that will not parse leaves what was already
/// loaded where it is and says so once: a machine whose buttons all stopped
/// working because of a typo in a table is worse than one still doing what it
/// was doing.
struct Bound {
    at: PathBuf,
    written: Option<std::time::SystemTime>,
    read: bool,
}

impl Bound {
    /// Where to look, decided once, because HOME cannot change under a running
    /// daemon and the fault in reading it belongs where it is read.
    fn of(home: &str) -> Self {
        Self { at: console_pad::jobs::path_in(home), written: None, read: false }
    }

    fn look(&mut self, turning: &mut Turning) -> Result<(), String> {
        let at = self.at.clone();

        // When it was last written, or nothing where there is no file. A file
        // that will not even be stat'd is neither of those, and telling them
        // apart is the difference between a table nobody has changed and a
        // table this daemon could not read. What it is answered as is recorded
        // before it is reported, so a stat that goes on failing is one line in
        // the journal rather than fifty a second.
        let written = match std::fs::metadata(&at).and_then(|held| held.modified()) {
            Ok(when) => Some(when),
            Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => None,
            Err(fault) => {
                self.written = None;
                self.read = true;

                return Err(format!("{}: asking when it was last written: {fault}", at.display()));
            }
        };

        if self.read && written == self.written {
            return Ok(());
        }

        self.written = written;
        self.read = true;

        // No file is ordinary: nobody on this machine has moved a button, and
        // the table with nothing moved in it is what an empty one reads as. A
        // file that is there and will not be read is not ordinary, and used to
        // arrive here as the same empty string.
        let said = match std::fs::read_to_string(&at) {
            Ok(said) => said,
            Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(fault) => return Err(format!("{}: reading it: {fault}", at.display())),
        };

        match console_pad::jobs::Jobs::read(&said) {
            Ok(jobs) => {
                if jobs.moved() == Rebound::Something {
                    eprintln!("stick-scroll: {} moves {} of them", at.display(), jobs.moved.len());
                }

                turning.bound_by(Table::of(&jobs));
            }
            Err(fault) => eprintln!("stick-scroll: {}: {fault}", at.display()),
        }

        Ok(())
    }
}

/// A word whenever a layer surface opened or closed.
///
/// The compositor and nothing else. Which mode this daemon is in used to be
/// three things nobody owned -- the pad's InputPlumber profile, a file naming
/// the profile from before the keyboard came up, and a SIGSTOP -- and every
/// one of them was a note a program left for another program to read. See
/// `console_controller::mode`.
fn watching() -> Result<Receiver<()>, String> {
    let (say, heard) = channel();
    console_door::watching_layers(say)?;

    Ok(heard)
}

/// The load this daemon has going, if it has one.
///
/// The one thing the daemon has to remember about the pad, and it is about its
/// own doing rather than about the machine: everything else it needs is read
/// off the compositor or off the bus. Kept because a load is not instant and a
/// daemon that has forgotten it asked for one will ask the bus, be told what
/// was true before it asked, and agree with it.
#[derive(Default)]
struct Wearing {
    loading: Option<Child>,
    asked: Asked,
}

impl Wearing {
    /// Whether the load this started is still going.
    ///
    /// Reaped here rather than left to the general reaping, because whether it
    /// has finished is the question `look` is about to ask.
    fn in_flight(&mut self) -> Loading {
        let Some(load) = self.loading.as_mut() else { return Loading::Settled };

        match load.try_wait() {
            Ok(None) => Loading::InFlight,
            _ => {
                self.loading = None;
                Loading::Settled
            }
        }
    }
}

/// Ask the compositor what is in front, tell the daemon, and put the pad on
/// the profile that goes with it.
///
/// A compositor that cannot be asked leaves the mode where it was. Falling
/// back to the desktop here would mean a keyboard up and a `hyprctl` that
/// failed once put the pad back under this daemon while the keyboard still has it,
/// which is the fight the mode exists to end.
///
/// The profile is loaded only when it is not the one already on. A load
/// destroys the pad and builds another every time, so a load that changes
/// nothing is not free: it is this daemon and the keyboard both losing the device
/// they are reading, for nothing.
///
/// It is also not loaded over one that has not landed. `controller-profile` is
/// spawned and let go of -- waiting for it is a daemon that stops reading the
/// pad for as long as InputPlumber takes -- so between asking and the pad
/// wearing it, the bus still answers with what came before. Deciding against
/// that answer is deciding against the past. See `console_controller::profile`.
fn look(turning: &mut Turning, wearing: &mut Wearing) -> Result<(), String> {
    let screens = console_door::screens()?;
    let mode = Mode::seen(&screens, Awake::asked());
    turning.held.now_in(mode);

    let loading = wearing.in_flight();
    let worn = loaded()?;

    let Some(asking) = wanted(mode.profile(), worn.as_deref(), loading, &wearing.asked) else {
        return Ok(());
    };

    wearing.loading = run(&["controller-profile".to_string(), asking.profile.clone()]);
    wearing.asked = asking;

    Ok(())
}

/// Which profile the pad has, as the machine answers.
///
/// Nothing where it answered and named no profile, which is not the same as
/// some other profile and is not written down as one. The bus is least askable
/// exactly while a load is tearing the pad down and building another.
///
/// A `controller-profile` that will not run at all is a different thing again,
/// and it comes back as the failure it is. Folded in with the rest, as it was,
/// a daemon that could not ask went on deciding as though it had.
fn loaded() -> Result<Option<String>, String> {
    let said = Command::new("controller-profile")
        .output()
        .map_err(|fault| format!("asking which profile the pad has: {fault}"))?;

    match said.status.success() {
        false => Ok(None),
        true => Ok(Some(String::from_utf8_lossy(&said.stdout).trim().to_string())
            .filter(|worn| !worn.is_empty())),
    }
}

/// This machine's own devices.
#[derive(Default)]
struct Machine {
    open: BTreeMap<String, Device>,
}

impl Plugged for Machine {
    fn every(&self) -> Vec<Says> {
        evdev::enumerate().map(|(path, device)| says(&path.display().to_string(), &device)).collect()
    }

    fn open(&mut self, path: &str) -> Took {
        if self.open.contains_key(path) {
            return Took::Held;
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

    /// The pad's ranges, read off it once when it is found.
    ///
    /// Asked for here and not per event, because the pad goes away whenever a
    /// profile is switched and arithmetic that has to ask a device that is not
    /// there is arithmetic that stops.
    fn ranges(&self, path: &str) -> Ranges {
        let Some(device) = self.open.get(path) else { return Ranges::default() };

        let mut told: BTreeMap<u16, (i32, i32)> = BTreeMap::new();

        if let Ok(states) = device.get_absinfo() {
            for (axis, info) in states {
                told.insert(axis.0, (info.minimum(), info.maximum()));
            }
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

        if arrived.is_err() {
            self.open.remove(path);
        }

        arrived
    }
}

/// A device this was pointed at rather than left to find.
fn told() -> BTreeMap<From, String> {
    let named = |which| match which {
        From::Pad => "CONSOLE_PAD",
        From::Keys => "CONSOLE_KEYS",
        From::Touch => "CONSOLE_TOUCHPAD",
    };
    let mut out = BTreeMap::new();

    for which in READ {
        let name = named(which);

        // Unset is ordinary and is most of the time: a device nobody pointed at
        // is a device this finds for itself. A name that is set to something
        // that is not text is somebody trying to point at a device and missing,
        // and it used to arrive here as the same silence.
        let path = match std::env::var(name) {
            Ok(path) => path,
            Err(std::env::VarError::NotPresent) => continue,
            Err(fault) => {
                eprintln!("stick-scroll: {name}: {fault}; finding that device instead");
                continue;
            }
        };

        if !path.is_empty() {
            out.insert(which, path);
        }
    }

    out
}

/// A device found or lost, said once each time it happens.
///
/// The journal is where a pad that never came back is diagnosed, and a line
/// per turn would bury it.
fn say_what_changed(holding: &mut BTreeMap<From, String>, turning: &Turning) {
    let now = turning.holding();
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
}

/// The device this daemon publishes: a wheel, a pointer, and every key and
/// button anything on this desktop is bound to.
///
/// The keys are read out of the table rather than listed here. A device that
/// does not claim a key cannot send it -- the press goes nowhere and the
/// button reads as dead -- so a job given a new key would otherwise be a job
/// that silently did nothing.
fn published() -> Result<VirtualDevice, String> {
    let mut keys = AttributeSet::<KeyCode>::new();

    for key in means::sends() {
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

    VirtualDevice::builder()
        .map_err(|fault| format!("no way in to /dev/uinput: {fault}"))?
        .name("stick-scroll")
        .with_keys(&keys)
        .map_err(|fault| format!("the button: {fault}"))?
        .with_relative_axes(&axes)
        .map_err(|fault| format!("the wheel: {fault}"))?
        .build()
        .map_err(|fault| format!("the device would not build: {fault}"))
}

/// One decision, done. What it started, if it started anything.
fn done(what: &Doing, out: &mut VirtualDevice) -> Option<Child> {
    match what {
        Doing::Frame(frame) => {
            let events: Vec<InputEvent> = frame
                .iter()
                .map(|written| InputEvent::new(written.kind.0, written.code, written.value))
                .collect();

            if let Err(fault) = out.emit(&events) {
                eprintln!("stick-scroll: nothing came out: {fault}");
            }

            None
        }
        Doing::Run(argv) => {
            eprintln!("stick-scroll: {}", argv.join(" "));
            run(argv)
        }
        Doing::Tell(said) => {
            // Nothing to start and nothing to wait for. A home screen that is
            // not listening is a word that goes nowhere, which is the same
            // answer a key would have got and costs the same nothing.
            if let Err(fault) = console_door::telling(*said) {
                eprintln!("stick-scroll: the home screen was not told: {fault}");
            }

            None
        }
    }
}

/// The ones that have ended, forgotten.
///
/// A child nobody asks after stays in the table as a zombie, and the daemon
/// starts one every time a button is pressed. Asking is all it takes.
fn reaped(running: Vec<Child>) -> Vec<Child> {
    running
        .into_iter()
        .filter_map(|mut child| match child.try_wait() {
            Ok(None) => Some(child),
            _ => None,
        })
        .collect()
}

/// Start something.
///
/// Whatever this starts is in this service's control group and stays there: a
/// control group is inherited by every child, and nothing a program can do to
/// itself leaves one. So restarting this daemon takes down whatever it has
/// opened, and a panel raised from a button goes when the daemon goes.
///
/// It keeps what they say on the way out. This daemon's stderr is the journal,
/// and a chooser that refuses to open is otherwise a button reported as broken
/// against a journal showing the press arriving and the chooser starting.
fn run(argv: &[String]) -> Option<Child> {
    let (program, rest) = argv.split_first()?;
    // The moment this was decided, on the clock that does not jump, so whatever
    // is started can say how long the thumb waited rather than how long it took
    // itself. Everything between here and that program's first line -- the
    // fork, the exec, the loader -- is time somebody spent looking at a screen
    // where nothing had happened yet, and it is the one stretch of an opening
    // that no panel can see from the inside.
    let pressed = console_timings::press_stamp();

    match Command::new(program)
        .args(rest)
        .envs(pressed)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => Some(child),
        Err(fault) => {
            eprintln!("stick-scroll: cannot run {program}: {fault}");
            None
        }
    }
}
