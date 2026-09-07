//! The processors, asked to hurry for as long as somebody is waiting.  This is
//! a handheld, and most of the time it is right for it to be slow. It sits in a
//! bag at its lowest clock and the battery lasts the day. But a panel is a
//! moment's work and then nothing at all, and a processor deciding how fast to
//! run by watching how busy it has been is always deciding about the wrong
//! moment: the whole of an opening is over before the load it made has been
//! noticed.  What that costs is in `console-response-times`, which reads what
//! every opening on this machine wrote down about itself, and it is not one
//! slow stretch. It is every stretch: the loader, GTK coming up, the card being
//! built, the rows going on it, the first frame. Nothing there is slow. All of
//! it is being done at a fraction of the clock the machine can run at, because
//! nothing asked it for more and by the time anything could have, the press was
//! answered.  So the daemon that reads the pad says so as it starts something:
//! hurry, for about as long as an opening takes, and then let it be. Run
//! `console-response-times` before and after to see what it is worth on the
//! machine in your hands. What it costs is a moment of ordinary clock speed per
//! press, on a device that is otherwise asleep between them.
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

use console_core_atomic_writes::{Held, read};
use console_core_never::Never;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const CPUS: &str = "/sys/devices/system/cpu";

pub const HINT: &str = "cpufreq/energy_performance_preference";

pub const HURRY: &str = "balance_performance";

pub const FOR: Duration = Duration::from_millis(750);

pub struct Hurrying {
    cpus: PathBuf,
    note: PathBuf,
    until: Option<Instant>,
    was: Vec<(PathBuf, String)>,
    said: bool,
}

impl Default for Hurrying {
    fn default() -> Self {
        let Ok(hurrying) = Hurrying::of(Path::new(CPUS));

        hurrying
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hurry {
    On,
    Off,
}

impl Hurrying {
    pub fn of(cpus: &Path) -> Result<Self, Never> {
        let Ok(note) = note();
        let Ok(mut hurrying) = Hurrying::noting(cpus, &note);
        let Ok(_left) = hurrying.put_back_what_was_left();

        Ok(hurrying)
    }

    pub fn noting(cpus: &Path, note: &Path) -> Result<Self, Never> {
        Ok(Hurrying {
            cpus: cpus.to_path_buf(),
            note: note.to_path_buf(),
            until: None,
            was: Vec::new(),
            said: false,
        })
    }

    pub fn put_back_what_was_left(&mut self) -> Result<Left, Never> {
        let Ok(note) = read(&self.note);

        let held = match note {
            Held::Nothing => return Ok(Left::Nothing),
            Held::Said(held) => held,
            Held::Unreadable(fault) => {
                eprintln!(
                    "console-haste: {} is what says which words the processors are holding, and                      it will not be read: {fault}. They may be left at {HURRY}; a reboot is what                      puts them back.",
                    self.note.display()
                );
                let Ok(()) = forget(&self.note);

                return Ok(Left::Nothing);
            }
        };

        let Ok((words, torn)) = words_in(&held);

        for (hint, was) in &words {
            match std::fs::write(hint, was) {
                Ok(()) => {}
                Err(fault) => eprintln!(
                    "console-haste: {} was left hurried by a run that did not finish and will                      not take {was} back: {fault}",
                    hint.display()
                ),
            }
        }

        for line in &torn {
            eprintln!("console-haste: {} holds a line this cannot read: {line}", self.note.display());
        }

        let Ok(()) = forget(&self.note);

        Ok(match words.is_empty() {
            true => Left::Nothing,
            false => Left::PutBack,
        })
    }

    pub fn on(&self) -> Result<Hurry, Never> {
        Ok(match self.until.is_some() {
            true => Hurry::On,
            false => Hurry::Off,
        })
    }

    pub fn asked(&mut self, now: Instant) -> Result<(), Never> {
        let ending = now + FOR;

        match self.until {
            Some(_) => {
                self.until = Some(ending);
                return Ok(());
            }
            None => {}
        }

        let mut taking: Words = Vec::new();
        let mut unreadable: Option<PathBuf> = None;

        let Ok(hints) = self.hints();

        for hint in hints {
            let Ok(was) = read(&hint);

            match was {
                Held::Said(was) => {
                    let was = was.trim().to_string();

                    match was == HURRY {
                        true => {}
                        false => taking.push((hint, was)),
                    }
                }
                Held::Nothing => {}
                Held::Unreadable(_) => unreadable = Some(hint),
            }
        }

        match unreadable {
            Some(hint) => {
                let Ok(()) = self.complain(&hint);
            }
            None => {}
        }

        match taking.is_empty() {
            true => return Ok(()),
            false => {}
        }

        match wrote_note(&self.note, &taking) {
            Ok(()) => {}
            Err(_fault) => {
                let Ok(()) = self.complain_about_the_note();

                return Ok(());
            }
        }

        let mut wrote = false;

        for (hint, was) in taking {
            match std::fs::write(&hint, HURRY) {
                Ok(()) => {
                    self.was.push((hint, was));
                    wrote = true;
                }
                Err(_fault) => {
                    let Ok(()) = self.complain(&hint);
                }
            }
        }

        match wrote {
            true => self.until = Some(ending),
            false => {}
        }

        Ok(())
    }

    pub fn settle(&mut self, now: Instant) -> Result<(), Never> {
        let Some(until) = self.until else { return Ok(()) };

        match now < until {
            true => return Ok(()),
            false => {}
        }

        for (hint, was) in std::mem::take(&mut self.was) {
            match std::fs::write(&hint, &was) {
                Ok(()) => {}
                Err(_fault) => {
                    let Ok(()) = self.complain(&hint);
                }
            }
        }

        let Ok(()) = forget(&self.note);

        self.until = None;

        Ok(())
    }

    fn hints(&self) -> Result<Vec<PathBuf>, Never> {
        let Ok(reading) = std::fs::read_dir(&self.cpus) else { return Ok(Vec::new()) };

        let mut hints: Vec<PathBuf> = reading
            .filter_map(Result::ok)
            .map(|one| one.path().join(HINT))
            .filter(|hint| hint.exists())
            .collect();
        hints.sort();

        Ok(hints)
    }

    fn complain(&mut self, hint: &Path) -> Result<(), Never> {
        match self.said {
            true => return Ok(()),
            false => {}
        }

        self.said = true;
        eprintln!(
            "console-haste: {} will not take a word, so panels open at whatever \
             clock the machine happens to be at",
            hint.display()
        );

        Ok(())
    }

    fn complain_about_the_note(&mut self) -> Result<(), Never> {
        match self.said {
            true => return Ok(()),
            false => {}
        }

        self.said = true;
        eprintln!(
            "console-haste: {} cannot be written, so the processors are left alone: hurrying \
             them without writing down what they held is how they get stuck at {HURRY}",
            self.note.display()
        );

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Left {
    Nothing,
    PutBack,
}

pub fn note() -> Result<PathBuf, Never> {
    let run = match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(run) => PathBuf::from(run),
        None => std::env::temp_dir(),
    };

    Ok(run.join("console").join("hurried"))
}

fn wrote_note(at: &Path, words: &[(PathBuf, String)]) -> Result<(), String> {
    match at.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| format!("{}: its directory: {fault}", at.display()))?,
        None => {}
    }

    let written: String = words
        .iter()
        .map(|(hint, was)| format!("{was}\t{}\n", hint.display()))
        .collect();

    console_core_atomic_writes::whole(at, written.as_bytes())
}

type Words = Vec<(PathBuf, String)>;

type Torn = Vec<String>;

fn words_in(held: &str) -> Result<(Words, Torn), Never> {
    let mut words = Vec::new();
    let mut torn = Vec::new();

    for line in held.lines().filter(|line| !line.is_empty()) {
        match line.split_once('\t') {
            Some((was, hint)) => words.push((PathBuf::from(hint), was.to_string())),
            None => torn.push(line.to_string()),
        }
    }

    Ok((words, torn))
}

fn forget(at: &Path) -> Result<(), Never> {
    match std::fs::remove_file(at) {
        Ok(()) => {}
        Err(fault) if fault.kind() == std::io::ErrorKind::NotFound => {}
        Err(fault) => eprintln!(
            "console-haste: {} will not go away: {fault}. The words in it go back at every start              until it does.",
            at.display()
        ),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn processors(named: &str, cores: usize) -> PathBuf {
        let here = std::env::temp_dir().join(format!("console-haste-{named}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&here);
        for core in 0..cores {
            let at = here.join(format!("cpu{core}")).join("cpufreq");
            std::fs::create_dir_all(&at).expect("somewhere to keep a hint");
            std::fs::write(at.join("energy_performance_preference"), "power\n")
                .expect("a hint to write");
        }
        std::fs::create_dir_all(here.join("cpufreq")).expect("the shared directory");
        std::fs::create_dir_all(here.join("cpuidle")).expect("the idle directory");
        here
    }

    fn a_note_of_our_own(at: &Path) -> PathBuf {
        at.join("hurried")
    }

    fn hurrying(at: &Path) -> Hurrying {
        let Ok(hurrying) = Hurrying::noting(at, &a_note_of_our_own(at));

        hurrying
    }

    fn said(at: &Path, core: usize) -> String {
        let hint = at.join(format!("cpu{core}")).join(HINT);
        std::fs::read_to_string(hint).unwrap_or_default().trim().to_string()
    }

    #[test]
    fn a_press_hurries_the_processors_and_the_moment_after_lets_them_be() {
        let at = processors("press", 4);
        let mut hurrying = hurrying(&at);
        let now = Instant::now();

        let Ok(()) = hurrying.asked(now);
        let Ok(on) = hurrying.on();

        assert_eq!(on, Hurry::On);
        for core in 0..4 {
            assert_eq!(said(&at, core), HURRY, "cpu{core} was not hurried");
        }

        let Ok(()) = hurrying.settle(now + FOR / 2);
        assert_eq!(said(&at, 0), HURRY);

        let Ok(()) = hurrying.settle(now + FOR);
        let Ok(on) = hurrying.on();

        assert_eq!(on, Hurry::Off);
        for core in 0..4 {
            assert_eq!(said(&at, core), "power", "cpu{core} was not let be");
        }
    }

    #[test]
    fn asking_again_moves_the_end_rather_than_starting_a_second_one() {
        let at = processors("again", 2);
        let mut hurrying = hurrying(&at);
        let now = Instant::now();

        let Ok(()) = hurrying.asked(now);
        let Ok(()) = hurrying.asked(now + FOR / 2);
        let Ok(()) = hurrying.settle(now + FOR);
        let Ok(on) = hurrying.on();

        assert_eq!(on, Hurry::On);
        assert_eq!(said(&at, 0), HURRY);

        let Ok(()) = hurrying.settle(now + FOR + FOR / 2);
        let Ok(on) = hurrying.on();

        assert_eq!(on, Hurry::Off);
        assert_eq!(said(&at, 0), "power");
    }

    #[test]
    fn what_goes_back_is_what_was_there() {
        let at = processors("kept", 1);
        let hint = at.join("cpu0").join(HINT);
        std::fs::write(&hint, "performance\n").expect("a hint to write");

        let mut hurrying = hurrying(&at);
        let now = Instant::now();
        let Ok(()) = hurrying.asked(now);
        let Ok(()) = hurrying.settle(now + FOR);
        assert_eq!(said(&at, 0), "performance");
    }

    #[test]
    fn a_processor_that_is_already_hurrying_is_not_written_to() {
        let at = processors("standing", 1);
        let hint = at.join("cpu0").join(HINT);
        std::fs::write(&hint, format!("{HURRY}\n")).expect("a hint to write");

        let mut hurrying = hurrying(&at);
        let Ok(()) = hurrying.asked(Instant::now());
        let Ok(on) = hurrying.on();

        assert_eq!(on, Hurry::Off);
        assert_eq!(said(&at, 0), HURRY);
    }

    #[test]
    fn processors_that_cannot_be_hurried_are_not_an_error() {
        let at = std::env::temp_dir().join(format!("console-haste-none-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        let mut hurrying = hurrying(&at);
        let now = Instant::now();
        let Ok(()) = hurrying.asked(now);
        let Ok(on) = hurrying.on();

        assert_eq!(on, Hurry::Off);
        let Ok(()) = hurrying.settle(now + FOR);
    }

    #[test]
    fn a_run_that_stops_mid_hurry_leaves_its_words_written_down() {
        let at = processors("stopped", 3);
        let mut dying = hurrying(&at);
        let Ok(()) = dying.asked(Instant::now());
        drop(dying);

        for core in 0..3 {
            assert_eq!(said(&at, core), HURRY, "cpu{core} was not hurried");
        }

        let note = a_note_of_our_own(&at);
        assert!(note.exists(), "nothing was written down, so nothing can put these back");
        let held = std::fs::read_to_string(&note).expect("the note");
        assert_eq!(held.lines().count(), 3, "the note does not cover every processor: {held:?}");
    }

    #[test]
    fn the_next_daemon_puts_back_what_a_stopped_one_left() {
        let at = processors("nextone", 3);
        let mut dying = hurrying(&at);
        let Ok(()) = dying.asked(Instant::now());
        drop(dying);

        let mut coming_up = hurrying(&at);
        let Ok(left) = coming_up.put_back_what_was_left();

        assert_eq!(left, Left::PutBack);

        for core in 0..3 {
            assert_eq!(said(&at, core), "power", "cpu{core} is still hurried");
        }
        assert!(!a_note_of_our_own(&at).exists(), "the note outlived the words going back");
    }

    #[test]
    fn a_stopped_run_does_not_leave_the_processors_hurried_for_ever() {
        let at = processors("forever", 2);
        let mut dying = hurrying(&at);
        let Ok(()) = dying.asked(Instant::now());
        drop(dying);

        let Ok(mut after) = Hurrying::noting(&at, &a_note_of_our_own(&at));
        let Ok(_left) = after.put_back_what_was_left();
        let now = Instant::now();
        let Ok(()) = after.asked(now);
        let Ok(()) = after.settle(now + FOR);

        for core in 0..2 {
            assert_eq!(said(&at, core), "power", "cpu{core} never came back");
        }
    }

    #[test]
    fn a_machine_that_asks_for_the_hurried_word_is_not_written_down() {
        let at = processors("agrees", 2);
        for core in 0..2 {
            let hint = at.join(format!("cpu{core}")).join(HINT);
            std::fs::write(&hint, format!("{HURRY}\n")).expect("a hint to write");
        }

        let mut hurrying = hurrying(&at);
        let Ok(()) = hurrying.asked(Instant::now());

        let Ok(on) = hurrying.on();

        assert_eq!(on, Hurry::Off, "a hurry was started with nothing to change");
        assert!(!a_note_of_our_own(&at).exists(), "a word nobody overwrote was written down");
    }

    #[test]
    fn settling_takes_the_note_away() {
        let at = processors("settled", 2);
        let mut hurrying = hurrying(&at);
        let now = Instant::now();

        let Ok(()) = hurrying.asked(now);
        assert!(a_note_of_our_own(&at).exists(), "nothing was written down during the hurry");

        let Ok(()) = hurrying.settle(now + FOR);
        assert!(!a_note_of_our_own(&at).exists(), "the note outlived the hurry");
    }

    #[test]
    fn a_note_that_cannot_be_written_stops_the_hurry_rather_than_risking_it() {
        let at = processors("nonote", 2);
        let blocked = at.join("in-the-way");
        std::fs::write(&blocked, b"not a directory").expect("something in the way");

        let Ok(mut hurrying) = Hurrying::noting(&at, &blocked.join("hurried"));
        let Ok(()) = hurrying.asked(Instant::now());

        let Ok(on) = hurrying.on();

        assert_eq!(on, Hurry::Off, "it hurried with nowhere to write the words down");
        for core in 0..2 {
            assert_eq!(said(&at, core), "power", "cpu{core} was hurried anyway");
        }
    }

    #[test]
    fn a_torn_line_in_a_note_is_kept_rather_than_skipped() {
        let Ok((words, torn)) = words_in("power\t/sys/cpu0/hint\nhalf a line\n");
        assert_eq!(words, [(PathBuf::from("/sys/cpu0/hint"), "power".to_string())]);
        assert_eq!(torn, ["half a line"]);
    }

    #[test]
    fn a_path_with_a_tab_in_it_still_reads_as_one_path() {
        let odd = PathBuf::from("/sys/a\tfolder/hint");
        let at = std::env::temp_dir().join(format!("console-haste-tab-{}", std::process::id()));
        let note = at.join("hurried");
        wrote_note(&note, &[(odd.clone(), "power".to_string())]).expect("a note");

        let held = std::fs::read_to_string(&note).expect("the note");
        let Ok((words, torn)) = words_in(&held);
        assert_eq!(words, [(odd, "power".to_string())]);
        assert!(torn.is_empty(), "a path with a tab in it was read as a torn line");

        let _ = std::fs::remove_dir_all(&at);
    }

    #[test]
    fn nothing_to_read_and_cannot_be_read_are_told_apart() {
        let at = std::env::temp_dir().join(format!("console-haste-held-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).expect("somewhere to work");

        let Ok(nothing) = read(&at.join("nothing-here"));

        assert_eq!(nothing, Held::Nothing);

        let Ok(held) = read(&at);

        match held {
            Held::Unreadable(_) => {}
            other => panic!("a directory read as {other:?} rather than as a fault"),
        }

        let _ = std::fs::remove_dir_all(&at);
    }
}
