//! The one thing the byte arrays cannot say: that a real bus agrees.
//!
//! Everything in `messages` is arithmetic and is tested against itself, which
//! proves the reader and the writer are the same opinion and proves nothing
//! about whether the opinion is right. A wire format is an agreement with
//! someone else, so this takes a name on the session bus that is actually
//! running, has `busctl` call it, and answers -- which puts a second
//! implementation on the other end of every byte in both directions.
//!
//! Where there is no session bus there is nothing to disagree with, and the
//! test says so and stops. That is a machine without a desktop rather than a
//! fault: the wire is still held by everything above.

use std::error::Error;
use std::process::Command;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

use console_bus::messages::{Kind, Value};
use console_core_iteration::{Step, iterate};
use console_bus::connection::{Bus, ConnectionError, NameRequestResult};
use console_core_external_programs::Program;

const PATH: &str = "/console/Bus";

const HEARD: &str = "heard: hello";

fn bus() -> Result<Option<Bus>, ConnectionError> {
    match Bus::session() {
        Ok(bus) => Ok(Some(bus)),
        Err(ConnectionError::Nowhere) => Ok(None),
        Err(why) => Err(why),
    }
}

#[test]
fn a_name_taken_here_is_a_name_the_bus_says_we_have() -> Result<(), Box<dyn Error>> {
    let bus = bus()?;

    let mut bus = match bus {
        Some(bus) => bus,
        None => return Ok(()),
    };

    let Ok(named) = bus.unique_name();
    let named = named.to_string();

    assert!(named.starts_with(':'), "the bus named this connection {named:?}");
    assert_eq!(bus.request_name("console.Bus.Asked"), Ok(NameRequestResult::PrimaryOwner));

    let Ok(busctl) = Program::Busctl.name();
    let listed = Command::new(busctl).args(["--user", "list"]).output()?;
    let listed = String::from_utf8_lossy(&listed.stdout).to_string();

    assert!(listed.contains("console.Bus.Asked"), "the bus does not list the name we took");

    Ok(())
}

#[test]
fn a_call_from_someone_else_is_read_and_the_answer_is_read_back() -> Result<(), Box<dyn Error>> {
    let bus = bus()?;

    let mut bus = match bus {
        Some(bus) => bus,
        None => return Ok(()),
    };

    assert_eq!(bus.request_name("console.Bus.Answering"), Ok(NameRequestResult::PrimaryOwner));

    let (say, heard) = channel();

    let _answering = std::thread::spawn(move || {
        let _ = iterate(bus, |mut bus| {
            let message = match bus.receive() {
                Ok(message) => message,
                Err(why) => {
                    let _ = say.send(Err(why.to_string()));

                    return Ok(Step::Halt(()));
                }
            };

            Ok(match (message.kind, message.member.as_deref()) {
                (Kind::Call, Some("Asked")) => {
                    let value = match message.values.first().map(Value::text) {
                        Some(Ok(Some(value))) => value,
                        Some(Ok(None)) | None => "",
                    };
                    let Ok(answer) = message.answering();
                    let Ok(answer) = answer.carrying("s", vec![Value::Word(format!("heard: {value}"))]);

                    let _ = say.send(bus.say(&answer).map(|_| ()).map_err(|why| why.to_string()));

                    Step::Halt(())
                }
                (Kind::Call, Some(_)) | (Kind::Call, None) => Step::Again(bus),
                (Kind::Answer | Kind::ErrorReply | Kind::Signal, _) => Step::Again(bus),
            })
        });
    });

    let Ok(busctl) = Program::Busctl.name();

    let called = Command::new(busctl)
        .args([
            "--user",
            "call",
            "console.Bus.Answering",
            PATH,
            "console.Bus",
            "Asked",
            "s",
            "hello",
        ])
        .output()?;

    let printed = String::from_utf8_lossy(&called.stdout).to_string();
    let complained = String::from_utf8_lossy(&called.stderr).to_string();

    assert!(called.status.success(), "busctl value: {complained}");
    assert!(printed.contains(HEARD), "busctl read back {printed:?}");

    match heard.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(why)) => Err(Box::from(format!("answering the call went wrong: {why}"))),
        Err(RecvTimeoutError::Timeout) => Err(Box::from("the call was never heard")),
        Err(RecvTimeoutError::Disconnected) => Err(Box::from("the thread holding the bus is gone")),
    }
}

#[test]
fn a_call_with_a_dictionary_in_it_arrives_whole() -> Result<(), Box<dyn Error>> {
    let bus = bus()?;

    let mut bus = match bus {
        Some(bus) => bus,
        None => return Ok(()),
    };

    assert_eq!(bus.request_name("console.Bus.Hinted"), Ok(NameRequestResult::PrimaryOwner));

    let (say, heard) = channel();

    let _answering = std::thread::spawn(move || {
        let _ = iterate(bus, |mut bus| {
            let message = match bus.receive() {
                Ok(message) => message,
                Err(_the_bus_hung_up) => return Ok(Step::Halt(())),
            };

            Ok(match (message.kind, message.member.as_deref()) {
                (Kind::Call, Some("Hinted")) => {
                    let Ok(answer) = message.answering();
                    let Ok(answer) = answer.carrying("", Vec::new());
                    let _ = bus.say(&answer);
                    let _ = say.send(message.values.clone());

                    Step::Halt(())
                }
                (Kind::Call, Some(_)) | (Kind::Call, None) => Step::Again(bus),
                (Kind::Answer | Kind::ErrorReply | Kind::Signal, _) => Step::Again(bus),
            })
        });
    });

    let Ok(busctl) = Program::Busctl.name();

    let called = Command::new(busctl)
        .args([
            "--user",
            "call",
            "console.Bus.Hinted",
            PATH,
            "console.Bus",
            "Hinted",
            "sa{sv}i",
            "Console",
            "2",
            "urgency",
            "y",
            "2",
            "value",
            "i",
            "40",
            "7",
        ])
        .output()?;

    assert!(called.status.success(), "busctl value: {}", String::from_utf8_lossy(&called.stderr));

    let value = heard.recv_timeout(Duration::from_secs(10))?;

    let (console, hints, seven) = match value.as_slice() {
        [console, hints, seven] => (console, hints, seven),
        other => return Err(Box::from(format!("the call came back as {other:?}"))),
    };

    let Ok(listed) = hints.listed();
    let hints = listed.ok_or(format!("the hints came back as {hints:?}"))?;

    assert_eq!(console.text(), Ok(Some("Console")));
    assert_eq!(seven, &Value::Signed32(7));
    assert_eq!(hints.len(), 2);

    let mut urgency = None;

    for hint in hints {
        let Ok(pair) = hint.pair();

        let (name, value) = match pair {
            Some(pair) => pair,
            None => continue,
        };

        match name.text() {
            Ok(Some("urgency")) => {
                let Ok(counted) = value.counted();

                urgency = counted;
            }
            Ok(Some(_)) | Ok(None) => {}
        }
    }

    assert_eq!(urgency, Some(2));

    Ok(())
}
