//! What the browser's window and the engine drawing its pages say to each other.
//!
//! The window is this desktop's: the tabs, the address line, the pad. The
//! engine is somebody else's -- WebKit, Servo, Chromium -- running as a process
//! of its own and drawing into a buffer the window shows. Ladybird draws the
//! same line between its browser and its WebContent process, and it is the
//! line that lets an engine be swapped under an open page: the window stops one
//! process, starts another on the same address, and nothing it holds has to
//! know which engine it was speaking to.
//! The tab owns the settings in [`Page`]. A replacement engine opens the
//! current address at the current viewport size with fresh document state.
//!
//! Lines, for the reason `console-events` gives: both ends are built from the
//! same commit, and a person can hold `socat` against the socket and read what
//! is going past. A frame's pixels do not travel here. The line says a frame
//! is ready and how big it is, and the buffer goes beside it.
//!
//! Nothing is escaped, because the web already forbids what would need it. A
//! URL parser strips every tab and newline out of an address, and a document's
//! title has its whitespace collapsed by the HTML standard before anybody reads
//! it. Encoding does the same and loses nothing an engine could have said.
//! Text input sends printable text. Control keys travel as named keys.
//! Reader responses carry bounded plain text and may contain paragraph breaks.
//! [`FrameLayout`] describes packed RGBA8 memory accompanying a frame event.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::str::FromStr;

use console_core_geometry::{Point, Size};
use console_core_never::Never;

#[cfg(kani)]
#[path = "../proofs/frames.rs"]
mod proofs;

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
  Open(String),
  Resize(Size<u32>),
  Click(Point<f64>),
  Scroll(Point<f64>),
  Text(String),
  Key(String),
  Back,
  Forward,
  Reload,
  Stop,
  Read(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
  Address(String),
  Title(String),
  Started,
  Finished,
  Frame(Size<u32>),
  Reader(u64, String),
}

/// The script returns plain text from the current document. Its result must
/// only be displayed as text in browser chrome.
pub const READER_SCRIPT: &str = include_str!("reader.js");
const MAX_READER_BYTES: usize = 48 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameLayout {
  pub size: Size<u32>,
  pub stride: u64,
  pub bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameLayoutError {
  Empty,
  Overflow,
  WrongBufferLength,
}

impl FrameLayout {
  pub fn rgba(size: Size<u32>) -> Result<Self, FrameLayoutError> {
    match (size.width, size.height) {
      (0, _) | (_, 0) => return Err(FrameLayoutError::Empty),
      (_, _) => {}
    }

    let stride = ::core::primitive::u64::wrapping_mul(u64::from(size.width), 4);

    let pixels =
      ::core::primitive::u64::wrapping_mul(u64::from(size.width), u64::from(size.height));
    let bytes = pixels.checked_mul(4).ok_or(FrameLayoutError::Overflow)?;

    Ok(Self {
      size,
      stride,
      bytes,
    })
  }

  pub fn pixels(self, bytes: &[u8]) -> Result<&[[u8; 4]], FrameLayoutError> {
    (u64::try_from(bytes.len()) == Ok(self.bytes))
      .then_some(bytes.as_chunks().0)
      .ok_or(FrameLayoutError::WrongBufferLength)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
  pub address: String,
  pub viewport: Size<u32>,
}

impl Page {
  pub fn opening_requests(&self) -> Result<[Request; 2], Never> {
    Ok([
      Request::Resize(self.viewport),
      Request::Open(self.address.clone()),
    ])
  }
}

pub fn encoded_request(request: &Request) -> Result<String, Never> {
  Ok(match request {
    Request::Open(address) => {
      let Ok(cleaned) = address_line(address);

      format!("open {cleaned}")
    }
    Request::Resize(size) => format!("resize {} {}", size.width, size.height),
    Request::Click(at) => format!("press {} {}", at.x, at.y),
    Request::Scroll(by) => format!("scroll {} {}", by.x, by.y),
    Request::Text(text) => {
      let Ok(printable) = address_line(text);
      format!("text {printable}")
    }
    Request::Key(key) => {
      let Ok(name) = address_line(key);
      format!("key {name}")
    }
    Request::Back => "back".to_string(),
    Request::Forward => "forward".to_string(),
    Request::Reload => "reload".to_string(),
    Request::Stop => "stop".to_string(),
    Request::Read(id) => format!("read {id}"),
  })
}

pub fn decoded_request(line: &str) -> Result<Option<Request>, Never> {
  let (verb, rest) = match line.split_once(' ') {
    Some((verb, rest)) => (verb, rest),
    None => (line, ""),
  };

  Ok(match (verb, rest) {
    ("open", "") => None,
    ("open", address) => Some(Request::Open(address.to_string())),
    ("resize", size) => {
      let Ok(read) = size_of(size);

      read.map(Request::Resize)
    }
    ("press", at) => {
      let Ok(read) = point_of(at);

      read.map(Request::Click)
    }
    ("scroll", by) => {
      let Ok(read) = point_of(by);

      read.map(Request::Scroll)
    }
    ("text" | "key", "") => None,
    ("text", text) => Some(Request::Text(text.to_string())),
    ("key", key) => Some(Request::Key(key.to_string())),
    ("back", "") => Some(Request::Back),
    ("forward", "") => Some(Request::Forward),
    ("reload", "") => Some(Request::Reload),
    ("stop", "") => Some(Request::Stop),
    ("read", id) => id.parse().ok().map(Request::Read),
    _ => None,
  })
}

pub fn encoded_event(event: &Event) -> Result<String, Never> {
  Ok(match event {
    Event::Address(address) => {
      let Ok(cleaned) = address_line(address);

      format!("address {cleaned}")
    }
    Event::Title(title) => {
      let collapsed = title.split_whitespace().collect::<Vec<&str>>().join(" ");

      format!("title {collapsed}")
    }
    Event::Started => "started".to_string(),
    Event::Finished => "finished".to_string(),
    Event::Frame(size) => format!("frame {} {}", size.width, size.height),
    Event::Reader(id, text) => {
      let bounded = text.floor_char_boundary(MAX_READER_BYTES);
      let text = text.get(..bounded).unwrap_or(text);
      let plain: String = text
        .chars()
        .filter(|character| *character == '\n' || !character.is_control())
        .collect();
      format!("reader {id} {plain}")
    }
  })
}

pub fn decoded_event(line: &str) -> Result<Option<Event>, Never> {
  let (verb, rest) = match line.split_once(' ') {
    Some((verb, rest)) => (verb, rest),
    None => (line, ""),
  };

  Ok(match (verb, rest) {
    ("address", "") => None,
    ("address", address) => Some(Event::Address(address.to_string())),
    ("title", title) => Some(Event::Title(title.to_string())),
    ("started", "") => Some(Event::Started),
    ("finished", "") => Some(Event::Finished),
    ("frame", size) => {
      let Ok(read) = size_of(size);

      read.map(Event::Frame)
    }
    ("reader", payload) => payload
      .split_once(' ')
      .filter(|(_, text)| text.len() <= MAX_READER_BYTES)
      .and_then(|(id, text)| Some(Event::Reader(id.parse().ok()?, text.to_string()))),
    _ => None,
  })
}

fn address_line(address: &str) -> Result<String, Never> {
  Ok(
    address
      .chars()
      .filter(|letter| !matches!(letter, '\t' | '\n' | '\r'))
      .collect(),
  )
}

fn pair_of<T: FromStr>(text: &str) -> Result<Option<(T, T)>, Never> {
  Ok(match text.split_once(' ') {
    Some((first, second)) => match (first.parse::<T>(), second.parse::<T>()) {
      (Ok(first_2), Ok(second_2)) => Some((first_2, second_2)),
      (Err(_not_a_number), _) | (_, Err(_not_a_number)) => None,
    },
    None => None,
  })
}

fn size_of(text: &str) -> Result<Option<Size<u32>>, Never> {
  let Ok(pair) = pair_of::<u32>(text);

  Ok(pair.map(|(width, height)| Size { width, height }))
}

fn point_of(text: &str) -> Result<Option<Point<f64>>, Never> {
  let Ok(pair) = pair_of::<f64>(text);

  Ok(pair.map(|(x, y)| Point { x, y }))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn a_frame_must_hold_exactly_its_packed_rgba_pixels() -> Result<(), FrameLayoutError> {
    let size = Size {
      width: 2,
      height: 1,
    };

    let layout = FrameLayout::rgba(size)?;

    assert_eq!(layout.stride, 8);
    assert_eq!(layout.pixels(&[0; 8]), Ok([[0; 4]; 2].as_slice()));
    assert_eq!(
      layout.pixels(&[0; 7]),
      Err(FrameLayoutError::WrongBufferLength)
    );
    assert_eq!(
      layout.pixels(&[0; 9]),
      Err(FrameLayoutError::WrongBufferLength)
    );
    assert_eq!(
      FrameLayout::rgba(Size {
        width: 0,
        height: 1
      }),
      Err(FrameLayoutError::Empty)
    );
    assert_eq!(
      FrameLayout::rgba(Size {
        width: u32::MAX,
        height: u32::MAX
      }),
      Err(FrameLayoutError::Overflow)
    );

    Ok(())
  }

  fn request_round(request: &Request) -> Result<Option<Request>, Never> {
    let Ok(spelled) = encoded_request(request);

    decoded_request(&spelled)
  }

  fn event_round(event: &Event) -> Result<Option<Event>, Never> {
    let Ok(spelled) = encoded_event(event);

    decoded_event(&spelled)
  }

  #[test]
  fn every_request_survives_the_journey() {
    for request in [
      Request::Open("https://example.org/a page?q=1".to_string()),
      Request::Resize(Size {
        width: 1280,
        height: 800,
      }),
      Request::Click(Point { x: 12.5, y: 300.0 }),
      Request::Scroll(Point { x: 0.0, y: -120.25 }),
      Request::Text("é ".to_string()),
      Request::Key("Backspace".to_string()),
      Request::Back,
      Request::Forward,
      Request::Reload,
      Request::Stop,
      Request::Read(3),
    ] {
      assert_eq!(request_round(&request), Ok(Some(request)));
    }
  }

  #[test]
  fn every_event_survives_the_journey() {
    for event in [
      Event::Address("https://example.org/".to_string()),
      Event::Title("A page, with a title".to_string()),
      Event::Title(String::new()),
      Event::Started,
      Event::Finished,
      Event::Reader(3, "Heading\n\nCafé, résumé".to_string()),
      Event::Reader(4, String::new()),
      Event::Frame(Size {
        width: 1920,
        height: 1200,
      }),
    ] {
      assert_eq!(event_round(&event), Ok(Some(event)));
    }
  }

  #[test]
  fn reader_text_is_bounded_at_a_unicode_boundary() {
    let mut text = "a".repeat(MAX_READER_BYTES - 1);
    text.push('€');
    assert_eq!(
      event_round(&Event::Reader(3, text)),
      Ok(Some(Event::Reader(3, "a".repeat(MAX_READER_BYTES - 1))))
    );
  }

  #[test]
  fn a_newline_in_an_address_goes_the_way_a_url_parser_sends_it() {
    let open = Request::Open("https://exam\nple.org/\t".to_string());

    assert_eq!(
      request_round(&open),
      Ok(Some(Request::Open("https://example.org/".to_string())))
    );
  }

  #[test]
  fn a_title_is_collapsed_the_way_a_document_collapses_it() {
    let title = Event::Title("  Two\nlines\t here ".to_string());

    assert_eq!(
      event_round(&title),
      Ok(Some(Event::Title("Two lines here".to_string())))
    );
  }

  #[test]
  fn nonsense_from_an_engine_is_refused_rather_than_guessed_at() {
    let Ok(nothing) = decoded_event("");
    let Ok(an_unknown_verb) = decoded_event("explode now");
    let Ok(a_frame_with_one_side) = decoded_event("frame 1920");
    let Ok(a_frame_of_words) = decoded_event("frame wide tall");
    let Ok(finished_with_more) = decoded_event("finished early");

    assert_eq!(nothing, None);
    assert_eq!(an_unknown_verb, None);
    assert_eq!(a_frame_with_one_side, None);
    assert_eq!(a_frame_of_words, None);
    assert_eq!(finished_with_more, None);
  }

  #[test]
  fn a_request_with_nothing_to_open_is_not_a_request() {
    let Ok(open_nothing) = decoded_request("open");
    let Ok(press_nowhere) = decoded_request("press 3");

    assert_eq!(open_nothing, None);
    assert_eq!(press_nowhere, None);
  }
}
