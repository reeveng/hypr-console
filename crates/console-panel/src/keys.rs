//! What a key means here, and what a finger dragged across a row means.
//!
//! The panel is driven by the pad, and the pad arrives as keys: the profile a
//! picker loads turns the d-pad into arrows, A into Enter, B into Escape and
//! the shoulders into the page keys. So this is the whole of what the front of
//! the machine does to a panel.
//!
//! A swipe is here for the same reason and is the same answer said with a
//! hand. Left and right on a row that carries a level is the one thing this
//! desktop offers that a finger could reach only by aiming at a mark the width
//! of a thumbnail, and every gallery anyone has used steps to the next picture
//! by pushing the one in front of them out of the way. So a swipe across a row
//! *is* that row's level, and nothing else has to be taught to it.
//!
//! A key is a keysym and not a toolkit's name for one. This module said it had
//! never heard of GTK and then spelled every key it reads as `gdk::Key`, which
//! is the same number under a name only one drawing library says. What the
//! compositor hands a surface of our own is the keysym itself, so that is what
//! is matched on here and the panel still on GTK converts at its own edge.

use console_core_geometry::Point;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_state_machine::{
  EffectDependencies, Service, SingleEffectMachine, SingleTransition,
};
use console_draw_surface::{Keysym, PhysicalKeyPress};
use console_input_event_devices::KeyCode;

const MORE: [Keysym; 2] = [Keysym::F18, Keysym::XF86Launch9];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meaning {
  Abandon,
  Choose,
  More,
  None,
  Nudge(i32),
  Close,
  Step(i32),
  Tab(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabMotion {
  Next,
  Previous(u32),
  Number(u32),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Navigation {
  #[default]
  Ready,
  Count(u32),
  AfterG(Option<u32>),
}

const SHIFT: u32 = 1;

pub struct NavigationKeys;

pub enum NavigationInput {
  Press {
    key: PhysicalKeyPress,
    driving: Driving,
  },
  Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationEffect {
  Apply(Meaning),
  Tab(TabMotion),
}

impl EffectDependencies for NavigationEffect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(&[Service::Display])
  }
}

impl SingleEffectMachine for NavigationKeys {
  type Input = ();
  type State = Navigation;
  type Request = NavigationInput;
  type Effect = NavigationEffect;

  fn initialize(
    _input: &(),
    _previous: Option<Navigation>,
  ) -> Result<SingleTransition<Navigation, NavigationEffect>, Never> {
    Ok(SingleTransition {
      state: Navigation::Ready,
      effect: None,
    })
  }

  fn handle(
    mut state: Navigation,
    request: NavigationInput,
  ) -> Result<SingleTransition<Navigation, NavigationEffect>, Never> {
    let effect = match request {
      NavigationInput::Press { key, driving } => {
        let Ok(meaning) = state.press(key, driving);
        meaning
      }
      NavigationInput::Cancel => {
        state = Navigation::Ready;
        None
      }
    };

    Ok(SingleTransition { state, effect })
  }
}

impl Navigation {
  fn press(
    &mut self,
    press: PhysicalKeyPress,
    driving: Driving,
  ) -> Result<Option<NavigationEffect>, Never> {
    let pending = std::mem::take(self);
    let Ok(fallback) = meaning(press.translated.key, driving);
    let ordinary = Some(NavigationEffect::Apply(fallback));

    match driving {
      Driving::Panel => {}
      Driving::Question | Driving::Sure | Driving::Search => {
        return Ok(ordinary);
      }
    }

    let Ok(physical) = fitted::<u32, u16>(press.code);
    let code = KeyCode(physical);

    Ok(match (pending, press.depressed_modifiers, code) {
      (Self::AfterG(count), 0, KeyCode::KEY_T) => Some(NavigationEffect::Tab(match count {
        Some(number) => TabMotion::Number(number),
        None => TabMotion::Next,
      })),
      (Self::AfterG(count), SHIFT, KeyCode::KEY_T) => {
        Some(NavigationEffect::Tab(TabMotion::Previous(match count {
          Some(previous) => previous,
          None => 1,
        })))
      }
      (Self::AfterG(count), _, KeyCode::KEY_LEFTSHIFT | KeyCode::KEY_RIGHTSHIFT) => {
        *self = Self::AfterG(count);
        None
      }
      (Self::AfterG(_), _, _) => ordinary,
      (Self::Ready | Self::Count(_), 0, KeyCode::KEY_G) => {
        *self = Self::AfterG(match pending {
          Self::Count(count) => Some(count),
          Self::Ready | Self::AfterG(_) => None,
        });
        None
      }
      (_, 0, KeyCode::KEY_J | KeyCode::KEY_K) => {
        let repeat = match pending {
          Self::Count(count) => count,
          Self::Ready | Self::AfterG(_) => 1,
        };
        let Ok(count) = fitted::<u32, i32>(repeat);
        let step = match (code == KeyCode::KEY_J).then_some(()) {
          Some(()) => count,
          None => count.saturating_neg(),
        };
        Some(NavigationEffect::Apply(Meaning::Step(step)))
      }
      (_, 0, KeyCode::KEY_H) => Some(NavigationEffect::Apply(Meaning::Nudge(-1))),
      (_, 0, KeyCode::KEY_L) => Some(NavigationEffect::Apply(Meaning::Nudge(1))),
      (_, SHIFT, KeyCode::KEY_H) => Some(NavigationEffect::Tab(TabMotion::Previous(1))),
      (_, SHIFT, KeyCode::KEY_L) => Some(NavigationEffect::Tab(TabMotion::Next)),
      (_, 0, _) => {
        let Ok(entered) = digit(code);

        match (pending, entered) {
          (Self::Ready, Some(0) | None) | (Self::Count(_), None) | (Self::AfterG(_), _) => {
            ordinary
          }
          (Self::Ready, Some(digit)) => {
            *self = Self::Count(digit);
            None
          }
          (Self::Count(count), Some(digit)) => {
            *self = Self::Count(count.saturating_mul(10).saturating_add(digit));
            None
          }
        }
      }
      (_, _, _) => ordinary,
    })
  }
}

fn digit(code: KeyCode) -> Result<Option<u32>, Never> {
  Ok(match code {
    KeyCode::KEY_0 => Some(0),
    KeyCode::KEY_1 => Some(1),
    KeyCode::KEY_2 => Some(2),
    KeyCode::KEY_3 => Some(3),
    KeyCode::KEY_4 => Some(4),
    KeyCode::KEY_5 => Some(5),
    KeyCode::KEY_6 => Some(6),
    KeyCode::KEY_7 => Some(7),
    KeyCode::KEY_8 => Some(8),
    KeyCode::KEY_9 => Some(9),
    _ => None,
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driving {
  Panel,
  Question,
  Sure,
  Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sweep {
  Back,
  On,
  None,
}

const MEANT_IT: f64 = 120.0;

pub fn swept(by: Point<f64>) -> Result<Sweep, Never> {
  let sideways = by.x.abs() > by.y.abs() && by.x.abs() >= MEANT_IT;

  Ok(match (sideways, by.x < 0.0) {
    (false, _) => Sweep::None,
    (true, true) => Sweep::On,
    (true, false) => Sweep::Back,
  })
}

impl Sweep {
  pub fn step(self) -> Result<Option<i32>, Never> {
    Ok(match self {
      Sweep::Back => Some(-1),
      Sweep::On => Some(1),
      Sweep::None => None,
    })
  }
}

pub fn meaning(key: Keysym, driving: Driving) -> Result<Meaning, Never> {
  match driving {
    Driving::Panel => driven(key),
    Driving::Question => Ok(match key {
      Keysym::Escape => Meaning::Abandon,
      key_2 => match ::core::primitive::bool::then_some(MORE.contains(&key_2), ()) {
        ::core::option::Option::Some(()) => Meaning::More,
        ::core::option::Option::None => Meaning::None,
      },
    }),
    Driving::Search => sought(key),
    Driving::Sure => Ok(match key {
      Keysym::BackSpace | Keysym::Escape => Meaning::Abandon,
      Keysym::KP_Enter | Keysym::Return | Keysym::space => Meaning::Choose,
      Keysym::Left => Meaning::Nudge(-1),
      Keysym::Right => Meaning::Nudge(1),
      _ => Meaning::None,
    }),
  }
}

fn sought(key: Keysym) -> Result<Meaning, Never> {
  Ok(match key {
    Keysym::Escape => Meaning::Close,
    Keysym::KP_Enter | Keysym::Return => Meaning::Choose,
    Keysym::Down => Meaning::Step(1),
    Keysym::Up => Meaning::Step(-1),
    Keysym::Page_Down => Meaning::Tab(1),
    Keysym::Page_Up => Meaning::Tab(-1),
    key_2 => match ::core::primitive::bool::then_some(MORE.contains(&key_2), ()) {
      ::core::option::Option::Some(()) => Meaning::More,
      ::core::option::Option::None => Meaning::None,
    },
  })
}

fn driven(key: Keysym) -> Result<Meaning, Never> {
  Ok(match key {
    Keysym::BackSpace | Keysym::Escape => Meaning::Close,
    Keysym::KP_Enter | Keysym::Return | Keysym::space => Meaning::Choose,
    Keysym::Down => Meaning::Step(1),
    Keysym::Up => Meaning::Step(-1),
    Keysym::Left => Meaning::Nudge(-1),
    Keysym::Right => Meaning::Nudge(1),
    Keysym::Page_Down => Meaning::Tab(1),
    Keysym::Page_Up => Meaning::Tab(-1),
    key_2 => match ::core::primitive::bool::then_some(MORE.contains(&key_2), ()) {
      ::core::option::Option::Some(()) => Meaning::More,
      ::core::option::Option::None => Meaning::None,
    },
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  fn key(
    code: KeyCode,
    symbol: Keysym,
    depressed_modifiers: u32,
  ) -> Result<PhysicalKeyPress, Never> {
    let Ok(translated) =
      console_draw_surface::KeyPress::with_modifiers(symbol, depressed_modifiers);

    Ok(PhysicalKeyPress {
      code: u32::from(code.0),
      depressed_modifiers,
      translated,
    })
  }

  #[test]
  fn a_count_moves_from_the_same_position_in_qwerty_and_mod_dh() {
    let Ok(three) = key(KeyCode::KEY_3, Keysym::_3, 0);
    let Ok(prefix) = NavigationKeys::handle(
      Navigation::Ready,
      NavigationInput::Press {
        key: three,
        driving: Driving::Panel,
      },
    );
    assert_eq!(prefix.effect, None);

    for (position, symbol, step) in [
      (KeyCode::KEY_J, Keysym::j, 3),
      (KeyCode::KEY_J, Keysym::n, 3),
      (KeyCode::KEY_K, Keysym::k, -3),
      (KeyCode::KEY_K, Keysym::e, -3),
    ] {
      let Ok(press) = key(position, symbol, 0);
      let Ok(result) = NavigationKeys::handle(
        prefix.state,
        NavigationInput::Press {
          key: press,
          driving: Driving::Panel,
        },
      );
      assert_eq!(
        result.effect,
        Some(NavigationEffect::Apply(Meaning::Step(step)))
      );
      assert_eq!(result.state, Navigation::Ready);
    }
  }

  #[test]
  fn the_tab_sequence_uses_positions_and_recognizes_a_shift_pressed_after_g() {
    let events = [
      (KeyCode::KEY_G, Keysym::g, 0),
      (KeyCode::KEY_LEFTSHIFT, Keysym::Shift_L, SHIFT),
      (KeyCode::KEY_T, Keysym::B, SHIFT),
    ];
    let mut state = Navigation::Count(3);
    let mut effect = None;

    for (position, symbol, modifiers) in events {
      let Ok(press) = key(position, symbol, modifiers);
      let Ok(result) = NavigationKeys::handle(
        state,
        NavigationInput::Press {
          key: press,
          driving: Driving::Panel,
        },
      );
      state = result.state;
      effect = result.effect;
    }

    assert_eq!(effect, Some(NavigationEffect::Tab(TabMotion::Previous(3))));
    assert_eq!(state, Navigation::Ready);
  }

  #[test]
  fn search_and_questions_receive_the_symbol_and_cancel_a_navigation_prefix() {
    let Ok(press) = key(KeyCode::KEY_J, Keysym::n, 0);

    for driving in [Driving::Search, Driving::Question] {
      let Ok(result) = NavigationKeys::handle(
        Navigation::Count(3),
        NavigationInput::Press {
          key: press,
          driving,
        },
      );
      assert_eq!(result.effect, Some(NavigationEffect::Apply(Meaning::None)));
      assert_eq!(result.state, Navigation::Ready);
    }
  }

  #[test]
  fn b_closes_a_panel_by_either_of_the_two_names_it_arrives_under() {
    assert_eq!(meaning(Keysym::Escape, Driving::Panel), Ok(Meaning::Close));
    assert_eq!(
      meaning(Keysym::BackSpace, Driving::Panel),
      Ok(Meaning::Close)
    );
  }

  #[test]
  fn the_dpad_moves_the_highlight_up_and_down() {
    assert_eq!(meaning(Keysym::Down, Driving::Panel), Ok(Meaning::Step(1)));
    assert_eq!(meaning(Keysym::Up, Driving::Panel), Ok(Meaning::Step(-1)));
  }

  #[test]
  fn the_shoulders_move_between_tabs_and_the_dpad_moves_within_a_row() {
    assert_eq!(
      meaning(Keysym::Page_Up, Driving::Panel),
      Ok(Meaning::Tab(-1))
    );
    assert_eq!(
      meaning(Keysym::Page_Down, Driving::Panel),
      Ok(Meaning::Tab(1))
    );
    assert_eq!(
      meaning(Keysym::Left, Driving::Panel),
      Ok(Meaning::Nudge(-1))
    );
    assert_eq!(
      meaning(Keysym::Right, Driving::Panel),
      Ok(Meaning::Nudge(1))
    );
  }

  #[test]
  fn while_something_is_being_typed_only_back_and_y_are_the_panels() {
    assert_eq!(
      meaning(Keysym::Escape, Driving::Question),
      Ok(Meaning::Abandon)
    );

    for key in [
      Keysym::BackSpace,
      Keysym::Down,
      Keysym::Left,
      Keysym::Page_Up,
      Keysym::Return,
      Keysym::space,
    ] {
      assert_eq!(meaning(key, Driving::Question), Ok(Meaning::None));
    }
  }

  #[test]
  fn y_asks_a_row_what_else_can_be_done_with_it() {
    assert_eq!(meaning(Keysym::F18, Driving::Panel), Ok(Meaning::More));
  }

  #[test]
  fn y_over_a_password_shows_or_hides_it() {
    assert_eq!(meaning(Keysym::F18, Driving::Question), Ok(Meaning::More));
  }

  #[test]
  fn y_is_heard_under_the_name_the_keymap_gives_it() {
    assert_eq!(
      meaning(Keysym::XF86Launch9, Driving::Panel),
      Ok(Meaning::More)
    );
    assert_eq!(
      meaning(Keysym::XF86Launch9, Driving::Search),
      Ok(Meaning::More)
    );
    assert_eq!(
      meaning(Keysym::XF86Launch9, Driving::Question),
      Ok(Meaning::More)
    );
  }

  #[test]
  fn a_search_line_keeps_the_list_and_lends_out_the_letters() {
    assert_eq!(meaning(Keysym::Down, Driving::Search), Ok(Meaning::Step(1)));
    assert_eq!(meaning(Keysym::Up, Driving::Search), Ok(Meaning::Step(-1)));
    assert_eq!(
      meaning(Keysym::Page_Down, Driving::Search),
      Ok(Meaning::Tab(1))
    );
    assert_eq!(
      meaning(Keysym::Return, Driving::Search),
      Ok(Meaning::Choose)
    );
    assert_eq!(meaning(Keysym::Escape, Driving::Search), Ok(Meaning::Close));

    for key in [
      Keysym::BackSpace,
      Keysym::Left,
      Keysym::Right,
      Keysym::space,
      Keysym::a,
    ] {
      assert_eq!(meaning(key, Driving::Search), Ok(Meaning::None));
    }
  }

  #[test]
  fn a_yes_or_no_question_is_answered_left_and_right() {
    assert_eq!(meaning(Keysym::Left, Driving::Sure), Ok(Meaning::Nudge(-1)));
    assert_eq!(meaning(Keysym::Right, Driving::Sure), Ok(Meaning::Nudge(1)));
    assert_eq!(meaning(Keysym::Return, Driving::Sure), Ok(Meaning::Choose));
    assert_eq!(meaning(Keysym::Escape, Driving::Sure), Ok(Meaning::Abandon));
    assert_eq!(
      meaning(Keysym::BackSpace, Driving::Sure),
      Ok(Meaning::Abandon)
    );
  }

  #[test]
  fn nothing_else_reaches_a_question_of_two_answers() {
    for key in [
      Keysym::Down,
      Keysym::Up,
      Keysym::Page_Down,
      Keysym::Page_Up,
      Keysym::F18,
      Keysym::XF86Launch9,
    ] {
      assert_eq!(meaning(key, Driving::Sure), Ok(Meaning::None));
    }
  }

  #[test]
  fn a_hand_pushing_the_picture_aside_brings_the_next_one_in() {
    assert_eq!(swept(Point { x: -400.0, y: 0.0 }), Ok(Sweep::On));
    assert_eq!(swept(Point { x: 400.0, y: 0.0 }), Ok(Sweep::Back));
    assert_eq!(Sweep::On.step(), Ok(Some(1)));
    assert_eq!(Sweep::Back.step(), Ok(Some(-1)));
  }

  #[test]
  fn a_finger_scrolling_the_list_is_not_stepping_a_row() {
    assert_eq!(swept(Point { x: 30.0, y: -800.0 }), Ok(Sweep::None));
    assert_eq!(
      swept(Point {
        x: -200.0,
        y: 900.0
      }),
      Ok(Sweep::None)
    );
    assert_eq!(Sweep::None.step(), Ok(None));
  }

  #[test]
  fn a_thumb_resting_on_a_row_has_not_asked_for_anything() {
    assert_eq!(swept(Point { x: 0.0, y: 0.0 }), Ok(Sweep::None));
    assert_eq!(
      swept(Point {
        x: MEANT_IT - 1.0,
        y: 0.0
      }),
      Ok(Sweep::None)
    );
    assert_eq!(
      swept(Point {
        x: MEANT_IT,
        y: 0.0
      }),
      Ok(Sweep::Back)
    );
  }

  #[test]
  fn a_key_the_panel_has_no_use_for_belongs_to_whatever_else_is_listening() {
    assert_eq!(meaning(Keysym::a, Driving::Panel), Ok(Meaning::None));
    assert_eq!(meaning(Keysym::F1, Driving::Panel), Ok(Meaning::None));
  }
}
