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

use std::process::Command;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

use console_bus::messages::{Kind, Value};
use console_bus::connection::{Bus, NameRequestResult};
use console_core_external_programs::Program;

const PATH: &str = "/console/Bus";

const HEARD: &str = "heard: hello";

fn bus() -> Option<Bus> {
    let _address = std::env::var("DBUS_SESSION_BUS_ADDRESS").ok()?;

    match Bus::session() {
        Ok(bus) => Some(bus),
        Err(why) => panic!("there is a session bus and this could not open it: {why}"),
    }
}

#[test]
fn a_name_taken_here_is_a_name_the_bus_says_we_have() {
    let mut bus = match bus() {
        Some(bus) => bus,
        None => return,
    };

    let named = bus.named().unwrap().to_string();

    assert!(named.starts_with(':'), "the bus named this connection {named:?}");
    assert_eq!(bus.taking("console.Bus.Asked"), Ok(NameRequestResult::PrimaryOwner));

    let listed = Command::new(Program::Busctl.name().unwrap()).args(["--user", "list"]).output().unwrap();
    let listed = String::from_utf8_lossy(&listed.stdout).to_string();

    assert!(listed.contains("console.Bus.Asked"), "the bus does not list the name we took");
}

#[test]
fn a_call_from_someone_else_is_read_and_the_answer_is_read_back() {
    let mut bus = match bus() {
        Some(bus) => bus,
        None => return,
    };

    assert_eq!(bus.taking("console.Bus.Answering"), Ok(NameRequestResult::PrimaryOwner));

    let (say, heard) = channel();

    let _answering = std::thread::spawn(move || {
        loop {
            let message = match bus.heard() {
                Ok(message) => message,
                Err(why) => {
                    let _ = say.send(Err(why.to_string()));

                    return;
                }
            };

            match (message.kind, message.member.as_deref()) {
                (Kind::Call, Some("Asked")) => {
                    let value = message.values.first().and_then(|value| value.text().unwrap());
                    let answer = message.answering().unwrap();
                    let answer = answer
                        .carrying("s", vec![Value::Word(format!("heard: {}", value.unwrap_or("")))])
                        .unwrap();

                    let _ = say.send(bus.say(&answer).map(|_| ()).map_err(|why| why.to_string()));

                    return;
                }
                _ => {}
            }
        }
    });

    let called = Command::new(Program::Busctl.name().unwrap())
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
        .output()
        .unwrap();

    let printed = String::from_utf8_lossy(&called.stdout).to_string();
    let complained = String::from_utf8_lossy(&called.stderr).to_string();

    assert!(called.status.success(), "busctl value: {complained}");
    assert!(printed.contains(HEARD), "busctl read back {printed:?}");

    match heard.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(())) => {}
        Ok(Err(why)) => panic!("answering the call went wrong: {why}"),
        Err(RecvTimeoutError::Timeout) => panic!("the call was never heard"),
        Err(RecvTimeoutError::Disconnected) => panic!("the thread holding the bus is gone"),
    }
}

#[test]
fn a_call_with_a_dictionary_in_it_arrives_whole() {
    let mut bus = match bus() {
        Some(bus) => bus,
        None => return,
    };

    assert_eq!(bus.taking("console.Bus.Hinted"), Ok(NameRequestResult::PrimaryOwner));

    let (say, heard) = channel();

    let _answering = std::thread::spawn(move || {
        loop {
            let message = match bus.heard() {
                Ok(message) => message,
                Err(_why) => return,
            };

            match (message.kind, message.member.as_deref()) {
                (Kind::Call, Some("Hinted")) => {
                    let answer = message.answering().unwrap();
                    let _ = bus.say(&answer.carrying("", Vec::new()).unwrap());
                    let _ = say.send(message.values.clone());

                    return;
                }
                _ => {}
            }
        }
    });

    let called = Command::new(Program::Busctl.name().unwrap())
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
        .output()
        .unwrap();

    assert!(called.status.success(), "busctl value: {}", String::from_utf8_lossy(&called.stderr));

    let value = match heard.recv_timeout(Duration::from_secs(10)) {
        Ok(value) => value,
        Err(why) => panic!("the call was never heard: {why}"),
    };

    assert_eq!(value.first().unwrap().text().unwrap(), Some("Console"));
    assert_eq!(value.get(2).unwrap(), &Value::Signed32(7));

    let hints = match value.get(1).unwrap() {
        Value::List(hints) => hints.clone(),
        other => panic!("the hints came back as {other:?}"),
    };

    assert_eq!(hints.len(), 2);

    let urgency = hints
        .iter()
        .filter_map(|hint| match hint {
            Value::Group(pair) => match (pair.first(), pair.get(1)) {
                (Some(name), Some(value)) => match name.text().unwrap() {
                    Some("urgency") => value.counted().unwrap(),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
        .next();

    assert_eq!(urgency, Some(2));
}
