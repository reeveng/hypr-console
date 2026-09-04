//! The on-screen keyboard.
//!
//! Started once with the session and kept for it. Most of that time there is
//! nothing of it on the screen: `--hidden` starts it away, and the controller
//! shows and hides it with a signal, because a keyboard that was started and
//! stopped would pay for a compositor connection, ten composed keymaps and a
//! font every time somebody wanted to type a word.
//!
//!     virtual-keyboard --landscape-layers landscape,thai,landscapespecial
//!
//! The unit runs this, and `keyboard-toggle` is the signal. It reads the
//! palette on the way in rather than being handed it: there was a second
//! program that did the reading and exec'd this one, from when this one was C
//! and could not be given a Rust crate to ask. Both ends are Rust in one crate
//! now, and the indirection cost more than it carried -- a unit that names a
//! program that starts the program that matters is a unit `named_by` cannot see
//! through, so a new keyboard could be installed and the old one go on running.


use console_number::fitted;
use std::process::ExitCode;

use console_colour::spent::{SPENT, read};
use console_pad::finding::{self, Says};
use evdev::{AbsoluteAxisCode, Device, EventSummary};
use keyboard::config::{self, Config};
use keyboard::drawing::Surface;
use keyboard::gamepad::{self, Asked, Held, Repeats};
use keyboard::layout::{Drops, Kind, Which, key, mods, named, of, placed, toward, under};
use keyboard::paint;
use keyboard::surface::{Gone, Missing, Poke, Screen, Showing};
use std::time::Instant;
use keyboard::typing::{After, Typist};

/// The layers walked when nothing says otherwise, in each orientation.
///
/// Latin, Thai, and the symbols that do not fit beside either. Which languages
/// a machine types is the machine's to say and not this file's -- `-l` and
/// `--landscape-layers` are how it says it, and `desktop.conf` is where.
/// These are what it falls back to, and they are the C's compiled-in lists.
///
/// The two orientations are not the same three arrangements at a different
/// size. In landscape nothing cycles: `landscape` and `landscapespecial` reach
/// each other and Thai *by name*, so every layer key says where it goes and
/// goes there in one press, where the portrait walk has one key that steps
/// round the ring. They are also the pair that share a grid -- both are four
/// rows of thirteen, so moving between them moves no key.
const WALK: [&str; 3] = ["full", "thai", "special"];
const LANDSCAPE_WALK: [&str; 3] = ["landscape", "thai", "landscapespecial"];

/// Which way round a surface is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// Wider than it is tall, which is what a strip across the bottom is.
    Landscape,
    /// Taller than it is wide.
    Portrait,
}

/// Whether a surface this size is a landscape one.
///
/// A keyboard is a strip across the bottom, so this is all but always true:
/// the surface is as wide as the output and `-H` tall, and no output is
/// narrower than the keyboard is deep. It is written as the comparison anyway,
/// and not as `true`, because it is the question actually being asked, and a
/// screen that is ever taller than it is wide gets the right answer for the
/// right reason.
fn landscape(wide: u32, tall: u32) -> Shape {
    match wide > tall {
        true => Shape::Landscape,
        false => Shape::Portrait,
    }
}

fn main() -> ExitCode {
    // The whole of argv, program name and all: `config::parse` takes it the
    // way C's main does and skips the first word itself. Handed the tail
    // instead, it eats the first flag -- which looks like `-H 260` being
    // ignored and `260` being an unknown flag.
    let argv: Vec<String> = std::env::args().collect();

    if argv.iter().any(|word| word == "--help" || word == "-h") {
        println!("usage: virtual-keyboard [--hidden] [-H height] [-l layers] [--fn font]");
        return ExitCode::SUCCESS;
    }

    if argv.iter().any(|word| word == "--list-layers") {
        for which in Which::ALL {
            println!("{}", of(which).name);
        }

        return ExitCode::SUCCESS;
    }

    let config = match config::from_env(&dressed(&argv)) {
        Ok(config) => config,
        Err(why) => {
            eprintln!("virtual-keyboard: {why:?}");
            return ExitCode::FAILURE;
        },
    };

    match run(&config) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("virtual-keyboard: {why}");
            ExitCode::FAILURE
        },
    }
}

/// The command line, with the palette's colours put in front of what was asked
/// for.
///
/// What colour each thing is `keyboard::palette` holds, so the unit file can be
/// about starting a program: it says which alphabets this machine types and
/// nothing else. The colours go in front of what was given rather than after,
/// so a colour named on the command line still wins -- `parse` takes the last
/// of any flag, and somebody debugging one key's colour should not have to
/// know what the palette put there first.
///
/// A palette that cannot be read is said and not refused. A keyboard in the
/// wrong colours is worse than one in the right colours and better than no
/// keyboard at all, and this is the one surface on the machine a person cannot
/// type without.
fn dressed(argv: &[String]) -> Vec<String> {
    let at = std::path::Path::new("/").join(SPENT);
    let held = match std::fs::read_to_string(&at) {
        Ok(held) => held,
        Err(why) => {
            eprintln!("virtual-keyboard: no palette at {}: {why}", at.display());
            String::new()
        },
    };
    let palette = read(&held);
    let missing = keyboard::palette::missing(&palette);

    if !missing.is_empty() {
        eprintln!(
            "virtual-keyboard: the palette has no {}, so the keyboard keeps its own colour for \
             those",
            missing.join(", ")
        );
    }

    keyboard::palette::argv(&palette, &argv[1..])
}

fn run(config: &Config) -> Result<(), String> {
    // Every alphabet the system has symbols for, composed before the keyboard
    // is on the screen. Ten of them cost a few milliseconds each and the layer
    // key has to be instant.
    let alphabets = keyboard::keymap::available("evdev", &keyboard::keymap::default_symbols_root())
        .map_err(|why| format!("no keymaps: {why:?}. Is xkeyboard-config installed?"))?;

    // Which orientation, before there is a surface to measure. The compositor
    // has not configured anything yet, and the arrangement has to be chosen to
    // build the typist; the C answers the same question the same way, by
    // assuming landscape at startup and correcting itself on the first
    // configure. A strip across the bottom of a screen is landscape, and the
    // first configure has all but always agreed.
    let shape = landscape(u32::MAX, config.height);
    let (asked, height) = orientation(config, shape);
    let walk: Vec<Which> = asked.iter().filter_map(|name| named(name)).collect();

    if walk.is_empty() {
        return Err(format!("no layer called any of {asked:?}. --list-layers says what there is"));
    }

    let signals = listening().map_err(|why| format!("no signals: {why}"))?;
    let mut screen = Screen::connect().map_err(said)?;

    if !config.hidden {
        screen.show(height).map_err(said)?;
    }

    let manager = screen
        .typing()
        .ok_or("this compositor has no zwp_virtual_keyboard_v1, so nothing here could type")?
        .clone();
    let hand = screen.hand();
    let mut typist = Typist::new(&manager, screen.seat(), &hand, alphabets, walk);

    // What is under the thumb, and what the thumb has already done to it. The
    // press happens on the way down, the way every key on this desktop does,
    // and the lift only clears the highlight.
    let mut down: Option<usize> = None;
    // What is on the screen, or `None` while nothing is. Compared against what
    // a frame would be drawn from now, which is the whole of the decision to
    // draw one.
    let mut shown: Option<Drawn> = None;

    // The pad, if this machine has one, and where its selection is sitting.
    // While the keyboard is up the pad is the keyboard's: the daemon stands
    // down for exactly as long as this surface is on the screen, which is what
    // `console_controller::mode::Mode::Keyboard` is.
    let mut pad = pad();

    if pad.is_none() {
        eprintln!("virtual-keyboard: no pad found, so it is touch only");
    }

    // What each stick's numbers run between. Asked once: they are the device's
    // own and do not change while it is plugged in, and asking inside the read
    // would borrow the device that is already lending out its events.
    let ranges: Vec<(AbsoluteAxisCode, (i32, i32))> = match pad.as_ref() {
        None => Vec::new(),
        Some(device) => match device.get_absinfo() {
            Ok(all) => all.map(|(code, info)| (code, (info.minimum(), info.maximum()))).collect(),
            // Without these the sticks are read against a range of nothing,
            // which is a selection that will not move. The touch half still
            // works, so this is said rather than fatal.
            Err(fault) => {
                eprintln!("the pad would not say what its sticks run between: {fault}");
                Vec::new()
            },
        },
    };
    let mut held = Held::default();
    let mut selected: Option<usize> = None;

    while screen.closed() == Gone::No {
        let frame = Drawn {
            showing: typist.showing,
            held: typist.held,
            pressed: down,
            selected,
            size: screen.size(),
            scale: screen.scale(),
        };

        if screen.showing() == Showing::Yes && shown != Some(frame) {
            let layout = typist.layout();
            let modifiers = typist.held;
            let pressed = down;
            // Worked out here rather than in the table: the language key says
            // which alphabet it is about to reach, and that is a fact about
            // the walk this machine was started with.
            let language = typist.next_language().map(|which| of(which).alphabet.written());
            screen
                .draw(|pixels, wide, tall, scale| {
                    // No surface is no frame. The one already on the screen
                    // stays there, which is a keyboard that missed a repaint
                    // rather than one that is not there at all.
                    let Some(onto) = Surface::new(pixels, fitted(wide * 4), fitted(tall), f64::from(scale))
                    else {
                        return;
                    };

                    let across = f64::from(wide) / f64::from(scale);
                    let deep = f64::from(tall) / f64::from(scale);
                    let keys = placed(layout, across, deep);
                    paint::keyboard(&onto, &paint::Look {
                        config,
                        layout,
                        keys: &keys,
                        pressed,
                        held: modifiers,
                        selected,
                        language,
                        wide: across,
                        tall: deep,
                    });
                })
                .map_err(said)?;
            shown = Some(frame);
        }

        // Nothing to wake for unless a direction is held on the pad, and then
        // wake when its next repeat is due.
        let now = Instant::now();
        let watching: Vec<std::os::fd::RawFd> = match pad.as_ref() {
            Some(device) => vec![signals, std::os::fd::AsRawFd::as_raw_fd(device)],
            None => vec![signals],
        };
        let spoke = screen.wait_with(&watching, held.until(now)).map_err(said)?;
        let now = Instant::now();

        if spoke & 1 != 0 {
            match woken(signals) {
                Some(Told::Show) => {
                    if screen.showing() == Showing::No {
                        screen.show(height).map_err(said)?;
                    }
                },
                Some(Told::Hide) => away(&mut screen, &mut shown, &mut down, &mut held),
                Some(Told::Either) => match screen.showing() {
                    Showing::Yes => away(&mut screen, &mut shown, &mut down, &mut held),
                    Showing::No => screen.show(height).map_err(said)?,
                },
                None => {},
            }
        }

        // What the pad asked for, and the repeat that is due whether or not it
        // said anything: a held direction reports once and then goes quiet.
        //
        // Only while the keyboard is up. Away, the pad belongs to the desktop
        // and one button here concerns us, so the events are read for that and
        // nothing else: running a stick somebody is walking a menu with through
        // the repeat machinery wakes this process every ninety milliseconds to
        // work out a move it throws away again below.
        let mut wants: Vec<Asked> = Vec::new();

        if let Some(device) = pad.as_mut() {
            if spoke & 2 != 0 {
                match screen.showing() {
                    Showing::Yes => wants.extend(from_pad(device, &ranges, &mut held, now)),
                    Showing::No => wants.extend(toggles(device)),
                }
            }
        }

        wants.extend(held.due(now));

        for want in wants {
            // While the keyboard is away the pad belongs to the desktop and one
            // button concerns us: X, which brings the keyboard up. It means the
            // same thing on this screen and every other, so there is one button
            // to learn wherever you press it.
            if screen.showing() == Showing::No {
                if want == Asked::Toggle {
                    screen.show(height).map_err(said)?;
                }

                continue;
            }

            let Some((wide, tall)) = screen.size() else { continue };

            let layout = typist.layout();
            let keys = placed(layout, f64::from(wide), f64::from(tall));

            match want {
                Asked::Toggle => {
                    away(&mut screen, &mut shown, &mut down, &mut held);
                    selected = None;
                },
                _ if want.direction().is_some() => {
                    let (dx, dy) = want.direction().unwrap_or((0, 0));
                    selected = toward(&keys, selected, dx, dy);
                },
                Asked::Press => {
                    // Where the stick is sitting is where the press lands. A
                    // press with nothing selected selects rather than types:
                    // typing a key nobody has pointed at would be a keyboard
                    // that types on its own.
                    let Some(at) = selected else {
                        selected = toward(&keys, None, 0, 0);
                        continue;
                    };

                    let Some(key) = layout.keys.get(at) else { continue };

                    if typist.pressed(key.kind, key.force, key.reset) == After::Draw
                        && !matches!(key.kind, Kind::Code { .. })
                    {
                        // The arrangement changed under the selection, and an
                        // index into the old one would light whatever key
                        // inherited the number.
                        selected = None;
                    }
                },
                Asked::Backspace => typist.tap(key::BACKSPACE),
                Asked::Enter => typist.tap(key::ENTER),
                Asked::Shift => {
                    typist.pressed(Kind::Mod(mods::SHIFT), mods::NONE, Drops::Nothing);
                },
                Asked::PreviousLanguage | Asked::NextLanguage => {
                    typist.pressed(Kind::Language, mods::NONE, Drops::Nothing);
                    // The arrangement changed under the selection, and an index
                    // into the old one would light whatever key inherited it.
                    selected = None;
                },
                _ => {},
            }
        }

        let Some((wide, tall)) = screen.size() else {
            let _ = screen.pokes();
            continue;
        };

        for poke in screen.pokes() {
            let layout = typist.layout();
            let keys = placed(layout, f64::from(wide), f64::from(tall));

            match poke {
                Poke::Down { x, y } => {
                    let Some(hit) = under(&keys, x, y) else { continue };

                    let Some(key) = layout.keys.get(hit.at) else { continue };

                    down = Some(hit.at);
                    let kind = key.kind;
                    let force = key.force;
                    let reset = key.reset;

                    if typist.pressed(kind, force, reset) == After::Draw {
                        down = match kind {
                            // A key that changed the arrangement is a key that
                            // is no longer under the thumb: highlighting an
                            // index into a table that has been swapped would
                            // light whatever key inherited the number.
                            Kind::Code { .. } => down,
                            _ => None,
                        };
                    }
                },
                Poke::Moved { .. } => {},
                Poke::Up => down = None,
            }
        }
    }

    Ok(())
}

/// Take the keyboard off the screen and forget everything about being on it.
///
/// Three things go together and were coming apart. The surface goes, which the
/// compositor is told; what was on the screen is forgotten, so that the next
/// showing draws a frame rather than comparing against one from before the
/// keyboard went away; and a direction still held is let go of, or the loop
/// goes on waking for a repeat nobody can see and nothing will act on.
fn away(screen: &mut Screen, shown: &mut Option<Drawn>, down: &mut Option<usize>, held: &mut Held) {
    screen.hide();
    *shown = None;
    *down = None;
    *held = Held::default();
}

/// The one thing the pad can ask for while the keyboard is away.
///
/// The events are read and thrown away rather than left, because they are read
/// from the same descriptor the loop is polling: a pad nobody drains is a pad
/// that says it has something to say for ever, which is a loop that never
/// sleeps again.
fn toggles(device: &mut Device) -> Vec<Asked> {
    let Ok(events) = device.fetch_events() else { return Vec::new() };

    events
        .filter_map(|event| match event.destructure() {
            EventSummary::Key(_, code, value) => {
                gamepad::from_button(code, gamepad::pushed(value))
            },
            _ => None,
        })
        .filter(|asked| *asked == Asked::Toggle)
        .collect()
}

/// The pad InputPlumber publishes, opened for reading.
///
/// The only part of the keyboard that touches a device. `console_pad::finding`
/// says which of them is the one to read -- the made one rather than the one
/// somebody is holding -- and it is asked of a list, so the rule it applies is
/// the same rule the daemon applies and is tested without a device in the room.
///
/// `None` is not a failure. A keyboard with no pad is a keyboard you type on
/// with a thumb, which is most of what it is for.
fn pad() -> Option<Device> {
    let mut said: Vec<(Says, Device)> = Vec::new();

    let reading = match std::fs::read_dir("/dev/input") {
        Ok(reading) => reading,
        // Not the same as a keyboard with no pad, which is the ordinary case
        // this returns `None` for: this is a keyboard that could not look, and
        // whoever is holding a pad that does nothing wants that in the journal.
        Err(fault) => {
            eprintln!("virtual-keyboard: /dev/input would not be read: {fault}");
            return None;
        },
    };

    for entry in reading {
        let Ok(entry) = entry else { continue };

        let path = entry.path();

        if !path.file_name().is_some_and(|n| n.to_string_lossy().starts_with("event")) {
            continue;
        }

        let at = path.to_string_lossy().to_string();

        let Ok(device) = Device::open(&path) else { continue };

        said.push((finding::says(&at, &device), device));
    }

    let names: Vec<Says> = said.iter().map(|(says, _)| says.clone()).collect();
    let wanted = finding::gamepad(&names)?.path.clone();
    let (_, device) = said.into_iter().find(|(says, _)| says.path == wanted)?;
    // Non-blocking, because it is polled beside the compositor's socket and a
    // read that waited would be a keyboard that stopped drawing.
    let _ = device.set_nonblocking(true);
    Some(device)
}

/// What the pad has said since it was last asked.
///
/// Read whole rather than one event at a time: a stick reports both axes in
/// one frame, and answering the first before reading the second is how a
/// diagonal push becomes two moves.
fn from_pad(device: &mut Device, ranges: &[(AbsoluteAxisCode, (i32, i32))], held: &mut Held, now: Instant) -> Vec<Asked> {
    let Ok(events) = device.fetch_events() else { return Vec::new() };

    let mut out = Vec::new();

    for event in events {
        let said = match event.destructure() {
            EventSummary::Key(_, code, value) => {
                gamepad::from_button(code, gamepad::pushed(value))
            },
            EventSummary::AbsoluteAxis(_, axis, value) => match axis {
                AbsoluteAxisCode::ABS_HAT0X | AbsoluteAxisCode::ABS_HAT0Y => {
                    gamepad::from_hat(axis, value)
                },
                _ => match ranges.iter().find(|(code, _)| *code == axis) {
                    Some((_, range)) => gamepad::from_stick(axis, value, *range),
                    None => None,
                },
            },
            _ => continue,
        };
        // A direction that is merely still held asks for nothing; the repeat
        // is what moves it after the first key. Everything else passes through.
        let moving = said.is_none_or(|a| a.repeats() == Repeats::Held);

        match moving {
            true => out.extend(held.went(said, now)),
            false => out.extend(said),
        }
    }

    out
}

/// Everything a frame is drawn from.
///
/// Held so that a frame nobody could tell from the one already on the screen is
/// not drawn. Every field is something `paint::keyboard` reads: the arrangement
/// decides which keys there are and what the language key says, the modifiers
/// decide which face of each key is shown, and the two indices are the key
/// under a thumb and the key under the stick. The size and the scale are here
/// because a configure changes what a frame has to be without changing
/// anything the keyboard did.
///
/// It replaces a `draw = true` written at the top of every arm that might have
/// changed something. That is the same list, kept by hand, and it was wrong in
/// the cheap direction: a direction held against the edge of the keyboard
/// repainted an identical strip eleven times a second, because moving the
/// selection and failing to move it went down the same arm.
#[derive(Clone, Copy, PartialEq)]
struct Drawn {
    showing: Which,
    held: u8,
    pressed: Option<usize>,
    selected: Option<usize>,
    size: Option<(u32, u32)>,
    scale: i32,
}

/// The arrangements to walk and the height to ask for, in one orientation.
///
/// Both come from the same answer, which is why they are decided together: the
/// C keeps a second height for landscape as well as a second list of layers,
/// and a keyboard that took the layers of one orientation and the height of
/// the other would draw four rows into the space of five.
fn orientation(config: &Config, shape: Shape) -> (Vec<&str>, u32) {
    let (given, fallback, height) = match shape {
        Shape::Landscape => {
            (&config.landscape_layers, LANDSCAPE_WALK, config.landscape_height)
        },
        Shape::Portrait => (&config.layers, WALK, config.height),
    };
    let asked: Vec<&str> = match given.is_empty() {
        true => fallback.to_vec(),
        false => given.iter().map(String::as_str).collect(),
    };
    (asked, height)
}

/// What a signal was asking for.
enum Told {
    Show,
    Hide,
    Either,
}

/// Take the three signals that show and hide the keyboard off the default
/// handlers and onto a descriptor.
///
/// A signal handler cannot draw a keyboard: it runs between two instructions
/// of whatever was happening and may not allocate, take a lock, or talk to a
/// compositor. A `signalfd` turns the signal into something readable, which is
/// something a poll loop can wait on beside the compositor's own socket, and
/// the keyboard answers it where it answers everything else.
fn listening() -> Result<std::os::fd::RawFd, std::io::Error> {
    // SAFETY: a mask on the stack, filled and applied by the calls that own it.
    unsafe {
        let mut mask: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut mask);
        libc::sigaddset(&mut mask, libc::SIGUSR1);
        libc::sigaddset(&mut mask, libc::SIGUSR2);
        libc::sigaddset(&mut mask, libc::SIGRTMIN());

        // Blocked first, or the default disposition kills the process before
        // the descriptor is ever read. SIGUSR1 and SIGRTMIN both end a program
        // that has not said otherwise.
        if libc::pthread_sigmask(libc::SIG_BLOCK, &mask, std::ptr::null_mut()) != 0 {
            return Err(std::io::Error::last_os_error());
        }

        let fd = libc::signalfd(-1, &mask, libc::SFD_CLOEXEC);

        match fd < 0 {
            true => Err(std::io::Error::last_os_error()),
            false => Ok(fd),
        }
    }
}

/// Read one signal off the descriptor and say what it asked for.
fn woken(from: std::os::fd::RawFd) -> Option<Told> {
    // SAFETY: a struct the kernel fills, read whole or not at all.
    let said = unsafe {
        let mut said: libc::signalfd_siginfo = std::mem::zeroed();
        let size = std::mem::size_of::<libc::signalfd_siginfo>();
        let got = libc::read(from, std::ptr::from_mut(&mut said).cast(), size);

        if got != fitted::<usize, isize>(size) {
            return None;
        }

        said
    };

    match fitted::<u32, i32>(said.ssi_signo) {
        libc::SIGUSR1 => Some(Told::Hide),
        libc::SIGUSR2 => Some(Told::Show),
        _ => Some(Told::Either),
    }
}

/// What went wrong with the compositor, in a sentence.
fn said(why: Missing) -> String {
    match why {
        Missing::Compositor(_) => "no compositor answered on WAYLAND_DISPLAY".to_string(),
        Missing::Global(what) => format!("the compositor has no {what}"),
        Missing::Gone(_) => "the compositor went away".to_string(),
        Missing::Memory(why) => format!("no memory for a frame: {why}"),
    }
}
