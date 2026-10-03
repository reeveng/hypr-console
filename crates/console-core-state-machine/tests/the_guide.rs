//! The guide overlay, pressed: closed, or open with one row highlighted.
//!
//! It is the smallest machine that has everything -- a state with a part
//! inside it, requests that mean nothing in some states, effects, and a layout
//! that is saved -- so it is where the transcript, the snapshot and the
//! migration are each made to go red.

use std::path::PathBuf;

use console_core_state_machine::{
    Decoder, Encoder, Machine, Never, ParseError, Queue, Serializable, SerializableMachine, Version, restore, run, snapshot,
};

type Failure = Box<dyn std::error::Error>;

const CLOSED: u8 = 0;
const OPEN: u8 = 1;

const FIRST: u8 = 0;
const SECOND: u8 = 1;
const THIRD: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    First,
    Second,
    Third,
}

impl Row {
    fn next(self) -> Result<Row, Never> {
        Ok(match self {
            Row::First => Row::Second,
            Row::Second => Row::Third,
            Row::Third => Row::First,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Closed,
    Open(Row),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonPress {
    GuideButton,
    Down,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    ShowGuide,
    HideGuide,
    Highlight(Row),
}

impl Serializable for Row {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), Never> {
        encoder.byte(match self {
            Row::First => FIRST,
            Row::Second => SECOND,
            Row::Third => THIRD,
        })
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError> {
        let tag = decoder.byte()?;

        match tag {
            FIRST => Ok(Row::First),
            SECOND => Ok(Row::Second),
            THIRD => Ok(Row::Third),
            unknown => Err(ParseError::UnknownTag(unknown)),
        }
    }
}

impl Serializable for Mode {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), Never> {
        match self {
            Mode::Closed => encoder.byte(CLOSED),
            Mode::Open(row) => {
                let Ok(()) = encoder.byte(OPEN);

                row.encode(encoder)
            }
        }
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError> {
        let tag = decoder.byte()?;

        match tag {
            CLOSED => Ok(Mode::Closed),
            OPEN => Row::decode(decoder).map(Mode::Open),
            unknown => Err(ParseError::UnknownTag(unknown)),
        }
    }
}

struct ButtonGuide;

impl Machine for ButtonGuide {
    type Input = ();
    type State = Mode;
    type Request = ButtonPress;
    type Effect = Effect;

    fn initialize(_input: &(), previous: Option<Mode>, effects: &mut Queue<Effect>) -> Result<Mode, Never> {
        match previous {
            None | Some(Mode::Closed) => Ok(Mode::Closed),
            Some(Mode::Open(row)) => {
                let Ok(()) = effects.offer_all([Effect::ShowGuide, Effect::Highlight(row)]);

                Ok(Mode::Open(row))
            }
        }
    }

    fn handle(state: Mode, request: ButtonPress, effects: &mut Queue<Effect>) -> Result<Mode, Never> {
        match (state, request) {
            (Mode::Closed, ButtonPress::GuideButton) => {
                let Ok(()) = effects.offer(Effect::ShowGuide);

                Ok(Mode::Open(Row::First))
            }
            (Mode::Open(_), ButtonPress::GuideButton | ButtonPress::Back) => {
                let Ok(()) = effects.offer(Effect::HideGuide);

                Ok(Mode::Closed)
            }
            (Mode::Open(row), ButtonPress::Down) => {
                let Ok(next) = row.next();
                let Ok(()) = effects.offer(Effect::Highlight(next));

                Ok(Mode::Open(next))
            }
            (Mode::Closed, ButtonPress::Down | ButtonPress::Back) => Ok(Mode::Closed),
        }
    }
}

impl SerializableMachine for ButtonGuide {
    const VERSION: Version = Version(1);
}

struct LaterGuide;

impl Machine for LaterGuide {
    type Input = ();
    type State = Mode;
    type Request = ButtonPress;
    type Effect = Effect;

    fn initialize(input: &(), previous: Option<Mode>, effects: &mut Queue<Effect>) -> Result<Mode, Never> {
        ButtonGuide::initialize(input, previous, effects)
    }

    fn handle(state: Mode, request: ButtonPress, effects: &mut Queue<Effect>) -> Result<Mode, Never> {
        ButtonGuide::handle(state, request, effects)
    }
}

impl SerializableMachine for LaterGuide {
    const VERSION: Version = Version(2);

    fn migrate(from: Version, decoder: &mut Decoder<'_>) -> Result<Mode, ParseError> {
        match from == ButtonGuide::VERSION {
            true => Mode::decode(decoder),
            false => Err(ParseError::Version(from)),
        }
    }
}

#[test]
fn a_recorded_run_reaches_the_same_place() {
    let Ok(said) = run::<ButtonGuide>(&(), &[ButtonPress::GuideButton, ButtonPress::Down, ButtonPress::Down, ButtonPress::Back]);

    assert_eq!(said.state, Mode::Closed);
    assert_eq!(
        said.effects(),
        Ok(vec![Effect::ShowGuide, Effect::Highlight(Row::Second), Effect::Highlight(Row::Third), Effect::HideGuide])
    );
}

#[test]
fn what_was_decided_is_kept_with_the_request_that_decided_it() {
    let Ok(said) = run::<ButtonGuide>(&(), &[ButtonPress::Down, ButtonPress::GuideButton, ButtonPress::Down]);

    assert_eq!(said.on(0), Ok(Some([].as_slice())), "a press that means nothing while closed decided something");
    assert_eq!(said.on(1), Ok(Some([Effect::ShowGuide].as_slice())));
    assert_eq!(said.on(2), Ok(Some([Effect::Highlight(Row::Second)].as_slice())));
}

#[test]
fn turned_off_and_on_it_is_where_it_was() -> Result<(), Failure> {
    let Ok(left) = run::<ButtonGuide>(&(), &[ButtonPress::GuideButton, ButtonPress::Down]);
    let Ok(saved) = snapshot::<ButtonGuide>(&left.state);
    let previous = restore::<ButtonGuide>(&saved)?;
    let Ok(mut effects) = Queue::unbounded();
    let Ok(state) = ButtonGuide::initialize(&(), Some(previous), &mut effects);

    assert_eq!(state, Mode::Open(Row::Second));
    assert_eq!(effects.take_all(), Ok(vec![Effect::ShowGuide, Effect::Highlight(Row::Second)]), "the overlay was not put back on screen");

    Ok(())
}

#[test]
fn a_snapshot_cut_short_is_met_rather_than_guessed() {
    let Ok(saved) = snapshot::<ButtonGuide>(&Mode::Open(Row::Third));
    let cut = saved.split_last().map(|(_, kept)| kept.to_vec());

    assert_eq!(restore::<ButtonGuide>(&[1, 2, 3]), Err(ParseError::Truncated));
    assert_eq!(cut.map(|cut| restore::<ButtonGuide>(&cut)), Some(Err(ParseError::Corrupt)));
}

#[test]
fn a_flipped_byte_is_met_as_corrupt() {
    let Ok(mut saved) = snapshot::<ButtonGuide>(&Mode::Open(Row::Second));

    match saved.get_mut(5) {
        Some(byte) => *byte = byte.wrapping_add(1),
        None => {}
    }

    assert_eq!(restore::<ButtonGuide>(&saved), Err(ParseError::Corrupt));
}

#[test]
fn an_older_layout_is_read_by_the_machine_that_knows_it() {
    let Ok(saved) = snapshot::<ButtonGuide>(&Mode::Open(Row::Third));

    assert_eq!(restore::<LaterGuide>(&saved), Ok(Mode::Open(Row::Third)));
}

#[test]
fn a_layout_nobody_answers_is_said_rather_than_read() {
    let Ok(saved) = snapshot::<LaterGuide>(&Mode::Open(Row::Third));

    assert_eq!(restore::<ButtonGuide>(&saved), Err(ParseError::Version(Version(2))));
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Book {
    file: Option<PathBuf>,
    title: String,
    page: u32,
}

impl Serializable for Book {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), Never> {
        let Ok(()) = self.file.encode(encoder);
        let Ok(()) = self.title.encode(encoder);

        self.page.encode(encoder)
    }

    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, ParseError> {
        let file = Option::<PathBuf>::decode(decoder)?;
        let title = String::decode(decoder)?;
        let page = u32::decode(decoder)?;

        Ok(Book { file, title, page })
    }
}

struct Shelf;

impl Machine for Shelf {
    type Input = ();
    type State = Vec<Book>;
    type Request = Never;
    type Effect = Never;

    fn initialize(_input: &(), previous: Option<Vec<Book>>, _effects: &mut Queue<Never>) -> Result<Vec<Book>, Never> {
        Ok(previous.into_iter().flatten().collect())
    }

    fn handle(_state: Vec<Book>, request: Never, _effects: &mut Queue<Never>) -> Result<Vec<Book>, Never> {
        match request {}
    }
}

impl SerializableMachine for Shelf {
    const VERSION: Version = Version(1);
}

#[test]
fn a_state_of_lists_paths_and_names_comes_back_whole() -> Result<(), Failure> {
    let shelf = vec![
        Book { file: Some(PathBuf::from("/home/someone/Books/Middlemarch.epub")), title: "Middlemarch".to_string(), page: 412 },
        Book { file: None, title: String::new(), page: 0 },
    ];
    let Ok(saved) = snapshot::<Shelf>(&shelf);
    let read = restore::<Shelf>(&saved)?;

    assert_eq!(read, shelf);

    Ok(())
}
