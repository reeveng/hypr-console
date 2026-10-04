//! The processors, asked to hurry for as long as someone is waiting.
//!
//! This is a handheld, and most of the time it is right for it to be slow. It
//! sits in a bag at its lowest clock and the battery lasts the day. But a panel
//! is a moment's work and then nothing at all, and a processor deciding how
//! fast to run by watching how busy it has been is always deciding about the
//! wrong moment: the whole of an opening is over before the load it made has
//! been noticed.
//!
//! What that costs is in `console-response-times`, which reads what every
//! opening on this machine wrote down about itself, and it is not one slow
//! stretch. It is every stretch: the loader, GTK coming up, the card being
//! built, the rows going on it, the first frame. Nothing there is slow. All of
//! it is being done at a fraction of the clock the machine can run at, because
//! nothing asked it for more and by the time anything could have, the press was
//! answered.
//!
//! So the daemon that reads the pad says so as it starts something: hurry, for
//! about as long as an opening takes, and then let it be. Run
//! `console-response-times` before and after to see what it is worth on the
//! machine in your hands. What it costs is a moment of ordinary clock speed per
//! press, on a device that is otherwise asleep between them.
//!
//! ## The knob, and why this one
//!
//! `amd-pstate-epp` picks the frequency in hardware, from how busy a core has
//! been and from one hint: `energy_performance_preference`, a word per core
//! under `/sys/devices/system/cpu`. There is no per-task version of it. The
//! scheduler's own `uclamp` drives `schedutil`, and this machine is not on
//! `schedutil`, so a hint about the task that is opening the panel is not a
//! thing this kernel can be given. The hint is the machine's, or it is nothing.
//!
//! Which is why what is written is put back. `power-profiles-daemon` owns this
//! file -- power-saver writes `power` into it, balanced writes
//! `balance_performance` -- and a desktop that raised it and walked away would
//! be a machine quietly ignoring the profile someone chose, for ever, with
//! nothing on any screen saying so. So the word that was there is read before
//! it is changed and written back when the moment is over, and what the profile
//! says is what the machine does between presses.
//!
//! `balance_performance` rather than `performance`: measured, they were the
//! same opening to within noise, and the gentler of two words that do the same
//! thing is the one to write into someone's power settings.
//!
//! ## What a daemon that dies owes the machine
//!
//! The word each core held is the one thing here that cannot be worked out
//! again. It is read off the hint, the hint is then overwritten, and from that
//! moment the only copy of it is in this process. A daemon that does not reach
//! `settle` -- a crash, the target stopping, an apply restarting it, a battery
//! that ran out -- takes that copy with it, and every core is left at
//! `balance_performance` for as long as the machine stays up. That is the
//! machine quietly ignoring the profile someone chose, which is the outcome
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

use console_core_atomic_writes::{Stored, read};
use console_core_never::Never;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const CPUS: &str = "/sys/devices/system/cpu";

pub const HINT: &str = "cpufreq/energy_performance_preference";

pub const HURRY: &str = "balance_performance";

pub const FOR: Duration = Duration::from_millis(750);

#[cfg_attr(
    dylint_lib = "explicit048_no_unreal_state",
    allow(
        explicit048_no_unreal_state,
        reason = "`until` is when a hurry ends and `said` is whether the one complaint about a processor that will not take a word has been printed; a machine that cannot be hurried is still one this has already complained about, so the two are answers to different questions"
    )
)]
pub struct HighPowerMode {
    cpus: PathBuf,
    note: PathBuf,
    until: Option<Instant>,
    was: Vec<(PathBuf, String)>,
    said: bool,
}

impl Default for HighPowerMode {
    fn default() -> Self {
        let Ok(hurrying) = HighPowerMode::of(Path::new(CPUS));

        hurrying
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boost {
    On,
    Off,
}

impl HighPowerMode {
    pub fn of(cpus: &Path) -> Result<Self, Never> {
        let Ok(note) = note();
        let Ok(mut hurrying) = HighPowerMode::new(cpus, &note);
        let Ok(_left) = hurrying.put_back_what_was_left();

        Ok(hurrying)
    }

    pub fn new(cpus: &Path, note: &Path) -> Result<Self, Never> {
        Ok(HighPowerMode {
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
            Stored::Absent => return Ok(Left::None),
            Stored::Text(held) => held,
            Stored::Failed(fault) => {
                eprintln!(
                    "console-haste: {} is what says which words the processors are holding, and                      it will not be read: {fault}. They may be left at {HURRY}; a reboot is what                      puts them back.",
                    self.note.display()
                );
                let Ok(()) = forget(&self.note);

                return Ok(Left::None);
            }
        };

        let Ok((words, torn)) = words_in(&held);

        for (hint, was) in &words {
            #[cfg_attr(
                dylint_lib = "explicit040_no_torn_write",
                allow(
                    explicit040_no_torn_write,
                    reason = "a hint is a kernel knob under `/sys` rather than a file: there is nothing beside it to write and nothing to rename over, and what it holds is the word last written to it"
                )
            )]
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
            true => Left::None,
            false => Left::Restore,
        })
    }

    pub fn on(&self) -> Result<Boost, Never> {
        Ok(match self.until.is_some() {
            true => Boost::On,
            false => Boost::Off,
        })
    }

    pub fn boost(&mut self, now: Instant) -> Result<(), Never> {
        let ending = now + FOR;

        match self.until {
            Some(_) => {
                self.until = Some(ending);
                return Ok(());
            }
            None => {}
        }

        let mut taking: Entries = Vec::new();
        let mut unreadable: Option<PathBuf> = None;

        let Ok(hints) = self.hints();

        for hint in hints {
            let Ok(was) = read(&hint);

            match was {
                Stored::Text(was) => {
                    let was = was.trim().to_string();

                    match was == HURRY {
                        true => {}
                        false => taking.push((hint, was)),
                    }
                }
                Stored::Absent => {}
                Stored::Failed(_) => unreadable = Some(hint),
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
            Err(_unwritten) => {
                let Ok(()) = self.complain_about_the_note();

                return Ok(());
            }
        }

        let mut wrote = false;

        for (hint, was) in taking {
            #[cfg_attr(
                dylint_lib = "explicit040_no_torn_write",
                allow(
                    explicit040_no_torn_write,
                    reason = "a hint is a kernel knob under `/sys` rather than a file: there is nothing beside it to write and nothing to rename over, and what it holds is the word last written to it"
                )
            )]
            match std::fs::write(&hint, HURRY) {
                Ok(()) => {
                    self.was.push((hint, was));
                    wrote = true;
                }
                Err(_unwritten) => {
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
        let until = match self.until {
            Some(until) => until,
            None => return Ok(()),
        };

        match now < until {
            true => return Ok(()),
            false => {}
        }

        for (hint, was) in std::mem::take(&mut self.was) {
            #[cfg_attr(
                dylint_lib = "explicit040_no_torn_write",
                allow(
                    explicit040_no_torn_write,
                    reason = "a hint is a kernel knob under `/sys` rather than a file: there is nothing beside it to write and nothing to rename over, and what it holds is the word last written to it"
                )
            )]
            match std::fs::write(&hint, &was) {
                Ok(()) => {}
                Err(_unwritten) => {
                    let Ok(()) = self.complain(&hint);
                }
            }
        }

        let Ok(()) = forget(&self.note);

        self.until = None;

        Ok(())
    }

    fn hints(&self) -> Result<Vec<PathBuf>, Never> {
        let reading = match std::fs::read_dir(&self.cpus) {
            Ok(reading) => reading,
            Err(_unreadable) => return Ok(Vec::new()),
        };

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
    None,
    Restore,
}

#[cfg_attr(
    dylint_lib = "explicit044_no_ambient_value",
    allow(
        explicit044_no_ambient_value,
        reason = "with no runtime directory the mark is left where the next run of this program will look for it, which is a directory shared on purpose and not one of this process's own"
    )
)]
pub fn note() -> Result<PathBuf, Never> {
    let ours = console_core_places::application_runtime()?;

    Ok(match ours {
        Some(ours) => ours.join("hurried"),
        None => std::env::temp_dir().join(console_core_places::APPLICATION).join("hurried"),
    })
}

#[derive(Debug)]
pub enum Unnoted {
    Holding(PathBuf, std::io::Error),
    Writing(console_core_atomic_writes::Unwritten),
}

impl fmt::Display for Unnoted {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unnoted::Holding(at, fault) => write!(to, "{}: its directory: {fault}", at.display()),
            Unnoted::Writing(fault) => write!(to, "{fault}"),
        }
    }
}

impl std::error::Error for Unnoted {}

impl From<console_core_atomic_writes::Unwritten> for Unnoted {
    fn from(fault: console_core_atomic_writes::Unwritten) -> Self {
        Unnoted::Writing(fault)
    }
}

fn wrote_note(at: &Path, words: &[(PathBuf, String)]) -> Result<(), Unnoted> {
    match at.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| Unnoted::Holding(at.to_path_buf(), fault))?,
        None => {}
    }

    let written: String = words
        .iter()
        .map(|(hint, was)| format!("{was}\t{}\n", hint.display()))
        .collect();

    console_core_atomic_writes::whole(at, written.as_bytes()).map_err(Unnoted::Writing)
}

type Entries = Vec<(PathBuf, String)>;

type Error = Vec<String>;

fn words_in(held: &str) -> Result<(Entries, Error), Never> {
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
    match console_core_atomic_writes::gone(at) {
        Ok(()) => {}
        Err(fault) => eprintln!("console-haste: {fault}. The words in it go back at every start until it does."),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    const NOTE: &str = "hurried";

    fn processors(named: &str, cores: u32) -> Result<PathBuf, Box<dyn Error>> {
        let here = console_core_temporary_directories::fresh(&format!("haste-{named}"))?;

        for core in 0..cores {
            let at = here.join(format!("cpu{core}")).join("cpufreq");

            std::fs::create_dir_all(&at)?;
            console_core_atomic_writes::whole(&at.join("energy_performance_preference"), b"power\n")?;
        }

        std::fs::create_dir_all(here.join("cpufreq"))?;
        std::fs::create_dir_all(here.join("cpuidle"))?;

        Ok(here)
    }

    fn said(at: &Path, core: u32) -> Result<String, std::io::Error> {
        let hint = at.join(format!("cpu{core}")).join(HINT);
        let written = std::fs::read_to_string(hint)?;

        Ok(written.trim().to_string())
    }

    #[test]
    fn a_press_hurries_the_processors_and_the_moment_after_lets_them_be() -> Result<(), Box<dyn Error>> {
        let at = processors("press", 4)?;
        let Ok(mut hurrying) = HighPowerMode::new(&at, &at.join(NOTE));
        let now = Instant::now();

        let Ok(()) = hurrying.boost(now);
        let Ok(on) = hurrying.on();

        assert_eq!(on, Boost::On);

        for core in 0..4 {
            let hint = said(&at, core)?;

            assert_eq!(hint, HURRY, "cpu{core} was not hurried");
        }

        let Ok(()) = hurrying.settle(now + FOR / 2);
        let hint = said(&at, 0)?;

        assert_eq!(hint, HURRY);

        let Ok(()) = hurrying.settle(now + FOR);
        let Ok(on) = hurrying.on();

        assert_eq!(on, Boost::Off);

        for core in 0..4 {
            let hint = said(&at, core)?;

            assert_eq!(hint, "power", "cpu{core} was not let be");
        }

        Ok(())
    }

    #[test]
    fn asking_again_moves_the_end_rather_than_starting_a_second_one() -> Result<(), Box<dyn Error>> {
        let at = processors("again", 2)?;
        let Ok(mut hurrying) = HighPowerMode::new(&at, &at.join(NOTE));
        let now = Instant::now();

        let Ok(()) = hurrying.boost(now);
        let Ok(()) = hurrying.boost(now + FOR / 2);
        let Ok(()) = hurrying.settle(now + FOR);
        let Ok(on) = hurrying.on();

        assert_eq!(on, Boost::On);
        let hint = said(&at, 0)?;

        assert_eq!(hint, HURRY);

        let Ok(()) = hurrying.settle(now + FOR + FOR / 2);
        let Ok(on) = hurrying.on();

        assert_eq!(on, Boost::Off);
        let hint = said(&at, 0)?;

        assert_eq!(hint, "power");

        Ok(())
    }

    #[test]
    fn what_goes_back_is_what_was_there() -> Result<(), Box<dyn Error>> {
        let at = processors("kept", 1)?;
        let hint = at.join("cpu0").join(HINT);
        console_core_atomic_writes::whole(&hint, b"performance\n")?;

        let Ok(mut hurrying) = HighPowerMode::new(&at, &at.join(NOTE));
        let now = Instant::now();
        let Ok(()) = hurrying.boost(now);
        let Ok(()) = hurrying.settle(now + FOR);
        let hint = said(&at, 0)?;

        assert_eq!(hint, "performance");

        Ok(())
    }

    #[test]
    fn a_processor_that_is_already_hurrying_is_not_written_to() -> Result<(), Box<dyn Error>> {
        let at = processors("standing", 1)?;
        let hint = at.join("cpu0").join(HINT);
        console_core_atomic_writes::whole(&hint, format!("{HURRY}\n").as_bytes())?;

        let Ok(mut hurrying) = HighPowerMode::new(&at, &at.join(NOTE));
        let Ok(()) = hurrying.boost(Instant::now());
        let Ok(on) = hurrying.on();

        assert_eq!(on, Boost::Off);
        let hint = said(&at, 0)?;

        assert_eq!(hint, HURRY);

        Ok(())
    }

    #[test]
    fn processors_that_cannot_be_hurried_are_not_an_error() -> Result<(), Box<dyn Error>> {
        let here = console_core_temporary_directories::fresh("haste-none")?;
        let at = here.join("nowhere");
        let Ok(mut hurrying) = HighPowerMode::new(&at, &at.join(NOTE));
        let now = Instant::now();
        let Ok(()) = hurrying.boost(now);
        let Ok(on) = hurrying.on();

        assert_eq!(on, Boost::Off);
        let Ok(()) = hurrying.settle(now + FOR);

        Ok(())
    }

    #[test]
    fn a_run_that_stops_mid_hurry_leaves_its_words_written_down() -> Result<(), Box<dyn Error>> {
        let at = processors("stopped", 3)?;
        let Ok(mut dying) = HighPowerMode::new(&at, &at.join(NOTE));
        let Ok(()) = dying.boost(Instant::now());
        drop(dying);

        for core in 0..3 {
            let hint = said(&at, core)?;

            assert_eq!(hint, HURRY, "cpu{core} was not hurried");
        }

        let note = at.join(NOTE);
        assert!(note.exists(), "nothing was written down, so nothing can put these back");
        let held = std::fs::read_to_string(&note)?;
        assert_eq!(held.lines().count(), 3, "the note does not cover every processor: {held:?}");

        Ok(())
    }

    #[test]
    fn the_next_daemon_puts_back_what_a_stopped_one_left() -> Result<(), Box<dyn Error>> {
        let at = processors("nextone", 3)?;
        let Ok(mut dying) = HighPowerMode::new(&at, &at.join(NOTE));
        let Ok(()) = dying.boost(Instant::now());
        drop(dying);

        let Ok(mut coming_up) = HighPowerMode::new(&at, &at.join(NOTE));
        let Ok(left) = coming_up.put_back_what_was_left();

        assert_eq!(left, Left::Restore);

        for core in 0..3 {
            let hint = said(&at, core)?;

            assert_eq!(hint, "power", "cpu{core} is still hurried");
        }

        assert!(!at.join(NOTE).exists(), "the note outlived the words going back");

        Ok(())
    }

    #[test]
    fn a_stopped_run_does_not_leave_the_processors_hurried_for_ever() -> Result<(), Box<dyn Error>> {
        let at = processors("forever", 2)?;
        let Ok(mut dying) = HighPowerMode::new(&at, &at.join(NOTE));
        let Ok(()) = dying.boost(Instant::now());
        drop(dying);

        let Ok(mut after) = HighPowerMode::new(&at, &at.join(NOTE));
        let Ok(_left) = after.put_back_what_was_left();
        let now = Instant::now();
        let Ok(()) = after.boost(now);
        let Ok(()) = after.settle(now + FOR);

        for core in 0..2 {
            let hint = said(&at, core)?;

            assert_eq!(hint, "power", "cpu{core} never came back");
        }

        Ok(())
    }

    #[test]
    fn a_machine_that_asks_for_the_hurried_word_is_not_written_down() -> Result<(), Box<dyn Error>> {
        let at = processors("agrees", 2)?;

        for core in 0..2 {
            let hint = at.join(format!("cpu{core}")).join(HINT);
            console_core_atomic_writes::whole(&hint, format!("{HURRY}\n").as_bytes())?;
        }

        let Ok(mut hurrying) = HighPowerMode::new(&at, &at.join(NOTE));
        let Ok(()) = hurrying.boost(Instant::now());

        let Ok(on) = hurrying.on();

        assert_eq!(on, Boost::Off, "a hurry was started with nothing to change");
        assert!(!at.join(NOTE).exists(), "a word no one overwrote was written down");

        Ok(())
    }

    #[test]
    fn settling_takes_the_note_away() -> Result<(), Box<dyn Error>> {
        let at = processors("settled", 2)?;
        let Ok(mut hurrying) = HighPowerMode::new(&at, &at.join(NOTE));
        let now = Instant::now();

        let Ok(()) = hurrying.boost(now);
        assert!(at.join(NOTE).exists(), "nothing was written down during the hurry");

        let Ok(()) = hurrying.settle(now + FOR);
        assert!(!at.join(NOTE).exists(), "the note outlived the hurry");

        Ok(())
    }

    #[test]
    fn a_note_that_cannot_be_written_stops_the_hurry_rather_than_risking_it() -> Result<(), Box<dyn Error>> {
        let at = processors("nonote", 2)?;
        let blocked = at.join("in-the-way");
        console_core_atomic_writes::whole(&blocked, b"not a directory")?;

        let Ok(mut hurrying) = HighPowerMode::new(&at, &blocked.join("hurried"));
        let Ok(()) = hurrying.boost(Instant::now());

        let Ok(on) = hurrying.on();

        assert_eq!(on, Boost::Off, "it hurried with nowhere to write the words down");

        for core in 0..2 {
            let hint = said(&at, core)?;

            assert_eq!(hint, "power", "cpu{core} was hurried anyway");
        }

        Ok(())
    }

    #[test]
    fn a_torn_line_in_a_note_is_kept_rather_than_skipped() {
        let Ok((words, torn)) = words_in("power\t/sys/cpu0/hint\nhalf a line\n");
        assert_eq!(words, [(PathBuf::from("/sys/cpu0/hint"), "power".to_string())]);
        assert_eq!(torn, ["half a line"]);
    }

    #[test]
    fn a_path_with_a_tab_in_it_still_reads_as_one_path() -> Result<(), Box<dyn Error>> {
        let odd = PathBuf::from("/sys/a\tfolder/hint");
        let at = console_core_temporary_directories::fresh("haste-tab")?;
        let note = at.join("hurried");
        wrote_note(&note, &[(odd.clone(), "power".to_string())])?;

        let held = std::fs::read_to_string(&note)?;
        let Ok((words, torn)) = words_in(&held);
        assert_eq!(words, [(odd, "power".to_string())]);
        assert!(torn.is_empty(), "a path with a tab in it was read as a torn line");

        let _ = std::fs::remove_dir_all(&at);

        Ok(())
    }

    #[test]
    fn nothing_to_read_and_cannot_be_read_are_told_apart() -> Result<(), Box<dyn Error>> {
        let at = console_core_temporary_directories::fresh("haste-held")?;

        let Ok(nothing) = read(&at.join("nothing-here"));

        assert_eq!(nothing, Stored::Absent);

        let Ok(held) = read(&at);

        let read_as = match held {
            Stored::Failed(_) => "a fault",
            Stored::Text(_) => "text",
            Stored::Absent => "nothing",
        };

        assert_eq!(read_as, "a fault", "a directory read as something other than a fault");

        let _ = std::fs::remove_dir_all(&at);

        Ok(())
    }
}
