use console_core_color::{Hue, Oklch};
use console_core_fonts::{Font, TextStyle, Weight};
use console_core_geometry::{Point, Size};
use console_core_ini_files::{Key, Under, field};
use console_core_places::Base;
use console_core_shapes::{Edge, Panel, Round, Shape, Text};
use console_draw_painting::{self as drawing, Frame, Run};
use console_input_key_symbols::Keysym;

pub const DARK: (Oklch, Oklch) = (
  Oklch {
    lightness: 0.20,
    chroma: 0.014,
    hue: Hue(255.0),
  },
  Oklch {
    lightness: 0.93,
    chroma: 0.010,
    hue: Hue(85.0),
  },
);
pub const DEFAULT_TEXT_SIZE: u32 = 18;
const MARGIN: u32 = 24;

pub fn background(size: Size<u32>, fill: Oklch) -> Shape {
  Shape::Panel(Panel {
    at: Point { x: 0, y: 0 },
    size,
    round: Round(0),
    fill,
    edge: Edge::None,
  })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
  Dark,
  Desktop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSize {
  Smaller,
  Larger,
}

pub fn desktop_font() -> Font {
  let Ok(fallback) = TextStyle::Body.font();
  let Ok(configuration) = Base::Configuration.user();
  let config = match configuration {
    Some(config) => config,
    None => return fallback,
  };
  for relative in ["gtk-4.0/settings.ini", "gtk-3.0/settings.ini"] {
    let settings = match std::fs::read_to_string(config.join(relative)) {
      Ok(settings) => settings,
      Err(_) => continue,
    };
    let Ok(name) = field(&settings, Under("Settings"), Key("gtk-font-name"));
    match name.and_then(interface_font) {
      Some(font) => return font,
      None => {}
    }
  }
  fallback
}

fn interface_font(name: &str) -> Option<Font> {
  let (family, points) = name.trim().rsplit_once(char::is_whitespace)?;
  let points = points.parse::<f64>().ok()?;
  let family = family.trim();
  match (!family.is_empty() && points.is_finite() && points > 0.0).then_some(()) {
    Some(()) => {}
    None => return None,
  }
  let Ok(height) = console_core_number_conversion::toward_zero_u32(
    (points * 96.0 / 72.0).round().clamp(10.0, 64.0),
  );
  Some(Font {
    family: family.to_owned(),
    height,
  })
}

#[derive(Default)]
pub enum Reader {
  #[default]
  Hidden,
  Pending {
    id: u64,
    mode: Mode,
  },
  Open {
    mode: Mode,
    article: Article,
  },
}

impl Reader {
  pub fn mode(&self) -> Option<Mode> {
    match self {
      Self::Hidden => None,
      Self::Pending { mode, .. } | Self::Open { mode, .. } => Some(*mode),
    }
  }
}

pub struct Article {
  title: String,
  text: String,
  font: Font,
  width: u32,
  rows: Vec<(String, Weight)>,
  first: usize,
}

impl Article {
  pub fn new(title: String, text: String) -> Self {
    let Ok(mut font) = TextStyle::Body.font();
    font.family = console_core_fonts::SERIF.to_owned();
    Self {
      title,
      text,
      font,
      width: 0,
      rows: Vec::new(),
      first: 0,
    }
  }

  pub fn prepare(&mut self, viewport: Size<u32>, family: &str, text_size: u32) {
    let width = viewport.width.saturating_sub(2 * MARGIN).clamp(1, 700);
    match (width != self.width || family != self.font.family || text_size != self.font.height)
      .then_some(())
    {
      Some(()) => {
        self.width = width;
        self.font.family = family.to_owned();
        self.font.height = text_size;
        self.rows.clear();
        for (paragraph, weight) in std::iter::once((self.title.as_str(), Weight::Bold))
          .chain(self.text.lines().map(|line| (line, Weight::Plain)))
        {
          let Ok(lines) = drawing::wrapped(
            Run {
              said: paragraph,
              weight,
              width,
            },
            &self.font,
          );
          match lines.as_slice() {
            [] => self.rows.push((String::new(), weight)),
            _ => {}
          }
          self
            .rows
            .extend(lines.into_iter().map(|line| (line, weight)));
          match weight {
            Weight::Bold => self.rows.push((String::new(), Weight::Plain)),
            Weight::Plain => {}
          }
        }
      }
      None => {}
    }
    self.first = self.first.min(self.end(viewport.height));
  }

  fn leading(&self) -> u32 {
    self.font.height.saturating_add(10)
  }

  fn page(&self, height: u32) -> usize {
    let rows = height
      .saturating_sub(2 * MARGIN)
      .wrapping_div(self.leading())
      .max(1);
    let Ok(rows) = console_core_number_conversion::index(rows);
    rows
  }

  fn end(&self, height: u32) -> usize {
    self.rows.len().saturating_sub(self.page(height))
  }

  pub fn scroll(&mut self, delta: f64, height: u32) {
    let Ok(rows) = console_core_number_conversion::toward_zero_i32(
      (delta.abs() / f64::from(self.leading()))
        .ceil()
        .copysign(delta),
    );
    let Ok(rows) = console_core_number_conversion::index(rows.unsigned_abs());
    self.first = match delta.is_sign_negative().then_some(()) {
      Some(()) => self.first.saturating_sub(rows),
      None => self.first.saturating_add(rows).min(self.end(height)),
    };
  }

  pub fn key(&mut self, key: Keysym, height: u32) {
    let page = self.page(height);
    let end = self.end(height);
    self.first = match key {
      Keysym::Up => self.first.saturating_sub(1),
      Keysym::Down => self.first.saturating_add(1).min(end),
      Keysym::Page_Up => self.first.saturating_sub(page),
      Keysym::Page_Down | Keysym::space => self.first.saturating_add(page).min(end),
      Keysym::Home => 0,
      Keysym::End => end,
      _ => self.first,
    };
  }

  pub fn paint(
    &self,
    pixels: &mut [u8],
    frame: Frame,
    (background, ink): (Oklch, Oklch),
  ) -> Result<(), drawing::Cannot> {
    let mut shapes = vec![self::background(frame.points, background)];
    let left = frame
      .points
      .width
      .saturating_sub(self.width)
      .wrapping_div(2);
    let Ok(x) = console_core_number_conversion::fitted(left);
    let Ok(mut y) = console_core_number_conversion::fitted::<_, i32>(MARGIN);
    let Ok(leading) = console_core_number_conversion::fitted(self.leading());
    for (line, weight) in self
      .rows
      .iter()
      .skip(self.first)
      .take(self.page(frame.points.height))
    {
      match line.as_str() {
        "" => {}
        _ => shapes.push(Shape::Text(Text {
          at: Point { x, y },
          width: self.width,
          said: line.clone(),
          weight: *weight,
          font: self.font.clone(),
          ink,
        })),
      }
      y = y.saturating_add(leading);
    }
    drawing::onto(pixels, frame, &shapes)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn desktop_font_sizes_are_converted_from_points() {
    assert_eq!(
      interface_font("Noto Sans 12"),
      Some(Font {
        family: "Noto Sans".to_owned(),
        height: 16
      })
    );
    assert_eq!(interface_font("Noto Sans invalid"), None);
    assert_eq!(interface_font("Noto Sans NaN"), None);
  }
}
