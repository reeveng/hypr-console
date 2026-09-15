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

use console_core_never::Never;

use crate::messages::{self, HEAD, Message, Said, Torn, Whom};

pub const BUS: &str = "org.freedesktop.DBus";

pub const BUS_PATH: &str = "/org/freedesktop/DBus";

const ADDRESS: &str = "DBUS_SESSION_BUS_ADDRESS";

const TAKING: u32 = 6;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Wire {
    Nowhere,
    Machine(String),
    Refused(String),
    Ended,
    Torn(Torn),
}

impl fmt::Display for Wire {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Wire::Nowhere => write!(to, "there is no session bus in this environment"),
            Wire::Machine(why) => write!(to, "the socket would not: {why}"),
            Wire::Refused(why) => write!(to, "the bus refused: {why}"),
            Wire::Ended => write!(to, "the bus hung up"),
            Wire::Torn(torn) => write!(to, "{torn}"),
        }
    }
}

impl std::error::Error for Wire {}

impl From<Torn> for Wire {
    fn from(torn: Torn) -> Self {
        Wire::Torn(torn)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Got {
    Ours,
    Queued,
    Taken,
    Already,
}

impl Got {
    fn of(said: u32) -> Result<Option<Got>, Never> {
        Ok(match said {
            1 => Some(Got::Ours),
            2 => Some(Got::Queued),
            3 => Some(Got::Taken),
            4 => Some(Got::Already),
            _ => None,
        })
    }
}

pub struct Hearing {
    stream: UnixStream,
}

#[derive(Clone)]
pub struct Saying {
    held: Arc<Mutex<Speaking>>,
}

struct Speaking {
    stream: UnixStream,
    serial: u32,
}

pub struct Bus {
    hearing: Hearing,
    saying: Saying,
    named: String,
}

impl Bus {
    pub fn session() -> Result<Bus, Wire> {
        #[cfg_attr(
            dylint_lib = "explicit026_env_read_once",
            allow(
                explicit026_env_read_once,
                reason = "DBUS_SESSION_BUS_ADDRESS is this crate's own: what a session bus is, where it is and what an unset one means is the question this crate exists to answer"
            )
        )]
        let said = match std::env::var(ADDRESS) {
            Ok(said) => said,
            Err(_fault) => return Err(Wire::Nowhere),
        };

        let stream = opened(&said)?;

        let writing = match stream.try_clone() {
            Ok(writing) => writing,
            Err(fault) => return Err(Wire::Machine(fault.to_string())),
        };

        let saying = Saying { held: Arc::new(Mutex::new(Speaking { stream: writing, serial: 0 })) };
        let mut bus = Bus { hearing: Hearing { stream }, saying, named: String::new() };

        bus.greet()?;

        let Ok(hello) = Message::call(&Whom { to: BUS, at: BUS_PATH, on: BUS, calling: "Hello" });
        let answer = bus.asking(&hello)?;

        let Ok(saying) = match answer.said.first() {
            Some(said) => said.saying(),
            None => Ok(None),
        };

        bus.named = match saying {
            Some(named) => named.to_string(),
            None => String::new(),
        };

        Ok(bus)
    }

    pub fn named(&self) -> Result<&str, Never> {
        Ok(&self.named)
    }

    pub fn taking(&mut self, name: &str) -> Result<Got, Wire> {
        let Ok(asking) =
            Message::call(&Whom { to: BUS, at: BUS_PATH, on: BUS, calling: "RequestName" });
        let Ok(word) = Said::word(name);
        let Ok(asking) = asking.carrying("su", vec![word, Said::Unsigned32(TAKING)]);

        let answer = self.asking(&asking)?;

        let Ok(counted) = match answer.said.first() {
            Some(said) => said.counted(),
            None => Ok(None),
        };

        let said = match counted {
            Some(said) => said,
            None => return Err(Wire::Refused(format!("{name} was answered with nothing"))),
        };

        let said = match u32::try_from(said) {
            Ok(said) => said,
            Err(_fault) => return Err(Wire::Refused(format!("{name} was answered with {said}"))),
        };

        let Ok(got) = Got::of(said);

        match got {
            Some(got) => Ok(got),
            None => Err(Wire::Refused(format!("{name} was answered with {said}"))),
        }
    }

    pub fn say(&mut self, message: &Message) -> Result<u32, Wire> {
        self.saying.say(message)
    }

    pub fn heard(&mut self) -> Result<Message, Wire> {
        self.hearing.heard()
    }

    pub fn apart(self) -> Result<(Hearing, Saying), Never> {
        Ok((self.hearing, self.saying))
    }

    fn asking(&mut self, message: &Message) -> Result<Message, Wire> {
        let serial = self.say(message)?;

        loop {
            let heard = self.heard()?;

            match heard.reply_to == Some(serial) {
                true => {
                    let named = heard.fault.clone();

                    match named {
                        Some(named) => return Err(Wire::Refused(named)),
                        None => return Ok(heard),
                    }
                }
                false => {}
            }
        }
    }

    fn taken(&mut self, many: usize) -> Result<Vec<u8>, Wire> {
        self.hearing.taken(many)
    }

    fn greet(&mut self) -> Result<(), Wire> {
        let Ok(mine) = whoever();

        let mut said = String::from("\0AUTH EXTERNAL ");

        for byte in mine.to_string().bytes() {
            said.push_str(&format!("{byte:02x}"));
        }

        said.push_str("\r\n");

        self.saying.plainly(said.as_bytes())?;

        let answer = self.line()?;

        match answer.starts_with("OK") {
            true => {}
            false => return Err(Wire::Refused(answer)),
        }

        self.saying.plainly(b"BEGIN\r\n")
    }

    fn line(&mut self) -> Result<String, Wire> {
        let mut said = String::new();

        loop {
            let byte = self.taken(1)?;

            let byte = match byte.first() {
                Some(byte) => *byte,
                None => return Err(Wire::Ended),
            };

            match byte {
                b'\n' => return Ok(said.trim_end().to_string()),
                _ => said.push(char::from(byte)),
            }
        }
    }
}

impl Hearing {
    pub fn heard(&mut self) -> Result<Message, Wire> {
        let mut bytes = self.taken(HEAD)?;
        let whole = messages::length(&bytes)?;
        let rest = whole.saturating_sub(HEAD);
        let more = self.taken(rest)?;

        bytes.extend_from_slice(&more);

        let message = messages::read(&bytes)?;

        Ok(message)
    }

    fn taken(&mut self, many: usize) -> Result<Vec<u8>, Wire> {
        let mut held = vec![0u8; many];

        match self.stream.read_exact(&mut held) {
            Ok(()) => Ok(held),
            Err(fault) => match fault.kind() {
                std::io::ErrorKind::UnexpectedEof => Err(Wire::Ended),
                _ => Err(Wire::Machine(fault.to_string())),
            },
        }
    }
}

impl Saying {
    pub fn say(&self, message: &Message) -> Result<u32, Wire> {
        let mut held = match self.held.lock() {
            Ok(held) => held,
            Err(_poisoned) => return Err(Wire::Ended),
        };

        let serial = held.serial.saturating_add(1);

        held.serial = serial;

        let bytes = message.bytes(serial)?;

        match held.stream.write_all(&bytes) {
            Ok(()) => {}
            Err(fault) => return Err(Wire::Machine(fault.to_string())),
        }

        match held.stream.flush() {
            Ok(()) => Ok(serial),
            Err(fault) => Err(Wire::Machine(fault.to_string())),
        }
    }

    fn plainly(&self, said: &[u8]) -> Result<(), Wire> {
        let mut held = match self.held.lock() {
            Ok(held) => held,
            Err(_poisoned) => return Err(Wire::Ended),
        };

        match held.stream.write_all(said) {
            Ok(()) => Ok(()),
            Err(fault) => Err(Wire::Machine(fault.to_string())),
        }
    }
}

fn whoever() -> Result<u32, Never> {
    // SAFETY: `getuid` reads this process's own real user id and answers with
    // it; there is nothing it can be given and nothing it can fail at.
    let mine = unsafe { libc::getuid() };

    Ok(mine)
}

fn opened(said: &str) -> Result<UnixStream, Wire> {
    let first = match said.split(';').next() {
        Some(first) => first,
        None => return Err(Wire::Nowhere),
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
            Err(fault) => Err(Wire::Machine(fault.to_string())),
        },
        (None, Some(hidden)) => {
            let address = match SocketAddr::from_abstract_name(hidden.as_bytes()) {
                Ok(address) => address,
                Err(fault) => return Err(Wire::Machine(fault.to_string())),
            };

            match UnixStream::connect_addr(&address) {
                Ok(stream) => Ok(stream),
                Err(fault) => Err(Wire::Machine(fault.to_string())),
            }
        }
        (None, None) => Err(Wire::Nowhere),
    }
}
