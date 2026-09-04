//! How long the machine kept somebody waiting, written down where it adds up.
//!
//! `console_manifest::went` measures an apply, which is one program, on demand,
//! with the answer on stderr for whoever typed the variable. This is the other
//! half of the question and it is asked by nobody: what a person waits for on
//! this device is a menu that takes a moment to appear, and nobody is standing
//! at a terminal with a stopwatch when it does. So it is written down as it
//! happens, on the machine it happens on, and read afterwards.
//!
//! One line per thing waited for. Not one line per stamp with an id tying them
//! together: the id would exist only to put back what the writing took apart,
//! and every question anybody has -- how long does the menu take, is it worse
//! than last week, which stretch is the slow one -- would need the pieces
//! joined before it could be asked. A line is one opening, its stretches are
//! its fields, and they add up to what the line says was waited for.
//!
//! ## What one looks like
//!
//! ```text
//! {"at":1756761123,"up":67932.4,"load":0.31,"who":"launcher","what":"opening",
//!  "waited":412.3,"press":11.4,"exec":3.1,"screen":21.0,"gtk":128.4,
//!  "built":9.2,"placed":96.1,"frame":143.1,"with":{"rows":73,"door":"menu"}}
//! ```
//!
//! `waited` is what a thumb waited, and the stretches after it are where that
//! went, in the order they happened. Every number at the top of a line is
//! milliseconds, so anything reading this can add the whole of it up without
//! being told which fields are time. What the wait was *about* -- how many rows
//! were drawn, which door it came out of -- is under `with`, where it cannot be
//! mistaken for a stretch of the wait.
//!
//! ## Where it goes, and why there
//!
//! `~/.local/state/console/waited.jsonl`, beside the tab a panel was left on
//! and not under `~/.cache`. A cache is the machine's own answer to a question
//! anybody can ask it again; this is the only record that the menu took four
//! hundred milliseconds at half past nine, and clearing it does not cost one
//! opening, it costs the week.
//!
//! Appended rather than held open: every panel, the bar and the daemon write to
//! the same file from processes that know nothing about each other, and a
//! single `write` of one short line to a descriptor opened `O_APPEND` lands
//! whole. So there is no lock here, and nothing to hand over.
//!
//! ## Always on
//!
//! Behind no variable. Timings that have to be asked for are timings nobody has
//! when they want them: the openings worth reading about are the ones that
//! happened while somebody was using the device, and by the time it is slow
//! enough to complain about, the run that was slow is over. It costs a handful
//! of `Instant`s and one line written at the end of something that already
//! drew a window.

pub mod line;
pub mod summary;

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Where the press was stamped, and what it was stamped with.
///
/// The daemon that reads the pad knows when the button went down; the panel
/// that draws knows when it drew. Between them are a fork, an exec and a
/// loader, and every one of those is time a thumb waited for. So the daemon
/// puts the moment in the child's environment, on the clock that does not jump
/// -- `CLOCK_MONOTONIC`, in nanoseconds -- and the child stamps against it.
///
/// Not set, and a program times itself from its own start, which is honest
/// about everything except the part it cannot see.
pub const PRESSED: &str = "CONSOLE_PRESSED";

/// The shortest wait anybody could have noticed.
///
/// One frame at sixty. Something that answered inside it was on the screen in
/// the same picture as the press that asked for it, which is the machine being
/// instant rather than the machine being quick.
pub const FELT: Duration = Duration::from_millis(16);

/// How big the store is let get before the old half is set aside.
///
/// One line is about two hundred bytes and a busy day is a few hundred lines,
/// so this is months. What it is really for is the machine nobody looks at for
/// a year: a file that grows without end on a handheld is a fault waiting in
/// the one place that is hardest to notice.
pub const CAP: u64 = 1 << 20;

/// What the session says a name is, or nothing where it says nothing.
///
/// Unset is ordinary and every caller here has a default for it. A name that is
/// set to something which is not text is not ordinary -- somebody's state
/// directory is not where they believe it is -- and it used to arrive here as
/// the same silence as unset, which is how a store ends up in a second place
/// nobody goes looking.
fn asked(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(said) => Some(said),
        Err(std::env::VarError::NotPresent) => None,
        Err(fault) => {
            eprintln!("console-timings: {name}: {fault}");
            None
        }
    }
}

/// The store, under whatever this session calls its state.
pub fn where_() -> PathBuf {
    let state = match asked("XDG_STATE_HOME").filter(|said| !said.is_empty()) {
        Some(state) => PathBuf::from(state),
        None => {
            let home = asked("HOME").unwrap_or_else(|| "/root".to_string());

            PathBuf::from(home).join(".local/state")
        },
    };

    state.join("console").join("waited.jsonl")
}

/// One thing somebody is waiting for, being timed.
///
/// Made when the waiting starts, marked as each stretch of it ends, and written
/// once when whatever was waited for has happened. Dropped without `done`, it
/// writes nothing: a panel that was killed before it drew is not an opening
/// that took forever, it is an opening that never finished, and a line saying
/// otherwise would be the worst number in the file every time.
pub struct Waiting {
    who: String,
    what: String,
    started: Instant,
    last: Instant,
    marks: Vec<(String, Duration)>,
    notes: Vec<(String, line::Said)>,
    /// What had already gone by the time this was made: the press, and the
    /// loader. Counted into `waited` and written as stretches of their own.
    before: Duration,
}

impl Waiting {
    /// Start timing something, from as far back as this process can see.
    ///
    /// Which is the press, where the daemon stamped one, and this program's own
    /// exec where it did not. A menu timed from `main` says the machine is fast
    /// and the thumb says otherwise, and both are telling the truth about
    /// different things.
    pub fn on(who: &str, what: &str) -> Self {
        let now = Instant::now();
        let exec = since_exec().unwrap_or_default();
        let press = since_press().map_or(Duration::ZERO, |waited| waited.saturating_sub(exec));
        Waiting {
            who: who.to_string(),
            what: what.to_string(),
            started: now,
            last: now,
            marks: vec![("press".to_string(), press), ("exec".to_string(), exec)],
            notes: Vec::new(),
            before: press + exec,
        }
    }

    /// The same, for something that did not begin with a press.
    ///
    /// A search that was typed, a wallpaper that was chosen: the waiting starts
    /// where the program says it does, and there is nothing before it to
    /// account for.
    pub fn here(who: &str, what: &str) -> Self {
        let now = Instant::now();
        Waiting {
            who: who.to_string(),
            what: what.to_string(),
            started: now,
            last: now,
            marks: Vec::new(),
            notes: Vec::new(),
            before: Duration::ZERO,
        }
    }

    /// The stretch that ends here, and what it was.
    ///
    /// Named for what was done rather than for what comes next, so a line reads
    /// as an account of where the time went and not as a list of places it
    /// passed through.
    pub fn mark(&mut self, doing: &str) {
        let now = Instant::now();
        self.marks.push((doing.to_string(), now.saturating_duration_since(self.last)));
        self.last = now;
    }

    /// A stretch that happened before this waiting was made, taken out of the
    /// one that swallowed it.
    ///
    /// A panel waits for the screen before it draws anything, in a function
    /// that runs before there is a panel to hold a stopwatch. That wait is real
    /// and it is already inside `exec`, which is measured from the kernel's own
    /// account of when this process began -- so naming it here moves it out of
    /// `exec` rather than adding to the total, and the line still adds up.
    ///
    /// Where there is no `exec` to take it out of, it is time nobody had
    /// counted yet, and it is added.
    pub fn taking(&mut self, doing: &str, took: Duration) {
        let exec = self.marks.iter_mut().find(|(name, _)| name == "exec");

        match exec {
            Some((_, was)) => *was = was.saturating_sub(took),
            None => self.before += took,
        }

        let after = self.marks.iter().position(|(name, _)| name == "exec").map_or(0, |at| at + 1);
        self.marks.insert(after, (doing.to_string(), took));
    }

    /// Something worth knowing beside the numbers, counted.
    ///
    /// How many rows were drawn is the difference between a menu that is slow
    /// and a machine with two hundred applications on it, and no stretch can
    /// say which of those it was.
    pub fn counted(&mut self, name: &str, many: u64) {
        self.notes.push((name.to_string(), line::Said::Count(many)));
    }

    /// The same, in words: which tab, which door, which folder.
    pub fn named(&mut self, name: &str, said: &str) {
        self.notes.push((name.to_string(), line::Said::Word(said.to_string())));
    }

    /// How long it has taken so far.
    pub fn so_far(&self) -> Duration {
        self.before + self.started.elapsed()
    }

    /// Write it down, if there was anything to wait for.
    ///
    /// For the readings that happen constantly and are mostly instant: every
    /// letter typed into the menu reads the list again, and a store with a line
    /// per keystroke is a store where the openings are hard to find. Anything
    /// that came back inside a frame was not waited for by anybody.
    pub fn done_if_felt(self) {
        if self.so_far() >= FELT {
            self.done();
        }
    }

    /// The wait is over. Write it down.
    ///
    /// Nothing here is worth a failure: a store that could not be written is a
    /// question nobody can answer later, and a panel that refused to open
    /// because of it would be this crate costing the thing it measures.
    pub fn done(self) {
        let waited = self.before + self.started.elapsed();

        // Nought where the machine would not say what it had been doing. That
        // is a worse number than a real one and a better one than no line at
        // all, and the reason it is nought is in the journal rather than
        // nowhere.
        let load = match load() {
            Ok(load) => load,
            Err(fault) => {
                eprintln!("console-timings: {fault}");
                0.0
            }
        };

        let entry = line::Entry {
            at: unix_now(),
            up: uptime().unwrap_or_default().as_secs_f64(),
            load,
            who: self.who,
            what: self.what,
            waited,
            marks: self.marks,
            notes: self.notes,
        };
        write(&line::written(&entry));
    }
}

/// Put one line in the store, and say nothing if it cannot be.
fn write(said: &str) {
    let at = where_();

    if let Some(above) = at.parent() {
        let _ = std::fs::create_dir_all(above);
    }

    set_aside(&at);
    let opened = OpenOptions::new().create(true).append(true).open(&at);

    if let Ok(mut handle) = opened {
        let _ = handle.write_all(format!("{said}\n").as_bytes());
    }
}

/// The old half of a full store, moved out of the way.
///
/// Renamed rather than trimmed. A file being rewritten is a file that another
/// panel is appending to at the same moment, and the whole reason there is no
/// lock here is that appending needs none. A rename needs none either, and what
/// it costs is one file's worth of history kept beside the live one.
fn set_aside(at: &std::path::Path) {
    let big = at.metadata().is_ok_and(|about| about.len() >= CAP);

    if big {
        let _ = std::fs::rename(at, at.with_extension("jsonl.old"));
    }
}

/// Seconds since the epoch, for saying when.
fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

/// How long this machine has been up.
///
/// Written beside every line because the first opening after a boot is not the
/// same measurement as the tenth: nothing is in the page cache, no font has
/// been looked at, and the icon theme has never been read. A store that could
/// not tell those apart would have one very slow outlier per session and no way
/// to say why.
pub fn uptime() -> Option<Duration> {
    let said = match std::fs::read_to_string("/proc/uptime") {
        Ok(s) => s,
        Err(_) => return None,
    };
    let first = said.split_whitespace().next()?;
    let seconds: f64 = match first.parse() {
        Ok(v) => v,
        Err(_) => return None,
    };
    Some(Duration::from_secs_f64(seconds))
}

/// What else the machine was doing, over the last minute.
///
/// Written beside every line because the worst numbers in this store were made
/// by a device compiling its own desktop while somebody pressed the menu, and
/// a store that cannot tell those from the menu being slow is a store that
/// makes people chase the wrong thing. One minute rather than one second: what
/// is wanted is whether the machine was busy around then, and the shortest
/// average is noisy enough to say no about a machine that was.
pub fn load() -> Result<f64, String> {
    let said = std::fs::read_to_string("/proc/loadavg")
        .map_err(|fault| format!("/proc/loadavg: {fault}"))?;

    let Some(first) = said.split_whitespace().next() else {
        return Err("/proc/loadavg: it said nothing at all".to_string());
    };

    first.parse().map_err(|fault| format!("/proc/loadavg: {first:?}: {fault}"))
}

/// How old this process is: the fork, the exec and the loader, before `main`.
///
/// Worked out from what the kernel says rather than measured, because the part
/// being measured is over before there is anything of ours running to measure
/// it. `/proc/self/stat` says when this process started, in ticks since the
/// machine did, and the boot clock says how long ago that was.
///
/// Good to a tick, which on this machine is ten milliseconds -- so a line
/// saying the exec took forty had one that took between thirty and fifty. The
/// clock is read with `clock_gettime` rather than out of `/proc/uptime`, which
/// is rounded to a tick of its own and would have doubled that. Where a press
/// was stamped it matters less than it looks: the press and the exec together
/// are measured to the nanosecond, and the tick only decides where the line
/// between them falls.
pub fn since_exec() -> Option<Duration> {
    let said = match std::fs::read_to_string("/proc/self/stat") {
        Ok(s) => s,
        Err(_) => return None,
    };
    let started = started_at(&said)?;
    // SAFETY: one call into libc that reads a constant and touches nothing.
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };

    if ticks <= 0 {
        return None;
    }

    // A clock that ticks more than two thousand million times a second is not
    // a machine this runs on, but the conversion is written rather than
    // assumed, so that if one ever exists it is a line in the journal and not
    // a number quietly wrong by a factor of nothing anybody could name.
    let ticks_f = match i32::try_from(ticks) {
        Ok(ticks) => f64::from(ticks),
        Err(fault) => {
            eprintln!("console-timings: the clock ticks {ticks} times a second: {fault}");
            return None;
        }
    };

    let began = Duration::from_secs_f64(started / ticks_f);
    since_boot()?.checked_sub(began)
}

/// How long the machine has been up, to the nanosecond.
///
/// The same clock `/proc/uptime` is written from, asked directly. It counts
/// through a suspend, which `CLOCK_MONOTONIC` does not, and a handheld is a
/// machine that spends most of its life suspended.
fn since_boot() -> Option<Duration> {
    let mut when = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: the struct is ours, initialised, and lives across the call.
    let asked = unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut when) };

    if asked != 0 {
        return None;
    }

    // A clock that answers with a time before the epoch is a clock that has
    // not been read, whatever it returned.
    let (Ok(seconds), Ok(nanoseconds)) =
        (u64::try_from(when.tv_sec), u32::try_from(when.tv_nsec))
    else {
        eprintln!("console-timings: the boot clock said {}s {}ns", when.tv_sec, when.tv_nsec);

        return None;
    };

    Some(Duration::new(seconds, nanoseconds))
}

/// The twenty-second field of `/proc/self/stat`, which is when this began.
///
/// Counted from the last `)` rather than from the front, because the second
/// field is the program's own name in brackets and a program is free to be
/// called `my (program) name`. Nothing on this device is, and the file is read
/// by every panel on every opening, which is exactly the kind of place a
/// once-a-year fault likes to live.
pub fn started_at(stat: &str) -> Option<f64> {
    let (_, after) = stat.rsplit_once(')')?;
    // The fields after the name start at the third, so the twenty-second is
    // the twentieth of what is left.
    let field = after.split_whitespace().nth(19)?;

    match field.parse() {
        Ok(started) => Some(started),
        // The field is there and is not a number, so this is not the file it is
        // being read as. Everything downstream of it is a timing that silently
        // does not get written, which is exactly the fault that hides longest.
        Err(fault) => {
            eprintln!("console-timings: /proc/self/stat: field 22 is {field:?}: {fault}");
            None
        }
    }
}

/// How long ago the press was, if something stamped one.
fn since_press() -> Option<Duration> {
    let raw = match std::env::var(PRESSED) {
        Ok(s) => s,
        Err(_) => return None,
    };
    let stamped: u64 = match raw.trim().parse() {
        Ok(v) => v,
        Err(_) => return None,
    };
    let now = monotonic_now()?;
    now.checked_sub(Duration::from_nanos(stamped))
}

/// The clock a press is stamped on: the one that does not jump.
///
/// `Instant` is this clock and cannot be built from a number, which is the
/// whole reason this is here: the stamp crosses a process boundary as a word in
/// the environment, and something has to read it back.
pub fn monotonic_now() -> Option<Duration> {
    let mut when = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: the struct is ours, initialised, and lives across the call.
    let asked = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut when) };

    if asked != 0 {
        return None;
    }

    // As above: a monotonic clock reading before the epoch has not been read.
    let (Ok(seconds), Ok(nanoseconds)) =
        (u64::try_from(when.tv_sec), u32::try_from(when.tv_nsec))
    else {
        eprintln!("console-timings: the monotonic clock said {}s {}ns", when.tv_sec, when.tv_nsec);

        return None;
    };

    Some(Duration::new(seconds, nanoseconds))
}

/// What to put in a child's environment so it can time from the press.
///
/// A pair rather than a call that sets it, because the daemon hands it to one
/// `Command` and setting it on itself would stamp every child it ever starts
/// with the moment the daemon began.
pub fn press_stamp() -> Option<(&'static str, String)> {
    Some((PRESSED, monotonic_now()?.as_nanos().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The name in the middle of the line is a program's own and can hold
    /// anything, brackets and spaces included, so the field after it is counted
    /// from the end of the name and not from the start of the line.
    #[test]
    fn when_a_process_began_is_read_past_a_name_with_spaces_in_it() {
        // The state is the third field and the start is the twenty-second, so
        // the line is the name and then those twenty, each written as its own
        // number.
        let fields: Vec<String> = (3..=22).map(|n| n.to_string()).collect();
        let stat = format!("1234 (a (odd) name) {}", fields.join(" "));
        assert_eq!(started_at(&stat), Some(22.0));
    }

    #[test]
    fn a_line_that_is_not_a_stat_says_nothing() {
        assert_eq!(started_at(""), None);
        assert_eq!(started_at("1234 (launcher) S 4 5"), None);
    }

    /// This machine's own clocks, asked once, because a store stamped with
    /// nothing is a store nobody can sort.
    #[test]
    fn the_machine_says_how_long_it_has_been_up_and_how_old_this_is() {
        assert!(uptime().is_some_and(|up| up > Duration::ZERO));
        assert!(since_exec().is_some());
        assert!(monotonic_now().is_some());
    }

    /// The press is the daemon's clock read in the panel's process, so the two
    /// have to be the same clock. Written and read back here, which is the same
    /// journey the environment makes.
    #[test]
    fn a_press_stamp_reads_back_as_a_moment_that_has_already_gone() {
        let (word, stamped) = press_stamp().expect("this machine has a monotonic clock");
        assert_eq!(word, PRESSED);
        let then: u64 = stamped.parse().expect("nanoseconds");
        let now = monotonic_now().expect("a monotonic clock").as_nanos() as u64;
        assert!(now >= then, "the clock went backwards between two reads");
    }

    /// The wait for the screen happens before there is a panel to time it, so
    /// it arrives already counted inside the exec. Naming it moves it; it does
    /// not count it twice.
    #[test]
    fn a_stretch_that_was_already_inside_the_exec_is_moved_out_of_it() {
        // Built rather than started, so what the exec was is known: a real one
        // is however long this test binary happened to take to load, which is
        // not a number an assertion can be written against.
        let now = Instant::now();
        let mut waiting = Waiting {
            who: "a test".to_string(),
            what: "opening".to_string(),
            started: now,
            last: now,
            marks: vec![
                ("press".to_string(), Duration::from_millis(10)),
                ("exec".to_string(), Duration::from_millis(100)),
            ],
            notes: Vec::new(),
            before: Duration::from_millis(110),
        };
        let whole = waiting.before;
        waiting.taking("screen", Duration::from_millis(20));
        let named: Vec<&str> = waiting.marks.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(named, ["press", "exec", "screen"]);
        assert_eq!(waiting.before, whole, "the total changed");
        assert_eq!(
            waiting.marks.iter().map(|(_, took)| *took).sum::<Duration>(),
            whole,
            "the stretches stopped adding up to the total"
        );
    }

    /// And where nothing had counted it, it is time added rather than moved.
    #[test]
    fn a_stretch_before_a_waiting_that_counts_nothing_before_it_is_added() {
        let mut waiting = Waiting::here("a test", "opening");
        waiting.taking("looking", Duration::from_millis(5));
        assert_eq!(waiting.before, Duration::from_millis(5));
    }

    /// A wait that was never finished is not a slow one. The panel that was
    /// killed on the way up used to be the worst number in every summary.
    #[test]
    fn a_waiting_that_is_dropped_writes_nothing() {
        let store = where_();
        let before = store.metadata().map(|about| about.len()).unwrap_or(0);
        {
            let mut waiting = Waiting::here("a test", "nothing");
            waiting.mark("thinking about it");
        }
        let after = store.metadata().map(|about| about.len()).unwrap_or(0);
        assert_eq!(before, after, "a dropped waiting wrote a line");
    }
}
