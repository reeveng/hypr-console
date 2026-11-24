//! Drawing and interaction components for native Console scenes.

use std::cmp::Ordering;

use console_core_color::Oklch;
use console_core_fonts::{LETTERS, MONOSPACED, SERIF};
use console_core_geometry::{Bounds, Containment, Point, Rectangle, Size};
use console_core_never::Never;
use console_core_shapes::{Clip, Edge, Font, Panel, Round, Shape, Text, Weight};
use console_draw_painting::{Lines, Run, at_most, measure_text};
use console_draw_surface::{Surface, SurfaceError};

pub type Area = Rectangle<i32>;

pub trait AreaLayout: Sized {
  fn right(self) -> std::result::Result<i32, Never>;

  fn bottom(self) -> std::result::Result<i32, Never>;

  fn pixel_size(self) -> std::result::Result<Size<u32>, Never>;

  fn contains_point(self, point: Point<f64>) -> Result<Containment, Never>;

  fn inside(self, other: Self) -> std::result::Result<Option<Self>, Never>;
}

impl AreaLayout for Area {
  fn right(self) -> std::result::Result<i32, Never> {
    Ok(i32::wrapping_add(self.origin.x, self.size.width))
  }

  fn bottom(self) -> std::result::Result<i32, Never> {
    Ok(i32::wrapping_add(self.origin.y, self.size.height))
  }

  fn pixel_size(self) -> std::result::Result<Size<u32>, Never> {
    Ok(Size {
      width: i32::cast_unsigned(self.size.width.max(0)),
      height: i32::cast_unsigned(self.size.height.max(0)),
    })
  }

  fn contains_point(self, point: Point<f64>) -> Result<Containment, Never> {
    let Ok(right) = self.right();
    let Ok(bottom) = self.bottom();
    let bounds = Bounds {
      x0: f64::from(self.origin.x),
      y0: f64::from(self.origin.y),
      x1: f64::from(right),
      y1: f64::from(bottom),
    };
    bounds.contains(point)
  }

  fn inside(self, other: Self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let (x, y) = (
      self.origin.x.max(other.origin.x),
      self.origin.y.max(other.origin.y),
    );
    let Ok(right) = self.right();
    let Ok(other_right) = other.right();
    let (w, h) = (
      i32::wrapping_sub(right.min(other_right), x),
      i32::wrapping_sub(
        ({
          let Ok(explicit_value) = self.bottom();
          explicit_value
        })
        .min({
          let Ok(explicit_value) = other.bottom();
          explicit_value
        }),
        y,
      ),
    );

    Ok((w > 0 && h > 0).then_some(Area {
      origin: Point { x, y },
      size: Size {
        width: w,
        height: h,
      },
    }))
  }
}

pub enum FocusDirection {
  Next,
  Previous,
}

pub struct Targets<A> {
  entries: Vec<(Area, A)>,
}

impl<A> Default for Targets<A> {
  fn default() -> Self {
    Self {
      entries: Vec::new(),
    }
  }
}

impl<A: Copy + PartialEq> Targets<A> {
  pub fn add(&mut self, area: Area, action: A) -> Result<(), Never> {
    self.entries.push((area, action));
    Ok(())
  }

  pub fn clear(&mut self) -> Result<(), Never> {
    self.entries.clear();
    Ok(())
  }

  pub fn action(&self, point: Point<f64>) -> Result<Option<A>, Never> {
    Ok(self.entries.iter().rev().find_map(|(area, action)| {
      let Ok(containment) = area.contains_point(point);

      match containment {
        Containment::Inside => Some(*action),
        Containment::Outside => None,
      }
    }))
  }

  pub fn tab(&self, focused: Option<A>, direction: FocusDirection) -> Result<Option<A>, Never> {
    let mut actions = self.entries.iter().map(|(_, action)| *action);
    Ok(match direction {
      FocusDirection::Next => {
        match focused {
          Some(action) => {
            let _matched = actions.find(|present| *present == action);
          }
          None => {}
        }

        actions
          .next()
          .or_else(|| self.entries.first().map(|(_, action)| *action))
      }
      FocusDirection::Previous => actions
        .take_while(|action| Some(*action) != focused)
        .last()
        .or_else(|| self.entries.last().map(|(_, action)| *action)),
    })
  }
}

pub struct Frame<A> {
  pub shapes: Vec<Shape>,
  pub max_scroll: i32,
  pub targets: Targets<A>,
}

impl<A> Default for Frame<A> {
  fn default() -> Self {
    Self {
      shapes: Vec::new(),
      max_scroll: 0,
      targets: Targets::default(),
    }
  }
}

impl<A> Frame<A> {
  pub fn paint(&self, surface: &mut Surface, room: Size<u32>) -> Result<(), SurfaceError> {
    surface.draw(|pixels, device, _| {
      let size = console_draw_painting::Frame {
        device,
        points: room,
      };

      match console_draw_painting::onto(pixels, size, &self.shapes) {
        Ok(()) => {}
        Err(fault) => console_core_logging::warn!("Console canvas: {fault}"),
      }

      Ok(())
    })
  }
}

#[derive(Clone, Copy)]
pub enum Face {
  Body,
  Medium,
  Title,
  Mono,
}

impl Face {
  #[must_use]
  fn font(self, height: u32) -> core::result::Result<(Font, Weight), console_core_never::Never> {
    let family = match self {
      Self::Body | Self::Medium => LETTERS,
      Self::Title => SERIF,
      Self::Mono => MONOSPACED,
    };
    let weight = match self {
      Self::Medium => Weight::Bold,
      Self::Body | Self::Title | Self::Mono => Weight::Plain,
    };

    Ok((
      Font {
        family: family.to_owned(),
        height,
      },
      weight,
    ))
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Button {
  Selected,
  Unselected,
  Unavailable,
}

impl From<bool> for Button {
  fn from(selected: bool) -> Self {
    match selected {
      true => Self::Selected,
      false => Self::Unselected,
    }
  }
}

#[derive(Clone, Copy)]
pub struct Style {
  pub rule: Oklch,
  pub lifted: Oklch,
  pub accent: Oklch,
  pub ink: Oklch,
  pub quiet: Oklch,
  pub disabled: Oklch,
}

pub struct Interaction<A> {
  pub focus: Option<A>,
  pub hover: Option<A>,
}

pub struct Canvas<A> {
  pub frame: Frame<A>,
  clip: Area,
  style: Style,
  interaction: Interaction<A>,
}

impl<A> Canvas<A> {
  pub fn new(clip: Area, style: Style, interaction: Interaction<A>) -> Result<Self, Never> {
    Ok(Self {
      frame: Frame::default(),
      clip,
      style,
      interaction,
    })
  }
}

impl<A: Copy + PartialEq> Canvas<A> {
  pub fn panel(
    &mut self,
    area: Area,
    fill: Oklch,
    radius: u32,
    outline: Option<Oklch>,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      match bool::then_some(area.size.width <= 0 || area.size.height <= 0, ()) {
        Some(()) => {
          return Ok(());
        }
        None => {}
      }

      let at = area.origin;
      let Ok(size) = area.pixel_size();
      self.frame.shapes.push(Shape::Panel(Panel {
        at,
        size,
        round: Round(radius),
        fill,
        edge: outline.map_or(Edge::None, |color| Edge::Of { wide: 1, color }),
      }));
    };

    Ok(())
  }

  pub fn line(
    &mut self,
    origin: Point<i32>,
    width: i32,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let area = Area {
        origin,
        size: Size { width, height: 1 },
      };
      let Ok(()) = self.panel(area, self.style.rule, 0, None);
    };

    Ok(())
  }

  pub fn text(
    &mut self,
    area: Area,
    title: &str,
    height: u32,
    face: Face,
    ink: Oklch,
    alignment: Align,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      match area.size.width.cmp(&3) {
        Ordering::Less => {
          return Ok(());
        }
        Ordering::Equal | Ordering::Greater => {}
      }

      let width = i32::cast_unsigned(area.size.width);
      let Ok((font, weight)) = face.font(height);
      let Ok(title_2) = at_most(
        Run {
          said: title,
          weight,
          width,
        },
        &font,
        Lines(1),
      );
      let Ok(size) = measure_text(
        Run {
          said: &title_2,
          weight,
          width,
        },
        &font,
      );
      let remaining = area
        .size
        .width
        .saturating_sub(u32::cast_signed(size.width))
        .max(0);
      let x = i32::wrapping_add(
        area.origin.x,
        match alignment {
          Align::Left => 0,
          Align::Center => i32::wrapping_div(remaining, 2),
          Align::Right => remaining,
        },
      );
      let y = i32::wrapping_add(
        area.origin.y,
        i32::wrapping_div(
          area
            .size
            .height
            .saturating_sub(u32::cast_signed(size.height)),
          2,
        )
        .max(0),
      );
      self.frame.shapes.push(Shape::Text(Text {
        at: Point { x, y },
        width,
        said: title_2,
        weight,
        font,
        ink,
      }));
    };

    Ok(())
  }

  pub fn hit(
    &mut self,
    area: Area,
    action: A,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      {
        let Ok(inside) = area.inside(self.clip);

        match inside {
          Some(visible) => {
            let Ok(()) = self.frame.targets.add(visible, action);
          }
          None => {}
        };
      }
    };

    Ok(())
  }

  pub fn button(
    &mut self,
    area: Area,
    text: &str,
    action: A,
    button: Button,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let colors = self.style;
      let focus = self.interaction.focus == Some(action);
      let hover = self.interaction.hover == Some(action);

      let interactive = button != Button::Unavailable;

      match (interactive && (button == Button::Selected || hover || focus)).then_some(()) {
        Some(()) => {
          let Ok(()) = self.panel(area, colors.lifted, 8, focus.then_some(colors.accent));
        }
        None => {}
      }

      let ink = match button {
        Button::Selected => colors.ink,
        Button::Unselected => colors.quiet,
        Button::Unavailable => colors.disabled,
      };
      let Ok(()) = self.text(area, text, 13, Face::Medium, ink, Align::Center);

      match interactive.then_some(()) {
        Some(()) => {
          let Ok(()) = self.hit(area, action);
        }
        None => {}
      }
    };

    Ok(())
  }

  pub fn clip(&mut self, area: Area) -> core::result::Result<(), console_core_never::Never> {
    {
      self.clip = area;
      let at = area.origin;
      let Ok(size) = area.pixel_size();
      self.frame.shapes.push(Shape::Clip(Clip::To { at, size }));
    };

    Ok(())
  }
}

#[derive(Clone, Copy)]
pub enum Align {
  Left,
  Center,
  Right,
}
