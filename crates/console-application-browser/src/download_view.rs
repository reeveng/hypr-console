//! The browser's native downloads view. Search uses the current snapshot
//! while a replacement catalog is built on the scanner thread.

use std::io;
use std::path::PathBuf;

use console_core_color::palette::Wearing;
use console_core_fonts::Font;
use console_core_geometry::{Point, Rectangle, Size};
use console_core_highlight::under;
use console_core_never::Never;
use console_core_number_conversion::toward_zero_u32;
use console_input_key_symbols::Keysym;
use console_input_text::{TextField, Typed};
use console_panel::ui::painting::{self, Frame};
use console_panel::ui::shapes::{Clip, Edge, Line, Shape};
use console_panel::ui::surface::KeyPress;

use crate::download_catalog::{Catalog, FileAction, Matches, Record};
use crate::downloads::{self, Id};
use crate::settings::{Alignment, label_at};
use crate::toolbar::{CONTROL, CONTROL_SHIFT, SHIFT, edit_press, measured, panel, words};

const ROW_TOP: i32 = 119;
const ROW_HEIGHT: i32 = 56;

pub(crate) enum Action {
  Handled,
  Dismiss,
  Refresh,
  Folder,
  Open(Id),
  File { id: Id, action: FileAction },
}

struct RowMenu {
  id: Id,
  anchor: i32,
  focus: FileAction,
  drawn: Option<[Rectangle<i32>; 2]>,
}

#[derive(Default)]
enum FileActivity {
  #[default]
  Idle,
  Working,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Focus {
  #[default]
  Search,
  Rows,
  Folder,
  Refresh,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Visibility {
  Hidden,
  Visible,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Drawing {
  Pending,
  Current,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Availability {
  Ready,
  Pending,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileProgress {
  Working,
  Idle,
}

#[derive(Default)]
enum Window {
  #[default]
  Hidden,
  Visible(Drawing),
}

#[derive(Default)]
enum Selection {
  #[default]
  Cursor,
  All,
}

#[derive(Default)]
enum CatalogState {
  #[default]
  Empty,
  Ready(Catalog),
  Scanning(Option<Catalog>),
  Failed {
    previous: Option<Catalog>,
    message: &'static str,
  },
}

impl CatalogState {
  fn snapshot(&self) -> Result<Option<&Catalog>, Never> {
    Ok(match self {
      Self::Empty => None,
      Self::Ready(catalog) => Some(catalog),
      Self::Scanning(previous) | Self::Failed { previous, .. } => previous.as_ref(),
    })
  }

  fn into_snapshot(self) -> Result<Option<Catalog>, Never> {
    Ok(match self {
      Self::Empty => None,
      Self::Ready(catalog) => Some(catalog),
      Self::Scanning(previous) | Self::Failed { previous, .. } => previous,
    })
  }
}

#[derive(Default)]
pub(crate) struct View {
  window: Window,
  catalog: CatalogState,
  matches: Matches,
  search: TextField,
  selection: Selection,
  focus: Focus,
  selected: u64,
  first: u64,
  layout: Option<Layout>,
  drawn_rows: Vec<Id>,
  menu: Option<RowMenu>,
  file_activity: FileActivity,
}

impl View {
  pub(crate) fn open(&mut self) -> Result<(), Never> {
    self.window = Window::Visible(Drawing::Pending);
    self.focus = Focus::Search;
    self.selection = Selection::Cursor;
    self.layout = None;
    self.drawn_rows.clear();
    self.menu = None;

    Ok(())
  }

  pub(crate) fn begin(&mut self) -> Result<(), Never> {
    let Ok(previous) = std::mem::take(&mut self.catalog).into_snapshot();
    self.catalog = CatalogState::Scanning(previous);
    let Ok(()) = self.invalidate();

    Ok(())
  }

  pub(crate) fn replace(
    &mut self,
    catalog: Catalog,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(previous) = self.catalog.snapshot();
      let selected = previous
        .and_then(|old| {
          let Ok(row) = old.row(&self.matches, self.selected);

          row
        })
        .map(|record| record.id.clone());
      self.catalog = CatalogState::Ready(catalog);
      let Ok(()) = self.find();
      let Ok(current) = self.catalog.snapshot();
      let position = selected.as_ref().and_then(|id| {
        let recorded = current?;
        let Ok(catalog_position) = recorded.position(&self.matches, id);

        catalog_position
      });

      match (position, self.focus) {
        (Some(position_2), _) => {
          self.selected = position_2;
          let Ok(()) = self.reveal();
        }
        (None, Focus::Rows) => self.focus = Focus::Search,
        (None, Focus::Search | Focus::Folder | Focus::Refresh) => {}
      }
    };

    Ok(())
  }

  pub(crate) fn report(&mut self, message: &'static str) -> Result<(), Never> {
    let Ok(previous) = std::mem::take(&mut self.catalog).into_snapshot();
    self.catalog = CatalogState::Failed { previous, message };
    let Ok(()) = self.invalidate();

    Ok(())
  }

  #[must_use]
  pub(crate) fn file_busy(&self) -> Result<FileProgress, Never> {
    Ok(match self.file_activity {
      FileActivity::Working => FileProgress::Working,
      FileActivity::Idle => FileProgress::Idle,
    })
  }

  pub(crate) fn file_started(&mut self, action: FileAction) -> Result<(), Never> {
    self.file_activity = FileActivity::Working;
    let message = match action {
      FileAction::Reveal => "Opening file location…",
      FileAction::Trash => "Moving to Trash…",
    };
    let Ok(()) =
      console_notifications::notify(console_notifications::Application("Browser"), message, None);

    let Ok(()) = self.invalidate();

    Ok(())
  }

  pub(crate) fn file_finished(&mut self, message: Option<&'static str>) -> Result<(), Never> {
    self.file_activity = FileActivity::Idle;

    match message {
      Some(notice) => {
        let Ok(()) = console_notifications::notify(
          console_notifications::Application("Browser"),
          notice,
          None,
        );
      }
      None => {}
    }

    let Ok(()) = self.invalidate();

    Ok(())
  }

  pub(crate) fn file(&self, id: &Id) -> io::Result<PathBuf> {
    let Ok(current) = self.catalog.snapshot();
    let root = match current {
      Some(catalog) => match catalog.file(id) {
        Ok(path) => return Ok(path),
        Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
          Some(()) => catalog.root.clone(),
          None => return Err(fault),
        },
      },
      None => downloads::folder()?,
    };
    downloads::locate(&root, id)
  }

  #[must_use]
  pub(crate) fn availability(&self) -> Result<Availability, Never> {
    Ok(match self.catalog {
      CatalogState::Ready(_)
      | CatalogState::Failed {
        previous: Some(_), ..
      } => Availability::Ready,
      CatalogState::Empty
      | CatalogState::Scanning(_)
      | CatalogState::Failed { previous: None, .. } => Availability::Pending,
    })
  }

  pub(crate) fn visibility(&self) -> Result<Visibility, Never> {
    Ok(match self.window {
      Window::Hidden => Visibility::Hidden,
      Window::Visible(_) => Visibility::Visible,
    })
  }

  pub(crate) fn drawing(&self) -> Result<Drawing, Never> {
    Ok(match self.window {
      Window::Hidden => Drawing::Current,
      Window::Visible(drawing) => drawing,
    })
  }

  pub(crate) fn close(&mut self) -> Result<(), Never> {
    self.window = Window::Hidden;
    Ok(())
  }

  fn invalidate(&mut self) -> Result<(), Never> {
    match self.window {
      Window::Hidden => {}
      Window::Visible(_) => self.window = Window::Visible(Drawing::Pending),
    }

    Ok(())
  }

  pub(crate) fn key(
    &mut self,
    key: Keysym,
    mods: u32,
  ) -> core::result::Result<Action, console_core_never::Never> {
    let Ok(()) = self.invalidate();

    match (mods, key) {
      (0, Keysym::Escape) => {
        return Ok(match (self.menu.take(), self.focus) {
          (Some(_menu), _) => Action::Handled,
          (None, Focus::Search) => {
            self.focus = Focus::Rows;
            self.selection = Selection::Cursor;
            Action::Handled
          }
          (None, _) => Action::Dismiss,
        });
      }
      (0, Keysym::F6 | Keysym::F18 | Keysym::XF86Launch9) | (CONTROL, Keysym::j | Keysym::w) => {
        return Ok(Action::Dismiss);
      }
      (CONTROL, Keysym::r) | (0, Keysym::F5) => return Ok(Action::Refresh),
      _ => {}
    }

    match self.menu {
      Some(_) => {
        let Ok(menu_key) = self.menu_key(key, mods);

        return Ok(menu_key);
      }
      None => {}
    }

    Ok(match (self.focus, mods, key, self.selected) {
      (Focus::Rows, SHIFT, Keysym::F10, _)
      | (Focus::Rows, 0, Keysym::Menu | Keysym::Right, _) => {
        let Ok(selected_menu) = self.selected_menu();

        selected_menu
      }
      (Focus::Search, CONTROL, Keysym::a, _) => {
        self.selection = Selection::All;
        Action::Handled
      }
      (_, 0, Keysym::Tab, _) => {
        self.focus = match self.focus {
          Focus::Search => Focus::Rows,
          Focus::Rows => Focus::Folder,
          Focus::Folder => Focus::Refresh,
          Focus::Refresh => Focus::Search,
        };
        Action::Handled
      }
      (_, SHIFT, Keysym::Tab | Keysym::ISO_Left_Tab, _) => {
        self.focus = match self.focus {
          Focus::Search => Focus::Refresh,
          Focus::Rows => Focus::Search,
          Focus::Folder => Focus::Rows,
          Focus::Refresh => Focus::Folder,
        };
        Action::Handled
      }
      (_, 0, Keysym::Down, _) => {
        let Ok(move_rows) = self.move_rows(1);

        move_rows
      }
      (Focus::Rows, 0, Keysym::Up, 0) => {
        self.focus = Focus::Search;
        Action::Handled
      }
      (_, 0, Keysym::Up, _) => {
        let Ok(move_rows) = self.move_rows(-1);

        move_rows
      }
      (_, 0, Keysym::Page_Down, _) => {
        let page_rows = self.page_rows()?;
        let Ok(move_rows) = self.move_rows(page_rows);

        move_rows
      }
      (_, 0, Keysym::Page_Up, _) => {
        let page_rows = self.page_rows()?;
        let Ok(move_rows) = self.move_rows(page_rows.wrapping_neg());

        move_rows
      }
      (focus, 0, Keysym::Return | Keysym::KP_Enter, _) => match focus {
        Focus::Search => {
          let Ok(search) = self.search.text();

          match Id::parse(search.trim()) {
            Ok(id) => Action::Open(id),
            Err(console_browser_downloads::InvalidId) => {
              self.focus = Focus::Rows;
              Action::Handled
            }
          }
        }
        Focus::Rows => {
          let Ok(open_row) = self.open_row(self.selected);

          open_row
        }
        Focus::Folder => Action::Folder,
        Focus::Refresh => Action::Refresh,
      },
      (Focus::Search, 0 | SHIFT, _, _)
      | (Focus::Search, CONTROL | CONTROL_SHIFT, Keysym::BackSpace, _) => {
        let Ok(press) = edit_press(key, mods);
        let Ok(()) = self.edit(press);
        Action::Handled
      }
      _ => Action::Handled,
    })
  }

  fn menu_key(
    &mut self,
    key: Keysym,
    mods: u32,
  ) -> core::result::Result<Action, console_core_never::Never> {
    let menu = match &mut self.menu {
      Some(matched_menu) => matched_menu,
      ::core::option::Option::None => return Ok(Action::Handled),
    };

    Ok(match (mods, key) {
      (0, Keysym::Return | Keysym::KP_Enter | Keysym::space) => {
        let action = menu.focus;
        let Ok(choose_file_action) = self.choose_file_action(action);

        choose_file_action
      }
      (0, Keysym::Up) | (SHIFT, Keysym::Tab | Keysym::ISO_Left_Tab) => {
        menu.focus = FileAction::Reveal;
        Action::Handled
      }
      (0, Keysym::Down | Keysym::Tab) => {
        menu.focus = FileAction::Trash;
        Action::Handled
      }
      (0, Keysym::Left) => {
        self.menu = None;
        Action::Handled
      }
      _ => Action::Handled,
    })
  }

  fn selected_menu(&mut self) -> core::result::Result<Action, console_core_never::Never> {
    let layout = match self.layout {
      Some(matched_layout) => matched_layout,
      ::core::option::Option::None => return Ok(Action::Handled),
    };

    let Ok(snapshot) = self.catalog.snapshot();
    let record = match snapshot.and_then(|catalog| {
      let Ok(row) = catalog.row(&self.matches, self.selected);

      row
    }) {
      Some(record) => record,
      None => return Ok(Action::Handled),
    };

    let Ok(offset) =
      console_core_number_conversion::fitted::<_, i32>(self.selected.saturating_sub(self.first));
    self.menu = Some(RowMenu {
      id: record.id.clone(),
      anchor: layout
        .rows
        .origin
        .y
        .saturating_add(offset.saturating_mul(ROW_HEIGHT)),
      focus: FileAction::Reveal,
      drawn: None,
    });

    Ok(Action::Handled)
  }

  fn choose_file_action(
    &mut self,
    action: FileAction,
  ) -> core::result::Result<Action, console_core_never::Never> {
    let Ok(()) = self.invalidate();

    Ok(
      self
        .menu
        .take()
        .map_or(Action::Handled, |menu| Action::File {
          id: menu.id,
          action,
        }),
    )
  }

  fn edit(&mut self, press: KeyPress) -> core::result::Result<(), console_core_never::Never> {
    {
      let key = press.key;
      let Ok(letter) = key.char();
      let printable = letter.is_some_and(|ch| !ch.is_control());
      let clear = matches!(self.selection, Selection::All)
        && (printable || matches!(key, Keysym::BackSpace | Keysym::Delete));
      self.selection = Selection::Cursor;

      match ::core::primitive::bool::then_some(clear, ()) {
        ::core::option::Option::Some(()) => {
          self.search = TextField::default();
        }
        ::core::option::Option::None => {}
      }

      let Ok(current) = self.search.text();

      match { ::core::primitive::bool::then_some(printable && current.len() >= 256, ()) } {
        ::core::option::Option::Some(()) => {
          return Ok(());
        }
        ::core::option::Option::None => {}
      }

      let Ok(edited) = self.search.typed_press(press);

      match { ::core::primitive::bool::then_some(clear || edited == Typed::Edited, ()) } {
        ::core::option::Option::Some(()) => {
          let Ok(()) = self.find();
        }
        ::core::option::Option::None => {}
      }
    };

    Ok(())
  }

  fn find(&mut self) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(query) = self.search.text();
      let Ok(snapshot) = self.catalog.snapshot();
      self.matches = snapshot.map_or(Matches::All, |index| {
        let Ok(search) = index.search(query);

        search
      });
      self.selected = 0;
      self.first = 0;
      let Ok(()) = self.invalidate();
    };

    Ok(())
  }

  pub(crate) fn click(&mut self, at: Point<i32>) -> Result<Action, Never> {
    let layout = match self.layout {
      Some(layout) => layout,
      None => return Ok(Action::Handled),
    };

    match &self.menu {
      Some(menu) => return self.menu_click(at, menu.drawn),
      None => {}
    }

    let Ok(control) = under(at, &[layout.search, layout.folder, layout.refresh]);

    match control {
      Some(0) => {
        self.focus = Focus::Search;
        let Ok(()) = self.invalidate();
        Ok(Action::Handled)
      }
      Some(1) => Ok(Action::Folder),
      Some(2) => Ok(Action::Refresh),
      Some(_) | None => self.row_click(at, layout),
    }
  }

  fn menu_click(
    &mut self,
    at: Point<i32>,
    drawn: Option<[Rectangle<i32>; 2]>,
  ) -> Result<Action, Never> {
    let actions = match drawn {
      Some(actions) => actions,
      None => return Ok(Action::Handled),
    };
    let Ok(hit) = under(at, &actions);

    match hit {
      Some(0) => self.choose_file_action(FileAction::Reveal),
      Some(1) => self.choose_file_action(FileAction::Trash),
      Some(_) | None => {
        self.menu = None;
        let Ok(()) = self.invalidate();
        Ok(Action::Handled)
      }
    }
  }

  fn row_click(&mut self, at: Point<i32>, layout: Layout) -> Result<Action, Never> {
    let Ok(in_rows) = under(at, &[layout.rows]);

    match in_rows {
      Some(_) => {}
      None => return Ok(Action::Handled),
    }

    let delta = at.y.saturating_sub(ROW_TOP).div_euclid(ROW_HEIGHT);
    let position = match u32::try_from(delta) {
      Ok(position) => position,
      Err(_above_rows) => return Ok(Action::Handled),
    };
    let Ok(index) = console_core_number_conversion::index(position);
    let id = match self.drawn_rows.get(index).cloned() {
      Some(id) => id,
      None => return Ok(Action::Handled),
    };
    let y = ROW_TOP.saturating_add(delta.saturating_mul(ROW_HEIGHT));
    let Ok(row) = layout.row(y);
    let Ok(action) = actions_button(row);
    let Ok(on_actions) = under(at, &[action]);

    match on_actions {
      Some(_) => self.show_row_menu(id, y),
      None => Ok(Action::Open(id)),
    }
  }

  fn show_row_menu(&mut self, id: Id, y: i32) -> Result<Action, Never> {
    let Ok(snapshot) = self.catalog.snapshot();
    let position = snapshot.and_then(|catalog| {
      let Ok(visible_row) = catalog.position(&self.matches, &id);
      visible_row
    });

    match position {
      Some(visible_row) => self.selected = visible_row,
      None => {}
    }

    self.focus = Focus::Rows;
    self.menu = Some(RowMenu {
      id,
      anchor: y,
      focus: FileAction::Reveal,
      drawn: None,
    });
    let Ok(()) = self.invalidate();
    Ok(Action::Handled)
  }

  #[must_use]
  fn open_row(&self, row: u64) -> core::result::Result<Action, console_core_never::Never> {
    let Ok(snapshot) = self.catalog.snapshot();
    Ok(
      snapshot
        .and_then(|catalog| {
          let Ok(catalog_row) = catalog.row(&self.matches, row);

          catalog_row
        })
        .map_or(Action::Handled, |record| Action::Open(record.id.clone())),
    )
  }

  #[must_use]
  pub(crate) fn count(&self) -> core::result::Result<u64, console_core_never::Never> {
    let Ok(snapshot) = self.catalog.snapshot();
    Ok(
      snapshot.map_or(0, |catalog| match catalog.count(&self.matches) {
        ::core::result::Result::Ok(count) => count,
      }),
    )
  }

  #[must_use]
  fn page_rows(&self) -> core::result::Result<i64, console_core_never::Never> {
    Ok(match self.layout {
      Some(layout) => {
        let Ok(rows) = layout.visible_rows();
        i64::from(rows)
      }
      None => 1,
    })
  }

  fn move_rows(&mut self, step: i64) -> core::result::Result<Action, console_core_never::Never> {
    match (self.focus == Focus::Rows).then_some(()) {
      Some(()) => {
        let Ok(count) = self.count();
        self.selected = self
          .selected
          .saturating_add_signed(step)
          .min(count.saturating_sub(1));
      }
      None => {
        self.focus = Focus::Rows;
        self.selected = self.first;
      }
    }

    let Ok(()) = self.reveal();
    Ok(Action::Handled)
  }

  fn reveal(&mut self) -> core::result::Result<(), console_core_never::Never> {
    let Ok(rows) = self.page_rows();
    let Ok(count) = self.count();
    let page = rows.unsigned_abs().max(1);
    let first_visible = self.selected.saturating_add(1).saturating_sub(page);
    self.first = self.first.min(self.selected).max(first_visible);
    self.first = self.first.min(count.saturating_sub(page));
    Ok(())
  }

  pub(crate) fn scroll(
    &mut self,
    crate::reader::ScrollPixels(by): crate::reader::ScrollPixels,
  ) -> core::result::Result<(), console_core_never::Never> {
    self.menu = None;
    let Ok(steps) = toward_zero_u32((by.abs() / f64::from(ROW_HEIGHT)).ceil());
    let step = u64::from(steps);
    let Ok(count) = self.count();
    let Ok(page) = self.page_rows();
    let maximum = count.saturating_sub(page.unsigned_abs().max(1));
    self.first = match (by > 0.0).then_some(()) {
      Some(()) => self.first.saturating_add(step),
      None => self.first.saturating_sub(step),
    }
    .min(maximum);
    let last = self
      .first
      .saturating_add(page.unsigned_abs().saturating_sub(1));
    self.selected = self.selected.clamp(self.first, last);
    let Ok(()) = self.invalidate();
    Ok(())
  }

  pub(crate) fn paint(
    &mut self,
    pixels: &mut [u8],
    frame: Frame,
    palette: &Wearing,
    font: &Font,
  ) -> Result<(), painting::Cannot> {
    let Ok(width) = console_core_number_conversion::fitted::<_, i32>(frame.points.width);
    let Ok(height) = console_core_number_conversion::fitted::<_, i32>(frame.points.height);
    let Ok(layout) = Layout::new(Size { width, height });
    self.layout = Some(layout);
    let Ok(()) = self.reveal();

    let Ok(backdrop) = panel(
      Rectangle {
        origin: Point { x: 0, y: 0 },
        size: Size { width, height },
      },
      0,
      palette.ground,
      Edge::None,
    );
    let mut shapes = vec![backdrop];
    let title_font = Font {
      family: font.family.clone(),
      height: 22,
    };
    let Ok(()) = label_at(
      &mut shapes,
      layout.title,
      "Downloads",
      &title_font,
      palette.text,
      Alignment::Start,
    );
    let Ok(()) = self.paint_button(
      &mut shapes,
      layout.folder,
      "Open folder",
      Focus::Folder,
      palette,
      font,
    );
    let Ok(()) = self.paint_button(
      &mut shapes,
      layout.refresh,
      "Refresh",
      Focus::Refresh,
      palette,
      font,
    );
    let Ok(()) = self.paint_search(&mut shapes, layout.search, palette, font);
    let Ok(()) = self.paint_rows(&mut shapes, layout, palette, font);
    let Ok(()) = self.paint_menu(&mut shapes, layout, palette, font);

    painting::onto(pixels, frame, &shapes)?;

    match self.window {
      Window::Hidden => {}
      Window::Visible(_) => self.window = Window::Visible(Drawing::Current),
    }

    Ok(())
  }

  fn paint_button(
    &self,
    shapes: &mut Vec<Shape>,
    area: Rectangle<i32>,
    name: &str,
    focus: Focus,
    palette: &Wearing,
    font: &Font,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(background) = panel(area, 6, palette.fill, {
        let Ok(explicit_value) = edge(Highlight::from(self.focus == focus), palette);
        explicit_value
      });
      shapes.push(background);
      let Ok(()) = label_at(shapes, area, name, font, palette.text, Alignment::Center);
    };

    Ok(())
  }

  fn paint_search(
    &self,
    shapes: &mut Vec<Shape>,
    area: Rectangle<i32>,
    palette: &Wearing,
    font: &Font,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let focused = self.focus == Focus::Search;
      let Ok(field) = panel(area, 8, palette.night, {
        let Ok(explicit_value) = edge(Highlight::from(focused), palette);
        explicit_value
      });
      shapes.push(field);
      let Ok(query) = self.search.text();
      let Ok(before) = self.search.before_caret();
      let Ok(measured_prefix) = measured(before, font);
      let Ok(caret) = console_core_number_conversion::fitted::<_, i32>(measured_prefix.width);
      let text_width = area.size.width.saturating_sub(18).max(1);
      let x = area.origin.x.saturating_add(9);
      let overflow = caret.saturating_sub(text_width.saturating_sub(1)).max(0);
      let label = match { ::core::primitive::bool::then_some(query.is_empty(), ()) } {
        ::core::option::Option::Some(()) => "Search by name, site, date, or ID",
        ::core::option::Option::None => query,
      };
      let ink = match { ::core::primitive::bool::then_some(query.is_empty(), ()) } {
        ::core::option::Option::Some(()) => palette.soft,
        ::core::option::Option::None => palette.text,
      };
      let Ok(metrics) = measured(label, font);
      let Ok(line_height) = console_core_number_conversion::fitted::<_, i32>(metrics.height);
      let y = area
        .origin
        .y
        .saturating_add(area.size.height.saturating_sub(line_height).div_euclid(2));
      shapes.push(Shape::Clip(Clip::To {
        at: Point {
          x,
          y: area.origin.y,
        },
        size: Size {
          width: text_width.unsigned_abs(),
          height: area.size.height.unsigned_abs(),
        },
      }));

      match {
        ::core::primitive::bool::then_some(
          focused && matches!(self.selection, Selection::All) && !query.is_empty(),
          (),
        )
      } {
        ::core::option::Option::Some(()) => {
          let Ok(selection) = panel(
            Rectangle {
              origin: Point { x, y },
              size: Size {
                width: caret.min(text_width),
                height: line_height,
              },
            },
            0,
            palette.fill,
            Edge::None,
          );
          shapes.push(selection);
        }
        ::core::option::Option::None => {}
      }

      let Ok(line) = words(
        label,
        font,
        Point {
          x: x.saturating_sub(overflow),
          y,
        },
        metrics.width.saturating_add(4),
        ink,
      );
      shapes.push(line);

      match {
        ::core::primitive::bool::then_some(
          focused && matches!(self.selection, Selection::Cursor),
          (),
        )
      } {
        ::core::option::Option::Some(()) => {
          let at = x.saturating_add(caret.saturating_sub(overflow));
          shapes.push(Shape::Line(Line {
            from: Point { x: at, y },
            to: Point {
              x: at,
              y: y.saturating_add(line_height),
            },
            width: 1,
            color: palette.pink,
          }));
        }
        ::core::option::Option::None => {}
      }

      shapes.push(Shape::Clip(Clip::Lifted));
    };

    Ok(())
  }

  fn paint_catalog_status(
    &self,
    shapes: &mut Vec<Shape>,
    area: Rectangle<i32>,
    palette: &Wearing,
    font: &Font,
  ) -> Result<(), Never> {
    let Ok(count) = self.count();
    let Ok(query) = self.search.text();
    let status = match &self.catalog {
      CatalogState::Failed { message, .. } => String::from(*message),
      CatalogState::Scanning(_) => String::from("Indexing downloads…"),
      CatalogState::Empty | CatalogState::Ready(_) => match (count, query.trim().is_empty()) {
        (0, true) => String::from("No downloads yet"),
        (0, false) => String::from("No matching downloads"),
        (1, _) => String::from("1 download"),
        (_, _) => format!("{count} downloads"),
      },
    };

    label_at(shapes, area, &status, font, palette.soft, Alignment::Start)
  }

  fn paint_rows(
    &mut self,
    shapes: &mut Vec<Shape>,
    layout: Layout,
    palette: &Wearing,
    font: &Font,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      self.drawn_rows.clear();
      let Ok(()) = self.paint_catalog_status(shapes, layout.status, palette, font);

      let catalog = {
        let Ok(optional_catalog) = self.catalog.snapshot();

        match optional_catalog {
          Some(matched_catalog) => matched_catalog,
          ::core::option::Option::None => return Ok(()),
        }
      };

      let subtitle = Font {
        family: font.family.clone(),
        height: 12,
      };
      let mut y = ROW_TOP;

      let Ok(visible_rows) = layout.visible_rows();
      let last = self.first.saturating_add(u64::from(visible_rows));

      for position in self.first..last {
        let record = {
          let Ok(optional_record) = catalog.row(&self.matches, position);

          match optional_record {
            Some(matched_record) => matched_record,
            ::core::option::Option::None => break,
          }
        };

        self.drawn_rows.push(record.id.clone());
        let focused = self.focus == Focus::Rows && self.selected == position;
        let Ok(row) = layout.row(y);
        let style = DownloadRow {
          palette,
          font,
          subtitle: &subtitle,
          highlight: Highlight::from(focused),
        };
        let Ok(()) = paint_download(shapes, row, record, style);
        y = y.saturating_add(ROW_HEIGHT);
      }
    };

    Ok(())
  }

  fn paint_menu(
    &mut self,
    shapes: &mut Vec<Shape>,
    layout: Layout,
    palette: &Wearing,
    font: &Font,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let menu = match &mut self.menu {
        Some(matched_menu) => matched_menu,
        ::core::option::Option::None => return Ok(()),
      };

      let Ok((bounds, [reveal, trash])) = layout.menu(menu.anchor);
      menu.drawn = Some([reveal, trash]);
      let Ok(background) = panel(bounds, 7, palette.night, {
        let Ok(explicit_value) = edge(Highlight::Normal, palette);
        explicit_value
      });
      shapes.push(background);

      for (area, action, label, ink) in [
        (reveal, FileAction::Reveal, "Reveal in Folder", palette.text),
        (trash, FileAction::Trash, "Move to Trash", palette.coral),
      ] {
        match { ::core::primitive::bool::then_some(menu.focus == action, ()) } {
          ::core::option::Option::Some(()) => {
            let Ok(selected) = panel(area, 5, palette.fill, {
              let Ok(explicit_value) = edge(Highlight::Focused, palette);
              explicit_value
            });
            shapes.push(selected);
          }
          ::core::option::Option::None => {}
        }

        let Ok(()) = label_at(shapes, area, label, font, ink, Alignment::Start);
      }
    };

    Ok(())
  }
}

#[derive(Clone, Copy)]
struct Layout {
  title: Rectangle<i32>,
  folder: Rectangle<i32>,
  refresh: Rectangle<i32>,
  search: Rectangle<i32>,
  status: Rectangle<i32>,
  rows: Rectangle<i32>,
}

impl Layout {
  #[must_use]
  fn new(size: Size<i32>) -> Result<Self, Never> {
    let Size { width, height } = size;
    let content = width.saturating_sub(24).clamp(1, 900);
    let left = width.saturating_sub(content).div_euclid(2);
    let refresh = content.div_euclid(4).min(88);
    let folder = content.div_euclid(3).min(110);
    let title = content
      .saturating_sub(folder)
      .saturating_sub(refresh)
      .saturating_sub(16)
      .max(1);
    let right = left.saturating_add(content);

    Ok(Self {
      title: (Rectangle {
        origin: Point { x: left, y: 15 },
        size: Size {
          width: title,
          height: 34,
        },
      }),
      folder: (Rectangle {
        origin: Point {
          x: right
            .saturating_sub(refresh)
            .saturating_sub(8)
            .saturating_sub(folder),
          y: 15,
        },
        size: Size {
          width: folder,
          height: 34,
        },
      }),
      refresh: (Rectangle {
        origin: Point {
          x: right.saturating_sub(refresh),
          y: 15,
        },
        size: Size {
          width: refresh,
          height: 34,
        },
      }),
      search: (Rectangle {
        origin: Point { x: left, y: 58 },
        size: Size {
          width: content,
          height: 34,
        },
      }),
      status: (Rectangle {
        origin: Point { x: left, y: 93 },
        size: Size {
          width: content,
          height: 24,
        },
      }),
      rows: (Rectangle {
        origin: Point {
          x: left,
          y: ROW_TOP,
        },
        size: Size {
          width: content,
          height: height.saturating_sub(ROW_TOP).max(0),
        },
      }),
    })
  }

  #[must_use]
  fn visible_rows(self) -> core::result::Result<u32, console_core_never::Never> {
    console_core_number_conversion::fitted(self.rows.size.height.div_euclid(ROW_HEIGHT).max(0))
  }

  #[must_use]
  fn row(self, y: i32) -> core::result::Result<Rectangle<i32>, console_core_never::Never> {
    Ok(Rectangle {
      origin: Point {
        x: self.rows.origin.x,
        y,
      },
      size: Size {
        width: self.rows.size.width,
        height: ROW_HEIGHT.saturating_sub(4),
      },
    })
  }

  #[must_use]
  fn menu(
    self,
    anchor: i32,
  ) -> core::result::Result<(Rectangle<i32>, [Rectangle<i32>; 2]), console_core_never::Never> {
    let height = 76;
    let width = self.rows.size.width.min(188);
    let max_top = self
      .rows
      .origin
      .y
      .saturating_add(self.rows.size.height)
      .saturating_sub(height)
      .max(0);
    let top = anchor.saturating_add(ROW_HEIGHT).min(max_top);
    let x = self
      .rows
      .origin
      .x
      .saturating_add(self.rows.size.width.saturating_sub(width));
    let action = |y| Rectangle {
      origin: Point {
        x: x.saturating_add(4),
        y,
      },
      size: Size {
        width: width.saturating_sub(8),
        height: 32,
      },
    };

    Ok((
      (Rectangle {
        origin: Point { x, y: top },
        size: Size { width, height },
      }),
      [
        action(top.saturating_add(4)),
        action(top.saturating_add(40)),
      ],
    ))
  }
}

#[must_use]
fn actions_button(
  row: Rectangle<i32>,
) -> core::result::Result<Rectangle<i32>, console_core_never::Never> {
  let width = row.size.width.saturating_sub(16).clamp(0, 38);
  let left = row
    .origin
    .x
    .saturating_add(row.size.width.saturating_sub(width).saturating_sub(8));

  Ok(Rectangle {
    origin: Point {
      x: left,
      y: row.origin.y.saturating_add(10),
    },
    size: Size { width, height: 32 },
  })
}

struct DownloadRow<'a> {
  palette: &'a Wearing,
  font: &'a Font,
  subtitle: &'a Font,
  highlight: Highlight,
}

fn paint_download(
  shapes: &mut Vec<Shape>,
  row: Rectangle<i32>,
  record: &Record,
  style: DownloadRow<'_>,
) -> Result<(), Never> {
  let DownloadRow {
    palette,
    font,
    subtitle,
    highlight,
  } = style;
  let Point { x, y } = row.origin;
  let Ok(actions) = actions_button(row);
  let label_width = actions.origin.x.saturating_sub(x).saturating_sub(12);
  let Ok(background) = panel(row, 7, palette.panel, {
    let Ok(explicit_value) = edge(highlight, palette);
    explicit_value
  });
  shapes.push(background);
  let Ok(action_background) = panel(actions, 5, palette.fill, Edge::None);
  shapes.push(action_background);
  let Ok(()) = label_at(shapes, actions, "⋯", font, palette.text, Alignment::Center);
  let Ok(()) = label_at(
    shapes,
    Rectangle {
      origin: Point {
        x: x.saturating_add(6),
        y,
      },
      size: Size {
        width: label_width,
        height: 29,
      },
    },
    &record.label,
    font,
    palette.text,
    Alignment::Start,
  );
  let Ok(id_text) = record.id.as_str();
  let short_id = match id_text.split_once('-') {
    Some((prefix, _suffix)) => prefix,
    None => id_text,
  };
  let summary = [
    record.site.as_deref(),
    record.date.as_deref(),
    Some(short_id),
  ]
  .into_iter()
  .flatten()
  .collect::<Vec<_>>()
  .join("  ·  ");
  let Ok(()) = label_at(
    shapes,
    Rectangle {
      origin: Point {
        x: x.saturating_add(6),
        y: y.saturating_add(27),
      },
      size: Size {
        width: label_width,
        height: 20,
      },
    },
    &summary,
    subtitle,
    palette.soft,
    Alignment::Start,
  );

  Ok(())
}

#[derive(Clone, Copy)]
enum Highlight {
  Focused,
  Normal,
}

impl From<bool> for Highlight {
  fn from(focused: bool) -> Self {
    match focused {
      true => Self::Focused,
      false => Self::Normal,
    }
  }
}

#[must_use]
fn edge(
  highlight: Highlight,
  palette: &Wearing,
) -> core::result::Result<Edge, console_core_never::Never> {
  Ok(Edge::Of {
    wide: 1,
    color: match highlight {
      Highlight::Focused => palette.pink,
      Highlight::Normal => palette.edge,
    },
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn escape_leaves_the_download_search_before_dismissing_the_view() {
    let mut view = View::default();
    let Ok(()) = view.open();
    let Ok(query) = TextField::holding("report");
    view.search = query;

    assert!(matches!(view.key(Keysym::Escape, 0), Ok(Action::Handled)));
    assert_eq!(view.search.text(), Ok("report"));
    assert!(matches!(view.key(Keysym::Escape, 0), Ok(Action::Dismiss)));
  }
}
