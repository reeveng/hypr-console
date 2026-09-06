//! Saying something where somebody who is not in a terminal sees it.
//!
//! Three programs raise notifications on this desktop and until now each of
//! them was a shell script that had worked out the same two things for itself.
//! `console-volume` said so in a comment -- *the same technique as
//! console-updating; two of them is not yet a pattern worth a file of its own* --
//! and there were three by then.
//!
//! The two things are these. A notice that replaces the one before it rather
//! than landing under it, which is what makes a rocker held down one card
//! rather than twenty. And a notice that stops repeating itself, because
//! everything that raises one here is inside a loop of some sort and the way a
//! machine shouting over itself ends is with the notifications turned off and
//! the fault still there.
//!
//! What is decided and what is done are kept apart, as everywhere else here.
//! Whether a thing is worth showing, what it should say and what it replaces
//! are functions of what was counted; raising it is the only part that needs a
//! daemon, and nothing below it can be asked without one.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use console_external_programs::Program;
use console_never::Never;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Urgency {
    Normal,
    Critical,
}

impl Urgency {
    fn said(self) -> Result<&'static str, Never> {
        Ok(match self {
            Urgency::Normal => "normal",
            Urgency::Critical => "critical",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Expiry {
    Stays,
    Milliseconds(u32),
}

impl Expiry {
    fn said(self) -> Result<String, Never> {
        Ok(match self {
            Expiry::Stays => "0".to_string(),
            Expiry::Milliseconds(many) => many.to_string(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Notice {
    pub urgency: Urgency,
    pub expiry: Expiry,
    pub summary: String,
    pub body: String,
    pub replacing: Option<u32>,
    pub value: Option<i64>,
}

impl Notice {
    pub fn new(summary: &str, body: &str) -> Result<Self, Never> {
        Ok(Notice {
            urgency: Urgency::Normal,
            expiry: Expiry::Milliseconds(4000),
            summary: summary.to_string(),
            body: body.to_string(),
            replacing: None,
            value: None,
        })
    }

    pub fn urgent(mut self) -> Result<Self, Never> {
        self.urgency = Urgency::Critical;

        Ok(self)
    }

    pub fn staying(mut self) -> Result<Self, Never> {
        self.expiry = Expiry::Stays;

        Ok(self)
    }

    pub fn lasting(mut self, milliseconds: u32) -> Result<Self, Never> {
        self.expiry = Expiry::Milliseconds(milliseconds);

        Ok(self)
    }

    pub fn replacing(mut self, was: Option<u32>) -> Result<Self, Never> {
        self.replacing = was;

        Ok(self)
    }

    pub fn valued(mut self, value: i64) -> Result<Self, Never> {
        self.value = Some(value);

        Ok(self)
    }

    pub fn argv(&self) -> Result<Vec<String>, Never> {
        let Ok(urgency) = self.urgency.said();
        let Ok(expiry) = self.expiry.said();

        let mut argv = vec![
            "notify-send".to_string(),
            "--app-name=Console".to_string(),
            "--print-id".to_string(),
            format!("--urgency={urgency}"),
            format!("--expire-time={expiry}"),
        ];

        match self.value {
            Some(value) => {
                argv.push("-h".to_string());
                argv.push(format!("int:value:{value}"));
            }
            None => {}
        }

        match self.replacing {
            Some(was) => argv.push(format!("--replace-id={was}")),
            None => {}
        }

        argv.push("--".to_string());
        argv.push(self.summary.clone());
        argv.push(self.body.clone());

        Ok(argv)
    }
}

pub const LOUD: u32 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Showing {
    Shown,
    Last,
    Quiet,
}

pub fn showing(count: u32) -> Result<Showing, Never> {
    Ok(match count {
        count if count < LOUD => Showing::Shown,
        count if count == LOUD => Showing::Last,
        _ => Showing::Quiet,
    })
}

pub fn last_of_them(body: &str) -> Result<String, Never> {
    let said = "Not shown again this session.";

    Ok(match body.is_empty() {
        true => said.to_string(),
        false => format!("{body} {said}"),
    })
}

pub fn fault(summary: &str, body: &str, count: u32) -> Result<Option<Notice>, Never> {
    let Ok(showing) = showing(count);

    match showing {
        Showing::Quiet => Ok(None),

        Showing::Shown => {
            let Ok(notice) = alarming(summary, body);

            Ok(Some(notice))
        }

        Showing::Last => {
            let Ok(body) = last_of_them(body);
            let Ok(notice) = alarming(summary, &body);

            Ok(Some(notice))
        }
    }
}

fn alarming(summary: &str, body: &str) -> Result<Notice, Never> {
    let Ok(notice) = Notice::new(summary, body);
    let Ok(notice) = notice.urgent();

    notice.staying()
}

pub fn once(summary: &str, body: &str, count: u32) -> Result<Option<Notice>, Never> {
    match count {
        1 => {
            let Ok(notice) = alarming(summary, body);

            Ok(Some(notice))
        }
        _ => Ok(None),
    }
}

pub fn for_the_journal(kind: &str, summary: &str, body: &str) -> Result<String, Never> {
    Ok(match body.is_empty() {
        true => format!("{kind}: {summary}"),
        false => format!("{kind}: {summary} - {body}"),
    })
}

pub fn under() -> Result<PathBuf, Never> {
    // SAFETY: `getuid` reads this process's own real user id out of the
    let mine = unsafe { libc::getuid() };
    let run = match std::env::var("XDG_RUNTIME_DIR") {
        Ok(run) => run,

        Err(_) => format!("/run/user/{mine}"),
    };

    Ok(PathBuf::from(run).join("console"))
}

pub struct Kept(PathBuf);

impl Kept {
    pub fn named(name: &str) -> Result<Self, Never> {
        let Ok(under) = under();

        Ok(Kept(under.join(name)))
    }

    pub fn counting(kind: &str) -> Result<Self, Never> {
        let Ok(under) = under();

        Ok(Kept(under.join("said").join(kind)))
    }

    pub fn read(&self) -> Result<Option<u32>, Never> {
        let Ok(said) = std::fs::read_to_string(&self.0) else {
            return Ok(None);
        };

        let Ok(number) = said.trim().parse::<u32>() else {
            return Ok(None);
        };

        Ok(Some(number))
    }

    pub fn write(&self, number: u32) -> Result<(), Never> {
        match self.0.parent() {
            Some(above) => {
                let _ = std::fs::create_dir_all(above);
            }
            None => {}
        }

        let _ = std::fs::write(&self.0, format!("{number}\n"));

        Ok(())
    }

    pub fn forget(&self) -> Result<(), Never> {
        let _ = std::fs::remove_file(&self.0);

        Ok(())
    }

    pub fn again(&self) -> Result<u32, Never> {
        let Ok(read) = self.read();

        let now = read.unwrap_or(0).saturating_add(1);

        let Ok(()) = self.write(now);

        Ok(now)
    }
}

const WAITING: Duration = Duration::from_secs(2);

pub fn raise(notice: &Notice) -> Result<Option<u32>, Never> {
    let Ok(argv) = notice.argv();
    let Ok(said) = said_within(&argv, WAITING);

    let Some(said) = said else {
        return Ok(None);
    };

    let Ok(number) = said.trim().parse::<u32>() else {
        return Ok(None);
    };

    Ok(Some(number))
}

fn said_within(argv: &[String], waiting: Duration) -> Result<Option<String>, Never> {
    let Some((program, rest)) = argv.split_first() else {
        return Ok(None);
    };

    let Ok(mut running) = Command::new(program)
        .args(rest)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return Ok(None);
    };

    let by = Instant::now() + waiting;

    while Instant::now() < by {
        match running.try_wait() {
            Ok(Some(_)) => {
                let Ok(said) = running.wait_with_output() else {
                    return Ok(None);
                };

                return Ok(Some(String::from_utf8_lossy(&said.stdout).into_owned()));
            }
            Ok(None) => std::thread::sleep(LOOKING),
            Err(_) => return Ok(None),
        }
    }

    let _ = running.kill();
    let _ = running.wait();

    Ok(None)
}

const LOOKING: Duration = Duration::from_millis(20);

pub fn raise_kept(notice: Notice, kept: &Kept) -> Result<(), Never> {
    let Ok(read) = kept.read();
    let Ok(notice) = notice.replacing(read);
    let Ok(raised) = raise(&notice);

    match raised {
        Some(number) => {
            let Ok(()) = kept.write(number);
        }
        None => {}
    }

    Ok(())
}

pub fn closing(number: u32) -> Result<Vec<String>, Never> {
    let number = number.to_string();

    Ok([
        "busctl",
        "--user",
        "call",
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
        "CloseNotification",
        "u",
        number.as_str(),
    ]
    .map(str::to_string)
    .to_vec())
}

pub fn withdraw(kept: &Kept) -> Result<(), Never> {
    let Ok(read) = kept.read();

    let Some(number) = read else {
        return Ok(());
    };

    let Ok(closing) = closing(number);
    let Ok(_) = said_within(&closing, WAITING);
    let Ok(()) = kept.forget();

    Ok(())
}

pub fn journal(said: &str) -> Result<(), Never> {
    let Ok(logger) = Program::Logger.name();

    let argv = [logger, "-t", "console", "-p", "user.warning", "--", said]
        .map(str::to_string)
        .to_vec();

    let Ok(_) = said_within(&argv, WAITING);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn something_that_will_not_answer_is_not_waited_on_for_ever() {
        let Ok(sh) = Program::Sh.name();

        let argv = [sh, "-c", "exec sleep 30"].map(str::to_string).to_vec();
        let began = Instant::now();
        let said = said_within(&argv, Duration::from_millis(200));
        assert_eq!(said, Ok(None), "a program that had to be killed said nothing");
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "waited {:?}, which is waiting for ever with extra steps",
            began.elapsed()
        );
    }

    #[test]
    fn something_that_answers_in_time_is_heard() {
        let Ok(argv) = Program::Echo.argv(&["41"]);

        let Ok(said) = said_within(&argv, Duration::from_secs(5));

        assert_eq!(said.as_deref().map(str::trim), Some("41"));
    }

    #[test]
    fn a_program_that_does_not_exist_is_answered_at_once() {
        let argv = ["console-nothing-is-called-this".to_string()];
        assert_eq!(said_within(&argv, Duration::from_secs(30)), Ok(None));
    }

    #[test]
    fn the_first_few_are_shown_and_the_rest_are_the_journals() {
        assert_eq!(showing(1), Ok(Showing::Shown));
        assert_eq!(showing(LOUD - 1), Ok(Showing::Shown));
        assert_eq!(showing(LOUD), Ok(Showing::Last));
        assert_eq!(showing(LOUD + 1), Ok(Showing::Quiet));
        assert_eq!(showing(200), Ok(Showing::Quiet));
    }

    #[test]
    fn the_last_one_says_it_is_the_last_one() {
        let Ok(fault) = fault("The picture would not delete", "", LOUD);
        let notice = fault.expect("the last");
        assert!(notice.body.contains("Not shown again"));
    }

    #[test]
    fn the_last_ones_sentence_comes_after_what_the_fault_said() {
        let Ok(fault) = fault("Gone wrong", "The folder is read-only.", LOUD);
        let notice = fault.expect("the last");
        assert!(notice.body.starts_with("The folder is read-only."));
    }

    #[test]
    fn nothing_is_shown_once_the_screen_has_had_enough() {
        assert_eq!(fault("Gone wrong", "again", LOUD + 1), Ok(None));
    }

    #[test]
    fn a_fault_stays_on_the_screen() {
        let Ok(fault) = fault("Gone wrong", "", 1);
        let notice = fault.expect("the first");
        assert_eq!(notice.expiry, Expiry::Stays);
        assert_eq!(notice.urgency, Urgency::Critical);
    }

    #[test]
    fn the_journal_is_told_the_kind_as_well_as_what_happened() {
        assert_eq!(
            for_the_journal("unit-x", "x stopped", "why"),
            Ok("unit-x: x stopped - why".to_string())
        );
        assert_eq!(
            for_the_journal("unit-x", "x stopped", ""),
            Ok("unit-x: x stopped".to_string())
        );
    }

    #[test]
    fn a_notice_that_replaces_another_says_which() {
        let Ok(notice) = Notice::new("Volume 40%", "");
        let Ok(notice) = notice.replacing(Some(17));
        let Ok(argv) = notice.argv();

        assert!(argv.contains(&"--replace-id=17".to_string()));
    }

    #[test]
    fn a_notice_that_replaces_nothing_asks_to_replace_nothing() {
        let Ok(notice) = Notice::new("Volume 40%", "");
        let Ok(argv) = notice.argv();

        assert!(!argv.iter().any(|word| word.starts_with("--replace-id")));
    }

    #[test]
    fn what_was_said_is_held_off_from_the_options() {
        let Ok(notice) = Notice::new("--urgent", "-h");
        let Ok(argv) = notice.argv();
        let end = argv.iter().position(|word| word == "--").expect("the end of the options");
        assert_eq!(&argv[end + 1..], ["--urgent", "-h"]);
    }

    #[test]
    fn a_reading_carries_its_number_for_anything_that_can_draw_one() {
        let Ok(notice) = Notice::new("Volume 40%", "");
        let Ok(notice) = notice.valued(40);
        let Ok(argv) = notice.argv();

        assert!(argv.contains(&"int:value:40".to_string()));
    }

    #[test]
    fn how_long_it_stays_is_said_the_way_notify_send_reads_it() {
        let Ok(notice) = Notice::new("a", "");
        let Ok(staying) = notice.clone().staying();
        let Ok(staying) = staying.argv();
        let Ok(lasting) = notice.lasting(1500);
        let Ok(lasting) = lasting.argv();

        assert!(staying.contains(&"--expire-time=0".to_string()));
        assert!(lasting.contains(&"--expire-time=1500".to_string()));
    }

    #[test]
    fn taking_a_card_down_names_it_by_the_number_it_came_back_under() {
        let Ok(argv) = closing(7);

        assert_eq!(argv.last().map(String::as_str), Some("7"));
        assert!(argv.contains(&"CloseNotification".to_string()), "{argv:?}");
        assert!(argv.contains(&"u".to_string()), "the number goes out untyped: {argv:?}");
    }

    #[test]
    fn a_card_nobody_kept_a_number_for_is_left_alone() {
        let at = std::env::temp_dir()
            .join(format!("console-withdraw-{}-{}", std::process::id(), line!()));
        let _ = std::fs::remove_file(&at);

        let kept = Kept(at.clone());
        let Ok(()) = withdraw(&kept);

        assert_eq!(kept.read(), Ok(None));
        assert!(!at.exists(), "a withdrawal wrote a file where there was nothing to withdraw");
    }
}
