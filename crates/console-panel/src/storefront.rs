//! A catalogue with navigation, search, featured content, entries, and dialogs.

mod dialog;
mod listing;

pub use dialog::{DetailActions, Dialog, DialogBody, Topic, dialog};
pub use listing::{
  Catalogue, Entry, EntryLayout, Feature, Introduction, Search, Sidebar, catalogue,
};

use console_core_color::palette::Wearing;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_draw_painting::Lines;
use console_draw_surface::Part;

use crate::canvas::{
  self, Align, Area, Canvas, ControlStyle, Face, Interaction, Style, TextLayout,
};

#[derive(Clone, Copy)]
pub struct Link<'a, A> {
  pub label: &'a str,
  pub action: A,
}

#[derive(Clone, Copy)]
pub struct Message<'a> {
  pub title: &'a str,
  pub detail: Option<&'a str>,
}

pub struct Notice<'a, A> {
  pub message: Message<'a>,
  pub details: Option<Link<'a, A>>,
  pub reply: Option<Link<'a, A>>,
}

pub struct Frame<A> {
  pub canvas: canvas::Frame<A>,
  pub caret: Option<Part>,
  pub scroll_area: Area,
}

impl<A> Default for Frame<A> {
  fn default() -> Self {
    Self {
      canvas: canvas::Frame::default(),
      caret: None,
      scroll_area: Area::default(),
    }
  }
}

#[derive(Clone, Copy)]
enum Button {
  Primary,
  Normal,
  Quiet,
  Selected,
}

struct Components<A> {
  canvas: Canvas<A>,
  colors: Wearing,
}

impl<A: Copy + PartialEq> Components<A> {
  fn new(root: Area, colors: Wearing, interaction: Interaction<A>) -> Result<Self, Never> {
    let style = Style {
      rule: colors.panel,
      lifted: colors.panel,
      accent: colors.pink,
      ink: colors.text,
      quiet: colors.soft,
      disabled: colors.edge,
    };
    let Ok(canvas) = Canvas::new(root, style, interaction);
    Ok(Self { canvas, colors })
  }

  fn text(
    &mut self,
    area: Area,
    text: &str,
    height: u32,
    face: Face,
    ink: console_core_color::Oklch,
    lines: Lines,
  ) -> Result<(), Never> {
    self.canvas.paragraph(
      area,
      text,
      height,
      face,
      ink,
      TextLayout {
        align: Align::Left,
        lines,
      },
    )
  }

  fn button(&mut self, area: Area, link: Link<'_, A>, kind: Button) -> Result<(), Never> {
    let colors = self.colors;
    let active = self.canvas.interaction.focus == Some(link.action)
      || self.canvas.interaction.hover == Some(link.action);
    let (fill, ink) = match (kind, active) {
      (Button::Primary, _) => (colors.pink, colors.night),
      (Button::Selected, _) => (colors.fill, colors.text),
      (Button::Normal, _) | (Button::Quiet, true) => (colors.panel, colors.text),
      (Button::Quiet, false) => (colors.ground, colors.soft),
    };
    let label_area = Area {
      origin: Point {
        x: area.origin.x.wrapping_add(13),
        y: area.origin.y,
      },
      size: Size {
        width: area.size.width.wrapping_sub(22),
        height: area.size.height,
      },
    };
    self.canvas.control(
      area,
      link.label,
      Some(link.action),
      ControlStyle {
        background: Some((fill, 9, active.then_some(colors.pink))),
        label_area,
        ink,
        height: 14,
        align: Align::Left,
      },
    )
  }
}

fn window(room: Size<u32>) -> Result<Area, Never> {
  Ok(Area {
    origin: Point { x: 0, y: 0 },
    size: Size {
      width: u32::cast_signed(room.width),
      height: u32::cast_signed(room.height),
    },
  })
}
