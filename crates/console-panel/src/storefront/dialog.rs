//! Dialog layouts for details, confirmation, reports, and help.

use console_core_color::palette::Wearing;
use console_core_geometry::{Insets, Point, Size};
use console_core_never::Never;
use console_draw_painting::Lines;

use crate::canvas::{Area, AreaLayout, Face, Interaction};

use super::{Button, Components, Frame, Link, window};

pub struct Dialog<'a, A> {
  pub back: Link<'a, A>,
  pub body: DialogBody<'a, A>,
}

pub struct Topic<'a> {
  pub title: &'a str,
  pub detail: &'a str,
}

pub enum DetailActions<'a, A> {
  WithHint {
    primary: Link<'a, A>,
    hint: &'a str,
  },
  PrimaryOrStatus {
    primary: Option<Link<'a, A>>,
    status: &'a str,
    secondary: Option<Link<'a, A>>,
  },
  Message(&'a str),
}

pub enum DialogBody<'a, A> {
  Details {
    title: &'a str,
    subtitle: &'a str,
    description: &'a str,
    actions: DetailActions<'a, A>,
  },
  Confirmation {
    title: &'a str,
    description: &'a str,
    confirm: Link<'a, A>,
    cancel: Link<'a, A>,
  },
  Report {
    title: &'a str,
    report: &'a str,
  },
  Help {
    title: &'a str,
    topics: &'a [Topic<'a>],
  },
}

#[must_use]
pub fn dialog<A: Copy + PartialEq>(
  view: &Dialog<'_, A>,
  room: Size<u32>,
  colors: Wearing,
  interaction: Interaction<A>,
) -> Result<Frame<A>, Never> {
  let Ok(root) = window(room);
  let Ok(mut components) = Components::new(root, colors, interaction);
  let Ok(()) = components.canvas.clip(root);
  let Ok(()) = components.canvas.panel(root, colors.night, 0, None);
  let (wide, tall) = (root.size.width, root.size.height);
  let width = wide.wrapping_sub(38).clamp(240, 630);
  let preferred = match &view.body {
    DialogBody::Details { .. } | DialogBody::Confirmation { .. } => 267_i32,
    DialogBody::Report { .. } | DialogBody::Help { .. } => 335,
  };
  let height = preferred.min(tall.wrapping_sub(24));
  let panel = Area {
    origin: Point {
      x: wide.wrapping_sub(width).wrapping_div(2),
      y: tall.wrapping_sub(height).wrapping_div(2),
    },
    size: Size { width, height },
  };
  let Ok(()) = components
    .canvas
    .panel(panel, colors.ground, 15, Some(colors.panel));
  let Ok(padding) = Insets::uniform(24);
  let Ok(content) = panel.inset(padding);
  let Ok(right) = content.right();
  let back = Area {
    origin: Point {
      x: right.wrapping_sub(67),
      y: content.origin.y,
    },
    size: Size {
      width: 67,
      height: 38,
    },
  };
  let Ok(()) = components.button(back, view.back, Button::Quiet);
  let Ok(()) = components.dialog_body(content, &view.body);
  Ok(Frame {
    canvas: components.canvas.frame,
    caret: None,
    scroll_area: Area::default(),
  })
}

impl<A: Copy + PartialEq> Components<A> {
  fn dialog_body(&mut self, content: Area, body: &DialogBody<'_, A>) -> Result<(), Never> {
    let (x, y) = (content.origin.x, content.origin.y);
    let width = content.size.width;
    let mut heading = Area {
      origin: content.origin,
      size: Size {
        width: width.wrapping_sub(72),
        height: 42,
      },
    };
    let Ok(bottom) = content.bottom();
    let controls = bottom.wrapping_sub(40);
    let colors = self.colors;

    match body {
      DialogBody::Details {
        title,
        subtitle,
        description,
        actions,
      } => {
        let Ok(()) = self.text(heading, title, 26, Face::Medium, colors.text, Lines(1));
        let version = Area {
          origin: Point {
            x,
            y: y.wrapping_add(46),
          },
          size: Size { width, height: 22 },
        };
        let Ok(()) = self.text(version, subtitle, 13, Face::Mono, colors.soft, Lines(1));
        let detail = Area {
          origin: Point {
            x,
            y: y.wrapping_add(83),
          },
          size: Size { width, height: 74 },
        };
        let Ok(()) = self.text(detail, description, 16, Face::Body, colors.text, Lines(3));
        let buttons = Area {
          origin: Point { x, y: controls },
          size: Size { width, height: 41 },
        };
        self.detail_actions(buttons, actions)
      }
      DialogBody::Confirmation {
        title,
        description,
        confirm,
        cancel,
      } => {
        let Ok(()) = self.text(heading, title, 25, Face::Medium, colors.text, Lines(1));
        let detail = Area {
          origin: Point {
            x,
            y: y.wrapping_add(73),
          },
          size: Size { width, height: 52 },
        };
        let Ok(()) = self.text(detail, description, 16, Face::Body, colors.soft, Lines(2));
        let primary = Area {
          origin: Point { x, y: controls },
          size: Size {
            width: 148,
            height: 41,
          },
        };
        let Ok(()) = self.button(primary, *confirm, Button::Primary);
        let secondary = Area {
          origin: Point {
            x: x.wrapping_add(160),
            y: controls,
          },
          size: Size {
            width: 85,
            height: 41,
          },
        };
        self.button(secondary, *cancel, Button::Normal)
      }
      DialogBody::Report { title, report } => {
        heading.size.height = 44;
        let Ok(()) = self.text(heading, title, 19, Face::Medium, colors.text, Lines(2));
        let surface = Area {
          origin: Point {
            x,
            y: y.wrapping_add(57),
          },
          size: Size {
            width,
            height: content.size.height.wrapping_sub(57),
          },
        };
        let Ok(()) = self.canvas.panel(surface, colors.night, 8, None);
        let Ok(padding) = Insets::uniform(11);
        let Ok(room) = surface.inset(padding);
        let lines = i32::cast_unsigned(room.size.height.wrapping_div(17).max(1));
        self.text(room, report, 12, Face::Mono, colors.soft, Lines(lines))
      }
      DialogBody::Help { title, topics } => {
        let Ok(()) = self.text(heading, title, 25, Face::Medium, colors.text, Lines(1));
        self.help_topics(content, topics)
      }
    }
  }

  fn help_topics(&mut self, content: Area, topics: &[Topic<'_>]) -> Result<(), Never> {
    let colors = self.colors;
    let x = content.origin.x;
    let width = content.size.width;
    let mut y = content.origin.y.wrapping_add(55);

    for topic in topics {
      let title = Area {
        origin: Point { x, y },
        size: Size { width, height: 22 },
      };
      let Ok(()) = self.text(title, topic.title, 16, Face::Medium, colors.pink, Lines(1));
      let detail = Area {
        origin: Point {
          x,
          y: y.wrapping_add(26),
        },
        size: Size { width, height: 46 },
      };
      let Ok(()) = self.text(detail, topic.detail, 15, Face::Body, colors.soft, Lines(2));
      y = y.wrapping_add(72);
    }

    Ok(())
  }

  fn detail_actions(&mut self, area: Area, actions: &DetailActions<'_, A>) -> Result<(), Never> {
    let (x, y) = (area.origin.x, area.origin.y);

    match actions {
      DetailActions::WithHint { primary, hint } => {
        let button = Area {
          origin: Point { x, y },
          size: Size {
            width: 97,
            height: 41,
          },
        };
        let Ok(()) = self.button(button, *primary, Button::Primary);
        let detail = Area {
          origin: Point {
            x: x.wrapping_add(112),
            y,
          },
          size: Size {
            width: area.size.width.wrapping_sub(112),
            height: 41,
          },
        };
        self.text(detail, hint, 14, Face::Body, self.colors.soft, Lines(1))
      }
      DetailActions::PrimaryOrStatus {
        primary,
        status,
        secondary,
      } => {
        let button = Area {
          origin: Point { x, y },
          size: Size {
            width: 151,
            height: 41,
          },
        };

        match primary {
          Some(link) => {
            let Ok(()) = self.button(button, *link, Button::Primary);
          }
          None => {
            let Ok(()) = self.text(button, status, 15, Face::Medium, self.colors.pink, Lines(1));
          }
        }

        match secondary {
          Some(link) => {
            let secondary_button = Area {
              origin: Point {
                x: x.wrapping_add(164),
                y,
              },
              size: Size {
                width: 98,
                height: 41,
              },
            };
            self.button(secondary_button, *link, Button::Normal)
          }
          None => Ok(()),
        }
      }
      DetailActions::Message(message) => {
        let room = Area {
          origin: Point { x, y },
          size: Size {
            width: area.size.width,
            height: 45,
          },
        };
        self.text(room, message, 14, Face::Body, self.colors.soft, Lines(2))
      }
    }
  }
}
