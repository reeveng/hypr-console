//! The controls above the page share placement for painting, tapping and
//! directional navigation. The engine occupies the space below this row.

#![forbid(unsafe_code)]

use console_core_color::{
  Oklch,
  palette::{self, Wearing},
};
use console_core_fonts::{Font, TextStyle, Weight};
use console_core_geometry::{Point, Rectangle, Size};
use console_core_highlight::{Actions, Heading, Press, Resolution, under};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_shapes::{Clip, Edge, Line, Panel, Round, Shape, Text};
use console_draw_painting::{self as drawing, Frame, Run};
use console_input_key_symbols::Keysym;
use console_input_text::TextField;

use crate::reader::{Mode, TextSize};
use crate::renderer::Renderer;
use crate::settings::{Alignment, Settings, label_at};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
  Back,
  Forward,
  Reload,
  Address,
  Settings,
  Close,
  Tab(u32),
  NewTab,
  CloseTab,
}

const CONTROLS: [Control; 6] = [
  Control::Back,
  Control::Forward,
  Control::Reload,
  Control::Address,
  Control::Settings,
  Control::Close,
];
const ADDRESS: u32 = 3;
const SETTINGS: u32 = 4;
pub(crate) const HEIGHT: u32 = 86;
pub(crate) const MAX_TABS: usize = 6;
pub(crate) const CONTROL: u32 = 1;
pub(crate) const SHIFT: u32 = 2;
const ALT: u32 = 4;
const CONTROL_SHIFT: u32 = CONTROL | SHIFT;

#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub enum Action {
  Pass,
  Handled,
  Compose,
  Back,
  Forward,
  Reload,
  Stop,
  Close,
  CloseTab,
  NewTab,
  SelectTab(u32),
  CycleTab(Direction),
  Reader(Mode),
  PageView,
  ReaderText(TextSize),
  SettingsChanged,
  Open(String),
  Switch(Renderer),
  FocusPage,
  Copy(String),
  Paste,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Direction {
  Previous,
  Next,
}

#[derive(PartialEq, Eq)]
pub(crate) struct TabLabel {
  pub id: u32,
  pub title: String,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum PageLoad {
  #[default]
  Settled,
  Loading,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ButtonAvailability {
  #[default]
  Disabled,
  Enabled,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ToolbarChange {
  Unchanged,
  Changed,
}

impl From<bool> for ToolbarChange {
  fn from(changed: bool) -> Self {
    match changed.then_some(()) {
      Some(()) => Self::Changed,
      None => Self::Unchanged,
    }
  }
}

enum FocusMark {
  Focused,
  Passive,
}

#[derive(Default, PartialEq, Eq)]
pub(crate) struct Status {
  pub(crate) loading: PageLoad,
  pub(crate) back: ButtonAvailability,
  pub(crate) forward: ButtonAvailability,
  pub(crate) renderer: Renderer,
  pub(crate) progress: u32,
  pub(crate) reader: Option<Mode>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Editing {
  Inactive,
  Draft,
  Selected,
}

pub struct Toolbar {
  palette: Wearing,
  font: Font,
  line_height: u32,
  renderers: &'static [Renderer],
  address: TextField,
  layout: [Rectangle<i32>; CONTROLS.len()],
  tabs: Vec<TabLabel>,
  tab_controls: Vec<Control>,
  tab_layout: Vec<Rectangle<i32>>,
  active_tab: u32,
  status: Status,
  focus: Option<u32>,
  editing: Editing,
  settings: Settings,
  pointer: Option<Point<i32>>,
}

impl Toolbar {
  pub fn with_renderers(renderers: &'static [Renderer]) -> Result<Self, palette::PaletteError> {
    let wearing = match Wearing::worn() {
      Ok(installed) => installed,
      Err(fault) => {
        console_core_logging::debug!("browser toolbar using its shipped palette: {fault}");
        let Ok(shipped) = palette::read(include_str!(
          "../../../files/usr/local/lib/console/palette.sh"
        ));
        Wearing::out_of(&shipped)?
      }
    };
    let Ok(font) = TextStyle::Footnote.font();
    let Ok(metrics) = measured("WebKit", &font);
    let line_height = metrics.height;
    let Ok(placement) = layout(Size {
      width: 1100,
      height: HEIGHT,
    });

    Ok(Self {
      palette: wearing,
      font,
      line_height,
      renderers,
      address: TextField::default(),
      layout: placement,
      tabs: Vec::new(),
      tab_controls: Vec::new(),
      tab_layout: Vec::new(),
      active_tab: 0,
      status: Status::default(),
      focus: None,
      editing: Editing::Inactive,
      settings: Settings::default(),
      pointer: None,
    })
  }

  pub(crate) fn tabs(&mut self, tabs: Vec<TabLabel>, active: u32) -> ToolbarChange {
    match (self.tabs == tabs && self.active_tab == active).then_some(()) {
      Some(()) => return ToolbarChange::Unchanged,
      None => {}
    }
    self.tabs = tabs;
    self.active_tab = active;
    ToolbarChange::Changed
  }

  pub(crate) fn reader_colors(&self) -> (Oklch, Oklch) {
    (self.palette.ground, self.palette.text)
  }

  pub(crate) fn refresh_palette(&mut self) {
    match Wearing::worn() {
      Ok(palette) => self.palette = palette,
      Err(_) => {}
    }
  }

  pub fn status(&mut self, next: Status) -> Result<ToolbarChange, Never> {
    let status = Status {
      progress: next.progress.min(1000),
      ..next
    };
    let change = ToolbarChange::from(self.status != status);

    match change {
      ToolbarChange::Changed => self.status = status,
      ToolbarChange::Unchanged => {}
    }

    Ok(change)
  }

  pub fn address(&mut self, address: &str) -> Result<ToolbarChange, Never> {
    let Ok(current) = self.address.text();
    let change = ToolbarChange::from(self.editing == Editing::Inactive && current != address);

    match change {
      ToolbarChange::Changed => {
        let Ok(field) = TextField::holding(address);
        self.address = field;
      }
      ToolbarChange::Unchanged => {}
    }

    Ok(change)
  }

  pub fn blur(&mut self, committed: &str) -> Result<(), Never> {
    self.focus = None;
    self.editing = Editing::Inactive;
    let Ok(_) = self.address(committed);

    Ok(())
  }

  fn focus(&mut self) -> Result<(), Never> {
    self.focus.get_or_insert(ADDRESS);

    Ok(())
  }

  pub(crate) fn edit(&mut self) -> Result<(), Never> {
    self.focus = Some(ADDRESS);
    self.editing = Editing::Selected;

    Ok(())
  }

  pub fn settings_visible(&self) -> bool {
    self.settings.visible()
  }

  pub fn hover(&mut self, point: Option<Point<i32>>) -> ToolbarChange {
    let change =
      ToolbarChange::from(self.over_settings(self.pointer) != self.over_settings(point));
    self.pointer = point;
    change
  }

  fn over_settings(&self, point: Option<Point<i32>>) -> Option<u32> {
    let Ok(button) = self.at(Control::Settings);
    point.and_then(|point| {
      let Ok(hit) = under(point, &[button]);
      hit
    })
  }

  pub fn paint_settings(&mut self, pixels: &mut [u8], frame: Frame) -> Result<(), Never> {
    match self.settings.paint(
      pixels,
      frame,
      &self.palette,
      &self.font,
      &self.status,
      self.renderers,
    ) {
      Ok(()) => {}
      Err(fault) => console_core_logging::error!("could not paint browser settings: {fault}"),
    }
    Ok(())
  }

  fn at(&self, control: Control) -> Result<Rectangle<i32>, Never> {
    let [back, forward, reload, address, settings, close] = self.layout;

    Ok(match control {
      Control::Back => back,
      Control::Forward => forward,
      Control::Reload => reload,
      Control::Address => address,
      Control::Settings => settings,
      Control::Close => close,
      Control::Tab(_) | Control::NewTab | Control::CloseTab => tab_button(0, 0),
    })
  }

  fn activate(&mut self, control: Control) -> Result<Action, Never> {
    Ok(match control {
      Control::Back => match self.status.back {
        ButtonAvailability::Enabled => Action::Back,
        ButtonAvailability::Disabled => Action::Handled,
      },
      Control::Forward => match self.status.forward {
        ButtonAvailability::Enabled => Action::Forward,
        ButtonAvailability::Disabled => Action::Handled,
      },
      Control::Reload => match self.status.loading {
        PageLoad::Loading => Action::Stop,
        PageLoad::Settled => Action::Reload,
      },
      Control::Close => Action::Close,
      Control::Settings => {
        self.settings.open();
        Action::SettingsChanged
      }
      Control::Tab(id) => Action::SelectTab(id),
      Control::NewTab => match self.tabs.len().cmp(&MAX_TABS) {
        std::cmp::Ordering::Less => Action::NewTab,
        std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => Action::Handled,
      },
      Control::CloseTab => Action::CloseTab,
      Control::Address => {
        match ::core::primitive::bool::then_some(self.editing != Editing::Inactive, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(address) = self.address.text();
            Action::Open(address.to_owned())
          }
          ::core::option::Option::None => {
            let Ok(()) = self.edit();
            Action::Handled
          }
        }
      }
    })
  }

  pub fn tap(&mut self, at: Point<i32>, committed: &str) -> Result<Action, Never> {
    match self.settings.visible().then_some(()) {
      Some(()) => return Ok(self.settings.tap(at, &self.status)),
      None => {}
    }
    let press = Press::Tap(at);
    let Ok(resolved) = self.resolve(press);
    let (target, action) = match resolved {
      Resolution::Activate { target, action } => (target, action),
      Resolution::Focus(_) | Resolution::Pass => return Ok(Action::Pass),
    };

    match (action == Control::Address && self.editing != Editing::Inactive).then_some(()) {
      Some(()) => return Ok(Action::Handled),
      None => {}
    }

    let Ok(()) = self.blur(committed);
    self.focus = Some(target);
    self.activate(action)
  }

  fn resolve(&self, press: Press) -> Result<Resolution<Control>, Never> {
    let mut frames = self.layout.to_vec();
    frames.extend_from_slice(&self.tab_layout);
    let mut controls = CONTROLS.to_vec();
    controls.extend_from_slice(&self.tab_controls);
    match Actions::new(&frames, &controls) {
      Ok(actions) => actions.resolve(press, self.focus),
      Err(fault) => {
        console_core_logging::error!(
          "browser toolbar layout has no matching controls: {fault:?}"
        );

        Ok(Resolution::Pass)
      }
    }
  }

  pub fn text(&mut self, text: &str) -> Result<(), Never> {
    match self.editing {
      Editing::Inactive => return Ok(()),
      Editing::Draft => {}
      Editing::Selected => {
        self.address = TextField::default();
        self.editing = Editing::Draft;
      }
    }

    for character in text.chars().filter(|character| !character.is_control()) {
      let Ok(_) = self.address.insert(character);
    }

    Ok(())
  }

  pub fn key(&mut self, key: Keysym, mods: u32, committed: &str) -> Result<Action, Never> {
    match self.settings.visible().then_some(()) {
      Some(()) => return Ok(self.settings.key(key, mods, &self.status)),
      None => {}
    }
    match (mods, key) {
      (0, Keysym::F6 | Keysym::F18 | Keysym::XF86Launch9) => {
        return Ok(match self.focus {
          Some(_) => {
            let Ok(()) = self.blur(committed);

            Action::FocusPage
          }
          None => {
            let Ok(()) = self.focus();

            Action::Handled
          }
        });
      }
      (CONTROL, Keysym::l) => {
        let Ok(()) = self.edit();

        return Ok(Action::Handled);
      }
      (CONTROL, Keysym::r) => return Ok(Action::Reload),
      (CONTROL, Keysym::w) => return Ok(Action::CloseTab),
      (CONTROL, Keysym::t) => return Ok(Action::NewTab),
      (CONTROL, Keysym::Tab) => return Ok(Action::CycleTab(Direction::Next)),
      (CONTROL_SHIFT, Keysym::Tab | Keysym::ISO_Left_Tab) => {
        return Ok(Action::CycleTab(Direction::Previous));
      }
      (ALT, Keysym::Left) => return Ok(Action::Back),
      (ALT, Keysym::Right) => return Ok(Action::Forward),
      _ => {}
    }

    match self.focus {
      Some(_) => {}
      None => return Ok(Action::Pass),
    }

    let draft = self.editing != Editing::Inactive;

    match (key, draft, mods & SHIFT != 0) {
      (Keysym::Escape, _, _) => {
        let Ok(()) = self.blur(committed);

        Ok(Action::FocusPage)
      }
      (Keysym::Left, false, _) => self.move_focus(Heading::Left, committed),
      (Keysym::Right, false, _) => self.move_focus(Heading::Right, committed),
      (Keysym::Up, false, _) => self.move_focus(Heading::Up, committed),
      (Keysym::Down, false, _) => self.move_focus(Heading::Down, committed),
      (Keysym::ISO_Left_Tab, _, _) | (Keysym::Tab, _, true) => {
        self.move_focus(Heading::Previous, committed)
      }
      (Keysym::Tab, _, false) => self.move_focus(Heading::Next, committed),
      (Keysym::Return | Keysym::KP_Enter, _, _) | (Keysym::space, false, _) => {
        let Ok(selected) = self.resolve(Press::Select);

        match selected {
          Resolution::Activate { action, .. } => self.activate(action),
          Resolution::Focus(_) | Resolution::Pass => Ok(Action::Pass),
        }
      }
      (_, true, _) => self.edit_key(key, mods),
      (_, false, _) => Ok(Action::Handled),
    }
  }

  fn move_focus(&mut self, heading: Heading, committed: &str) -> Result<Action, Never> {
    let Ok(resolved) = self.resolve(Press::Toward(heading));

    Ok(match (resolved, heading) {
      (Resolution::Focus(next), _) => {
        let Ok(()) = self.blur(committed);
        self.focus = Some(next);
        Action::Handled
      }
      (
        Resolution::Activate { .. } | Resolution::Pass,
        Heading::Previous | Heading::Next | Heading::Down,
      ) => {
        let Ok(()) = self.blur(committed);

        Action::FocusPage
      }
      _ => Action::Handled,
    })
  }

  fn edit_key(&mut self, key: Keysym, mods: u32) -> Result<Action, Never> {
    Ok(match (mods, key, self.editing) {
      (CONTROL, Keysym::a, _) => {
        self.editing = Editing::Selected;
        Action::Handled
      }
      (CONTROL, Keysym::v, _) => Action::Paste,
      (CONTROL, Keysym::c | Keysym::x, Editing::Selected) => {
        let Ok(address) = self.address.text();
        let copied = address.to_owned();

        match key {
          Keysym::x => {
            self.address = TextField::default();
            self.editing = Editing::Draft;
          }
          _ => {}
        }

        Action::Copy(copied)
      }
      (
        0 | SHIFT,
        Keysym::Left
        | Keysym::Right
        | Keysym::Home
        | Keysym::End
        | Keysym::BackSpace
        | Keysym::Delete,
        _,
      ) => {
        match (self.editing, key) {
          (Editing::Selected, Keysym::BackSpace | Keysym::Delete) => {
            self.address = TextField::default()
          }
          (Editing::Selected, Keysym::Left) => {
            let Ok(_) = self.address.moved_to(0);
          }
          (Editing::Selected, Keysym::Right) => {}
          _ => {
            let Ok(_) = self.address.typed(key);
          }
        }

        self.editing = Editing::Draft;
        Action::Handled
      }
      (0 | SHIFT, _, _) => Action::Compose,
      _ => Action::Handled,
    })
  }

  pub fn paint(&mut self, pixels: &mut [u8], frame: Frame) -> Result<(), drawing::Cannot> {
    let Ok(layout) = layout(frame.points);
    self.layout = layout;
    let Ok(width) = positive(frame.points.width);
    let Ok(height) = positive(frame.points.height);
    let bounds = Rectangle {
      origin: Point { x: 0, y: 0 },
      size: Size { width, height },
    };
    let Ok(backdrop) = panel(bounds, 0, self.palette.panel, Edge::None);
    let mut shapes = vec![backdrop];
    let Ok(()) = self.paint_button(&mut shapes, Control::Back, self.status.back);
    let Ok(()) = self.paint_button(&mut shapes, Control::Forward, self.status.forward);
    let Ok(()) = self.paint_button(&mut shapes, Control::Reload, ButtonAvailability::Enabled);
    let Ok(()) = self.paint_button(&mut shapes, Control::Close, ButtonAvailability::Enabled);
    let Ok(()) = self.paint_button(&mut shapes, Control::Settings, ButtonAvailability::Enabled);
    self.paint_tabs(&mut shapes, width);
    let Ok(()) = self.paint_address(&mut shapes);
    self.paint_tooltip(&mut shapes, width);

    match (self.status.loading, self.status.progress) {
      (PageLoad::Loading, 1..) => {
        let progressed = frame
          .points
          .width
          .saturating_mul(self.status.progress)
          .wrapping_div(1000);
        let Ok(wide) = positive(progressed);

        {
          let Ok(shape) = panel(
            Rectangle {
              origin: Point {
                x: 0,
                y: height.saturating_sub(2),
              },
              size: Size {
                width: wide,
                height: 2,
              },
            },
            0,
            self.palette.pink,
            Edge::None,
          );
          shapes.push(shape);
        }
      }
      _ => {}
    }

    drawing::onto(pixels, frame, &shapes)
  }

  fn focus_edge(&self, focused: FocusMark) -> Result<Edge, Never> {
    Ok(match focused {
      FocusMark::Focused => Edge::Of {
        wide: 1,
        color: self.palette.pink,
      },
      FocusMark::Passive => Edge::None,
    })
  }

  fn paint_button(
    &self,
    shapes: &mut Vec<Shape>,
    control: Control,
    availability: ButtonAvailability,
  ) -> Result<(), Never> {
    let Ok(place) = self.at(control);
    let focused = match (self.focus, control) {
      (Some(0), Control::Back)
      | (Some(1), Control::Forward)
      | (Some(2), Control::Reload)
      | (Some(SETTINGS), Control::Settings)
      | (Some(5), Control::Close) => FocusMark::Focused,
      (_, Control::Settings) => match self.settings.visible().then_some(()) {
        Some(()) => FocusMark::Focused,
        None => FocusMark::Passive,
      },
      _ => FocusMark::Passive,
    };

    let Ok(highlight) = self.focus_edge(FocusMark::Focused);

    match ::core::primitive::bool::then_some(matches!(focused, FocusMark::Focused), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(shape) = panel(place, 6, self.palette.fill, highlight);
        shapes.push(shape);
      }
      ::core::option::Option::None => {}
    }

    let color = match availability {
      ButtonAvailability::Enabled => self.palette.text,
      ButtonAvailability::Disabled => self.palette.edge,
    };
    let x = place
      .origin
      .x
      .saturating_add(place.size.width.wrapping_div(2));
    let y = place
      .origin
      .y
      .saturating_add(place.size.height.wrapping_div(2));
    let stroke = |(ax, ay), (bx, by)| {
      Shape::Line(Line {
        from: Point {
          x: x.saturating_add(ax),
          y: y.saturating_add(ay),
        },
        to: Point {
          x: x.saturating_add(bx),
          y: y.saturating_add(by),
        },
        width: 2,
        color,
      })
    };

    match (control, self.status.loading) {
      (Control::Back | Control::Forward, _) => {
        let direction = match control {
          Control::Back => -1,
          Control::Forward
          | Control::Reload
          | Control::Address
          | Control::Settings
          | Control::Close
          | Control::Tab(_)
          | Control::NewTab
          | Control::CloseTab => 1,
        };
        let tip = (6i32.saturating_mul(direction), 0);
        shapes.extend([
          stroke(((-6i32).saturating_mul(direction), 0), tip),
          stroke((direction, -5), tip),
          stroke((direction, 5), tip),
        ]);
      }
      (Control::Reload, PageLoad::Settled) => {
        let mut font = self.font.clone();
        font.height = 20;
        let Ok(size) = measured("↻", &font);
        let Ok(wide) = positive(size.width);
        let Ok(tall) = positive(size.height);

        {
          let Ok(shape) = words(
            "↻",
            &font,
            Point {
              x: x.saturating_sub(wide.wrapping_div(2)),
              y: y.saturating_sub(tall.wrapping_div(2)),
            },
            size.width.saturating_add(2),
            color,
          );
          shapes.push(shape);
        }
      }
      (Control::Reload, PageLoad::Loading) | (Control::Close, _) => {
        shapes.extend([stroke((-5, -5), (5, 5)), stroke((-5, 5), (5, -5))]);
      }
      (Control::Settings, _) => {
        shapes.extend([-5, 0, 5].map(|row| stroke((-7, row), (7, row))));
      }
      (Control::Address | Control::Tab(_) | Control::NewTab | Control::CloseTab, _) => {}
    }

    Ok(())
  }

  fn paint_tooltip(&self, shapes: &mut Vec<Shape>, width: i32) {
    match (
      self.over_settings(self.pointer),
      self.settings.visible().then_some(()),
    ) {
      (Some(_), None) => {}
      _ => return,
    }
    let Ok(button) = self.at(Control::Settings);
    let wide = 84i32.min(width.max(0));
    let center = button
      .origin
      .x
      .saturating_add(button.size.width.wrapping_div(2));
    let x = center
      .saturating_sub(wide.wrapping_div(2))
      .clamp(0, width.saturating_sub(wide).max(0));
    let y = button
      .origin
      .y
      .saturating_add(button.size.height)
      .saturating_add(4);
    let frame = Rectangle {
      origin: Point { x, y },
      size: Size {
        width: wide,
        height: 25,
      },
    };
    let Ok(background) = panel(
      frame,
      5,
      self.palette.night,
      Edge::Of {
        wide: 1,
        color: self.palette.edge,
      },
    );
    shapes.push(background);
    label_at(
      shapes,
      frame,
      "Settings",
      &self.font,
      self.palette.text,
      Alignment::Center,
    );
  }

  fn paint_tabs(&mut self, shapes: &mut Vec<Shape>, width: i32) {
    self.tab_layout.clear();
    self.tab_controls.clear();
    let Ok(count) = fitted::<_, i32>(self.tabs.len().max(1));
    let tab_width = width.saturating_sub(80).wrapping_div(count).clamp(1, 190);
    let mut x = 8;
    for (position, tab) in self.tabs.iter().enumerate() {
      let frame = tab_button(x, tab_width);
      let Ok(focus) = fitted(position.saturating_add(CONTROLS.len()));
      self.paint_label(
        shapes,
        frame,
        &tab.title,
        self.active_tab == tab.id,
        self.focus == Some(focus),
        Alignment::Start,
      );
      self.tab_controls.push(Control::Tab(tab.id));
      self.tab_layout.push(frame);
      x = x.saturating_add(tab_width);
    }
    for (control, label, enabled) in [
      (Control::NewTab, "+", self.tabs.len() < MAX_TABS),
      (Control::CloseTab, "×", !self.tabs.is_empty()),
    ] {
      let frame = tab_button(x, 32);
      let Ok(focus) = fitted(self.tab_layout.len().saturating_add(CONTROLS.len()));
      self.paint_label(
        shapes,
        frame,
        label,
        false,
        enabled && self.focus == Some(focus),
        Alignment::Center,
      );
      self.tab_controls.push(control);
      self.tab_layout.push(frame);
      x = x.saturating_add(32);
    }
  }

  fn paint_label(
    &self,
    shapes: &mut Vec<Shape>,
    frame: Rectangle<i32>,
    label: &str,
    active: bool,
    focused: bool,
    alignment: Alignment,
  ) {
    let Ok(edge) = self.focus_edge(match (focused || active).then_some(()) {
      Some(()) => FocusMark::Focused,
      None => FocusMark::Passive,
    });
    let fill = match active.then_some(()) {
      Some(()) => self.palette.ground,
      None => self.palette.fill,
    };
    let Ok(background) = panel(frame, 6, fill, edge);
    shapes.push(background);
    label_at(
      shapes,
      frame,
      label,
      &self.font,
      self.palette.text,
      alignment,
    );
  }

  fn paint_address(&self, shapes: &mut Vec<Shape>) -> Result<(), Never> {
    let Ok(field) = self.at(Control::Address);
    let focused = match self.focus {
      Some(ADDRESS) => FocusMark::Focused,
      Some(_) | None => FocusMark::Passive,
    };
    let Ok(edge) = self.focus_edge(focused);

    {
      let Ok(shape) = panel(field, 8, self.palette.night, edge);
      shapes.push(shape);
    }

    let left = field.origin.x.saturating_add(9);
    let wide = field.size.width.saturating_sub(18).max(0);
    let Ok(text) = self.address.text();
    let Ok(before) = self.address.before_caret();
    let caret = match self.editing {
      Editing::Draft => {
        let Ok(prefix) = measured(before, &self.font);
        let Ok(width) = positive(prefix.width);

        width
      }
      Editing::Inactive | Editing::Selected => 0,
    };
    let shift = caret.saturating_sub(wide.saturating_sub(1)).max(0);
    let said = match (text, self.editing) {
      ("", Editing::Inactive) => "Address or search",
      _ => text,
    };
    let ink = match ::core::primitive::bool::then_some(said == text, ()) {
      ::core::option::Option::Some(()) => self.palette.text,
      ::core::option::Option::None => self.palette.soft,
    };
    let available = wide.unsigned_abs();
    let Ok(line_height) = positive(self.line_height);
    let vertical = field.origin.y.saturating_add(
      field
        .size
        .height
        .saturating_sub(line_height)
        .wrapping_div(2),
    );
    let Ok(original) = measured(said, &self.font);
    let displayed = match ::core::primitive::bool::then_some(
      self.editing != Editing::Inactive || original.width <= available,
      (),
    ) {
      ::core::option::Option::Some(()) => std::borrow::Cow::Borrowed(said),
      ::core::option::Option::None => {
        let Ok(ellipsis) = measured("…", &self.font);
        let Ok(shortened) = drawing::at_most(
          Run {
            said,
            weight: Weight::Plain,
            width: available.saturating_sub(ellipsis.width),
          },
          &self.font,
          drawing::Lines(1),
        );
        std::borrow::Cow::Owned(shortened)
      }
    };
    shapes.push(Shape::Clip(Clip::To {
      at: Point {
        x: left,
        y: field.origin.y,
      },
      size: Size {
        width: available,
        height: field.size.height.unsigned_abs(),
      },
    }));

    match self.editing {
      Editing::Selected => {
        let Ok(selection) = measured(text, &self.font);
        let Ok(wide_2) = positive(selection.width.min(available));

        {
          let Ok(shape) = panel(
            Rectangle {
              origin: Point {
                x: left,
                y: vertical,
              },
              size: Size {
                width: wide_2,
                height: line_height,
              },
            },
            2,
            self.palette.fill,
            Edge::None,
          );
          shapes.push(shape);
        }
      }
      Editing::Inactive | Editing::Draft => {}
    }

    {
      let Ok(shape) = words(
        &displayed,
        &self.font,
        Point {
          x: left.saturating_sub(shift),
          y: vertical,
        },
        u32::MAX,
        ink,
      );
      shapes.push(shape);
    }

    match self.editing {
      Editing::Draft => {
        let Ok(shape) = panel(
          Rectangle {
            origin: Point {
              x: left.saturating_add(caret).saturating_sub(shift),
              y: vertical,
            },
            size: Size {
              width: 1,
              height: line_height,
            },
          },
          0,
          self.palette.pink,
          Edge::None,
        );
        shapes.push(shape);
      }
      Editing::Inactive | Editing::Selected => {}
    }

    shapes.push(Shape::Clip(Clip::Lifted));

    Ok(())
  }
}

fn positive(value: u32) -> Result<i32, Never> {
  fitted(value)
}

fn tab_button(x: i32, width: i32) -> Rectangle<i32> {
  Rectangle {
    origin: Point { x, y: 50 },
    size: Size {
      width: width.saturating_sub(4).max(0),
      height: 29,
    },
  }
}

fn layout(size: Size<u32>) -> Result<[Rectangle<i32>; CONTROLS.len()], Never> {
  let Ok(width) = positive(size.width);
  let Ok(height) = positive(size.height);
  let top = height.min(50).saturating_sub(34).max(0).wrapping_div(2);
  let spacing: i32 = match width {
    ..490 => 32,
    _ => 40,
  };
  let button_width = spacing.saturating_sub(4);
  let close_left = width.saturating_sub(button_width.saturating_add(8));
  let address_left = 8i32.saturating_add(3i32.saturating_mul(spacing));
  let settings_left = close_left.saturating_sub(button_width).saturating_sub(8);
  let placed = |x, width_2| Rectangle {
    origin: Point { x, y: top },
    size: Size {
      width: width_2,
      height: 34,
    },
  };
  let address_width = settings_left
    .saturating_sub(address_left)
    .saturating_sub(8)
    .max(0);

  Ok([
    placed(8, button_width),
    placed(8i32.saturating_add(spacing), button_width),
    placed(
      8i32.saturating_add(2i32.saturating_mul(spacing)),
      button_width,
    ),
    placed(address_left, address_width),
    placed(settings_left, button_width),
    placed(close_left, button_width),
  ])
}

pub(crate) fn panel(
  frame: Rectangle<i32>,
  round: u32,
  fill: Oklch,
  edge: Edge,
) -> Result<Shape, Never> {
  Ok(Shape::Panel(Panel {
    at: frame.origin,
    size: Size {
      width: frame.size.width.max(0).unsigned_abs(),
      height: frame.size.height.max(0).unsigned_abs(),
    },
    round: Round(round),
    fill,
    edge,
  }))
}

pub(crate) fn words(
  text: &str,
  font: &Font,
  at: Point<i32>,
  width: u32,
  ink: Oklch,
) -> Result<Shape, Never> {
  Ok(Shape::Text(Text {
    at,
    width,
    said: text.to_owned(),
    weight: Weight::Plain,
    font: font.clone(),
    ink,
  }))
}

pub(crate) fn measured(text: &str, font: &Font) -> Result<Size<u32>, Never> {
  drawing::measure_text(
    Run {
      said: text,
      weight: Weight::Plain,
      width: u32::MAX,
    },
    font,
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn keyboard_pad_and_pointer_resolve_the_same_navigation_commands()
  -> Result<(), palette::PaletteError> {
    const PAGE: &str = "https://example.org/";

    for (target, distance, loading, availability, expected) in [
      (
        Control::Back,
        3,
        PageLoad::Settled,
        ButtonAvailability::Enabled,
        Action::Back,
      ),
      (
        Control::Forward,
        2,
        PageLoad::Settled,
        ButtonAvailability::Enabled,
        Action::Forward,
      ),
      (
        Control::Reload,
        1,
        PageLoad::Settled,
        ButtonAvailability::Enabled,
        Action::Reload,
      ),
      (
        Control::Reload,
        1,
        PageLoad::Loading,
        ButtonAvailability::Enabled,
        Action::Stop,
      ),
      (
        Control::Back,
        3,
        PageLoad::Settled,
        ButtonAvailability::Disabled,
        Action::Handled,
      ),
      (
        Control::Forward,
        2,
        PageLoad::Settled,
        ButtonAvailability::Disabled,
        Action::Handled,
      ),
    ] {
      let mut toolbar = Toolbar::with_renderers(&[Renderer::WebKit, Renderer::Chromium])?;
      let Ok(_) = toolbar.status(Status {
        loading,
        back: availability,
        forward: availability,
        ..Status::default()
      });

      for (focus, step, modifiers) in [
        (Keysym::F6, Keysym::ISO_Left_Tab, SHIFT),
        (Keysym::XF86Launch9, Keysym::Left, 0),
      ] {
        assert_eq!(toolbar.key(focus, 0, PAGE), Ok(Action::Handled));

        for _ in 0..distance {
          assert_eq!(toolbar.key(step, modifiers, PAGE), Ok(Action::Handled));
        }

        assert_eq!(toolbar.key(Keysym::Return, 0, PAGE).as_ref(), Ok(&expected));
        let Ok(()) = toolbar.blur(PAGE);
      }

      let Ok(frame) = toolbar.at(target);
      assert_eq!(toolbar.tap(frame.origin, PAGE).as_ref(), Ok(&expected));
    }

    Ok(())
  }

  #[test]
  fn keyboard_and_pointer_open_settings() -> Result<(), palette::PaletteError> {
    let mut toolbar = Toolbar::with_renderers(Renderer::ALL.as_slice())?;
    let address = "https://example.org/";
    assert_eq!(toolbar.key(Keysym::F9, 0, address), Ok(Action::Pass));
    assert_eq!(toolbar.key(Keysym::F6, 0, address), Ok(Action::Handled));
    assert_eq!(toolbar.key(Keysym::Right, 0, address), Ok(Action::Handled));
    assert_eq!(
      toolbar.key(Keysym::Return, 0, address),
      Ok(Action::SettingsChanged)
    );
    assert!(toolbar.settings_visible());
    assert_eq!(
      toolbar.key(Keysym::F9, 0, address),
      Ok(Action::Reader(Mode::Dark))
    );
    assert_eq!(
      toolbar.key(Keysym::F9, SHIFT, address),
      Ok(Action::Reader(Mode::Desktop)),
    );
    assert_eq!(
      toolbar.key(Keysym::Escape, 0, address),
      Ok(Action::SettingsChanged)
    );
    let Ok(settings) = toolbar.at(Control::Settings);
    assert_eq!(
      toolbar.tap(settings.origin, address),
      Ok(Action::SettingsChanged)
    );
    assert!(toolbar.settings_visible());
    Ok(())
  }

  #[test]
  fn the_menu_glyph_has_a_hover_label() -> Result<(), palette::PaletteError> {
    let mut toolbar = Toolbar::with_renderers(Renderer::ALL.as_slice())?;
    let mut shapes = Vec::new();
    let Ok(()) =
      toolbar.paint_button(&mut shapes, Control::Settings, ButtonAvailability::Enabled);
    assert!(matches!(
      shapes.as_slice(),
      [Shape::Line(_), Shape::Line(_), Shape::Line(_)]
    ));
    shapes.clear();
    toolbar.paint_tooltip(&mut shapes, 1100);
    assert!(shapes.is_empty());
    let Ok(button) = toolbar.at(Control::Settings);
    assert_eq!(toolbar.hover(Some(button.origin)), ToolbarChange::Changed);
    toolbar.paint_tooltip(&mut shapes, 1100);
    let label = shapes.iter().find_map(|shape| match shape {
      Shape::Text(label) => Some(label.said.as_str()),
      _ => None,
    });
    assert_eq!(label, Some("Settings"));
    assert_eq!(toolbar.hover(None), ToolbarChange::Changed);
    shapes.clear();
    toolbar.paint_tooltip(&mut shapes, 1100);
    assert!(shapes.is_empty());
    Ok(())
  }

  #[test]
  fn the_close_button_uses_the_same_action_for_navigation_and_a_tap()
  -> Result<(), palette::PaletteError> {
    let mut toolbar = Toolbar::with_renderers(&[Renderer::WebKit, Renderer::Chromium])?;
    assert_eq!(toolbar.key(Keysym::F6, 0, ""), Ok(Action::Handled));
    assert_eq!(toolbar.key(Keysym::Right, 0, ""), Ok(Action::Handled));
    assert_eq!(toolbar.key(Keysym::Right, 0, ""), Ok(Action::Handled));
    assert_eq!(toolbar.key(Keysym::Return, 0, ""), Ok(Action::Close));
    let Ok(close) = toolbar.at(Control::Close);
    assert_eq!(toolbar.tap(close.origin, ""), Ok(Action::Close));

    Ok(())
  }

  #[test]
  fn editing_keeps_the_draft_until_submit_or_cancel() -> Result<(), palette::PaletteError> {
    let mut toolbar = Toolbar::with_renderers(&[Renderer::WebKit, Renderer::Chromium])?;
    let Ok(_) = toolbar.address("https://example.org/");
    assert!(matches!(
      toolbar.key(Keysym::l, CONTROL, ""),
      Ok(Action::Handled)
    ));
    let Ok(()) = toolbar.text("café");
    assert_eq!(
      toolbar.address("https://example.org/redirect"),
      Ok(ToolbarChange::Unchanged)
    );
    assert_eq!(
      toolbar.key(Keysym::Return, 0, ""),
      Ok(Action::Open("café".to_owned()))
    );
    assert!(matches!(
      toolbar.key(Keysym::Escape, 0, "https://example.org/redirect"),
      Ok(Action::FocusPage)
    ));
    assert_eq!(toolbar.address.text(), Ok("https://example.org/redirect"));

    Ok(())
  }
}
