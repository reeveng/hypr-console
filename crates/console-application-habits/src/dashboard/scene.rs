//! Lay out the window and its pointer targets from the same logical geometry.

use std::cmp::Ordering;
use std::num::NonZeroI32;

use console_core_color::{Oklch, Rgba, hexcode};
use console_core_fonts::{LETTERS, MONOSPACED, SERIF};
use console_core_geometry::{Point, Size};
use console_core_jpeg_files::JpegError;
use console_core_never::Never;
use console_core_number_conversion::{
  Float, checked_index, fitted, index, toward_zero_u8, whole_i32,
};
use console_core_shapes::{Clip, Edge, Font, Panel, Picture, Pixels, Round, Shape, Text, Weight};
use console_draw_painting::{Lines, Run, at_most, measure_text};

use super::calendar::{CURRENT_DAY, Day};
use super::model::{Kind, Lane, LaneView, Period, Scope, duration, lanes};
use super::theme::Palette;
use super::{Action, Dashboard, Page, RecorderState, Refresh, Rows};
use crate::Result;

const TOP: i32 = 69;
const FOOTER: i32 = 36;
const ROW: i32 = 38;

pub fn titlebar((_, y): (f64, f64)) -> std::result::Result<Option<()>, Never> {
  Ok((0.0..53.0).contains(&y).then_some(()))
}

const ART: &[u8] = include_bytes!("../../assets/woodland.jpg");

#[derive(Clone, Copy, Default)]
struct Area {
  x: i32,
  y: i32,
  w: i32,
  h: i32,
}

impl Area {
  #[must_use]
  fn right(self) -> core::result::Result<i32, console_core_never::Never> {
    Ok(i32::wrapping_add(self.x, self.w))
  }

  #[must_use]
  fn bottom(self) -> core::result::Result<i32, console_core_never::Never> {
    Ok(i32::wrapping_add(self.y, self.h))
  }

  #[must_use]
  fn origin(self) -> core::result::Result<Point<i32>, console_core_never::Never> {
    Ok(Point {
      x: self.x,
      y: self.y,
    })
  }

  #[must_use]
  fn size(self) -> core::result::Result<Size<u32>, console_core_never::Never> {
    Ok(Size {
      width: i32::cast_unsigned(self.w.max(0)),
      height: i32::cast_unsigned(self.h.max(0)),
    })
  }

  fn action_at(
    self,
    (x, y): (f64, f64),
    action: Action,
  ) -> std::result::Result<Option<Action>, Never> {
    Ok(
      (x >= f64::from(self.x)
        && x
          < f64::from({
            let Ok(explicit_value) = self.right();
            explicit_value
          })
        && y >= f64::from(self.y)
        && y
          < f64::from({
            let Ok(explicit_value) = self.bottom();
            explicit_value
          }))
      .then_some(action),
    )
  }

  #[must_use]
  fn inside(self, other: Self) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let (x, y) = (self.x.max(other.x), self.y.max(other.y));
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

    Ok((w > 0 && h > 0).then_some(Area { x, y, w, h }))
  }
}

pub struct Art {
  palette: Palette,
  woods: Option<Pixels>,
  palette_error: Option<String>,
}

impl Art {
  pub fn new() -> Result<Self> {
    let mut art = Self {
      palette: {
        let checked_value = Palette::bundled()?;
        checked_value
      },
      woods: None,
      palette_error: None,
    };

    match art.refresh() {
      Ok(Refresh::Unchanged) => {
        art.woods = {
          let Ok(explicit_value) = picture(art.palette);
          explicit_value
        }
      }
      Ok(Refresh::Changed) => {}
    }

    Ok(art)
  }

  pub(super) fn refresh(&mut self) -> std::result::Result<Refresh, Never> {
    let current = match Palette::current() {
      Ok(current) => {
        self.palette_error = None;
        current
      }
      Err(fault) => {
        let message = fault.to_string();

        match bool::then_some(self.palette_error.as_ref() != Some(&message), ()) {
          Some(()) => {
            console_core_logging::warn!("Habits palette: {message}");
            self.palette_error = Some(message);
          }
          None => {}
        }

        return Ok(Refresh::Unchanged);
      }
    };

    let palette = {
      let optional_palette = current.filter(|palette| *palette != self.palette);

      match optional_palette {
        Some(matched_palette) => matched_palette,
        None => return Ok(Refresh::Unchanged),
      }
    };

    self.palette = palette;
    self.woods = {
      let Ok(explicit_value) = picture(palette);
      explicit_value
    };
    Ok(Refresh::Changed)
  }
}

#[must_use]
fn picture(palette: Palette) -> core::result::Result<Option<Pixels>, console_core_never::Never> {
  Ok(match woodland(palette) {
    Ok(pixels) => Some(pixels),
    Err(fault) => {
      console_core_logging::warn!("Habits artwork: {fault}");
      None
    }
  })
}

fn channels(color: Oklch) -> std::result::Result<[u8; 3], Never> {
  let Ok(code) = hexcode(color);
  let rgba = match Rgba::of(&code) {
    Ok(rgba) => rgba,
    Err(fault) => {
      console_core_logging::warn!("Habits artwork color: {fault}");
      Rgba::BLACK
    }
  };
  Ok([rgba.red, rgba.green, rgba.blue])
}

fn woodland(palette: Palette) -> std::result::Result<Pixels, JpegError> {
  let mut art = console_core_jpeg_files::decoded_as_stored(ART)?;
  let Ok(ground) = channels(palette.surface);
  let Ok(accent) = channels(palette.accent);
  let Ok(tint) = tint_table([ground, accent]);
  tint_pixels(&mut art.bytes, art.size, &tint)?;

  let Ok(pixels) = art.into_strided();
  Ok(pixels)
}

fn tint_table(
  [[red, green, blue], [accent_red, accent_green, accent_blue]]: [[u8; 3]; 2],
) -> std::result::Result<[[u8; 3]; 256], Never> {
  Ok(std::array::from_fn(|level| {
    let Ok(level_2) = fitted::<_, u16>(level);

    let shade = level_2.wrapping_mul(3).wrapping_div(4);

    [
      (red, accent_red),
      (green, accent_green),
      (blue, accent_blue),
    ]
    .map(|(base, highlight)| {
      let blended = u16::from(base)
        .wrapping_mul(255_u16.wrapping_sub(shade))
        .wrapping_add(u16::from(highlight).wrapping_mul(shade))
        .wrapping_div(255);
      let [channel, _upper] = blended.to_le_bytes();
      channel
    })
  }))
}

fn tint_pixels(
  bytes: &mut [u8],
  size: Size<u32>,
  tint: &[[u8; 3]; 256],
) -> std::result::Result<(), JpegError> {
  let width = f64::from(size.width);
  let height = f64::from(size.height);
  let stride = checked_index(size.width).map_err(|_| JpegError::TooLarge)?;
  let (mut remaining, tail) = bytes.as_chunks_mut::<4>();

  match bool::then_some(size.width != 0 && tail.is_empty(), ()) {
    Some(()) => {}
    None => return Err(JpegError::Mismatched),
  }

  let opacity = (0..size.width)
    .map(|column| {
      let x = f64::from(column) / width;
      let left = ((x - 0.08) / 0.45).clamp(0.0, 1.0);
      let right = ((1.0 - x) / 0.028).clamp(0.0, 1.0);
      200.0 * left * right
    })
    .collect::<Vec<_>>();

  for row in 0..size.height {
    let (row_pixels, next) = match remaining.split_at_mut_checked(stride) {
      Some(parts) => parts,
      None => return Err(JpegError::Mismatched),
    };
    remaining = next;
    let y = f64::from(row) / height;
    let bottom = ((0.99 - y) / 0.47).clamp(0.0, 1.0);
    let top = (y / 0.17).clamp(0.0, 1.0);

    for (rgba, horizontal) in row_pixels.iter_mut().zip(&opacity) {
      let [red, green, blue, _alpha] = *rgba;
      let light = u16::from(red)
        .wrapping_mul(54)
        .wrapping_add(u16::from(green).wrapping_mul(183))
        .wrapping_add(u16::from(blue).wrapping_mul(19));
      let [_fraction, shade] = light.to_le_bytes();
      let [red_2, green_2, blue_2] = tint
        .get(match ::console_core_number_conversion::index::<u8>(shade) {
          Ok(converted_position) => converted_position,
        })
        .copied()
        .ok_or(JpegError::Mismatched)?;
      let Ok(alpha) = toward_zero_u8(horizontal * top * bottom);
      *rgba = [red_2, green_2, blue_2, alpha];
    }
  }

  Ok(())
}

#[derive(Default)]
pub struct Frame {
  pub shapes: Vec<Shape>,
  pub max_scroll: i32,
  hits: Vec<(Area, Action)>,
}

impl Frame {
  pub fn action(&self, point: (f64, f64)) -> std::result::Result<Option<Action>, Never> {
    Ok(self.hits.iter().rev().find_map(|(area, action)| {
      let Ok(action_2) = area.action_at(point, *action);
      action_2
    }))
  }

  pub fn tab(
    &self,
    focused: Option<Action>,
    direction: FocusDirection,
  ) -> std::result::Result<Option<Action>, Never> {
    let mut actions = self.hits.iter().map(|(_, action)| *action);
    Ok(match direction {
      FocusDirection::Forward => {
        match focused {
          Some(action) => {
            let _matched = actions.find(|present| *present == action);
          }
          None => {}
        }

        actions
          .next()
          .or_else(|| self.hits.first().map(|(_, action)| *action))
      }
      FocusDirection::Backward => actions
        .take_while(|action| Some(*action) != focused)
        .last()
        .or_else(|| self.hits.last().map(|(_, action)| *action)),
    })
  }
}

pub enum FocusDirection {
  Forward,
  Backward,
}

#[derive(Clone, Copy)]
enum Face {
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
enum Button {
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

struct Scene<'a> {
  frame: Frame,
  app: &'a Dashboard,
  palette: Palette,
  clip: Area,
}

impl Scene<'_> {
  fn panel(
    &mut self,
    area: Area,
    fill: Oklch,
    radius: u32,
    outline: Option<Oklch>,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      match bool::then_some(area.w <= 0 || area.h <= 0, ()) {
        Some(()) => {
          return Ok(());
        }
        None => {}
      }

      let Ok(at) = area.origin();
      let Ok(size) = area.size();
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

  fn line(
    &mut self,
    origin: Point<i32>,
    width: i32,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let area = Area { x: origin.x, y: origin.y, w: width, h: 1 };
      let Ok(()) = self.panel(area, self.palette.rule, 0, None);
    };

    Ok(())
  }

  fn text(
    &mut self,
    area: Area,
    title: &str,
    height: u32,
    face: Face,
    ink: Oklch,
    alignment: Align,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      match area.w.cmp(&3) {
        Ordering::Less => {
          return Ok(());
        }
        Ordering::Equal | Ordering::Greater => {}
      }

      let width = i32::cast_unsigned(area.w);
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
      let remaining = area.w.saturating_sub(u32::cast_signed(size.width)).max(0);
      let x = i32::wrapping_add(
        area.x,
        match alignment {
          Align::Left => 0,
          Align::Center => i32::wrapping_div(remaining, 2),
          Align::Right => remaining,
        },
      );
      let y = i32::wrapping_add(
        area.y,
        i32::wrapping_div(area.h.saturating_sub(u32::cast_signed(size.height)), 2).max(0),
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

  fn hit(
    &mut self,
    area: Area,
    action: Action,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      {
        let Ok(inside) = area.inside(self.clip);

        match inside {
          Some(visible) => {
            self.frame.hits.push((visible, action));
          }
          None => {}
        };
      }
    };

    Ok(())
  }

  fn button(
    &mut self,
    area: Area,
    text: &str,
    action: Action,
    button: Button,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let colors = self.palette;
      let focus = self.app.focus == Some(action);
      let hover = self.app.hover == Some(action);

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

  fn clip(&mut self, area: Area) -> core::result::Result<(), console_core_never::Never> {
    {
      self.clip = area;
      let Ok(at) = area.origin();
      let Ok(size) = area.size();
      self.frame.shapes.push(Shape::Clip(Clip::To { at, size }));
    };

    Ok(())
  }
}

#[derive(Clone, Copy)]
enum Align {
  Left,
  Center,
  Right,
}

#[must_use]
pub fn draw(
  app: &Dashboard,
  room: Size<u32>,
  art: &Art,
) -> core::result::Result<Frame, console_core_never::Never> {
  let width = u32::cast_signed(room.width);
  let height = u32::cast_signed(room.height);
  let frame = Frame::default();
  let whole = Area { x: 0, y: 0, w: width, h: height };
  let colors = art.palette;
  let mut scene = Scene {
    frame,
    app,
    palette: colors,
    clip: whole,
  };
  let Ok(()) = scene.panel(whole, colors.ground, 0, None);

  match bool::then_some(width < 300 || height < 155, ()) {
    Some(()) => {
      let Ok(()) = scene.text(
        whole,
        "Enlarge the window to see activity",
        15,
        Face::Body,
        colors.quiet,
        Align::Center,
      );
      return Ok(scene.frame);
    }
    None => {}
  }

  let padding = match width.cmp(&640) {
    Ordering::Less => 17,
    Ordering::Equal | Ordering::Greater => 36,
  };
  let content = i32::wrapping_sub(width, i32::wrapping_mul(2, padding)).min(1070);
  let x = i32::wrapping_div(i32::wrapping_sub(width, content), 2);
  let top = match height.cmp(&500) {
    Ordering::Less => 53,
    Ordering::Equal | Ordering::Greater => TOP,
  };
  let page = Area {
    x,
    y: top,
    w: content,
    h: i32::wrapping_sub(i32::wrapping_sub(height, top), FOOTER),
  };
  let Ok(()) = header(&mut scene, Area { x, y: 0, w: content, h: top });
  let area = Area { x: 0, y: page.y, w: width, h: page.h };
  let Ok(()) = scene.clip(area);
  let lead = match height.cmp(&500) {
    Ordering::Less => 8,
    Ordering::Equal | Ordering::Greater => i32::wrapping_add(
      15,
      i32::wrapping_div(i32::wrapping_sub(height, 900), 8).clamp(0, 30),
    ),
  };
  let column = Area {
    x,
    y: i32::wrapping_sub(i32::wrapping_add(page.y, lead), app.scroll),
    w: content,
    h: i32::wrapping_sub(page.h, lead),
  };
  let end = match app.page {
    Page::Overview => {
      let Ok(explicit_value) = overview(&mut scene, column, art);
      explicit_value
    }
    Page::Timeline => {
      let Ok(explicit_value) = timeline(&mut scene, column);
      explicit_value
    }
  };
  let overflow = i32::wrapping_sub(i32::wrapping_add(i32::wrapping_add(end, app.scroll), 12), {
    let Ok(explicit_value) = page.bottom();
    explicit_value
  })
  .max(0);
  scene.frame.max_scroll = overflow;
  scene.frame.shapes.push(Shape::Clip(Clip::Lifted));

  match bool::then_some(overflow > 0, ()) {
    Some(()) => {
      let rail = page.h.max(1);
      let thumb = match rail.saturating_mul(rail).checked_div(rail.saturating_add(overflow)) {
        Some(length) => length.clamp(18.min(rail), rail),
        None => rail,
      };
      let travel = rail.saturating_sub(thumb);
      let scroll = travel.saturating_mul(app.scroll.min(overflow));
      let offset = match scroll.checked_div(overflow) {
        Some(offset) => offset,
        None => 0,
      };
      let y = page.y.saturating_add(offset);
      let Ok(()) = scene.panel(
        Area { x: i32::wrapping_sub(width, 7), y, w: 2, h: thumb },
        colors.rule,
        1,
        None,
      );
    }
    None => {}
  }

  let Ok(()) = footer(&mut scene, Area { x, y: height.saturating_sub(FOOTER), w: content, h: FOOTER });

  Ok(scene.frame)
}

fn header(scene: &mut Scene<'_>, header: Area) -> core::result::Result<(), console_core_never::Never> {
  {
    let colors = scene.palette;
    let Area { x, w: content, h: height, .. } = header;
    let middle = i32::wrapping_div(height, 2);
    let Ok(()) = scene.panel(
      Area { x: i32::wrapping_add(x, 1), y: i32::wrapping_sub(middle, 4), w: 8, h: 8 },
      colors.accent,
      4,
      None,
    );
    let Ok(()) = scene.text(
      Area {
        x: i32::wrapping_add(x, 17),
        y: i32::wrapping_sub(middle, 23),
        w: 104,
        h: 46,
      },
      "Habits",
      26,
      Face::Title,
      colors.ink,
      Align::Left,
    );
    let center = i32::wrapping_add(header.x, i32::wrapping_div(header.w, 2));
    let tabs = match content.cmp(&490) {
      Ordering::Less => {
        let Ok(right) = header.right();

        i32::wrapping_sub(right, 200)
      }
      Ordering::Equal | Ordering::Greater => i32::wrapping_sub(center, 90),
    };

    for (index, (page, label)) in
      (0_i32..2).zip([(Page::Overview, "Overview"), (Page::Timeline, "Timeline")])
    {
      let area = Area {
        x: i32::wrapping_add(tabs, i32::wrapping_mul(82, index)),
        y: i32::wrapping_sub(middle, 15),
        w: 80,
        h: 30,
      };
      let Ok(()) = scene.button(
        area,
        label,
        Action::Page(page),
        Button::from(scene.app.page == page),
      );
    }

    let Ok(right) = header.right();
    let Ok(()) = scene.button(
      Area {
        x: i32::wrapping_sub(right, 31),
        y: i32::wrapping_sub(middle, 15),
        w: 31,
        h: 30,
      },
      "×",
      Action::Close,
      Button::Unselected,
    );
    let Ok(()) = scene.line(Point { x, y: i32::wrapping_sub(height, 1) }, content);
  };

  Ok(())
}

fn footer(scene: &mut Scene<'_>, area: Area) -> std::result::Result<(), Never> {
  let colors = scene.palette;
  let Area { x, w: width, .. } = area;
  let Ok(()) = scene.line(Point { x: area.x, y: area.y }, area.w);
  let (status, interval) = match (&scene.app.recorder, scene.app.recorder_state) {
    (Some(recorder), RecorderState::Stale) => ("Recorder hasn’t updated", recorder.interval),
    (Some(recorder), RecorderState::Current) => {
      let message = match recorder.status.as_str() {
        "No tracked window in focus" => "Paused while viewing",
        other => other,
      };
      (message, recorder.interval)
    }
    (None, RecorderState::Current | RecorderState::Stale) => {
      ("Waiting for recorder", crate::DEFAULT_INTERVAL)
    }
  };
  let displayed_status = match scene.app.problem {
    Some(problem) => problem,
    None => status,
  };
  let label = match interval {
    60 => "Sampled every minute".to_owned(),
    seconds => {
      let Ok(minutes) = duration(seconds);
      format!("Sampled every {minutes}")
    }
  };
  let half = i32::wrapping_div(width, 2);
  let Ok(()) = scene.text(
    Area { x, y: area.y, w: half, h: area.h },
    &label,
    12,
    Face::Body,
    colors.quiet,
    Align::Left,
  );
  let Ok(()) = scene.text(
    Area {
      x: i32::wrapping_add(x, half),
      y: area.y,
      w: i32::wrapping_sub(width, half),
      h: area.h,
    },
    displayed_status,
    12,
    Face::Body,
    colors.quiet,
    Align::Right,
  );
  Ok(())
}

fn kind_buttons(
  scene: &mut Scene<'_>,
  anchor: Point<i32>,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let Point { x: right, y } = anchor;

    for (index, (kind, text)) in (0_i32..2).zip(Kind::CHOICES) {
      let area = Area {
        x: i32::wrapping_add(i32::wrapping_sub(right, 147), i32::wrapping_mul(74, index)),
        y,
        w: 73,
        h: 30,
      };
      let Ok(()) = scene.button(area, text, Action::Kind(kind), Button::from(scene.app.kind == kind));
    }
  };

  Ok(())
}

struct Bar {
  day: u32,
  seconds: u64,
  title: String,
  label: String,
}

#[must_use]
fn period(day: &Day) -> core::result::Result<Period, console_core_never::Never> {
  Ok(Period {
    start: day.start,
    end: day.end,
  })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChartDensity {
  Compact,
  Comfortable,
}

impl From<bool> for ChartDensity {
  fn from(compact: bool) -> Self {
    match compact {
      true => Self::Compact,
      false => Self::Comfortable,
    }
  }
}

#[must_use]
fn bars(
  app: &Dashboard,
  start: u32,
  density: ChartDensity,
) -> core::result::Result<Vec<Bar>, console_core_never::Never> {
  let last_day = match app.calendar.last() {
    Some(day) => day,
    None => return Ok(Vec::new()),
  };

  match bool::then_some(app.scope == Scope::Today, ()) {
    Some(()) => {
      let ticks = &app.today_ticks;
      return Ok(
        ticks
          .iter()
          .zip(ticks.iter().skip(1).map(|(seconds, _)| *seconds).chain([last_day.end]))
          .enumerate()
          .map(|(index, ((start_2, label), end))| {
            let Ok(position) = fitted::<_, u32>(index);
            let skip = density == ChartDensity::Compact && position.wrapping_rem(2) == 1;
            let label_2 = match skip.then_some(())
            {
              Some(()) => String::new(),
              None => label.clone(),
            };
            let period = Period {
              start: *start_2,
              end,
            };
            let Ok(seconds) = period.total(&app.history);
            Bar {
              day: CURRENT_DAY,
              seconds,
              title: label.clone(),
              label: label_2,
            }
          })
          .collect(),
      );
    }
    None => {}
  }

  let Ok(start_index) = index(start);
  let days = match app.calendar.get(start_index..) {
    Some(days) => days,
    None => return Ok(Vec::new()),
  };

  Ok(
    (start..=CURRENT_DAY)
      .zip(days)
      .map(|(day_number, day)| {
        let label = match app.scope {
          Scope::Week => day.weekday.clone(),
          Scope::Month => match day_number.wrapping_sub(start).wrapping_rem(7) {
            0 => {
              let Ok(label) = day.short();
              label
            }
            _ => String::new(),
          },
          Scope::Today => String::new(),
        };
        let Ok(seconds) = ({
          let Ok(explicit_value) = period(day);
          explicit_value
        })
        .total(&app.history);
        Bar {
          day: day_number,
          seconds,
          title: day.title.clone(),
          label,
        }
      })
      .collect(),
  )
}

fn overview(
  scene: &mut Scene<'_>,
  column: Area,
  art: &Art,
) -> core::result::Result<i32, console_core_never::Never> {
  let app = scene.app;
  let Ok(day_count) = app.scope.days();
  let start = CURRENT_DAY.wrapping_add(1).saturating_sub(day_count);
  let Ok(start_index) = index(start);
  let (first_day, today_day) = match (app.calendar.get(start_index), app.calendar.last()) {
    (Some(first), Some(today)) => (first, today),
    (None, _) | (Some(_), None) => return Ok(column.y),
  };
  let range = Period {
    start: first_day.start,
    end: today_day.end,
  };
  let days = OverviewPeriod { start, first: first_day, last: today_day, range };
  let Ok(ranked_area) = overview_summary(scene, column, art, &days);
  rank_summary(scene, ranked_area, range, ChartDensity::from(column.w < 640))
}

struct OverviewPeriod<'a> {
  start: u32,
  first: &'a Day,
  last: &'a Day,
  range: Period,
}

fn overview_summary(
  scene: &mut Scene<'_>,
  column: Area,
  art: &Art,
  days: &OverviewPeriod<'_>,
) -> std::result::Result<Area, Never> {
  let colors = scene.palette;
  let app = scene.app;
  let compact = column.w < 640;
  let short = column.h < 370;
  let stacked = column.w < 352;
  let hero_height = match bool::then_some(short, ()) {
    Some(()) => column.h.clamp(182, 245),
    None => match bool::then_some(stacked, ()) {
      Some(()) => 326,
      None => 319,
    },
  };
  let hero = Area { x: column.x, y: column.y, w: column.w, h: hero_height };
  let Ok(()) = scene.panel(hero, colors.surface, 13, Some(colors.rule));

  let Ok(()) = overview_art(scene, hero, art);

  let inset = match bool::then_some(compact, ()) {
    Some(()) => 18,
    None => 29,
  };
  let left = i32::wrapping_add(hero.x, inset);
  let Ok(hero_right) = hero.right();
  let right = i32::wrapping_sub(hero_right, inset);
  let picker_y = i32::wrapping_add(
    hero.y,
    match bool::then_some(short, ()) {
      Some(()) => 9,
      None => 17,
    },
  );

  let Ok(()) = scope_choices(scene, Point { x: right, y: picker_y });

  let count_y = i32::wrapping_add(
    hero.y,
    match bool::then_some(stacked, ()) {
      Some(()) => 39,
      None => match bool::then_some(short, ()) {
        Some(()) => 10,
        None => 18,
      },
    },
  );
  let labels = FocusTotals {
    area: Area { x: left, y: count_y, w: hero.w.wrapping_sub(inset.wrapping_mul(2)), h: 0 },
    typography: match (short, compact) {
      (true, _) => FocusTypography::Short,
      (false, true) => FocusTypography::Compact,
      (false, false) => FocusTypography::Full,
    },
  };
  let Ok(()) = focus_totals(scene, labels, days);
  let chart_y = i32::wrapping_add(
    count_y,
    match bool::then_some(short, ()) {
      Some(()) => 87,
      None => 118,
    },
  );
  let chart = Area {
    x: left,
    y: chart_y,
    w: i32::wrapping_sub(right, left),
    h: i32::wrapping_sub(
      i32::wrapping_sub(
        {
          let Ok(explicit_value) = hero.bottom();
          explicit_value
        },
        chart_y,
      ),
      32,
    )
    .max(22),
  };
  let Ok(()) = trend(
    scene,
    chart,
    &({
      let Ok(explicit_value) = bars(app, days.start, ChartDensity::from(compact));
      explicit_value
    }),
  );

  let Ok(bottom) = hero.bottom();
  Ok(Area {
    x: left,
    y: i32::wrapping_add(bottom, 26),
    w: i32::wrapping_sub(right, left),
    h: 0,
  })
}

fn overview_art(scene: &mut Scene<'_>, hero: Area, art: &Art) -> std::result::Result<(), Never> {
  match &art.woods {
    Some(pixels) => {
      let width = i32::wrapping_div(i32::wrapping_mul(hero.w, 3), 4).min(730);
      let Ok(hero_right) = hero.right();
      scene.frame.shapes.push(Shape::Picture(Picture {
        at: Point {
          x: i32::wrapping_sub(i32::wrapping_sub(hero_right, width), 2),
          y: i32::wrapping_add(hero.y, 1),
        },
        size: Size {
          width: i32::cast_unsigned(width),
          height: i32::cast_unsigned(i32::wrapping_sub(hero.h, 3)),
        },
        pixels: pixels.clone(),
      }));
    }
    None => {}
  }

  Ok(())
}

fn scope_choices(scene: &mut Scene<'_>, anchor: Point<i32>) -> std::result::Result<(), Never> {
  for (index, (scope, text)) in (0_i32..3).zip(Scope::CHOICES) {
    let area = Area {
      x: i32::wrapping_add(i32::wrapping_sub(anchor.x, 179), i32::wrapping_mul(index, 60)),
      y: anchor.y,
      w: 58,
      h: 29,
    };
    let Ok(()) = scene.button(area, text, Action::Scope(scope), Button::from(scene.app.scope == scope));
  }

  Ok(())
}

enum FocusTypography {
  Short,
  Compact,
  Full,
}

struct FocusTotals {
  area: Area,
  typography: FocusTypography,
}

fn focus_totals(
  scene: &mut Scene<'_>,
  labels: FocusTotals,
  days: &OverviewPeriod<'_>,
) -> std::result::Result<(), Never> {
  let FocusTotals { area, typography } = labels;
  let Area { x: left, y: count_y, .. } = area;
  let colors = scene.palette;
  let Ok(()) = scene.text(
    Area {
      x: left,
      y: count_y,
      w: area.w,
      h: 23,
    },
    "Time in focus",
    14,
    Face::Medium,
    colors.quiet,
    Align::Left,
  );
  let Ok(seconds) = days.range.total(&scene.app.history);
  let Ok(count) = duration(seconds);
  let Ok(()) = scene.text(
    Area {
      x: i32::wrapping_sub(left, 2),
      y: i32::wrapping_add(count_y, 17),
      w: area.w,
      h: match typography {
        FocusTypography::Short => 50,
        FocusTypography::Compact | FocusTypography::Full => 70,
      },
    },
    &count,
    match typography {
      FocusTypography::Short => 38,
      FocusTypography::Compact => 47,
      FocusTypography::Full => 59,
    },
    Face::Title,
    colors.ink,
    Align::Left,
  );
  let dates = match bool::then_some(days.start == CURRENT_DAY, ()) {
    Some(()) => days.last.title.clone(),
    None => {
      let Ok(first) = days.first.short();
      let Ok(last) = days.last.short();
      format!("{first} – {last}")
    }
  };
  let Ok(()) = scene.text(
    Area {
      x: left,
      y: i32::wrapping_add(
          count_y,
          match typography {
            FocusTypography::Short => 59,
            FocusTypography::Compact | FocusTypography::Full => 83,
          },
        ),
      w: area.w,
      h: 22,
    },
    &dates,
    13,
    Face::Body,
    colors.quiet,
    Align::Left,
  );
  Ok(())
}

fn rank_summary(
  scene: &mut Scene<'_>,
  area: Area,
  range: Period,
  density: ChartDensity,
) -> std::result::Result<i32, Never> {
  let app = scene.app;
  let colors = scene.palette;
  let Area { x: left, y: section, .. } = area;
  let Ok(right) = area.right();
  let Ok(()) = scene.text(
    Area {
      x: left,
      y: section,
      w: i32::wrapping_sub(i32::wrapping_sub(right, left), 149),
      h: 37,
    },
    "Most used",
    22,
    Face::Title,
    colors.ink,
    Align::Left,
  );
  let Ok(()) = kind_buttons(scene, Point { x: right, y: i32::wrapping_add(section, 4) });
  let Ok(ranked) = lanes(&app.history, range, app.kind, LaneView::Overview);
  let first = i32::wrapping_add(section, 46);

  let high = match ranked.first().map(|lane| lane.total) {
    Some(high) => high,
    None => {
      let Ok(()) = empty(
        scene,
        Area {
          x: left,
          y: i32::wrapping_add(first, 10),
          w: i32::wrapping_sub(right, left),
          h: 48,
        },
        app.kind,
        EmptyPeriod::Range,
      );
      return Ok(i32::wrapping_add(first, 67));
    }
  };

  let Ok(lane_count) = fitted::<_, u32>(ranked.len());
  let count = match app.more {
    Rows::Expanded => lane_count,
    Rows::Summary => lane_count.min(5),
  };
  let Ok(visible_rows) = index(count);

  for (index, lane) in ranked.iter().take(visible_rows).enumerate() {
    let Ok(row) = fitted::<_, i32>(index);
    let y = i32::wrapping_add(first, i32::wrapping_mul(row, ROW));
    let strip = Area { x: left, y, w: area.w, h: ROW };
    let Ok(()) = ranked_lane(scene, strip, lane, RankScale { high, density });
  }

  let Ok(rows) = fitted::<_, i32>(count);
  let end = i32::wrapping_add(first, i32::wrapping_mul(rows, ROW));

  Ok(match bool::then_some(ranked.len() > 5, ()) {
    Some(()) => {
      let Ok(()) = scene.button(
        Area { x: left, y: i32::wrapping_add(end, 7), w: 93, h: 30 },
        match app.more {
          Rows::Expanded => "Show less",
          Rows::Summary => "Show all",
        },
        Action::More,
        Button::Unselected,
      );
      i32::wrapping_add(end, 39)
    }
    None => end,
  })
}

struct RankScale {
  high: u64,
  density: ChartDensity,
}

fn ranked_lane(
  scene: &mut Scene<'_>,
  strip: Area,
  lane: &Lane<'_>,
  scale: RankScale,
) -> std::result::Result<(), Never> {
  let colors = scene.palette;
  let Area { x: left, y, .. } = strip;
  let Ok(right) = strip.right();
  let RankScale { high, density } = scale;
  let label_width = i32::wrapping_div(
    i32::wrapping_mul(
      i32::wrapping_sub(i32::wrapping_sub(right, left), 72),
      match (density == ChartDensity::Compact).then_some(()) {
        Some(()) => 65,
        None => 45,
      },
    ),
    100,
  );
  let track = Area {
    x: i32::wrapping_add(i32::wrapping_add(left, label_width), 10),
    y: i32::wrapping_add(y, 17),
    w: i32::wrapping_sub(
      i32::wrapping_sub(i32::wrapping_sub(right, left), label_width),
      89,
    ),
    h: 4,
  };
  let Ok(()) = scene.text(
    Area { x: left, y, w: label_width, h: ROW },
    lane.label,
    15,
    Face::Body,
    colors.ink,
    Align::Left,
  );
  let Ok(()) = scene.panel(track, colors.lifted, 2, None);
  let bar = Area {
    x: track.x,
    y: track.y,
    w: ({
      let Ok(explicit_value) = scaled(Proportion { value: lane.total, maximum: high }, track.w);
      explicit_value
    })
    .max(2),
    h: track.h,
  };
  let Ok(()) = scene.panel(bar, colors.bars, 2, None);
  let Ok(time_spent) = duration(lane.total);
  let Ok(()) = scene.text(
    Area { x: i32::wrapping_sub(right, 69), y, w: 69, h: ROW },
    &time_spent,
    13,
    Face::Mono,
    colors.quiet,
    Align::Right,
  );
  Ok(())
}

struct Proportion {
  value: u64,
  maximum: u64,
}

#[must_use]
fn scaled(proportion: Proportion, pixels: i32) -> core::result::Result<i32, console_core_never::Never> {
  let Ok(amount) = proportion.value.float();
  let Ok(limit) = proportion.maximum.max(1).float();
  let Ok(scaled) = whole_i32(amount / limit * f64::from(pixels.max(0)));

  Ok(scaled)
}

fn trend(
  scene: &mut Scene<'_>,
  area: Area,
  bars: &[Bar],
) -> core::result::Result<(), console_core_never::Never> {
  let colors = scene.palette;
  let width = area.w.max(1);
  let Ok(bar_count) = fitted::<_, i32>(bars.len());
  let plateau = match bars.iter().map(|bar| bar.seconds).max() {
    Some(present) => present,
    None => 0,
  };
  let Ok(bottom) = area.bottom();
  let Ok(()) = scene.line(Point { x: area.x, y: bottom }, width);
  let filled = bars.iter().any(|bar| bar.seconds != 0);

  match bool::then_some(!filled, ()) {
    Some(()) => {
      let Ok(()) = scene.text(
        area,
        "Daily activity will appear as you use your apps",
        13,
        Face::Body,
        colors.quiet,
        Align::Center,
      );
    }
    None => {}
  }

  let counts = match NonZeroI32::new(bar_count) {
    Some(count) => count,
    None => return Ok(()),
  };
  let chart = TrendChart { area, width, counts, plateau };

  for (position, bar) in bars.iter().enumerate() {
    let Ok(number) = fitted::<_, u32>(position);
    let Ok(()) = trend_bar(scene, &chart, number, bar);
  }

  Ok(())
}

struct TrendChart {
  area: Area,
  width: i32,
  counts: NonZeroI32,
  plateau: u64,
}

fn trend_bar(
  scene: &mut Scene<'_>,
  chart: &TrendChart,
  number: u32,
  bar: &Bar,
) -> std::result::Result<(), Never> {
  let TrendChart { area, width, counts, plateau } = *chart;
  let colors = scene.palette;
  let Ok(bar_index) = fitted::<_, i32>(number);
  let column = area
    .x
    .wrapping_add(bar_index.wrapping_mul(width).wrapping_div(counts.get()));
  let next = area.x.wrapping_add(
    bar_index
      .wrapping_add(1)
      .wrapping_mul(width)
      .wrapping_div(counts.get()),
  );
  let space = i32::wrapping_sub(next, column);
  let action = Action::Chart {
    day: bar.day,
    slot: number,
  };
  let hover = scene.app.hover == Some(action);
  let focus = scene.app.focus == Some(action);
  let is_today = bar.day == CURRENT_DAY && scene.app.scope != Scope::Today;
  let ratio = Proportion { value: bar.seconds, maximum: plateau };
  let Ok(scaled_height) = scaled(ratio, i32::wrapping_sub(area.h, 10));
  let height = scaled_height.max(2);
  let bar_width = i32::wrapping_div(i32::wrapping_mul(space, 54), 100).clamp(3, 69);
  let x = i32::wrapping_add(
    column,
    i32::wrapping_div(i32::wrapping_sub(space, bar_width), 2),
  );
  let ink = match (bar.seconds, hover || focus || is_today) {
    (0, _) => colors.rule,
    (_, true) => colors.accent,
    _ => colors.bars,
  };
  let Ok(bottom) = area.bottom();
  let Ok(()) = scene.panel(
    Area { x, y: i32::wrapping_sub(bottom, height), w: bar_width, h: height },
    ink,
    5,
    None,
  );

  match bool::then_some(focus, ()) {
    Some(()) => {
      let Ok(area_bottom) = area.bottom();
      let Ok(()) = scene.panel(
        Area { x, y: i32::wrapping_add(area_bottom, 3), w: bar_width, h: 1 },
        colors.accent,
        0,
        None,
      );
    }
    None => {}
  }

  let (label_x, label_width, align) = match counts.get().cmp(&8) {
    Ordering::Greater => (
      i32::wrapping_add(column, 3),
      43.min(
        ({
          let Ok(explicit_value) = area.right();
          explicit_value
        })
        .saturating_sub(column),
      ),
      Align::Left,
    ),
    Ordering::Less | Ordering::Equal => (column, space, Align::Center),
  };
  let Ok(area_bottom) = area.bottom();
  let Ok(()) = scene.text(
    Area { x: label_x, y: i32::wrapping_add(area_bottom, 7), w: label_width, h: 22 },
    &bar.label,
    12,
    Face::Body,
    colors.quiet,
    align,
  );
  let Ok(()) = scene.hit(
    Area { x: column, y: area.y, w: space, h: i32::wrapping_add(area.h, 27) },
    action,
  );

  match ((hover || focus) && width >= 510).then_some(()) {
    Some(()) => {
      let Ok(time_spent) = duration(bar.seconds);
      let detail = format!("{}  ·  {time_spent}", bar.title);
      let Ok(()) = scene.text(
        Area { x: area.x, y: i32::wrapping_sub(area.y, 30), w: width, h: 22 },
        &detail,
        12,
        Face::Mono,
        colors.ink,
        Align::Right,
      );
    }
    None => {}
  }

  Ok(())
}

enum EmptyPeriod {
  Day,
  Range,
}

fn empty(
  scene: &mut Scene<'_>,
  area: Area,
  kind: Kind,
  period: EmptyPeriod,
) -> std::result::Result<(), Never> {
  let title = match (kind, period) {
    (Kind::Apps, EmptyPeriod::Day) => "No foreground time on this day",
    (Kind::Apps, EmptyPeriod::Range) => "No foreground time in this range yet",
    (Kind::Websites, EmptyPeriod::Day) => "No websites tracked on this day",
    (Kind::Websites, EmptyPeriod::Range) => "No websites tracked in this range",
  };
  let Ok(()) = scene.text(
    Area { x: area.x, y: area.y, w: area.w, h: 24 },
    title,
    15,
    Face::Body,
    scene.palette.ink,
    Align::Left,
  );

  Ok(match bool::then_some(kind == Kind::Websites, ()) {
    Some(()) => {
      let Ok(()) = scene.text(
        Area { x: area.x, y: i32::wrapping_add(area.y, 24), w: area.w, h: 24 },
        "Website tracking uses the Habits Companion extension.",
        12,
        Face::Body,
        scene.palette.quiet,
        Align::Left,
      );
    }
    None => {}
  })
}

fn timeline(
  scene: &mut Scene<'_>,
  column: Area,
) -> core::result::Result<i32, console_core_never::Never> {
  let app = scene.app;
  let Ok(position) = index(app.day);
  let day = match app.calendar.get(position) {
    Some(day) => day,
    None => return Ok(column.y),
  };
  let Ok(range) = period(day);
  let Ok(panel_y) = timeline_heading(scene, column, day, range);
  timeline_panel(scene, Area { y: panel_y, ..column }, range)
}

fn timeline_heading(
  scene: &mut Scene<'_>,
  column: Area,
  day: &Day,
  range: Period,
) -> std::result::Result<i32, Never> {
  let app = scene.app;
  let colors = scene.palette;
  let compact = column.w < 540;
  let short = column.h < 370;
  let heading = Area {
    x: i32::wrapping_add(column.x, 5),
    y: i32::wrapping_add(
      column.y,
      match bool::then_some(short, ()) {
        Some(()) => 1,
        None => 10,
      },
    ),
    w: i32::wrapping_sub(column.w, 10),
    h: match bool::then_some(short, ()) {
      Some(()) => 35,
      None => 46,
    },
  };
  let arrow_y = i32::wrapping_add(
    heading.y,
    match bool::then_some(short, ()) {
      Some(()) => 1,
      None => 7,
    },
  );
  let area = Area {
    x: heading.x,
    y: heading.y,
    w: i32::wrapping_sub(heading.w, 90),
    h: heading.h,
  };
  let Ok(()) = scene.text(
    area,
    &day.title,
    match bool::then_some(short, ()) {
      Some(()) => 23,
      None => match bool::then_some(compact, ()) {
        Some(()) => 29,
        None => 38,
      },
    },
    Face::Title,
    colors.ink,
    Align::Left,
  );
  let Ok(seconds) = range.total(&app.history);
  let Ok(time_spent) = duration(seconds);
  let total = format!("{time_spent} in focus");
  let Ok(()) = scene.text(
    Area {
      x: heading.x,
      y: {
          let Ok(explicit_value) = heading.bottom();
          explicit_value
        },
      w: heading.w,
      h: 23,
    },
    &total,
    14,
    Face::Body,
    colors.quiet,
    Align::Left,
  );
  let Ok(()) = timeline_day_buttons(scene, heading, arrow_y);

  let Ok(bottom) = heading.bottom();
  let panel_y = i32::wrapping_add(
    bottom,
    match bool::then_some(short, ()) {
      Some(()) => 27,
      None => 41,
    },
  );
  Ok(panel_y)
}

struct DayButton {
  label: &'static str,
  step: i32,
  distance: i32,
  available: Button,
}

fn timeline_day_buttons(
  scene: &mut Scene<'_>,
  heading: Area,
  y: i32,
) -> std::result::Result<(), Never> {
  let Ok(right) = heading.right();
  let previous = match (scene.app.day > 0).then_some(()) {
    Some(()) => Button::Unselected,
    None => Button::Unavailable,
  };
  let next = match (scene.app.day < CURRENT_DAY).then_some(()) {
    Some(()) => Button::Unselected,
    None => Button::Unavailable,
  };

  for arrow in [
    DayButton { label: "‹", step: -1, distance: 81, available: previous },
    DayButton { label: "›", step: 1, distance: 37, available: next },
  ] {
    let area = Area { x: right.wrapping_sub(arrow.distance), y, w: 36, h: 32 };
    let Ok(()) = scene.button(area, arrow.label, Action::Step(arrow.step), arrow.available);
  }

  Ok(())
}

fn timeline_panel(
  scene: &mut Scene<'_>,
  column: Area,
  range: Period,
) -> std::result::Result<i32, Never> {
  let app = scene.app;
  let colors = scene.palette;
  let panel_y = column.y;
  let short = column.h < 370;
  let left = i32::wrapping_add(column.x, 22);
  let Ok(column_right) = column.right();
  let right = i32::wrapping_sub(column_right, 22);
  let Ok(lanes) = lanes(&app.history, range, app.kind, LaneView::Timeline);
  let row_top = i32::wrapping_add(
    panel_y,
    match bool::then_some(short, ()) {
      Some(()) => 65,
      None => 86,
    },
  );
  let row_height = match bool::then_some(short, ()) {
    Some(()) => 30,
    None => 37,
  };
  let Ok(row_count) = fitted::<_, i32>(lanes.len());
  let plot_end = row_top.wrapping_add(row_count.wrapping_mul(row_height).max(38));
  let end = i32::wrapping_add(
    plot_end,
    match bool::then_some(short, ()) {
      Some(()) => 40,
      None => 51,
    },
  );
  let panel = Area {
    x: column.x,
    y: panel_y,
    w: column.w,
    h: i32::wrapping_sub(end, panel_y),
  };
  let Ok(()) = scene.panel(panel, colors.surface, 13, Some(colors.rule));
  let panel_header = i32::wrapping_add(
    panel_y,
    match bool::then_some(short, ()) {
      Some(()) => 6,
      None => 14,
    },
  );
  let Ok(()) = scene.text(
    Area {
      x: left,
      y: panel_header,
      w: i32::wrapping_sub(i32::wrapping_sub(right, left), 150),
      h: 32,
    },
    "Switch timeline",
    15,
    Face::Medium,
    colors.ink,
    Align::Left,
  );
  let Ok(()) = kind_buttons(scene, Point { x: right, y: i32::wrapping_add(panel_header, 1) });

  match bool::then_some(lanes.is_empty(), ()) {
    Some(()) => {
      let Ok(()) = empty(
        scene,
        Area {
          x: left,
          y: i32::wrapping_add(panel_header, 40),
          w: i32::wrapping_sub(right, left),
          h: 50,
        },
        app.kind,
        EmptyPeriod::Day,
      );
      return Ok(end);
    }
    None => {}
  }


  let rows = Area {
    x: left,
    y: row_top,
    w: i32::wrapping_sub(right, left),
    h: i32::wrapping_sub(plot_end, row_top),
  };
  let Ok(()) = timeline_tracks(scene, rows, row_height, range, &lanes,
    ChartDensity::from(column.w < 540));
  Ok(end)
}

fn timeline_tracks(
  scene: &mut Scene<'_>,
  rows: Area,
  row_height: i32,
  range: Period,
  lanes: &[Lane<'_>],
  density: ChartDensity,
) -> std::result::Result<(), Never> {
  let left = rows.x;
  let row_top = rows.y;
  let colors = scene.palette;
  let label_width =
    i32::wrapping_div(i32::wrapping_mul(rows.w, 22), 100).clamp(77, 165);
  let plot = Area {
    x: i32::wrapping_add(i32::wrapping_add(left, label_width), 10),
    y: row_top,
    w: i32::wrapping_sub(
      i32::wrapping_sub(rows.w, label_width),
      10,
    ),
    h: rows.h,
  };
  let Ok(range_seconds) = range.seconds();
  let grid = TimelineGrid { plot, range, range_seconds };
  let Ok(()) = timeline_ticks(scene, &grid, density);
  let mut detail = None;

  for (position, lane) in lanes.iter().enumerate() {
    let Ok(number) = fitted::<_, u32>(position);
    let Ok(row) = fitted::<_, i32>(position);
    let y = row_top.wrapping_add(row.wrapping_mul(row_height));
    let layout = TimelineLane {
      area: Area { x: left, y, w: rows.w, h: row_height },
      label_width,
      number,
    };
    let Ok(selected) = timeline_lane(scene, &grid, layout, lane);

    match selected {
      Some(text) => detail = Some(text),
      None => {}
    }
  }

  let Ok(plot_end) = rows.bottom();
  let Ok(()) = scene.line(Point { x: left, y: plot_end }, rows.w);
  let hint = match detail.as_deref() {
    Some(hint) => hint,
    None => "Sampled switches are approximate. Select a segment for its time.",
  };
  let ink = match detail {
    Some(_) => colors.ink,
    None => colors.quiet,
  };
  let Ok(()) = scene.text(
    Area {
      x: left,
      y: i32::wrapping_add(plot_end, 10),
      w: rows.w,
      h: 30,
    },
    hint,
    12,
    Face::Body,
    ink,
    Align::Left,
  );

  Ok(())
}

struct TimelineGrid {
  plot: Area,
  range: Period,
  range_seconds: u64,
}

impl TimelineGrid {
  fn x(&self, time: u64) -> std::result::Result<i32, Never> {
    let amount = Proportion {
      value: time.saturating_sub(self.range.start),
      maximum: self.range_seconds,
    };
    let Ok(offset) = scaled(amount, self.plot.w);
    Ok(self.plot.x.wrapping_add(offset))
  }
}

fn timeline_ticks(
  scene: &mut Scene<'_>,
  grid: &TimelineGrid,
  density: ChartDensity,
) -> std::result::Result<(), Never> {
  let plot = grid.plot;
  let row_top = plot.y;
  let colors = scene.palette;

  for (position, (unix, label)) in scene.app.ticks.iter().enumerate() {
    let Ok(index) = fitted::<_, u32>(position);

    match bool::then_some(density == ChartDensity::Compact && index.wrapping_rem(2) == 1, ()) {
      Some(()) => {
        continue;
      }
      None => {}
    }

    let Ok(x) = grid.x(*unix);
    let Ok(()) = scene.text(
      Area {
        x: i32::wrapping_sub(
            x,
            match index {
              0 => 0,
              _ => 16,
            },
          ),
        y: i32::wrapping_sub(row_top, 24),
        w: 42,
        h: 18,
      },
      label,
      11,
      Face::Mono,
      colors.quiet,
      Align::Left,
    );

    match bool::then_some(index > 0, ()) {
      Some(()) => {
        let Ok(()) = scene.panel(
          Area { x, y: row_top, w: 1, h: plot.h },
          colors.rule,
          0,
          None,
        );
      }
      None => {}
    }
  }

  Ok(())
}

struct TimelineLane {
  area: Area,
  label_width: i32,
  number: u32,
}

fn timeline_lane(
  scene: &mut Scene<'_>,
  grid: &TimelineGrid,
  layout: TimelineLane,
  lane: &Lane<'_>,
) -> std::result::Result<Option<String>, Never> {
  let TimelineLane { area, label_width, number } = layout;
  let Area { x: left, y, h: row_height, .. } = area;
  let app = scene.app;
  let colors = scene.palette;
  let plot = grid.plot;
  let mut detail = None;
  let Ok(()) = scene.line(Point { x: left, y }, area.w);
  let Ok(()) = scene.text(
    Area { x: left, y, w: label_width, h: row_height },
    lane.label,
    13,
    Face::Body,
    colors.ink,
    Align::Left,
  );
  let Ok(pigment) = lane_color(lane.label, colors);

  for item in &lane.periods {
    let action = Action::Segment {
      lane: number,
      period: *item,
    };
    let hot = app.hover == Some(action) || app.focus == Some(action);
    let selected = app.details.as_ref().filter(|(period, _)| period == item);

    match selected {
      Some((_, hours)) => {
        detail = Some(format!("{}  ·  {hours}", lane.label));
      }
      None => {}
    }

    let Ok(x) = grid.x(item.start);
    let Ok(end) = grid.x(item.end);
    let width = i32::wrapping_sub(end, x).max(3);
    let Ok(plot_right) = plot.right();
    let chip = Area {
      x: x.min(i32::wrapping_sub(plot_right, width)),
      y: i32::wrapping_add(y, i32::wrapping_div(i32::wrapping_sub(row_height, 13), 2)),
      w: width,
      h: 13,
    };
    let ink = match bool::then_some(hot || selected.is_some(), ()) {
      Some(()) => colors.ink,
      None => pigment,
    };
    let Ok(()) = scene.panel(chip, ink, 4, None);
    let hit = Area {
      x: chip.x,
      y: i32::wrapping_add(y, 2),
      w: chip.w.max(8),
      h: i32::wrapping_sub(row_height, 4),
    };

    {
      let Ok(inside) = hit.inside(Area { x: plot.x, y, w: plot.w, h: row_height });

      match inside {
        Some(hit_2) => {
          let Ok(()) = scene.hit(hit_2, action);
        }
        None => {}
      };
    }
  }

  Ok(detail)
}

fn lane_color(name: &str, colors: Palette) -> std::result::Result<Oklch, Never> {
  let hash = name
    .bytes()
    .fold(0_u8, |hash, byte| hash.wrapping_mul(31).wrapping_add(byte));
  let slot = hash.wrapping_rem(5);
  let Ok(position) = index(slot);
  Ok(match colors.lanes.get(position) {
    Some(color) => *color,
    None => colors.accent,
  })
}
