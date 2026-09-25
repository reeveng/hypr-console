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
//! list of bytes, a structure of four integers naming a color. So the reader
//! has to know the whole of the type system in order to walk past a hint it
//! does not care about, and a reader that met an unknown one by stopping would
//! be a notification daemon that dropped a card because someone else's
//! program had an opinion about its color. The writer knows only what leaves
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

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

pub const HEAD: u32 = 16;

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
    pub fn marked(marker: u8) -> Result<Option<Order>, Never> {
        Ok(match marker {
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
    fn size(self) -> Result<u32, Never> {
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
    ErrorReply,
    Signal,
}

impl Kind {
    fn code(self) -> Result<u8, Never> {
        Ok(match self {
            Kind::Call => 1,
            Kind::Answer => 2,
            Kind::ErrorReply => 3,
            Kind::Signal => 4,
        })
    }

    fn of(code: u8) -> Result<Option<Kind>, Never> {
        Ok(match code {
            1 => Some(Kind::Call),
            2 => Some(Kind::Answer),
            3 => Some(Kind::ErrorReply),
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
pub enum Value {
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
    List(Vec<Value>),
    Group(Vec<Value>),
    Variant { shape: String, value: Box<Value> },
}

impl Value {
    pub fn word(text: &str) -> Result<Value, Never> {
        Ok(Value::Word(text.to_string()))
    }

    pub fn held(shape: &str, value: Value) -> Result<Value, Never> {
        Ok(Value::Variant { shape: shape.to_string(), value: Box::new(value) })
    }

    fn unheld(&self) -> Result<&Value, Never> {
        let mut here = self;

        while let Value::Variant { value, .. } = here {
            here = value;
        }

        Ok(here)
    }

    pub fn text(&self) -> Result<Option<&str>, Never> {
        let Ok(here) = self.unheld();

        Ok(match here {
            Value::Word(text) | Value::Path(text) | Value::Shape(text) => Some(text),
            Value::Variant { .. }
            | Value::Byte(_)
            | Value::Truth(_)
            | Value::Signed16(_)
            | Value::Unsigned16(_)
            | Value::Signed32(_)
            | Value::Unsigned32(_)
            | Value::Signed64(_)
            | Value::Unsigned64(_)
            | Value::Fraction(_)
            | Value::List(_)
            | Value::Group(_) => None,
        })
    }

    pub fn listed(&self) -> Result<Option<&[Value]>, Never> {
        let Ok(here) = self.unheld();

        Ok(match here {
            Value::List(held) => Some(held),
            Value::Variant { .. }
            | Value::Byte(_)
            | Value::Truth(_)
            | Value::Signed16(_)
            | Value::Unsigned16(_)
            | Value::Signed32(_)
            | Value::Unsigned32(_)
            | Value::Signed64(_)
            | Value::Unsigned64(_)
            | Value::Fraction(_)
            | Value::Word(_)
            | Value::Path(_)
            | Value::Shape(_)
            | Value::Group(_) => None,
        })
    }

    pub fn pair(&self) -> Result<Option<(&Value, &Value)>, Never> {
        let held = match self {
            Value::Group(held) => held,
            Value::Byte(_)
            | Value::Truth(_)
            | Value::Signed16(_)
            | Value::Unsigned16(_)
            | Value::Signed32(_)
            | Value::Unsigned32(_)
            | Value::Signed64(_)
            | Value::Unsigned64(_)
            | Value::Fraction(_)
            | Value::Word(_)
            | Value::Path(_)
            | Value::Shape(_)
            | Value::List(_)
            | Value::Variant { .. } => return Ok(None),
        };

        Ok(match (held.first(), held.get(1)) {
            (Some(name), Some(value)) => Some((name, value)),
            (Some(_), None) | (None, Some(_)) | (None, None) => None,
        })
    }

    pub fn counted(&self) -> Result<Option<i64>, Never> {
        let Ok(here) = self.unheld();

        Ok(match here {
            Value::Byte(value) => Some(i64::from(*value)),
            Value::Signed16(value) => Some(i64::from(*value)),
            Value::Unsigned16(value) => Some(i64::from(*value)),
            Value::Signed32(value) => Some(i64::from(*value)),
            Value::Unsigned32(value) => Some(i64::from(*value)),
            Value::Signed64(value) => Some(*value),
            Value::Unsigned64(value) => {
                let Ok(value) = fitted::<u64, i64>(*value);

                Some(value)
            }
            Value::Variant { .. }
            | Value::Truth(_)
            | Value::Fraction(_)
            | Value::Word(_)
            | Value::Path(_)
            | Value::Shape(_)
            | Value::List(_)
            | Value::Group(_) => None,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Short,
    Shape(char),
    Order(u8),
    Version(u8),
    Kind(u8),
    Unterminated,
    NotUtf8,
    Mismatched(char),
}

impl fmt::Display for Error {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Short => write!(to, "the message stops in the middle of a value"),
            Error::Shape(head) => write!(to, "a signature this reads nothing for: {head}"),
            Error::Order(value) => write!(to, "a byte order that is neither l nor B: {value}"),
            Error::Version(value) => write!(to, "a protocol this does not speak: {value}"),
            Error::Kind(value) => write!(to, "a kind of message with no name here: {value}"),
            Error::Unterminated => write!(to, "a string with no nul after it"),
            Error::NotUtf8 => write!(to, "a string that is not utf-8"),
            Error::Mismatched(head) => write!(to, "a value that is not the {head} its signature promised"),
        }
    }
}

impl std::error::Error for Error {}

fn edge(head: char) -> Result<Edge, Error> {
    Ok(match head {
        'y' | 'g' | 'v' => Edge::One,
        'n' | 'q' => Edge::Two,
        'b' | 'i' | 'u' | 's' | 'o' | 'a' | 'h' => Edge::Four,
        'x' | 't' | 'd' | '(' | '{' => Edge::Eight,
        other => return Err(Error::Shape(other)),
    })
}

#[derive(Clone, Debug)]
struct Walk {
    rest: Vec<char>,
}

impl Walk {
    fn over(shape: &str) -> Result<Walk, Never> {
        let mut rest: Vec<char> = shape.chars().collect();
        rest.reverse();

        Ok(Walk { rest })
    }

    fn head(&mut self) -> Result<char, Error> {
        match self.rest.pop() {
            Some(head) => Ok(head),
            None => Err(Error::Short),
        }
    }
}

impl Iterator for Walk {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        self.rest.pop()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Opening {
    List,
    Group(char),
}

fn onward(shape: &mut Walk) -> Result<(), Error> {
    let mut open: Vec<Opening> = Vec::new();

    loop {
        let head = shape.head()?;

        match head {
            'a' => open.push(Opening::List),
            '(' => open.push(Opening::Group(')')),
            '{' => open.push(Opening::Group('}')),

            other => {
                match open.last() == Some(&Opening::Group(other)) {
                    true => {
                        let _ = open.pop();
                    }
                    false => {
                        let _ = edge(other)?;
                    }
                }

                while open.last() == Some(&Opening::List) {
                    let _ = open.pop();
                }

                match open.is_empty() {
                    true => return Ok(()),
                    false => {}
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
enum ReadFrame {
    Body,
    List { element: Walk, end: u32 },
    Group { close: char },
    Variant { shape: String },
}

struct Reads {
    frame: ReadFrame,
    walking: Walk,
    held: Vec<Value>,
}

enum ReadStep {
    Opened(Reads),
    Read(Value),
    Closed,
}

struct Reading<'a> {
    bytes: &'a [u8],
    at: u32,
    order: Order,
}

impl<'a> Reading<'a> {
    fn onto(&mut self, edge: Edge) -> Result<(), Error> {
        let Ok(wide) = edge.size();

        self.at = match self.at.checked_next_multiple_of(wide) {
            Some(at) => at,
            None => return Err(Error::Short),
        };

        Ok(())
    }

    fn taking(&mut self, many: u32) -> Result<&'a [u8], Error> {
        let to = match self.at.checked_add(many) {
            Some(to) => to,
            None => return Err(Error::Short),
        };

        let Ok(from) = index(self.at);
        let Ok(until) = index(to);

        let taken = match self.bytes.get(from..until) {
            Some(taken) => taken,
            None => return Err(Error::Short),
        };

        self.at = to;

        Ok(taken)
    }

    fn byte(&mut self) -> Result<u8, Error> {
        let taken = self.taking(1)?;

        match taken.first() {
            Some(byte) => Ok(*byte),
            None => Err(Error::Short),
        }
    }

    fn two(&mut self) -> Result<[u8; 2], Error> {
        self.onto(Edge::Two)?;
        let taken = self.taking(2)?;

        match <[u8; 2]>::try_from(taken) {
            Ok(taken) => Ok(taken),
            Err(_fault) => Err(Error::Short),
        }
    }

    fn four(&mut self) -> Result<[u8; 4], Error> {
        self.onto(Edge::Four)?;
        let taken = self.taking(4)?;

        match <[u8; 4]>::try_from(taken) {
            Ok(taken) => Ok(taken),
            Err(_fault) => Err(Error::Short),
        }
    }

    fn eight(&mut self) -> Result<[u8; 8], Error> {
        self.onto(Edge::Eight)?;
        let taken = self.taking(8)?;

        match <[u8; 8]>::try_from(taken) {
            Ok(taken) => Ok(taken),
            Err(_fault) => Err(Error::Short),
        }
    }

    fn unsigned32(&mut self) -> Result<u32, Error> {
        let taken = self.four()?;

        Ok(match self.order {
            Order::Little => u32::from_le_bytes(taken),
            Order::Big => u32::from_be_bytes(taken),
        })
    }

    fn word(&mut self) -> Result<String, Error> {
        let many = self.unsigned32()?;

        self.text(many)
    }

    fn shape(&mut self) -> Result<String, Error> {
        let many = self.byte()?;

        self.text(u32::from(many))
    }

    fn text(&mut self, many: u32) -> Result<String, Error> {
        let taken = self.taking(many)?;
        let text = match std::str::from_utf8(taken) {
            Ok(text) => text.to_string(),
            Err(_fault) => return Err(Error::NotUtf8),
        };

        let nul = self.byte()?;

        match nul == NUL {
            true => Ok(text),
            false => Err(Error::Unterminated),
        }
    }

    fn values(&mut self, shape: &str) -> Result<Vec<Value>, Error> {
        let Ok(walking) = Walk::over(shape);
        let mut open = vec![Reads { frame: ReadFrame::Body, walking, held: Vec::new() }];

        loop {
            let top = match open.last_mut() {
                Some(top) => top,
                None => return Err(Error::Short),
            };

            let step = self.step(top)?;

            match step {
                ReadStep::Opened(inner) => open.push(inner),
                ReadStep::Read(value) => top.held.push(value),
                ReadStep::Closed => {
                    let finished = closed(&mut open)?;

                    match finished {
                        Some(held) => return Ok(held),
                        None => {}
                    }
                }
            }
        }
    }

    fn step(&mut self, top: &mut Reads) -> Result<ReadStep, Error> {
        match &top.frame {
            ReadFrame::Body => match top.walking.next() {
                Some(head) => self.one(head, &mut top.walking),
                None => Ok(ReadStep::Closed),
            },
            ReadFrame::Group { close } => {
                let head = top.walking.head()?;

                match head == *close {
                    true => Ok(ReadStep::Closed),
                    false => self.one(head, &mut top.walking),
                }
            }
            ReadFrame::List { element, end } => match self.at.cmp(end) {
                std::cmp::Ordering::Less => {
                    top.walking = element.clone();
                    let head = top.walking.head()?;

                    self.one(head, &mut top.walking)
                }
                std::cmp::Ordering::Equal => Ok(ReadStep::Closed),
                std::cmp::Ordering::Greater => Err(Error::Short),
            },
            ReadFrame::Variant { .. } => match top.held.is_empty() {
                true => {
                    let head = top.walking.head()?;

                    self.one(head, &mut top.walking)
                }
                false => Ok(ReadStep::Closed),
            },
        }
    }

    fn one(&mut self, head: char, shape: &mut Walk) -> Result<ReadStep, Error> {
        match head {
            'y' => {
                let value = self.byte()?;

                Ok(ReadStep::Read(Value::Byte(value)))
            }
            'b' => {
                let value = self.unsigned32()?;

                Ok(ReadStep::Read(Value::Truth(match value {
                    0 => Truth::No,
                    _ => Truth::Yes,
                })))
            }
            'n' => {
                let taken = self.two()?;

                Ok(ReadStep::Read(Value::Signed16(match self.order {
                    Order::Little => i16::from_le_bytes(taken),
                    Order::Big => i16::from_be_bytes(taken),
                })))
            }
            'q' => {
                let taken = self.two()?;

                Ok(ReadStep::Read(Value::Unsigned16(match self.order {
                    Order::Little => u16::from_le_bytes(taken),
                    Order::Big => u16::from_be_bytes(taken),
                })))
            }
            'i' => {
                let taken = self.four()?;

                Ok(ReadStep::Read(Value::Signed32(match self.order {
                    Order::Little => i32::from_le_bytes(taken),
                    Order::Big => i32::from_be_bytes(taken),
                })))
            }
            'u' | 'h' => {
                let value = self.unsigned32()?;

                Ok(ReadStep::Read(Value::Unsigned32(value)))
            }
            'x' => {
                let taken = self.eight()?;

                Ok(ReadStep::Read(Value::Signed64(match self.order {
                    Order::Little => i64::from_le_bytes(taken),
                    Order::Big => i64::from_be_bytes(taken),
                })))
            }
            't' => {
                let taken = self.eight()?;

                Ok(ReadStep::Read(Value::Unsigned64(match self.order {
                    Order::Little => u64::from_le_bytes(taken),
                    Order::Big => u64::from_be_bytes(taken),
                })))
            }
            'd' => {
                let taken = self.eight()?;

                Ok(ReadStep::Read(Value::Fraction(match self.order {
                    Order::Little => f64::from_le_bytes(taken),
                    Order::Big => f64::from_be_bytes(taken),
                })))
            }
            's' => {
                let value = self.word()?;

                Ok(ReadStep::Read(Value::Word(value)))
            }
            'o' => {
                let value = self.word()?;

                Ok(ReadStep::Read(Value::Path(value)))
            }
            'g' => {
                let value = self.shape()?;

                Ok(ReadStep::Read(Value::Shape(value)))
            }
            'a' => self.list(shape),
            '(' => self.group(shape, ')'),
            '{' => self.group(shape, '}'),
            'v' => {
                let shape = self.shape()?;
                let Ok(walking) = Walk::over(&shape);

                Ok(ReadStep::Opened(Reads { frame: ReadFrame::Variant { shape }, walking, held: Vec::new() }))
            }
            other => Err(Error::Shape(other)),
        }
    }

    fn list(&mut self, shape: &mut Walk) -> Result<ReadStep, Error> {
        let many = self.unsigned32()?;
        let element = shape.clone();

        onward(shape)?;

        let head = element.clone().head()?;
        let edge = edge(head)?;

        self.onto(edge)?;

        let end = match self.at.checked_add(many) {
            Some(end) => end,
            None => return Err(Error::Short),
        };

        Ok(ReadStep::Opened(Reads { frame: ReadFrame::List { element, end }, walking: shape.clone(), held: Vec::new() }))
    }

    fn group(&mut self, shape: &mut Walk, close: char) -> Result<ReadStep, Error> {
        self.onto(Edge::Eight)?;

        Ok(ReadStep::Opened(Reads { frame: ReadFrame::Group { close }, walking: shape.clone(), held: Vec::new() }))
    }
}

fn closed(open: &mut Vec<Reads>) -> Result<Option<Vec<Value>>, Error> {
    let done = match open.pop() {
        Some(done) => done,
        None => return Err(Error::Short),
    };

    let mut held = done.held;

    let (value, walked) = match done.frame {
        ReadFrame::Body => return Ok(Some(held)),
        ReadFrame::List { .. } => (Value::List(held), None),
        ReadFrame::Group { .. } => (Value::Group(held), Some(done.walking)),
        ReadFrame::Variant { shape } => match held.pop() {
            Some(value) => (Value::Variant { shape, value: Box::new(value) }, None),
            None => return Err(Error::Short),
        },
    };

    let parent = match open.last_mut() {
        Some(parent) => parent,
        None => return Err(Error::Short),
    };

    match walked {
        Some(walked) => parent.walking = walked,
        None => {}
    }

    parent.held.push(value);

    Ok(None)
}

struct Writing {
    bytes: Vec<u8>,
}

impl Writing {
    fn new() -> Result<Writing, Never> {
        Ok(Writing { bytes: Vec::new() })
    }

    fn pad(&mut self, edge: Edge) -> Result<(), Error> {
        let Ok(wide) = edge.size();
        let Ok(written) = fitted::<_, u32>(self.bytes.len());

        let to = match written.checked_next_multiple_of(wide) {
            Some(to) => to,
            None => return Err(Error::Short),
        };

        let Ok(to) = index(to);

        self.bytes.resize(to, NUL);

        Ok(())
    }

    fn byte(&mut self, value: u8) -> Result<(), Never> {
        self.bytes.push(value);

        Ok(())
    }

    fn unsigned32(&mut self, value: u32) -> Result<(), Error> {
        self.pad(Edge::Four)?;
        self.bytes.extend_from_slice(&value.to_le_bytes());

        Ok(())
    }

    fn word(&mut self, value: &str) -> Result<(), Error> {
        let Ok(many) = fitted::<_, u32>(value.len());

        self.unsigned32(many)?;
        self.bytes.extend_from_slice(value.as_bytes());
        self.bytes.push(NUL);

        Ok(())
    }

    fn shape(&mut self, value: &str) -> Result<(), Never> {
        let Ok(many) = fitted::<_, u8>(value.len());

        self.bytes.push(many);
        self.bytes.extend_from_slice(value.as_bytes());
        self.bytes.push(NUL);

        Ok(())
    }

    fn values(&mut self, shape: &str, value: &[Value]) -> Result<(), Error> {
        let Ok(walking) = Walk::over(shape);
        let mut open = vec![Writes { frame: WriteFrame::Body, walking, items: value.iter() }];

        loop {
            let top = match open.last_mut() {
                Some(top) => top,
                None => return Err(Error::Short),
            };

            let step = self.step(top)?;

            match step {
                WriteStep::Opened(inner) => open.push(inner),
                WriteStep::Wrote => {}
                WriteStep::Closed => {
                    let finished = self.closed(&mut open)?;

                    match finished {
                        Finished::Whole => return Ok(()),
                        Finished::Part => {}
                    }
                }
            }
        }
    }

    fn step<'v>(&mut self, top: &mut Writes<'v>) -> Result<WriteStep<'v>, Error> {
        match &top.frame {
            WriteFrame::Body => match top.walking.next() {
                Some(head) => {
                    let value = next_item(&mut top.items)?;

                    self.one(head, &mut top.walking, value)
                }
                None => Ok(WriteStep::Closed),
            },
            WriteFrame::Group { close } => {
                let head = top.walking.head()?;

                match head == *close {
                    true => Ok(WriteStep::Closed),
                    false => {
                        let value = next_item(&mut top.items)?;

                        self.one(head, &mut top.walking, value)
                    }
                }
            }
            WriteFrame::List { element, .. } => match top.items.next() {
                Some(value) => {
                    top.walking = element.clone();
                    let head = top.walking.head()?;

                    self.one(head, &mut top.walking, value)
                }
                None => Ok(WriteStep::Closed),
            },
            WriteFrame::Variant => match top.items.next() {
                Some(value) => {
                    let head = top.walking.head()?;

                    self.one(head, &mut top.walking, value)
                }
                None => Ok(WriteStep::Closed),
            },
        }
    }

    fn closed(&mut self, open: &mut Vec<Writes<'_>>) -> Result<Finished, Error> {
        let done = match open.pop() {
            Some(done) => done,
            None => return Err(Error::Short),
        };

        match done.frame {
            WriteFrame::Body => return Ok(Finished::Whole),
            WriteFrame::List { length, .. } => self.counted(length)?,
            WriteFrame::Group { .. } => match open.last_mut() {
                Some(parent) => parent.walking = done.walking,
                None => return Err(Error::Short),
            },
            WriteFrame::Variant => {}
        }

        Ok(Finished::Part)
    }

    fn one<'v>(&mut self, head: char, shape: &mut Walk, value: &'v Value) -> Result<WriteStep<'v>, Error> {
        match (head, value) {
            ('y', Value::Byte(value)) => {
                let Ok(()) = self.byte(*value);

                Ok(WriteStep::Wrote)
            }
            ('b', Value::Truth(value)) => {
                self.unsigned32(match value {
                    Truth::Yes => 1,
                    Truth::No => 0,
                })?;

                Ok(WriteStep::Wrote)
            }
            ('n', Value::Signed16(value)) => self.laid(Edge::Two, &value.to_le_bytes()),
            ('q', Value::Unsigned16(value)) => self.laid(Edge::Two, &value.to_le_bytes()),
            ('i', Value::Signed32(value)) => self.laid(Edge::Four, &value.to_le_bytes()),
            ('u', Value::Unsigned32(value)) | ('h', Value::Unsigned32(value)) => self.laid(Edge::Four, &value.to_le_bytes()),
            ('x', Value::Signed64(value)) => self.laid(Edge::Eight, &value.to_le_bytes()),
            ('t', Value::Unsigned64(value)) => self.laid(Edge::Eight, &value.to_le_bytes()),
            ('d', Value::Fraction(value)) => self.laid(Edge::Eight, &value.to_le_bytes()),
            ('s', Value::Word(value)) | ('o', Value::Path(value)) => {
                self.word(value)?;

                Ok(WriteStep::Wrote)
            }
            ('g', Value::Shape(value)) => {
                let Ok(()) = self.shape(value);

                Ok(WriteStep::Wrote)
            }
            ('a', Value::List(held)) => self.list(shape, held),
            ('(', Value::Group(held)) => self.group(shape, held, ')'),
            ('{', Value::Group(held)) => self.group(shape, held, '}'),
            ('v', Value::Variant { shape, value }) => {
                let Ok(()) = self.shape(shape);
                let Ok(walking) = Walk::over(shape);
                let items = std::slice::from_ref(value.as_ref()).iter();

                Ok(WriteStep::Opened(Writes { frame: WriteFrame::Variant, walking, items }))
            }
            (head, _value) => Err(Error::Mismatched(head)),
        }
    }

    fn laid<'v>(&mut self, edge: Edge, bytes: &[u8]) -> Result<WriteStep<'v>, Error> {
        self.pad(edge)?;
        self.bytes.extend_from_slice(bytes);

        Ok(WriteStep::Wrote)
    }

    fn list<'v>(&mut self, shape: &mut Walk, held: &'v [Value]) -> Result<WriteStep<'v>, Error> {
        let element = shape.clone();

        onward(shape)?;

        let head = element.clone().head()?;
        let edge = edge(head)?;

        self.pad(Edge::Four)?;

        let Ok(length_at) = fitted::<_, u32>(self.bytes.len());

        self.bytes.extend_from_slice(&0u32.to_le_bytes());
        self.pad(edge)?;

        let Ok(from) = fitted::<_, u32>(self.bytes.len());
        let frame = WriteFrame::List { element, length: Length { at: length_at, from } };

        Ok(WriteStep::Opened(Writes { frame, walking: shape.clone(), items: held.iter() }))
    }

    fn counted(&mut self, length: Length) -> Result<(), Error> {
        let Length { at: length_at, from } = length;
        let Ok(written) = fitted::<_, u32>(self.bytes.len());
        let many = written.saturating_sub(from);

        let to = length_at.saturating_add(4);
        let Ok(length_at) = index(length_at);
        let Ok(to) = index(to);

        match self.bytes.get_mut(length_at..to) {
            Some(room) => room.copy_from_slice(&many.to_le_bytes()),
            None => return Err(Error::Short),
        }

        Ok(())
    }

    fn group<'v>(&mut self, shape: &mut Walk, held: &'v [Value], close: char) -> Result<WriteStep<'v>, Error> {
        self.pad(Edge::Eight)?;

        let frame = WriteFrame::Group { close };

        Ok(WriteStep::Opened(Writes { frame, walking: shape.clone(), items: held.iter() }))
    }
}

fn next_item<'v>(items: &mut std::slice::Iter<'v, Value>) -> Result<&'v Value, Error> {
    match items.next() {
        Some(value) => Ok(value),
        None => Err(Error::Short),
    }
}

#[derive(Clone, Debug)]
enum WriteFrame {
    Body,
    List { element: Walk, length: Length },
    Group { close: char },
    Variant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Length {
    at: u32,
    from: u32,
}

struct Writes<'v> {
    frame: WriteFrame,
    walking: Walk,
    items: std::slice::Iter<'v, Value>,
}

enum WriteStep<'v> {
    Opened(Writes<'v>),
    Wrote,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Finished {
    Whole,
    Part,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Whom<'a> {
    pub to: &'a str,
    pub at: &'a str,
    pub on: &'a str,
    pub calling: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Signal<'a> {
    pub at: &'a str,
    pub on: &'a str,
    pub name: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    Failed,
    UnknownMethod,
    UnknownInterface,
    UnknownProperty,
    PropertyReadOnly,
    InvalidArgs,
}

impl ValidationError {
    pub fn named(self) -> Result<&'static str, Never> {
        Ok(match self {
            ValidationError::Failed => "org.freedesktop.DBus.Error.Failed",
            ValidationError::UnknownMethod => "org.freedesktop.DBus.Error.UnknownMethod",
            ValidationError::UnknownInterface => "org.freedesktop.DBus.Error.UnknownInterface",
            ValidationError::UnknownProperty => "org.freedesktop.DBus.Error.UnknownProperty",
            ValidationError::PropertyReadOnly => "org.freedesktop.DBus.Error.PropertyReadOnly",
            ValidationError::InvalidArgs => "org.freedesktop.DBus.Error.InvalidArgs",
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
    pub values: Vec<Value>,
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

    pub fn signal(signal: &Signal<'_>) -> Result<Message, Never> {
        Ok(Message {
            kind: Kind::Signal,
            path: Some(signal.at.to_string()),
            interface: Some(signal.on.to_string()),
            member: Some(signal.name.to_string()),
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

    pub fn complaining(&self, complaint: ValidationError, why: &str) -> Result<Message, Never> {
        let Ok(value) = Value::word(why);
        let Ok(named) = complaint.named();

        Ok(Message {
            kind: Kind::ErrorReply,
            reply_to: Some(self.serial),
            destination: self.sender.clone(),
            fault: Some(named.to_string()),
            shape: "s".to_string(),
            values: vec![value],
            ..Message::default()
        })
    }

    pub fn carrying(mut self, shape: &str, values: Vec<Value>) -> Result<Message, Never> {
        self.shape = shape.to_string();
        self.values = values;

        Ok(self)
    }

    pub fn bytes(&self, serial: u32) -> Result<Vec<u8>, Error> {
        let Ok(mut body) = Writing::new();

        body.values(&self.shape, &self.values)?;

        let Ok(length) = fitted::<_, u32>(body.bytes.len());
        let Ok(kind) = self.kind.code();

        let Ok(mut whole) = Writing::new();
        let Ok(()) = whole.byte(LITTLE);
        let Ok(()) = whole.byte(kind);
        let Ok(()) = whole.byte(NUL);
        let Ok(()) = whole.byte(VERSION);

        whole.unsigned32(length)?;
        whole.unsigned32(serial)?;

        let mut fields: Vec<Value> = Vec::new();

        for (field, value) in [
            (Field::Path, &self.path),
            (Field::Interface, &self.interface),
            (Field::Member, &self.member),
            (Field::FaultName, &self.fault),
            (Field::Destination, &self.destination),
            (Field::Sender, &self.sender),
        ] {
            let value = match value {
                Some(value) => value,
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
                Field::Path => Value::Path(value.to_string()),
                Field::Interface
                | Field::Member
                | Field::FaultName
                | Field::Destination
                | Field::Sender
                | Field::ReplyTo
                | Field::Shape => Value::Word(value.to_string()),
            };

            let Ok(held) = Value::held(shape, carried);

            fields.push(Value::Group(vec![Value::Byte(code), held]));
        }

        match self.reply_to {
            Some(serial) => {
                let Ok(code) = Field::ReplyTo.code();
                let Ok(held) = Value::held("u", Value::Unsigned32(serial));

                fields.push(Value::Group(vec![Value::Byte(code), held]));
            }
            None => {}
        }

        match self.shape.is_empty() {
            true => {}
            false => {
                let Ok(code) = Field::Shape.code();
                let Ok(held) = Value::held("g", Value::Shape(self.shape.clone()));

                fields.push(Value::Group(vec![Value::Byte(code), held]));
            }
        }

        whole.values("a(yv)", &[Value::List(fields)])?;
        whole.pad(Edge::Eight)?;
        whole.bytes.extend_from_slice(&body.bytes);

        Ok(whole.bytes)
    }
}

pub fn length(bytes: &[u8]) -> Result<u32, Error> {
    let order = order(bytes)?;
    let mut reading = Reading { bytes, at: 4, order };
    let body = reading.unsigned32()?;
    let _serial = reading.unsigned32()?;
    let fields = reading.unsigned32()?;

    let to = match HEAD.checked_add(fields) {
        Some(to) => to,
        None => return Err(Error::Short),
    };

    let to = match to.checked_next_multiple_of(8) {
        Some(to) => to,
        None => return Err(Error::Short),
    };

    match to.checked_add(body) {
        Some(whole) => Ok(whole),
        None => Err(Error::Short),
    }
}

fn order(bytes: &[u8]) -> Result<Order, Error> {
    let mark = match bytes.first() {
        Some(mark) => *mark,
        None => return Err(Error::Short),
    };

    let Ok(order) = Order::marked(mark);

    match order {
        Some(order) => Ok(order),
        None => Err(Error::Order(mark)),
    }
}

pub fn read(bytes: &[u8]) -> Result<Message, Error> {
    let order = order(bytes)?;

    let code = match bytes.get(1) {
        Some(code) => *code,
        None => return Err(Error::Short),
    };

    let version = match bytes.get(3) {
        Some(version) => *version,
        None => return Err(Error::Short),
    };

    match version == VERSION {
        true => {}
        false => return Err(Error::Version(version)),
    }

    let Ok(kind) = Kind::of(code);

    let kind = match kind {
        Some(kind) => kind,
        None => return Err(Error::Kind(code)),
    };

    let mut reading = Reading { bytes, at: 4, order };
    let _body = reading.unsigned32()?;
    let serial = reading.unsigned32()?;
    let fields = reading.values("a(yv)")?;

    let mut message = Message { kind, serial, ..Message::default() };

    for held in fields {
        let held = match held {
            Value::List(held) => held,
            Value::Byte(_)
            | Value::Truth(_)
            | Value::Signed16(_)
            | Value::Unsigned16(_)
            | Value::Signed32(_)
            | Value::Unsigned32(_)
            | Value::Signed64(_)
            | Value::Unsigned64(_)
            | Value::Fraction(_)
            | Value::Word(_)
            | Value::Path(_)
            | Value::Shape(_)
            | Value::Group(_)
            | Value::Variant { .. } => return Err(Error::Mismatched('a')),
        };

        for one in held {
            let Ok(()) = kept(&mut message, &one);
        }
    }

    reading.onto(Edge::Eight)?;

    let shape = message.shape.clone();

    let values = reading.values(&shape)?;

    message.values = values;

    Ok(message)
}

fn kept(message: &mut Message, one: &Value) -> Result<(), Never> {
    let held = match one {
        Value::Group(held) => held,
        Value::Byte(_)
        | Value::Truth(_)
        | Value::Signed16(_)
        | Value::Unsigned16(_)
        | Value::Signed32(_)
        | Value::Unsigned32(_)
        | Value::Signed64(_)
        | Value::Unsigned64(_)
        | Value::Fraction(_)
        | Value::Word(_)
        | Value::Path(_)
        | Value::Shape(_)
        | Value::List(_)
        | Value::Variant { .. } => return Ok(()),
    };

    let code = match held.first() {
        Some(Value::Byte(code)) => *code,
        Some(_) | None => return Ok(()),
    };

    let value = match held.get(1) {
        Some(value) => value,
        None => return Ok(()),
    };

    let Ok(field) = Field::of(code);

    let field = match field {
        Some(field) => field,
        None => return Ok(()),
    };

    let Ok(text) = value.text();
    let word = text.map(str::to_string);

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
            let Ok(counted) = value.counted();

            message.reply_to = match counted {
                Some(counted) => match u32::try_from(counted) {
                    Ok(serial) => Some(serial),
                    Err(_too_large) => None,
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

        let hints = Value::List(vec![Value::Group(vec![
            Value::Word("urgency".to_string()),
            Value::Variant { shape: "y".to_string(), value: Box::new(Value::Byte(2)) },
        ])]);

        let Ok(call) = call.carrying(
            "susssasa{sv}i",
            vec![
                Value::Word("Console".to_string()),
                Value::Unsigned32(0),
                Value::Word(String::new()),
                Value::Word("Notifications fell over".to_string()),
                Value::Word("console-notify.service stopped".to_string()),
                Value::List(Vec::new()),
                hints,
                Value::Signed32(0),
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
        assert_eq!(heard.values, call.values);
    }

    #[test]
    fn the_length_in_the_head_is_the_length_of_the_whole_message() {
        let signal = Signal { at: "/a", on: "b.c", name: "D" };

        for message in [notifying(), Message::signal(&signal).unwrap()] {
            let bytes = message.bytes(3).unwrap();

            assert_eq!(length(&bytes), Ok(u32::try_from(bytes.len()).unwrap()));
        }
    }

    #[test]
    fn a_body_starts_at_a_multiple_of_eight() {
        let bytes = notifying().bytes(1).unwrap();
        let fields = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let body = HEAD + fields;
        let whole = u32::try_from(bytes.len()).unwrap();

        assert_eq!(length(&bytes).unwrap(), whole);
        assert!(body <= whole);
        assert_eq!(whole - body.next_multiple_of(8), {
            let mut counting = Writing::new().unwrap();
            counting.values("susssasa{sv}i", &notifying().values).unwrap();
            u32::try_from(counting.bytes.len()).unwrap()
        });
    }

    #[test]
    fn a_string_carries_its_length_and_its_nul() {
        let mut writing = Writing::new().unwrap();

        writing.values("s", &[Value::Word("ok".to_string())]).unwrap();

        assert_eq!(writing.bytes, vec![2, 0, 0, 0, b'o', b'k', 0]);
    }

    #[test]
    fn a_number_after_a_byte_is_padded_out_to_its_own_width() {
        let mut writing = Writing::new().unwrap();

        writing.values("yu", &[Value::Byte(1), Value::Unsigned32(2)]).unwrap();

        assert_eq!(writing.bytes, vec![1, 0, 0, 0, 2, 0, 0, 0]);
    }

    #[test]
    fn a_structure_after_a_byte_starts_eight_along() {
        let mut writing = Writing::new().unwrap();
        let group = Value::Group(vec![Value::Byte(9)]);

        writing.values("y(y)", &[Value::Byte(1), group]).unwrap();

        assert_eq!(writing.bytes, vec![1, 0, 0, 0, 0, 0, 0, 0, 9]);
    }

    #[test]
    fn an_empty_list_is_a_length_of_nothing() {
        let mut writing = Writing::new().unwrap();

        writing.values("as", &[Value::List(Vec::new())]).unwrap();

        assert_eq!(writing.bytes, vec![0, 0, 0, 0]);
    }

    #[test]
    fn a_list_says_how_long_its_contents_are_and_not_how_many_there_are() {
        let mut writing = Writing::new().unwrap();
        let held = Value::List(vec![Value::Unsigned32(1), Value::Unsigned32(2)]);

        writing.values("au", &[held]).unwrap();

        assert_eq!(writing.bytes, vec![8, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0]);
    }

    #[test]
    fn a_list_of_structures_is_padded_before_the_first_one_is_written() {
        let mut writing = Writing::new().unwrap();
        let held = Value::List(vec![Value::Group(vec![Value::Byte(7)])]);

        writing.values("a(y)", &[held]).unwrap();

        assert_eq!(writing.bytes, vec![1, 0, 0, 0, 0, 0, 0, 0, 7]);
    }

    #[test]
    fn a_variant_carries_the_shape_of_what_is_in_it() {
        let mut writing = Writing::new().unwrap();
        let held = Value::Variant { shape: "u".to_string(), value: Box::new(Value::Unsigned32(5)) };

        writing.values("v", std::slice::from_ref(&held)).unwrap();

        assert_eq!(writing.bytes, vec![1, b'u', 0, 0, 5, 0, 0, 0]);

        let mut reading = Reading { bytes: &writing.bytes, at: 0, order: Order::Little };

        assert_eq!(reading.values("v").unwrap(), vec![held]);
    }

    #[test]
    fn a_hint_this_knows_nothing_about_is_walked_past_rather_than_stopping_the_message() {
        let mut writing = Writing::new().unwrap();
        let odd = Value::Variant {
            shape: "(iiii)".to_string(),
            value: Box::new(Value::Group(vec![
                Value::Signed32(1),
                Value::Signed32(2),
                Value::Signed32(3),
                Value::Signed32(4),
            ])),
        };
        let hints = Value::List(vec![
            Value::Group(vec![Value::Word("color".to_string()), odd]),
            Value::Group(vec![
                Value::Word("value".to_string()),
                Value::Variant { shape: "i".to_string(), value: Box::new(Value::Signed32(40)) },
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

        assert_eq!(length(&big), Ok(u32::try_from(big.len()).unwrap()));
    }

    #[test]
    fn a_signature_with_nothing_written_for_it_says_which_letter() {
        let mut reading = Reading { bytes: &[0, 0, 0, 0], at: 0, order: Order::Little };

        assert_eq!(reading.values("Z"), Err(Error::Shape('Z')));
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
        let Ok(answer) = answer.carrying("u", vec![Value::Unsigned32(4)]);
        let bytes = answer.bytes(2).unwrap();
        let heard = read(&bytes).unwrap();

        assert_eq!(heard.kind, Kind::Answer);
        assert_eq!(heard.reply_to, Some(call.serial));
        assert_eq!(heard.values, vec![Value::Unsigned32(4)]);
    }

    #[test]
    fn a_complaint_carries_a_name_and_a_sentence() {
        let Ok(fault) = notifying().complaining(ValidationError::Failed, "no");
        let bytes = fault.bytes(2).unwrap();
        let heard = read(&bytes).unwrap();

        assert_eq!(heard.kind, Kind::ErrorReply);
        assert_eq!(heard.fault.as_deref(), Some("org.freedesktop.DBus.Error.Failed"));
        assert_eq!(heard.values, vec![Value::Word("no".to_string())]);
    }
}
