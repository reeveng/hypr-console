//! Native Console drawing and hit testing share the same layout.

use std::cmp::Ordering;
use std::io;
use std::num::NonZeroI32;

use console_core_color::palette::{self, Wearing};
use console_core_color::{Oklch, to_oklch};
use console_core_external_programs::Installed;
use console_core_fonts::{Font, LETTERS, MONOSPACED, Weight};
use console_core_geometry::{Containment, Point, Size};
use console_core_number_conversion::{checked_index, fitted, toward_zero_u8};
use console_core_shapes::{Clip, Cropped, Edge, Panel, Pixels, Round, Shape, Text};
use console_draw_painting::{Lines, Run, at_most, measure_text};
use console_draw_surface::Part;

use crate::catalogue::{Category, Package};
use crate::{
  Action, Notice, Overlay, Page, ProfileAvailability, ProfileStatus, SearchStatus, Shop,
};

const PALETTE: &str = include_str!("../../../files/usr/local/lib/console/palette.sh");
const FEATURE: &[u8] = include_bytes!("../assets/obs-feature.png");

pub struct Theme {
  colors: Wearing,
  art: Pixels,
  white: Oklch,
  lilac: Oklch,
}

impl Theme {
  pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
    let colors = match Wearing::worn() {
      Ok(colors) => colors,
      Err(fault) => {
        console_core_logging::debug!("store theme: {fault}");
        let Ok(fallback) = palette::read(PALETTE);
        Wearing::out_of(&fallback)?
      }
    };
    let mut artwork = console_core_png_files::decoded(FEATURE)?;

    shade_feature(&mut artwork.bytes, artwork.size)?;

    let Ok(art) = artwork.into_strided();
    let Ok(white) = to_oklch("#ffffff");
    let Ok(lilac) = to_oklch("#d7cefa");
    Ok(Self {
      colors,
      art,
      white,
      lilac,
    })
  }
}

fn shade_feature(bytes: &mut [u8], size: Size<u32>) -> io::Result<()> {
  let stride = checked_index(size.width).map_err(io::Error::other)?;
  let (mut pixels, tail) = bytes.as_chunks_mut::<4>();

  match bool::then_some(size.width > 0 && tail.is_empty(), ()) {
    Some(()) => {}
    None => {
      return Err(io::Error::other(
        "feature image has an incomplete pixel or no columns",
      ));
    }
  }

  let columns = f64::from(size.width);
  let brightness = |x| 0.68 + f64::from(x) / columns;
  let shaded = match (0..size.width).find(|x| brightness(*x) >= 1.0) {
    Some(full_brightness) => full_brightness,
    None => size.width,
  };

  for _row in 0..size.height {
    let (row_pixels, rest) = pixels
      .split_at_mut_checked(stride)
      .ok_or_else(|| io::Error::other("feature image has an incomplete row"))?;
    pixels = rest;

    for (x, [red, green, blue, _alpha]) in (0..shaded).zip(row_pixels) {
      let light = brightness(x);

      for channel in [red, green, blue] {
        let Ok(shaded_channel) = toward_zero_u8(f64::from(*channel) * light);
        *channel = shaded_channel;
      }
    }
  }

  Ok(())
}

#[derive(Clone, Copy, Default)]
pub struct Area {
  x: i32,
  y: i32,
  w: i32,
  h: i32,
}

impl Area {
  #[must_use]
  fn origin(self) -> core::result::Result<Point<i32>, core::convert::Infallible> {
    Ok(Point {
      x: self.x,
      y: self.y,
    })
  }

  #[must_use]
  fn size(self) -> core::result::Result<Size<u32>, core::convert::Infallible> {
    Ok(Size {
      width: i32::cast_unsigned(self.w.max(0)),
      height: i32::cast_unsigned(self.h.max(0)),
    })
  }

  #[must_use]
  fn right(self) -> core::result::Result<i32, core::convert::Infallible> {
    Ok(i32::wrapping_add(self.x, self.w))
  }

  #[must_use]
  fn bottom(self) -> core::result::Result<i32, core::convert::Infallible> {
    Ok(i32::wrapping_add(self.y, self.h))
  }

  #[must_use]
  fn inset(self, inset: i32) -> core::result::Result<Self, core::convert::Infallible> {
    let area = Self {
      x: i32::wrapping_add(self.x, inset),
      y: i32::wrapping_add(self.y, inset),
      w: i32::wrapping_sub(self.w, i32::wrapping_mul(2, inset)),
      h: i32::wrapping_sub(self.h, i32::wrapping_mul(2, inset)),
    };

    Ok(area)
  }

  #[must_use]
  pub fn hit(self, (x, y): (f64, f64)) -> Result<Containment, core::convert::Infallible> {
    let Ok(right) = self.right();
    let Ok(bottom) = self.bottom();
    let inside = f64::from(self.x) <= x
      && x < f64::from(right)
      && f64::from(self.y) <= y
      && y < f64::from(bottom);
    Ok(match inside.then_some(()) {
      Some(()) => Containment::Inside,
      None => Containment::Outside,
    })
  }

  #[must_use]
  fn inside(self, limit: Self) -> core::result::Result<Option<Self>, core::convert::Infallible> {
    let x = self.x.max(limit.x);
    let y = self.y.max(limit.y);
    let Ok(right) = self.right();
    let Ok(limit_right) = limit.right();
    let w = i32::wrapping_sub(right.min(limit_right), x);
    let Ok(bottom) = self.bottom();
    let Ok(limit_bottom) = limit.bottom();
    let h = i32::wrapping_sub(bottom.min(limit_bottom), y);

    Ok((w > 0 && h > 0).then_some(Self { x, y, w, h }))
  }
}

#[derive(Default)]
pub struct Frame {
  pub shapes: Vec<Shape>,
  pub caret: Option<Part>,
  pub scroll_area: Area,
  pub max_scroll: i32,
  hits: Vec<(Area, Action)>,
}

#[derive(Clone, Copy)]
pub enum FocusDirection {
  Next,
  Previous,
}

impl Frame {
  #[must_use]
  pub fn hit(
    &self,
    point: (f64, f64),
  ) -> core::result::Result<Option<Action>, core::convert::Infallible> {
    Ok(
      self.hits.iter().rev().find_map(|(area, action)| {
        (area.hit(point) == Ok(Containment::Inside)).then_some(*action)
      }),
    )
  }

  #[must_use]
  pub fn tab(
    &self,
    current: Option<Action>,
    direction: FocusDirection,
  ) -> core::result::Result<Option<Action>, core::convert::Infallible> {
    let actions = self.hits.iter().map(|&(_, action)| action);
    Ok(match direction {
      FocusDirection::Next => actions
        .clone()
        .skip_while(|action| Some(*action) != current)
        .nth(1)
        .or_else(|| actions.clone().next()),
      FocusDirection::Previous => actions
        .clone()
        .take_while(|action| Some(*action) != current)
        .last()
        .or_else(|| actions.last()),
    })
  }
}

#[derive(Clone, Copy)]
enum Face {
  Plain,
  Bold,
  Mono,
}

impl Face {
  #[must_use]
  fn at(self, height: u32) -> core::result::Result<(Font, Weight), core::convert::Infallible> {
    let (family, weight) = match self {
      Self::Mono => (MONOSPACED, Weight::Plain),
      Self::Plain => (LETTERS, Weight::Plain),
      Self::Bold => (LETTERS, Weight::Bold),
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

struct Scene<'a> {
  app: &'a Shop,
  theme: &'a Theme,
  colors: Wearing,
  clip: Area,
  frame: Frame,
}

impl Scene<'_> {
  fn panel(
    &mut self,
    area: Area,
    fill: Oklch,
    radius: u32,
    outline: Option<Oklch>,
  ) -> core::result::Result<(), core::convert::Infallible> {
    {
      let Ok(inside) = area.inside(self.clip);

      match bool::then_some(inside.is_some(), ()) {
        Some(()) => {
          let Ok(at) = area.origin();
          let Ok(size) = area.size();
          self.frame.shapes.push(Shape::Panel(Panel {
            at,
            size,
            round: Round(radius),
            fill,
            edge: outline.map_or(Edge::None, |color| Edge::Of { wide: 1, color }),
          }));
        }
        None => {}
      }
    };

    Ok(())
  }

  fn text(
    &mut self,
    area: Area,
    text: &str,
    size: u32,
    face: Face,
    ink: Oklch,
    lines: Lines,
  ) -> core::result::Result<(), core::convert::Infallible> {
    {
      let Ok(inside) = area.inside(self.clip);

      match bool::then_some(inside.is_none() || text.is_empty(), ()) {
        Some(()) => {
          return Ok(());
        }
        None => {}
      }

      let Ok((font, weight)) = face.at(size);
      let Ok(area_size) = area.size();
      let width = area_size.width;
      let Ok(said) = at_most(
        Run {
          said: text,
          weight,
          width,
        },
        &font,
        lines,
      );
      let Ok(measured) = measure_text(
        Run {
          said: &said,
          weight,
          width,
        },
        &font,
      );
      let shift = match lines {
        Lines(1) => {
          let height = match i32::try_from(measured.height) {
            Ok(height) => height,
            Err(_larger_than_the_viewport) => i32::MAX,
          };
          area.h.saturating_sub(height).max(0).wrapping_div(2)
        }
        _ => 0,
      };
      self.frame.shapes.push(Shape::Text(Text {
        at: Point {
          x: area.x,
          y: i32::wrapping_add(area.y, shift),
        },
        width,
        said,
        weight,
        font,
        ink,
      }));
    };

    Ok(())
  }

  fn button(
    &mut self,
    area: Area,
    label: &str,
    action: Action,
    kind: Button,
  ) -> core::result::Result<(), core::convert::Infallible> {
    {
      let focus = self.app.focus == Some(action);
      let hover = self.app.hover == Some(action);
      let (fill, ink) = match kind {
        Button::Primary => (self.colors.pink, self.colors.night),
        Button::Selected => (self.colors.fill, self.colors.text),
        Button::Quiet => match (focus || hover).then_some(()) {
          Some(()) => (self.colors.panel, self.colors.text),
          None => (self.colors.ground, self.colors.soft),
        },
        Button::Normal => (self.colors.panel, self.colors.text),
      };
      let Ok(()) = self.panel(area, fill, 9, (focus || hover).then_some(self.colors.pink));
      let Ok(()) = self.text(
        Area {
          x: i32::wrapping_add(area.x, 13),
          y: area.y,
          w: i32::wrapping_sub(area.w, 22),
          h: area.h,
        },
        label,
        14,
        Face::Bold,
        ink,
        Lines(1),
      );
      let Ok(()) = self.hit(area, action);
    };

    Ok(())
  }

  fn hit(
    &mut self,
    area: Area,
    action: Action,
  ) -> core::result::Result<(), core::convert::Infallible> {
    {
      {
        let Ok(inside) = area.inside(self.clip);

        match inside {
          Some(area_2) => {
            self.frame.hits.push((area_2, action));
          }
          None => {}
        };
      }
    };

    Ok(())
  }

  fn clip(&mut self, area: Area) -> core::result::Result<(), core::convert::Infallible> {
    {
      self.clip = area;
      let Ok(at) = area.origin();
      let Ok(size) = area.size();
      self.frame.shapes.push(Shape::Clip(Clip::To { at, size }));
    };

    Ok(())
  }

  fn picture(&mut self, area: Area) -> core::result::Result<(), core::convert::Infallible> {
    {
      let Ok(inside) = area.inside(self.clip);

      match bool::then_some(inside.is_none(), ()) {
        Some(()) => {
          return Ok(());
        }
        None => {}
      }

      let art = &self.theme.art;
      let source_width = f64::from(art.width);
      let source_height = f64::from(art.height);
      let ratio = f64::from(area.w) / f64::from(area.h);
      let height = (source_width / ratio).min(source_height);
      let width = (height * ratio).min(source_width);
      let Ok(at) = area.origin();
      let Ok(size) = area.size();
      self.frame.shapes.push(Shape::Cropped(Cropped {
        at,
        size,
        pixels: art.clone(),
        from: Point {
          x: (source_width - width) / 2.0,
          y: (source_height - height) / 2.0,
        },
        seen: Size { width, height },
      }));
    };

    Ok(())
  }

  fn sidebar(
    &mut self,
    side: Area,
    view: StoreView,
  ) -> core::result::Result<(), core::convert::Infallible> {
    {
      let Ok(()) = self.panel(side, self.colors.ground, 0, None);
      let mark = Area {
        x: 18,
        y: 20,
        w: 37,
        h: 37,
      };
      let Ok(()) = self.panel(mark, self.colors.pink, 10, None);
      let area = Area {
        x: 28,
        y: 20,
        w: 27,
        h: 36,
      };
      let Ok(()) = self.text(area, "◇", 22, Face::Bold, self.colors.night, Lines(1));
      let Ok(()) = self.text(
        Area {
          x: 64,
          y: 20,
          w: i32::wrapping_sub(side.w, 66),
          h: 38,
        },
        match side.w.cmp(&170) {
          Ordering::Less => "Software\nStore",
          Ordering::Equal | Ordering::Greater => "Software Store",
        },
        16,
        Face::Bold,
        self.colors.text,
        Lines(match side.w.cmp(&170) {
          Ordering::Less => 2,
          Ordering::Equal | Ordering::Greater => 1,
        }),
      );

      for (position, (page, label)) in
        (0_i32..2).zip([(Page::Browse, "Browse"), (Page::Installed, "Installed")])
      {
        let selected = view != StoreView::Search && self.app.page == page;
        let kind = match bool::then_some(selected, ()) {
          Some(()) => Button::Selected,
          None => Button::Quiet,
        };
        let Ok(()) = self.button(
          Area {
            x: 14,
            y: i32::wrapping_add(93, 50_i32.wrapping_mul(position)),
            w: i32::wrapping_sub(side.w, 28),
            h: 43,
          },
          label,
          Action::Page(page),
          kind,
        );
      }

      let Ok(bottom) = side.bottom();
      let Ok(()) = self.button(
        Area {
          x: 14,
          y: i32::wrapping_sub(bottom, 59),
          w: i32::wrapping_sub(side.w, 28),
          h: 40,
        },
        "Help",
        Action::Help,
        Button::Quiet,
      );
    };

    Ok(())
  }

  fn search_bar(&mut self, area: Area) -> core::result::Result<(), core::convert::Infallible> {
    {
      let focus = self.app.focus == Some(Action::Search);
      let Ok(()) = self.panel(
        area,
        self.colors.ground,
        10,
        Some(match bool::then_some(focus, ()) {
          Some(()) => self.colors.pink,
          None => self.colors.panel,
        }),
      );
      let Ok(()) = self.text(
        Area {
          x: i32::wrapping_add(area.x, 12),
          y: area.y,
          w: 27,
          h: area.h,
        },
        "⌕",
        21,
        Face::Bold,
        self.colors.soft,
        Lines(1),
      );
      let started = i32::wrapping_add(area.x, 39);
      let Ok(edited) = self.app.query();
      let Ok(before) = self.app.query.before_caret();
      let remaining = i32::wrapping_sub(area.w, 111).max(1);
      let line = Area {
        x: started,
        y: area.y,
        w: remaining,
        h: area.h,
      };

      match bool::then_some(
        edited.is_empty() && self.app.preedit.is_empty() && !focus,
        (),
      ) {
        Some(()) => {
          let Ok(()) = self.text(
            line,
            "Search for an app or package",
            15,
            Face::Plain,
            self.colors.soft,
            Lines(1),
          );
        }
        None => {
          let Ok((font, weight)) = Face::Plain.at(15);
          let after = match edited.get(before.len()..) {
            Some(after) => after,
            None => "",
          };
          let shown = format!("{before}{}{after}", self.app.preedit);
          let at_caret = format!("{before}{}", self.app.preedit);
          let Ok(measured) = measure_text(
            Run {
              said: &at_caret,
              weight,
              width: u32::MAX,
            },
            &font,
          );
          let caret_x = i32::wrapping_add(
            started,
            u32::cast_signed(measured.width).clamp(0, remaining.saturating_sub(1)),
          );
          let Ok(()) = self.text(line, &shown, 15, Face::Plain, self.colors.text, Lines(1));

          match bool::then_some(focus, ()) {
            Some(()) => {
              let caret = Area {
                x: caret_x,
                y: area.y.wrapping_add(12),
                w: 1,
                h: 19,
              };
              let Ok(()) = self.panel(caret, self.colors.pink, 0, None);
              let Ok(at) = caret.origin();
              let Ok(size) = caret.size();
              self.frame.caret = Some(Part { at, size });
            }
            None => {}
          }
        }
      }

      match bool::then_some(edited.is_empty(), ()) {
        Some(()) => {
          let Ok(right) = area.right();
          let Ok(()) = self.text(
            Area {
              x: i32::wrapping_sub(right, 66),
              y: area.y,
              w: 57,
              h: area.h,
            },
            "Ctrl+K",
            12,
            Face::Mono,
            self.colors.soft,
            Lines(1),
          );
          let Ok(()) = self.hit(area, Action::Search);
        }
        None => {
          let Ok(()) = self.hit(
            Area {
              x: area.x,
              y: area.y,
              w: i32::wrapping_sub(area.w, 68),
              h: area.h,
            },
            Action::Search,
          );
          let Ok(right) = area.right();
          let Ok(()) = self.button(
            Area {
              x: i32::wrapping_sub(right, 64),
              y: i32::wrapping_add(area.y, 6),
              w: 58,
              h: i32::wrapping_sub(area.h, 12),
            },
            "Clear",
            Action::ClearSearch,
            Button::Quiet,
          );
        }
      }
    };

    Ok(())
  }

  fn notice(
    &mut self,
    area: Area,
    notice: &Notice,
    kind: NoticeKind,
  ) -> core::result::Result<(), core::convert::Infallible> {
    {
      let Ok(()) = self.panel(area, self.colors.ground, 10, Some(self.colors.panel));
      let buttons = match bool::then_some(kind == NoticeKind::Search, ()) {
        Some(()) => 154,
        None => match bool::then_some(notice.error.is_some(), ()) {
          Some(()) => 160,
          None => 80,
        },
      };
      let words = i32::wrapping_sub(i32::wrapping_sub(area.w, buttons), 20).max(70);
      let title_at = match bool::then_some(notice.detail.is_some(), ()) {
        Some(()) => i32::wrapping_add(area.y, 6),
        None => i32::wrapping_add(area.y, 16),
      };
      let Ok(()) = self.text(
        Area {
          x: i32::wrapping_add(area.x, 13),
          y: title_at,
          w: words,
          h: 22,
        },
        &notice.title,
        15,
        Face::Bold,
        self.colors.text,
        Lines(1),
      );

      match &notice.detail {
        Some(detail) => {
          let Ok(()) = self.text(
            Area {
              x: i32::wrapping_add(area.x, 13),
              y: i32::wrapping_add(area.y, 29),
              w: words,
              h: 20,
            },
            detail,
            12,
            Face::Plain,
            self.colors.soft,
            Lines(1),
          );
        }
        None => {}
      }

      match bool::then_some(notice.error.is_some(), ()) {
        Some(()) => {
          let Ok(right) = area.right();
          let Ok(()) = self.button(
            Area {
              x: i32::wrapping_sub(right, 148),
              y: i32::wrapping_add(area.y, 11),
              w: 74,
              h: 34,
            },
            "Details",
            match bool::then_some(kind == NoticeKind::Search, ()) {
              Some(()) => Action::SearchDetails,
              None => Action::ErrorDetails,
            },
            Button::Normal,
          );
        }
        None => {}
      }

      match bool::then_some(kind == NoticeKind::Search, ()) {
        Some(()) => {
          let Ok(right) = area.right();
          let Ok(()) = self.button(
            Area {
              x: i32::wrapping_sub(right, 66),
              y: i32::wrapping_add(area.y, 11),
              w: 58,
              h: 34,
            },
            "Retry",
            Action::RetrySearch,
            Button::Quiet,
          );
        }
        None => match bool::then_some(
          self.app.profile_status != crate::ProfileStatus::Changing,
          (),
        ) {
          Some(()) => {
            let Ok(right) = area.right();
            let Ok(()) = self.button(
              Area {
                x: i32::wrapping_sub(right, 66),
                y: i32::wrapping_add(area.y, 11),
                w: 58,
                h: 34,
              },
              "Close",
              Action::Dismiss,
              Button::Quiet,
            );
          }
          None => {}
        },
      }
    };

    Ok(())
  }

  fn hero(&mut self, area: Area) -> core::result::Result<(), core::convert::Infallible> {
    {
      let Ok(row) = crate::catalogue::package("obs-studio");
      let Ok(()) = self.panel(area, self.colors.ground, 0, None);
      let Ok(()) = self.picture(area);
      let x = i32::wrapping_add(area.x, 23);
      let width = i32::wrapping_div(i32::wrapping_mul(area.w, 53), 100).clamp(145, 340);
      let Ok(()) = self.text(
        Area {
          x,
          y: i32::wrapping_add(area.y, 31),
          w: width,
          h: 25,
        },
        "Screen recording & streaming",
        13,
        Face::Bold,
        self.theme.lilac,
        Lines(1),
      );
      let Ok(()) = self.text(
        Area {
          x,
          y: i32::wrapping_add(area.y, 57),
          w: width,
          h: 62,
        },
        &row.title,
        match area.w.cmp(&610) {
          Ordering::Greater => 47,
          Ordering::Less | Ordering::Equal => 33,
        },
        Face::Bold,
        self.theme.white,
        Lines(1),
      );
      let Ok(()) = self.text(
        Area {
          x,
          y: i32::wrapping_add(area.y, 124),
          w: width,
          h: 48,
        },
        &row.description,
        15,
        Face::Plain,
        self.theme.white,
        Lines(2),
      );
      let Ok(local) = self.app.local(&row);
      let label = match bool::then_some(local.is_some(), ()) {
        Some(()) => "View installed app",
        None => "View OBS Studio  →",
      };
      let Ok(()) = self.button(
        Area {
          x,
          y: i32::wrapping_add(area.y, 181),
          w: 170,
          h: 41,
        },
        label,
        Action::Featured,
        Button::Primary,
      );
    };

    Ok(())
  }

  fn card(
    &mut self,
    area: Area,
    item: &Package,
    index: u32,
    view: StoreView,
  ) -> core::result::Result<(), core::convert::Infallible> {
    {
      let action = Action::Details(index);
      let active = self.app.focus == Some(action) || self.app.hover == Some(action);
      let fill = match bool::then_some(active, ()) {
        Some(()) => self.colors.panel,
        None => self.colors.ground,
      };
      let Ok(()) = self.panel(
        area,
        fill,
        11,
        Some(match bool::then_some(active, ()) {
          Some(()) => self.colors.pink,
          None => self.colors.panel,
        }),
      );
      let icon = Area {
        x: i32::wrapping_add(area.x, 13),
        y: i32::wrapping_add(area.y, 13),
        w: 42,
        h: 42,
      };
      let Ok(()) = self.panel(icon, self.colors.panel, 9, None);
      let Ok(()) = self.text(
        Area {
          x: i32::wrapping_add(icon.x, 12),
          y: icon.y,
          w: 25,
          h: 42,
        },
        {
          let Ok(explicit_value) = item.category.mark();
          explicit_value
        },
        18,
        Face::Bold,
        self.colors.pink,
        Lines(1),
      );
      let Ok(()) = self.card_words(area, item, view);
      let Ok(()) = self.hit(area, action);
    };

    Ok(())
  }

  fn card_words(
    &mut self,
    area: Area,
    item: &Package,
    view: StoreView,
  ) -> Result<(), core::convert::Infallible> {
    let (left, top, right, description_height, description_lines) = match view {
      StoreView::Browse => (13, 61, 13, 39, Lines(2)),
      StoreView::Installed | StoreView::Search => (69, 7, 52, 26, Lines(1)),
    };
    let x = area.x.wrapping_add(left);
    let y = area.y.wrapping_add(top);
    let w = area.w.wrapping_sub(left).wrapping_sub(right);
    let Ok(()) = self.text(
      Area { x, y, w, h: 24 },
      &item.title,
      17,
      Face::Bold,
      self.colors.text,
      Lines(1),
    );
    let description = match item.description.is_empty().then_some(()) {
      Some(()) => &item.attribute,
      None => &item.description,
    };
    let Ok(()) = self.text(
      Area {
        x,
        y: y.wrapping_add(28),
        w,
        h: description_height,
      },
      description,
      13,
      Face::Plain,
      self.colors.soft,
      description_lines,
    );
    let Ok(installed) = self.app.local(item);
    let mark = match installed {
      Some(_) => "✓",
      None => "→",
    };
    let Ok(card_right) = area.right();
    let Ok(()) = self.text(
      Area {
        x: card_right.wrapping_sub(37),
        y: area.y.wrapping_add(13),
        w: 28,
        h: 42,
      },
      mark,
      19,
      Face::Plain,
      self.colors.pink,
      Lines(1),
    );
    Ok(())
  }

  fn overlay(
    &mut self,
    whole: Area,
    overlay: &Overlay,
  ) -> core::result::Result<(), core::convert::Infallible> {
    {
      self.frame.hits.clear();
      let Ok(()) = self.clip(whole);
      let Ok(()) = self.panel(whole, self.colors.night, 0, None);
      let width = i32::wrapping_sub(whole.w, 38).clamp(240, 630);
      let height = match overlay {
        Overlay::Error { .. } | Overlay::Help => 335,
        Overlay::Package(_) | Overlay::Remove(_) => 267,
      };
      let height_2 = height.min(i32::wrapping_sub(whole.h, 24));
      let area = Area {
        x: i32::wrapping_div(i32::wrapping_sub(whole.w, width), 2),
        y: i32::wrapping_div(i32::wrapping_sub(whole.h, height_2), 2),
        w: width,
        h: height_2,
      };
      let Ok(()) = self.panel(area, self.colors.ground, 15, Some(self.colors.panel));
      let Ok(content) = area.inset(24);
      let c = self.colors;
      let Ok(bottom) = content.bottom();
      let controls = i32::wrapping_sub(bottom, 40);
      let mut heading = Area {
        x: content.x,
        y: content.y,
        w: i32::wrapping_sub(content.w, 72),
        h: 42,
      };
      let Ok(right) = content.right();
      let Ok(()) = self.button(
        Area {
          x: i32::wrapping_sub(right, 67),
          y: content.y,
          w: 67,
          h: 38,
        },
        "Back",
        Action::Back,
        Button::Quiet,
      );

      match overlay {
        Overlay::Package(item) => {
          let Ok(()) = self.text(heading, &item.title, 26, Face::Bold, c.text, Lines(1));
          let version = match bool::then_some(item.version.is_empty(), ()) {
            Some(()) => item.attribute.clone(),
            None => {
              format!("{}  ·  Version {}", item.attribute, item.version)
            }
          };
          let Ok(()) = self.text(
            Area {
              x: content.x,
              y: i32::wrapping_add(content.y, 46),
              w: content.w,
              h: 22,
            },
            &version,
            13,
            Face::Mono,
            c.soft,
            Lines(1),
          );
          let Ok(()) = self.text(
            Area {
              x: content.x,
              y: i32::wrapping_add(content.y, 83),
              w: content.w,
              h: 74,
            },
            &item.description,
            16,
            Face::Plain,
            c.text,
            Lines(3),
          );

          let actions = Area {
            y: controls,
            ..content
          };
          let Ok(()) = self.package_actions(actions, item);
        }
        Overlay::Remove(item) => {
          let Ok(()) = self.text(
            heading,
            &format!("Remove {}?", item.package.title),
            25,
            Face::Bold,
            c.text,
            Lines(1),
          );
          let Ok(()) = self.text(
            Area {
              x: content.x,
              y: i32::wrapping_add(content.y, 73),
              w: content.w,
              h: 52,
            },
            "You can install this package again from Browse or Search.",
            16,
            Face::Plain,
            c.soft,
            Lines(2),
          );
          let Ok(()) = self.button(
            Area {
              x: content.x,
              y: controls,
              w: 148,
              h: 41,
            },
            "Remove package",
            Action::ConfirmRemove,
            Button::Primary,
          );
          let Ok(()) = self.button(
            Area {
              x: i32::wrapping_add(content.x, 160),
              y: controls,
              w: 85,
              h: 41,
            },
            "Cancel",
            Action::Back,
            Button::Normal,
          );
        }
        Overlay::Error { summary, text } => {
          heading.h = 44;
          let Ok(()) = self.text(heading, summary, 19, Face::Bold, c.text, Lines(2));
          let report = Area {
            x: content.x,
            y: i32::wrapping_add(content.y, 57),
            w: content.w,
            h: i32::wrapping_sub(content.h, 57),
          };
          let Ok(()) = self.panel(report, c.night, 8, None);
          let Ok(room) = report.inset(11);
          let lines = i32::cast_unsigned(i32::wrapping_div(room.h, 17).max(1));
          let Ok(()) = self.text(room, text, 12, Face::Mono, c.soft, Lines(lines));
        }
        Overlay::Help => {
          let Ok(()) = self.text(heading, "Using the store", 25, Face::Bold, c.text, Lines(1));

          let notes = [
            (
              "Find an app",
              "Search by name or what you want to do. Select a result to see its details.",
            ),
            (
              "Install and open",
              "Select Install, then look under Installed to open or remove the app.",
            ),
            (
              "Keyboard",
              "Ctrl+K searches. Tab moves between controls. Enter activates. Esc goes back.",
            ),
          ];
          for (position, (label, detail)) in (0_i32..3).zip(notes) {
            let y = content
              .y
              .wrapping_add(55)
              .wrapping_add(position.wrapping_mul(72));
            let Ok(()) = self.text(
              Area {
                x: content.x,
                y,
                w: content.w,
                h: 22,
              },
              label,
              16,
              Face::Bold,
              c.pink,
              Lines(1),
            );
            let Ok(()) = self.text(
              Area {
                x: content.x,
                y: i32::wrapping_add(y, 26),
                w: content.w,
                h: 46,
              },
              detail,
              15,
              Face::Plain,
              c.soft,
              Lines(2),
            );
          }
        }
      }
    };

    Ok(())
  }

  fn package_actions(
    &mut self,
    area: Area,
    item: &Package,
  ) -> Result<(), core::convert::Infallible> {
    let c = self.colors;
    let Ok(installed) = self.app.local(item);

    match installed {
      Some(local) => {
        let Ok(launch_label) = local.launch_label();

        match launch_label.filter(|_| self.app.launching == crate::LaunchStatus::Idle) {
          Some(label) => {
            let Ok(()) = self.button(
              Area {
                x: area.x,
                y: area.y,
                w: 151,
                h: 41,
              },
              label,
              Action::Open,
              Button::Primary,
            );
          }
          None => {
            let Ok(()) = self.text(
              Area {
                x: area.x,
                y: area.y,
                w: 151,
                h: 41,
              },
              "Installed",
              15,
              Face::Bold,
              c.pink,
              Lines(1),
            );
          }
        }

        match bool::then_some(
          self.app.can_change() == Ok(crate::ProfileAvailability::Ready),
          (),
        ) {
          Some(()) => {
            let Ok(()) = self.button(
              Area {
                x: i32::wrapping_add(area.x, 164),
                y: area.y,
                w: 98,
                h: 41,
              },
              "Remove",
              Action::Remove,
              Button::Normal,
            );
          }
          None => {}
        }
      }
      None => match self.app.can_change() {
        Ok(crate::ProfileAvailability::Ready) => {
          let Ok(()) = self.button(
            Area {
              x: area.x,
              y: area.y,
              w: 97,
              h: 41,
            },
            "Install",
            Action::Install,
            Button::Primary,
          );
          let Ok(()) = self.text(
            Area {
              x: i32::wrapping_add(area.x, 112),
              y: area.y,
              w: i32::wrapping_sub(area.w, 112),
              h: 41,
            },
            "Installs for your user",
            14,
            Face::Plain,
            c.soft,
            Lines(1),
          );
        }
        Ok(crate::ProfileAvailability::Unavailable) => {
          let message = match self.app.nix_available {
            Installed::Yes => {
              "Install is unavailable while the store is busy or the profile needs attention."
            }
            Installed::No => "Install Nix, then reopen the store.",
          };
          let Ok(()) = self.text(
            Area {
              x: area.x,
              y: area.y,
              w: area.w,
              h: 45,
            },
            message,
            14,
            Face::Plain,
            c.soft,
            Lines(2),
          );
        }
      },
    }

    Ok(())
  }
}

#[derive(Clone, Copy)]
enum Button {
  Primary,
  Normal,
  Quiet,
  Selected,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StoreView {
  Browse,
  Installed,
  Search,
}

impl StoreView {
  fn current(shop: &Shop) -> Result<Self, core::convert::Infallible> {
    let Ok(query) = shop.query();
    Ok(match query.trim() {
      "" => match shop.page {
        Page::Browse => Self::Browse,
        Page::Installed => Self::Installed,
      },
      _ => Self::Search,
    })
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NoticeKind {
  Search,
  Store,
}

#[must_use]
pub fn draw(
  shop: &Shop,
  room: Size<u32>,
  theme: &Theme,
) -> Result<Frame, core::convert::Infallible> {
  let root = Area {
    x: 0,
    y: 0,
    w: u32::cast_signed(room.width),
    h: u32::cast_signed(room.height),
  };
  let mut scene = Scene {
    app: shop,
    theme,
    colors: theme.colors,
    clip: root,
    frame: Frame::default(),
  };

  match &shop.overlay {
    Some(overlay) => {
      let Ok(()) = scene.overlay(root, overlay);
    }
    None => {
      let Ok(view) = StoreView::current(shop);
      let Ok(rows) = shop.rows();
      let Ok(content) = scene.storefront(root, view);
      let Ok(headed) = scene.results_header(content, view, &rows);
      let Ok(with_notices) = scene.results_notices(headed, view);
      let Ok(cards) = scene.results_intro(with_notices, view, &rows);
      let Ok(bottom) = scene.package_cards(cards, view, &rows);
      let Ok(()) = scene.scrollbar(root, bottom);
    }
  }

  Ok(scene.frame)
}

impl Scene<'_> {
  fn storefront(
    &mut self,
    root: Area,
    view: StoreView,
  ) -> Result<Area, core::convert::Infallible> {
    let Ok(()) = self.panel(root, self.colors.night, 0, None);
    let (side, margin) = match root.w.cmp(&820) {
      Ordering::Less => (152_i32, 18),
      Ordering::Equal | Ordering::Greater => (200_i32, 28),
    };
    let Ok(()) = self.sidebar(Area { w: side, ..root }, view);
    let x = side.wrapping_add(margin);
    let w = root.w.wrapping_sub(x).wrapping_sub(margin).max(1);
    let right = x.wrapping_add(w);
    let Ok(()) = self.search_bar(Area {
      x,
      y: 21,
      w: w.wrapping_sub(47).min(640),
      h: 44,
    });
    let Ok(()) = self.button(
      Area {
        x: right.wrapping_sub(38),
        y: 23,
        w: 38,
        h: 39,
      },
      "×",
      Action::Close,
      Button::Quiet,
    );

    let mut top = 86_i32;

    match &self.app.notice {
      Some(notice) => {
        let Ok(()) = self.notice(
          Area {
            x,
            y: top,
            w,
            h: 56,
          },
          notice,
          NoticeKind::Store,
        );
        top = top.wrapping_add(66);
      }
      None => {}
    }

    let viewport = Area {
      x: side.wrapping_add(1),
      y: top,
      w: root.w.wrapping_sub(side).wrapping_sub(1),
      h: root.h.wrapping_sub(top),
    };
    self.frame.scroll_area = viewport;
    let Ok(()) = self.clip(viewport);
    let y = top.wrapping_add(4).wrapping_sub(self.app.scroll);
    Ok(Area {
      x,
      y,
      w,
      h: viewport.h,
    })
  }

  fn results_header(
    &mut self,
    area: Area,
    view: StoreView,
    rows: &[Package],
  ) -> Result<Area, core::convert::Infallible> {
    let (title, subtitle) = match view {
      StoreView::Browse => (
        "Browse apps",
        "Discover desktop apps and command-line tools from Nixpkgs.".to_owned(),
      ),
      StoreView::Installed => (
        "Installed",
        "Apps and tools you installed with the store.".to_owned(),
      ),
      StoreView::Search => {
        let Ok(subtitle) = self.search_subtitle(rows);
        ("Search results", subtitle)
      }
    };
    let Ok(()) = self.text(
      Area { h: 48, ..area },
      title,
      34,
      Face::Bold,
      self.colors.text,
      Lines(1),
    );
    let Ok(()) = self.text(
      Area {
        y: area.y.wrapping_add(49),
        h: 28,
        ..area
      },
      &subtitle,
      15,
      Face::Plain,
      self.colors.soft,
      Lines(1),
    );
    Ok(Area {
      y: area.y.wrapping_add(88),
      ..area
    })
  }

  fn search_subtitle(&self, rows: &[Package]) -> Result<String, core::convert::Infallible> {
    let Ok(status) = self.app.searching();

    match status {
      SearchStatus::Searching => return Ok("Searching Nixpkgs…".to_owned()),
      SearchStatus::Settled => {}
    }

    let Ok(visible) = fitted::<_, u32>(rows.len());
    let total = self.app.match_count;
    Ok(match total.cmp(&visible) {
      Ordering::Greater => format!(
        "Showing {visible} of {total} packages. Use a narrower search for more specific results."
      ),
      Ordering::Equal | Ordering::Less => match (total, rows) {
        (0, []) => "Search by name or by what you want to do.".to_owned(),
        (0, _) => "Featured matches".to_owned(),
        _ => format!("{total} packages found"),
      },
    })
  }

  fn results_notices(
    &mut self,
    area: Area,
    view: StoreView,
  ) -> Result<Area, core::convert::Infallible> {
    let mut at = area;

    match self.app.nix_available {
      Installed::No => {
        let Ok(()) = self.nix_notice(at);
        at.y = at.y.wrapping_add(77);
      }
      Installed::Yes => {}
    }

    let search_notice = self
      .app
      .search_problem
      .as_ref()
      .filter(|_| view == StoreView::Search);

    match search_notice {
      Some(problem) => {
        let Ok(()) = self.notice(Area { h: 56, ..at }, problem, NoticeKind::Search);
        at.y = at.y.wrapping_add(67);
      }
      None => {}
    }

    Ok(at)
  }

  fn nix_notice(&mut self, area: Area) -> Result<(), core::convert::Infallible> {
    let Ok(()) = self.panel(Area { h: 66, ..area }, self.colors.ground, 9, None);
    let inset = Area {
      x: area.x.wrapping_add(14),
      w: area.w.wrapping_sub(28),
      ..area
    };
    let Ok(()) = self.text(
      Area {
        y: inset.y.wrapping_add(8),
        h: 21,
        ..inset
      },
      "Nix is needed to install packages",
      15,
      Face::Bold,
      self.colors.text,
      Lines(1),
    );
    let Ok(()) = self.text(
      Area {
        y: inset.y.wrapping_add(32),
        h: 22,
        ..inset
      },
      "Install Nix and reopen the store to search the full catalogue.",
      13,
      Face::Plain,
      self.colors.soft,
      Lines(1),
    );
    Ok(())
  }

  fn results_intro(
    &mut self,
    area: Area,
    view: StoreView,
    rows: &[Package],
  ) -> Result<Area, core::convert::Infallible> {
    match (view, rows) {
      (StoreView::Browse, _) => self.browse_categories(area),
      (StoreView::Search | StoreView::Installed, []) => self.empty_results(area, view),
      (StoreView::Search | StoreView::Installed, _) => Ok(area),
    }
  }

  fn browse_categories(&mut self, area: Area) -> Result<Area, core::convert::Infallible> {
    let mut at = area;

    match self.app.category {
      Category::All => {
        let Ok(()) = self.hero(Area { h: 246, ..at });
        at.y = at.y.wrapping_add(261);
      }
      Category::Media | Category::Creative | Category::Everyday | Category::Developer => {}
    }

    let Ok(()) = self.text(
      Area { h: 31, ..at },
      "Featured apps",
      22,
      Face::Bold,
      self.colors.text,
      Lines(1),
    );
    at.y = at.y.wrapping_add(42);
    let mut left = at.x;
    let Ok(right) = at.right();

    for (category, width) in Category::EVERY.into_iter().zip([77, 73, 91, 92, 102]) {
      match (left.wrapping_add(width) > right && left != at.x).then_some(()) {
        Some(()) => {
          left = at.x;
          at.y = at.y.wrapping_add(44);
        }
        None => {}
      }

      let Ok(label) = category.title();
      let kind = match (self.app.category == category).then_some(()) {
        Some(()) => Button::Selected,
        None => Button::Normal,
      };
      let Ok(()) = self.button(
        Area {
          x: left,
          w: width,
          h: 37,
          ..at
        },
        label,
        Action::Category(category),
        kind,
      );
      left = left.wrapping_add(width.wrapping_add(7));
    }

    at.y = at.y.wrapping_add(51);
    Ok(at)
  }

  fn empty_results(
    &mut self,
    area: Area,
    view: StoreView,
  ) -> Result<Area, core::convert::Infallible> {
    let Ok((heading, detail, action)) = self.empty_message(view);
    let c = self.colors;
    let Ok(()) = self.panel(Area { h: 92, ..area }, c.ground, 10, Some(c.panel));
    let inset = Area {
      x: area.x.wrapping_add(17),
      w: area.w.wrapping_sub(34),
      ..area
    };
    let Ok(()) = self.text(
      Area {
        y: inset.y.wrapping_add(13),
        h: 25,
        ..inset
      },
      heading,
      17,
      Face::Bold,
      c.text,
      Lines(1),
    );
    let Ok(()) = self.text(
      Area {
        y: inset.y.wrapping_add(47),
        h: 22,
        ..inset
      },
      detail,
      14,
      Face::Plain,
      c.soft,
      Lines(1),
    );
    let mut next = Area {
      y: area.y.wrapping_add(105),
      ..area
    };

    match action {
      Some((label, activated)) => {
        let Ok(()) = self.button(
          Area {
            w: 95,
            h: 39,
            ..next
          },
          label,
          activated,
          Button::Normal,
        );
        next.y = next.y.wrapping_add(52);
      }
      None => {}
    }

    Ok(next)
  }

  fn empty_message(
    &self,
    view: StoreView,
  ) -> Result<
    (&'static str, &'static str, Option<(&'static str, Action)>),
    core::convert::Infallible,
  > {
    let Ok(search) = self.app.searching();
    let Ok(profile) = self.app.availability();
    let unavailable = (self.app.nix_available, profile);
    let browse = Some(("Browse", Action::Page(Page::Browse)));
    Ok(
      match (
        view,
        search,
        self.app.nix_available,
        &self.app.search_problem,
      ) {
        (StoreView::Search, SearchStatus::Searching, _, _) => (
          "Looking for packages",
          "The first search can take a little longer.",
          None,
        ),
        (StoreView::Search, SearchStatus::Settled, Installed::No, _)
        | (StoreView::Search, SearchStatus::Settled, Installed::Yes, Some(_)) => (
          "No featured matches",
          "Try a different name or browse the featured apps.",
          None,
        ),
        (StoreView::Search, SearchStatus::Settled, Installed::Yes, None) => (
          "No packages found",
          "Try a shorter name or a word like video or paint.",
          None,
        ),
        (StoreView::Installed | StoreView::Browse, _, _, _) => {
          match (self.app.profile_status, unavailable) {
            (ProfileStatus::Reading(_), _) => (
              "Reading installed packages",
              "the store is loading your library.",
              None,
            ),
            (_, (Installed::Yes, ProfileAvailability::Unavailable)) => (
              "Installed packages are unavailable",
              "Refresh the list after checking Nix.",
              Some(("Refresh", Action::Refresh)),
            ),
            _ => (
              "Your shelf is empty",
              "Choose an app from Browse or Search to install it.",
              browse,
            ),
          }
        }
      },
    )
  }

  fn package_cards(
    &mut self,
    area: Area,
    view: StoreView,
    rows: &[Package],
  ) -> Result<i32, core::convert::Infallible> {
    let columns = match (view, area.w) {
      (StoreView::Browse, 700..) => 3_i32,
      (StoreView::Browse, 360..=699) => 2,
      _ => 1,
    };
    let column_count = match NonZeroI32::new(columns) {
      Some(count) => count,
      None => return Ok(area.y),
    };
    let gap = 10_i32;
    let width = area
      .w
      .wrapping_sub(gap.wrapping_mul(columns.wrapping_sub(1)))
      .wrapping_div(column_count.get());
    let height = match view {
      StoreView::Browse => 132_i32,
      StoreView::Installed | StoreView::Search => 76,
    };
    let mut count = 0_i32;

    for (index, item) in rows.iter().enumerate() {
      match (view, self.app.category, item.attribute.as_str()) {
        (StoreView::Browse, Category::All, "obs-studio") => continue,
        _ => {}
      }

      let x = count.wrapping_rem(column_count.get());
      let y = count.wrapping_div(column_count.get());
      let card = Area {
        x: area.x.wrapping_add(x.wrapping_mul(width.wrapping_add(gap))),
        y: area
          .y
          .wrapping_add(y.wrapping_mul(height.wrapping_add(gap))),
        w: width,
        h: height,
      };
      let Ok(inside) = card.inside(self.frame.scroll_area);

      match inside {
        Some(_) => {
          let Ok(position) = fitted::<_, u32>(index);
          let Ok(()) = self.card(card, item, position, view);
        }
        None => {}
      }

      count = count.wrapping_add(1);
    }

    let occupied_rows = count
      .wrapping_add(columns)
      .wrapping_sub(1)
      .wrapping_div(column_count.get());
    Ok(
      area
        .y
        .wrapping_add(occupied_rows.wrapping_mul(height.wrapping_add(gap))),
    )
  }

  fn scrollbar(&mut self, root: Area, bottom: i32) -> Result<(), core::convert::Infallible> {
    self.frame.max_scroll = bottom
      .wrapping_add(self.app.scroll)
      .wrapping_add(14)
      .wrapping_sub(root.h)
      .max(0);
    self.frame.shapes.push(Shape::Clip(Clip::Lifted));
    let viewport = self.frame.scroll_area;
    let scroll_range = NonZeroI32::new(self.frame.max_scroll).filter(|_| viewport.h > 0);
    let maximum = match scroll_range {
      Some(maximum) => maximum,
      None => return Ok(()),
    };

    let Ok(()) = self.clip(root);
    let track = viewport.h.saturating_sub(12).max(1);
    let content = match NonZeroI32::new(track.saturating_add(maximum.get())) {
      Some(content) => content,
      None => return Ok(()),
    };
    let visible = track
      .wrapping_mul(track)
      .wrapping_div(content.get())
      .max(22)
      .min(track);
    let scroll = self.app.scroll.min(maximum.get());
    let offset = track
      .wrapping_sub(visible)
      .wrapping_mul(scroll)
      .wrapping_div(maximum.get());
    let Ok(right) = root.right();
    let Ok(()) = self.panel(
      Area {
        x: right.wrapping_sub(8),
        y: viewport.y.wrapping_add(6).wrapping_add(offset),
        w: 3,
        h: visible,
      },
      self.colors.edge,
      1,
      None,
    );
    Ok(())
  }
}
