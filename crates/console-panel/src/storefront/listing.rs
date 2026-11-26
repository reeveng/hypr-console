//! Layout for a searchable catalogue and its scrolling entries.

use std::num::NonZeroI32;

use console_core_color::palette::Wearing;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_shapes::Pixels;
use console_draw_painting::Lines;
use console_draw_surface::{Composition, Part};

use crate::canvas::{Align, Area, AreaLayout, Face, Interaction, ROW_RADIUS, RowTone};
use crate::ui::list;
use crate::{field, fitting, marks, strip};

use super::{Button, Components, Frame, Link, Message, Notice, window};

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
  pub clear: A,
  pub placeholder: &'a str,
  pub shortcut: &'a str,
  pub before_caret: &'a str,
  pub after_caret: &'a str,
  pub preedit: Option<&'a Composition>,
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
    let Ok(header) = strip::window_area(root.size.width);
    let Ok(()) = strip::header(&mut self.canvas, header, &view.header, &colors);
    let x = header.origin.x;
    let width = header.size.width;
    let search_y = strip::CONTENT_TOP;
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
    let focused = self.canvas.interaction.focus == Some(search.edit);
    let empty = search.before_caret.is_empty() && search.after_caret.is_empty();
    let Ok(draft) = field::Draft::new(
      field::TextParts {
        before: search.before_caret,
        after: search.after_caret,
      },
      search.preedit,
    );
    let content = match draft.text.as_str() {
      "" => field::Content::Placeholder(search.placeholder),
      written => field::Content::Value(written),
    };
    let editing = match focused.then_some(()) {
      Some(()) => draft.editing,
      None => field::Editing::Inactive,
    };
    let button_width = area.size.width.min(58);
    let Ok(cursor) = field::Field {
      area,
      trailing: button_width,
      content,
      editing,
    }
    .paint(&mut self.canvas.frame.shapes, &self.colors);
    let Ok(right) = area.right();
    let button = Area {
      origin: Point {
        x: right.saturating_sub(button_width),
        y: area.origin.y.saturating_add(6),
      },
      size: Size {
        width: button_width.saturating_sub(6),
        height: area.size.height.saturating_sub(12),
      },
    };

    match empty.then_some(()) {
      Some(()) => {
        let Ok(()) = self.canvas.text(
          button,
          search.shortcut,
          12,
          Face::Mono,
          self.colors.soft,
          Align::Center,
        );
        let Ok(()) = self.canvas.hit(area, search.edit);
      }
      None => {
        let edit = Area {
          size: Size {
            width: area.size.width.saturating_sub(button_width),
            height: area.size.height,
          },
          ..area
        };
        let Ok(()) = self.canvas.hit(edit, search.edit);
        let Ok(()) = self.icon_button(button, marks::SHUT, search.clear);
      }
    }

    Ok(cursor)
  }

  fn icon_button(&mut self, area: Area, icon: &str, action: A) -> Result<(), Never> {
    let state = match (self.canvas.interaction.focus == Some(action)
      || self.canvas.interaction.hover == Some(action))
    .then_some(())
    {
      Some(()) => strip::Control::Focused,
      None => strip::Control::Ready,
    };
    let Ok(()) = strip::paint_control(
      &mut self.canvas.frame.shapes,
      area,
      icon,
      state,
      &self.colors,
    );
    self.canvas.hit(area, action)
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
            x: right.wrapping_sub(160),
            y: y.wrapping_add(11),
          },
          size: Size {
            width: 86,
            height: 34,
          },
        };
        let Ok(()) = self.button(button, link, Button::Normal);
      }
      None => {}
    }

    match notice.retry {
      Some(action) => {
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
        let Ok(()) = self.icon_button(button, marks::RELOAD, action);
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
    let tone = match active.then_some(()) {
      Some(()) => RowTone::Focused,
      None => RowTone::Normal,
    };
    let Ok(row) = tone.colors(&colors);
    let Ok(()) = self.canvas.panel(area, row.fill, ROW_RADIUS, None);
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
          row.ink,
          crate::canvas::TextLayout {
            align: crate::canvas::Align::Center,
            lines: Lines(1),
          },
        );
      }
    }

    let left = x.saturating_add(65);
    let width = area.size.width.saturating_sub(113);
    let label_area = Area {
      origin: Point {
        x: left,
        y: y.saturating_add(2),
      },
      size: Size {
        width,
        height: area.size.height.saturating_sub(4),
      },
    };
    let Ok(()) = list::label(
      &mut self.canvas,
      label_area,
      list::Label {
        title: entry.title,
        detail: entry.detail,
      },
      &row,
    );
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
    let Ok(()) = self.text(mark, entry.indicator, 19, Face::Body, row.ink, Lines(1));
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
