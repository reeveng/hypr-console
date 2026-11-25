//! A card, in numbers: how wide, how far down, and how far along a reading is.
//!
//! The card sits in the top-right corner of the area the compositor leaves
//! after reserving space for the bar. Its top and right margins are relative
//! to that area.
//!
//! Three stacks fit at rest. Hovering opens a stack in place and shows up to
//! three of its cards. The wheel can bring older cards in the stack into view.
//!
//! The fill is the one piece of arithmetic here that is not a constant. A
//! reading -- the volume, the brightness, what is left in the battery -- is
//! sent as a number between zero and a hundred in a hint, and what a card
//! does with it is fill a bar that far across. Which is a proportion of a
//! width this file already knows, so it is answered here rather than measured
//! by whatever is drawing: a bar filled to a width no one worked out is the
//! strip under the bar all over again, painting to a mark the number never
//! meant.

use console_core_color::Oklch;
use console_core_fonts::{EM, TextStyle};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{Float, fitted, index, toward_zero_i32};
use console_core_shapes::{Edge as Border, Panel, Round, Shape, Text};
use console_draw_painting::{Lines, Run, at_most};

use crate::reading::Urgency;
use crate::serving::{Action, Choice, Target};

pub use console_onscreen::NOTIFICATION as WHO;

pub use console_core_color::palette::{PaletteError, SPENDS, Wearing};

pub const WIDE: i32 = EM * 160 / 9;

pub const IN: i32 = EM * 4 / 9;

pub const PAD: i32 = EM * 5 / 9;

pub const EDGE: i32 = EM / 9;

pub const ROUND: i32 = EM * 2 / 9;

pub const BETWEEN: i32 = EM * 4 / 9;

pub const DEEP: i32 = EM / 3;

pub const MOST: u32 = 3;

const WHOLE: i64 = 100;

pub fn x() -> Result<i32, Never> {
  Ok(
    WIDE
      .saturating_sub(PAD.saturating_mul(2))
      .saturating_sub(EDGE.saturating_mul(2)),
  )
}

pub fn filled(value: i64) -> Result<i32, Never> {
  let Ok(across) = x();

  let held = value.clamp(0, WHOLE);
  let Ok(held_2) = held.float();
  let Ok(across_2) = i64::from(across).float();

  toward_zero_i32(across_2 * held_2 / 100.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Worn {
  edge: Oklch,
  ink: Oklch,
}

fn worn(wearing: &Wearing, urgency: Urgency) -> Result<Worn, Never> {
  Ok(match urgency {
    Urgency::Normal => Worn {
      edge: wearing.edge,
      ink: wearing.text,
    },
    Urgency::Low => Worn {
      edge: wearing.soft,
      ink: wearing.soft,
    },
    Urgency::Critical => Worn {
      edge: wearing.coral,
      ink: wearing.text,
    },
  })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measured {
  pub summary: Size<u32>,
  pub body: Option<Size<u32>>,
}

pub const SUMMARY: TextStyle = TextStyle::Headline;

pub const BODY: TextStyle = TextStyle::Callout;

#[derive(Debug, Clone, PartialEq)]
pub struct CardContent<'a> {
  pub id: u32,
  pub raised: u64,
  pub summary: &'a str,
  pub body: &'a str,
  pub urgency: Urgency,
  pub value: Option<i64>,
  pub actions: &'a [Action],
  pub behind: u32,
}

#[derive(Clone, Copy)]
pub struct Banner<'a> {
  pub active: &'a crate::serving::ActiveNotification,
  pub behind: u32,
}

struct Group<'a> {
  application: Option<&'a str>,
  active: Vec<&'a crate::serving::ActiveNotification>,
}

pub fn application(active: &crate::serving::ActiveNotification) -> Result<Option<&str>, Never> {
  let name = active.notification.application.trim();

  Ok(match (active.history, name) {
    (crate::delivery::History::Keep, "") | (crate::delivery::History::Transient, _) => None,
    (crate::delivery::History::Keep, application) => Some(application),
  })
}

pub fn banners<'a>(
  active: &[&'a crate::serving::ActiveNotification],
  expanded: Option<&str>,
  offset: u32,
) -> Result<Vec<Banner<'a>>, Never> {
  use std::collections::BTreeMap;

  let mut positions = BTreeMap::<&str, u32>::new();
  let mut groups: Vec<Group<'_>> = Vec::new();

  for notice in active {
    let Ok(name) = application(notice);
    let found = name.and_then(|app| positions.get(app));
    let group = match found {
      Some(at) => {
        let Ok(position) = index(*at);

        groups.get_mut(position)
      }
      None => None,
    };

    match group {
      Some(existing) => existing.active.push(notice),
      None => {
        match name {
          Some(app) => {
            let Ok(position) = fitted::<_, u32>(groups.len());
            let _old = positions.insert(app, position);
          }
          None => {}
        }

        groups.push(Group {
          application: name,
          active: vec![notice],
        });
      }
    }
  }

  let Ok(maximum) = index(MOST);
  let mut banners = Vec::new();
  let mut room = MOST;

  for group in groups.iter().take(maximum) {
    let Ok(placed) = fitted::<_, u32>(banners.len());

    match (placed >= room).then_some(()) {
      Some(()) => break,
      None => {}
    }

    let Ok(count) = fitted::<_, u32>(group.active.len());

    match (expanded.is_some() && group.application == expanded).then_some(()) {
      Some(()) => {
        room = room.max(placed.saturating_add(count.min(MOST)));
        let Ok(first) = index(offset.min(count.saturating_sub(MOST)));
        banners.extend(
          group
            .active
            .iter()
            .skip(first)
            .take(maximum)
            .map(|notice| Banner {
              active: notice,
              behind: 0,
            }),
        );
      }
      None => {
        banners.extend(group.active.first().map(|notice| Banner {
          active: notice,
          behind: count.saturating_sub(1),
        }));
      }
    }
  }

  Ok(banners)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Card {
  pub shapes: Vec<Shape>,
  pub height: u32,
  buttons: Vec<HitRegion>,
  body_choice: Choice,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitRegion {
  pub target: Target,
  pub panel: Panel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stack {
  pub shapes: Vec<Shape>,
  pub room: Size<u32>,
  pub touching: Vec<HitRegion>,
}

pub fn height(measured: &Measured, value: Option<i64>) -> Result<u32, Never> {
  let Ok(edges) = twice(EDGE);
  let Ok(padding) = twice(PAD);
  let mut down = measured.summary.height;

  match measured.body {
    Some(body) => {
      let Ok(gap) = gap();

      down = down.saturating_add(gap).saturating_add(body.height);
    }
    None => {}
  }

  match value {
    Some(_) => {
      let Ok(gap) = gap();
      let Ok(deep) = deep();

      down = down.saturating_add(gap).saturating_add(deep);
    }
    None => {}
  }

  Ok(down.saturating_add(edges).saturating_add(padding))
}

pub fn card(
  saying: &CardContent<'_>,
  measured: &Measured,
  wearing: &Wearing,
  at: Point<i32>,
) -> Result<Card, Never> {
  let Ok(base_height) = height(measured, saying.value);
  let Ok(worn) = worn(wearing, saying.urgency);
  let Ok(wide) = fitted::<i32, u32>(WIDE);
  let Ok(inner) = x();
  let Ok(inner_2) = fitted::<i32, u32>(inner);
  let Ok(inset) = fitted::<i32, i32>(EDGE.saturating_add(PAD));
  let Ok(border) = fitted::<i32, u32>(EDGE);
  let Ok(round) = fitted::<i32, u32>(ROUND);
  let Ok(summary_font) = SUMMARY.font();
  let Ok(summary_weight) = SUMMARY.weight();
  let Ok(body_font) = BODY.font();
  let Ok(body_weight) = BODY.weight();
  let Ok(controls) = buttons(saying, wearing, at, base_height);
  let tall = controls.height;

  let mut shapes = vec![Shape::Panel(Panel {
    at,
    size: Size {
      width: wide,
      height: tall,
    },
    round: Round(round),
    fill: wearing.panel,
    edge: Border::Of {
      wide: border,
      color: worn.edge,
    },
  })];
  let across = at.x.saturating_add(inset);
  let mut down = at.y.saturating_add(inset);

  shapes.push(Shape::Text(Text {
    at: Point { x: across, y: down },
    width: inner_2,
    said: saying.summary.to_string(),
    weight: summary_weight,
    font: summary_font,
    ink: worn.ink,
  }));

  let Ok(over) = step(measured.summary.height);

  down = down.saturating_add(over);

  match measured.body {
    Some(body) => {
      shapes.push(Shape::Text(Text {
        at: Point { x: across, y: down },
        width: inner_2,
        said: saying.body.to_string(),
        weight: body_weight,
        font: body_font.clone(),
        ink: worn.ink,
      }));

      let Ok(over_2) = step(body.height);

      down = down.saturating_add(over_2);
    }
    None => {}
  }

  match saying.value {
    Some(value) => {
      let Ok(filled) = filled(value);
      let Ok(filled_2) = fitted::<i32, u32>(filled);

      let Ok(deep) = deep();

      shapes.push(Shape::Panel(Panel {
        at: Point { x: across, y: down },
        size: Size {
          width: inner_2,
          height: deep,
        },
        round: Round(round),
        fill: wearing.ground,
        edge: Border::None,
      }));
      shapes.push(Shape::Panel(Panel {
        at: Point { x: across, y: down },
        size: Size {
          width: filled_2,
          height: deep,
        },
        round: Round(round),
        fill: wearing.fill,
        edge: Border::None,
      }));
    }
    None => {}
  }

  shapes.extend(controls.shapes);
  Ok(Card {
    shapes,
    height: tall,
    buttons: controls.regions,
    body_choice: controls.body_choice,
  })
}

struct Buttons {
  height: u32,
  regions: Vec<HitRegion>,
  shapes: Vec<Shape>,
  body_choice: Choice,
}

impl Buttons {
  fn push(
    &mut self,
    region: HitRegion,
    label: &str,
    wearing: &Wearing,
    gap: u32,
  ) -> Result<(), Never> {
    let panel = region.panel;
    self.regions.push(region);
    self.shapes.push(Shape::Panel(panel));
    let Ok(words) = button_label(label, &panel, wearing);
    self.shapes.push(words);
    self.height = self
      .height
      .saturating_add(gap)
      .saturating_add(panel.size.height);
    Ok(())
  }
}

fn buttons(
  saying: &CardContent<'_>,
  wearing: &Wearing,
  at: Point<i32>,
  base_height: u32,
) -> Result<Buttons, Never> {
  let Ok(gap) = gap();
  let Ok(button_height) = fitted::<i32, u32>(EM.saturating_mul(2));
  let margin = EDGE.saturating_add(PAD);
  let Ok(bottom_inset) = fitted::<i32, u32>(margin);
  let Ok(round) = fitted::<i32, u32>(ROUND);
  let Ok(inner) = x();
  let Ok(button_width) = fitted::<i32, u32>(inner);
  let mut controls = Buttons {
    height: base_height,
    regions: Vec::new(),
    shapes: Vec::new(),
    body_choice: Choice::Dismiss,
  };

  for (ordinal, action) in (0_u32..).zip(saying.actions) {
    let choice = Choice::Invoke(ordinal);

    match action.key.as_str() {
      "default" => {
        controls.body_choice = choice;
        continue;
      }
      _ => {}
    }

    match action.label.as_str() {
      "" => continue,
      _ => {}
    }

    let offset = controls
      .height
      .saturating_sub(bottom_inset)
      .saturating_add(gap);
    let Ok(top) = fitted::<u32, i32>(offset);
    let panel = Panel {
      at: Point {
        x: at.x.saturating_add(margin),
        y: at.y.saturating_add(top),
      },
      size: Size {
        width: button_width,
        height: button_height,
      },
      round: Round(round),
      fill: wearing.ground,
      edge: Border::None,
    };
    let region = HitRegion {
      target: Target {
        id: saying.id,
        raised: saying.raised,
        choice,
      },
      panel,
    };
    let Ok(()) = controls.push(region, &action.label, wearing, gap);
  }

  Ok(controls)
}

fn button_label(label: &str, panel: &Panel, wearing: &Wearing) -> Result<Shape, Never> {
  let Ok(insets) = twice(PAD);
  let width = panel.size.width.saturating_sub(insets);
  let Ok(font) = BODY.font();
  let Ok(weight) = BODY.weight();
  let Ok(said) = at_most(
    Run {
      said: label,
      weight,
      width,
    },
    &font,
    Lines(1),
  );

  Ok(Shape::Text(Text {
    at: Point {
      x: panel.at.x.saturating_add(PAD),
      y: panel.at.y.saturating_add(PAD),
    },
    width,
    said,
    weight,
    font,
    ink: wearing.text,
  }))
}

impl Stack {
  pub fn on(&self, at: Point<i32>) -> Result<Option<Target>, Never> {
    Ok(
      self
        .touching
        .iter()
        .find_map(|touching| match touching.panel.covers(at) {
          Ok(console_core_shapes::Covers::Yes) => Some(touching.target),
          Ok(console_core_shapes::Covers::No) | Err(_) => None,
        }),
    )
  }
}

pub fn cards(said: &[(CardContent<'_>, Measured)], wearing: &Wearing) -> Result<Stack, Never> {
  let Ok(wide) = fitted::<i32, u32>(WIDE);
  let mut shapes = Vec::new();
  let mut touching = Vec::new();
  let mut down: u32 = 0;

  for (saying, measured) in said {
    let Ok(at) = fitted::<u32, i32>(down);
    let Ok(card) = card(saying, measured, wearing, Point { x: 0, y: at });
    let mut height = card.height;
    let layers = saying.behind.min(2);
    let Ok(worn) = worn(wearing, saying.urgency);

    for layer in (1..=layers).rev() {
      let inset = layer.saturating_mul(6);
      let rise = layer.saturating_mul(4);
      let Ok(x) = fitted::<u32, i32>(inset);
      let Ok(y) = fitted::<u32, i32>(rise);
      let Ok(round) = fitted::<i32, u32>(ROUND);
      height = height.max(card.height.saturating_add(rise));
      shapes.push(Shape::Panel(Panel {
        at: Point {
          x,
          y: at.saturating_add(y),
        },
        size: Size {
          width: wide.saturating_sub(inset.saturating_mul(2)),
          height: card.height,
        },
        round: Round(round),
        fill: wearing.panel,
        edge: Border::Of {
          wide: 1,
          color: worn.edge,
        },
      }));
    }

    touching.extend(card.buttons);
    touching.push(HitRegion {
      target: Target {
        id: saying.id,
        raised: saying.raised,
        choice: card.body_choice,
      },
      panel: Panel {
        at: Point { x: 0, y: at },
        size: Size {
          width: wide,
          height,
        },
        round: Round(0),
        fill: wearing.panel,
        edge: Border::None,
      },
    });
    shapes.extend(card.shapes);

    let Ok(gap) = gap();

    down = down.saturating_add(height).saturating_add(gap);
  }

  let Ok(gap) = gap();
  let tall = down.saturating_sub(gap);

  Ok(Stack {
    shapes,
    room: Size {
      width: wide,
      height: tall,
    },
    touching,
  })
}

fn step(over: u32) -> Result<i32, Never> {
  let Ok(gap) = gap();

  fitted(over.saturating_add(gap))
}

fn gap() -> Result<u32, Never> {
  fitted(BETWEEN)
}

fn deep() -> Result<u32, Never> {
  fitted(DEEP)
}

fn twice(many: i32) -> Result<u32, Never> {
  fitted(many.saturating_mul(2))
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::BTreeMap;

  #[test]
  fn a_reading_of_nothing_fills_nothing_and_a_full_one_fills_the_width() {
    assert_eq!(filled(0), Ok(0));
    assert_eq!(filled(100), x());
  }

  #[test]
  fn a_reading_halfway_is_halfway_across() {
    let Ok(across) = x();
    let Ok(half) = filled(50);

    assert!(
      (half.saturating_mul(2).saturating_sub(across)).abs() <= 1,
      "{half} of {across}"
    );
  }

  #[test]
  fn a_reading_past_either_end_is_the_end_rather_than_a_bar_off_the_card() {
    assert_eq!(filled(-20), Ok(0));
    assert_eq!(filled(4000), x());
  }

  pub(super) fn wearing() -> Result<Wearing, PaletteError> {
    let mut spent = BTreeMap::new();

    for (named, six) in [
      ("panel", "372c3a"),
      ("text", "f7e7f3"),
      ("edge", "8a7d8e"),
      ("soft", "e5cde0"),
      ("coral", "ff8f8f"),
      ("ground", "231b26"),
      ("fill", "723b5f"),
      ("night", "1a1320"),
      ("pink", "f9b8d8"),
    ] {
      spent.insert(named.to_string(), six.to_string());
    }

    Wearing::out_of(&spent)
  }

  pub(super) const MEASURED: Measured = Measured {
    summary: Size {
      width: 280,
      height: 20,
    },
    body: None,
  };

  pub(super) const SAYING: CardContent<'static> = CardContent {
    id: 1,
    raised: 1,
    summary: "something happened",
    body: "",
    urgency: Urgency::Normal,
    value: None,
    actions: &[],
    behind: 0,
  };

  #[test]
  fn a_palette_missing_a_color_names_it_rather_than_drawing_in_black() {
    let mut spent = BTreeMap::new();

    spent.insert("panel".to_string(), "372c3a".to_string());

    assert_eq!(Wearing::out_of(&spent), Err(PaletteError::Absent("text")));
  }

  #[test]
  fn a_palette_spending_something_that_is_not_a_color_says_so() {
    let mut spent = BTreeMap::new();

    for named in SPENDS {
      spent.insert(named.to_string(), "372c3a".to_string());
    }

    spent.insert("edge".to_string(), "mauve".to_string());

    assert_eq!(
      Wearing::out_of(&spent),
      Err(PaletteError::Invalid {
        color: "edge",
        value: "mauve".to_string()
      })
    );
  }

  #[test]
  fn a_card_is_its_edges_its_padding_and_what_it_says() {
    let Ok(y) = height(&MEASURED, None);

    let Ok(padding) = twice(PAD);
    let Ok(edges) = twice(EDGE);

    assert_eq!(y, padding.saturating_add(edges).saturating_add(20));
  }

  #[test]
  fn a_reading_makes_a_card_taller_by_the_bar_and_the_gap_over_it() {
    let Ok(without) = height(&MEASURED, None);
    let Ok(with) = height(&MEASURED, Some(50));

    let Ok(gap) = gap();
    let Ok(deep) = deep();

    assert_eq!(with.saturating_sub(without), gap.saturating_add(deep));
  }

  #[test]
  fn a_card_with_nothing_but_a_summary_is_a_panel_and_a_run_of_words() -> Result<(), PaletteError>
  {
    let wearing = wearing()?;
    let Ok(card) = card(&SAYING, &MEASURED, &wearing, Point { x: 0, y: 0 });

    assert_eq!(card.shapes.len(), 2, "{:?}", card.shapes);

    Ok(())
  }

  #[test]
  fn a_reading_adds_the_bar_and_the_fill_over_it() -> Result<(), PaletteError> {
    let wearing = wearing()?;
    let saying = CardContent {
      value: Some(50),
      ..SAYING
    };
    let Ok(card) = card(&saying, &MEASURED, &wearing, Point { x: 0, y: 0 });

    assert_eq!(card.shapes.len(), 4, "{:?}", card.shapes);

    Ok(())
  }

  #[test]
  fn what_a_card_says_starts_inside_its_edge_and_its_padding() -> Result<(), PaletteError> {
    let wearing = wearing()?;
    let Ok(card) = card(&SAYING, &MEASURED, &wearing, Point { x: 0, y: 0 });
    let inset = EDGE.saturating_add(PAD);
    let words = card.shapes.iter().find_map(|shape| match shape {
      Shape::Text(words) => Some(words.at),
      Shape::Panel(_)
      | Shape::Picture(_)
      | Shape::Cropped(_)
      | Shape::Line(_)
      | Shape::Clip(_) => None,
    });

    assert_eq!(words, Some(Point { x: inset, y: inset }));

    Ok(())
  }

  #[test]
  fn a_low_notification_wears_the_soft_color_on_both_its_edge_and_its_words()
  -> Result<(), PaletteError> {
    let wearing = wearing()?;
    let saying = CardContent {
      urgency: Urgency::Low,
      ..SAYING
    };
    let Ok(card) = card(&saying, &MEASURED, &wearing, Point { x: 0, y: 0 });
    let worn = &wearing;
    let ink = card.shapes.iter().find_map(|shape| match shape {
      Shape::Text(words) => Some(words.ink),
      Shape::Panel(_)
      | Shape::Picture(_)
      | Shape::Cropped(_)
      | Shape::Line(_)
      | Shape::Clip(_) => None,
    });

    assert_eq!(ink, Some(worn.soft));

    Ok(())
  }

  #[test]
  fn a_critical_notification_wears_coral_on_its_edge_and_keeps_its_words_readable()
  -> Result<(), PaletteError> {
    let wearing = wearing()?;
    let saying = CardContent {
      urgency: Urgency::Critical,
      ..SAYING
    };
    let Ok(card) = card(&saying, &MEASURED, &wearing, Point { x: 0, y: 0 });
    let worn = &wearing;
    let edge = card.shapes.iter().find_map(|shape| match shape {
      Shape::Panel(panel) => Some(panel.edge),
      Shape::Text(_)
      | Shape::Picture(_)
      | Shape::Cropped(_)
      | Shape::Line(_)
      | Shape::Clip(_) => None,
    });
    let ink = card.shapes.iter().find_map(|shape| match shape {
      Shape::Text(words) => Some(words.ink),
      Shape::Panel(_)
      | Shape::Picture(_)
      | Shape::Cropped(_)
      | Shape::Line(_)
      | Shape::Clip(_) => None,
    });

    assert!(
      matches!(edge, Some(console_core_shapes::Edge::Of { color, .. }) if color == worn.coral)
    );
    assert_eq!(ink, Some(worn.text));

    Ok(())
  }

  #[test]
  fn every_visible_card_is_drawn_and_can_be_clicked() -> Result<(), PaletteError> {
    let wearing = wearing()?;
    let said: Vec<_> = (0..4).map(|_| (SAYING, MEASURED)).collect();
    let Ok(stack) = cards(&said, &wearing);

    let Ok(shapes) = fitted::<_, u32>(stack.shapes.len());
    let Ok(touching) = fitted::<_, u32>(stack.touching.len());

    assert_eq!(shapes, 8);
    assert_eq!(touching, 4);

    Ok(())
  }

  #[test]
  fn a_thumb_on_the_second_card_is_on_the_second_card_and_not_the_first()
  -> Result<(), PaletteError> {
    let wearing = wearing()?;
    let first = CardContent { id: 7, ..SAYING };
    let second = CardContent { id: 9, ..SAYING };
    let said = vec![(first, MEASURED), (second, MEASURED)];
    let Ok(stack) = cards(&said, &wearing);
    let Ok(one) = height(&MEASURED, None);
    let Ok(gap) = gap();
    let Ok(into) = fitted::<u32, i32>(one.saturating_add(gap).saturating_add(2));
    let Ok(found) = stack.on(Point { x: 10, y: into });

    let first_target = Target {
      id: 7,
      raised: 1,
      choice: Choice::Dismiss,
    };
    assert_eq!(
      found,
      Some(Target {
        id: 9,
        ..first_target
      })
    );

    use console_draw_surface::{KeyboardFocus, Keysym, layer_focus::Selection};
    let actions: Vec<_> = stack.touching.iter().map(|region| region.target).collect();
    let mut selected = Selection::Idle;
    let now = std::time::Instant::now();
    let Ok(()) = selected.enter(&actions, now);
    let Ok(()) = selected.observe(KeyboardFocus::Here, now);
    assert_eq!(selected.pressed(Keysym::Down, &actions), Ok(None));
    assert_eq!(selected.pressed(Keysym::Return, &actions), Ok(found));
    let Ok(()) = selected.keep(&[first_target]);
    assert_eq!(
      selected.pressed(Keysym::Return, &[first_target]),
      Ok(Some(first_target))
    );

    Ok(())
  }

  #[test]
  fn a_thumb_in_the_gap_between_two_cards_is_on_neither() -> Result<(), PaletteError> {
    let wearing = wearing()?;
    let said = vec![
      (SAYING, MEASURED),
      (CardContent { id: 9, ..SAYING }, MEASURED),
    ];
    let Ok(stack) = cards(&said, &wearing);
    let Ok(one) = height(&MEASURED, None);
    let Ok(into) = fitted::<u32, i32>(one.saturating_add(1));
    let Ok(found) = stack.on(Point { x: 10, y: into });

    assert_eq!(found, None);

    Ok(())
  }

  #[test]
  fn a_stack_of_cards_is_as_tall_as_the_cards_and_the_gaps_between_them()
  -> Result<(), PaletteError> {
    let wearing = wearing()?;
    let said = vec![(SAYING, MEASURED), (SAYING, MEASURED)];
    let Ok(stack) = cards(&said, &wearing);
    let Ok(one) = height(&MEASURED, None);
    let Ok(width) = fitted::<i32, u32>(WIDE);

    let Ok(gap) = gap();

    assert_eq!(stack.room.height, one.saturating_mul(2).saturating_add(gap));
    assert_eq!(stack.room.width, width);

    Ok(())
  }
}

#[cfg(test)]
mod the_stack {
  use super::tests::{MEASURED, SAYING, wearing};
  use super::*;

  #[test]
  fn the_last_card_ends_exactly_where_the_stack_does() -> Result<(), PaletteError> {
    let wearing = wearing()?;
    let said = vec![
      (SAYING, MEASURED),
      (
        SAYING,
        Measured {
          summary: Size {
            width: 280,
            height: 44,
          },
          body: None,
        },
      ),
      (SAYING, MEASURED),
    ];
    let Ok(stack) = cards(&said, &wearing);
    let ended = stack
      .touching
      .iter()
      .map(|touching| {
        let Ok(height) = fitted::<u32, i32>(touching.panel.size.height);

        touching.panel.at.y.saturating_add(height)
      })
      .max();
    let Ok(room) = fitted::<u32, i32>(stack.room.height);

    assert_eq!(
      ended,
      Some(room),
      "the stack is not as tall as what is in it"
    );

    Ok(())
  }

  #[test]
  fn one_application_stacks_and_expands_its_newest_notices() {
    use crate::delivery::History;
    use crate::saying::Expiry;
    use crate::serving::{Holding, Request};

    let mut holding = Holding::default();

    for (application, summary, history) in [
      ("Files", "First", History::Keep),
      ("Browser", "Separate", History::Keep),
      ("Files", "Second", History::Keep),
      ("Files", "Third", History::Keep),
      ("Files", "Fourth", History::Keep),
      ("Files", "Temporary", History::Transient),
      ("Music", "Now Playing", History::Keep),
    ] {
      let Ok(_armed) = holding.raised(&Request {
        application: application.to_owned(),
        summary: summary.to_owned(),
        body: String::new(),
        urgency: Urgency::Normal,
        expiry: Expiry::Stays,
        history,
        replacing: 0,
        value: None,
        actions: Vec::new(),
      });
    }

    let Ok(visible) = holding.visible();
    let Ok(collapsed) = banners(&visible, None, 0);
    let cards: Vec<_> = collapsed
      .iter()
      .map(|banner| (banner.active.notification.summary.as_str(), banner.behind))
      .collect();
    assert_eq!(cards, [("Now Playing", 0), ("Temporary", 0), ("Fourth", 3)]);

    let Ok(expanded) = banners(&visible, Some("Files"), 0);
    let summaries: Vec<_> = expanded
      .iter()
      .map(|banner| banner.active.notification.summary.as_str())
      .collect();
    assert_eq!(
      summaries,
      ["Now Playing", "Temporary", "Fourth", "Third", "Second"]
    );

    let Ok(scrolled) = banners(&visible, Some("Files"), 1);
    let older: Vec<_> = scrolled
      .iter()
      .map(|banner| banner.active.notification.summary.as_str())
      .collect();
    assert_eq!(
      older,
      ["Now Playing", "Temporary", "Third", "Second", "First"]
    );
  }
}
