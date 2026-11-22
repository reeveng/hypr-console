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
//! The same actor holds the active Wi-Fi QR selection independently of the
//! current tab destination.
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

use console_core_media_access_control_addresses::Address;
use console_core_never::Never;
use console_core_state_machine::{Machine, Queue};
use console_home_screen::shape::{self, Shape, Size};
use console_program_contract::{Effect, Event};

const THE_FIRST_SIZE: u32 = 0;

pub const SEARCH: u32 = 0;

pub const WHERE: u32 = 1;

pub const CLOCK: u32 = 2;

pub const CALLED: u32 = 3;

pub const FIRST_KIND: u32 = 7;

pub const SAYS: u32 = 0;

pub const TYPES: u32 = 1;

pub const DICTATION: u32 = 2;

pub const DEEPER: u32 = 2;

pub const MEETING: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meeting {
  pub address: Address,
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
  ShowNetwork(crate::wifi::Network),
  HideNetwork,
  Across { shape: Shape, step: i32 },
  Down { shape: Shape, step: i32 },
  Sized { shape: Shape, step: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsEffect {
  Replace(u32),
  HomeScreen(Shape),
  NetworkShown,
  NetworkHidden,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsState {
  pub destination: Destination,
  pub shared_network: Option<crate::wifi::Network>,
}

impl Machine for Settings {
  type Input = ();
  type State = SettingsState;
  type Request = Event<SettingsEvent>;
  type Effect = Effect<SettingsEffect>;

  fn initialize(
    _input: &(),
    _previous: Option<SettingsState>,
    _effects: &mut Queue<Effect<SettingsEffect>>,
  ) -> Result<SettingsState, Never> {
    Ok(SettingsState {
      destination: Destination::Settings,
      shared_network: None,
    })
  }

  fn handle(
    mut state: SettingsState,
    event: Event<SettingsEvent>,
    effects: &mut Queue<Effect<SettingsEffect>>,
  ) -> Result<SettingsState, Never> {
    match event {
      Event::Custom(SettingsEvent::Opened(onto)) => {
        let Ok(at) = standing_on(&onto);
        let Ok(()) = effects.offer(Effect::Custom(SettingsEffect::Replace(at)));

        state.destination = onto;
        Ok(state)
      }

      Event::Custom(SettingsEvent::Back) => {
        let Ok(row) = row_of(&state.destination);
        let Ok(()) = effects.offer(Effect::Custom(SettingsEffect::Replace(row)));

        state.destination = Destination::Settings;
        Ok(state)
      }

      Event::Custom(SettingsEvent::ShowNetwork(network)) => {
        state.shared_network = Some(network);
        let Ok(()) = effects.offer(Effect::Custom(SettingsEffect::NetworkShown));
        Ok(state)
      }

      Event::Custom(SettingsEvent::HideNetwork) => {
        match state.shared_network.take() {
          Some(_) => {
            let Ok(()) = effects.offer(Effect::Custom(SettingsEffect::NetworkHidden));
          }
          None => {}
        }

        Ok(state)
      }

      Event::Custom(SettingsEvent::Across { shape, step }) => {
        let Ok(columns) = step_index(shape.columns, step);

        let Ok(across) = shape.with_columns(columns);

        grid(state, across, effects)
      }

      Event::Custom(SettingsEvent::Down { shape, step }) => {
        let Ok(rows) = step_index(shape.rows, step);

        let Ok(down) = shape.with_rows(rows);

        grid(state, down, effects)
      }

      Event::Custom(SettingsEvent::Sized { shape, step }) => {
        let Ok(size) = rung(shape.size, step);

        let Ok(sized) = shape.sized(size);

        grid(state, sized, effects)
      }

      Event::Opened
      | Event::Modified(_)
      | Event::Tick(_, _)
      | Event::Replied(_)
      | Event::Written(_)
      | Event::Chosen(_)
      | Event::Stopping => Ok(state),
    }
  }
}

fn grid(
  state: SettingsState,
  shape: Shape,
  effects: &mut Queue<Effect<SettingsEffect>>,
) -> Result<SettingsState, Never> {
  let Ok(()) = effects.offer(Effect::Custom(SettingsEffect::HomeScreen(shape)));

  Ok(state)
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
    Destination::Languages
    | Destination::Language(_)
    | Destination::Alphabets
    | Destination::Dictation => Ok(Under::Language),
    Destination::Settings
    | Destination::Search
    | Destination::Kind(_)
    | Destination::Meeting(_)
    | Destination::Zones
    | Destination::Zone(_)
    | Destination::Clock => Ok(Under::Configuration),
  }
}

fn step_index(now: u32, step: i32) -> Result<u32, Never> {
  match ::core::primitive::bool::then_some(step > 0, ()) {
    ::core::option::Option::Some(()) => Ok(now.saturating_add(1)),
    ::core::option::Option::None => Ok(now.saturating_sub(1)),
  }
}

fn rung(now: Size, step: i32) -> Result<Size, Never> {
  let Ok(at) = match shape::EVERY.iter().position(|size| *size == now) {
    Some(at) => console_core_number_conversion::fitted::<_, u32>(at),
    None => Ok(THE_FIRST_SIZE),
  };

  let Ok(last) =
    console_core_number_conversion::fitted::<_, i32>(shape::EVERY.len().saturating_sub(1));
  let Ok(from) = console_core_number_conversion::fitted::<u32, i32>(at);
  let went = from.saturating_add(step).clamp(0, last);
  let Ok(went_2) = console_core_number_conversion::index(went);

  Ok(match shape::EVERY.get(went_2).copied() {
    Some(size) => size,
    None => now,
  })
}

#[cfg(test)]
mod tests {
  use console_core_state_machine::run;

  use super::*;

  type Ran = (Destination, Vec<Effect<SettingsEffect>>);

  fn ran(heard: &[SettingsEvent]) -> Result<Ran, Never> {
    let events: Vec<Event<SettingsEvent>> = heard.iter().cloned().map(Event::Custom).collect();

    let Ok(told) = run::<Settings>(&(), &events);
    let Ok(effects) = told.effects();

    Ok((told.state.destination, effects))
  }

  const A_DEVICE: Address = Address([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);

  #[test]
  fn hiding_a_shared_network_keeps_the_current_tab_destination() {
    let network = crate::wifi::Network {
      name: "Home".to_string(),
      here: true,
      signal: 80,
      band: crate::wifi::Band::FiveGigahertz,
      locked: true,
    };
    let events = [
      Event::Custom(SettingsEvent::ShowNetwork(network.clone())),
      Event::Custom(SettingsEvent::Opened(Destination::Search)),
    ];
    let Ok(open) = run::<Settings>(&(), &events);
    assert_eq!(open.state.destination, Destination::Search);
    assert_eq!(open.state.shared_network, Some(network));

    let Ok(mut effects) = Queue::unbounded();
    let Ok(closed) = Settings::handle(
      open.state,
      Event::Custom(SettingsEvent::HideNetwork),
      &mut effects,
    );
    assert_eq!(closed.destination, Destination::Search);
    assert_eq!(closed.shared_network, None);
    let Ok(emitted) = effects.take_all();
    assert_eq!(emitted, [Effect::Custom(SettingsEffect::NetworkHidden)]);
  }

  #[test]
  fn backing_out_lands_on_the_row_that_opened_it() {
    let Ok((state, effects)) = ran(&[
      SettingsEvent::Opened(Destination::Kind(3)),
      SettingsEvent::Back,
    ]);

    assert_eq!(state, Destination::Settings);
    assert_eq!(
      effects.last(),
      Some(&Effect::Custom(SettingsEffect::Replace(
        FIRST_KIND.saturating_add(3)
      )))
    );

    let Ok((_state, dictation)) = ran(&[
      SettingsEvent::Opened(Destination::Dictation),
      SettingsEvent::Back,
    ]);

    assert_eq!(
      dictation.last(),
      Some(&Effect::Custom(SettingsEffect::Replace(DICTATION)))
    );
  }

  #[test]
  fn opening_one_stands_the_thumb_where_the_choices_start() {
    let Ok((_state, effects)) = ran(&[SettingsEvent::Opened(Destination::Search)]);

    assert_eq!(
      effects,
      vec![Effect::Custom(SettingsEffect::Replace(DEEPER))]
    );
  }

  #[test]
  fn b_leaves_the_panel_only_from_the_top() {
    let meeting = Meeting {
      address: A_DEVICE,
      at: 4,
    };

    assert_eq!(closes(&Destination::Settings), Ok(Closes::Yes));
    assert_eq!(closes(&Destination::Search), Ok(Closes::No));
    assert_eq!(closes(&Destination::Kind(0)), Ok(Closes::No));
    assert_eq!(closes(&Destination::Meeting(meeting)), Ok(Closes::No));
  }

  #[test]
  fn a_device_is_still_the_same_device_when_the_list_has_moved_under_it() {
    let meeting = Meeting {
      address: A_DEVICE,
      at: 4,
    };
    let Ok((state, effects)) =
      ran(&[SettingsEvent::Opened(Destination::Meeting(meeting.clone()))]);

    assert_eq!(state, Destination::Meeting(meeting));
    assert_eq!(
      effects,
      vec![Effect::Custom(SettingsEffect::Replace(MEETING))]
    );
  }

  #[test]
  fn closing_a_device_stands_the_thumb_back_on_its_row() {
    let meeting = Meeting {
      address: A_DEVICE,
      at: 4,
    };
    let Ok((state, effects)) = ran(&[
      SettingsEvent::Opened(Destination::Meeting(meeting)),
      SettingsEvent::Back,
    ]);

    assert_eq!(state, Destination::Settings);
    assert_eq!(
      effects.last(),
      Some(&Effect::Custom(SettingsEffect::Replace(4)))
    );
  }

  #[test]
  fn the_grid_stops_at_the_narrowest_and_the_shallowest_it_is_allowed() {
    let Ok(narrow) = Shape::USUAL.with_columns(*Shape::COLUMNS.start());

    let Ok(least) = narrow.with_rows(*Shape::ROWS.start());
    let Ok((_state, across)) = ran(&[SettingsEvent::Across {
      shape: least,
      step: -1,
    }]);
    let Ok((_state, down)) = ran(&[SettingsEvent::Down {
      shape: least,
      step: -1,
    }]);

    assert_eq!(
      across,
      vec![Effect::Custom(SettingsEffect::HomeScreen(least))]
    );
    assert_eq!(
      down,
      vec![Effect::Custom(SettingsEffect::HomeScreen(least))]
    );
  }

  #[test]
  fn the_grid_steps_one_at_a_time_whichever_way_it_is_pushed() {
    let usual = Shape::USUAL;

    let Ok(wider) = usual.with_columns(usual.columns.saturating_add(1));

    let Ok(shallower) = usual.with_rows(usual.rows.saturating_sub(1));
    let Ok((_state, across)) = ran(&[SettingsEvent::Across {
      shape: usual,
      step: 1,
    }]);
    let Ok((_state, down)) = ran(&[SettingsEvent::Down {
      shape: usual,
      step: -1,
    }]);

    assert_eq!(
      across,
      vec![Effect::Custom(SettingsEffect::HomeScreen(wider))]
    );
    assert_eq!(
      down,
      vec![Effect::Custom(SettingsEffect::HomeScreen(shallower))]
    );
  }

  #[test]
  fn the_size_stops_at_both_ends_of_the_ladder() {
    let Ok(smallest) = Shape::USUAL.sized(Size::Tiny);

    let Ok(biggest) = Shape::USUAL.sized(Size::Huge);
    let Ok((_state, smaller)) = ran(&[SettingsEvent::Sized {
      shape: smallest,
      step: -1,
    }]);
    let Ok((_state, bigger)) = ran(&[SettingsEvent::Sized {
      shape: biggest,
      step: 1,
    }]);

    assert_eq!(
      smaller,
      vec![Effect::Custom(SettingsEffect::HomeScreen(smallest))]
    );
    assert_eq!(
      bigger,
      vec![Effect::Custom(SettingsEffect::HomeScreen(biggest))]
    );
  }

  #[test]
  fn a_size_chosen_three_rungs_away_is_reached_in_one_step() {
    let Ok(smallest) = Shape::USUAL.sized(Size::Tiny);
    let Ok(bigger) = Shape::USUAL.sized(Size::Bigger);
    let Ok((_state, chosen)) = ran(&[SettingsEvent::Sized {
      shape: smallest,
      step: 3,
    }]);

    assert_eq!(
      chosen,
      vec![Effect::Custom(SettingsEffect::HomeScreen(bigger))]
    );
  }
}
