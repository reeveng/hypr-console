//! Saying something where someone who is not in a terminal sees it.
//!
//! Three programs raise notifications on this desktop and until now each of
//! them was a shell script that had worked out the same two things for itself.
//! `console-volume` said so in a comment -- *the same technique as
//! console-updating; two of them is not yet a pattern worth a file of its own* --
//! and there were three by then.
//!
//! The two things are these. A notification that replaces the one before it rather
//! than landing under it, which is what makes a rocker held down one card
//! rather than twenty. And a notification that stops repeating itself, because
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
use std::time::Duration;
#[cfg(test)]
use std::time::Instant;

use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_words::Words;
use console_waiting::{Schedule, Ready, Outcome, until_handed};
use rustix::process::getuid;

const NEVER_SAID_BEFORE: u32 = 0;


#[derive(Clone, Copy, Debug, Eq, PartialEq, Words)]
pub enum Urgency {
    #[words(said = "normal")]
    Normal,
    #[words(said = "critical")]
    Critical,
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
pub struct Notification {
    pub urgency: Urgency,
    pub expiry: Expiry,
    pub summary: String,
    pub body: String,
    pub replacing: Option<u32>,
    pub value: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Content<'a> {
    pub summary: &'a str,
    pub body: &'a str,
}

impl Notification {
    pub fn new(said: Content<'_>) -> Result<Self, Never> {
        Ok(Notification {
            urgency: Urgency::Normal,
            expiry: Expiry::Milliseconds(4000),
            summary: said.summary.to_string(),
            body: said.body.to_string(),
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

    pub fn arguments(&self) -> Result<Vec<String>, Never> {
        let Ok(urgency) = self.urgency.said();
        let Ok(expiry) = self.expiry.said();

        let mut arguments = vec![
            "notify-send".to_string(),
            "--app-name=Console".to_string(),
            "--print-id".to_string(),
            format!("--urgency={urgency}"),
            format!("--expire-time={expiry}"),
        ];

        match self.value {
            Some(value) => {
                arguments.push("-h".to_string());
                arguments.push(format!("int:value:{value}"));
            }
            None => {}
        }

        match self.replacing {
            Some(was) => arguments.push(format!("--replace-id={was}")),
            None => {}
        }

        arguments.push("--".to_string());
        arguments.push(self.summary.clone());
        arguments.push(self.body.clone());

        Ok(arguments)
    }
}

pub const LOUD: u32 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Shown,
    Last,
    Hidden,
}

pub fn visibility(count: u32) -> Result<Visibility, Never> {
    Ok(match count.cmp(&LOUD) {
        std::cmp::Ordering::Less => Visibility::Shown,
        std::cmp::Ordering::Equal => Visibility::Last,
        std::cmp::Ordering::Greater => Visibility::Hidden,
    })
}

pub fn last_of_them(body: &str) -> Result<String, Never> {
    let said = "Not shown again this session.";

    Ok(match body.is_empty() {
        true => said.to_string(),
        false => format!("{body} {said}"),
    })
}

pub fn fault(said: Content<'_>, count: u32) -> Result<Option<Notification>, Never> {
    let Ok(visibility) = visibility(count);

    match visibility {
        Visibility::Hidden => Ok(None),

        Visibility::Shown => {
            let Ok(notification) = alarming(said);

            Ok(Some(notification))
        }

        Visibility::Last => {
            let Ok(body) = last_of_them(said.body);
            let Ok(notification) = alarming(Content { summary: said.summary, body: &body });

            Ok(Some(notification))
        }
    }
}

fn alarming(said: Content<'_>) -> Result<Notification, Never> {
    let Ok(notification) = Notification::new(said);
    let Ok(notification) = notification.urgent();

    notification.staying()
}

pub fn once(said: Content<'_>, count: u32) -> Result<Option<Notification>, Never> {
    match count {
        1 => {
            let Ok(notification) = alarming(said);

            Ok(Some(notification))
        }
        _ => Ok(None),
    }
}

pub fn for_the_journal(kind: &str, said: Content<'_>) -> Result<String, Never> {
    let Content { summary, body } = said;

    Ok(match body.is_empty() {
        true => format!("{kind}: {summary}"),
        false => format!("{kind}: {summary} - {body}"),
    })
}

pub fn under() -> Result<PathBuf, Never> {
    let ours = console_core_places::runtime_ours()?;

Ok(match ours {
            Some(ours) => ours,
            None => {
                let mine = getuid();

                PathBuf::from(format!("/run/user/{mine}")).join(console_core_places::OURS)
            }
        })
}

pub struct StatePath(PathBuf);

impl StatePath {
    pub fn named(name: &str) -> Result<Self, Never> {
        let Ok(under) = under();

        Ok(StatePath(under.join(name)))
    }

    pub fn counting(kind: &str) -> Result<Self, Never> {
        let Ok(under) = under();

        Ok(StatePath(under.join("said").join(kind)))
    }

    pub fn read(&self) -> Result<Option<u32>, Never> {
        console_core_atomic_writes::number(&self.0)
    }

    pub fn write(&self, number: u32) -> Result<(), Never> {
        let _ = console_core_atomic_writes::whole_with_folders(&self.0, format!("{number}\n").as_bytes());

        Ok(())
    }

    pub fn forget(&self) -> Result<(), Never> {
        let _ = std::fs::remove_file(&self.0);

        Ok(())
    }

    pub fn again(&self) -> Result<u32, Never> {
        let Ok(read) = self.read();

        let before = match read {
            Some(before) => before,
            None => NEVER_SAID_BEFORE,
        };

        let now = before.saturating_add(1);

        let Ok(()) = self.write(now);

        Ok(now)
    }
}

const WAITING: Duration = Duration::from_secs(2);

pub fn raise(notification: &Notification) -> Result<Option<u32>, Never> {
    let Ok(arguments) = notification.arguments();
    let Ok(said) = said_within(&arguments, WAITING);

    let said = match said {
        Some(said) => said,
        None => return Ok(None),
    };

    let number = match said.trim().parse::<u32>() {
        Ok(number) => number,
        Err(_not_a_number) => return Ok(None),
    };

    Ok(Some(number))
}

fn said_within(arguments: &[String], waiting: Duration) -> Result<Option<String>, Never> {
    let (program, rest) = match arguments.split_first() {
        Some((program, rest)) => (program, rest),
        None => return Ok(None),
    };

    let mut running = match Command::new(program)
        .args(rest)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(running) => running,
        Err(_would_not_start) => return Ok(None),
    };

    let Ok(patience) = Schedule::asking_every(waiting, LOOKING);
    let Ok(ended) = until_handed(patience, &mut running, |running| {
        Ok(match running.try_wait() {
            Ok(Some(_)) => Ready::Yes,
            Ok(None) => Ready::NotYet,
            Err(_nothing_can_be_asked_about_it) => Ready::Yes,
        })
    });

    match ended {
        Outcome::Happened => {
            let said = match running.wait_with_output() {
                Ok(said) => said,
                Err(_would_not_wait) => return Ok(None),
            };

            return Ok(Some(String::from_utf8_lossy(&said.stdout).into_owned()));
        }
        Outcome::RanOut => {},
    }

    let _ = running.kill();
    let _ = running.wait();

    Ok(None)
}

const LOOKING: Duration = Duration::from_millis(20);

pub fn raise_kept(notification: Notification, kept: &StatePath) -> Result<(), Never> {
    let Ok(read) = kept.read();
    let Ok(notification) = notification.replacing(read);
    let Ok(raised) = raise(&notification);

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

pub fn withdraw(kept: &StatePath) -> Result<(), Never> {
    let Ok(read) = kept.read();

    let number = match read {
        Some(number) => number,
        None => return Ok(()),
    };

    let Ok(closing) = closing(number);
    let Ok(_) = said_within(&closing, WAITING);
    let Ok(()) = kept.forget();

    Ok(())
}

pub fn journal(said: &str) -> Result<(), Never> {
    let Ok(logger) = Program::Logger.name();

    let arguments = [logger, "-t", "console", "-p", "user.warning", "--", said]
        .map(str::to_string)
        .to_vec();

    let Ok(_) = said_within(&arguments, WAITING);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn something_that_will_not_answer_is_not_waited_on_for_ever() {
        let Ok(sh) = Program::Sh.name();

        let arguments = [sh, "-c", "exec sleep 30"].map(str::to_string).to_vec();
        let began = Instant::now();
        let said = said_within(&arguments, Duration::from_millis(200));
        assert_eq!(said, Ok(None), "a program that had to be killed said nothing");
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "waited {:?}, which is waiting for ever with extra steps",
            began.elapsed()
        );
    }

    #[test]
    fn something_that_answers_in_time_is_heard() {
        let Ok(arguments) = Program::Echo.arguments(&["41"]);

        let Ok(said) = said_within(&arguments, Duration::from_secs(5));

        assert_eq!(said.as_deref().map(str::trim), Some("41"));
    }

    #[test]
    fn a_program_that_does_not_exist_is_answered_at_once() {
        let arguments = ["console-nothing-is-called-this".to_string()];
        assert_eq!(said_within(&arguments, Duration::from_secs(30)), Ok(None));
    }

    #[test]
    fn the_first_few_are_shown_and_the_rest_are_the_journals() {
        assert_eq!(visibility(1), Ok(Visibility::Shown));
        assert_eq!(visibility(LOUD - 1), Ok(Visibility::Shown));
        assert_eq!(visibility(LOUD), Ok(Visibility::Last));
        assert_eq!(visibility(LOUD + 1), Ok(Visibility::Hidden));
        assert_eq!(visibility(200), Ok(Visibility::Hidden));
    }

    #[test]
    fn the_last_one_says_it_is_the_last_one() {
        let Ok(fault) = fault(Content { summary: "The picture would not delete", body: "" }, LOUD);
        let notification = fault.expect("the last");
        assert!(notification.body.contains("Not shown again"));
    }

    #[test]
    fn the_last_ones_sentence_comes_after_what_the_fault_said() {
        let said = Content { summary: "Closed wrong", body: "The folder is read-only." };
        let Ok(fault) = fault(said, LOUD);
        let notification = fault.expect("the last");
        assert!(notification.body.starts_with("The folder is read-only."));
    }

    #[test]
    fn nothing_is_shown_once_the_screen_has_had_enough() {
        assert_eq!(fault(Content { summary: "Closed wrong", body: "again" }, LOUD + 1), Ok(None));
    }

    #[test]
    fn a_fault_stays_on_the_screen() {
        let Ok(fault) = fault(Content { summary: "Closed wrong", body: "" }, 1);
        let notification = fault.expect("the first");
        assert_eq!(notification.expiry, Expiry::Stays);
        assert_eq!(notification.urgency, Urgency::Critical);
    }

    #[test]
    fn the_journal_is_told_the_kind_as_well_as_what_happened() {
        assert_eq!(
            for_the_journal("unit-x", Content { summary: "x stopped", body: "why" }),
            Ok("unit-x: x stopped - why".to_string())
        );
        assert_eq!(
            for_the_journal("unit-x", Content { summary: "x stopped", body: "" }),
            Ok("unit-x: x stopped".to_string())
        );
    }

    #[test]
    fn a_notification_that_replaces_another_says_which() {
        let Ok(notification) = Notification::new(Content { summary: "Volume 40%", body: "" });
        let Ok(notification) = notification.replacing(Some(17));
        let Ok(arguments) = notification.arguments();

        assert!(arguments.contains(&"--replace-id=17".to_string()));
    }

    #[test]
    fn a_notification_that_replaces_nothing_asks_to_replace_nothing() {
        let Ok(notification) = Notification::new(Content { summary: "Volume 40%", body: "" });
        let Ok(arguments) = notification.arguments();

        assert!(!arguments.iter().any(|word| word.starts_with("--replace-id")));
    }

    #[test]
    fn what_was_said_is_held_off_from_the_options() {
        let Ok(notification) = Notification::new(Content { summary: "--urgent", body: "-h" });
        let Ok(arguments) = notification.arguments();
        let after: Vec<&String> = arguments.iter().skip_while(|word| *word != "--").skip(1).collect();
        assert_eq!(after, ["--urgent", "-h"]);
    }

    #[test]
    fn a_reading_carries_its_number_for_anything_that_can_draw_one() {
        let Ok(notification) = Notification::new(Content { summary: "Volume 40%", body: "" });
        let Ok(notification) = notification.valued(40);
        let Ok(arguments) = notification.arguments();

        assert!(arguments.contains(&"int:value:40".to_string()));
    }

    #[test]
    fn how_long_it_stays_is_said_the_way_notify_send_reads_it() {
        let Ok(notification) = Notification::new(Content { summary: "a", body: "" });
        let Ok(staying) = notification.clone().staying();
        let Ok(staying) = staying.arguments();
        let Ok(lasting) = notification.lasting(1500);
        let Ok(lasting) = lasting.arguments();

        assert!(staying.contains(&"--expire-time=0".to_string()));
        assert!(lasting.contains(&"--expire-time=1500".to_string()));
    }

    #[test]
    fn taking_a_card_down_names_it_by_the_number_it_came_back_under() {
        let Ok(arguments) = closing(7);

        assert_eq!(arguments.last().map(String::as_str), Some("7"));
        assert!(arguments.contains(&"CloseNotification".to_string()), "{arguments:?}");
        assert!(arguments.contains(&"u".to_string()), "the number goes out untyped: {arguments:?}");
    }

    #[test]
    fn a_card_no_one_kept_a_number_for_is_left_alone() {
        let at = std::env::temp_dir()
            .join(format!("console-withdraw-{}-{}", std::process::id(), line!()));
        let _ = std::fs::remove_file(&at);

        let kept = StatePath(at.clone());
        let Ok(()) = withdraw(&kept);

        assert_eq!(kept.read(), Ok(None));
        assert!(!at.exists(), "a withdrawal wrote a file where there was nothing to withdraw");
    }
}
