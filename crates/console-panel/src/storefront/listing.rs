//! Layout for a searchable catalogue and its scrolling entries.

use std::num::NonZeroI32;

use console_core_color::palette::Wearing;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_shapes::Pixels;
use console_draw_painting::{Lines, Run, measure_text};
use console_draw_surface::Part;

use crate::canvas::{Area, AreaLayout, Face, Interaction};
use crate::{fitting, strip};

use super::{Button, Components, Frame, Link, Message, Notice, NoticeTone, window};

struct Scroll {
  bottom: i32,
  offset: i32,
}

enum NoticePlacement {
  Header,
  Results,
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

pub enum Introduction<'a, A> {
  Featured {
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

pub struct Entry<'a, A> {
  pub title: &'a str,
  pub detail: &'a str,
  pub icon: &'a str,
  pub art: Option<&'a Pixels>,
  pub indicator: &'a str,
  pub action: A,
}

pub struct Catalogue<'a, A> {
  pub header: strip::Header<'a, A>,
  pub search: Search<'a, A>,
  pub notice: Option<Notice<'a, A>>,
  pub status: &'a str,
  pub advisory: Option<Message<'a>>,
  pub problem: Option<Notice<'a, A>>,
  pub introduction: Introduction<'a, A>,
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
  let Ok(headed) = components.results_status(content, view.status);
  let Ok(with_notices) = components.results_notices(headed, view);
  let Ok(cards) = components.introduction(with_notices, &view.introduction);
  let Ok((bottom, entries)) = components.entries(cards, viewport, rows, describe);
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
    entries,
  })
}

impl<A: Copy + PartialEq> Components<A> {
  fn catalogue_header(
    &mut self,
    root: Area,
    view: &Catalogue<'_, A>,
  ) -> Result<(Area, Area, Option<Part>), Never> {
    let colors = self.colors;
    let Ok(()) = self.canvas.panel(root, colors.panel, 0, None);
    let margin = strip::MARGIN;
    let x = margin;
    let width = root
      .size
      .width
      .saturating_sub(margin.saturating_mul(2))
      .max(1);
    let header = Area {
      origin: Point { x, y: margin },
      size: Size {
        width,
        height: strip::HEIGHT,
      },
    };
    let Ok(()) = strip::header(&mut self.canvas, header, &view.header, &colors);
    let search_y = margin
      .saturating_add(strip::HEIGHT)
      .saturating_add(strip::GAP);
    let search_area = Area {
      origin: Point { x, y: search_y },
      size: Size {
        width,
        height: strip::HEIGHT,
      },
    };
    let Ok(caret) = self.search(search_area, &view.search);
    let list_y = search_y
      .saturating_add(strip::HEIGHT)
      .saturating_add(strip::GAP);
    let top = match &view.notice {
      Some(notice) => {
        let area = Area {
          origin: Point { x, y: list_y },
          size: Size { width, height: 56 },
        };
        let Ok(()) = self.notice(area, notice, NoticePlacement::Header);
        list_y.saturating_add(62)
      }
      None => list_y,
    };

    let viewport = Area {
      origin: Point { x: 0, y: top },
      size: Size {
        width: root.size.width,
        height: root.size.height.saturating_sub(top),
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
    let title_color = match notice.tone {
      NoticeTone::Neutral => colors.text,
      NoticeTone::Error => colors.coral,
    };
    let Ok(()) = self.text(
      title,
      notice.message.title,
      15,
      Face::Medium,
      title_color,
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

  fn results_status(&mut self, mut area: Area, status: &str) -> Result<Area, Never> {
    match status {
      "" => {}
      text => {
        let mut line = area;
        line.size.height = 28;
        let Ok(()) = self.text(line, text, 14, Face::Body, self.colors.soft, Lines(1));
        area.origin.y = area.origin.y.saturating_add(line.size.height);
      }
    }

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

  fn introduction(&mut self, area: Area, intro: &Introduction<'_, A>) -> Result<Area, Never> {
    match intro {
      Introduction::Featured {
        title,
        filters,
        selected,
      } => self.filters(area, title, filters, *selected),
      Introduction::Empty { message, link } => self.empty(area, *message, *link),
      Introduction::None => Ok(area),
    }
  }

  fn filters(
    &mut self,
    mut area: Area,
    title: &str,
    filters: &[(Link<'_, A>, i32)],
    selected: A,
  ) -> Result<Area, Never> {
    let mut heading = area;
    heading.size.height = 26;
    let Ok(()) = self.text(heading, title, 16, Face::Medium, self.colors.text, Lines(1));
    area.origin.y = area.origin.y.wrapping_add(29);
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

    area.origin.y = area.origin.y.wrapping_add(44);
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
    mut area: Area,
    viewport: Area,
    rows: impl IntoIterator<Item = Row>,
    describe: impl Fn(Row) -> Entry<'a, A>,
  ) -> Result<(i32, Vec<(A, Area)>), Never> {
    area.size.height = strip::HEIGHT;
    let mut entries = Vec::new();

    for row in rows {
      let entry = describe(row);
      entries.push((entry.action, area));
      let Ok(visible) = area.inside(viewport);

      match visible {
        Some(_) => {
          let Ok(()) = self.entry(area, &entry);
        }
        None => {}
      }

      area.origin.y = area.origin.y.saturating_add(fitting::ROW);
    }

    Ok((area.origin.y, entries))
  }

  fn entry(&mut self, area: Area, entry: &Entry<'_, A>) -> Result<(), Never> {
    let colors = self.colors;
    let active = self.canvas.interaction.focus == Some(entry.action);
    let hover = self.canvas.interaction.hover == Some(entry.action);
    let (fill, ink, quiet) = match (active, hover) {
      (true, _) => (colors.pink, colors.night, colors.night),
      (false, true) => (colors.fill, colors.text, colors.soft),
      (false, false) => (colors.ground, colors.text, colors.soft),
    };
    let Ok(()) = self.canvas.panel(area, fill, 12, None);
    let (x, y) = (area.origin.x, area.origin.y);
    let icon = Area {
      origin: Point {
        x: x.saturating_add(9),
        y: y.saturating_add(6),
      },
      size: Size {
        width: 46,
        height: area.size.height.saturating_sub(12),
      },
    };

    match entry.art {
      Some(art) => {
        let Ok(()) = self.canvas.cover(icon, art);
      }
      None => {
        let Ok(()) = self.canvas.paragraph(
          icon,
          entry.icon,
          18,
          Face::Medium,
          ink,
          crate::canvas::TextLayout {
            align: crate::canvas::Align::Center,
            lines: Lines(1),
          },
        );
      }
    }

    let left = x.saturating_add(65);
    let width = area.size.width.saturating_sub(113);
    let title = Area {
      origin: Point {
        x: left,
        y: y.saturating_add(2),
      },
      size: Size { width, height: 24 },
    };
    let detail = Area {
      origin: Point {
        x: left,
        y: y.saturating_add(24),
      },
      size: Size { width, height: 21 },
    };
    let Ok(()) = self.text(title, entry.title, 16, Face::Medium, ink, Lines(1));
    let Ok(()) = self.text(detail, entry.detail, 13, Face::Body, quiet, Lines(1));
    let Ok(edge) = area.right();
    let mark = Area {
      origin: Point {
        x: edge.saturating_sub(37),
        y,
      },
      size: Size {
        width: 28,
        height: area.size.height,
      },
    };
    let Ok(()) = self.text(mark, entry.indicator, 19, Face::Body, ink, Lines(1));
    self.canvas.hit(area, entry.action)
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
