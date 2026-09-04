//! The processors, asked to hurry for as long as somebody is waiting.
//!
//! This is a handheld, and most of the time it is right for it to be slow. It
//! sits in a bag at its lowest clock and the battery lasts the day. But a
//! panel is a moment's work and then nothing at all, and a processor deciding
//! how fast to run by watching how busy it has been is always deciding about
//! the wrong moment: the whole of an opening is over before the load it made
//! has been noticed.
//!
//! What that costs is in `console-timings`, which reads what every opening on
//! this machine wrote down about itself, and it is not one slow stretch. It is
//! every stretch: the loader, GTK coming up, the card being built, the rows
//! going on it, the first frame. Nothing there is slow. All of it is being
//! done at a fraction of the clock the machine can run at, because nothing
//! asked it for more and by the time anything could have, the press was
//! answered.
//!
//! So the daemon that reads the pad says so as it starts something: hurry, for
//! about as long as an opening takes, and then let it be. Run
//! `console-timings` before and after to see what it is worth on the machine
//! in your hands. What it costs is a moment of ordinary clock speed per press,
//! on a device that is otherwise asleep between them.
//!
//! ## The knob, and why this one
//!
//! `amd-pstate-epp` picks the frequency in hardware, from how busy a core has
//! been and from one hint: `energy_performance_preference`, a word per core
//! under `/sys/devices/system/cpu`. There is no per-task version of it. The
//! scheduler's own `uclamp` drives `schedutil`, and this machine is not on
//! `schedutil`, so a hint about the task that is opening the panel is not a
//! thing this kernel can be given. The hint is the machine's, or it is
//! nothing.
//!
//! Which is why what is written is put back. `power-profiles-daemon` owns this
//! file -- power-saver writes `power` into it, balanced writes
//! `balance_performance` -- and a desktop that raised it and walked away would
//! be a machine quietly ignoring the profile somebody chose, for ever, with
//! nothing on any screen saying so. So the word that was there is read before
//! it is changed and written back when the moment is over, and what the
//! profile says is what the machine does between presses.
//!
//! `balance_performance` rather than `performance`: measured, they were the
//! same opening to within noise, and the gentler of two words that do the same
//! thing is the one to write into somebody's power settings.
//!
//! ## What a daemon that dies owes the machine
//!
//! The word each core held is the one thing here that cannot be worked out
//! again. It is read off the hint, the hint is then overwritten, and from that
//! moment the only copy of it is in this process. A daemon that does not reach
//! `settle` -- a crash, the target stopping, an apply restarting it, a battery
//! that ran out -- takes that copy with it, and every core is left at
//! `balance_performance` for as long as the machine stays up. That is the
//! machine quietly ignoring the profile somebody chose, which is the outcome
//! the paragraph above says this exists to prevent, arriving by the one road
//! that paragraph did not watch.
//!
//! It could not heal, either, and that is the worse half. `asked` steps over a
//! hint that already reads `balance_performance`, because a machine whose
//! profile genuinely asks for that word is one this must leave alone. After a
//! run that died, every core reads exactly that, so every core is stepped over,
//! nothing is recorded, `settle` has nothing to put back, and the daemon that
//! is running now cannot tell the wreckage of the last one from a setting it
//! must not touch. Nothing short of a reboot got it back.
//!
//! So the words are written down before the hints are changed, in the runtime
//! directory, and the note is removed when they go back. A daemon starting up
//! reads it: a note that is there is a run that did not finish, and putting its
//! words back is the first thing this does. The runtime directory is the right
//! place for it because a reboot empties it, and a reboot is the one event
//! after which the words in it are worthless -- the kernel and
//! `power-profiles-daemon` decide the hint again from nothing.
//!
//! ## Nothing here fails
//!
//! The files belong to root and are handed to this desktop's user by
//! `/etc/udev/rules.d/93-console-cpufreq.rules`. A machine where that has not
//! been applied yet is a machine where every write here is refused, which is
//! the desktop exactly as it was before any of this: slower, and working. It
//! says so once and goes on.

use console_writing::{Held, read};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Where the processors are, and where each one keeps its hint.
pub const CPUS: &str = "/sys/devices/system/cpu";

/// The hint, under one processor.
pub const HINT: &str = "cpufreq/energy_performance_preference";

/// What is written while a press is being answered.
pub const HURRY: &str = "balance_performance";

/// How long a press is given before the processors are let be.
///
/// Comfortably longer than the slowest openings `console-timings` reports and
/// shorter than the gap between two deliberate presses. It is not a budget for
/// the panel: a panel that takes longer finishes at whatever clock the machine
/// has settled back to, the way all of them did before this. It is the point
/// past which whatever is being waited for is no longer this press.
pub const FOR: Duration = Duration::from_millis(750);

/// The processors, and whether they have been asked to hurry.
///
/// Held by the daemon across its turns, because the asking and the letting go
/// are two different moments and only one of them is a press. Nothing here
/// runs on a clock of its own: it is told the time by whoever holds it, which
/// is what lets the whole of it be asked the same question twice in a test.
pub struct Hurrying {
    cpus: PathBuf,
    /// Where the words are written down while the hints are changed.
    ///
    /// The copy that outlives this process. Everything in `was` is also here
    /// from the moment before the first hint is overwritten until the moment
    /// after the last one goes back.
    note: PathBuf,
    /// When the hurry is over, if one is on.
    until: Option<Instant>,
    /// The word each hint held before it was changed, to be put back.
    was: Vec<(PathBuf, String)>,
    /// Whether the one complaint has been made.
    said: bool,
}

impl Default for Hurrying {
    fn default() -> Self {
        Hurrying::of(Path::new(CPUS))
    }
}

/// Whether the processors are being hurried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hurry {
    /// A hurry is on, and something is holding the governor up.
    On,
    /// Nothing is, and the machine is running at whatever it settles to.
    Off,
}

impl Hurrying {
    /// The processors, wherever they are. A path so a test can have some.
    ///
    /// Whatever the last run left written down goes back here, before anything
    /// else happens. A note that is there is a run that did not reach `settle`,
    /// and this is the only moment anybody can tell that from a machine whose
    /// profile asks for the word that is in the file.
    pub fn of(cpus: &Path) -> Self {
        let mut hurrying = Hurrying::noting(cpus, &note());
        hurrying.put_back_what_was_left();
        hurrying
    }

    /// The same, with the note somewhere a test can look at it.
    ///
    /// Nothing is put back here. A test that wants the starting-up half asks
    /// for it by name, and every other test gets a daemon that has not been
    /// handed the leavings of a run it knows nothing about.
    pub fn noting(cpus: &Path, note: &Path) -> Self {
        Hurrying {
            cpus: cpus.to_path_buf(),
            note: note.to_path_buf(),
            until: None,
            was: Vec::new(),
            said: false,
        }
    }

    /// Put back whatever a run that did not finish left written down.
    ///
    /// The note is removed either way. A note nobody could read is one this
    /// will not be able to read next time either, and leaving it there would
    /// make every start of this daemon complain about the same file for ever.
    /// What it said is on the way out, which is where a fault belongs.
    pub fn put_back_what_was_left(&mut self) -> Left {
        let held = match read(&self.note) {
            Held::Nothing => return Left::Nothing,
            Held::Said(held) => held,
            // Unusable, and it will be just as unusable at the next start. What
            // it was holding is the only copy of some words that are now lost,
            // so this is said as loudly as a library can say anything and the
            // file is taken away rather than left to say it again every boot.
            Held::Unreadable(fault) => {
                eprintln!(
                    "console-haste: {} is what says which words the processors are holding, and                      it will not be read: {fault}. They may be left at {HURRY}; a reboot is what                      puts them back.",
                    self.note.display()
                );
                forget(&self.note);
                return Left::Nothing;
            }
        };

        let (words, torn) = words_in(&held);

        for (hint, was) in &words {
            if let Err(fault) = std::fs::write(hint, was) {
                eprintln!(
                    "console-haste: {} was left hurried by a run that did not finish and will                      not take {was} back: {fault}",
                    hint.display()
                );
            }
        }

        for line in &torn {
            eprintln!("console-haste: {} holds a line this cannot read: {line}", self.note.display());
        }

        forget(&self.note);

        match words.is_empty() {
            true => Left::Nothing,
            false => Left::PutBack,
        }
    }

    /// Whether the processors are being hurried just now.
    pub fn on(&self) -> Hurry {
        match self.until.is_some() {
            true => Hurry::On,
            false => Hurry::Off,
        }
    }

    /// Somebody pressed something. Hurry.
    ///
    /// Asked again while a hurry is already on -- a second press, a button held
    /// down stepping a scale -- and the hint is not written twice. Only the
    /// moment it ends moves, so a run of presses is one hurry lasting through
    /// them rather than one per press with the machine dropping back between.
    pub fn asked(&mut self, now: Instant) {
        let ending = now + FOR;

        if self.until.is_some() {
            self.until = Some(ending);
            return;
        }

        // Read first, write second, and the note between them. Every word
        // this is about to overwrite is known before any of them is
        // overwritten, so the note covers the whole set rather than the part
        // of it a loop had reached when the machine stopped.
        let mut taking: Vec<(PathBuf, String)> = Vec::new();
        let mut unreadable: Option<PathBuf> = None;

        for hint in self.hints() {
            match read(&hint) {
                Held::Said(was) => {
                    let was = was.trim().to_string();

                    if was != HURRY {
                        taking.push((hint, was));
                    }
                }
                // A hint that was there a moment ago when `hints` filtered on
                // it and is gone now is a core that went offline between the
                // two, which is a thing this machine does and not a fault.
                Held::Nothing => {}
                Held::Unreadable(_) => unreadable = Some(hint),
            }
        }

        if let Some(hint) = unreadable {
            self.complain(&hint);
        }

        if taking.is_empty() {
            return;
        }

        // Written down before a single hint is changed, and the hurry is
        // called off if it cannot be. A machine running at its ordinary clock
        // is this desktop exactly as it was before any of this -- slower, and
        // working -- and it is a great deal better than a machine held at
        // balance_performance with nothing anywhere recording that it is.
        if let Err(_fault) = wrote_note(&self.note, &taking) {
            self.complain_about_the_note();
            return;
        }

        let mut wrote = false;

        for (hint, was) in taking {
            if let Err(_fault) = std::fs::write(&hint, HURRY) {
                self.complain(&hint);
                continue;
            }

            self.was.push((hint, was));
            wrote = true;
        }

        if wrote {
            self.until = Some(ending);
        }
    }

    /// The moment is over. Put back what was there.
    ///
    /// Called every turn rather than scheduled, because the daemon has a turn
    /// every twenty milliseconds anyway and a timer would be a second clock to
    /// keep in step with the one it already runs on.
    pub fn settle(&mut self, now: Instant) {
        let Some(until) = self.until else { return };

        if now < until {
            return;
        }

        for (hint, was) in std::mem::take(&mut self.was) {
            if let Err(_fault) = std::fs::write(&hint, &was) {
                self.complain(&hint);
            }
        }

        // After the words are back, never before. The note is what says the
        // machine is still holding words that are not its own, and removing it
        // first would mean a stop between the two left a hurried machine with
        // nothing written down -- which is the whole fault, reintroduced at the
        // one moment it is hardest to see.
        forget(&self.note);
        self.until = None;
    }

    /// Every processor's hint, as the machine has them.
    ///
    /// Read each time rather than found once and kept: a core that was offline
    /// when this daemon started is a core with no hint to read then and one to
    /// write now, and a list made at the start would have missed it for the
    /// life of the session.
    fn hints(&self) -> Vec<PathBuf> {
        let Ok(reading) = std::fs::read_dir(&self.cpus) else { return Vec::new() };

        let mut hints: Vec<PathBuf> = reading
            .filter_map(Result::ok)
            .map(|one| one.path().join(HINT))
            .filter(|hint| hint.exists())
            .collect();
        hints.sort();
        hints
    }

    /// Once, and then never again.
    ///
    /// A daemon that printed a line every time a button was pressed on a
    /// machine where the rule has not been applied would fill the journal with
    /// the same sentence and hide everything else in it.
    fn complain(&mut self, hint: &Path) {
        if self.said {
            return;
        }

        self.said = true;
        eprintln!(
            "console-haste: {} will not take a word, so panels open at whatever \
             clock the machine happens to be at",
            hint.display()
        );
    }

    /// The same, for the note rather than for a hint.
    ///
    /// Its own sentence because it means something different. A hint that will
    /// not take a word is a machine that stays slow, which is the desktop as it
    /// was. A note that cannot be written is this refusing to hurry at all --
    /// on purpose, because hurrying without a record is the fault rather than
    /// the feature -- and the two should not read as the same complaint.
    fn complain_about_the_note(&mut self) {
        if self.said {
            return;
        }

        self.said = true;
        eprintln!(
            "console-haste: {} cannot be written, so the processors are left alone: hurrying \
             them without writing down what they held is how they get stuck at {HURRY}",
            self.note.display()
        );
    }
}

/// What a run that did not finish had left written down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Left {
    /// Nothing was, so the last run ended the way it meant to.
    Nothing,
    /// Words were, and they have gone back onto the processors.
    PutBack,
}

/// Where the words are written down, on this machine.
///
/// Under the runtime directory, which is this session's own and is emptied at
/// every boot. That is exactly the life the note should have: it is worth
/// something to the next daemon that starts inside this session and worth
/// nothing at all after a reboot, because a reboot is when the kernel and
/// `power-profiles-daemon` decide the hint again from nothing.
///
/// A machine with no runtime directory gets a path under `/tmp` instead. It is
/// the worse place -- `/tmp` outlives a boot on some machines, and a stale note
/// there would put yesterday's words back -- and it is better than no note,
/// which is the state this whole thing exists to leave behind.
pub fn note() -> PathBuf {
    let run = match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(run) => PathBuf::from(run),
        None => std::env::temp_dir(),
    };
    run.join("console").join("hurried")
}

/// Write the words down, whole, before any of them is overwritten.
///
/// A path can hold anything a path can hold and a word cannot hold a newline,
/// so the word goes first and the rest of the line is the path. Split the other
/// way round, a directory with a tab in its name would be read as a word.
fn wrote_note(at: &Path, words: &[(PathBuf, String)]) -> Result<(), String> {
    if let Some(holding) = at.parent() {
        std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{}: its directory: {fault}", at.display()))?;
    }

    let written: String = words
        .iter()
        .map(|(hint, was)| format!("{was}\t{}\n", hint.display()))
        .collect();

    console_writing::whole(at, written.as_bytes())
}

/// The words in a note, and the lines that are not words.
///
/// A torn line is handed back rather than skipped. A note is written whole by
/// one process and read by one process, so a line that does not parse means the
/// machine stopped in the middle of writing it -- which is a thing worth saying
/// out loud, and exactly the thing a filter would swallow.
fn words_in(held: &str) -> (Vec<(PathBuf, String)>, Vec<String>) {
    let mut words = Vec::new();
    let mut torn = Vec::new();

    for line in held.lines().filter(|line| !line.is_empty()) {
        match line.split_once('\t') {
            Some((was, hint)) => words.push((PathBuf::from(hint), was.to_string())),
            None => torn.push(line.to_string()),
        }
    }

    (words, torn)
}

/// Take the note away, the words being back where they came from.
fn forget(at: &Path) {
    match std::fs::remove_file(at) {
        Ok(()) => {}
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => {}
        // Left behind, this is put back again at every start of the daemon for
        // the rest of the session. Harmless each time and wrong every time, so
        // it is said rather than shrugged at.
        Err(fault) => eprintln!(
            "console-haste: {} will not go away: {fault}. The words in it go back at every start              until it does.",
            at.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Some processors, each holding the word a power-saver machine holds.
    fn processors(named: &str, cores: usize) -> PathBuf {
        let here = std::env::temp_dir().join(format!("console-haste-{named}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&here);
        for core in 0..cores {
            let at = here.join(format!("cpu{core}")).join("cpufreq");
            std::fs::create_dir_all(&at).expect("somewhere to keep a hint");
            std::fs::write(at.join("energy_performance_preference"), "power\n")
                .expect("a hint to write");
        }
        // The things beside the processors that are not processors, which is
        // what makes reading the directory a filter rather than a listing.
        std::fs::create_dir_all(here.join("cpufreq")).expect("the shared directory");
        std::fs::create_dir_all(here.join("cpuidle")).expect("the idle directory");
        here
    }

    /// A note of this test's own, beside its processors.
    ///
    /// Never the real one. `Hurrying::of` looks under the runtime directory,
    /// and a test that used it would put words onto the processors of the
    /// machine running the suite.
    fn a_note_of_our_own(at: &Path) -> PathBuf {
        at.join("hurried")
    }

    /// The daemon, pointed at a test's own processors and a test's own note.
    fn hurrying(at: &Path) -> Hurrying {
        Hurrying::noting(at, &a_note_of_our_own(at))
    }

    fn said(at: &Path, core: usize) -> String {
        let hint = at.join(format!("cpu{core}")).join(HINT);
        std::fs::read_to_string(hint).unwrap_or_default().trim().to_string()
    }

    /// The whole of it: a press hurries every processor, and the word each one
    /// had is what it has again once the moment has gone by.
    #[test]
    fn a_press_hurries_the_processors_and_the_moment_after_lets_them_be() {
        let at = processors("press", 4);
        let mut hurrying = hurrying(&at);
        let now = Instant::now();

        hurrying.asked(now);
        assert_eq!(hurrying.on(), Hurry::On);
        for core in 0..4 {
            assert_eq!(said(&at, core), HURRY, "cpu{core} was not hurried");
        }

        // Still inside the moment: nothing is put back yet.
        hurrying.settle(now + FOR / 2);
        assert_eq!(said(&at, 0), HURRY);

        hurrying.settle(now + FOR);
        assert_eq!(hurrying.on(), Hurry::Off);
        for core in 0..4 {
            assert_eq!(said(&at, core), "power", "cpu{core} was not let be");
        }
    }

    /// A button held down is one hurry that lasts, not one per step with the
    /// machine dropping back to the profile's word in between.
    #[test]
    fn asking_again_moves_the_end_rather_than_starting_a_second_one() {
        let at = processors("again", 2);
        let mut hurrying = hurrying(&at);
        let now = Instant::now();

        hurrying.asked(now);
        hurrying.asked(now + FOR / 2);
        // The first press's moment has gone by and the second's has not.
        hurrying.settle(now + FOR);
        assert_eq!(hurrying.on(), Hurry::On);
        assert_eq!(said(&at, 0), HURRY);

        hurrying.settle(now + FOR + FOR / 2);
        assert_eq!(hurrying.on(), Hurry::Off);
        assert_eq!(said(&at, 0), "power");
    }

    /// What is put back is what was there, and not a word written here. A
    /// machine on the balanced profile is already saying what this would say,
    /// and it must not be left on the power-saver's word for having been
    /// asked.
    #[test]
    fn what_goes_back_is_what_was_there() {
        let at = processors("kept", 1);
        let hint = at.join("cpu0").join(HINT);
        std::fs::write(&hint, "performance\n").expect("a hint to write");

        let mut hurrying = hurrying(&at);
        let now = Instant::now();
        hurrying.asked(now);
        hurrying.settle(now + FOR);
        assert_eq!(said(&at, 0), "performance");
    }

    /// A processor already saying it wants speed is left alone, so that a
    /// balanced machine is not written to sixteen times on every press and,
    /// more to the point, cannot have the word it already had put back wrong.
    #[test]
    fn a_processor_that_is_already_hurrying_is_not_written_to() {
        let at = processors("standing", 1);
        let hint = at.join("cpu0").join(HINT);
        std::fs::write(&hint, format!("{HURRY}\n")).expect("a hint to write");

        let mut hurrying = hurrying(&at);
        hurrying.asked(Instant::now());
        // Nothing was changed, so there is nothing to put back and no moment
        // to wait out.
        assert_eq!(hurrying.on(), Hurry::Off);
        assert_eq!(said(&at, 0), HURRY);
    }

    /// A machine with no such file is the desktop as it was: slower, working,
    /// and saying nothing on every press.
    #[test]
    fn processors_that_cannot_be_hurried_are_not_an_error() {
        let at = std::env::temp_dir().join(format!("console-haste-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        let mut hurrying = hurrying(&at);
        let now = Instant::now();
        hurrying.asked(now);
        assert_eq!(hurrying.on(), Hurry::Off);
        hurrying.settle(now + FOR);
    }

    // ------------------------------------------------- a run that did not end

    /// A daemon that dies inside the moment leaves the words on the machine.
    ///
    /// This is the state the note exists for, made on purpose: `asked` and then
    /// no `settle`, which is what a crash, a stopped target or a flat battery
    /// looks like from the outside.
    #[test]
    fn a_run_that_stops_mid_hurry_leaves_its_words_written_down() {
        let at = processors("stopped", 3);
        let mut dying = hurrying(&at);
        dying.asked(Instant::now());
        drop(dying);

        for core in 0..3 {
            assert_eq!(said(&at, core), HURRY, "cpu{core} was not hurried");
        }

        let note = a_note_of_our_own(&at);
        assert!(note.exists(), "nothing was written down, so nothing can put these back");
        let held = std::fs::read_to_string(&note).expect("the note");
        assert_eq!(held.lines().count(), 3, "the note does not cover every processor: {held:?}");
    }

    /// The whole of it: the next daemon puts back what the last one left.
    #[test]
    fn the_next_daemon_puts_back_what_a_stopped_one_left() {
        let at = processors("nextone", 3);
        let mut dying = hurrying(&at);
        dying.asked(Instant::now());
        drop(dying);

        let mut coming_up = hurrying(&at);
        assert_eq!(coming_up.put_back_what_was_left(), Left::PutBack);

        for core in 0..3 {
            assert_eq!(said(&at, core), "power", "cpu{core} is still hurried");
        }
        assert!(!a_note_of_our_own(&at).exists(), "the note outlived the words going back");
    }

    /// The fault as it actually was, held here so it cannot come back.
    ///
    /// Without the note there is nothing to tell the wreckage of a stopped run
    /// from a machine whose profile genuinely asks for this word: `asked` steps
    /// over every hint that already reads it, records nothing, and `settle` has
    /// nothing to put back. The processors stayed hurried until a reboot. This
    /// presses on exactly that sequence and asks that it comes out the other
    /// way.
    #[test]
    fn a_stopped_run_does_not_leave_the_processors_hurried_for_ever() {
        let at = processors("forever", 2);
        let mut dying = hurrying(&at);
        dying.asked(Instant::now());
        drop(dying);

        // The daemon that comes up next, doing what a daemon does: it is asked,
        // and the moment goes by.
        let mut after = Hurrying::noting(&at, &a_note_of_our_own(&at));
        after.put_back_what_was_left();
        let now = Instant::now();
        after.asked(now);
        after.settle(now + FOR);

        for core in 0..2 {
            assert_eq!(said(&at, core), "power", "cpu{core} never came back");
        }
    }

    /// A machine whose profile asks for this word is still left alone.
    ///
    /// The other side of the same rule, and the reason `asked` steps over a
    /// hint it agrees with. Putting a word back is only ever putting back a
    /// word this wrote over.
    #[test]
    fn a_machine_that_asks_for_the_hurried_word_is_not_written_down() {
        let at = processors("agrees", 2);
        for core in 0..2 {
            let hint = at.join(format!("cpu{core}")).join(HINT);
            std::fs::write(&hint, format!("{HURRY}\n")).expect("a hint to write");
        }

        let mut hurrying = hurrying(&at);
        hurrying.asked(Instant::now());

        assert_eq!(hurrying.on(), Hurry::Off, "a hurry was started with nothing to change");
        assert!(!a_note_of_our_own(&at).exists(), "a word nobody overwrote was written down");
    }

    /// The note goes when the words do, and not a moment before.
    #[test]
    fn settling_takes_the_note_away() {
        let at = processors("settled", 2);
        let mut hurrying = hurrying(&at);
        let now = Instant::now();

        hurrying.asked(now);
        assert!(a_note_of_our_own(&at).exists(), "nothing was written down during the hurry");

        hurrying.settle(now + FOR);
        assert!(!a_note_of_our_own(&at).exists(), "the note outlived the hurry");
    }

    /// A note that cannot be written means the processors are left alone.
    ///
    /// Hurrying without a record is the fault rather than the feature, so this
    /// declines to hurry at all. What that costs is one panel opening at the
    /// clock the machine settled to, which is every panel before this module
    /// existed.
    #[test]
    fn a_note_that_cannot_be_written_stops_the_hurry_rather_than_risking_it() {
        let at = processors("nonote", 2);
        // Somewhere a directory cannot be made, because a file is in the way.
        let blocked = at.join("in-the-way");
        std::fs::write(&blocked, b"not a directory").expect("something in the way");

        let mut hurrying = Hurrying::noting(&at, &blocked.join("hurried"));
        hurrying.asked(Instant::now());

        assert_eq!(hurrying.on(), Hurry::Off, "it hurried with nowhere to write the words down");
        for core in 0..2 {
            assert_eq!(said(&at, core), "power", "cpu{core} was hurried anyway");
        }
    }

    /// A line that does not parse is handed back, not filtered away.
    #[test]
    fn a_torn_line_in_a_note_is_kept_rather_than_skipped() {
        let (words, torn) = words_in("power\t/sys/cpu0/hint\nhalf a line\n");
        assert_eq!(words, [(PathBuf::from("/sys/cpu0/hint"), "power".to_string())]);
        assert_eq!(torn, ["half a line"]);
    }

    /// The word first and the path second, because a path can hold a tab and a
    /// word cannot. Split the other way round this would read a directory with
    /// a tab in its name as the word to put back.
    #[test]
    fn a_path_with_a_tab_in_it_still_reads_as_one_path() {
        let odd = PathBuf::from("/sys/a\tfolder/hint");
        let at = std::env::temp_dir().join(format!("console-haste-tab-{}", std::process::id()));
        let note = at.join("hurried");
        wrote_note(&note, &[(odd.clone(), "power".to_string())]).expect("a note");

        let held = std::fs::read_to_string(&note).expect("the note");
        let (words, torn) = words_in(&held);
        assert_eq!(words, [(odd, "power".to_string())]);
        assert!(torn.is_empty(), "a path with a tab in it was read as a torn line");

        let _ = std::fs::remove_dir_all(&at);
    }

    /// A file that is not there and a file that will not be read are two
    /// different answers, and the second one is a fault.
    #[test]
    fn nothing_to_read_and_cannot_be_read_are_told_apart() {
        let at = std::env::temp_dir().join(format!("console-haste-held-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).expect("somewhere to work");

        assert_eq!(read(&at.join("nothing-here")), Held::Nothing);

        // A directory is there and is not a file, so reading it fails as
        // something other than absence.
        match read(&at) {
            Held::Unreadable(_) => {}
            other => panic!("a directory read as {other:?} rather than as a fault"),
        }

        let _ = std::fs::remove_dir_all(&at);
    }
}
