//! How long the machine kept somebody waiting, written down where it adds up.
//! `console_manifest_engine::went` measures an apply, which is one program, on
//! demand, with the answer on stderr for whoever typed the variable. This is
//! the other half of the question and it is asked by nobody: what a person
//! waits for on this device is a menu that takes a moment to appear, and nobody
//! is standing at a terminal with a stopwatch when it does. So it is written
//! down as it happens, on the machine it happens on, and read afterwards.  One
//! line per thing waited for. Not one line per stamp with an id tying them
//! together: the id would exist only to put back what the writing took apart,
//! and every question anybody has -- how long does the menu take, is it worse
//! than last week, which stretch is the slow one -- would need the pieces
//! joined before it could be asked. A line is one opening, its stretches are
//! its fields, and they add up to what the line says was waited for.
//! ## What one looks like
//!
//! ```text
//! {"at":1756761123,"up":67932.4,"load":0.31,"who":"launcher","what":"opening",
//!  "waited":412.3,"press":11.4,"exec":3.1,"screen":21.0,"gtk":128.4,
//!  "built":9.2,"placed":96.1,"frame":143.1,
//!  "with":{"from":"pad","rows":73,"door":"menu"}}
//! ```
//!
//! `waited` is what a thumb waited, and the stretches after it are where that
//! went, in the order they happened. Every number at the top of a line is
//! milliseconds, so anything reading this can add the whole of it up without
//! being told which fields are time. What the wait was *about* -- how many rows
//! were drawn, which door it came out of -- is under `with`, where it cannot be
//! mistaken for a stretch of the wait.
//!
//! ## `press`, and when there is none
//!
//! `press` is the stretch between a finger and this process existing, and it is
//! the only one nothing in this process can see. Whoever starts the program
//! stamps `CONSOLE_PRESSED` with the moment of the press and `CONSOLE_FROM`
//! with where it came from; `Waiting::on` reads them back.
//!
//! A stamp that is not there leaves the field out rather than writing a zero.
//! Zero is a measurement -- it says the machine answered instantly -- and a
//! field that says that on nine lines in ten is a field nobody can believe on
//! the tenth. Absence says the honest thing, and `from` says why: an opening
//! from the bar has no `press` because waybar forks on the touch, so the fork
//! *is* the press and `exec` already holds the whole of that wait. An opening
//! from the pad has one, because the daemon holds the press for as long as a
//! turn of its loop takes before it starts anything.
//!
//! A stamp also goes stale. The environment is inherited, so a panel started by
//! a press carries that press to everything *it* ever starts, and a stamp read
//! back an hour later measured a wait that ended an hour ago. Anything past
//! `STALE` is not a press, and a program that starts something which is not a
//! press strips both marks with `not_a_press`.
//!
//! ## Where it goes, and why there
//!
//! `~/.local/state/console/waited.jsonl`, beside the tab a panel was left on
//! and not under `~/.cache`. A cache is the machine's own answer to a question
//! anybody can ask it again; this is the only record that the menu took four
//! hundred milliseconds at half past nine, and clearing it does not cost one
//! opening, it costs the week.
//!
//! Appended: every panel, the bar and the daemon write to the same file from
//! processes that know nothing about each other, and a single `write` of one
//! short line to a descriptor opened `O_APPEND` lands whole. So there is no
//! lock here, and nothing to hand over.
//!
//! The writing itself happens on a thread of the writing process's own, which
//! is `writing`. Nothing that is being timed opens a file, asks how long one is
//! or waits for a disk.
//!
//! The file is kept. It rotates at ten gigabytes, which on this device is a
//! stop against a program in a loop rather than a retention policy: the
//! question these lines exist to answer is whether the machine is getting
//! slower, and a store that keeps a week cannot be asked it.
//!
//! ## Always on
//!
//! Behind no variable. Timings that have to be asked for are timings nobody has
//! when they want them: the openings worth reading about are the ones that
//! happened while somebody was using the device, and by the time it is slow
//! enough to complain about, the run that was slow is over. It costs a handful
//! of `Instant`s and a line handed to a queue at the end of something that
//! already drew a window.

pub mod line;
pub mod summary;
pub mod writing;

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use console_core_never::Never;

pub use writing::settled;

pub const PRESSED: &str = "CONSOLE_PRESSED";

pub const FROM: &str = "CONSOLE_FROM";

pub const FELT: Duration = Duration::from_millis(16);

pub const STALE: Duration = Duration::from_secs(10);

fn asked(name: &str) -> Result<Option<String>, Never> {
    match std::env::var(name) {
        Ok(said) => Ok(Some(said)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(fault) => {
            eprintln!("console-response-times: {name}: {fault}");

            Ok(None)
        }
    }
}

pub fn where_() -> Result<Option<PathBuf>, Never> {
    let ours = console_core_places::Base::State.ours()?;

    Ok(ours.map(|ours| ours.join("waited.jsonl")))
}

pub struct Waiting {
    who: String,
    what: String,
    started: Instant,
    last: Instant,
    marks: Vec<(String, Duration)>,
    notes: Vec<(String, line::Said)>,
    before: Duration,
}

impl Waiting {
    pub fn on(who: &str, what: &str) -> Result<Self, Never> {
        let now = Instant::now();

        let Ok(since) = since_exec();
        let Ok(stamped) = since_press();

        let exec = since.unwrap_or_default();
        let press = stamped.map(|waited| waited.saturating_sub(exec));
        let mut marks = Vec::new();
        let mut before = exec;

        match press {
            Some(press) => {
                marks.push(("press".to_string(), press));
                before = before.saturating_add(press);
            }
            None => {}
        }

        marks.push(("exec".to_string(), exec));
        let mut notes = Vec::new();

        let Ok(came) = came_from();

        match came {
            Some(from) => notes.push(("from".to_string(), line::Said::Word(from))),
            None => {}
        }

        Ok(Waiting {
            who: who.to_string(),
            what: what.to_string(),
            started: now,
            last: now,
            marks,
            notes,
            before,
        })
    }

    pub fn asked(
        who: &str,
        what: &str,
        pressed: Option<&str>,
        from: &str,
        exec: Duration,
    ) -> Result<Self, Never> {
        let now = Instant::now();

        let stamped = match pressed {
            Some(raw) => {
                let Ok(waited) = waited_since(raw);

                waited
            }
            None => None,
        };

        let press = stamped.map(|waited| waited.saturating_sub(exec));
        let mut marks = Vec::new();
        let mut before = exec;

        match press {
            Some(press) => {
                marks.push(("press".to_string(), press));
                before = before.saturating_add(press);
            }
            None => {}
        }

        marks.push(("exec".to_string(), exec));
        let mut notes = Vec::new();

        match from.is_empty() {
            true => {}
            false => notes.push(("from".to_string(), line::Said::Word(from.to_string()))),
        }

        Ok(Waiting {
            who: who.to_string(),
            what: what.to_string(),
            started: now,
            last: now,
            marks,
            notes,
            before,
        })
    }

    pub fn here(who: &str, what: &str) -> Result<Self, Never> {
        let now = Instant::now();

        Ok(Waiting {
            who: who.to_string(),
            what: what.to_string(),
            started: now,
            last: now,
            marks: Vec::new(),
            notes: Vec::new(),
            before: Duration::ZERO,
        })
    }

    pub fn mark(&mut self, doing: &str) -> Result<(), Never> {
        let now = Instant::now();
        self.marks.push((doing.to_string(), now.saturating_duration_since(self.last)));
        self.last = now;

        Ok(())
    }

    pub fn taking(&mut self, doing: &str, took: Duration) -> Result<(), Never> {
        let exec = self.marks.iter_mut().find(|(name, _)| name == "exec");

        match exec {
            Some((_, was)) => *was = was.saturating_sub(took),
            None => self.before += took,
        }

        let after = self
            .marks
            .iter()
            .position(|(name, _)| name == "exec")
            .map_or(0, |at| at.saturating_add(1));
        self.marks.insert(after, (doing.to_string(), took));

        Ok(())
    }

    pub fn counted(&mut self, name: &str, many: u64) -> Result<(), Never> {
        self.notes.push((name.to_string(), line::Said::Count(many)));

        Ok(())
    }

    pub fn named(&mut self, name: &str, said: &str) -> Result<(), Never> {
        self.notes.push((name.to_string(), line::Said::Word(said.to_string())));

        Ok(())
    }

    pub fn so_far(&self) -> Result<Duration, Never> {
        Ok(self.before + self.started.elapsed())
    }

    pub fn done_if_felt(self) -> Result<(), Never> {
        let Ok(so_far) = self.so_far();

        match so_far >= FELT {
            true => {
                let Ok(()) = self.done();
            }
            false => {}
        }

        Ok(())
    }

    pub fn done(self) -> Result<(), Never> {
        let waited = self.before + self.started.elapsed();

        let load = match load() {
            Ok(load) => load,
            Err(fault) => {
                eprintln!("console-response-times: {fault}");
                0.0
            }
        };

        let Ok(at) = unix_now();
        let Ok(up) = uptime();

        let entry = line::Entry {
            at,
            up: up.unwrap_or_default().as_secs_f64(),
            load,
            who: self.who,
            what: self.what,
            waited,
            marks: self.marks,
            notes: self.notes,
        };

        let Ok(said) = line::written(&entry);
        let Ok(()) = writing::line(&said);

        Ok(())
    }
}

fn unix_now() -> Result<u64, Never> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs()))
}

pub fn uptime() -> Result<Option<Duration>, Never> {
    let said = match std::fs::read_to_string("/proc/uptime") {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };

    let first = match said.split_whitespace().next() {
        Some(first) => first,
        None => return Ok(None),
    };

    let seconds: f64 = match first.parse() {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    Ok(Some(Duration::from_secs_f64(seconds)))
}

pub fn load() -> Result<f64, String> {
    let said = std::fs::read_to_string("/proc/loadavg")
        .map_err(|fault| format!("/proc/loadavg: {fault}"))?;

    let first = match said.split_whitespace().next() {
        Some(first) => first,
        None => return Err("/proc/loadavg: it said nothing at all".to_string()),
    };

    first.parse().map_err(|fault| format!("/proc/loadavg: {first:?}: {fault}"))
}

pub fn since_exec() -> Result<Option<Duration>, Never> {
    let said = match std::fs::read_to_string("/proc/self/stat") {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };

    let Ok(began_at) = started_at(&said);

    let started = match began_at {
        Some(started) => started,
        None => return Ok(None),
    };

    // SAFETY: one call into libc that reads a constant and touches nothing.
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };

    match ticks <= 0 {
        true => return Ok(None),
        false => {}
    }

    let ticks_f = match i32::try_from(ticks) {
        Ok(ticks) => f64::from(ticks),
        Err(fault) => {
            eprintln!("console-response-times: the clock ticks {ticks} times a second: {fault}");

            return Ok(None);
        }
    };

    let began = Duration::from_secs_f64(started / ticks_f);

    let Ok(boot) = since_boot();

    let up = match boot {
        Some(up) => up,
        None => return Ok(None),
    };

    Ok(up.checked_sub(began))
}

fn since_boot() -> Result<Option<Duration>, Never> {
    let mut when = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: the struct is ours, initialised, and lives across the call.
    let asked = unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut when) };

    match asked == 0 {
        true => {}
        false => return Ok(None),
    }

    let (seconds, nanoseconds) = match (u64::try_from(when.tv_sec), u32::try_from(when.tv_nsec)) {
        (Ok(seconds), Ok(nanoseconds)) => (seconds, nanoseconds),
        (Err(_), _) | (_, Err(_)) => {
            eprintln!("console-response-times: the boot clock said {}s {}ns", when.tv_sec, when.tv_nsec);

            return Ok(None);
        }
    };

    Ok(Some(Duration::new(seconds, nanoseconds)))
}

pub fn started_at(stat: &str) -> Result<Option<f64>, Never> {
    let (_taken, after) = match stat.rsplit_once(')') {
        Some((_taken, after)) => (_taken, after),
        None => return Ok(None),
    };

    let field = match after.split_whitespace().nth(19) {
        Some(field) => field,
        None => return Ok(None),
    };

    match field.parse() {
        Ok(started) => Ok(Some(started)),
        Err(fault) => {
            eprintln!("console-response-times: /proc/self/stat: field 22 is {field:?}: {fault}");

            Ok(None)
        }
    }
}

fn since_press() -> Result<Option<Duration>, Never> {
    let raw = match std::env::var(PRESSED) {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };

    waited_since(&raw)
}

pub fn press_said() -> Result<Option<String>, Never> {
    let Ok(said) = asked(PRESSED);

    Ok(said.filter(|said| !said.trim().is_empty()))
}

pub fn from_said() -> Result<String, Never> {
    let Ok(came) = came_from();

    Ok(came.unwrap_or_default())
}

pub fn waited_since(raw: &str) -> Result<Option<Duration>, Never> {
    let stamped: u64 = match raw.trim().parse() {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    let Ok(clock) = monotonic_now();

    let now = match clock {
        Some(now) => now,
        None => return Ok(None),
    };

    let waited = match now.checked_sub(Duration::from_nanos(stamped)) {
        Some(waited) => waited,
        None => return Ok(None),
    };

    fresh(waited)
}

pub fn fresh(waited: Duration) -> Result<Option<Duration>, Never> {
    Ok(match waited > STALE {
        true => None,
        false => Some(waited),
    })
}

fn came_from() -> Result<Option<String>, Never> {
    let Ok(said) = asked(FROM);

    Ok(said.filter(|said| !said.is_empty()))
}

pub fn monotonic_now() -> Result<Option<Duration>, Never> {
    let mut when = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: the struct is ours, initialised, and lives across the call.
    let asked = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut when) };

    match asked == 0 {
        true => {}
        false => return Ok(None),
    }

    let (seconds, nanoseconds) = match (u64::try_from(when.tv_sec), u32::try_from(when.tv_nsec)) {
        (Ok(seconds), Ok(nanoseconds)) => (seconds, nanoseconds),
        (Err(_), _) | (_, Err(_)) => {
            eprintln!("console-response-times: the monotonic clock said {}s {}ns", when.tv_sec, when.tv_nsec);

            return Ok(None);
        }
    };

    Ok(Some(Duration::new(seconds, nanoseconds)))
}

pub fn press_stamp() -> Result<Option<(&'static str, String)>, Never> {
    let Ok(clock) = monotonic_now();

    let now = match clock {
        Some(now) => now,
        None => return Ok(None),
    };

    Ok(Some((PRESSED, now.as_nanos().to_string())))
}

pub fn this_program() -> Result<Option<String>, Never> {
    let argv0 = match std::env::args().next() {
        Some(argv0) => argv0,
        None => return Ok(None),
    };

    let at = PathBuf::from(argv0);

    let name = match at.file_name() {
        Some(name) => name,
        None => return Ok(None),
    };

    Ok(Some(name.to_string_lossy().to_string()))
}

pub fn pressed_here(starting: &mut Command) -> Result<(), Never> {
    let Ok(named) = this_program();

    let Ok(()) = pressed(starting, &named.unwrap_or_default());

    Ok(())
}

pub fn pressed(starting: &mut Command, from: &str) -> Result<(), Never> {
    match from.is_empty() {
        true => {
            let _ = starting.env_remove(FROM);
        }
        false => {
            let _ = starting.env(FROM, from);
        }
    }

    let Ok(stamp) = press_stamp();

    match stamp {
        Some((name, stamped)) => {
            let _ = starting.env(name, stamped);
        }
        None => {
            let _ = starting.env_remove(PRESSED);
        }
    }

    Ok(())
}

pub fn not_a_press(starting: &mut Command) -> Result<(), Never> {
    let _ = starting.env_remove(PRESSED);
    let _ = starting.env_remove(FROM);

    Ok(())
}

#[cfg(test)]
mod tests {
    use console_core_external_programs::Program;

    use super::*;

    #[test]
    fn when_a_process_began_is_read_past_a_name_with_spaces_in_it() {
        let fields: Vec<String> = (3..=22).map(|n| n.to_string()).collect();
        let stat = format!("1234 (a (odd) name) {}", fields.join(" "));
        assert_eq!(started_at(&stat), Ok(Some(22.0)));
    }

    #[test]
    fn a_line_that_is_not_a_stat_says_nothing() {
        assert_eq!(started_at(""), Ok(None));
        assert_eq!(started_at("1234 (launcher) S 4 5"), Ok(None));
    }

    #[test]
    fn the_machine_says_how_long_it_has_been_up_and_how_old_this_is() {
        let Ok(up) = uptime();
        let Ok(exec) = since_exec();
        let Ok(now) = monotonic_now();

        assert!(up.is_some_and(|up| up > Duration::ZERO));
        assert!(exec.is_some());
        assert!(now.is_some());
    }

    #[test]
    fn a_stamp_older_than_any_opening_is_not_a_press_at_all() {
        assert_eq!(fresh(STALE.saturating_add(Duration::from_secs(1))), Ok(None));
        assert_eq!(fresh(Duration::from_millis(40)), Ok(Some(Duration::from_millis(40))));
        assert_eq!(fresh(STALE), Ok(Some(STALE)), "the window itself is still inside it");
    }

    #[test]
    fn an_opening_that_nothing_stamped_says_nothing_about_a_press() {
        match std::env::var(PRESSED) {
            Ok(_) => {}
            Err(_) => {
                let Ok(waiting) = Waiting::on("a test", "opening");

                let named: Vec<&str> =
                    waiting.marks.iter().map(|(name, _)| name.as_str()).collect();
                assert!(!named.contains(&"press"), "a press nobody made was written down as zero");
                assert!(named.contains(&"exec"), "how long the process took to exist is knowable");
            }
        }
    }

    fn handed(starting: &Command) -> Vec<(String, Option<String>)> {
        starting
            .get_envs()
            .map(|(name, said)| {
                (
                    name.to_string_lossy().to_string(),
                    said.map(|said| said.to_string_lossy().to_string()),
                )
            })
            .collect()
    }

    #[test]
    fn a_start_that_is_a_press_hands_on_the_moment_and_where_it_came_from() {
        let Ok(mut starting) = Program::True.command();
        let Ok(()) = pressed(&mut starting, "pad");

        let handed = handed(&starting);
        let from = handed.iter().find(|(name, _)| name == FROM).and_then(|(_, said)| said.clone());
        let stamp =
            handed.iter().find(|(name, _)| name == PRESSED).and_then(|(_, said)| said.clone());
        assert_eq!(from.as_deref(), Some("pad"));
        assert!(stamp.is_some_and(|said| said.parse::<u64>().is_ok()), "the stamp is nanoseconds");
    }

    #[test]
    fn a_start_that_is_not_a_press_takes_both_marks_off_whatever_it_starts() {
        let Ok(mut starting) = Program::True.command();
        let Ok(()) = pressed(&mut starting, "pad");
        let Ok(()) = not_a_press(&mut starting);

        let handed = handed(&starting);

        for name in [PRESSED, FROM] {
            let said = handed.iter().find(|(had, _)| had == name);
            assert_eq!(
                said.map(|(_, said)| said.clone()),
                Some(None),
                "{name} was carried on to something nobody pressed"
            );
        }
    }

    #[test]
    fn a_press_stamp_reads_back_as_a_moment_that_has_already_gone() {
        let Ok(stamp) = press_stamp();
        let (word, stamped) = stamp.expect("this machine has a monotonic clock");

        assert_eq!(word, PRESSED);

        let then: u64 = stamped.parse().expect("nanoseconds");

        let Ok(clock) = monotonic_now();

        let now = clock.expect("a monotonic clock").as_nanos() as u64;

        assert!(now >= then, "the clock went backwards between two reads");
    }

    #[test]
    fn a_stretch_that_was_already_inside_the_exec_is_moved_out_of_it() {
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

        let Ok(()) = waiting.taking("screen", Duration::from_millis(20));

        let named: Vec<&str> = waiting.marks.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(named, ["press", "exec", "screen"]);
        assert_eq!(waiting.before, whole, "the total changed");
        assert_eq!(
            waiting.marks.iter().map(|(_, took)| *took).sum::<Duration>(),
            whole,
            "the stretches stopped adding up to the total"
        );
    }

    #[test]
    fn a_stretch_before_a_waiting_that_counts_nothing_before_it_is_added() {
        let Ok(mut waiting) = Waiting::here("a test", "opening");

        let Ok(()) = waiting.taking("looking", Duration::from_millis(5));

        assert_eq!(waiting.before, Duration::from_millis(5));
    }

    #[test]
    fn a_waiting_that_is_dropped_writes_nothing() {
        let Ok(store) = where_();
        let store = store.expect("a home to keep the times under");

        let before = store.metadata().map(|about| about.len()).unwrap_or(0);

        {
            let Ok(mut waiting) = Waiting::here("a test", "nothing");

            let Ok(()) = waiting.mark("thinking about it");
        }

        let after = store.metadata().map(|about| about.len()).unwrap_or(0);
        assert_eq!(before, after, "a dropped waiting wrote a line");
    }
}
