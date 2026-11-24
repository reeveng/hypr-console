//! Layout for a searchable catalogue and its scrolling entries.

use std::cmp::Ordering;
use std::num::NonZeroI32;

use console_core_color::{palette::Wearing, to_oklch};
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_shapes::Pixels;
use console_draw_painting::{Lines, Run, measure_text};
use console_draw_surface::Part;

use crate::canvas::{Area, AreaLayout, Face, Interaction};

use super::{Button, Components, Frame, Link, Message, Notice, window};

struct Scroll {
  bottom: i32,
  offset: i32,
}

enum NoticePlacement {
  Header,
  Results,
}

pub struct Sidebar<'a, A> {
  pub title: &'a str,
  pub narrow_title: &'a str,
  pub mark: &'a str,
  pub pages: &'a [Link<'a, A>],
  pub selected: Option<A>,
  pub footer: Link<'a, A>,
}

pub struct Search<'a, A> {
  pub edit: A,
  pub clear: Link<'a, A>,
  pub placeholder: &'a str,
  pub shortcut: &'a str,
  pub before_caret: &'a str,
  pub after_caret: &'a str,
  pub preedit: &'a str,
}

pub struct Feature<'a, A> {
  pub art: &'a Pixels,
  pub eyebrow: &'a str,
  pub title: &'a str,
  pub description: &'a str,
  pub link: Link<'a, A>,
}

pub enum Introduction<'a, A> {
  Featured {
    banner: Option<Feature<'a, A>>,
    title: &'a str,
    filters: &'a [(Link<'a, A>, i32)],
    selected: A,
  },
  Empty {
    message: Message<'a>,
    link: Option<Link<'a, A>>,
  },
  None,
}

#[derive(Clone, Copy)]
pub enum EntryLayout {
  Tiles,
  List,
}

pub struct Entry<'a, A> {
  pub title: &'a str,
  pub detail: &'a str,
  pub icon: &'a str,
  pub indicator: &'a str,
  pub action: A,
}

pub struct Catalogue<'a, A> {
  pub sidebar: Sidebar<'a, A>,
  pub search: Search<'a, A>,
  pub close: Link<'a, A>,
  pub notice: Option<Notice<'a, A>>,
  pub title: &'a str,
  pub subtitle: &'a str,
  pub advisory: Option<Message<'a>>,
  pub problem: Option<Notice<'a, A>>,
  pub introduction: Introduction<'a, A>,
  pub layout: EntryLayout,
  pub scroll: i32,
}

#[must_use]
pub fn catalogue<'a, A: Copy + PartialEq, Row>(
  view: &Catalogue<'_, A>,
  room: Size<u32>,
  colors: Wearing,
  interaction: Interaction<A>,
  rows: impl IntoIterator<Item = Row>,
  describe: impl Fn(Row) -> Entry<'a, A>,
) -> Result<Frame<A>, Never> {
  let Ok(root) = window(room);
  let Ok(mut components) = Components::new(root, colors, interaction);
  let Ok((viewport, content, caret)) = components.catalogue_header(root, view);
  let Ok(headed) = components.results_header(content, view);
  let Ok(with_notices) = components.results_notices(headed, view);
  let Ok(cards) = components.introduction(with_notices, &view.introduction);
  let Ok(bottom) = components.entries(cards, viewport, view.layout, rows, describe);
  let Ok(()) = components.scrollbar(
    root,
    viewport,
    Scroll {
      bottom,
      offset: view.scroll,
    },
  );
  Ok(Frame {
    canvas: components.canvas.frame,
    caret,
    scroll_area: viewport,
  })
}

impl<A: Copy + PartialEq> Components<A> {
  fn catalogue_header(
    &mut self,
    root: Area,
    view: &Catalogue<'_, A>,
  ) -> Result<(Area, Area, Option<Part>), Never> {
    let colors = self.colors;
    let Ok(()) = self.canvas.panel(root, colors.night, 0, None);
    let (side, margin) = match root.size.width.cmp(&820) {
      Ordering::Less => (152_i32, 18),
      Ordering::Equal | Ordering::Greater => (200_i32, 28),
    };
    let sidebar = Area {
      origin: Point { x: 0, y: 0 },
      size: Size {
        width: side,
        height: root.size.height,
      },
    };
    let Ok(()) = self.sidebar(sidebar, &view.sidebar);
    let x = side.wrapping_add(margin);
    let width = root.size.width.wrapping_sub(x).wrapping_sub(margin).max(1);
    let right = x.wrapping_add(width);
    let search_area = Area {
      origin: Point { x, y: 21 },
      size: Size {
        width: width.wrapping_sub(47).min(640),
        height: 44,
      },
    };
    let Ok(caret) = self.search(search_area, &view.search);
    let close = Area {
      origin: Point {
        x: right.wrapping_sub(38),
        y: 23,
      },
      size: Size {
        width: 38,
        height: 39,
      },
    };
    let Ok(()) = self.button(close, view.close, Button::Quiet);

    let top = match &view.notice {
      Some(notice) => {
        let area = Area {
          origin: Point { x, y: 86 },
          size: Size { width, height: 56 },
        };
        let Ok(()) = self.notice(area, notice, NoticePlacement::Header);
        152
      }
      None => 86,
    };

    let viewport = Area {
      origin: Point {
        x: side.wrapping_add(1),
        y: top,
      },
      size: Size {
        width: root.size.width.wrapping_sub(side).wrapping_sub(1),
        height: root.size.height.wrapping_sub(top),
      },
    };
    let Ok(()) = self.canvas.clip(viewport);
    let y = top.wrapping_add(4).wrapping_sub(view.scroll);
    let content = Area {
      origin: Point { x, y },
      size: Size {
        width,
        height: viewport.size.height,
      },
    };
    Ok((viewport, content, caret))
  }

  fn sidebar(&mut self, area: Area, sidebar: &Sidebar<'_, A>) -> Result<(), Never> {
    let colors = self.colors;
    let width = area.size.width;
    let Ok(()) = self.canvas.panel(area, colors.ground, 0, None);
    let mark = Area {
      origin: Point { x: 18, y: 20 },
      size: Size {
        width: 37,
        height: 37,
      },
    };
    let Ok(()) = self.canvas.panel(mark, colors.pink, 10, None);
    let glyph = Area {
      origin: Point { x: 28, y: 20 },
      size: Size {
        width: 27,
        height: 36,
      },
    };
    let Ok(()) = self.text(
      glyph,
      sidebar.mark,
      22,
      Face::Medium,
      colors.night,
      Lines(1),
    );
    let (title, lines) = match width.cmp(&170) {
      Ordering::Less => (sidebar.narrow_title, Lines(2)),
      Ordering::Equal | Ordering::Greater => (sidebar.title, Lines(1)),
    };
    let heading = Area {
      origin: Point { x: 64, y: 20 },
      size: Size {
        width: width.wrapping_sub(66),
        height: 38,
      },
    };
    let Ok(()) = self.text(heading, title, 16, Face::Medium, colors.text, lines);

    for (position, link) in (0_i32..).zip(sidebar.pages) {
      let kind = match (sidebar.selected == Some(link.action)).then_some(()) {
        Some(()) => Button::Selected,
        None => Button::Quiet,
      };
      let y = 93_i32.wrapping_add(50_i32.wrapping_mul(position));
      let page = Area {
        origin: Point { x: 14, y },
        size: Size {
          width: width.wrapping_sub(28),
          height: 43,
        },
      };
      let Ok(()) = self.button(page, *link, kind);
    }

    let Ok(bottom) = area.bottom();
    let footer = Area {
      origin: Point {
        x: 14,
        y: bottom.wrapping_sub(59),
      },
      size: Size {
        width: width.wrapping_sub(28),
        height: 40,
      },
    };
    self.button(footer, sidebar.footer, Button::Quiet)
  }

  fn search(&mut self, area: Area, search: &Search<'_, A>) -> Result<Option<Part>, Never> {
    let colors = self.colors;
    let focus = self.canvas.interaction.focus == Some(search.edit);
    let outline = match focus.then_some(()) {
      Some(()) => colors.pink,
      None => colors.panel,
    };
    let Ok(()) = self.canvas.panel(area, colors.ground, 10, Some(outline));
    let (x, y) = (area.origin.x, area.origin.y);
    let icon = Area {
      origin: Point {
        x: x.wrapping_add(12),
        y,
      },
      size: Size {
        width: 27,
        height: area.size.height,
      },
    };
    let Ok(()) = self.text(icon, "⌕", 21, Face::Medium, colors.soft, Lines(1));
    let line = Area {
      origin: Point {
        x: x.wrapping_add(39),
        y,
      },
      size: Size {
        width: area.size.width.wrapping_sub(111).max(1),
        height: area.size.height,
      },
    };
    let empty = search.before_caret.is_empty() && search.after_caret.is_empty();
    let Ok(caret) = self.search_text(
      line,
      search,
      (empty && search.preedit.is_empty() && !focus).then_some(search.placeholder),
    );
    let Ok(right) = area.right();

    match empty.then_some(()) {
      Some(()) => {
        let hint = Area {
          origin: Point {
            x: right.wrapping_sub(66),
            y,
          },
          size: Size {
            width: 57,
            height: area.size.height,
          },
        };
        let Ok(()) = self.text(hint, search.shortcut, 12, Face::Mono, colors.soft, Lines(1));
        let Ok(()) = self.canvas.hit(area, search.edit);
      }
      None => {
        let edit = Area {
          origin: Point { x, y },
          size: Size {
            width: area.size.width.wrapping_sub(68),
            height: area.size.height,
          },
        };
        let Ok(()) = self.canvas.hit(edit, search.edit);
        let clear = Area {
          origin: Point {
            x: right.wrapping_sub(64),
            y: y.wrapping_add(6),
          },
          size: Size {
            width: 58,
            height: area.size.height.wrapping_sub(12),
          },
        };
        let Ok(()) = self.button(clear, search.clear, Button::Quiet);
      }
    }

    Ok(caret)
  }

  fn search_text(
    &mut self,
    line: Area,
    search: &Search<'_, A>,
    placeholder: Option<&str>,
  ) -> Result<Option<Part>, Never> {
    match placeholder {
      Some(text) => {
        let Ok(()) = self.text(line, text, 15, Face::Body, self.colors.soft, Lines(1));
        return Ok(None);
      }
      None => {}
    }

    let inserted = format!("{}{}", search.before_caret, search.preedit);
    let shown = format!("{inserted}{}", search.after_caret);
    let Ok(()) = self.text(line, &shown, 15, Face::Body, self.colors.text, Lines(1));

    match (self.canvas.interaction.focus == Some(search.edit)).then_some(()) {
      Some(()) => {
        let Ok((font, weight)) = Face::Body.font(15);
        let Ok(measured) = measure_text(
          Run {
            said: &inserted,
            weight,
            width: u32::MAX,
          },
          &font,
        );
        let offset = u32::cast_signed(measured.width).clamp(0, line.size.width.saturating_sub(1));
        let caret = Area {
          origin: Point {
            x: line.origin.x.wrapping_add(offset),
            y: line.origin.y.wrapping_add(12),
          },
          size: Size {
            width: 1,
            height: 19,
          },
        };
        let Ok(()) = self.canvas.panel(caret, self.colors.pink, 0, None);
        let Ok(size) = caret.pixel_size();
        Ok(Some(Part {
          at: caret.origin,
          size,
        }))
      }
      None => Ok(None),
    }
  }

  fn notice(
    &mut self,
    area: Area,
    notice: &Notice<'_, A>,
    placement: NoticePlacement,
  ) -> Result<(), Never> {
    let colors = self.colors;
    let (x, y) = (area.origin.x, area.origin.y);
    let Ok(()) = self
      .canvas
      .panel(area, colors.ground, 10, Some(colors.panel));
    let reserve = match (placement, notice.details) {
      (NoticePlacement::Results, _) => 154,
      (NoticePlacement::Header, Some(_)) => 160,
      (NoticePlacement::Header, None) => 80,
    };
    let width = area
      .size
      .width
      .wrapping_sub(reserve)
      .wrapping_sub(20)
      .max(70);
    let title_y = match notice.message.detail {
      Some(_) => y.wrapping_add(6),
      None => y.wrapping_add(16),
    };
    let title = Area {
      origin: Point {
        x: x.wrapping_add(13),
        y: title_y,
      },
      size: Size { width, height: 22 },
    };
    let Ok(()) = self.text(
      title,
      notice.message.title,
      15,
      Face::Medium,
      colors.text,
      Lines(1),
    );

    match notice.message.detail {
      Some(detail) => {
        let row = Area {
          origin: Point {
            x: x.wrapping_add(13),
            y: y.wrapping_add(29),
          },
          size: Size { width, height: 20 },
        };
        let Ok(()) = self.text(row, detail, 12, Face::Body, colors.soft, Lines(1));
      }
      None => {}
    }

    let Ok(right) = area.right();

    match notice.details {
      Some(link) => {
        let button = Area {
          origin: Point {
            x: right.wrapping_sub(148),
            y: y.wrapping_add(11),
          },
          size: Size {
            width: 74,
            height: 34,
          },
        };
        let Ok(()) = self.button(button, link, Button::Normal);
      }
      None => {}
    }

    match notice.reply {
      Some(link) => {
        let button = Area {
          origin: Point {
            x: right.wrapping_sub(66),
            y: y.wrapping_add(11),
          },
          size: Size {
            width: 58,
            height: 34,
          },
        };
        let Ok(()) = self.button(button, link, Button::Quiet);
      }
      None => {}
    }

    Ok(())
  }

  fn results_header(&mut self, mut area: Area, view: &Catalogue<'_, A>) -> Result<Area, Never> {
    let (x, y) = (area.origin.x, area.origin.y);
    let heading = Area {
      origin: Point { x, y },
      size: Size {
        width: area.size.width,
        height: 48,
      },
    };
    let deck = Area {
      origin: Point {
        x,
        y: y.wrapping_add(49),
      },
      size: Size {
        width: area.size.width,
        height: 28,
      },
    };
    let Ok(()) = self.text(
      heading,
      view.title,
      34,
      Face::Medium,
      self.colors.text,
      Lines(1),
    );
    let Ok(()) = self.text(
      deck,
      view.subtitle,
      15,
      Face::Body,
      self.colors.soft,
      Lines(1),
    );
    area.origin.y = y.wrapping_add(88);
    Ok(area)
  }

  fn results_notices(&mut self, mut area: Area, view: &Catalogue<'_, A>) -> Result<Area, Never> {
    let colors = self.colors;

    match view.advisory {
      Some(message) => {
        let (x, y) = (area.origin.x, area.origin.y);
        let width = area.size.width;
        let panel = Area {
          origin: Point { x, y },
          size: Size { width, height: 66 },
        };
        let Ok(()) = self.canvas.panel(panel, colors.ground, 9, None);
        let title = Area {
          origin: Point {
            x: x.wrapping_add(14),
            y: y.wrapping_add(8),
          },
          size: Size {
            width: width.wrapping_sub(28),
            height: 21,
          },
        };
        let Ok(()) = self.text(
          title,
          message.title,
          15,
          Face::Medium,
          colors.text,
          Lines(1),
        );

        match message.detail {
          Some(detail) => {
            let row = Area {
              origin: Point {
                x: title.origin.x,
                y: y.wrapping_add(32),
              },
              size: Size {
                width: title.size.width,
                height: 22,
              },
            };
            let Ok(()) = self.text(row, detail, 13, Face::Body, colors.soft, Lines(1));
          }
          None => {}
        }

        area.origin.y = y.wrapping_add(77);
      }
      None => {}
    }

    match &view.problem {
      Some(problem) => {
        let mut panel = area;
        panel.size.height = 56;
        let Ok(()) = self.notice(panel, problem, NoticePlacement::Results);
        area.origin.y = area.origin.y.wrapping_add(67);
      }
      None => {}
    }

    Ok(area)
  }

  fn introduction(&mut self, mut area: Area, intro: &Introduction<'_, A>) -> Result<Area, Never> {
    match intro {
      Introduction::Featured {
        banner,
        title,
        filters,
        selected,
      } => {
        match banner {
          Some(feature) => {
            let mut panel = area;
            panel.size.height = 246;
            let Ok(()) = self.feature(panel, feature);
            area.origin.y = area.origin.y.wrapping_add(261);
          }
          None => {}
        }

        self.filters(area, title, filters, *selected)
      }
      Introduction::Empty { message, link } => self.empty(area, *message, *link),
      Introduction::None => Ok(area),
    }
  }

  fn feature(&mut self, area: Area, feature: &Feature<'_, A>) -> Result<(), Never> {
    let Ok(()) = self.canvas.panel(area, self.colors.ground, 0, None);
    let Ok(()) = self.canvas.cover(area, feature.art);
    let x = area.origin.x.wrapping_add(23);
    let y = area.origin.y;
    let width = area
      .size
      .width
      .wrapping_mul(53)
      .wrapping_div(100)
      .clamp(145, 340);
    let Ok(white) = to_oklch("#ffffff");
    let Ok(lilac) = to_oklch("#d7cefa");
    let eyebrow = Area {
      origin: Point {
        x,
        y: y.wrapping_add(31),
      },
      size: Size { width, height: 25 },
    };
    let Ok(()) = self.text(eyebrow, feature.eyebrow, 13, Face::Medium, lilac, Lines(1));
    let title_size = match area.size.width.cmp(&610) {
      Ordering::Greater => 47,
      Ordering::Less | Ordering::Equal => 33,
    };
    let title = Area {
      origin: Point {
        x,
        y: y.wrapping_add(57),
      },
      size: Size { width, height: 62 },
    };
    let Ok(()) = self.text(
      title,
      feature.title,
      title_size,
      Face::Medium,
      white,
      Lines(1),
    );
    let description = Area {
      origin: Point {
        x,
        y: y.wrapping_add(124),
      },
      size: Size { width, height: 48 },
    };
    let Ok(()) = self.text(
      description,
      feature.description,
      15,
      Face::Body,
      white,
      Lines(2),
    );
    let button = Area {
      origin: Point {
        x,
        y: y.wrapping_add(181),
      },
      size: Size {
        width: 170,
        height: 41,
      },
    };
    self.button(button, feature.link, Button::Primary)
  }

  fn filters(
    &mut self,
    mut area: Area,
    title: &str,
    filters: &[(Link<'_, A>, i32)],
    selected: A,
  ) -> Result<Area, Never> {
    let mut heading = area;
    heading.size.height = 31;
    let Ok(()) = self.text(heading, title, 22, Face::Medium, self.colors.text, Lines(1));
    area.origin.y = area.origin.y.wrapping_add(42);
    let mut left = area.origin.x;
    let Ok(right) = area.right();

    for &(link, width) in filters {
      match (left.wrapping_add(width) > right && left != area.origin.x).then_some(()) {
        Some(()) => {
          left = area.origin.x;
          area.origin.y = area.origin.y.wrapping_add(44);
        }
        None => {}
      }

      let kind = match (link.action == selected).then_some(()) {
        Some(()) => Button::Selected,
        None => Button::Normal,
      };
      let button = Area {
        origin: Point {
          x: left,
          y: area.origin.y,
        },
        size: Size { width, height: 37 },
      };
      let Ok(()) = self.button(button, link, kind);
      left = left.wrapping_add(width.wrapping_add(7));
    }

    area.origin.y = area.origin.y.wrapping_add(51);
    Ok(area)
  }

  fn empty(
    &mut self,
    mut area: Area,
    message: Message<'_>,
    link: Option<Link<'_, A>>,
  ) -> Result<Area, Never> {
    let colors = self.colors;
    let (x, y) = (area.origin.x, area.origin.y);
    let panel = Area {
      origin: Point { x, y },
      size: Size {
        width: area.size.width,
        height: 92,
      },
    };
    let Ok(()) = self
      .canvas
      .panel(panel, colors.ground, 10, Some(colors.panel));
    let width = area.size.width.wrapping_sub(34);
    let title = Area {
      origin: Point {
        x: x.wrapping_add(17),
        y: y.wrapping_add(13),
      },
      size: Size { width, height: 25 },
    };
    let Ok(()) = self.text(
      title,
      message.title,
      17,
      Face::Medium,
      colors.text,
      Lines(1),
    );

    match message.detail {
      Some(detail) => {
        let row = Area {
          origin: Point {
            x: title.origin.x,
            y: y.wrapping_add(47),
          },
          size: Size { width, height: 22 },
        };
        let Ok(()) = self.text(row, detail, 14, Face::Body, colors.soft, Lines(1));
      }
      None => {}
    }

    area.origin.y = y.wrapping_add(105);

    match link {
      Some(next) => {
        let button = Area {
          origin: Point {
            x,
            y: area.origin.y,
          },
          size: Size {
            width: 95,
            height: 39,
          },
        };
        let Ok(()) = self.button(button, next, Button::Normal);
        area.origin.y = area.origin.y.wrapping_add(52);
      }
      None => {}
    }

    Ok(area)
  }

  fn entries<'a, Row>(
    &mut self,
    area: Area,
    viewport: Area,
    layout: EntryLayout,
    rows: impl IntoIterator<Item = Row>,
    describe: impl Fn(Row) -> Entry<'a, A>,
  ) -> Result<i32, Never> {
    let columns = match (layout, area.size.width) {
      (EntryLayout::Tiles, 700..) => 3_i32,
      (EntryLayout::Tiles, 360..=699) => 2,
      (EntryLayout::Tiles | EntryLayout::List, _) => 1,
    };
    let column_count = match NonZeroI32::new(columns) {
      Some(count) => count,
      None => return Ok(area.origin.y),
    };
    let gap = 10_i32;
    let width = area
      .size
      .width
      .wrapping_sub(gap.wrapping_mul(columns.wrapping_sub(1)))
      .wrapping_div(column_count.get());
    let height = match layout {
      EntryLayout::Tiles => 132_i32,
      EntryLayout::List => 76,
    };
    let mut count = 0_i32;

    for row in rows {
      let column = count.wrapping_rem(column_count.get());
      let line = count.wrapping_div(column_count.get());
      let card = Area {
        origin: Point {
          x: area
            .origin
            .x
            .wrapping_add(column.wrapping_mul(width.wrapping_add(gap))),
          y: area
            .origin
            .y
            .wrapping_add(line.wrapping_mul(height.wrapping_add(gap))),
        },
        size: Size { width, height },
      };
      let Ok(visible) = card.inside(viewport);

      match visible {
        Some(_) => {
          let Ok(()) = self.entry(card, &describe(row), layout);
        }
        None => {}
      }

      count = count.wrapping_add(1);
    }

    let occupied = count
      .wrapping_add(columns)
      .wrapping_sub(1)
      .wrapping_div(column_count.get());
    Ok(
      area
        .origin
        .y
        .wrapping_add(occupied.wrapping_mul(height.wrapping_add(gap))),
    )
  }

  fn entry(
    &mut self,
    area: Area,
    entry: &Entry<'_, A>,
    layout: EntryLayout,
  ) -> Result<(), Never> {
    let colors = self.colors;
    let active = self.canvas.interaction.focus == Some(entry.action)
      || self.canvas.interaction.hover == Some(entry.action);
    let (fill, edge) = match active.then_some(()) {
      Some(()) => (colors.panel, colors.pink),
      None => (colors.ground, colors.panel),
    };
    let Ok(()) = self.canvas.panel(area, fill, 11, Some(edge));
    let (x, y) = (area.origin.x, area.origin.y);
    let icon = Area {
      origin: Point {
        x: x.wrapping_add(13),
        y: y.wrapping_add(13),
      },
      size: Size {
        width: 42,
        height: 42,
      },
    };
    let Ok(()) = self.canvas.panel(icon, colors.panel, 9, None);
    let glyph = Area {
      origin: Point {
        x: icon.origin.x.wrapping_add(12),
        y: icon.origin.y,
      },
      size: Size {
        width: 25,
        height: 42,
      },
    };
    let Ok(()) = self.text(glyph, entry.icon, 18, Face::Medium, colors.pink, Lines(1));
    let Ok(()) = self.entry_words(area, entry, layout);
    self.canvas.hit(area, entry.action)
  }

  fn entry_words(
    &mut self,
    area: Area,
    entry: &Entry<'_, A>,
    layout: EntryLayout,
  ) -> Result<(), Never> {
    let (left, top, right, height, lines) = match layout {
      EntryLayout::Tiles => (13, 61, 13, 39, Lines(2)),
      EntryLayout::List => (69, 7, 52, 26, Lines(1)),
    };
    let (x, y) = (
      area.origin.x.wrapping_add(left),
      area.origin.y.wrapping_add(top),
    );
    let width = area.size.width.wrapping_sub(left).wrapping_sub(right);
    let title = Area {
      origin: Point { x, y },
      size: Size { width, height: 24 },
    };
    let Ok(()) = self.text(
      title,
      entry.title,
      17,
      Face::Medium,
      self.colors.text,
      Lines(1),
    );
    let detail = Area {
      origin: Point {
        x,
        y: y.wrapping_add(28),
      },
      size: Size { width, height },
    };
    let Ok(()) = self.text(
      detail,
      entry.detail,
      13,
      Face::Body,
      self.colors.soft,
      lines,
    );
    let Ok(edge) = area.right();
    let mark = Area {
      origin: Point {
        x: edge.wrapping_sub(37),
        y: area.origin.y.wrapping_add(13),
      },
      size: Size {
        width: 28,
        height: 42,
      },
    };
    self.text(
      mark,
      entry.indicator,
      19,
      Face::Body,
      self.colors.pink,
      Lines(1),
    )
  }

  fn scrollbar(&mut self, root: Area, viewport: Area, scroll: Scroll) -> Result<(), Never> {
    let overflow = scroll
      .bottom
      .wrapping_add(scroll.offset)
      .wrapping_add(14)
      .wrapping_sub(root.size.height)
      .max(0);
    self.canvas.frame.max_scroll = overflow;
    let Ok(()) = self.canvas.lift_clip(root);
    let maximum = match NonZeroI32::new(overflow).filter(|_| viewport.size.height > 0) {
      Some(range) => range,
      None => return Ok(()),
    };

    let Ok(()) = self.canvas.clip(root);
    let track = viewport.size.height.saturating_sub(12).max(1);
    let extent = match NonZeroI32::new(track.saturating_add(maximum.get())) {
      Some(extent) => extent,
      None => return Ok(()),
    };
    let visible = track
      .wrapping_mul(track)
      .wrapping_div(extent.get())
      .max(22)
      .min(track);
    let offset = track
      .wrapping_sub(visible)
      .wrapping_mul(scroll.offset.min(maximum.get()))
      .wrapping_div(maximum.get());
    let Ok(right) = root.right();
    let thumb = Area {
      origin: Point {
        x: right.wrapping_sub(8),
        y: viewport.origin.y.wrapping_add(6).wrapping_add(offset),
      },
      size: Size {
        width: 3,
        height: visible,
      },
    };
    self.canvas.panel(thumb, self.colors.edge, 1, None)
  }
}
