//! Layout and interaction for native Console screens.
//!
//! A canvas records logical shapes and input targets from the same placement.
//! `console-draw-painting` rasterizes the shapes, and `console-draw-surface`
//! presents the pixels on a Wayland surface.

use std::cmp::Ordering;

use console_core_color::Oklch;
use console_core_color::palette::Wearing;
use console_core_fonts::{EM, LETTERS, MONOSPACED, SERIF};
use console_core_geometry::{Bounds, Containment, CropMinimum, Point, Rectangle, Size};
use console_core_never::Never;
use console_core_shapes::{
  Clip, Cropped, Edge, Font, Panel, Picture, Pixels, Round, Shape, Text, Weight, hover_indicator,
};
use console_draw_painting::{Lines, Run, at_most, measure_text};
use console_draw_surface::{PointerCursor, Surface, SurfaceError};

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
    let Ok(hit) = self.hit(point);

    Ok(hit.map(|(_, action)| action))
  }

  pub fn hit(&self, point: Point<f64>) -> Result<Option<(Area, A)>, Never> {
    Ok(self.entries.iter().rev().find_map(|(area, action)| {
      let Ok(containment) = area.contains_point(point);

      match containment {
        Containment::Inside => Some((*area, *action)),
        Containment::Outside => None,
      }
    }))
  }

  pub fn under_pointer(&self, surface: &Surface) -> Result<Option<(Area, A)>, Never> {
    let Ok(position) = surface.pointer_position();

    match position {
      Some(point) => self.hit(point),
      None => Ok(None),
    }
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
  hover_colors: Option<(Oklch, Oklch)>,
}

impl<A> Default for Frame<A> {
  fn default() -> Self {
    Self {
      shapes: Vec::new(),
      max_scroll: 0,
      targets: Targets::default(),
      hover_colors: None,
    }
  }
}

impl<A: Copy + PartialEq> Frame<A> {
  pub fn paint(&self, surface: &mut Surface, room: Size<u32>) -> Result<(), SurfaceError> {
    let Ok(target) = self.targets.under_pointer(surface);
    let cursor = match target {
      Some(_) => PointerCursor::Hand,
      None => PointerCursor::Default,
    };
    let Ok(()) = surface.set_pointer_cursor(cursor);
    let hover = match (target, self.hover_colors) {
      (Some((area, _)), Some(colors)) => {
        let Ok(size) = area.pixel_size();
        let Ok(indicator) = hover_indicator(area.origin, size, colors);

        Some(indicator)
      }
      (None, _) | (_, None) => None,
    };

    surface.draw(|pixels, device, _| {
      let size = console_draw_painting::Frame {
        device,
        points: room,
      };

      paint_frame(pixels, size, &self.shapes, hover.as_ref())
    })
  }
}

fn paint_frame(
  pixels: &mut [u8],
  size: console_draw_painting::Frame,
  shapes: &[Shape],
  hover: Option<&[Shape; 2]>,
) -> Result<(), Never> {
  let painted = console_draw_painting::onto(pixels, size, shapes);

  let after_hover = match painted {
    Ok(()) => match hover {
      Some(indicator) => console_draw_painting::over(pixels, size, indicator),
      None => Ok(()),
    },
    Err(fault) => Err(fault),
  };

  match after_hover {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("Console canvas: {fault}"),
  }

  Ok(())
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
  pub(crate) fn font(self, height: u32) -> Result<(Font, Weight), Never> {
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
  rule: Oklch,
  lifted: Oklch,
  accent: Oklch,
  ink: Oklch,
  quiet: Oklch,
  disabled: Oklch,
  on_accent: Oklch,
}

impl From<&Wearing> for Style {
  fn from(colors: &Wearing) -> Self {
    Self {
      rule: colors.edge,
      lifted: colors.panel,
      accent: colors.pink,
      ink: colors.text,
      quiet: colors.soft,
      disabled: colors.edge,
      on_accent: colors.night,
    }
  }
}

pub const ROW_RADIUS: u32 = (EM * 2 / 3).unsigned_abs();

#[derive(Clone, Copy)]
pub enum RowTone {
  Normal,
  Focused,
  Destructive,
}

pub struct RowColors {
  pub fill: Oklch,
  pub ink: Oklch,
  pub quiet: Oklch,
}

impl RowTone {
  pub fn colors(self, colors: &Wearing) -> Result<RowColors, Never> {
    Ok(match self {
      Self::Normal => RowColors {
        fill: colors.ground,
        ink: colors.text,
        quiet: colors.soft,
      },
      Self::Focused => RowColors {
        fill: colors.pink,
        ink: colors.night,
        quiet: colors.night,
      },
      Self::Destructive => RowColors {
        fill: colors.coral,
        ink: colors.night,
        quiet: colors.night,
      },
    })
  }
}

pub struct Interaction<A> {
  pub focus: Option<A>,
  pub hover: Option<A>,
}

#[derive(Clone, Copy)]
pub struct TextLayout {
  pub align: Align,
  pub lines: Lines,
}

pub(crate) struct ControlStyle {
  pub background: Option<(Oklch, u32, Option<Oklch>)>,
  pub label_area: Area,
  pub ink: Oklch,
  pub height: u32,
  pub align: Align,
}

pub struct Canvas<A> {
  pub frame: Frame<A>,
  clip: Area,
  style: Style,
  pub(crate) interaction: Interaction<A>,
}

impl<A> Canvas<A> {
  pub fn new(clip: Area, style: Style, interaction: Interaction<A>) -> Result<Self, Never> {
    Ok(Self {
      frame: Frame {
        hover_colors: Some((style.accent, style.on_accent)),
        ..Frame::default()
      },
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
    match area.size.width.cmp(&3) {
      Ordering::Less => Ok(()),
      Ordering::Equal | Ordering::Greater => self.draw_text(
        area,
        title,
        height,
        face,
        ink,
        TextLayout {
          align: alignment,
          lines: Lines(1),
        },
      ),
    }
  }

  pub fn paragraph(
    &mut self,
    area: Area,
    text: &str,
    height: u32,
    face: Face,
    ink: Oklch,
    layout: TextLayout,
  ) -> Result<(), Never> {
    let Ok(visible) = area.inside(self.clip);

    match (visible, text) {
      (None, _) | (Some(_), "") => Ok(()),
      (Some(_), _) => self.draw_text(area, text, height, face, ink, layout),
    }
  }

  fn draw_text(
    &mut self,
    area: Area,
    text: &str,
    height: u32,
    face: Face,
    ink: Oklch,
    layout: TextLayout,
  ) -> Result<(), Never> {
    let width = i32::cast_unsigned(area.size.width);
    let Ok((font, weight)) = face.font(height);
    let Ok(said) = at_most(
      Run {
        said: text,
        weight,
        width,
      },
      &font,
      layout.lines,
    );
    let Ok(measured) = measure_text(
      Run {
        said: &said,
        weight,
        width,
      },
      &font,
    );
    let remaining = area
      .size
      .width
      .saturating_sub(u32::cast_signed(measured.width))
      .max(0);
    let left = match layout.align {
      Align::Left => 0,
      Align::Center => remaining.wrapping_div(2),
      Align::Right => remaining,
    };
    let top = match layout.lines {
      Lines(1) => {
        let measured_height = match i32::try_from(measured.height) {
          Ok(value) => value,
          Err(_outside_the_viewport_range) => i32::MAX,
        };
        area
          .size
          .height
          .saturating_sub(measured_height)
          .max(0)
          .wrapping_div(2)
      }
      Lines(_) => 0,
    };
    self.frame.shapes.push(Shape::Text(Text {
      at: Point {
        x: area.origin.x.wrapping_add(left),
        y: area.origin.y.wrapping_add(top),
      },
      width,
      said,
      weight,
      font,
      ink,
    }));
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

      let background = (interactive && (button == Button::Selected || hover || focus))
        .then_some((colors.lifted, 8, focus.then_some(colors.accent)));
      let ink = match button {
        Button::Selected => colors.ink,
        Button::Unselected => colors.quiet,
        Button::Unavailable => colors.disabled,
      };
      let look = ControlStyle {
        background,
        label_area: area,
        ink,
        height: 13,
        align: Align::Center,
      };
      let Ok(()) = self.control(area, text, interactive.then_some(action), look);
    };

    Ok(())
  }

  pub(crate) fn control(
    &mut self,
    area: Area,
    text: &str,
    action: Option<A>,
    style: ControlStyle,
  ) -> Result<(), Never> {
    match style.background {
      Some((fill, radius, edge)) => {
        let Ok(()) = self.panel(area, fill, radius, edge);
      }
      None => {}
    }

    let Ok(()) = match style.align {
      Align::Left => self.paragraph(
        style.label_area,
        text,
        style.height,
        Face::Medium,
        style.ink,
        TextLayout {
          align: Align::Left,
          lines: Lines(1),
        },
      ),
      Align::Center | Align::Right => self.text(
        style.label_area,
        text,
        style.height,
        Face::Medium,
        style.ink,
        style.align,
      ),
    };

    match action {
      Some(activated) => self.hit(area, activated),
      None => Ok(()),
    }
  }

  pub fn cover(&mut self, area: Area, pixels: &Pixels) -> Result<(), Never> {
    let Ok(visible) = area.inside(self.clip);

    match visible {
      Some(_) => {}
      None => return Ok(()),
    }

    let Ok(size) = area.pixel_size();
    let Ok(destination) = size.map(f64::from);
    let source = Size {
      width: pixels.width,
      height: pixels.height,
    };
    let Ok(crop) = Rectangle::centered_cover(source, destination, CropMinimum::Subpixel);
    let Ok(picture) = Picture::rectangular(area.origin, size, pixels.clone());
    self.frame.shapes.push(Shape::Cropped(Cropped {
      picture,
      from: crop.origin,
      seen: crop.size,
    }));
    Ok(())
  }

  pub fn lift_clip(&mut self, whole: Area) -> Result<(), Never> {
    self.clip = whole;
    self.frame.shapes.push(Shape::Clip(Clip::Lifted));
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
