//! Where the panel is standing, and how the home grid steps.
//!
//! Most of these tabs hold nothing between one drawing and the next: they ask
//! the machine what the sound, the radios and the battery are doing and draw
//! the answer. Three of them go deeper -- Language into which words the machine
//! is in, which alphabets the keyboard types and which one the paddle listens
//! for, Setup into which browser search, where the hour is kept and what opens
//! each kind of file, and Bluetooth into one device at a time -- and the rule
//! worth writing down is where B lands.
//!
//! One state for the whole card, and three pages reading it. `under` is what
//! keeps that honest: a page asked to draw while the standing place belongs to
//! another tab draws its own top rows rather than someone else\'s list. The
//! alternative was a state per page, which is three things to keep in step
//! where the panel only ever stands in one place at a time.
//! Backing out of a page puts the thumb on the row that opened it rather than
//! at the top, because the row that opened it is what someone was looking at.
//!
//! A device is carried here by address rather than by the row it was on. The
//! Bluetooth list is the one list on this panel that changes underneath the
//! thumb: a scan adds strangers and bluez drops the ones it stops hearing, so
//! the fourth row is not the same device it was a second ago and a page opened
//! by number would be a page about whoever moved into that place. The row is
//! carried too, but only to stand on when the page closes, which is a guess
//! that costs nothing when it is wrong.
//!
//! A device can also leave while its own page is up -- forgotten from that
//! page, or dropped by bluez when a scan ends -- and the tab draws the list
//! again when the address it is standing on names nothing. Forgetting says so
//! here as it goes; the other way leaves the page standing on a device that is
//! gone, which costs the one B press that puts it right.
//!
//! The home grid is here for a different reason. Stepping it is arithmetic on
//! someone's screen -- one column fewer, one size up the ladder -- and it was
//! written twice inside two closures with no test on either. The file it ends
//! up in is written by the binary, because where someone's home is is not
//! something this may look up.

use console_home_screen::shape::{self, Shape, Size};
use console_core_never::Never;
use console_program_contract::{Arguments, Effect, Initial, Program, Update, Event};

const THE_FIRST_SIZE: u32 = 0;


pub const SEARCH: u32 = 0;

pub const WHERE: u32 = 1;

pub const CLOCK: u32 = 2;

pub const CALLED: u32 = 3;

pub const FIRST_KIND: u32 = 6;

pub const SAYS: u32 = 0;

pub const TYPES: u32 = 1;

pub const DICTATION: u32 = 2;

pub const DEEPER: u32 = 2;

pub const MEETING: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meeting {
    pub address: String,
    pub at: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deeper {
    pub name: String,
    pub at: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    Settings,
    Search,
    Dictation,
    Kind(u32),
    Meeting(Meeting),
    Languages,
    Language(Deeper),
    Alphabets,
    Zones,
    Zone(Deeper),
    Clock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsEvent {
    Opened(Destination),
    Back,
    Across { shape: Shape, step: i32 },
    Down { shape: Shape, step: i32 },
    Sized { shape: Shape, step: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsEffect {
    Replace(u32),
    HomeScreen(Shape),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Under {
    Language,
    Configuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closes {
    Yes,
    No,
}

pub struct Settings;

impl Program for Settings {
    type State = Destination;
    type Event = SettingsEvent;
    type Effect = SettingsEffect;

    fn init(_argv: &Arguments) -> Initial<Destination> {
        let Ok(opening) = Initial::new(Destination::Settings);

        opening
    }

    fn update(state: &Destination, event: &Event<SettingsEvent>) -> Update<Destination, SettingsEffect> {
        let heard = match event {
            Event::Custom(heard) => heard,

            Event::Opened
            | Event::Changed(_)
            | Event::Tick(_, _)
            | Event::Replied(_)
            | Event::Chosen(_)
            | Event::Stopping => {
                let Ok(nothing) = Update::none(state.clone());

                return nothing;
            }
        };

        let Ok(turn) = match heard {
            SettingsEvent::Opened(onto) => {
                let Ok(at) = standing_on(onto);

                Update::new(onto.clone(), vec![Effect::Custom(SettingsEffect::Replace(at))])
            }

            SettingsEvent::Back => {
                let Ok(row) = row_of(state);

                Update::new(Destination::Settings, vec![Effect::Custom(SettingsEffect::Replace(row))])
            }

            SettingsEvent::Across { shape, step } => {
                let Ok(columns) = stepped(shape.columns, *step);

                let Ok(across) = shape.with_columns(columns);

                grid(state.clone(), across)
            }

            SettingsEvent::Down { shape, step } => {
                let Ok(rows) = stepped(shape.rows, *step);

                let Ok(down) = shape.with_rows(rows);

                grid(state.clone(), down)
            }

            SettingsEvent::Sized { shape, step } => {
                let Ok(size) = rung(shape.size, *step);

                let Ok(sized) = shape.sized(size);

                grid(state.clone(), sized)
            }
        };

        turn
    }
}

fn grid(state: Destination, shape: Shape) -> Result<Update<Destination, SettingsEffect>, Never> {
    Update::new(state, vec![Effect::Custom(SettingsEffect::HomeScreen(shape))])
}

pub fn row_of(onto: &Destination) -> Result<u32, Never> {
    match onto {
        Destination::Dictation => Ok(DICTATION),
        Destination::Kind(at) => Ok(FIRST_KIND.saturating_add(*at)),
        Destination::Meeting(meeting) => Ok(meeting.at),
        Destination::Languages => Ok(SAYS),
        Destination::Alphabets => Ok(TYPES),
        Destination::Zones => Ok(WHERE),
        Destination::Clock => Ok(CLOCK),
        Destination::Language(deeper) | Destination::Zone(deeper) => Ok(deeper.at),
        Destination::Settings | Destination::Search => Ok(SEARCH),
    }
}

fn standing_on(onto: &Destination) -> Result<u32, Never> {
    match onto {
        Destination::Meeting(_) => Ok(MEETING),
        Destination::Settings
        | Destination::Search
        | Destination::Dictation
        | Destination::Kind(_)
        | Destination::Languages
        | Destination::Language(_)
        | Destination::Alphabets
        | Destination::Zones
        | Destination::Zone(_)
        | Destination::Clock => Ok(DEEPER),
    }
}

pub fn closes(onto: &Destination) -> Result<Closes, Never> {
    match onto {
        Destination::Settings => Ok(Closes::Yes),
        Destination::Search
        | Destination::Dictation
        | Destination::Kind(_)
        | Destination::Meeting(_)
        | Destination::Languages
        | Destination::Language(_)
        | Destination::Alphabets
        | Destination::Zones
        | Destination::Zone(_)
        | Destination::Clock => Ok(Closes::No),
    }
}

pub fn under(onto: &Destination) -> Result<Under, Never> {
    match onto {
        Destination::Languages | Destination::Language(_) | Destination::Alphabets | Destination::Dictation => {
            Ok(Under::Language)
        }
        Destination::Settings
        | Destination::Search
        | Destination::Kind(_)
        | Destination::Meeting(_)
        | Destination::Zones
        | Destination::Zone(_)
        | Destination::Clock => Ok(Under::Configuration),
    }
}

fn stepped(now: u32, step: i32) -> Result<u32, Never> {
    match step > 0 {
        true => Ok(now.saturating_add(1)),
        false => Ok(now.saturating_sub(1)),
    }
}

fn rung(now: Size, step: i32) -> Result<Size, Never> {
    let Ok(at) = match shape::EVERY.iter().position(|size| *size == now) {
        Some(at) => console_core_number_conversion::fitted::<_, u32>(at),
        None => Ok(THE_FIRST_SIZE),
    };

    let went = match step > 0 {
        true => at.saturating_add(1),
        false => at.saturating_sub(1),
    };

    let Ok(went) = console_core_number_conversion::index(went);

    Ok(match shape::EVERY.get(went).copied() {
        Some(size) => size,
        None => now,
    })
}

#[cfg(test)]
mod tests {
    use console_program_contract::{Trace, run};

    use super::*;

    fn said(heard: &[SettingsEvent]) -> Trace<Destination, SettingsEvent, SettingsEffect> {
        let events: Vec<Event<SettingsEvent>> = heard.iter().cloned().map(Event::Custom).collect();

        let Ok(told) = run::<Settings>(&Arguments::default(), &events);

        told
    }

    fn effects(said: &Trace<Destination, SettingsEvent, SettingsEffect>) -> Vec<Effect<SettingsEffect>> {
        let Ok(effects) = said.effects();

        effects
    }

    #[test]
    fn backing_out_lands_on_the_row_that_opened_it() {
        let out = said(&[SettingsEvent::Opened(Destination::Kind(3)), SettingsEvent::Back]);

        assert_eq!(out.state, Destination::Settings);
        assert_eq!(effects(&out).last(), Some(&Effect::Custom(SettingsEffect::Replace(FIRST_KIND + 3))));

        let dictation = said(&[SettingsEvent::Opened(Destination::Dictation), SettingsEvent::Back]);

        assert_eq!(effects(&dictation).last(), Some(&Effect::Custom(SettingsEffect::Replace(DICTATION))));
    }

    #[test]
    fn opening_one_stands_the_thumb_where_the_choices_start() {
        assert_eq!(effects(&said(&[SettingsEvent::Opened(Destination::Search)])), vec![Effect::Custom(SettingsEffect::Replace(
            DEEPER
        ))]);
    }

    #[test]
    fn b_leaves_the_panel_only_from_the_top() {
        assert_eq!(closes(&Destination::Settings), Ok(Closes::Yes));
        assert_eq!(closes(&Destination::Search), Ok(Closes::No));
        assert_eq!(closes(&Destination::Kind(0)), Ok(Closes::No));
        assert_eq!(closes(&Destination::Meeting(meeting(4))), Ok(Closes::No));
    }

    fn meeting(at: u32) -> Meeting {
        Meeting { address: "AA:BB:CC:DD:EE:FF".to_string(), at }
    }

    #[test]
    fn a_device_is_still_the_same_device_when_the_list_has_moved_under_it() {
        let out = said(&[SettingsEvent::Opened(Destination::Meeting(meeting(4)))]);

        assert_eq!(out.state, Destination::Meeting(meeting(4)));
        assert_eq!(effects(&out), vec![Effect::Custom(SettingsEffect::Replace(MEETING))]);
    }

    #[test]
    fn closing_a_device_stands_the_thumb_back_on_its_row() {
        let out = said(&[SettingsEvent::Opened(Destination::Meeting(meeting(4))), SettingsEvent::Back]);

        assert_eq!(out.state, Destination::Settings);
        assert_eq!(effects(&out).last(), Some(&Effect::Custom(SettingsEffect::Replace(4))));
    }

    #[test]
    fn the_grid_stops_at_the_narrowest_and_the_shallowest_it_is_allowed() {
        let Ok(narrow) = Shape::USUAL.with_columns(*Shape::COLUMNS.start());

        let Ok(least) = narrow.with_rows(*Shape::ROWS.start());

        assert_eq!(
            effects(&said(&[SettingsEvent::Across { shape: least, step: -1 }])),
            vec![Effect::Custom(SettingsEffect::HomeScreen(least))]
        );
        assert_eq!(
            effects(&said(&[SettingsEvent::Down { shape: least, step: -1 }])),
            vec![Effect::Custom(SettingsEffect::HomeScreen(least))]
        );
    }

    #[test]
    fn the_grid_steps_one_at_a_time_whichever_way_it_is_pushed() {
        let usual = Shape::USUAL;

        let Ok(wider) = usual.with_columns(usual.columns + 1);

        let Ok(shallower) = usual.with_rows(usual.rows - 1);

        assert_eq!(
            effects(&said(&[SettingsEvent::Across { shape: usual, step: 1 }])),
            vec![Effect::Custom(SettingsEffect::HomeScreen(wider))]
        );
        assert_eq!(
            effects(&said(&[SettingsEvent::Down { shape: usual, step: -1 }])),
            vec![Effect::Custom(SettingsEffect::HomeScreen(shallower))]
        );
    }

    #[test]
    fn the_size_stops_at_both_ends_of_the_ladder() {
        let Ok(smallest) = Shape::USUAL.sized(Size::Tiny);

        let Ok(biggest) = Shape::USUAL.sized(Size::Huge);

        assert_eq!(
            effects(&said(&[SettingsEvent::Sized { shape: smallest, step: -1 }])),
            vec![Effect::Custom(SettingsEffect::HomeScreen(smallest))]
        );
        assert_eq!(
            effects(&said(&[SettingsEvent::Sized { shape: biggest, step: 1 }])),
            vec![Effect::Custom(SettingsEffect::HomeScreen(biggest))]
        );
    }
}
