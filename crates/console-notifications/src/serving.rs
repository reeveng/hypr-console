//! Being the thing that answers `org.freedesktop.Notifications`.
//!
//! mako held this name for a year and drew the card, and it was the last
//! surface on the machine drawn in somebody else's colours: a second
//! stylesheet, written out of the palette by `just theme` and read only when a
//! daemon started, for a card nothing here could decide anything about. What
//! the rest of this crate already was is the other three quarters of a
//! notification daemon -- `saying` is what raises one, `reading` and `rows` are
//! what keeps and draws it afterwards -- so what was missing was the middle:
//! hearing the call, giving it a number, holding it until it goes, and saying
//! when it went.
//!
//! That is what is here, and none of it touches a socket or a screen. A
//! message goes in and what to answer, what to say afterwards and whether the
//! screen changed come out, so every rule below -- which notification replaces
//! which, how long a fault stays, what a number means when it is handed back
//! -- can be asked in a test with no bus anywhere. `console-notify` is the
//! program that carries the answers out; `console-bus` is the wire under it.
//!
//! **A number is never handed out twice and never handed out as zero.** Zero
//! is what the specification uses to mean *this is not a replacement*, so a
//! server that ever answered zero would be handing a caller an id that means
//! `no id` -- and the caller's next notification would silently land beside
//! the one it meant to replace rather than over it. The count starts at one
//! and only goes up.
//!
//! **What replaces what is the caller's decision and is checked all the same.**
//! A rocker held down raises a card many times a second and every one of them
//! names the id it replaces, which is what makes it one card rather than
//! twenty. An id that names nothing waiting is not a fault and is not silently
//! dropped either: it is raised as a new one, because a card that arrived while
//! the last one was going out is still a card somebody asked for.
//!
//! **Five seconds, or until it is seen.** A caller that says nothing about how
//! long is answered with five seconds, which is long enough to read a sentence
//! at arm's length; a caller that asks for none at all gets none. Everything
//! `console-say` raises is critical and critical stays, because the whole point
//! of it is that a thing which broke while nobody was looking is still there
//! when somebody looks.
//!
//! **Quiet is not deaf.** The mode holds the card back and changes nothing
//! else: what was sent is still held, the bell still counts it and still turns
//! coral for a fault. A handheld is held in front of a game as often as it is
//! worked on, and the thing worth stopping is the interruption rather than the
//! news.
//!
//! **Where the file goes can be said, for the reason the strip's can.** A check
//! brings a desktop up in a session of its own, and a daemon inside it writing
//! into this laptop's own runtime directory would take away the reading the
//! machine it is running on already had. `CONSOLE_NOTICES_PATH` is how a stage
//! points it somewhere else; the device is never told, and the default is the
//! one place the panel and the bell look.

use console_bus::messages::{Complaint, Message, Said, Saying};
use console_core_never::Never;

use std::path::PathBuf;

use crate::reading::{Notice, Quiet, Urgency, Whole};
use crate::saying::Expiry;

pub use console_events::sources::{NOTICES, OURS};

pub const AT: &str = "/org/freedesktop/Notifications";

pub const KEPT: &str = "notices.json";

pub const WHERE: &str = "CONSOLE_NOTICES_PATH";

const PEER: &str = "org.freedesktop.DBus.Peer";

const LOOKING: &str = "org.freedesktop.DBus.Introspectable";

const A_WHILE: u32 = 5000;

pub const EARLIER: usize = 20;

const URGENCY: &str = "urgency";

const VALUE: &str = "value";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Why {
    RanOut,
    #[default]
    Dismissed,
    Asked,
}

impl Why {
    fn code(self) -> Result<u32, Never> {
        Ok(match self {
            Why::RanOut => 1,
            Why::Dismissed => 2,
            Why::Asked => 3,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Changed {
    Yes,
    #[default]
    No,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Gone {
    Yes,
    #[default]
    No,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asked {
    pub app: String,
    pub replacing: u32,
    pub summary: String,
    pub body: String,
    pub urgency: Urgency,
    pub expiry: Expiry,
    pub value: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
    pub notice: Notice,
    pub expiry: Expiry,
    pub value: Option<i64>,
    pub raised: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Armed {
    pub id: u32,
    pub raised: u64,
    pub expiry: Expiry,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Holding {
    pub waiting: Vec<Held>,
    pub earlier: Vec<Notice>,
    pub quiet: Quiet,
    counted: u32,
    raises: u64,
}

impl Holding {
    pub fn raised(&mut self, asked: &Asked) -> Result<Armed, Never> {
        let standing = self.waiting.iter().position(|held| held.notice.id == asked.replacing);

        let id = match standing {
            Some(_) => asked.replacing,
            None => {
                let counted = self.counted.saturating_add(1);

                self.counted = counted;

                counted
            }
        };

        let raises = self.raises.saturating_add(1);

        self.raises = raises;

        let held = Held {
            notice: Notice {
                id,
                app: asked.app.clone(),
                summary: asked.summary.clone(),
                body: asked.body.clone(),
                urgency: asked.urgency,
            },
            expiry: asked.expiry,
            value: asked.value,
            raised: raises,
        };

        match standing {
            Some(at) => match self.waiting.get_mut(at) {
                Some(room) => *room = held,
                None => self.waiting.push(held),
            },
            None => self.waiting.push(held),
        }

        Ok(Armed { id, raised: raises, expiry: asked.expiry })
    }

    pub fn ran_out(&mut self, armed: &Armed) -> Result<Gone, Never> {
        let standing = self
            .waiting
            .iter()
            .any(|held| held.notice.id == armed.id && held.raised == armed.raised);

        match standing {
            true => self.closed(armed.id),
            false => Ok(Gone::No),
        }
    }

    pub fn closed(&mut self, id: u32) -> Result<Gone, Never> {
        let standing = self.waiting.iter().position(|held| held.notice.id == id);

        let at = match standing {
            Some(at) => at,
            None => return Ok(Gone::No),
        };

        let held = self.waiting.remove(at);

        self.earlier.insert(0, held.notice);
        self.earlier.truncate(EARLIER);

        Ok(Gone::Yes)
    }

    pub fn cleared(&mut self) -> Result<Vec<u32>, Never> {
        let every: Vec<u32> = self.waiting.iter().map(|held| held.notice.id).collect();

        for id in &every {
            let Ok(_gone) = self.closed(*id);
        }

        Ok(every)
    }

    pub fn quietened(&mut self) -> Result<Quiet, Never> {
        self.quiet = match self.quiet {
            Quiet::Coming => Quiet::HeldBack,
            Quiet::HeldBack => Quiet::Coming,
        };

        Ok(self.quiet)
    }

    pub fn showing(&self) -> Result<Vec<&Held>, Never> {
        Ok(match self.quiet {
            Quiet::HeldBack => Vec::new(),
            Quiet::Coming => self.waiting.iter().rev().collect(),
        })
    }
}

#[derive(Debug, Default)]
pub struct Turn {
    pub say: Option<Message>,
    pub gone: Vec<Message>,
    pub changed: Changed,
    pub arm: Option<Armed>,
}

pub fn heard(holding: &mut Holding, message: &Message) -> Result<Turn, Never> {
    let asking = match (message.interface.as_deref(), message.member.as_deref()) {
        (Some(NOTICES), Some(member)) => member,
        (Some(OURS), Some(member)) => member,
        (Some(PEER), Some("Ping")) => {
            let Ok(answer) = message.answering();

            return Ok(Turn { say: Some(answer), ..Turn::default() });
        }
        (Some(LOOKING), Some("Introspect")) | (None, Some("Introspect")) => {
            let Ok(answer) = message.answering();
            let Ok(looked) = looked_at();
            let Ok(said) = Said::word(&looked);
            let Ok(answer) = answer.carrying("s", vec![said]);

            return Ok(Turn { say: Some(answer), ..Turn::default() });
        }
        (Some(_), Some(_)) | (Some(_), None) | (None, Some(_)) | (None, None) => {
            let Ok(complaint) = message
                .complaining(Complaint::UnknownMethod, "this desktop answers notifications only");

            return Ok(Turn { say: Some(complaint), ..Turn::default() });
        }
    };

    match asking {
        "Notify" => notifying(holding, message),
        "CloseNotification" => closing(holding, message),
        "GetCapabilities" => {
            let Ok(answer) = message.answering();
            let Ok(body) = Said::word("body");
            let Ok(persistence) = Said::word("persistence");
            let Ok(answer) = answer.carrying("as", vec![Said::List(vec![body, persistence])]);

            Ok(Turn { say: Some(answer), ..Turn::default() })
        }
        "GetServerInformation" => {
            let Ok(answer) = message.answering();
            let mut said: Vec<Said> = Vec::new();

            for word in ["console", "console", env!("CARGO_PKG_VERSION"), "1.2"] {
                let Ok(word) = Said::word(word);

                said.push(word);
            }

            let Ok(answer) = answer.carrying("ssss", said);

            Ok(Turn { say: Some(answer), ..Turn::default() })
        }
        "ClearAll" => {
            let Ok(every) = holding.cleared();
            let Ok(answer) = message.answering();
            let Ok(gone) = going(&every, Why::Dismissed);

            Ok(Turn { say: Some(answer), gone, changed: Changed::Yes, arm: None })
        }
        "Quieten" => {
            let Ok(quiet) = holding.quietened();
            let Ok(word) = quiet.said();
            let Ok(said) = Said::word(word);
            let Ok(answer) = message.answering();
            let Ok(answer) = answer.carrying("s", vec![said]);

            Ok(Turn { say: Some(answer), changed: Changed::Yes, ..Turn::default() })
        }
        _other => {
            let Ok(complaint) =
                message.complaining(Complaint::UnknownMethod, "nothing here answers to that");

            Ok(Turn { say: Some(complaint), ..Turn::default() })
        }
    }
}

fn notifying(holding: &mut Holding, message: &Message) -> Result<Turn, Never> {
    let Ok(reading) = asked(&message.said);

    let asked = match reading {
        Some(asked) => asked,
        None => {
            let Ok(complaint) = message
                .complaining(Complaint::InvalidArgs, "a notification is susssasa{sv}i and this was not");

            return Ok(Turn { say: Some(complaint), ..Turn::default() });
        }
    };

    let Ok(armed) = holding.raised(&asked);
    let Ok(answer) = message.answering();
    let Ok(answer) = answer.carrying("u", vec![Said::Unsigned32(armed.id)]);

    Ok(Turn { say: Some(answer), gone: Vec::new(), changed: Changed::Yes, arm: Some(armed) })
}

fn closing(holding: &mut Holding, message: &Message) -> Result<Turn, Never> {
    let counted = match message.said.first() {
        Some(said) => {
            let Ok(counted) = said.counted();

            counted
        }
        None => None,
    };

    let id = match counted {
        Some(id) => match u32::try_from(id) {
            Ok(id) => id,
            Err(_too_big) => 0,
        },
        None => 0,
    };

    let Ok(gone) = holding.closed(id);
    let Ok(answer) = message.answering();

    Ok(match gone {
        Gone::No => Turn { say: Some(answer), ..Turn::default() },
        Gone::Yes => {
            let Ok(gone) = going(&[id], Why::Asked);

            Turn { say: Some(answer), gone, changed: Changed::Yes, arm: None }
        }
    })
}

pub fn going(every: &[u32], why: Why) -> Result<Vec<Message>, Never> {
    let mut said: Vec<Message> = Vec::new();

    for id in every {
        let Ok(signal) =
            Message::signal(&Saying { at: AT, on: NOTICES, saying: "NotificationClosed" });
        let Ok(code) = why.code();
        let Ok(signal) = signal.carrying("uu", vec![Said::Unsigned32(*id), Said::Unsigned32(code)]);

        said.push(signal);
    }

    Ok(said)
}

pub fn asked(said: &[Said]) -> Result<Option<Asked>, Never> {
    let Ok(app) = worded(said.first());
    let Ok(summary) = worded(said.get(3));
    let Ok(body) = worded(said.get(4));

    let enough = match (said.first(), said.get(3), said.get(4), said.get(7)) {
        (Some(_), Some(_), Some(_), Some(_)) => Enough::Yes,
        (_, _, _, _) => Enough::No,
    };

    match enough {
        Enough::No => return Ok(None),
        Enough::Yes => {}
    }

    let hints = said.get(6);
    let Ok(saying) = named(hints, URGENCY);
    let Ok(valued) = named(hints, VALUE);
    let Ok(urgency) = urgently(saying);
    let Ok(value) = numbered(valued);
    let Ok(asking) = numbered(said.get(7));
    let Ok(expiry) = lasting(asking, urgency);
    let Ok(replacing) = counted(said.get(1));

    Ok(Some(Asked { app, replacing, summary, body, urgency, expiry, value }))
}

enum Enough {
    Yes,
    No,
}

fn urgently(said: Option<&Said>) -> Result<Urgency, Never> {
    let Ok(counted) = numbered(said);

    Ok(match counted {
        Some(0) => Urgency::Low,
        Some(2) => Urgency::Critical,
        Some(_) | None => Urgency::Normal,
    })
}

fn numbered(said: Option<&Said>) -> Result<Option<i64>, Never> {
    match said {
        Some(said) => said.counted(),
        None => Ok(None),
    }
}

fn counted(said: Option<&Said>) -> Result<u32, Never> {
    let Ok(counted) = numbered(said);

    Ok(match counted {
        Some(counted) => match u32::try_from(counted) {
            Ok(counted) => counted,
            Err(_too_big) => 0,
        },
        None => 0,
    })
}

fn worded(said: Option<&Said>) -> Result<String, Never> {
    let saying = match said {
        Some(said) => {
            let Ok(saying) = said.saying();

            saying
        }
        None => None,
    };

    Ok(match saying {
        Some(saying) => saying.to_string(),
        None => String::new(),
    })
}

fn lasting(asking: Option<i64>, urgency: Urgency) -> Result<Expiry, Never> {
    Ok(match asking {
        Some(0) => Expiry::Stays,
        Some(asking) if asking > 0 => match u32::try_from(asking) {
            Ok(asking) => Expiry::Milliseconds(asking),
            Err(_fault) => Expiry::Stays,
        },
        Some(_) | None => match urgency {
            Urgency::Critical => Expiry::Stays,
            Urgency::Low | Urgency::Normal => Expiry::Milliseconds(A_WHILE),
        },
    })
}

fn named<'a>(hints: Option<&'a Said>, wanted: &str) -> Result<Option<&'a Said>, Never> {
    let listed = match hints {
        Some(hints) => {
            let Ok(listed) = hints.listed();

            listed
        }
        None => None,
    };

    let held = match listed {
        Some(held) => held,
        None => return Ok(None),
    };

    for hint in held {
        let Ok(pair) = hint.pair();

        let (name, value) = match pair {
            Some((name, value)) => (name, value),
            None => continue,
        };

        let Ok(name) = name.saying();

        match name == Some(wanted) {
            true => return Ok(Some(value)),
            false => {}
        }
    }

    Ok(None)
}

fn looked_at() -> Result<String, Never> {
    Ok(format!(
        "<!DOCTYPE node PUBLIC \"-//freedesktop//DTD D-BUS Object Introspection 1.0//EN\" \
\"http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd\">
<node>
  <interface name=\"{NOTICES}\">
    <method name=\"Notify\">
      <arg type=\"s\" name=\"app_name\" direction=\"in\"/>
      <arg type=\"u\" name=\"replaces_id\" direction=\"in\"/>
      <arg type=\"s\" name=\"app_icon\" direction=\"in\"/>
      <arg type=\"s\" name=\"summary\" direction=\"in\"/>
      <arg type=\"s\" name=\"body\" direction=\"in\"/>
      <arg type=\"as\" name=\"actions\" direction=\"in\"/>
      <arg type=\"a{{sv}}\" name=\"hints\" direction=\"in\"/>
      <arg type=\"i\" name=\"expire_timeout\" direction=\"in\"/>
      <arg type=\"u\" name=\"id\" direction=\"out\"/>
    </method>
    <method name=\"CloseNotification\">
      <arg type=\"u\" name=\"id\" direction=\"in\"/>
    </method>
    <method name=\"GetCapabilities\">
      <arg type=\"as\" name=\"capabilities\" direction=\"out\"/>
    </method>
    <method name=\"GetServerInformation\">
      <arg type=\"s\" name=\"name\" direction=\"out\"/>
      <arg type=\"s\" name=\"vendor\" direction=\"out\"/>
      <arg type=\"s\" name=\"version\" direction=\"out\"/>
      <arg type=\"s\" name=\"spec_version\" direction=\"out\"/>
    </method>
    <signal name=\"NotificationClosed\">
      <arg type=\"u\" name=\"id\"/>
      <arg type=\"u\" name=\"reason\"/>
    </signal>
  </interface>
  <interface name=\"{OURS}\">
    <method name=\"ClearAll\"/>
    <method name=\"Quieten\">
      <arg type=\"s\" name=\"quiet\" direction=\"out\"/>
    </method>
  </interface>
</node>
"
    ))
}

pub fn keeps(told: Option<&str>) -> Result<PathBuf, Never> {
    match told.map(str::trim).filter(|said| !said.is_empty()) {
        Some(said) => Ok(PathBuf::from(said)),
        None => {
            let Ok(under) = crate::saying::under();

            Ok(under.join(KEPT))
        }
    }
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(
        explicit026_env_read_once,
        reason = "CONSOLE_NOTICES_PATH belongs to this crate, and the const beside it is the only spelling of the name"
    )
)]
pub fn kept() -> Result<PathBuf, Never> {
    let told = match std::env::var(WHERE) {
        Ok(said) => Some(said),
        Err(std::env::VarError::NotPresent) => None,
        Err(fault) => {
            eprintln!("console-notify: {WHERE}: {fault}");

            None
        }
    };

    keeps(told.as_deref())
}

pub fn held() -> Result<Whole, Never> {
    let Ok(at) = kept();
    let Ok(read) = console_core_atomic_writes::read(&at);

    let said = match read {
        console_core_atomic_writes::Held::Said(said) => said,
        console_core_atomic_writes::Held::Nothing => return Ok(Whole::default()),
        console_core_atomic_writes::Held::Unreadable(why) => {
            eprintln!("{}: {why}", at.display());

            return Ok(Whole::default());
        }
    };

    crate::reading::whole(&said)
}

pub fn keeping(holding: &Holding) -> Result<Whole, Never> {
    let waiting: Vec<Notice> = holding.waiting.iter().map(|held| held.notice.clone()).collect();

    Ok(Whole { waiting, earlier: holding.earlier.clone(), quiet: holding.quiet })
}

pub fn closing_one(id: u32) -> Result<Vec<String>, Never> {
    Ok(vec![
        "--user".to_string(),
        "call".to_string(),
        NOTICES.to_string(),
        AT.to_string(),
        NOTICES.to_string(),
        "CloseNotification".to_string(),
        "u".to_string(),
        id.to_string(),
    ])
}

pub fn asking(member: &str) -> Result<Vec<String>, Never> {
    Ok(vec![
        "--user".to_string(),
        "call".to_string(),
        NOTICES.to_string(),
        AT.to_string(),
        OURS.to_string(),
        member.to_string(),
    ])
}


#[cfg(test)]
mod tests {
    use super::*;
    use console_bus::messages::{Kind, Whom};

    fn calling(member: &str) -> Message {
        Message::call(&Whom { to: NOTICES, at: AT, on: NOTICES, calling: member }).unwrap()
    }

    fn notify(summary: &str, replacing: u32, hints: Vec<Said>, lasting: i64) -> Message {
        calling("Notify")
            .carrying(
                "susssasa{sv}i",
                vec![
                    Said::Word("Console".to_string()),
                    Said::Unsigned32(replacing),
                    Said::Word(String::new()),
                    Said::Word(summary.to_string()),
                    Said::Word("the body".to_string()),
                    Said::List(Vec::new()),
                    Said::List(hints),
                    Said::Signed32(i32::try_from(lasting).unwrap()),
                ],
            )
            .unwrap()
    }

    fn hint(named: &str, shape: &str, said: Said) -> Said {
        Said::Group(vec![
            Said::Word(named.to_string()),
            Said::Held { shape: shape.to_string(), said: Box::new(said) },
        ])
    }

    fn urgently(urgency: u8) -> Vec<Said> {
        vec![hint("urgency", "y", Said::Byte(urgency))]
    }

    fn raising(holding: &mut Holding, summary: &str) -> u32 {
        let turn = heard(holding, &notify(summary, 0, Vec::new(), -1)).unwrap();
        let said = turn.say.unwrap();

        match said.said.first() {
            Some(Said::Unsigned32(id)) => *id,
            other => panic!("a notification was answered with {other:?}"),
        }
    }

    #[test]
    fn a_notification_is_answered_with_a_number_and_the_number_is_never_nothing() {
        let mut holding = Holding::default();

        assert_eq!(raising(&mut holding, "one"), 1);
        assert_eq!(raising(&mut holding, "two"), 2);
        assert_eq!(holding.waiting.len(), 2);
    }

    #[test]
    fn what_a_card_says_is_what_the_call_carried() {
        let mut holding = Holding::default();
        let _id = raising(&mut holding, "Notifications fell over");
        let held = holding.waiting.first().unwrap();

        assert_eq!(held.notice.summary, "Notifications fell over");
        assert_eq!(held.notice.body, "the body");
        assert_eq!(held.notice.app, "Console");
    }

    #[test]
    fn a_notification_that_replaces_one_that_is_waiting_takes_its_place_and_its_number() {
        let mut holding = Holding::default();
        let first = raising(&mut holding, "40%");
        let turn = heard(&mut holding, &notify("45%", first, Vec::new(), -1)).unwrap();

        assert_eq!(turn.say.unwrap().said, vec![Said::Unsigned32(first)]);
        assert_eq!(holding.waiting.len(), 1);
        assert_eq!(holding.waiting.first().unwrap().notice.summary, "45%");
    }

    #[test]
    fn a_notification_that_replaces_one_that_has_gone_is_raised_as_a_new_one() {
        let mut holding = Holding::default();
        let first = raising(&mut holding, "40%");
        let Ok(_gone) = holding.closed(first);
        let turn = heard(&mut holding, &notify("45%", first, Vec::new(), -1)).unwrap();

        assert_eq!(turn.say.unwrap().said, vec![Said::Unsigned32(2)]);
        assert_eq!(holding.waiting.len(), 1);
    }

    #[test]
    fn five_seconds_is_what_a_caller_that_says_nothing_gets() {
        let mut holding = Holding::default();
        let _id = raising(&mut holding, "listening");

        assert_eq!(holding.waiting.first().unwrap().expiry, Expiry::Milliseconds(A_WHILE));
    }

    #[test]
    fn a_fault_stays_until_somebody_has_seen_it() {
        let mut holding = Holding::default();
        let _turn = heard(&mut holding, &notify("it broke", 0, urgently(2), -1)).unwrap();
        let held = holding.waiting.first().unwrap();

        assert_eq!(held.expiry, Expiry::Stays);
        assert_eq!(held.notice.urgency, Urgency::Critical);
    }

    #[test]
    fn a_caller_that_asks_for_a_length_is_given_the_one_it_asked_for() {
        let mut holding = Holding::default();
        let _turn = heard(&mut holding, &notify("a moment", 0, Vec::new(), 1200)).unwrap();

        assert_eq!(holding.waiting.first().unwrap().expiry, Expiry::Milliseconds(1200));
    }

    #[test]
    fn a_caller_that_asks_for_no_length_at_all_is_asking_for_it_to_stay() {
        let mut holding = Holding::default();
        let _turn = heard(&mut holding, &notify("standing", 0, Vec::new(), 0)).unwrap();

        assert_eq!(holding.waiting.first().unwrap().expiry, Expiry::Stays);
    }

    #[test]
    fn a_reading_carries_the_number_it_was_sent_with() {
        let mut holding = Holding::default();
        let hints = vec![hint("value", "i", Said::Signed32(40))];
        let _turn = heard(&mut holding, &notify("Volume", 0, hints, -1)).unwrap();

        assert_eq!(holding.waiting.first().unwrap().value, Some(40));
    }

    #[test]
    fn the_length_a_card_was_given_runs_out_and_takes_that_card_down() {
        let mut holding = Holding::default();
        let turn = heard(&mut holding, &notify("40%", 0, Vec::new(), 400)).unwrap();
        let armed = turn.arm.unwrap();

        assert_eq!(armed.expiry, Expiry::Milliseconds(400));
        assert_eq!(holding.ran_out(&armed), Ok(Gone::Yes));
        assert!(holding.waiting.is_empty());
    }

    #[test]
    fn a_card_replaced_before_its_length_ran_out_is_not_taken_down_by_the_old_length() {
        let mut holding = Holding::default();
        let first = heard(&mut holding, &notify("40%", 0, Vec::new(), 400)).unwrap();
        let armed = first.arm.unwrap();
        let again = heard(&mut holding, &notify("45%", armed.id, Vec::new(), 400)).unwrap();

        assert_eq!(holding.ran_out(&armed), Ok(Gone::No));
        assert_eq!(holding.waiting.len(), 1);
        assert_eq!(holding.ran_out(&again.arm.unwrap()), Ok(Gone::Yes));
        assert!(holding.waiting.is_empty());
    }

    #[test]
    fn a_card_that_stays_is_armed_with_nothing_to_run_out() {
        let mut holding = Holding::default();
        let turn = heard(&mut holding, &notify("it broke", 0, urgently(2), -1)).unwrap();

        assert_eq!(turn.arm.unwrap().expiry, Expiry::Stays);
    }

    #[test]
    fn closing_one_moves_it_to_earlier_and_says_which_and_why() {
        let mut holding = Holding::default();
        let id = raising(&mut holding, "one");
        let closing = calling("CloseNotification").carrying("u", vec![Said::Unsigned32(id)]).unwrap();
        let turn = heard(&mut holding, &closing).unwrap();

        assert_eq!(turn.changed, Changed::Yes);
        assert!(holding.waiting.is_empty());
        assert_eq!(holding.earlier.len(), 1);
        assert_eq!(turn.gone.len(), 1);

        let gone = turn.gone.first().unwrap();

        assert_eq!(gone.kind, Kind::Signal);
        assert_eq!(gone.member.as_deref(), Some("NotificationClosed"));
        assert_eq!(gone.said, vec![Said::Unsigned32(id), Said::Unsigned32(3)]);
    }

    #[test]
    fn closing_one_that_is_not_there_says_nothing_and_is_not_a_fault() {
        let mut holding = Holding::default();
        let closing = calling("CloseNotification").carrying("u", vec![Said::Unsigned32(9)]).unwrap();
        let turn = heard(&mut holding, &closing).unwrap();

        assert_eq!(turn.changed, Changed::No);
        assert!(turn.gone.is_empty());
        assert_eq!(turn.say.unwrap().kind, Kind::Answer);
    }

    #[test]
    fn clearing_them_all_says_one_of_each_and_keeps_them_all() {
        let mut holding = Holding::default();
        let one = raising(&mut holding, "one");
        let two = raising(&mut holding, "two");
        let clearing = Message::call(&Whom { to: NOTICES, at: AT, on: OURS, calling: "ClearAll" }).unwrap();
        let turn = heard(&mut holding, &clearing).unwrap();

        assert!(holding.waiting.is_empty());
        assert_eq!(holding.earlier.len(), 2);
        assert_eq!(turn.gone.len(), 2);

        let said: Vec<Vec<Said>> = turn.gone.iter().map(|gone| gone.said.clone()).collect();

        assert_eq!(said[0], vec![Said::Unsigned32(one), Said::Unsigned32(2)]);
        assert_eq!(said[1], vec![Said::Unsigned32(two), Said::Unsigned32(2)]);
    }

    #[test]
    fn what_is_cleared_is_in_earlier_a_moment_later_and_the_newest_is_first() {
        let mut holding = Holding::default();
        let _one = raising(&mut holding, "one");
        let _two = raising(&mut holding, "two");
        let Ok(_every) = holding.cleared();

        assert_eq!(holding.earlier.first().unwrap().summary, "two");
    }

    #[test]
    fn earlier_holds_what_a_screenful_holds_and_lets_the_rest_go() {
        let mut holding = Holding::default();

        for many in 0..EARLIER + 5 {
            let id = raising(&mut holding, &format!("one of {many}"));
            let Ok(_gone) = holding.closed(id);
        }

        assert_eq!(holding.earlier.len(), EARLIER);
        assert_eq!(holding.earlier.first().unwrap().summary, format!("one of {}", EARLIER + 4));
    }

    #[test]
    fn quiet_holds_the_card_back_and_holds_nothing_else_back() {
        let mut holding = Holding::default();
        let _id = raising(&mut holding, "one");
        let quietening = Message::call(&Whom { to: NOTICES, at: AT, on: OURS, calling: "Quieten" }).unwrap();
        let turn = heard(&mut holding, &quietening).unwrap();

        assert_eq!(turn.say.unwrap().said, vec![Said::Word("held-back".to_string())]);
        assert_eq!(holding.quiet, Quiet::HeldBack);
        assert_eq!(holding.waiting.len(), 1);
        assert!(holding.showing().unwrap().is_empty());

        let _turn = heard(&mut holding, &quietening).unwrap();

        assert_eq!(holding.quiet, Quiet::Coming);
        assert_eq!(holding.showing().unwrap().len(), 1);
    }

    #[test]
    fn the_newest_card_is_the_one_drawn_first() {
        let mut holding = Holding::default();
        let _one = raising(&mut holding, "one");
        let _two = raising(&mut holding, "two");
        let showing = holding.showing().unwrap();

        assert_eq!(showing.first().unwrap().notice.summary, "two");
    }

    #[test]
    fn what_this_can_do_is_said_when_it_is_asked() {
        let mut holding = Holding::default();
        let turn = heard(&mut holding, &calling("GetCapabilities")).unwrap();
        let said = turn.say.unwrap().said;

        assert_eq!(said, vec![Said::List(vec![
            Said::Word("body".to_string()),
            Said::Word("persistence".to_string()),
        ])]);
    }

    #[test]
    fn what_this_is_is_said_when_it_is_asked() {
        let mut holding = Holding::default();
        let turn = heard(&mut holding, &calling("GetServerInformation")).unwrap();
        let said = turn.say.unwrap().said;

        assert_eq!(said.first(), Some(&Said::Word("console".to_string())));
        assert_eq!(said.len(), 4);
    }

    #[test]
    fn a_call_nothing_here_answers_is_complained_about_rather_than_ignored() {
        let mut holding = Holding::default();
        let turn = heard(&mut holding, &calling("Whatever")).unwrap();
        let said = turn.say.unwrap();

        assert_eq!(said.kind, Kind::Fault);
        assert_eq!(said.fault.as_deref(), Some("org.freedesktop.DBus.Error.UnknownMethod"));
    }

    #[test]
    fn a_notification_that_is_not_a_notification_is_complained_about() {
        let mut holding = Holding::default();
        let wrong = calling("Notify").carrying("s", vec![Said::Word("only this".to_string())]).unwrap();
        let turn = heard(&mut holding, &wrong).unwrap();

        assert_eq!(turn.say.unwrap().kind, Kind::Fault);
        assert!(holding.waiting.is_empty());
    }

    #[test]
    fn a_hint_nothing_here_reads_is_walked_past_rather_than_stopping_the_card() {
        let mut holding = Holding::default();
        let hints = vec![
            hint("image-path", "s", Said::Word("/a/picture.png".to_string())),
            hint("urgency", "y", Said::Byte(2)),
        ];
        let _turn = heard(&mut holding, &notify("it broke", 0, hints, -1)).unwrap();

        assert_eq!(holding.waiting.first().unwrap().notice.urgency, Urgency::Critical);
    }

    #[test]
    fn what_the_panel_reads_is_what_the_daemon_is_holding() {
        let mut holding = Holding::default();
        let id = raising(&mut holding, "one");
        let _two = raising(&mut holding, "two");
        let Ok(_gone) = holding.closed(id);
        let Ok(keeping) = keeping(&holding);

        assert_eq!(keeping.waiting.len(), 1);
        assert_eq!(keeping.earlier.len(), 1);
        assert_eq!(keeping.quiet, Quiet::Coming);
    }
}
