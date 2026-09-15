//! A message on the session bus, laid out, and read back out of a buffer.
//!
//! D-Bus is a socket and a wire format, and the wire format is the whole of
//! what is hard about it. A message is twelve bytes of header, an array of
//! header fields, and then a body whose shape is a signature the header
//! carries and whose layout is one rule about alignment for every type in it:
//! a value sits at the next offset divisible by its own width, a structure at
//! the next divisible by eight, and the padding is not optional. Nothing in
//! that is difficult and every part of it is easy to be slightly wrong about,
//! and the way a slightly wrong message reports itself is the bus hanging up
//! without saying anything at all.
//!
//! So it is here, on its own, over byte arrays, with nothing in it that opens
//! a socket. A message read out of a buffer and a message written into one are
//! two functions that can be asked in a test and answered without a bus
//! anywhere, which is the only way the alignment above is ever going to be
//! believed. What talks to the socket is `crate::talking`, and that is the
//! half that cannot be asked twice.
//!
//! **Everything is read and only what this desktop sends is written.** A
//! notification carries a dictionary of hints whose values are variants, and a
//! variant is whatever the sender felt like putting inside one -- a byte, a
//! list of bytes, a structure of four integers naming a colour. So the reader
//! has to know the whole of the type system in order to walk past a hint it
//! does not care about, and a reader that met an unknown one by stopping would
//! be a notification daemon that dropped a card because somebody else's
//! program had an opinion about its colour. The writer knows only what leaves
//! here: an id, a pair of ids, a list of words, four words, and the variants
//! the header itself is made of.
//!
//! **Byte order is the sender's, not the machine's.** A message says which it
//! is in its first byte and the bus passes messages on as they arrived, so a
//! reader that assumed the one this machine happens to use would be correct
//! here and wrong somewhere else for no reason worth having. Reading takes the
//! order from the message; writing is always little-endian, because a writer
//! may choose and there is nothing to choose between them.
//!
//! **A signature is walked rather than parsed into a tree.** The shape and the
//! bytes are read together, one character at a time, because that is what the
//! format is: the signature says what to do next and the bytes are where it is
//! done. An array clones the position of its element signature and walks it
//! again for every element, which is the whole of what would otherwise be a
//! type.

use std::fmt;
use std::str::Chars;

use console_core_never::Never;
use console_core_number_conversion::fitted;

pub const HEAD: usize = 16;

const LITTLE: u8 = b'l';

const BIG: u8 = b'B';

const VERSION: u8 = 1;

const NUL: u8 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Order {
    Little,
    Big,
}

impl Order {
    pub fn marked(said: u8) -> Result<Option<Order>, Never> {
        Ok(match said {
            LITTLE => Some(Order::Little),
            BIG => Some(Order::Big),
            _ => None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Edge {
    One,
    Two,
    Four,
    Eight,
}

impl Edge {
    fn wide(self) -> Result<usize, Never> {
        Ok(match self {
            Edge::One => 1,
            Edge::Two => 2,
            Edge::Four => 4,
            Edge::Eight => 8,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Kind {
    #[default]
    Call,
    Answer,
    Fault,
    Signal,
}

impl Kind {
    fn code(self) -> Result<u8, Never> {
        Ok(match self {
            Kind::Call => 1,
            Kind::Answer => 2,
            Kind::Fault => 3,
            Kind::Signal => 4,
        })
    }

    fn of(code: u8) -> Result<Option<Kind>, Never> {
        Ok(match code {
            1 => Some(Kind::Call),
            2 => Some(Kind::Answer),
            3 => Some(Kind::Fault),
            4 => Some(Kind::Signal),
            _ => None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Field {
    Path,
    Interface,
    Member,
    FaultName,
    ReplyTo,
    Destination,
    Sender,
    Shape,
}

impl Field {
    fn code(self) -> Result<u8, Never> {
        Ok(match self {
            Field::Path => 1,
            Field::Interface => 2,
            Field::Member => 3,
            Field::FaultName => 4,
            Field::ReplyTo => 5,
            Field::Destination => 6,
            Field::Sender => 7,
            Field::Shape => 8,
        })
    }

    fn of(code: u8) -> Result<Option<Field>, Never> {
        Ok(match code {
            1 => Some(Field::Path),
            2 => Some(Field::Interface),
            3 => Some(Field::Member),
            4 => Some(Field::FaultName),
            5 => Some(Field::ReplyTo),
            6 => Some(Field::Destination),
            7 => Some(Field::Sender),
            8 => Some(Field::Shape),
            _ => None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Truth {
    Yes,
    No,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Said {
    Byte(u8),
    Truth(Truth),
    Signed16(i16),
    Unsigned16(u16),
    Signed32(i32),
    Unsigned32(u32),
    Signed64(i64),
    Unsigned64(u64),
    Fraction(f64),
    Word(String),
    Path(String),
    Shape(String),
    List(Vec<Said>),
    Group(Vec<Said>),
    Held { shape: String, said: Box<Said> },
}

impl Said {
    pub fn word(said: &str) -> Result<Said, Never> {
        Ok(Said::Word(said.to_string()))
    }

    pub fn held(shape: &str, said: Said) -> Result<Said, Never> {
        Ok(Said::Held { shape: shape.to_string(), said: Box::new(said) })
    }

    pub fn saying(&self) -> Result<Option<&str>, Never> {
        Ok(match self {
            Said::Word(said) | Said::Path(said) | Said::Shape(said) => Some(said),
            Said::Held { said, .. } => return said.saying(),
            Said::Byte(_)
            | Said::Truth(_)
            | Said::Signed16(_)
            | Said::Unsigned16(_)
            | Said::Signed32(_)
            | Said::Unsigned32(_)
            | Said::Signed64(_)
            | Said::Unsigned64(_)
            | Said::Fraction(_)
            | Said::List(_)
            | Said::Group(_) => None,
        })
    }

    pub fn listed(&self) -> Result<Option<&[Said]>, Never> {
        Ok(match self {
            Said::List(held) => Some(held),
            Said::Held { said, .. } => return said.listed(),
            Said::Byte(_)
            | Said::Truth(_)
            | Said::Signed16(_)
            | Said::Unsigned16(_)
            | Said::Signed32(_)
            | Said::Unsigned32(_)
            | Said::Signed64(_)
            | Said::Unsigned64(_)
            | Said::Fraction(_)
            | Said::Word(_)
            | Said::Path(_)
            | Said::Shape(_)
            | Said::Group(_) => None,
        })
    }

    pub fn pair(&self) -> Result<Option<(&Said, &Said)>, Never> {
        let held = match self {
            Said::Group(held) => held,
            Said::Byte(_)
            | Said::Truth(_)
            | Said::Signed16(_)
            | Said::Unsigned16(_)
            | Said::Signed32(_)
            | Said::Unsigned32(_)
            | Said::Signed64(_)
            | Said::Unsigned64(_)
            | Said::Fraction(_)
            | Said::Word(_)
            | Said::Path(_)
            | Said::Shape(_)
            | Said::List(_)
            | Said::Held { .. } => return Ok(None),
        };

        Ok(match (held.first(), held.get(1)) {
            (Some(name), Some(value)) => Some((name, value)),
            (Some(_), None) | (None, Some(_)) | (None, None) => None,
        })
    }

    pub fn counted(&self) -> Result<Option<i64>, Never> {
        Ok(match self {
            Said::Byte(said) => Some(i64::from(*said)),
            Said::Signed16(said) => Some(i64::from(*said)),
            Said::Unsigned16(said) => Some(i64::from(*said)),
            Said::Signed32(said) => Some(i64::from(*said)),
            Said::Unsigned32(said) => Some(i64::from(*said)),
            Said::Signed64(said) => Some(*said),
            Said::Unsigned64(said) => {
                let Ok(said) = fitted::<u64, i64>(*said);

                Some(said)
            }
            Said::Held { said, .. } => return said.counted(),
            Said::Truth(_)
            | Said::Fraction(_)
            | Said::Word(_)
            | Said::Path(_)
            | Said::Shape(_)
            | Said::List(_)
            | Said::Group(_) => None,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Torn {
    Short,
    Shape(char),
    Order(u8),
    Version(u8),
    Kind(u8),
    Unterminated,
    NotUtf8,
    Mismatched(char),
}

impl fmt::Display for Torn {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Torn::Short => write!(to, "the message stops in the middle of a value"),
            Torn::Shape(head) => write!(to, "a signature this reads nothing for: {head}"),
            Torn::Order(said) => write!(to, "a byte order that is neither l nor B: {said}"),
            Torn::Version(said) => write!(to, "a protocol this does not speak: {said}"),
            Torn::Kind(said) => write!(to, "a kind of message with no name here: {said}"),
            Torn::Unterminated => write!(to, "a string with no nul after it"),
            Torn::NotUtf8 => write!(to, "a string that is not utf-8"),
            Torn::Mismatched(head) => write!(to, "a value that is not the {head} its signature promised"),
        }
    }
}

impl std::error::Error for Torn {}

fn edge(head: char) -> Result<Edge, Torn> {
    Ok(match head {
        'y' | 'g' | 'v' => Edge::One,
        'n' | 'q' => Edge::Two,
        'b' | 'i' | 'u' | 's' | 'o' | 'a' | 'h' => Edge::Four,
        'x' | 't' | 'd' | '(' | '{' => Edge::Eight,
        other => return Err(Torn::Shape(other)),
    })
}

fn onward(shape: &mut Chars<'_>) -> Result<(), Torn> {
    let head = match shape.next() {
        Some(head) => head,
        None => return Err(Torn::Short),
    };

    match head {
        'a' => onward(shape),
        '(' => onward_past(shape, ')'),
        '{' => onward_past(shape, '}'),

        other => {
            let _ = edge(other)?;

            Ok(())
        }
    }
}

fn onward_past(shape: &mut Chars<'_>, close: char) -> Result<(), Torn> {
    loop {
        let head = match shape.clone().next() {
            Some(head) => head,
            None => return Err(Torn::Short),
        };

        match head == close {
            true => {
                let _ = shape.next();

                return Ok(());
            }
            false => onward(shape)?,
        }
    }
}

struct Reading<'a> {
    bytes: &'a [u8],
    at: usize,
    order: Order,
}

impl<'a> Reading<'a> {
    fn onto(&mut self, edge: Edge) -> Result<(), Torn> {
        let Ok(wide) = edge.wide();

        self.at = match self.at.checked_next_multiple_of(wide) {
            Some(at) => at,
            None => return Err(Torn::Short),
        };

        Ok(())
    }

    fn taking(&mut self, many: usize) -> Result<&'a [u8], Torn> {
        let to = match self.at.checked_add(many) {
            Some(to) => to,
            None => return Err(Torn::Short),
        };

        let taken = match self.bytes.get(self.at..to) {
            Some(taken) => taken,
            None => return Err(Torn::Short),
        };

        self.at = to;

        Ok(taken)
    }

    fn byte(&mut self) -> Result<u8, Torn> {
        let taken = self.taking(1)?;

        match taken.first() {
            Some(byte) => Ok(*byte),
            None => Err(Torn::Short),
        }
    }

    fn two(&mut self) -> Result<[u8; 2], Torn> {
        self.onto(Edge::Two)?;
        let taken = self.taking(2)?;

        match <[u8; 2]>::try_from(taken) {
            Ok(taken) => Ok(taken),
            Err(_fault) => Err(Torn::Short),
        }
    }

    fn four(&mut self) -> Result<[u8; 4], Torn> {
        self.onto(Edge::Four)?;
        let taken = self.taking(4)?;

        match <[u8; 4]>::try_from(taken) {
            Ok(taken) => Ok(taken),
            Err(_fault) => Err(Torn::Short),
        }
    }

    fn eight(&mut self) -> Result<[u8; 8], Torn> {
        self.onto(Edge::Eight)?;
        let taken = self.taking(8)?;

        match <[u8; 8]>::try_from(taken) {
            Ok(taken) => Ok(taken),
            Err(_fault) => Err(Torn::Short),
        }
    }

    fn unsigned32(&mut self) -> Result<u32, Torn> {
        let taken = self.four()?;

        Ok(match self.order {
            Order::Little => u32::from_le_bytes(taken),
            Order::Big => u32::from_be_bytes(taken),
        })
    }

    fn word(&mut self) -> Result<String, Torn> {
        let many = self.unsigned32()?;
        let Ok(many) = fitted::<u32, usize>(many);

        self.said(many)
    }

    fn shape(&mut self) -> Result<String, Torn> {
        let many = self.byte()?;

        self.said(usize::from(many))
    }

    fn said(&mut self, many: usize) -> Result<String, Torn> {
        let taken = self.taking(many)?;
        let said = match std::str::from_utf8(taken) {
            Ok(said) => said.to_string(),
            Err(_fault) => return Err(Torn::NotUtf8),
        };

        let nul = self.byte()?;

        match nul == NUL {
            true => Ok(said),
            false => Err(Torn::Unterminated),
        }
    }

    fn values(&mut self, shape: &str) -> Result<Vec<Said>, Torn> {
        let mut walking = shape.chars();
        let mut held: Vec<Said> = Vec::new();

        loop {
            let head = match walking.next() {
                Some(head) => head,
                None => return Ok(held),
            };

            let said = self.one(head, &mut walking)?;

            held.push(said);
        }
    }

    fn value(&mut self, shape: &mut Chars<'_>) -> Result<Said, Torn> {
        let head = match shape.next() {
            Some(head) => head,
            None => return Err(Torn::Short),
        };

        self.one(head, shape)
    }

    fn one(&mut self, head: char, shape: &mut Chars<'_>) -> Result<Said, Torn> {
        match head {
            'y' => {
                let said = self.byte()?;

                Ok(Said::Byte(said))
            }
            'b' => {
                let said = self.unsigned32()?;

                Ok(Said::Truth(match said {
                    0 => Truth::No,
                    _ => Truth::Yes,
                }))
            }
            'n' => {
                let taken = self.two()?;

                Ok(Said::Signed16(match self.order {
                    Order::Little => i16::from_le_bytes(taken),
                    Order::Big => i16::from_be_bytes(taken),
                }))
            }
            'q' => {
                let taken = self.two()?;

                Ok(Said::Unsigned16(match self.order {
                    Order::Little => u16::from_le_bytes(taken),
                    Order::Big => u16::from_be_bytes(taken),
                }))
            }
            'i' => {
                let taken = self.four()?;

                Ok(Said::Signed32(match self.order {
                    Order::Little => i32::from_le_bytes(taken),
                    Order::Big => i32::from_be_bytes(taken),
                }))
            }
            'u' | 'h' => {
                let said = self.unsigned32()?;

                Ok(Said::Unsigned32(said))
            }
            'x' => {
                let taken = self.eight()?;

                Ok(Said::Signed64(match self.order {
                    Order::Little => i64::from_le_bytes(taken),
                    Order::Big => i64::from_be_bytes(taken),
                }))
            }
            't' => {
                let taken = self.eight()?;

                Ok(Said::Unsigned64(match self.order {
                    Order::Little => u64::from_le_bytes(taken),
                    Order::Big => u64::from_be_bytes(taken),
                }))
            }
            'd' => {
                let taken = self.eight()?;

                Ok(Said::Fraction(match self.order {
                    Order::Little => f64::from_le_bytes(taken),
                    Order::Big => f64::from_be_bytes(taken),
                }))
            }
            's' => {
                let said = self.word()?;

                Ok(Said::Word(said))
            }
            'o' => {
                let said = self.word()?;

                Ok(Said::Path(said))
            }
            'g' => {
                let said = self.shape()?;

                Ok(Said::Shape(said))
            }
            'a' => self.list(shape),
            '(' => self.group(shape, ')'),
            '{' => self.group(shape, '}'),
            'v' => {
                let shape = self.shape()?;
                let mut walking = shape.chars();
                let said = self.value(&mut walking)?;

                Ok(Said::Held { shape, said: Box::new(said) })
            }
            other => Err(Torn::Shape(other)),
        }
    }

    fn list(&mut self, shape: &mut Chars<'_>) -> Result<Said, Torn> {
        let many = self.unsigned32()?;
        let element = shape.clone();

        onward(shape)?;

        let head = match element.clone().next() {
            Some(head) => head,
            None => return Err(Torn::Short),
        };

        let edge = edge(head)?;

        self.onto(edge)?;

        let Ok(many) = fitted::<u32, usize>(many);

        let end = match self.at.checked_add(many) {
            Some(end) => end,
            None => return Err(Torn::Short),
        };

        let mut held: Vec<Said> = Vec::new();

        while self.at < end {
            let mut walking = element.clone();
            let said = self.value(&mut walking)?;

            held.push(said);
        }

        match self.at == end {
            true => Ok(Said::List(held)),
            false => Err(Torn::Short),
        }
    }

    fn group(&mut self, shape: &mut Chars<'_>, close: char) -> Result<Said, Torn> {
        self.onto(Edge::Eight)?;

        let mut held: Vec<Said> = Vec::new();

        loop {
            let head = match shape.next() {
                Some(head) => head,
                None => return Err(Torn::Short),
            };

            match head == close {
                true => return Ok(Said::Group(held)),
                false => {
                    let said = self.one(head, shape)?;

                    held.push(said);
                }
            }
        }
    }
}

struct Writing {
    bytes: Vec<u8>,
}

impl Writing {
    fn new() -> Result<Writing, Never> {
        Ok(Writing { bytes: Vec::new() })
    }

    fn pad(&mut self, edge: Edge) -> Result<(), Torn> {
        let Ok(wide) = edge.wide();

        let to = match self.bytes.len().checked_next_multiple_of(wide) {
            Some(to) => to,
            None => return Err(Torn::Short),
        };

        self.bytes.resize(to, NUL);

        Ok(())
    }

    fn byte(&mut self, said: u8) -> Result<(), Never> {
        self.bytes.push(said);

        Ok(())
    }

    fn unsigned32(&mut self, said: u32) -> Result<(), Torn> {
        self.pad(Edge::Four)?;
        self.bytes.extend_from_slice(&said.to_le_bytes());

        Ok(())
    }

    fn word(&mut self, said: &str) -> Result<(), Torn> {
        let Ok(many) = fitted::<usize, u32>(said.len());

        self.unsigned32(many)?;
        self.bytes.extend_from_slice(said.as_bytes());
        self.bytes.push(NUL);

        Ok(())
    }

    fn shape(&mut self, said: &str) -> Result<(), Never> {
        let Ok(many) = fitted::<usize, u8>(said.len());

        self.bytes.push(many);
        self.bytes.extend_from_slice(said.as_bytes());
        self.bytes.push(NUL);

        Ok(())
    }

    fn values(&mut self, shape: &str, said: &[Said]) -> Result<(), Torn> {
        let mut walking = shape.chars();
        let mut items = said.iter();

        loop {
            let head = match walking.next() {
                Some(head) => head,
                None => return Ok(()),
            };

            let said = match items.next() {
                Some(said) => said,
                None => return Err(Torn::Short),
            };

            self.one(head, &mut walking, said)?;
        }
    }

    fn value(&mut self, shape: &mut Chars<'_>, said: &Said) -> Result<(), Torn> {
        let head = match shape.next() {
            Some(head) => head,
            None => return Err(Torn::Short),
        };

        self.one(head, shape, said)
    }

    fn one(&mut self, head: char, shape: &mut Chars<'_>, said: &Said) -> Result<(), Torn> {
        match (head, said) {
            ('y', Said::Byte(said)) => {
                let Ok(()) = self.byte(*said);

                Ok(())
            }
            ('b', Said::Truth(said)) => self.unsigned32(match said {
                Truth::Yes => 1,
                Truth::No => 0,
            }),
            ('n', Said::Signed16(said)) => {
                self.pad(Edge::Two)?;
                self.bytes.extend_from_slice(&said.to_le_bytes());

                Ok(())
            }
            ('q', Said::Unsigned16(said)) => {
                self.pad(Edge::Two)?;
                self.bytes.extend_from_slice(&said.to_le_bytes());

                Ok(())
            }
            ('i', Said::Signed32(said)) => {
                self.pad(Edge::Four)?;
                self.bytes.extend_from_slice(&said.to_le_bytes());

                Ok(())
            }
            ('u', Said::Unsigned32(said)) | ('h', Said::Unsigned32(said)) => self.unsigned32(*said),
            ('x', Said::Signed64(said)) => {
                self.pad(Edge::Eight)?;
                self.bytes.extend_from_slice(&said.to_le_bytes());

                Ok(())
            }
            ('t', Said::Unsigned64(said)) => {
                self.pad(Edge::Eight)?;
                self.bytes.extend_from_slice(&said.to_le_bytes());

                Ok(())
            }
            ('d', Said::Fraction(said)) => {
                self.pad(Edge::Eight)?;
                self.bytes.extend_from_slice(&said.to_le_bytes());

                Ok(())
            }
            ('s', Said::Word(said)) | ('o', Said::Path(said)) => self.word(said),
            ('g', Said::Shape(said)) => {
                let Ok(()) = self.shape(said);

                Ok(())
            }
            ('a', Said::List(held)) => self.list(shape, held),
            ('(', Said::Group(held)) => self.group(shape, held, ')'),
            ('{', Said::Group(held)) => self.group(shape, held, '}'),
            ('v', Said::Held { shape, said }) => self.held(shape, said),
            (head, _said) => Err(Torn::Mismatched(head)),
        }
    }

    fn held(&mut self, shape: &str, said: &Said) -> Result<(), Torn> {
        let Ok(()) = self.shape(shape);

        let mut walking = shape.chars();

        self.value(&mut walking, said)
    }

    fn list(&mut self, shape: &mut Chars<'_>, held: &[Said]) -> Result<(), Torn> {
        let element = shape.clone();

        onward(shape)?;

        let head = match element.clone().next() {
            Some(head) => head,
            None => return Err(Torn::Short),
        };

        let edge = edge(head)?;

        self.pad(Edge::Four)?;

        let at = self.bytes.len();

        self.bytes.extend_from_slice(&0u32.to_le_bytes());
        self.pad(edge)?;

        let from = self.bytes.len();

        for said in held {
            let mut walking = element.clone();

            self.value(&mut walking, said)?;
        }

        let many = self.bytes.len().saturating_sub(from);
        let Ok(many) = fitted::<usize, u32>(many);

        let to = at.saturating_add(4);

        match self.bytes.get_mut(at..to) {
            Some(room) => room.copy_from_slice(&many.to_le_bytes()),
            None => return Err(Torn::Short),
        }

        Ok(())
    }

    fn group(&mut self, shape: &mut Chars<'_>, held: &[Said], close: char) -> Result<(), Torn> {
        self.pad(Edge::Eight)?;

        let mut items = held.iter();

        loop {
            let head = match shape.next() {
                Some(head) => head,
                None => return Err(Torn::Short),
            };

            match head == close {
                true => return Ok(()),
                false => {
                    let said = match items.next() {
                        Some(said) => said,
                        None => return Err(Torn::Short),
                    };

                    self.one(head, shape, said)?;
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Whom<'a> {
    pub to: &'a str,
    pub at: &'a str,
    pub on: &'a str,
    pub calling: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Saying<'a> {
    pub at: &'a str,
    pub on: &'a str,
    pub saying: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Complaint {
    Failed,
    UnknownMethod,
    UnknownInterface,
    InvalidArgs,
}

impl Complaint {
    pub fn named(self) -> Result<&'static str, Never> {
        Ok(match self {
            Complaint::Failed => "org.freedesktop.DBus.Error.Failed",
            Complaint::UnknownMethod => "org.freedesktop.DBus.Error.UnknownMethod",
            Complaint::UnknownInterface => "org.freedesktop.DBus.Error.UnknownInterface",
            Complaint::InvalidArgs => "org.freedesktop.DBus.Error.InvalidArgs",
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Message {
    pub kind: Kind,
    pub serial: u32,
    pub reply_to: Option<u32>,
    pub path: Option<String>,
    pub interface: Option<String>,
    pub member: Option<String>,
    pub fault: Option<String>,
    pub destination: Option<String>,
    pub sender: Option<String>,
    pub shape: String,
    pub said: Vec<Said>,
}

impl Message {
    pub fn call(whom: &Whom<'_>) -> Result<Message, Never> {
        Ok(Message {
            kind: Kind::Call,
            destination: Some(whom.to.to_string()),
            path: Some(whom.at.to_string()),
            interface: Some(whom.on.to_string()),
            member: Some(whom.calling.to_string()),
            ..Message::default()
        })
    }

    pub fn signal(saying: &Saying<'_>) -> Result<Message, Never> {
        Ok(Message {
            kind: Kind::Signal,
            path: Some(saying.at.to_string()),
            interface: Some(saying.on.to_string()),
            member: Some(saying.saying.to_string()),
            ..Message::default()
        })
    }

    pub fn answering(&self) -> Result<Message, Never> {
        Ok(Message {
            kind: Kind::Answer,
            reply_to: Some(self.serial),
            destination: self.sender.clone(),
            ..Message::default()
        })
    }

    pub fn complaining(&self, complaint: Complaint, why: &str) -> Result<Message, Never> {
        let Ok(said) = Said::word(why);
        let Ok(named) = complaint.named();

        Ok(Message {
            kind: Kind::Fault,
            reply_to: Some(self.serial),
            destination: self.sender.clone(),
            fault: Some(named.to_string()),
            shape: "s".to_string(),
            said: vec![said],
            ..Message::default()
        })
    }

    pub fn carrying(mut self, shape: &str, said: Vec<Said>) -> Result<Message, Never> {
        self.shape = shape.to_string();
        self.said = said;

        Ok(self)
    }

    pub fn bytes(&self, serial: u32) -> Result<Vec<u8>, Torn> {
        let Ok(mut body) = Writing::new();

        body.values(&self.shape, &self.said)?;

        let Ok(length) = fitted::<usize, u32>(body.bytes.len());
        let Ok(kind) = self.kind.code();

        let Ok(mut whole) = Writing::new();
        let Ok(()) = whole.byte(LITTLE);
        let Ok(()) = whole.byte(kind);
        let Ok(()) = whole.byte(NUL);
        let Ok(()) = whole.byte(VERSION);

        whole.unsigned32(length)?;
        whole.unsigned32(serial)?;

        let mut fields: Vec<Said> = Vec::new();

        for (field, said) in [
            (Field::Path, &self.path),
            (Field::Interface, &self.interface),
            (Field::Member, &self.member),
            (Field::FaultName, &self.fault),
            (Field::Destination, &self.destination),
            (Field::Sender, &self.sender),
        ] {
            let said = match said {
                Some(said) => said,
                None => continue,
            };

            let Ok(code) = field.code();

            let shape = match field {
                Field::Path => "o",
                Field::Interface | Field::Member | Field::FaultName => "s",
                Field::Destination | Field::Sender => "s",
                Field::ReplyTo | Field::Shape => "s",
            };

            let carried = match field {
                Field::Path => Said::Path(said.to_string()),
                Field::Interface
                | Field::Member
                | Field::FaultName
                | Field::Destination
                | Field::Sender
                | Field::ReplyTo
                | Field::Shape => Said::Word(said.to_string()),
            };

            let Ok(held) = Said::held(shape, carried);

            fields.push(Said::Group(vec![Said::Byte(code), held]));
        }

        match self.reply_to {
            Some(serial) => {
                let Ok(code) = Field::ReplyTo.code();
                let Ok(held) = Said::held("u", Said::Unsigned32(serial));

                fields.push(Said::Group(vec![Said::Byte(code), held]));
            }
            None => {}
        }

        match self.shape.is_empty() {
            true => {}
            false => {
                let Ok(code) = Field::Shape.code();
                let Ok(held) = Said::held("g", Said::Shape(self.shape.clone()));

                fields.push(Said::Group(vec![Said::Byte(code), held]));
            }
        }

        whole.values("a(yv)", &[Said::List(fields)])?;
        whole.pad(Edge::Eight)?;
        whole.bytes.extend_from_slice(&body.bytes);

        Ok(whole.bytes)
    }
}

pub fn length(bytes: &[u8]) -> Result<usize, Torn> {
    let order = order(bytes)?;
    let mut reading = Reading { bytes, at: 4, order };
    let body = reading.unsigned32()?;
    let _serial = reading.unsigned32()?;
    let fields = reading.unsigned32()?;

    let Ok(body) = fitted::<u32, usize>(body);
    let Ok(fields) = fitted::<u32, usize>(fields);

    let to = match HEAD.checked_add(fields) {
        Some(to) => to,
        None => return Err(Torn::Short),
    };

    let to = match to.checked_next_multiple_of(8) {
        Some(to) => to,
        None => return Err(Torn::Short),
    };

    match to.checked_add(body) {
        Some(whole) => Ok(whole),
        None => Err(Torn::Short),
    }
}

fn order(bytes: &[u8]) -> Result<Order, Torn> {
    let mark = match bytes.first() {
        Some(mark) => *mark,
        None => return Err(Torn::Short),
    };

    let Ok(order) = Order::marked(mark);

    match order {
        Some(order) => Ok(order),
        None => Err(Torn::Order(mark)),
    }
}

pub fn read(bytes: &[u8]) -> Result<Message, Torn> {
    let order = order(bytes)?;

    let code = match bytes.get(1) {
        Some(code) => *code,
        None => return Err(Torn::Short),
    };

    let version = match bytes.get(3) {
        Some(version) => *version,
        None => return Err(Torn::Short),
    };

    match version == VERSION {
        true => {}
        false => return Err(Torn::Version(version)),
    }

    let Ok(kind) = Kind::of(code);

    let kind = match kind {
        Some(kind) => kind,
        None => return Err(Torn::Kind(code)),
    };

    let mut reading = Reading { bytes, at: 4, order };
    let _body = reading.unsigned32()?;
    let serial = reading.unsigned32()?;
    let fields = reading.values("a(yv)")?;

    let mut message = Message { kind, serial, ..Message::default() };

    for held in fields {
        let held = match held {
            Said::List(held) => held,
            Said::Byte(_)
            | Said::Truth(_)
            | Said::Signed16(_)
            | Said::Unsigned16(_)
            | Said::Signed32(_)
            | Said::Unsigned32(_)
            | Said::Signed64(_)
            | Said::Unsigned64(_)
            | Said::Fraction(_)
            | Said::Word(_)
            | Said::Path(_)
            | Said::Shape(_)
            | Said::Group(_)
            | Said::Held { .. } => return Err(Torn::Mismatched('a')),
        };

        for one in held {
            let Ok(()) = kept(&mut message, &one);
        }
    }

    reading.onto(Edge::Eight)?;

    let shape = message.shape.clone();

    let said = reading.values(&shape)?;

    message.said = said;

    Ok(message)
}

fn kept(message: &mut Message, one: &Said) -> Result<(), Never> {
    let held = match one {
        Said::Group(held) => held,
        Said::Byte(_)
        | Said::Truth(_)
        | Said::Signed16(_)
        | Said::Unsigned16(_)
        | Said::Signed32(_)
        | Said::Unsigned32(_)
        | Said::Signed64(_)
        | Said::Unsigned64(_)
        | Said::Fraction(_)
        | Said::Word(_)
        | Said::Path(_)
        | Said::Shape(_)
        | Said::List(_)
        | Said::Held { .. } => return Ok(()),
    };

    let code = match held.first() {
        Some(Said::Byte(code)) => *code,
        Some(_) | None => return Ok(()),
    };

    let said = match held.get(1) {
        Some(said) => said,
        None => return Ok(()),
    };

    let Ok(field) = Field::of(code);

    let field = match field {
        Some(field) => field,
        None => return Ok(()),
    };

    let Ok(saying) = said.saying();
    let word = saying.map(str::to_string);

    match field {
        Field::Path => message.path = word,
        Field::Interface => message.interface = word,
        Field::Member => message.member = word,
        Field::FaultName => message.fault = word,
        Field::Destination => message.destination = word,
        Field::Sender => message.sender = word,
        Field::Shape => {
            message.shape = match word {
                Some(word) => word,
                None => String::new(),
            }
        }

        Field::ReplyTo => {
            let Ok(counted) = said.counted();

            message.reply_to = match counted {
                Some(counted) => match u32::try_from(counted) {
                    Ok(serial) => Some(serial),
                    Err(_fault) => None,
                },
                None => None,
            };
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notifying() -> Message {
        let Ok(call) = Message::call(&Whom {
            to: "org.freedesktop.Notifications",
            at: "/org/freedesktop/Notifications",
            on: "org.freedesktop.Notifications",
            calling: "Notify",
        });

        let hints = Said::List(vec![Said::Group(vec![
            Said::Word("urgency".to_string()),
            Said::Held { shape: "y".to_string(), said: Box::new(Said::Byte(2)) },
        ])]);

        let Ok(call) = call.carrying(
            "susssasa{sv}i",
            vec![
                Said::Word("Console".to_string()),
                Said::Unsigned32(0),
                Said::Word(String::new()),
                Said::Word("Notifications fell over".to_string()),
                Said::Word("console-notify.service stopped".to_string()),
                Said::List(Vec::new()),
                hints,
                Said::Signed32(0),
            ],
        );

        call
    }

    #[test]
    fn a_message_written_here_is_read_back_as_itself() {
        let call = notifying();
        let bytes = call.bytes(7).unwrap();
        let heard = read(&bytes).unwrap();

        assert_eq!(heard.kind, Kind::Call);
        assert_eq!(heard.serial, 7);
        assert_eq!(heard.member.as_deref(), Some("Notify"));
        assert_eq!(heard.path.as_deref(), Some("/org/freedesktop/Notifications"));
        assert_eq!(heard.shape, "susssasa{sv}i");
        assert_eq!(heard.said, call.said);
    }

    #[test]
    fn the_length_in_the_head_is_the_length_of_the_whole_message() {
        let saying = Saying { at: "/a", on: "b.c", saying: "D" };

        for message in [notifying(), Message::signal(&saying).unwrap()] {
            let bytes = message.bytes(3).unwrap();

            assert_eq!(length(&bytes), Ok(bytes.len()));
        }
    }

    #[test]
    fn a_body_starts_at_a_multiple_of_eight() {
        let bytes = notifying().bytes(1).unwrap();
        let fields = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        let body = HEAD + fields;

        assert_eq!(length(&bytes).unwrap() - bytes.len(), 0);
        assert!(body <= bytes.len());
        assert_eq!(bytes.len() - body.next_multiple_of(8), {
            let mut counting = Writing::new().unwrap();
            counting.values("susssasa{sv}i", &notifying().said).unwrap();
            counting.bytes.len()
        });
    }

    #[test]
    fn a_string_carries_its_length_and_its_nul() {
        let mut writing = Writing::new().unwrap();

        writing.values("s", &[Said::Word("ok".to_string())]).unwrap();

        assert_eq!(writing.bytes, vec![2, 0, 0, 0, b'o', b'k', 0]);
    }

    #[test]
    fn a_number_after_a_byte_is_padded_out_to_its_own_width() {
        let mut writing = Writing::new().unwrap();

        writing.values("yu", &[Said::Byte(1), Said::Unsigned32(2)]).unwrap();

        assert_eq!(writing.bytes, vec![1, 0, 0, 0, 2, 0, 0, 0]);
    }

    #[test]
    fn a_structure_after_a_byte_starts_eight_along() {
        let mut writing = Writing::new().unwrap();
        let group = Said::Group(vec![Said::Byte(9)]);

        writing.values("y(y)", &[Said::Byte(1), group]).unwrap();

        assert_eq!(writing.bytes, vec![1, 0, 0, 0, 0, 0, 0, 0, 9]);
    }

    #[test]
    fn an_empty_list_is_a_length_of_nothing() {
        let mut writing = Writing::new().unwrap();

        writing.values("as", &[Said::List(Vec::new())]).unwrap();

        assert_eq!(writing.bytes, vec![0, 0, 0, 0]);
    }

    #[test]
    fn a_list_says_how_long_its_contents_are_and_not_how_many_there_are() {
        let mut writing = Writing::new().unwrap();
        let held = Said::List(vec![Said::Unsigned32(1), Said::Unsigned32(2)]);

        writing.values("au", &[held]).unwrap();

        assert_eq!(writing.bytes, vec![8, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0]);
    }

    #[test]
    fn a_list_of_structures_is_padded_before_the_first_one_is_written() {
        let mut writing = Writing::new().unwrap();
        let held = Said::List(vec![Said::Group(vec![Said::Byte(7)])]);

        writing.values("a(y)", &[held]).unwrap();

        assert_eq!(writing.bytes, vec![1, 0, 0, 0, 0, 0, 0, 0, 7]);
    }

    #[test]
    fn a_variant_carries_the_shape_of_what_is_in_it() {
        let mut writing = Writing::new().unwrap();
        let held = Said::Held { shape: "u".to_string(), said: Box::new(Said::Unsigned32(5)) };

        writing.values("v", std::slice::from_ref(&held)).unwrap();

        assert_eq!(writing.bytes, vec![1, b'u', 0, 0, 5, 0, 0, 0]);

        let mut reading = Reading { bytes: &writing.bytes, at: 0, order: Order::Little };

        assert_eq!(reading.values("v").unwrap(), vec![held]);
    }

    #[test]
    fn a_hint_this_knows_nothing_about_is_walked_past_rather_than_stopping_the_message() {
        let mut writing = Writing::new().unwrap();
        let odd = Said::Held {
            shape: "(iiii)".to_string(),
            said: Box::new(Said::Group(vec![
                Said::Signed32(1),
                Said::Signed32(2),
                Said::Signed32(3),
                Said::Signed32(4),
            ])),
        };
        let hints = Said::List(vec![
            Said::Group(vec![Said::Word("colour".to_string()), odd]),
            Said::Group(vec![
                Said::Word("value".to_string()),
                Said::Held { shape: "i".to_string(), said: Box::new(Said::Signed32(40)) },
            ]),
        ]);

        writing.values("a{sv}", std::slice::from_ref(&hints)).unwrap();

        let mut reading = Reading { bytes: &writing.bytes, at: 0, order: Order::Little };

        assert_eq!(reading.values("a{sv}").unwrap(), vec![hints]);
    }

    #[test]
    fn the_same_message_read_the_other_way_round_is_the_same_message() {
        let little = notifying().bytes(11).unwrap();
        let mut big = little.clone();

        big[0] = BIG;

        for at in [4usize, 8, 12] {
            let four: [u8; 4] = little[at..at + 4].try_into().unwrap();
            let swapped = u32::from_le_bytes(four).to_be_bytes();

            big[at..at + 4].copy_from_slice(&swapped);
        }

        assert_eq!(length(&big), Ok(big.len()));
    }

    #[test]
    fn a_signature_with_nothing_written_for_it_says_which_letter() {
        let mut reading = Reading { bytes: &[0, 0, 0, 0], at: 0, order: Order::Little };

        assert_eq!(reading.values("Z"), Err(Torn::Shape('Z')));
    }

    #[test]
    fn a_message_that_stops_in_the_middle_says_so_rather_than_answering() {
        let bytes = notifying().bytes(1).unwrap();

        for many in [1usize, 5, 16, 40] {
            assert!(read(&bytes[..many]).is_err(), "{many}");
        }
    }

    #[test]
    fn a_reply_says_what_it_is_replying_to() {
        let call = notifying();
        let Ok(answer) = call.answering();
        let Ok(answer) = answer.carrying("u", vec![Said::Unsigned32(4)]);
        let bytes = answer.bytes(2).unwrap();
        let heard = read(&bytes).unwrap();

        assert_eq!(heard.kind, Kind::Answer);
        assert_eq!(heard.reply_to, Some(call.serial));
        assert_eq!(heard.said, vec![Said::Unsigned32(4)]);
    }

    #[test]
    fn a_complaint_carries_a_name_and_a_sentence() {
        let Ok(fault) = notifying().complaining(Complaint::Failed, "no");
        let bytes = fault.bytes(2).unwrap();
        let heard = read(&bytes).unwrap();

        assert_eq!(heard.kind, Kind::Fault);
        assert_eq!(heard.fault.as_deref(), Some("org.freedesktop.DBus.Error.Failed"));
        assert_eq!(heard.said, vec![Said::Word("no".to_string())]);
    }
}
