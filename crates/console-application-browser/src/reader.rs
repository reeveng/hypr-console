use console_core_color::{Hue, Oklch};
use console_core_fonts::{Font, TextStyle, Weight};
use console_core_geometry::{Point, Size};
use console_core_ini_files::{Key, Under, field};
use console_core_never::Never;
use console_core_places::Base;
use console_input_key_symbols::Keysym;
use console_panel::ui::painting::{self, Frame, Run};
use console_panel::ui::shapes::{Edge, Panel, Round, Shape, Text};

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

#[must_use]
pub fn background(
  size: Size<u32>,
  fill: Oklch,
) -> core::result::Result<Shape, console_core_never::Never> {
  Ok(Shape::Panel(Panel {
    at: Point { x: 0, y: 0 },
    size,
    round: Round(0),
    fill,
    edge: Edge::None,
  }))
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

pub fn desktop_font() -> Result<Font, Never> {
  let Ok(fallback) = TextStyle::Body.font();
  let Ok(configuration) = Base::Configuration.user();
  let config = match configuration {
    Some(config) => config,
    None => return Ok(fallback),
  };

  for path in [
    config.join("gtk-4.0/settings.ini"),
    config.join("gtk-3.0/settings.ini"),
  ] {
    let settings = match std::fs::read_to_string(path) {
      Ok(settings) => settings,
      Err(fault) => {
        match ::core::primitive::bool::then_some(fault.kind() == std::io::ErrorKind::NotFound, ())
        {
          ::core::option::Option::Some(()) => continue,
          ::core::option::Option::None => {
            console_core_logging::debug!("reading browser reader font settings: {fault}");
            continue;
          }
        }
      }
    };
    let Ok(name) = field(&settings, Under("Settings"), Key("gtk-font-name"));
    let Ok(font) = match name {
      Some(configured) => interface_font(configured),
      None => continue,
    };

    match font {
      Some(selected) => return Ok(selected),
      None => {}
    }
  }

  Ok(fallback)
}

fn interface_font(name: &str) -> Result<Option<Font>, Never> {
  let (family, points) = match name.trim().rsplit_once(char::is_whitespace) {
    Some(font_parts) => font_parts,
    None => return Ok(None),
  };
  let points_2 = match points.parse::<f64>() {
    Ok(size) => size,
    Err(_invalid_size) => return Ok(None),
  };
  let family_2 = family.trim();

  match (!family_2.is_empty() && points_2.is_finite() && points_2 > 0.0).then_some(()) {
    Some(()) => {}
    None => return Ok(None),
  }

  let Ok(height) = console_core_number_conversion::toward_zero_u32(
    (points_2 * 96.0 / 72.0).round().clamp(10.0, 64.0),
  );
  Ok(Some(Font {
    family: family_2.to_owned(),
    height,
  }))
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
  #[must_use]
  pub fn mode(&self) -> core::result::Result<Option<Mode>, console_core_never::Never> {
    Ok(match self {
      Self::Hidden => None,
      Self::Pending { mode, .. } | Self::Open { mode, .. } => Some(*mode),
    })
  }
}

pub(crate) struct ReaderText {
  pub(crate) title: String,
  pub(crate) body: String,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "this scalar stores a signed vertical scroll delta in logical pixels"
  )
)]
pub(crate) struct ScrollPixels(pub f64);

pub struct Article {
  title: String,
  text: String,
  font: Font,
  width: u32,
  rows: Vec<(String, Weight)>,
  first: u64,
}

impl Article {
  #[must_use]
  pub fn new(ReaderText { title, body }: ReaderText) -> Result<Self, console_core_never::Never> {
    let Ok(mut font) = TextStyle::Body.font();
    font.family = console_core_fonts::SERIF.to_owned();

    Ok(Self {
      title,
      text: body,
      font,
      width: 0,
      rows: Vec::new(),
      first: 0,
    })
  }

  pub fn prepare(
    &mut self,
    viewport: Size<u32>,
    family: &str,
    text_size: u32,
  ) -> Result<(), console_core_never::Never> {
    let width = viewport
      .width
      .saturating_sub(MARGIN.saturating_mul(2))
      .clamp(1, 700);
    let changed =
      width != self.width || family != self.font.family || text_size != self.font.height;

    match changed.then_some(()) {
      Some(()) => {
        self.width = width;
        self.font.family = family.to_owned();
        self.font.height = text_size;
        let Ok(rows) = self.wrap_rows();
        self.rows = rows;
      }
      None => {}
    }

    let Ok(last) = self.end(viewport.height);
    self.first = self.first.min(last);
    Ok(())
  }

  fn wrap_rows(&self) -> Result<Vec<(String, Weight)>, console_core_never::Never> {
    let paragraphs = std::iter::once((self.title.as_str(), Weight::Bold))
      .chain(self.text.lines().map(|line| (line, Weight::Plain)));
    let mut rows = Vec::new();

    for (said, weight) in paragraphs {
      let Ok(lines) = painting::wrapped(
        Run {
          said,
          weight,
          width: self.width,
        },
        &self.font,
      );

      match lines.as_slice() {
        [] => rows.push((String::new(), weight)),
        [_, ..] => rows.extend(lines.into_iter().map(|line| (line, weight))),
      }

      match weight {
        Weight::Bold => rows.push((String::new(), Weight::Plain)),
        Weight::Plain => {}
      }
    }

    Ok(rows)
  }

  fn leading(&self) -> Result<u32, console_core_never::Never> {
    Ok(self.font.height.saturating_add(10))
  }

  fn page(&self, height: u32) -> Result<u64, console_core_never::Never> {
    let room = height.saturating_sub(MARGIN.saturating_mul(2));
    let Ok(leading) = self.leading();
    let rows = match room.checked_div(leading) {
      Some(visible) => visible.max(1),
      None => 1,
    };

    Ok(u64::from(rows))
  }

  fn end(&self, height: u32) -> Result<u64, console_core_never::Never> {
    let Ok(count) = console_core_number_conversion::fixed_index(self.rows.len());
    let Ok(visible) = self.page(height);
    Ok(count.saturating_sub(visible))
  }

  pub fn scroll(
    &mut self,
    ScrollPixels(delta): ScrollPixels,
    height: u32,
  ) -> Result<(), console_core_never::Never> {
    let Ok(leading) = self.leading();
    let Ok(rows) = console_core_number_conversion::toward_zero_i32(
      (delta.abs() / f64::from(leading)).ceil().copysign(delta),
    );
    let count = u64::from(rows.unsigned_abs());
    let Ok(last) = self.end(height);
    self.first = match delta.is_sign_negative().then_some(()) {
      Some(()) => self.first.saturating_sub(count),
      None => self.first.saturating_add(count).min(last),
    };
    Ok(())
  }

  pub fn key(&mut self, key: Keysym, height: u32) -> Result<(), console_core_never::Never> {
    let Ok(page) = self.page(height);
    let Ok(end) = self.end(height);
    self.first = match key {
      Keysym::Up => self.first.saturating_sub(1),
      Keysym::Down => self.first.saturating_add(1).min(end),
      Keysym::Page_Up => self.first.saturating_sub(page),
      Keysym::Page_Down | Keysym::space => self.first.saturating_add(page).min(end),
      Keysym::Home => 0,
      Keysym::End => end,
      _ => self.first,
    };
    Ok(())
  }

  pub fn paint(
    &self,
    pixels: &mut [u8],
    frame: Frame,
    (background, ink): (Oklch, Oklch),
  ) -> Result<(), painting::Cannot> {
    let mut shapes = vec![
      ({
        let Ok(explicit_value) = self::background(frame.points, background);
        explicit_value
      }),
    ];
    let left = frame
      .points
      .width
      .saturating_sub(self.width)
      .wrapping_div(2);
    let Ok(x) = console_core_number_conversion::fitted(left);
    let Ok(mut y) = console_core_number_conversion::fitted::<_, i32>(MARGIN);
    let Ok(own_leading) = self.leading();
    let Ok(leading) = console_core_number_conversion::fitted(own_leading);

    let Ok(first) = console_core_number_conversion::index(self.first);
    let Ok(visible) = self.page(frame.points.height);
    let Ok(count) = console_core_number_conversion::index(visible);

    for (line, weight) in self.rows.iter().skip(first).take(count) {
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

    painting::onto(pixels, frame, &shapes)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn desktop_font_sizes_are_converted_from_points() {
    assert_eq!(
      interface_font("Noto Sans 12"),
      Ok(Some(Font {
        family: "Noto Sans".to_owned(),
        height: 16
      }))
    );
    assert_eq!(interface_font("Noto Sans invalid"), Ok(None));
    assert_eq!(interface_font("Noto Sans NaN"), Ok(None));
  }
}
