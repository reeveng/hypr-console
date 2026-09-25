//! What the browser's window and the engine drawing its pages say to each other.
//!
//! The window is this desktop's: the tabs, the address line, the pad. The
//! engine is somebody else's -- WebKit, Servo, Chromium -- running as a process
//! of its own and drawing into a buffer the window shows. Ladybird draws the
//! same line between its browser and its WebContent process, and it is the
//! line that lets an engine be swapped under an open page: the window stops one
//! process, starts another on the same address, and nothing it holds has to
//! know which engine it was speaking to.
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

use console_core_geometry::{Point, Size};
use console_core_never::Never;

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Open(String),
    Resize(Size<u32>),
    Click(Point<f64>),
    Scroll(Point<f64>),
    Back,
    Forward,
    Reload,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Address(String),
    Title(String),
    Started,
    Finished,
    Frame(Size<u32>),
}

pub fn encoded_request(request: &Request) -> Result<String, Never> {
    Ok(match request {
        Request::Open(address) => {
            let Ok(address) = address_line(address);

            format!("open {address}")
        }
        Request::Resize(size) => format!("resize {} {}", size.width, size.height),
        Request::Click(at) => format!("press {} {}", at.x, at.y),
        Request::Scroll(by) => format!("scroll {} {}", by.x, by.y),
        Request::Back => "back".to_string(),
        Request::Forward => "forward".to_string(),
        Request::Reload => "reload".to_string(),
        Request::Stop => "stop".to_string(),
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
        ("back", "") => Some(Request::Back),
        ("forward", "") => Some(Request::Forward),
        ("reload", "") => Some(Request::Reload),
        ("stop", "") => Some(Request::Stop),
        _ => None,
    })
}

pub fn encoded_event(event: &Event) -> Result<String, Never> {
    Ok(match event {
        Event::Address(address) => {
            let Ok(address) = address_line(address);

            format!("address {address}")
        }
        Event::Title(title) => {
            let collapsed = title.split_whitespace().collect::<Vec<&str>>().join(" ");

            format!("title {collapsed}")
        }
        Event::Started => "started".to_string(),
        Event::Finished => "finished".to_string(),
        Event::Frame(size) => format!("frame {} {}", size.width, size.height),
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
        _ => None,
    })
}

fn address_line(address: &str) -> Result<String, Never> {
    Ok(address.chars().filter(|letter| !matches!(letter, '\t' | '\n' | '\r')).collect())
}

fn pair_of<T: std::str::FromStr>(text: &str) -> Result<Option<(T, T)>, Never> {
    Ok(match text.split_once(' ') {
        Some((first, second)) => match (first.parse::<T>(), second.parse::<T>()) {
            (Ok(first), Ok(second)) => Some((first, second)),
            (Err(_), _) | (_, Err(_)) => None,
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

    fn request_round(request: &Request) -> Option<Request> {
        let Ok(spelled) = encoded_request(request);
        let Ok(read) = decoded_request(&spelled);

        read
    }

    fn event_round(event: &Event) -> Option<Event> {
        let Ok(spelled) = encoded_event(event);
        let Ok(read) = decoded_event(&spelled);

        read
    }

    #[test]
    fn every_request_survives_the_journey() {
        for request in [
            Request::Open("https://example.org/a page?q=1".to_string()),
            Request::Resize(Size { width: 1280, height: 800 }),
            Request::Click(Point { x: 12.5, y: 300.0 }),
            Request::Scroll(Point { x: 0.0, y: -120.25 }),
            Request::Back,
            Request::Forward,
            Request::Reload,
            Request::Stop,
        ] {
            assert_eq!(request_round(&request), Some(request));
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
            Event::Frame(Size { width: 1920, height: 1200 }),
        ] {
            assert_eq!(event_round(&event), Some(event));
        }
    }

    #[test]
    fn a_newline_in_an_address_goes_the_way_a_url_parser_sends_it() {
        let open = Request::Open("https://exam\nple.org/\t".to_string());

        assert_eq!(request_round(&open), Some(Request::Open("https://example.org/".to_string())));
    }

    #[test]
    fn a_title_is_collapsed_the_way_a_document_collapses_it() {
        let title = Event::Title("  Two\nlines\t here ".to_string());

        assert_eq!(event_round(&title), Some(Event::Title("Two lines here".to_string())));
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
