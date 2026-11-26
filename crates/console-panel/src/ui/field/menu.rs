//! Context actions for the focused text editor.

use console_core_color::palette::Wearing;
use console_core_fonts::TextStyle;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_shapes::{Edge, Panel, Round, Shape, Text, Weight};
use console_draw_surface::{KeyPress, KeyPressKind, Keysym};

const WIDE: u32 = 156;
const ROW: u32 = 30;
const COUNT: u32 = 6;
const COMMAND_MODIFIERS: u32 = 1 | KeyPress::CONTROL | (1 << 3) | (1 << 6);

const ACTIONS: [(&str, Keysym); 6] = [
  ("Undo", Keysym::z),
  ("Redo", Keysym::y),
  ("Cut", Keysym::x),
  ("Copy", Keysym::c),
  ("Paste", Keysym::v),
  ("Select All", Keysym::a),
];

#[derive(Default)]
pub struct Menu(Option<Open>);

struct Open {
  at: Point<i32>,
  selected: u32,
}

pub enum Input {
  Continue(KeyPress),
  Action(KeyPress),
  Consumed,
}

impl Menu {
  pub fn open(&mut self, at: Point<i32>, room: Size<u32>) -> Result<(), Never> {
    let Ok(right) = fitted::<_, i32>(room.width.saturating_sub(WIDE));
    let Ok(bottom) = fitted::<_, i32>(room.height.saturating_sub(ROW.saturating_mul(COUNT)));
    self.0 = Some(Open {
      at: Point {
        x: at.x.clamp(0, right),
        y: at.y.clamp(0, bottom),
      },
      selected: 0,
    });
    Ok(())
  }

  pub fn close(&mut self) -> Result<(), Never> {
    self.0 = None;
    Ok(())
  }

  pub fn key(
    &mut self,
    press: KeyPress,
    place: Option<(Point<i32>, Size<u32>)>,
  ) -> Result<Input, Never> {
    match (
      press.key,
      press.modifiers & COMMAND_MODIFIERS,
      press.kind,
      place,
    ) {
      (Keysym::Menu, 0, KeyPressKind::Initial, Some((point, room)))
      | (Keysym::F10, 1, KeyPressKind::Initial, Some((point, room))) => {
        let Ok(()) = self.open(point, room);
        return Ok(Input::Consumed);
      }
      _ => {}
    }

    let menu = match self.0.as_mut() {
      Some(menu) => menu,
      None => return Ok(Input::Continue(press)),
    };

    Ok(match (press.key, press.kind) {
      (Keysym::Up | Keysym::k, _) => {
        menu.selected = menu.selected.saturating_sub(1);
        Input::Consumed
      }
      (Keysym::Down | Keysym::j, _) => {
        menu.selected = menu.selected.saturating_add(1).min(COUNT.saturating_sub(1));
        Input::Consumed
      }
      (Keysym::Escape, KeyPressKind::Initial) => {
        let Ok(()) = self.close();
        Input::Consumed
      }
      (Keysym::Return | Keysym::KP_Enter | Keysym::space, KeyPressKind::Initial) => {
        let selected = menu.selected;
        let Ok(()) = self.close();
        let Ok(input) = action(selected);
        input
      }
      _ => Input::Consumed,
    })
  }

  pub fn click(&mut self, point: Point<i32>) -> Result<Option<Input>, Never> {
    let menu = match self.0.take() {
      Some(menu) => menu,
      None => return Ok(None),
    };
    let (x, y) = (
      point.x.saturating_sub(menu.at.x),
      point.y.saturating_sub(menu.at.y),
    );
    let row = y.unsigned_abs().saturating_div(ROW);

    Ok(Some(
      match (x >= 0 && x.unsigned_abs() < WIDE && y >= 0 && row < COUNT).then_some(()) {
        Some(()) => {
          let Ok(input) = action(row);
          input
        }
        None => Input::Consumed,
      },
    ))
  }

  pub fn paint(&self, shapes: &mut Vec<Shape>, colors: &Wearing) -> Result<(), Never> {
    let menu = match self.0.as_ref() {
      Some(menu) => menu,
      None => return Ok(()),
    };
    shapes.push(Shape::Panel(Panel {
      at: menu.at,
      size: Size {
        width: WIDE,
        height: ROW.saturating_mul(COUNT),
      },
      fill: colors.night,
      round: Round(4),
      edge: Edge::Of {
        wide: 1,
        color: colors.pink,
      },
    }));
    let Ok(font) = TextStyle::Callout.font();

    for (row, (label, _)) in (0_u32..COUNT).zip(ACTIONS) {
      let Ok(top) = fitted::<_, i32>(ROW.saturating_mul(row));
      let y = menu.at.y.saturating_add(top);

      match (row == menu.selected).then_some(()) {
        Some(()) => shapes.push(Shape::Panel(Panel {
          at: Point { x: menu.at.x, y },
          size: Size {
            width: WIDE,
            height: ROW,
          },
          fill: colors.fill,
          round: Round(4),
          edge: Edge::None,
        })),
        None => {}
      }

      shapes.push(Shape::Text(Text {
        at: Point {
          x: menu.at.x.saturating_add(11),
          y: y.saturating_add(5),
        },
        said: label.to_owned(),
        width: WIDE.saturating_sub(22),
        weight: Weight::Plain,
        font: font.clone(),
        ink: colors.text,
      }));
    }

    Ok(())
  }
}

fn action(row: u32) -> Result<Input, Never> {
  let Ok(index) = console_core_number_conversion::index(row);
  Ok(match ACTIONS.get(index) {
    Some((_, key)) => {
      let Ok(press) = KeyPress::with_modifiers(*key, KeyPress::CONTROL);
      Input::Action(press)
    }
    None => Input::Consumed,
  })
}
