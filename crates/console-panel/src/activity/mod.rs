//! Draw an activity dashboard and its pointer targets from the same layout.

mod data;
mod theme;

use std::cmp::Ordering;
use std::num::NonZeroI32;

use console_core_color::{Oklch, Rgba, hexcode};
use console_core_geometry::{Point, Size};
use console_core_jpeg_files::JpegError;
use console_core_never::Never;
use console_core_number_conversion::{
  Float, checked_index, fitted, index, toward_zero_u8, whole_i32,
};
use console_core_shapes::{Clip, Picture, Pixels, Shape};
use console_draw_surface::Keysym;

use crate::canvas::{
  Align, Area, AreaLayout, Button, Canvas, Face, FocusDirection, Interaction, Style,
};
use crate::strip;

pub use data::{
  Action, Day, Kind, Lane, LaneView, Page, Period, Recorder, Refresh, Rows, Scope, Source, View,
  duration,
};
use theme::Palette;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const FOOTER: i32 = 36;
const ROW: i32 = 38;

pub const WINDOW_SIZE: Size<u32> = Size {
  width: 1080,
  height: 760,
};

pub enum Press {
  Activate(Action),
  MoveWindow,
  Ignore,
}

pub fn press(frame: &Frame, at: Point<f64>) -> std::result::Result<Press, Never> {
  let Ok(action) = frame.targets.action(at);
  Ok(match action {
    Some(requested) => Press::Activate(requested),
    None => match (0.0..f64::from(strip::CONTENT_TOP))
      .contains(&at.y)
      .then_some(())
    {
      Some(()) => Press::MoveWindow,
      None => Press::Ignore,
    },
  })
}

pub enum KeyAction {
  Activate(Action),
  Focus(FocusDirection),
  Scroll(i32),
  Ignore,
}

pub fn keyboard(
  key: Keysym,
  page: Page,
  focus: Option<Action>,
) -> std::result::Result<KeyAction, Never> {
  use KeyAction::{Activate, Focus, Ignore, Scroll};

  Ok(match (key, page) {
    (Keysym::Escape | Keysym::q, _) => Activate(Action::Close),
    (Keysym::o, _) => Activate(Action::Page(Page::Overview)),
    (Keysym::t, _) => Activate(Action::Page(Page::Timeline)),
    (Keysym::_1, _) => Activate(Action::Scope(Scope::Today)),
    (Keysym::_7, _) => Activate(Action::Scope(Scope::Week)),
    (Keysym::_3, _) => Activate(Action::Scope(Scope::Month)),
    (Keysym::Left, Page::Timeline) => Activate(Action::Step(-1)),
    (Keysym::Right, Page::Timeline) => Activate(Action::Step(1)),
    (Keysym::Return | Keysym::KP_Enter | Keysym::space, _) => match focus {
      Some(action) => Activate(action),
      None => Ignore,
    },
    (Keysym::Tab, _) => Focus(FocusDirection::Next),
    (Keysym::ISO_Left_Tab, _) => Focus(FocusDirection::Previous),
    (Keysym::Down, _) => Scroll(45),
    (Keysym::Up, _) => Scroll(-45),
    (Keysym::Page_Down, _) => Scroll(270),
    (Keysym::Page_Up, _) => Scroll(-270),
    _ => Ignore,
  })
}

pub struct Art<'a> {
  source: &'a [u8],
  palette: Palette,
  woods: Option<Pixels>,
  palette_error: Option<String>,
}

impl<'a> Art<'a> {
  pub fn new(source: &'a [u8]) -> Result<Self> {
    let mut art = Self {
      source,
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
          let Ok(explicit_value) = picture(art.source, art.palette);
          explicit_value
        }
      }
      Ok(Refresh::Changed) => {}
    }

    Ok(art)
  }

  pub fn refresh(&mut self) -> std::result::Result<Refresh, Never> {
    let current = match Palette::current() {
      Ok(current) => {
        self.palette_error = None;
        current
      }
      Err(fault) => {
        let message = fault.to_string();

        match bool::then_some(self.palette_error.as_ref() != Some(&message), ()) {
          Some(()) => {
            console_core_logging::warn!("Activity dashboard palette: {message}");
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
      let Ok(explicit_value) = picture(self.source, palette);
      explicit_value
    };
    Ok(Refresh::Changed)
  }
}

#[must_use]
fn picture(
  source: &[u8],
  palette: Palette,
) -> core::result::Result<Option<Pixels>, console_core_never::Never> {
  Ok(match tinted_art(source, palette) {
    Ok(pixels) => Some(pixels),
    Err(fault) => {
      console_core_logging::warn!("Activity dashboard artwork: {fault}");
      None
    }
  })
}

fn channels(color: Oklch) -> std::result::Result<[u8; 3], Never> {
  let Ok(code) = hexcode(color);
  let rgba = match Rgba::of(&code) {
    Ok(rgba) => rgba,
    Err(fault) => {
      console_core_logging::warn!("Activity dashboard artwork color: {fault}");
      Rgba::BLACK
    }
  };
  Ok([rgba.red, rgba.green, rgba.blue])
}

fn tinted_art(source: &[u8], palette: Palette) -> std::result::Result<Pixels, JpegError> {
  let mut art = console_core_jpeg_files::decoded_as_stored(source)?;
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

pub type Frame = crate::canvas::Frame<Action>;

struct Scene<'a> {
  canvas: Canvas<Action>,
  app: &'a View<'a>,
  palette: Palette,
}

impl<'a> Scene<'a> {
  fn new(app: &'a View<'a>, window: Area, colors: Palette) -> std::result::Result<Self, Never> {
    let style = Style::from(&colors.chrome);
    let interaction = Interaction {
      focus: app.focus,
      hover: app.hover,
    };
    let Ok(mut canvas) = Canvas::new(window, style, interaction);
    let Ok(()) = canvas.panel(window, colors.ground, 0, None);
    Ok(Self {
      canvas,
      app,
      palette: colors,
    })
  }
}

#[must_use]
pub fn draw(
  app: &View<'_>,
  room: Size<u32>,
  art: &Art<'_>,
) -> core::result::Result<Frame, console_core_never::Never> {
  let width = u32::cast_signed(room.width);
  let height = u32::cast_signed(room.height);
  let whole = Area {
    origin: Point { x: 0, y: 0 },
    size: Size { width, height },
  };
  let colors = art.palette;
  let Ok(mut scene) = Scene::new(app, whole, colors);
  let Ok(chrome) = strip::window_area(width);
  let Ok(()) = header(&mut scene, chrome);

  match bool::then_some(width < 300 || height < 155, ()) {
    Some(()) => {
      let below_header = Area {
        origin: Point {
          x: 0,
          y: strip::CONTENT_TOP,
        },
        size: Size {
          width,
          height: height.saturating_sub(strip::CONTENT_TOP),
        },
      };
      let Ok(()) = scene.canvas.text(
        below_header,
        "Enlarge the window to see activity",
        15,
        Face::Body,
        colors.quiet,
        Align::Center,
      );
      return Ok(scene.canvas.frame);
    }
    None => {}
  }

  let padding = match width.cmp(&640) {
    Ordering::Less => 17,
    Ordering::Equal | Ordering::Greater => 36,
  };
  let content = i32::wrapping_sub(width, i32::wrapping_mul(2, padding)).min(1070);
  let x = i32::wrapping_div(i32::wrapping_sub(width, content), 2);
  let top = strip::CONTENT_TOP;
  let page = Area {
    origin: Point { x, y: top },
    size: Size {
      width: content,
      height: i32::wrapping_sub(i32::wrapping_sub(height, top), FOOTER),
    },
  };
  let Ok(end) = draw_page(&mut scene, page, whole, art);
  let overflow = i32::wrapping_sub(i32::wrapping_add(i32::wrapping_add(end, app.scroll), 12), {
    let Ok(explicit_value) = page.bottom();
    explicit_value
  })
  .max(0);
  scene.canvas.frame.max_scroll = overflow;
  scene.canvas.frame.shapes.push(Shape::Clip(Clip::Lifted));

  match bool::then_some(overflow > 0, ()) {
    Some(()) => {
      let rail = page.size.height.max(1);
      let thumb = match rail
        .saturating_mul(rail)
        .checked_div(rail.saturating_add(overflow))
      {
        Some(length) => length.clamp(18.min(rail), rail),
        None => rail,
      };
      let travel = rail.saturating_sub(thumb);
      let scroll = travel.saturating_mul(app.scroll.min(overflow));
      let offset = match scroll.checked_div(overflow) {
        Some(offset) => offset,
        None => 0,
      };
      let y = page.origin.y.saturating_add(offset);
      let Ok(()) = scene.canvas.panel(
        Area {
          origin: Point {
            x: i32::wrapping_sub(width, 7),
            y,
          },
          size: Size {
            width: 2,
            height: thumb,
          },
        },
        colors.rule,
        1,
        None,
      );
    }
    None => {}
  }

  let Ok(()) = footer(
    &mut scene,
    Area {
      origin: Point {
        x,
        y: height.saturating_sub(FOOTER),
      },
      size: Size {
        width: content,
        height: FOOTER,
      },
    },
  );

  Ok(scene.canvas.frame)
}

fn draw_page(
  scene: &mut Scene<'_>,
  page: Area,
  window: Area,
  art: &Art<'_>,
) -> std::result::Result<i32, Never> {
  let Ok(()) = scene.canvas.clip(Area {
    origin: Point {
      x: window.origin.x,
      y: page.origin.y,
    },
    size: Size {
      width: window.size.width,
      height: page.size.height,
    },
  });
  let lead = match window.size.height.cmp(&500) {
    Ordering::Less => 8,
    Ordering::Equal | Ordering::Greater => i32::wrapping_add(
      15,
      i32::wrapping_div(i32::wrapping_sub(window.size.height, 900), 8).clamp(0, 30),
    ),
  };
  let column = Area {
    origin: Point {
      y: i32::wrapping_sub(i32::wrapping_add(page.origin.y, lead), scene.app.scroll),
      ..page.origin
    },
    size: Size {
      height: i32::wrapping_sub(page.size.height, lead),
      ..page.size
    },
  };

  match scene.app.page {
    Page::Overview => overview(scene, column, art),
    Page::Timeline => timeline(scene, column),
  }
}

fn header(scene: &mut Scene<'_>, area: Area) -> std::result::Result<(), Never> {
  let selected = match scene.app.page {
    Page::Overview => 0,
    Page::Timeline => 1,
  };

  strip::header(
    &mut scene.canvas,
    area,
    &strip::Header {
      title: scene.app.title,
      tabs: &["Overview", "Timeline"],
      selected,
      select: |index| match index {
        0 => Action::Page(Page::Overview),
        _ => Action::Page(Page::Timeline),
      },
      previous: Action::PageArrow(Page::Overview),
      next: Action::PageArrow(Page::Timeline),
      close: Action::Close,
    },
    &scene.palette.chrome,
  )
}

fn footer(scene: &mut Scene<'_>, area: Area) -> std::result::Result<(), Never> {
  let colors = scene.palette;
  let Area {
    origin: Point { x, .. },
    size: Size { width, .. },
  } = area;
  let Ok(()) = scene.canvas.line(
    Point {
      x: area.origin.x,
      y: area.origin.y,
    },
    area.size.width,
  );
  let (status, interval) = match scene.app.recorder {
    Recorder::Stale { interval } => ("Recorder hasn’t updated", interval),
    Recorder::Current { status, interval } => {
      let message = match status {
        "No tracked window in focus" => "Paused while viewing",
        other => other,
      };
      (message, interval)
    }
    Recorder::Waiting { interval } => ("Waiting for recorder", interval),
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
  let Ok(()) = scene.canvas.text(
    Area {
      origin: Point {
        x,
        y: area.origin.y,
      },
      size: Size {
        width: half,
        height: area.size.height,
      },
    },
    &label,
    12,
    Face::Body,
    colors.quiet,
    Align::Left,
  );
  let Ok(()) = scene.canvas.text(
    Area {
      origin: Point {
        x: i32::wrapping_add(x, half),
        y: area.origin.y,
      },
      size: Size {
        width: i32::wrapping_sub(width, half),
        height: area.size.height,
      },
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
        origin: Point {
          x: i32::wrapping_add(i32::wrapping_sub(right, 147), i32::wrapping_mul(74, index)),
          y,
        },
        size: Size {
          width: 73,
          height: 30,
        },
      };
      let Ok(()) = scene.canvas.button(
        area,
        text,
        Action::Kind(kind),
        Button::from(scene.app.kind == kind),
      );
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
  app: &View<'_>,
  start: u32,
  density: ChartDensity,
) -> core::result::Result<Vec<Bar>, console_core_never::Never> {
  let Ok(today) = app.source.day(app.current_day);
  let last_day = match today {
    Some(day) => day,
    None => return Ok(Vec::new()),
  };

  match bool::then_some(app.scope == Scope::Today, ()) {
    Some(()) => {
      let ticks = app.today_ticks;
      return Ok(
        ticks
          .iter()
          .zip(
            ticks
              .iter()
              .skip(1)
              .map(|(seconds, _)| *seconds)
              .chain([last_day.period.end]),
          )
          .enumerate()
          .map(|(index, ((start_2, label), end))| {
            let Ok(position) = fitted::<_, u32>(index);
            let skip = density == ChartDensity::Compact && position.wrapping_rem(2) == 1;
            let label_2 = match skip.then_some(()) {
              Some(()) => String::new(),
              None => label.clone(),
            };
            let period = Period {
              start: *start_2,
              end,
            };
            let Ok(seconds) = app.source.total(period);
            Bar {
              day: app.current_day,
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

  Ok(
    (start..=app.current_day)
      .filter_map(|day_number| {
        let Ok(available) = app.source.day(day_number);
        let day = available?;
        let label = match app.scope {
          Scope::Week => day.weekday.to_owned(),
          Scope::Month => match day_number.wrapping_sub(start).wrapping_rem(7) {
            0 => {
              let Ok(label) = day.short();
              label
            }
            _ => String::new(),
          },
          Scope::Today => String::new(),
        };
        let Ok(seconds) = app.source.total(day.period);
        Some(Bar {
          day: day_number,
          seconds,
          title: day.title.to_owned(),
          label,
        })
      })
      .collect(),
  )
}

fn overview(
  scene: &mut Scene<'_>,
  column: Area,
  art: &Art<'_>,
) -> core::result::Result<i32, console_core_never::Never> {
  let app = scene.app;
  let Ok(day_count) = app.scope.days();
  let start = app.current_day.wrapping_add(1).saturating_sub(day_count);
  let Ok(earliest) = app.source.day(start);
  let Ok(latest) = app.source.day(app.current_day);
  let (first_day, today_day) = match (earliest, latest) {
    (Some(first), Some(today)) => (first, today),
    (None, _) | (Some(_), None) => return Ok(column.origin.y),
  };
  let range = Period {
    start: first_day.period.start,
    end: today_day.period.end,
  };
  let days = OverviewPeriod {
    start,
    first: first_day,
    last: today_day,
    range,
  };
  let Ok(ranked_area) = overview_summary(scene, column, art, &days);
  rank_summary(
    scene,
    ranked_area,
    range,
    ChartDensity::from(column.size.width < 640),
  )
}

struct OverviewPeriod<'a> {
  start: u32,
  first: Day<'a>,
  last: Day<'a>,
  range: Period,
}

fn overview_summary(
  scene: &mut Scene<'_>,
  column: Area,
  art: &Art<'_>,
  days: &OverviewPeriod<'_>,
) -> std::result::Result<Area, Never> {
  let colors = scene.palette;
  let app = scene.app;
  let compact = column.size.width < 640;
  let short = column.size.height < 370;
  let stacked = column.size.width < 352;
  let hero_height = match bool::then_some(short, ()) {
    Some(()) => column.size.height.clamp(182, 245),
    None => match bool::then_some(stacked, ()) {
      Some(()) => 326,
      None => 319,
    },
  };
  let hero = Area {
    origin: column.origin,
    size: Size {
      width: column.size.width,
      height: hero_height,
    },
  };
  let Ok(()) = scene
    .canvas
    .panel(hero, colors.surface, 13, Some(colors.rule));

  let Ok(()) = overview_art(scene, hero, art);

  let inset = match bool::then_some(compact, ()) {
    Some(()) => 18,
    None => 29,
  };
  let left = i32::wrapping_add(hero.origin.x, inset);
  let Ok(hero_right) = hero.right();
  let right = i32::wrapping_sub(hero_right, inset);
  let picker_y = i32::wrapping_add(
    hero.origin.y,
    match bool::then_some(short, ()) {
      Some(()) => 9,
      None => 17,
    },
  );

  let Ok(()) = scope_choices(
    scene,
    Point {
      x: right,
      y: picker_y,
    },
  );

  let typography = match (short, compact) {
    (true, _) => FocusTypography::Short,
    (false, true) => FocusTypography::Compact,
    (false, false) => FocusTypography::Full,
  };
  let Ok(labels) = focus_layout(hero, inset, typography);
  let count_y = labels.area.origin.y;
  let Ok(()) = focus_totals(scene, labels, days);
  let chart_y = i32::wrapping_add(
    count_y,
    match bool::then_some(short, ()) {
      Some(()) => 87,
      None => 118,
    },
  );
  let Ok(bottom) = hero.bottom();
  let chart = Area {
    origin: Point {
      x: left,
      y: chart_y,
    },
    size: Size {
      width: i32::wrapping_sub(right, left),
      height: bottom.wrapping_sub(chart_y).wrapping_sub(32).max(22),
    },
  };
  let Ok(series) = bars(app, days.start, ChartDensity::from(compact));
  let Ok(()) = trend(scene, chart, &series);

  Ok(Area {
    origin: Point {
      x: left,
      y: i32::wrapping_add(bottom, 26),
    },
    size: Size {
      width: i32::wrapping_sub(right, left),
      height: 0,
    },
  })
}

fn overview_art(
  scene: &mut Scene<'_>,
  hero: Area,
  art: &Art<'_>,
) -> std::result::Result<(), Never> {
  match &art.woods {
    Some(pixels) => {
      let width = i32::wrapping_div(i32::wrapping_mul(hero.size.width, 3), 4).min(730);
      let Ok(hero_right) = hero.right();
      let Ok(picture) = Picture::rectangular(
        Point {
          x: i32::wrapping_sub(i32::wrapping_sub(hero_right, width), 2),
          y: i32::wrapping_add(hero.origin.y, 1),
        },
        Size {
          width: i32::cast_unsigned(width),
          height: i32::cast_unsigned(i32::wrapping_sub(hero.size.height, 3)),
        },
        pixels.clone(),
      );
      scene.canvas.frame.shapes.push(Shape::Picture(picture));
    }
    None => {}
  }

  Ok(())
}

fn scope_choices(scene: &mut Scene<'_>, anchor: Point<i32>) -> std::result::Result<(), Never> {
  for (index, (scope, text)) in (0_i32..3).zip(Scope::CHOICES) {
    let area = Area {
      origin: Point {
        x: i32::wrapping_add(
          i32::wrapping_sub(anchor.x, 179),
          i32::wrapping_mul(index, 60),
        ),
        y: anchor.y,
      },
      size: Size {
        width: 58,
        height: 29,
      },
    };
    let Ok(()) = scene.canvas.button(
      area,
      text,
      Action::Scope(scope),
      Button::from(scene.app.scope == scope),
    );
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

fn focus_layout(
  hero: Area,
  inset: i32,
  typography: FocusTypography,
) -> std::result::Result<FocusTotals, Never> {
  let offset = match (hero.size.width < 352, &typography) {
    (true, _) => 39,
    (false, FocusTypography::Short) => 10,
    (false, FocusTypography::Compact | FocusTypography::Full) => 18,
  };
  Ok(FocusTotals {
    area: Area {
      origin: Point {
        x: i32::wrapping_add(hero.origin.x, inset),
        y: i32::wrapping_add(hero.origin.y, offset),
      },
      size: Size {
        width: hero.size.width.wrapping_sub(inset.wrapping_mul(2)),
        height: 0,
      },
    },
    typography,
  })
}

fn focus_totals(
  scene: &mut Scene<'_>,
  labels: FocusTotals,
  days: &OverviewPeriod<'_>,
) -> std::result::Result<(), Never> {
  let FocusTotals { area, typography } = labels;
  let Point {
    x: left,
    y: count_y,
  } = area.origin;
  let colors = scene.palette;
  let Ok(()) = scene.canvas.text(
    Area {
      origin: area.origin,
      size: Size {
        width: area.size.width,
        height: 23,
      },
    },
    "Time in focus",
    14,
    Face::Medium,
    colors.quiet,
    Align::Left,
  );
  let Ok(seconds) = scene.app.source.total(days.range);
  let Ok(count) = duration(seconds);
  let Ok(()) = scene.canvas.text(
    Area {
      origin: Point {
        x: i32::wrapping_sub(left, 2),
        y: i32::wrapping_add(count_y, 17),
      },
      size: Size {
        width: area.size.width,
        height: match typography {
          FocusTypography::Short => 50,
          FocusTypography::Compact | FocusTypography::Full => 70,
        },
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
  let dates = match bool::then_some(days.start == scene.app.current_day, ()) {
    Some(()) => days.last.title.to_owned(),
    None => {
      let Ok(first) = days.first.short();
      let Ok(last) = days.last.short();
      format!("{first} – {last}")
    }
  };
  let Ok(()) = scene.canvas.text(
    Area {
      origin: Point {
        x: left,
        y: i32::wrapping_add(
          count_y,
          match typography {
            FocusTypography::Short => 59,
            FocusTypography::Compact | FocusTypography::Full => 83,
          },
        ),
      },
      size: Size {
        width: area.size.width,
        height: 22,
      },
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
  let Area {
    origin: Point {
      x: left,
      y: section,
    },
    ..
  } = area;
  let Ok(right) = area.right();
  let Ok(()) = scene.canvas.text(
    Area {
      origin: Point {
        x: left,
        y: section,
      },
      size: Size {
        width: i32::wrapping_sub(i32::wrapping_sub(right, left), 149),
        height: 37,
      },
    },
    "Most used",
    22,
    Face::Title,
    colors.ink,
    Align::Left,
  );
  let Ok(()) = kind_buttons(
    scene,
    Point {
      x: right,
      y: i32::wrapping_add(section, 4),
    },
  );
  let Ok(ranked) = app.source.lanes(range, app.kind, LaneView::Overview);
  let first = i32::wrapping_add(section, 46);

  let high = match ranked.first().map(|lane| lane.total) {
    Some(high) => high,
    None => {
      let Ok(()) = empty(
        scene,
        Area {
          origin: Point {
            x: left,
            y: i32::wrapping_add(first, 10),
          },
          size: Size {
            width: i32::wrapping_sub(right, left),
            height: 48,
          },
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
    let strip = Area {
      origin: Point { x: left, y },
      size: Size {
        width: area.size.width,
        height: ROW,
      },
    };
    let Ok(()) = ranked_lane(scene, strip, lane, RankScale { high, density });
  }

  let Ok(rows) = fitted::<_, i32>(count);
  let end = i32::wrapping_add(first, i32::wrapping_mul(rows, ROW));

  Ok(match bool::then_some(ranked.len() > 5, ()) {
    Some(()) => {
      let Ok(()) = scene.canvas.button(
        Area {
          origin: Point {
            x: left,
            y: i32::wrapping_add(end, 7),
          },
          size: Size {
            width: 93,
            height: 30,
          },
        },
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
  let Area {
    origin: Point { x: left, y },
    ..
  } = strip;
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
    origin: Point {
      x: i32::wrapping_add(i32::wrapping_add(left, label_width), 10),
      y: i32::wrapping_add(y, 17),
    },
    size: Size {
      width: i32::wrapping_sub(
        i32::wrapping_sub(i32::wrapping_sub(right, left), label_width),
        89,
      ),
      height: 4,
    },
  };
  let Ok(()) = scene.canvas.text(
    Area {
      origin: Point { x: left, y },
      size: Size {
        width: label_width,
        height: ROW,
      },
    },
    lane.label,
    15,
    Face::Body,
    colors.ink,
    Align::Left,
  );
  let proportion = Proportion {
    value: lane.total,
    maximum: high,
  };
  let Ok(()) = rank_bar(scene, track, proportion);
  let Ok(time_spent) = duration(lane.total);
  let Ok(()) = scene.canvas.text(
    Area {
      origin: Point {
        x: i32::wrapping_sub(right, 69),
        y,
      },
      size: Size {
        width: 69,
        height: ROW,
      },
    },
    &time_spent,
    13,
    Face::Mono,
    colors.quiet,
    Align::Right,
  );
  Ok(())
}

fn rank_bar(
  scene: &mut Scene<'_>,
  track: Area,
  proportion: Proportion,
) -> std::result::Result<(), Never> {
  let Ok(()) = scene.canvas.panel(track, scene.palette.lifted, 2, None);
  let Ok(width) = scaled(proportion, track.size.width);
  scene.canvas.panel(
    Area {
      size: Size {
        width: width.max(2),
        ..track.size
      },
      ..track
    },
    scene.palette.bars,
    2,
    None,
  )
}

struct Proportion {
  value: u64,
  maximum: u64,
}

#[must_use]
fn scaled(
  proportion: Proportion,
  pixels: i32,
) -> core::result::Result<i32, console_core_never::Never> {
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
  let width = area.size.width.max(1);
  let Ok(bar_count) = fitted::<_, i32>(bars.len());
  let plateau = match bars.iter().map(|bar| bar.seconds).max() {
    Some(present) => present,
    None => 0,
  };
  let Ok(bottom) = area.bottom();
  let Ok(()) = scene.canvas.line(
    Point {
      x: area.origin.x,
      y: bottom,
    },
    width,
  );
  let filled = bars.iter().any(|bar| bar.seconds != 0);

  match bool::then_some(!filled, ()) {
    Some(()) => {
      let Ok(()) = scene.canvas.text(
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
  let chart = TrendChart {
    area,
    width,
    counts,
    plateau,
  };

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
  let TrendChart {
    area,
    width,
    plateau,
    ..
  } = *chart;
  let colors = scene.palette;
  let Ok(hit) = trend_column(chart, number);
  let action = Action::Chart {
    day: bar.day,
    slot: number,
  };
  let hover = scene.app.hover == Some(action);
  let focus = scene.app.focus == Some(action);
  let is_today = bar.day == scene.app.current_day && scene.app.scope != Scope::Today;
  let ratio = Proportion {
    value: bar.seconds,
    maximum: plateau,
  };
  let Ok(scaled_height) = scaled(ratio, i32::wrapping_sub(area.size.height, 10));
  let height = scaled_height.max(2);
  let space = hit.size.width;
  let bar_width = i32::wrapping_div(i32::wrapping_mul(space, 54), 100).clamp(3, 69);
  let x = i32::wrapping_add(
    hit.origin.x,
    i32::wrapping_div(i32::wrapping_sub(space, bar_width), 2),
  );
  let ink = match (bar.seconds, hover || focus || is_today) {
    (0, _) => colors.rule,
    (_, true) => colors.accent,
    _ => colors.bars,
  };
  let Ok(bottom) = area.bottom();
  let Ok(()) = scene.canvas.panel(
    Area {
      origin: Point {
        x,
        y: i32::wrapping_sub(bottom, height),
      },
      size: Size {
        width: bar_width,
        height,
      },
    },
    ink,
    5,
    None,
  );

  match bool::then_some(focus, ()) {
    Some(()) => {
      let Ok(area_bottom) = area.bottom();
      let Ok(()) = scene.canvas.panel(
        Area {
          origin: Point {
            x,
            y: i32::wrapping_add(area_bottom, 3),
          },
          size: Size {
            width: bar_width,
            height: 1,
          },
        },
        colors.accent,
        0,
        None,
      );
    }
    None => {}
  }

  let Ok(()) = trend_bar_label(scene, chart, hit, &bar.label);
  let Ok(()) = scene.canvas.hit(hit, action);

  match ((hover || focus) && width >= 510).then_some(()) {
    Some(()) => {
      let Ok(time_spent) = duration(bar.seconds);
      let detail = format!("{}  ·  {time_spent}", bar.title);
      let Ok(()) = scene.canvas.text(
        Area {
          origin: Point {
            x: area.origin.x,
            y: i32::wrapping_sub(area.origin.y, 30),
          },
          size: Size { width, height: 22 },
        },
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

fn trend_column(chart: &TrendChart, number: u32) -> std::result::Result<Area, Never> {
  let TrendChart {
    area,
    width,
    counts,
    ..
  } = *chart;
  let Ok(bar_index) = fitted::<_, i32>(number);
  let x = area
    .origin
    .x
    .wrapping_add(bar_index.wrapping_mul(width).wrapping_div(counts.get()));
  let next = area.origin.x.wrapping_add(
    bar_index
      .wrapping_add(1)
      .wrapping_mul(width)
      .wrapping_div(counts.get()),
  );
  Ok(Area {
    origin: Point { x, ..area.origin },
    size: Size {
      width: i32::wrapping_sub(next, x),
      height: i32::wrapping_add(area.size.height, 27),
    },
  })
}

fn trend_bar_label(
  scene: &mut Scene<'_>,
  chart: &TrendChart,
  column: Area,
  label: &str,
) -> std::result::Result<(), Never> {
  let (x, w, align) = match chart.counts.get().cmp(&8) {
    Ordering::Greater => {
      let Ok(right) = chart.area.right();
      let width = 43.min(right.saturating_sub(column.origin.x));

      (i32::wrapping_add(column.origin.x, 3), width, Align::Left)
    }
    Ordering::Less | Ordering::Equal => (column.origin.x, column.size.width, Align::Center),
  };
  let Ok(bottom) = chart.area.bottom();
  scene.canvas.text(
    Area {
      origin: Point {
        x,
        y: i32::wrapping_add(bottom, 7),
      },
      size: Size {
        width: w,
        height: 22,
      },
    },
    label,
    12,
    Face::Body,
    scene.palette.quiet,
    align,
  )
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
  let Ok(()) = scene.canvas.text(
    Area {
      origin: area.origin,
      size: Size {
        width: area.size.width,
        height: 24,
      },
    },
    title,
    15,
    Face::Body,
    scene.palette.ink,
    Align::Left,
  );

  Ok(match bool::then_some(kind == Kind::Websites, ()) {
    Some(()) => {
      let Ok(()) = scene.canvas.text(
        Area {
          origin: Point {
            x: area.origin.x,
            y: i32::wrapping_add(area.origin.y, 24),
          },
          size: Size {
            width: area.size.width,
            height: 24,
          },
        },
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
  let Ok(selected) = app.source.day(app.day);
  let day = match selected {
    Some(day) => day,
    None => return Ok(column.origin.y),
  };
  let range = day.period;
  let Ok(panel_y) = timeline_heading(scene, column, &day, range);
  timeline_panel(
    scene,
    Area {
      origin: Point {
        y: panel_y,
        ..column.origin
      },
      size: column.size,
    },
    range,
  )
}

fn timeline_heading(
  scene: &mut Scene<'_>,
  column: Area,
  day: &Day<'_>,
  range: Period,
) -> std::result::Result<i32, Never> {
  let app = scene.app;
  let colors = scene.palette;
  let compact = column.size.width < 540;
  let short = column.size.height < 370;
  let heading = Area {
    origin: Point {
      x: i32::wrapping_add(column.origin.x, 5),
      y: i32::wrapping_add(
        column.origin.y,
        match bool::then_some(short, ()) {
          Some(()) => 1,
          None => 10,
        },
      ),
    },
    size: Size {
      width: i32::wrapping_sub(column.size.width, 10),
      height: match bool::then_some(short, ()) {
        Some(()) => 35,
        None => 46,
      },
    },
  };
  let arrow_y = i32::wrapping_add(
    heading.origin.y,
    match bool::then_some(short, ()) {
      Some(()) => 1,
      None => 7,
    },
  );
  let area = Area {
    origin: heading.origin,
    size: Size {
      width: i32::wrapping_sub(heading.size.width, 90),
      height: heading.size.height,
    },
  };
  let Ok(()) = scene.canvas.text(
    area,
    day.title,
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
  let Ok(seconds) = app.source.total(range);
  let Ok(time_spent) = duration(seconds);
  let total = format!("{time_spent} in focus");
  let Ok(bottom) = heading.bottom();
  let Ok(()) = scene.canvas.text(
    Area {
      origin: Point {
        y: bottom,
        ..heading.origin
      },
      size: Size {
        width: heading.size.width,
        height: 23,
      },
    },
    &total,
    14,
    Face::Body,
    colors.quiet,
    Align::Left,
  );
  let Ok(()) = timeline_day_buttons(scene, heading, arrow_y);

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
  let next = match (scene.app.day < scene.app.current_day).then_some(()) {
    Some(()) => Button::Unselected,
    None => Button::Unavailable,
  };

  for arrow in [
    DayButton {
      label: "‹",
      step: -1,
      distance: 81,
      available: previous,
    },
    DayButton {
      label: "›",
      step: 1,
      distance: 37,
      available: next,
    },
  ] {
    let area = Area {
      origin: Point {
        x: right.wrapping_sub(arrow.distance),
        y,
      },
      size: Size {
        width: 36,
        height: 32,
      },
    };
    let Ok(()) =
      scene
        .canvas
        .button(area, arrow.label, Action::Step(arrow.step), arrow.available);
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
  let panel_y = column.origin.y;
  let short = column.size.height < 370;
  let left = i32::wrapping_add(column.origin.x, 22);
  let Ok(column_right) = column.right();
  let right = i32::wrapping_sub(column_right, 22);
  let Ok(lanes) = app.source.lanes(range, app.kind, LaneView::Timeline);
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
    origin: Point {
      x: column.origin.x,
      y: panel_y,
    },
    size: Size {
      width: column.size.width,
      height: i32::wrapping_sub(end, panel_y),
    },
  };
  let Ok(()) = scene
    .canvas
    .panel(panel, colors.surface, 13, Some(colors.rule));
  let panel_header = i32::wrapping_add(
    panel_y,
    match bool::then_some(short, ()) {
      Some(()) => 6,
      None => 14,
    },
  );
  let rows = Area {
    origin: Point {
      x: left,
      y: row_top,
    },
    size: Size {
      width: i32::wrapping_sub(right, left),
      height: i32::wrapping_sub(plot_end, row_top),
    },
  };
  let Ok(()) = timeline_panel_header(scene, rows, panel_header);

  match bool::then_some(lanes.is_empty(), ()) {
    Some(()) => {
      let Ok(()) = empty(
        scene,
        Area {
          origin: Point {
            x: rows.origin.x,
            y: i32::wrapping_add(panel_header, 40),
          },
          size: Size {
            width: rows.size.width,
            height: 50,
          },
        },
        app.kind,
        EmptyPeriod::Day,
      );
      return Ok(end);
    }
    None => {}
  }

  let Ok(()) = timeline_tracks(
    scene,
    rows,
    row_height,
    range,
    &lanes,
    ChartDensity::from(column.size.width < 540),
  );
  Ok(end)
}

fn timeline_panel_header(
  scene: &mut Scene<'_>,
  rows: Area,
  y: i32,
) -> std::result::Result<(), Never> {
  let Ok(right) = rows.right();
  let Ok(()) = scene.canvas.text(
    Area {
      origin: Point { y, ..rows.origin },
      size: Size {
        width: i32::wrapping_sub(rows.size.width, 150),
        height: 32,
      },
    },
    "Switch timeline",
    15,
    Face::Medium,
    scene.palette.ink,
    Align::Left,
  );
  kind_buttons(
    scene,
    Point {
      x: right,
      y: i32::wrapping_add(y, 1),
    },
  )
}

fn timeline_tracks(
  scene: &mut Scene<'_>,
  rows: Area,
  row_height: i32,
  range: Period,
  lanes: &[Lane<'_>],
  density: ChartDensity,
) -> std::result::Result<(), Never> {
  let left = rows.origin.x;
  let row_top = rows.origin.y;
  let label_width = i32::wrapping_div(i32::wrapping_mul(rows.size.width, 22), 100).clamp(77, 165);
  let plot = Area {
    origin: Point {
      x: i32::wrapping_add(i32::wrapping_add(left, label_width), 10),
      y: row_top,
    },
    size: Size {
      width: i32::wrapping_sub(i32::wrapping_sub(rows.size.width, label_width), 10),
      height: rows.size.height,
    },
  };
  let Ok(range_seconds) = range.seconds();
  let grid = TimelineGrid {
    plot,
    range,
    range_seconds,
  };
  let Ok(()) = timeline_ticks(scene, &grid, density);
  let mut detail = None;

  for (position, lane) in lanes.iter().enumerate() {
    let Ok(number) = fitted::<_, u32>(position);
    let Ok(row) = fitted::<_, i32>(position);
    let y = row_top.wrapping_add(row.wrapping_mul(row_height));
    let layout = TimelineLane {
      area: Area {
        origin: Point { x: left, y },
        size: Size {
          width: rows.size.width,
          height: row_height,
        },
      },
      label_width,
      number,
    };
    let Ok(selected) = timeline_lane(scene, &grid, layout, lane);

    match selected {
      Some(text) => detail = Some(text),
      None => {}
    }
  }

  timeline_footer(scene, rows, detail.as_deref())
}

fn timeline_footer(
  scene: &mut Scene<'_>,
  rows: Area,
  detail: Option<&str>,
) -> std::result::Result<(), Never> {
  let left = rows.origin.x;
  let Ok(plot_end) = rows.bottom();
  let Ok(()) = scene.canvas.line(
    Point {
      x: left,
      y: plot_end,
    },
    rows.size.width,
  );
  let (hint, ink) = match detail {
    Some(hint) => (hint, scene.palette.ink),
    None => (
      "Sampled switches are approximate. Select a segment for its time.",
      scene.palette.quiet,
    ),
  };
  scene.canvas.text(
    Area {
      origin: Point {
        x: left,
        y: i32::wrapping_add(plot_end, 10),
      },
      size: Size {
        width: rows.size.width,
        height: 30,
      },
    },
    hint,
    12,
    Face::Body,
    ink,
    Align::Left,
  )
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
    let Ok(offset) = scaled(amount, self.plot.size.width);
    Ok(self.plot.origin.x.wrapping_add(offset))
  }
}

fn timeline_ticks(
  scene: &mut Scene<'_>,
  grid: &TimelineGrid,
  density: ChartDensity,
) -> std::result::Result<(), Never> {
  let plot = grid.plot;
  let row_top = plot.origin.y;
  let colors = scene.palette;

  for (position, (unix, label)) in scene.app.ticks.iter().enumerate() {
    let Ok(index) = fitted::<_, u32>(position);

    match bool::then_some(
      density == ChartDensity::Compact && index.wrapping_rem(2) == 1,
      (),
    ) {
      Some(()) => {
        continue;
      }
      None => {}
    }

    let Ok(x) = grid.x(*unix);
    let Ok(()) = scene.canvas.text(
      Area {
        origin: Point {
          x: i32::wrapping_sub(
            x,
            match index {
              0 => 0,
              _ => 16,
            },
          ),
          y: i32::wrapping_sub(row_top, 24),
        },
        size: Size {
          width: 42,
          height: 18,
        },
      },
      label,
      11,
      Face::Mono,
      colors.quiet,
      Align::Left,
    );

    match bool::then_some(index > 0, ()) {
      Some(()) => {
        let Ok(()) = scene.canvas.panel(
          Area {
            origin: Point { x, y: row_top },
            size: Size {
              width: 1,
              height: plot.size.height,
            },
          },
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
  let TimelineLane {
    area,
    label_width,
    number,
  } = layout;
  let Area {
    origin: Point { y, .. },
    size: Size {
      height: row_height, ..
    },
  } = area;
  let app = scene.app;
  let colors = scene.palette;
  let plot = grid.plot;
  let mut detail = None;
  let Ok(()) = timeline_lane_heading(scene, area, label_width, lane.label);
  let Ok(pigment) = lane_color(lane.label, colors);

  for item in &lane.periods {
    let action = Action::Segment {
      lane: number,
      period: *item,
    };
    let hot = app.hover == Some(action) || app.focus == Some(action);
    let selected = app.details.filter(|(period, _)| period == item);

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
      origin: Point {
        x: x.min(i32::wrapping_sub(plot_right, width)),
        y: i32::wrapping_add(y, i32::wrapping_div(i32::wrapping_sub(row_height, 13), 2)),
      },
      size: Size { width, height: 13 },
    };
    let ink = match bool::then_some(hot || selected.is_some(), ()) {
      Some(()) => colors.ink,
      None => pigment,
    };
    let Ok(()) = scene.canvas.panel(chip, ink, 4, None);
    let hit = Area {
      origin: Point {
        x: chip.origin.x,
        y: i32::wrapping_add(y, 2),
      },
      size: Size {
        width: chip.size.width.max(8),
        height: i32::wrapping_sub(row_height, 4),
      },
    };

    {
      let Ok(inside) = hit.inside(Area {
        origin: Point {
          x: plot.origin.x,
          y,
        },
        size: Size {
          width: plot.size.width,
          height: row_height,
        },
      });

      match inside {
        Some(hit_2) => {
          let Ok(()) = scene.canvas.hit(hit_2, action);
        }
        None => {}
      };
    }
  }

  Ok(detail)
}

fn timeline_lane_heading(
  scene: &mut Scene<'_>,
  row: Area,
  width: i32,
  label: &str,
) -> std::result::Result<(), Never> {
  let at = row.origin;
  let Ok(()) = scene.canvas.line(at, row.size.width);
  scene.canvas.text(
    Area {
      origin: row.origin,
      size: Size { width, ..row.size },
    },
    label,
    13,
    Face::Body,
    scene.palette.ink,
    Align::Left,
  )
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
