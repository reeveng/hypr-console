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

/// How loudly a notice asks to be looked at.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Urgency {
    Normal,
    /// The screen's own word for it, which mako draws differently.
    Critical,
}

impl Urgency {
    fn said(self) -> &'static str {
        match self {
            Urgency::Normal => "normal",
            Urgency::Critical => "critical",
        }
    }
}

/// How long a notice stays on the screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Expiry {
    /// Until somebody takes it away, which is what an unfinished thing wants.
    Stays,
    Milliseconds(u32),
}

impl Expiry {
    fn said(self) -> String {
        match self {
            Expiry::Stays => "0".to_string(),
            Expiry::Milliseconds(many) => many.to_string(),
        }
    }
}

/// One notice, said without being raised.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Notice {
    pub urgency: Urgency,
    pub expiry: Expiry,
    pub summary: String,
    pub body: String,
    /// The number of the notice this one stands in place of, where there is
    /// one. Without it, the second call is a second card under a first that
    /// never goes, which is worse than saying nothing.
    pub replacing: Option<u32>,
    /// A number from nought to a hundred, drawn as a length.
    ///
    /// mako fills the card to that proportion in `progress-color`, which the
    /// theme sets to pink: the one place a colour on a notification is read as
    /// a length rather than as a colour, because nothing has to stay legible
    /// on top of it. The volume rocker and the screen both send it, and the
    /// sentence carries the figure so two adjacent presses can be told apart.
    pub value: Option<i64>,
}

impl Notice {
    pub fn new(summary: &str, body: &str) -> Self {
        Notice {
            urgency: Urgency::Normal,
            expiry: Expiry::Milliseconds(4000),
            summary: summary.to_string(),
            body: body.to_string(),
            replacing: None,
            value: None,
        }
    }

    pub fn urgent(mut self) -> Self {
        self.urgency = Urgency::Critical;
        self
    }

    pub fn staying(mut self) -> Self {
        self.expiry = Expiry::Stays;
        self
    }

    pub fn lasting(mut self, milliseconds: u32) -> Self {
        self.expiry = Expiry::Milliseconds(milliseconds);
        self
    }

    pub fn replacing(mut self, was: Option<u32>) -> Self {
        self.replacing = was;
        self
    }

    pub fn valued(mut self, value: i64) -> Self {
        self.value = Some(value);
        self
    }

    /// The whole command line, worked out and not run.
    ///
    /// Said as words so a test can read what would be raised without a daemon
    /// to raise it at, which is the one thing none of the three scripts could
    /// be asked.
    pub fn argv(&self) -> Vec<String> {
        let mut argv = vec![
            "notify-send".to_string(),
            "--app-name=Console".to_string(),
            "--print-id".to_string(),
            format!("--urgency={}", self.urgency.said()),
            format!("--expire-time={}", self.expiry.said()),
        ];
        if let Some(value) = self.value {
            argv.push("-h".to_string());
            argv.push(format!("int:value:{value}"));
        }
        if let Some(was) = self.replacing {
            argv.push(format!("--replace-id={was}"));
        }
        argv.push("--".to_string());
        argv.push(self.summary.clone());
        argv.push(self.body.clone());
        argv
    }
}

// ------------------------------------------------------------------- counting

/// How many times one kind of fault is put on the screen in a session.
///
/// The first few tell somebody something is wrong. The two hundredth is a
/// machine shouting over itself.
pub const LOUD: u32 = 5;

/// What to do with a notice, given how many times its kind has been raised.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Showing {
    /// Put it on the screen.
    Shown,
    /// Put it on the screen, saying that this is the last of them.
    Last,
    /// The journal has it and the screen has had enough.
    Quiet,
}

/// Whether this one is shown, given that it is the `count`th of its kind.
///
/// Counting from one: the first call is `1`.
pub fn showing(count: u32) -> Showing {
    match count {
        count if count < LOUD => Showing::Shown,
        count if count == LOUD => Showing::Last,
        _ => Showing::Quiet,
    }
}

/// What the body says, given that this is the last time it will be shown.
///
/// A notice that simply stops is a fault that looks like it went away. This is
/// the sentence that says otherwise, and it is added rather than replacing
/// anything, so what the fault said is still the first thing read.
pub fn last_of_them(body: &str) -> String {
    let said = "This is the last time it will be shown this session.";
    match body.is_empty() {
        true => said.to_string(),
        false => format!("{body} {said}"),
    }
}

/// The notice for a fault of this kind, or nothing if the screen has had enough.
pub fn fault(summary: &str, body: &str, count: u32) -> Option<Notice> {
    match showing(count) {
        Showing::Quiet => None,
        Showing::Shown => Some(Notice::new(summary, body).urgent().staying()),
        Showing::Last => Some(Notice::new(summary, &last_of_them(body)).urgent().staying()),
    }
}

/// What the journal is told, which is everything, however quiet the screen is.
pub fn for_the_journal(kind: &str, summary: &str, body: &str) -> String {
    match body.is_empty() {
        true => format!("{kind}: {summary}"),
        false => format!("{kind}: {summary} - {body}"),
    }
}

// ---------------------------------------------------------------- where it is

/// The runtime directory, which is where all of this lives.
///
/// A fresh session is a fresh five, and a reboot forgets the whole count. That
/// is the same lifetime as the fault it is counting.
pub fn under() -> PathBuf {
    let run = std::env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
    PathBuf::from(run).join("console")
}

/// A number kept between one call and the next, under a name of its own.
///
/// Two things are kept this way: how many times a kind of fault has been said,
/// and the number the notice a rocker is replacing came back under.
pub struct Kept(PathBuf);

impl Kept {
    pub fn named(name: &str) -> Self {
        Kept(under().join(name))
    }

    /// The same, one directory down, for the counts which are one file each.
    pub fn counting(kind: &str) -> Self {
        Kept(under().join("said").join(kind))
    }

    pub fn read(&self) -> Option<u32> {
        std::fs::read_to_string(&self.0).ok()?.trim().parse().ok()
    }

    /// Nothing here is worth failing the thing that called it, so a directory
    /// that cannot be made is a number that is not kept and no more than that.
    pub fn write(&self, number: u32) {
        if let Some(above) = self.0.parent() {
            let _ = std::fs::create_dir_all(above);
        }
        let _ = std::fs::write(&self.0, format!("{number}\n"));
    }

    pub fn forget(&self) {
        let _ = std::fs::remove_file(&self.0);
    }

    /// One more than what was there, written back, and handed to the caller.
    pub fn again(&self) -> u32 {
        let now = self.read().unwrap_or(0).saturating_add(1);
        self.write(now);
        now
    }
}

// --------------------------------------------------------------------- doing

/// How long a notice is given to be taken.
///
/// Raising one is a D-Bus call to whatever is drawing notifications, made and
/// waited on. A daemon that is there answers in the time it takes to draw; a
/// daemon that is not there is waited on until the bus gives up, which is tens
/// of seconds.
///
/// That wait is not free and it is not rare, because of when it happens. The
/// notices this raises are about pieces of the desktop falling over, and the
/// way a piece of the desktop most often falls over is the whole session going
/// away underneath it -- at which point the thing that draws notifications has
/// gone away too. So the one moment there is genuinely something to say is the
/// moment nothing is listening.
///
/// Every unit here runs `console-fell` as `ExecStopPost`, and `ExecStopPost=-`
/// forgives a program that fails, not one that hangs. So a session tearing
/// down waited on this, eleven times over, and systemd killed each one with
/// `State 'stop-post' timed out. Terminating.` Seen on the device when
/// hyprsunset went down with the compositor.
///
/// Two seconds because a notification daemon that is alive answers in
/// milliseconds, so this is only ever waited out when the answer was never
/// coming.
const WAITING: Duration = Duration::from_secs(2);

/// Put it on the screen, and say what number it came back under.
///
/// Never the reason anything fails. Every caller here is already doing
/// something more important than saying so, and the journal has what was said
/// either way -- including when the screen could not be reached at all, which
/// is the case this is careful about.
pub fn raise(notice: &Notice) -> Option<u32> {
    let argv = notice.argv();
    said_within(&argv, WAITING)?.trim().parse().ok()
}

/// Run something, and stop waiting for it after a while.
///
/// `Command::output` waits as long as it takes, and there is no asking it not
/// to. So this starts the program, looks at it now and then, and kills it if
/// the time runs out -- then waits on the corpse, because a child nobody reaps
/// is a child that stays.
///
/// Nothing where it was killed. A program that had to be stopped said nothing
/// worth reading, and a caller told "no answer" behaves the way it should:
/// gets on with what it was doing.
fn said_within(argv: &[String], waiting: Duration) -> Option<String> {
    let mut running = Command::new(&argv[0])
        .args(&argv[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let by = Instant::now() + waiting;
    while Instant::now() < by {
        match running.try_wait() {
            Ok(Some(_)) => {
                let said = running.wait_with_output().ok()?;
                return Some(String::from_utf8_lossy(&said.stdout).into_owned());
            }
            Ok(None) => std::thread::sleep(LOOKING),
            Err(_) => return None,
        }
    }
    let _ = running.kill();
    let _ = running.wait();
    None
}

/// How often the waiting looks.
///
/// Short enough that a daemon answering at once is not held up by the looking,
/// long enough that the looking is not the work.
const LOOKING: Duration = Duration::from_millis(20);

/// Raise it and keep the number, so the next one replaces this one.
pub fn raise_kept(notice: Notice, kept: &Kept) {
    let notice = notice.replacing(kept.read());
    if let Some(number) = raise(&notice) {
        kept.write(number);
    }
}

/// Tell the journal, which is told whatever the screen is doing.
///
/// Bounded like the raising, and for the same reason rather than for the same
/// risk. journald is a system service and does not go away when a session
/// does, so this is the safer of the two by a long way. But it is called from
/// the same place at the same moment -- `console-fell`, as a unit stops -- and
/// a path that must not hang should not have one half of it that cannot and
/// one half that merely probably will not.
pub fn journal(said: &str) {
    let argv = ["logger", "-t", "console", "-p", "user.warning", "--", said]
        .map(str::to_string)
        .to_vec();
    said_within(&argv, WAITING);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole of the fault, held to a program that will not answer.
    ///
    /// Raising a notice is a call to a daemon and a wait for its reply, and the
    /// notices that matter most are raised when the session is going away --
    /// which is exactly when that daemon has gone. Waited on without a bound,
    /// eleven units each held their own stop open until systemd killed the
    /// thing they were waiting for.
    #[test]
    fn something_that_will_not_answer_is_not_waited_on_for_ever() {
        // Through `sh`, and `exec` so that sh becomes the sleep rather than
        // parenting it: killed, it is the whole of what was started that goes,
        // with nothing left running behind the test.
        let argv = ["sh", "-c", "exec sleep 30"].map(str::to_string).to_vec();
        let began = Instant::now();
        let said = said_within(&argv, Duration::from_millis(200));
        assert_eq!(said, None, "a program that had to be killed said nothing");
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "waited {:?}, which is waiting for ever with extra steps",
            began.elapsed()
        );
    }

    /// And something that answers is heard, rather than the bound being a way
    /// of not listening.
    #[test]
    fn something_that_answers_in_time_is_heard() {
        let argv = ["echo".to_string(), "41".to_string()];
        let said = said_within(&argv, Duration::from_secs(5));
        assert_eq!(said.as_deref().map(str::trim), Some("41"));
    }

    /// A program that is not there is not there, and is not waited on either.
    #[test]
    fn a_program_that_does_not_exist_is_answered_at_once() {
        let argv = ["console-nothing-is-called-this".to_string()];
        assert_eq!(said_within(&argv, Duration::from_secs(30)), None);
    }

    #[test]
    fn the_first_few_are_shown_and_the_rest_are_the_journals() {
        assert_eq!(showing(1), Showing::Shown);
        assert_eq!(showing(LOUD - 1), Showing::Shown);
        assert_eq!(showing(LOUD), Showing::Last);
        assert_eq!(showing(LOUD + 1), Showing::Quiet);
        assert_eq!(showing(200), Showing::Quiet);
    }

    /// A notice that simply stops is a fault that looks like it went away.
    #[test]
    fn the_last_one_says_it_is_the_last_one() {
        let notice = fault("The picture would not delete", "", LOUD).expect("the last");
        assert!(notice.body.contains("last time"));
    }

    #[test]
    fn the_last_ones_sentence_comes_after_what_the_fault_said() {
        let notice = fault("Gone wrong", "The folder is read-only.", LOUD).expect("the last");
        assert!(notice.body.starts_with("The folder is read-only."));
    }

    #[test]
    fn nothing_is_shown_once_the_screen_has_had_enough() {
        assert_eq!(fault("Gone wrong", "again", LOUD + 1), None);
    }

    /// A fault stands until somebody takes it away. It is the one kind of
    /// notice here that must not go by itself.
    #[test]
    fn a_fault_stays_on_the_screen() {
        let notice = fault("Gone wrong", "", 1).expect("the first");
        assert_eq!(notice.expiry, Expiry::Stays);
        assert_eq!(notice.urgency, Urgency::Critical);
    }

    #[test]
    fn the_journal_is_told_the_kind_as_well_as_what_happened() {
        assert_eq!(for_the_journal("unit-x", "x stopped", "why"), "unit-x: x stopped - why");
        assert_eq!(for_the_journal("unit-x", "x stopped", ""), "unit-x: x stopped");
    }

    /// The whole of what makes a rocker held down one card rather than twenty.
    #[test]
    fn a_notice_that_replaces_another_says_which() {
        let argv = Notice::new("Volume 40%", "").replacing(Some(17)).argv();
        assert!(argv.contains(&"--replace-id=17".to_string()));
    }

    #[test]
    fn a_notice_that_replaces_nothing_asks_to_replace_nothing() {
        let argv = Notice::new("Volume 40%", "").argv();
        assert!(!argv.iter().any(|word| word.starts_with("--replace-id")));
    }

    /// Everything after the -- is what was said, so a summary beginning with a
    /// dash is a summary and not an option.
    #[test]
    fn what_was_said_is_held_off_from_the_options() {
        let argv = Notice::new("--urgent", "-h").argv();
        let end = argv.iter().position(|word| word == "--").expect("the end of the options");
        assert_eq!(&argv[end + 1..], ["--urgent", "-h"]);
    }

    #[test]
    fn a_reading_carries_its_number_for_anything_that_can_draw_one() {
        let argv = Notice::new("Volume 40%", "").valued(40).argv();
        assert!(argv.contains(&"int:value:40".to_string()));
    }

    #[test]
    fn how_long_it_stays_is_said_the_way_notify_send_reads_it() {
        assert!(Notice::new("a", "").staying().argv().contains(&"--expire-time=0".to_string()));
        assert!(
            Notice::new("a", "")
                .lasting(1500)
                .argv()
                .contains(&"--expire-time=1500".to_string())
        );
    }
}
