//! The socket the messages go down, and the four sentences that open it.
//!
//! A session bus is a unix socket named by `DBUS_SESSION_BUS_ADDRESS`, and
//! everything that happens on it happens after a short conversation in
//! plain text: a nul byte, `AUTH EXTERNAL` with this process's user id in
//! hexadecimal, and `BEGIN`. The nul byte is not a formality -- it is how the
//! other end knows to look at the credentials the kernel attached to the
//! connection -- and after `BEGIN` not one byte of text is spoken again.
//!
//! Two calls follow that are not this desktop's. `Hello` is how a connection
//! learns the name the bus has given it, and it is mandatory: a bus refuses
//! everything else until it has been asked. `RequestName` is the one that
//! matters here, and it is asked with the two flags that say what taking a
//! name means -- replace whoever holds it, and do not stand in a queue behind
//! them. A daemon that queued would come up looking like it had worked and
//! would draw nothing, because the notifications would be going to whoever was
//! already there.
//!
//! **Reading is blocking and that is the point.** There is no poll and no
//! timer here: `heard` waits on the socket for the next whole message, which
//! is what a thread is for. A card is drawn from glib's loop, so what runs
//! here runs on a thread of its own and hands over what it heard.
//!
//! **A message is read in two goes.** The first sixteen bytes say how long the
//! rest is -- the header fields and the body each carry their own length --
//! so there is no scanning for a boundary and no buffer of leftovers: what is
//! read is exactly one message, every time.

use std::fmt;
use std::io::{Read, Write};
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixStream};
use std::sync::{Arc, Mutex};

use console_core_iteration::{Endless, Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::index;
use rustix::process::getuid;

use crate::messages::{self, HEAD, Message, Error, Value, Whom};

pub const BUS: &str = "org.freedesktop.DBus";

pub const BUS_PATH: &str = "/org/freedesktop/DBus";

const ADDRESS: &str = "DBUS_SESSION_BUS_ADDRESS";

const TAKING: u32 = 6;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionError {
    Nowhere,
    Machine(String),
    Rejected(String),
    Ended,
    Error(Error),
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionError::Nowhere => write!(to, "there is no session bus in this environment"),
            ConnectionError::Machine(why) => write!(to, "the socket would not: {why}"),
            ConnectionError::Rejected(why) => write!(to, "the bus refused: {why}"),
            ConnectionError::Ended => write!(to, "the bus hung up"),
            ConnectionError::Error(torn) => write!(to, "{torn}"),
        }
    }
}

impl std::error::Error for ConnectionError {}

impl From<Error> for ConnectionError {
    fn from(torn: Error) -> Self {
        ConnectionError::Error(torn)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameRequestResult {
    PrimaryOwner,
    InQueue,
    Exists,
    AlreadyOwner,
}

impl NameRequestResult {
    fn of(value: u32) -> Result<Option<NameRequestResult>, Never> {
        Ok(match value {
            1 => Some(NameRequestResult::PrimaryOwner),
            2 => Some(NameRequestResult::InQueue),
            3 => Some(NameRequestResult::Exists),
            4 => Some(NameRequestResult::AlreadyOwner),
            _ => None,
        })
    }
}

pub struct Receiver {
    stream: UnixStream,
}

#[derive(Clone)]
pub struct Sender {
    held: Arc<Mutex<SenderState>>,
}

struct SenderState {
    stream: UnixStream,
    serial: u32,
}

pub struct Bus {
    hearing: Receiver,
    saying: Sender,
    named: String,
}

impl Bus {
    pub fn session() -> Result<Bus, ConnectionError> {
        #[cfg_attr(
            dylint_lib = "explicit026_env_read_once",
            allow(
                explicit026_env_read_once,
                reason = "DBUS_SESSION_BUS_ADDRESS is this crate's own: what a session bus is, where it is and what an unset one means is the question this crate exists to answer"
            )
        )]
        let value = match std::env::var(ADDRESS) {
            Ok(value) => value,
            Err(_fault) => return Err(ConnectionError::Nowhere),
        };

        let stream = opened(&value)?;

        let writing = match stream.try_clone() {
            Ok(writing) => writing,
            Err(fault) => return Err(ConnectionError::Machine(fault.to_string())),
        };

        let saying = Sender { held: Arc::new(Mutex::new(SenderState { stream: writing, serial: 0 })) };
        let mut bus = Bus { hearing: Receiver { stream }, saying, named: String::new() };

        bus.greet()?;

        let Ok(hello) = Message::call(&Whom { to: BUS, at: BUS_PATH, on: BUS, calling: "Hello" });
        let answer = bus.call(&hello)?;

        let Ok(saying) = match answer.values.first() {
            Some(value) => value.text(),
            None => Ok(None),
        };

        bus.named = match saying {
            Some(named) => named.to_string(),
            None => String::new(),
        };

        Ok(bus)
    }

    pub fn unique_name(&self) -> Result<&str, Never> {
        Ok(&self.named)
    }

    pub fn request_name(&mut self, name: &str) -> Result<NameRequestResult, ConnectionError> {
        let Ok(asking) =
            Message::call(&Whom { to: BUS, at: BUS_PATH, on: BUS, calling: "RequestName" });
        let Ok(word) = Value::word(name);
        let Ok(asking) = asking.carrying("su", vec![word, Value::Unsigned32(TAKING)]);

        let answer = self.call(&asking)?;

        let Ok(counted) = match answer.values.first() {
            Some(value) => value.counted(),
            None => Ok(None),
        };

        let value = match counted {
            Some(value) => value,
            None => return Err(ConnectionError::Rejected(format!("{name} was answered with nothing"))),
        };

        let value = match u32::try_from(value) {
            Ok(value) => value,
            Err(_fault) => return Err(ConnectionError::Rejected(format!("{name} was answered with {value}"))),
        };

        let Ok(got) = NameRequestResult::of(value);

        match got {
            Some(got) => Ok(got),
            None => Err(ConnectionError::Rejected(format!("{name} was answered with {value}"))),
        }
    }

    pub fn say(&mut self, message: &Message) -> Result<u32, ConnectionError> {
        self.saying.say(message)
    }

    pub fn receive(&mut self) -> Result<Message, ConnectionError> {
        self.hearing.receive()
    }

    pub fn split(self) -> Result<(Receiver, Sender), Never> {
        Ok((self.hearing, self.saying))
    }

    fn call(&mut self, message: &Message) -> Result<Message, ConnectionError> {
        let serial = self.say(message)?;

        let answered = iterate(self, |this| {
            Ok(match this.receive() {
                Ok(heard) => match (heard.reply_to == Some(serial), heard.fault.clone()) {
                    (true, Some(named)) => Step::Halt(Err(ConnectionError::Rejected(named))),
                    (true, None) => Step::Halt(Ok(heard)),
                    (false, _) => Step::Again(this),
                },
                Err(fault) => Step::Halt(Err(fault)),
            })
        });

        match answered {
            Ok(answered) => answered,
            Err(Endless) => Err(ConnectionError::Ended),
        }
    }

    fn read_bytes(&mut self, many: u32) -> Result<Vec<u8>, ConnectionError> {
        self.hearing.read_bytes(many)
    }

    fn greet(&mut self) -> Result<(), ConnectionError> {
        let Ok(mine) = whoever();

        let mut value = String::from("\0AUTH EXTERNAL ");

        for byte in mine.to_string().bytes() {
            value.push_str(&format!("{byte:02x}"));
        }

        value.push_str("\r\n");

        self.saying.plainly(value.as_bytes())?;

        let answer = self.line()?;

        match answer.starts_with("OK") {
            true => {}
            false => return Err(ConnectionError::Rejected(answer)),
        }

        self.saying.plainly(b"BEGIN\r\n")
    }

    fn line(&mut self) -> Result<String, ConnectionError> {
        let read = iterate((self, String::new()), |(this, mut value)| {
            let byte = match this.read_bytes(1) {
                Ok(byte) => byte,
                Err(fault) => return Ok(Step::Halt(Err(fault))),
            };

            Ok(match byte.first() {
                Some(b'\n') => Step::Halt(Ok(value.trim_end().to_string())),
                Some(byte) => {
                    value.push(char::from(*byte));

                    Step::Again((this, value))
                }
                None => Step::Halt(Err(ConnectionError::Ended)),
            })
        });

        match read {
            Ok(read) => read,
            Err(Endless) => Err(ConnectionError::Ended),
        }
    }
}

impl Receiver {
    pub fn receive(&mut self) -> Result<Message, ConnectionError> {
        let mut bytes = self.read_bytes(HEAD)?;
        let whole = messages::length(&bytes)?;
        let rest = whole.saturating_sub(HEAD);
        let more = self.read_bytes(rest)?;

        bytes.extend_from_slice(&more);

        let message = messages::read(&bytes)?;

        Ok(message)
    }

    fn read_bytes(&mut self, many: u32) -> Result<Vec<u8>, ConnectionError> {
        let Ok(many) = index(many);
        let mut held = vec![0u8; many];

        match self.stream.read_exact(&mut held) {
            Ok(()) => Ok(held),
            Err(fault) => match fault.kind() == std::io::ErrorKind::UnexpectedEof {
                true => Err(ConnectionError::Ended),
                false => Err(ConnectionError::Machine(fault.to_string())),
            },
        }
    }
}

impl Sender {
    pub fn say(&self, message: &Message) -> Result<u32, ConnectionError> {
        let mut held = match self.held.lock() {
            Ok(held) => held,
            Err(_poisoned) => return Err(ConnectionError::Ended),
        };

        let serial = held.serial.saturating_add(1);

        held.serial = serial;

        let bytes = message.bytes(serial)?;

        match held.stream.write_all(&bytes) {
            Ok(()) => {}
            Err(fault) => return Err(ConnectionError::Machine(fault.to_string())),
        }

        match held.stream.flush() {
            Ok(()) => Ok(serial),
            Err(fault) => Err(ConnectionError::Machine(fault.to_string())),
        }
    }

    fn plainly(&self, value: &[u8]) -> Result<(), ConnectionError> {
        let mut held = match self.held.lock() {
            Ok(held) => held,
            Err(_poisoned) => return Err(ConnectionError::Ended),
        };

        match held.stream.write_all(value) {
            Ok(()) => Ok(()),
            Err(fault) => Err(ConnectionError::Machine(fault.to_string())),
        }
    }
}

fn whoever() -> Result<u32, Never> {
    let mine = getuid();

    Ok(mine.as_raw())
}

fn opened(value: &str) -> Result<UnixStream, ConnectionError> {
    let first = match value.split(';').next() {
        Some(first) => first,
        None => return Err(ConnectionError::Nowhere),
    };

    let mut path: Option<&str> = None;
    let mut hidden: Option<&str> = None;

    for part in first.split(',') {
        match part.split_once('=') {
            Some(("unix:path", where_)) => path = Some(where_),
            Some(("path", where_)) => path = Some(where_),
            Some(("unix:abstract", where_)) => hidden = Some(where_),
            Some(("abstract", where_)) => hidden = Some(where_),
            Some(_) | None => {}
        }
    }

    match (path, hidden) {
        (Some(path), _) => match UnixStream::connect(path) {
            Ok(stream) => Ok(stream),
            Err(fault) => Err(ConnectionError::Machine(fault.to_string())),
        },
        (None, Some(hidden)) => {
            let address = match SocketAddr::from_abstract_name(hidden.as_bytes()) {
                Ok(address) => address,
                Err(fault) => return Err(ConnectionError::Machine(fault.to_string())),
            };

            match UnixStream::connect_addr(&address) {
                Ok(stream) => Ok(stream),
                Err(fault) => Err(ConnectionError::Machine(fault.to_string())),
            }
        }
        (None, None) => Err(ConnectionError::Nowhere),
    }
}
