//! Put a picture on the screen, and keep the right one there.
//!
//!     console-sky        keep the right picture up
//!     console-sky --now  put the right one up and stop
//!
//! It wakes for three reasons and no others. The compositor says something has
//! covered the wallpaper or stopped covering it; the weather has an answer;
//! or enough time has gone by that the sun has moved. Between those it is
//! asleep. So is the wallpaper daemon, whenever anything is in front of the
//! picture, because what it was handed then is one frame that lasts for ever.
//!
//! `--now` is for the settings panel, which has just written down what she
//! asked for and would rather she saw it happen than waited for the next time
//! this came round.
//!
//! Nothing here decides anything. What picture answers a rainy dusk is
//! `console_sky::choose`, whether the wallpaper is covered is
//! `console_sky::covered`, where the sun is is `console_sky::sun`. This is the
//! loop that asks them and the one line that tells the wallpaper daemon.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use console_sky::choose::{self, Outside, Set, Wanted};
use console_sky::weather::Weather;
use console_sky::{covered, here, moon, place, sun, weather};

/// How often the sun is worked out again when nothing else has happened.
///
/// The sun crosses the six degrees that separate dusk from night in about half
/// an hour at these latitudes, so five minutes is fine enough that nobody sees
/// a picture arrive late, and coarse enough that a machine left alone all day
/// wakes fewer than three hundred times.
const LOOK_AGAIN: Duration = Duration::from_secs(300);

/// How often the weather is asked for.
///
/// The service reports in quarter hours and the sky does not turn from clear to
/// snowing between two of them. Asking more often would be asking the same
/// question again.
const ASK_AGAIN: Duration = Duration::from_secs(1200);

/// How soon it is asked again when there was no answer.
///
/// The commonest reason for no answer is that this started before the network
/// did, which is over in seconds. Waiting the full twenty minutes for that
/// would mean a machine that booted a little too quickly ignored the weather
/// for the rest of the morning.
const ASK_SOONER: Duration = Duration::from_secs(60);

/// How soon the picture is put up again when it did not take.
///
/// The wallpaper daemon is started a moment before this is, and one told
/// before it is ready takes the request, answers cleanly and draws nothing.
/// The loop then had nothing to bring it round for five minutes, so a fresh
/// session opened on a blank screen and stayed there until the weather
/// happened to answer and wake it.
const TRY_AGAIN: Duration = Duration::from_secs(2);

/// How long the wallpaper has to stay covered before the movement is put away.
///
/// Putting it away means handing the daemon a different file, and a daemon
/// handed a file starts it at its first frame. The picture rests for most of
/// its loop and stirs for a few seconds of it, so a menu opened and closed
/// while it was stirring threw the stir away and started the rest over: the
/// movement did not carry on where it had got to, it went back to the
/// beginning and waited.
///
/// Almost everything that covers the wallpaper here covers it for a moment. A
/// menu, a panel, the guide, the keyboard: opened, read and gone. Waiting this
/// long leaves all of those alone, and anything that stays, which is a window
/// somebody is working in, still puts the movement away.
const SETTLE: Duration = Duration::from_secs(15);

/// Why the loop woke up.
enum Woke {
    /// Something covered the wallpaper, or stopped covering it.
    Compositor,
    /// The weather answered, or would not.
    Weather(Option<Weather>),
}

fn main() -> ExitCode {
    let once = std::env::args().nth(1).is_some_and(|word| word == "--now");
    match run(once) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("{fault}");
            ExitCode::FAILURE
        }
    }
}

fn run(once: bool) -> Result<(), String> {
    let table = read_table()?;

    // Asked for on the way past rather than waited for. The picture that
    // answers an unknown sky is a picture, and one is on the screen before the
    // first answer comes back; the answer arriving is one of the three things
    // that wakes this loop, and the picture changes then if it should.
    //
    // `--now` has nothing after it to be woken by, so it waits for the answer
    // where it needs one, and it needs one only when the picture turns on it. A
    // picture somebody pinned is that picture in any weather, and asking anyway
    // put a web service with an eight second timeout between a press on the
    // wallpaper tab and the wallpaper: the commonest thing anybody does here
    // was the slowest, for an answer that was thrown away.
    let (say, woken) = channel();
    let mut sky_outside = match once {
        true => match choose::pinned(&table.pictures, &Wanted::asked()) {
            Some(_) => None,
            None => weather::now(&here::here()),
        },
        false => {
            listen(say.clone());
            ask_the_weather(say);
            None
        }
    };

    let mut showing: Option<PathBuf> = None;
    // When the wallpaper was first found covered, while it still is.
    let mut covered_since: Option<f64> = None;

    loop {
        // Five minutes unless something did not take, and then a moment.
        let mut waiting = LOOK_AGAIN;
        let now = SystemTime::now();
        let seconds = now
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        // Asked again every pass rather than held from the first one. The
        // machine can be carried somewhere else between two passes, and a
        // wallpaper still keeping the sun of the country it was set up in is
        // the one thing a person on a train would notice.
        let at = here::here();
        let here = Outside {
            moon: moon::moon(seconds),
            season: sun::season(&at, seconds),
            sky: sun::sky(&at, seconds),
            weather: sky_outside,
        };
        let wanted = Wanted::asked();

        covered_since = match covered::now() {
            true => covered_since.or(Some(seconds)),
            false => None,
        };
        // Covered, and for long enough to be worth interrupting the picture
        // for. Something that came and went inside the wait is something the
        // wallpaper never noticed.
        let put_away = covered_since.is_some_and(|since| seconds - since >= SETTLE.as_secs_f64());

        let chosen = choose::wanted(&table.pictures, &wanted, &here);
        if let Some((moving, still)) = chosen.and_then(|picture| place::picture(&picture.name)) {
            // The still is one frame that lasts for ever, so a covered
            // wallpaper is a daemon asleep rather than one drawing.
            let resting = put_away && still.is_file();
            let put_up = match resting {
                true => &still,
                false => &moving,
            };
            if showing.as_deref() != Some(put_up.as_path()) {
                // The still goes up first, and the movement over it. A moving
                // picture is decoded and compressed whole before any of it is
                // drawn, which is most of a minute the first time it is asked
                // for; the still is one frame of the same painting and is up in
                // the moment. So the wallpaper is the right picture from the
                // moment the desktop is there, and it starts moving when it can.
                let first = !resting && showing.as_deref() != Some(still.as_path());
                if first && still.is_file() && paint(&still) {
                    showing = Some(still.clone());
                }
                if !resting {
                    place::freshen(&moving);
                }
                match paint(put_up) {
                    true => showing = Some(put_up.clone()),
                    false => waiting = TRY_AGAIN,
                }
            }
        }

        // A cover that has not lasted long enough yet is the one thing here
        // that is waited out rather than waited for: nothing else is going to
        // wake this loop when it does.
        if let Some(since) = covered_since.filter(|_| !put_away) {
            let left = SETTLE.as_secs_f64() - (seconds - since);
            waiting = waiting.min(Duration::from_secs_f64(left.max(0.5)));
        }

        if once {
            return Ok(());
        }
        // A compositor event and a timeout mean the same thing here, which is
        // "look again", so neither is told apart from the other. The weather
        // is the one wake that carries something with it.
        match woken.recv_timeout(waiting) {
            Ok(Woke::Weather(said)) => sky_outside = said,
            Ok(Woke::Compositor) | Err(RecvTimeoutError::Timeout) => (),
            // Both threads are gone, which leaves the timeout. A wallpaper that
            // follows the sun and stops noticing windows is worth more than one
            // that stops.
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(LOOK_AGAIN),
        }
    }
}

/// The weather, asked for off the loop.
///
/// A fetch is a network request with an eight second timeout, and the loop it
/// used to sit at the top of is the loop that answers a window opening. So it
/// is asked on a thread of its own, which says what it found and wakes the
/// loop, and the first picture is on the screen before the first answer is.
fn ask_the_weather(say: Sender<Woke>) {
    std::thread::spawn(move || {
        loop {
            let said = weather::now(&here::here());
            let again = match said.is_some() {
                true => ASK_AGAIN,
                false => ASK_SOONER,
            };
            if say.send(Woke::Weather(said)).is_err() {
                return;
            }
            std::thread::sleep(again);
        }
    });
}

/// The table, out of the tree that holds it.
fn read_table() -> Result<Set, String> {
    let at = place::table();
    let held = std::fs::read_to_string(&at)
        .map_err(|fault| format!("{} could not be read: {fault}", at.display()))?;
    toml::from_str(&held).map_err(|fault| format!("{} does not parse: {fault}", at.display()))
}

/// Tell the wallpaper daemon, and say whether the picture went up.
///
/// A daemon that is not listening yet is one this tells again, and the loop
/// comes round in a moment rather than in five minutes to do it.
fn paint(picture: &Path) -> bool {
    let told = Command::new("awww")
        .arg("img")
        .arg(picture)
        .args(["--resize", "crop", "--transition-type", "none"])
        .output();
    match told {
        Ok(done) if done.status.success() => up(picture),
        Ok(done) => {
            eprintln!(
                "the wallpaper would not take {}: {}",
                picture.display(),
                String::from_utf8_lossy(&done.stderr).trim()
            );
            false
        }
        Err(fault) => {
            eprintln!("the wallpaper daemon could not be told: {fault}");
            false
        }
    }
}

/// Whether the daemon is showing what it was just told to show.
///
/// Asked rather than taken on trust. A daemon still finding its feet accepts
/// the picture, exits nothing but zero and draws none of it, which is a blank
/// screen that no exit code mentions and nothing else would notice.
fn up(picture: &Path) -> bool {
    let Some(name) = picture.to_str() else {
        return false;
    };
    let Ok(said) = Command::new("awww").arg("query").output() else {
        return false;
    };
    String::from_utf8_lossy(&said.stdout).contains(name)
}

/// Everything the compositor says that changes whether the wallpaper is seen.
///
/// Off on its own thread, because the loop wants to sleep on a timeout as well
/// as on this and a socket cannot be read with one. A compositor that never
/// answers leaves the loop running on its timeout alone, which is a wallpaper
/// that follows the sun and stops noticing windows, and is worth more than a
/// program that refuses to start.
fn listen(say: Sender<Woke>) {
    std::thread::spawn(move || {
        // Said on the way out. Carrying on without this is deliberate, and
        // what it leaves is a wallpaper that follows the sun and never notices
        // a window again, which from the outside is indistinguishable from one
        // that is working.
        let Some(socket) = covered::events() else {
            eprintln!("there is no compositor socket to listen to: windows will not be noticed");
            return;
        };
        let Ok(stream) = UnixStream::connect(&socket) else {
            eprintln!(
                "{} would not open: windows will not be noticed",
                socket.display()
            );
            return;
        };
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if covered::worth_waking_for(&line) && say.send(Woke::Compositor).is_err() {
                return;
            }
        }
    });
}
