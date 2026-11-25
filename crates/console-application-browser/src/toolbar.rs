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
use console_core_web_engine::DownloadState;
use console_draw_painting::{self as drawing, Frame, Run};
use console_draw_surface::KeyPress;
use console_input_key_symbols::Keysym;
use console_input_text::TextField;
use console_panel::strip::{TabFace, Tabs};
use console_panel::{marks, strip};

use crate::reader::{Mode, TextSize};
use crate::renderer::Renderer;
use crate::settings::{Alignment, Settings, Visibility as SettingsVisibility, label_at};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
  Back,
  Forward,
  Reload,
  Address,
  Settings,
  Close,
  Tab(u32),
  PreviousTab,
  NextTab,
  NewTab,
  CloseTab,
  Downloads,
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
const CORNER_RADIUS: u32 = 5;
const TAB_TOP: i32 = 4;
const ADDRESS_TOP: i32 = TAB_TOP + strip::HEIGHT + strip::GAP;
const ADDRESS_HEIGHT: i32 = 34;
const BROWSER_MARGIN: i32 = 8;
pub(crate) const HEIGHT: u32 = (ADDRESS_TOP + ADDRESS_HEIGHT + 6).unsigned_abs();
pub(crate) const MAX_TABS: u64 = 6;
pub(crate) const CONTROL: u32 = 1;
pub(crate) const SHIFT: u32 = 2;
const ALT: u32 = 4;
pub(crate) const CONTROL_SHIFT: u32 = CONTROL | SHIFT;

pub(crate) fn edit_press(key: Keysym, mods: u32) -> Result<KeyPress, Never> {
  let modifiers = match (mods & CONTROL != 0).then_some(()) {
    Some(()) => KeyPress::CONTROL,
    None => 0,
  };

  KeyPress::with_modifiers(key, modifiers)
}

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
  Downloads,
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
  Ready,
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
  Different,
}

impl From<bool> for ToolbarChange {
  fn from(changed: bool) -> Self {
    match changed.then_some(()) {
      Some(()) => Self::Different,
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DownloadsPanel {
  Closed,
  Open,
}

pub(crate) struct TextPrompt<'a> {
  pub(crate) placeholder: &'static str,
  pub(crate) value: &'a str,
}

pub(crate) enum EntryMode {
  Address,
  Prompt,
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
  download_status: Option<DownloadState>,
  download_tooltip: String,
  downloads_open: DownloadsPanel,
  status: Status,
  focus: Option<u32>,
  editing: Editing,
  prompt: Option<&'static str>,
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
    let Ok(toolbar) = Self::with_palette(renderers, wearing);

    Ok(toolbar)
  }

  #[must_use]
  pub(crate) fn with_palette(
    renderers: &'static [Renderer],
    wearing: Wearing,
  ) -> core::result::Result<Self, console_core_never::Never> {
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
      download_status: None,
      download_tooltip: "Downloads".to_owned(),
      downloads_open: DownloadsPanel::Closed,
      status: Status::default(),
      focus: None,
      editing: Editing::Inactive,
      prompt: None,
      settings: Settings::default(),
      pointer: None,
    })
  }

  pub(crate) fn tabs(
    &mut self,
    tabs: Vec<TabLabel>,
    active: u32,
  ) -> core::result::Result<ToolbarChange, console_core_never::Never> {
    match (self.tabs == tabs && self.active_tab == active).then_some(()) {
      Some(()) => return Ok(ToolbarChange::Unchanged),
      None => {}
    }

    self.tabs = tabs;
    self.active_tab = active;

    Ok(ToolbarChange::Different)
  }

  #[must_use]
  pub(crate) fn reader_colors(
    &self,
  ) -> core::result::Result<(Oklch, Oklch), console_core_never::Never> {
    Ok((self.palette.ground, self.palette.text))
  }

  pub(crate) fn refresh_palette(&mut self) -> Result<(), Never> {
    match Wearing::worn() {
      Ok(palette) => self.palette = palette,
      Err(fault) => console_core_logging::debug!("Keeping the previous browser palette: {fault}"),
    }

    Ok(())
  }

  pub fn status(&mut self, next: Status) -> Result<ToolbarChange, Never> {
    let status = Status {
      progress: next.progress.min(1000),
      ..next
    };
    let change = ToolbarChange::from(self.status != status);

    match change {
      ToolbarChange::Different => self.status = status,
      ToolbarChange::Unchanged => {}
    }

    Ok(change)
  }

  pub(crate) fn download_notice(
    &mut self,
    notice: Option<(DownloadState, &str)>,
  ) -> core::result::Result<ToolbarChange, console_core_never::Never> {
    let state = notice.map(|(state, _)| state);
    let label = match notice {
      Some((DownloadState::Started, name)) => format!("Downloading {name}"),
      Some((DownloadState::Saved, name)) => format!("Saved {name}"),
      Some((DownloadState::Failed, "")) => "Downloads unavailable".to_owned(),
      Some((DownloadState::Failed, name)) => format!("Couldn’t save {name}"),
      None => "Downloads".to_owned(),
    };
    let change =
      ToolbarChange::from(self.download_status != state || self.download_tooltip != label);
    self.download_status = state;
    self.download_tooltip = label;

    Ok(change)
  }

  pub(crate) fn downloads_open(&mut self, open: DownloadsPanel) -> Result<(), Never> {
    self.downloads_open = open;

    Ok(())
  }

  pub fn address(&mut self, address: &str) -> Result<ToolbarChange, Never> {
    let Ok(current) = self.address.text();
    let change = ToolbarChange::from(self.editing == Editing::Inactive && current != address);

    match change {
      ToolbarChange::Different => {
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
    self.prompt = None;
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

  pub(crate) fn prompt(
    &mut self,
    TextPrompt { placeholder, value }: TextPrompt<'_>,
  ) -> Result<(), Never> {
    let Ok(field) = TextField::holding(value);
    self.address = field;
    self.prompt = Some(placeholder);
    self.edit()
  }

  #[must_use]
  pub(crate) fn entry_mode(&self) -> Result<EntryMode, Never> {
    Ok(match self.prompt {
      Some(_) => EntryMode::Prompt,
      None => EntryMode::Address,
    })
  }

  pub(crate) fn settings_visibility(&self) -> Result<SettingsVisibility, Never> {
    self.settings.visibility()
  }

  pub(crate) fn dismiss_settings(&mut self) -> Result<(), Never> {
    let Ok(_closed) = self.settings.close();

    Ok(())
  }

  #[must_use]
  pub(crate) fn style(
    &self,
  ) -> core::result::Result<(&Wearing, &Font), console_core_never::Never> {
    Ok((&self.palette, &self.font))
  }

  pub fn hover(
    &mut self,
    point: Option<Point<i32>>,
  ) -> core::result::Result<ToolbarChange, console_core_never::Never> {
    let Ok(tooltip_control) = self.tooltip_control(self.pointer);
    let Ok(own_tooltip_control) = self.tooltip_control(point);
    let change = ToolbarChange::from(tooltip_control != own_tooltip_control);
    self.pointer = point;

    Ok(change)
  }

  #[must_use]
  fn tooltip_control(
    &self,
    point: Option<Point<i32>>,
  ) -> core::result::Result<Option<Control>, console_core_never::Never> {
    let Ok(settings) = self.at(Control::Settings);
    let Ok(downloads) = self.at(Control::Downloads);
    let checked_value = match point {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(hit) = under(checked_value, &[settings, downloads]);

    Ok(match hit {
      Some(0) => Some(Control::Settings),
      Some(1) => Some(Control::Downloads),
      Some(_) | None => None,
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

    Ok(match (control, self.tab_controls.last()) {
      (Control::Back, _) => back,
      (Control::Forward, _) => forward,
      (Control::Reload, _) => reload,
      (Control::Address, _) => address,
      (Control::Settings, _) => settings,
      (Control::Close, _) => close,
      (Control::Downloads, Some(Control::Downloads)) => match self.tab_layout.last().copied() {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => {
          let Ok(explicit_value) = tab_button(TabPlacement { x: 0, width: 0 });
          explicit_value
        }
      },
      (
        Control::Tab(_)
        | Control::PreviousTab
        | Control::NextTab
        | Control::NewTab
        | Control::CloseTab
        | Control::Downloads,
        _,
      ) => {
        let Ok(explicit_value) = tab_button(TabPlacement { x: 0, width: 0 });
        explicit_value
      }
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
        PageLoad::Ready => Action::Reload,
      },
      Control::Close => Action::Close,
      Control::Settings => {
        let Ok(()) = self.settings.open();
        Action::SettingsChanged
      }
      Control::Tab(id) => Action::SelectTab(id),
      Control::PreviousTab => {
        let Ok(action) = self.step_tab(-1);
        action
      }
      Control::NextTab => {
        let Ok(action) = self.step_tab(1);
        action
      }
      Control::NewTab => {
        let Ok(count) = self.tab_count();

        match count.cmp(&MAX_TABS) {
          std::cmp::Ordering::Less => Action::NewTab,
          std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => Action::Handled,
        }
      }
      Control::CloseTab => Action::CloseTab,
      Control::Downloads => Action::Downloads,
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
    match matches!(self.settings.visibility(), Ok(SettingsVisibility::Visible)).then_some(()) {
      Some(()) => {
        let Ok(tap) = self.settings.tap(at, &self.status);

        return Ok(tap);
      }
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
    match (mods, key) {
      (CONTROL, Keysym::j) => return Ok(Action::Downloads),
      _ => {}
    }

    match matches!(self.settings.visibility(), Ok(SettingsVisibility::Visible)).then_some(()) {
      Some(()) => {
        let Ok(own_key) = self.settings.key(key, mods, &self.status);

        return Ok(own_key);
      }
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
      (Keysym::Page_Up, _, _) => self.step_tab(-1),
      (Keysym::Page_Down, _, _) => self.step_tab(1),
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
      )
      | (CONTROL | CONTROL_SHIFT, Keysym::BackSpace, _) => {
        match (self.editing, key) {
          (Editing::Selected, Keysym::BackSpace | Keysym::Delete) => {
            self.address = TextField::default()
          }
          (Editing::Selected, Keysym::Left) => {
            let Ok(_) = self.address.moved_to(0);
          }
          (Editing::Selected, Keysym::Right) => {}
          _ => {
            let Ok(press) = edit_press(key, mods);
            let Ok(_) = self.address.typed_press(press);
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
    let Ok(close) = self.at(Control::Close);
    let close_state = match (self.focus == Some(5)).then_some(()) {
      Some(()) => strip::Control::Focused,
      None => strip::Control::Ready,
    };
    let Ok(()) =
      strip::paint_control(&mut shapes, close, marks::SHUT, close_state, &self.palette);
    let Ok(()) = self.paint_button(&mut shapes, Control::Settings, ButtonAvailability::Enabled);
    let Ok(()) = self.paint_tabs(&mut shapes, width);
    let Ok(()) = self.paint_address(&mut shapes);
    let Ok(()) = self.paint_tooltip(&mut shapes, width);

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
      | (Some(SETTINGS), Control::Settings) => FocusMark::Focused,
      (_, Control::Settings) => {
        match matches!(self.settings.visibility(), Ok(SettingsVisibility::Visible)).then_some(())
        {
          Some(()) => FocusMark::Focused,
          None => FocusMark::Passive,
        }
      }
      (_, Control::Downloads) => {
        let Ok(target) = fitted(
          self
            .tab_layout
            .len()
            .saturating_sub(1)
            .saturating_add(CONTROLS.len()),
        );

        match (self.downloads_open == DownloadsPanel::Open || self.focus == Some(target))
          .then_some(())
        {
          Some(()) => FocusMark::Focused,
          None => FocusMark::Passive,
        }
      }
      _ => FocusMark::Passive,
    };

    let Ok(highlight) = self.focus_edge(FocusMark::Focused);

    match ::core::primitive::bool::then_some(matches!(focused, FocusMark::Focused), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(shape) = panel(place, CORNER_RADIUS, self.palette.fill, highlight);
        shapes.push(shape);
      }
      ::core::option::Option::None => {}
    }

    let color = match (control, availability, self.download_status) {
      (Control::Downloads, _, Some(DownloadState::Started | DownloadState::Saved)) => {
        self.palette.pink
      }
      (Control::Downloads, _, Some(DownloadState::Failed)) => self.palette.coral,
      (_, ButtonAvailability::Enabled, _) => self.palette.text,
      (_, ButtonAvailability::Disabled, _) => self.palette.edge,
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
          | Control::PreviousTab
          | Control::NextTab
          | Control::NewTab
          | Control::CloseTab
          | Control::Downloads => 1,
        };
        let tip = (6i32.saturating_mul(direction), 0);
        shapes.extend([
          stroke(((-6i32).saturating_mul(direction), 0), tip),
          stroke((direction, -5), tip),
          stroke((direction, 5), tip),
        ]);
      }
      (Control::Reload, PageLoad::Ready) => {
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
      (Control::Reload, PageLoad::Loading) => {
        shapes.extend([stroke((-5, -5), (5, 5)), stroke((-5, 5), (5, -5))]);
      }
      (Control::Settings, _) => {
        shapes.extend([-5, 0, 5].map(|row| stroke((-7, row), (7, row))));
      }
      (Control::Downloads, _) => {
        let tip = (0, 3);
        shapes.extend([
          stroke((0, -8), tip),
          stroke((-4, -1), tip),
          stroke((4, -1), tip),
          stroke((-7, 4), (-7, 8)),
          stroke((-7, 8), (7, 8)),
          stroke((7, 8), (7, 4)),
        ]);
      }
      (
        Control::Close
        | Control::Address
        | Control::Tab(_)
        | Control::PreviousTab
        | Control::NextTab
        | Control::NewTab
        | Control::CloseTab,
        _,
      ) => {}
    }

    Ok(())
  }

  fn paint_tooltip(
    &self,
    shapes: &mut Vec<Shape>,
    width: i32,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(tooltip_control) = self.tooltip_control(self.pointer);
      let (control, label) = match (tooltip_control, self.settings.visibility()) {
        (Some(Control::Settings), Ok(SettingsVisibility::Hidden)) => {
          (Control::Settings, "Settings")
        }
        (Some(Control::Downloads), Ok(SettingsVisibility::Hidden)) => {
          (Control::Downloads, self.download_tooltip.as_str())
        }
        _ => return Ok(()),
      };

      let Ok(button) = self.at(control);
      let Ok(measured) = measured(label, &self.font);
      let Ok(wide) = positive(measured.width.saturating_add(20));
      let wide_2 = wide.clamp(84, 248).min(width.max(0));
      let center = button
        .origin
        .x
        .saturating_add(button.size.width.wrapping_div(2));
      let x = center
        .saturating_sub(wide_2.wrapping_div(2))
        .clamp(0, width.saturating_sub(wide_2).max(0));
      let y = button.origin.y.saturating_sub(29);
      let frame = Rectangle {
        origin: Point { x, y },
        size: Size {
          width: wide_2,
          height: 25,
        },
      };
      let Ok(background) = panel(
        frame,
        CORNER_RADIUS,
        self.palette.night,
        Edge::Of {
          wide: 1,
          color: self.palette.edge,
        },
      );
      shapes.push(background);
      let Ok(()) = label_at(
        shapes,
        frame,
        label,
        &self.font,
        self.palette.text,
        Alignment::Center,
      );
    };

    Ok(())
  }

  fn current_tab(&self) -> Result<Option<u32>, Never> {
    Ok(
      (0_u32..)
        .zip(&self.tabs)
        .find_map(|(position, tab)| (tab.id == self.active_tab).then_some(position)),
    )
  }

  fn step_tab(&self, step: i32) -> Result<Action, Never> {
    let Ok(active) = self.current_tab();
    let current = match active {
      Some(index) => index,
      None => return Ok(Action::Handled),
    };
    let Ok(many) = fitted::<_, u32>(self.tabs.len());
    let Ok(next) = strip::along(many, strip::Stop::Tab(current), step);
    let position = match next {
      strip::Stop::Tab(index) => match (index != current).then_some(()) {
        Some(()) => index,
        None => return Ok(Action::Handled),
      },
      strip::Stop::Outside => return Ok(Action::Handled),
    };
    let Ok(index) = console_core_number_conversion::index(position);

    Ok(match self.tabs.get(index) {
      Some(tab) => Action::SelectTab(tab.id),
      None => Action::Handled,
    })
  }

  fn paint_tabs(&mut self, shapes: &mut Vec<Shape>, width: i32) -> Result<(), Never> {
    let focused = self.focus.and_then(|target| {
      let Ok(index) = console_core_number_conversion::index(target);
      index
        .checked_sub(CONTROLS.len())
        .and_then(|position| self.tab_controls.get(position))
        .copied()
    });
    self.tab_layout.clear();
    self.tab_controls.clear();
    let Ok(geometry) = tab_bar(width);
    let TabBar {
      right,
      next,
      start,
      size,
      available,
    } = geometry;
    let Ok(many) = fitted::<_, u32>(self.tabs.len());
    let Ok(current) = self.current_tab();
    let here = match current {
      Some(index) => index,
      None => 0,
    };
    let Ok(minimum) = fitted::<_, u32>(available.saturating_div(80).max(1));
    let window = Tabs {
      many,
      here,
      from: 0,
      fits: minimum,
    };
    let Ok(visible) = strip::visible_range(window);
    let Ok(count) = fitted::<_, i32>(visible.end.saturating_sub(visible.start).max(1));
    let column = available.saturating_div(count);
    let Ok(()) = self.tab_arrow(
      shapes,
      focused,
      (Control::PreviousTab, marks::BEFORE, BROWSER_MARGIN),
      size,
    );
    let Ok(()) = self.paint_tab_labels(
      shapes,
      focused,
      visible,
      TabPlacement {
        x: start,
        width: column,
      },
    );
    let Ok(()) = self.paint_tab_commands(
      shapes,
      focused,
      TabPlacement {
        x: next,
        width: size,
      },
      window,
    );

    let Ok(()) = self.place_downloads(right);
    let Ok(()) = self.retain_tab_focus(focused);
    self.paint_button(shapes, Control::Downloads, ButtonAvailability::Enabled)
  }

  fn place_downloads(&mut self, right: i32) -> Result<(), Never> {
    let downloads = Rectangle {
      origin: Point {
        x: right.saturating_sub(32),
        y: ADDRESS_TOP,
      },
      size: Size {
        width: 32,
        height: ADDRESS_HEIGHT,
      },
    };
    self.tab_controls.push(Control::Downloads);
    self.tab_layout.push(downloads);
    Ok(())
  }

  fn paint_tab_commands(
    &mut self,
    shapes: &mut Vec<Shape>,
    focused: Option<Control>,
    place: TabPlacement,
    tabs: Tabs,
  ) -> Result<(), Never> {
    let TabPlacement {
      x: next,
      width: size,
    } = place;
    let Ok(tab_count) = self.tab_count();

    for (control, label, x, allowed) in [
      (
        Control::NextTab,
        marks::AFTER,
        next,
        tabs.here.saturating_add(1) < tabs.many,
      ),
      (
        Control::NewTab,
        marks::MORE,
        next.saturating_add(size),
        tab_count < MAX_TABS,
      ),
      (
        Control::CloseTab,
        marks::LESS,
        next.saturating_add(size.saturating_mul(2)),
        tabs.many > 1,
      ),
    ] {
      let state = match (allowed, focused == Some(control)) {
        (true, true) => strip::Control::Focused,
        (true, false) => strip::Control::Ready,
        (false, _) => strip::Control::Unavailable,
      };
      let Ok(()) = self.tab_control(
        shapes,
        control,
        label,
        TabPlacement { x, width: size },
        state,
      );
    }

    Ok(())
  }

  fn tab_arrow(
    &mut self,
    shapes: &mut Vec<Shape>,
    focused: Option<Control>,
    (control, label, x): (Control, &str, i32),
    width: i32,
  ) -> Result<(), Never> {
    let Ok(current) = self.current_tab();
    let state = match (current, focused == Some(control)) {
      (Some(1..), true) => strip::Control::Focused,
      (Some(1..), false) => strip::Control::Ready,
      (Some(0) | None, _) => strip::Control::Unavailable,
    };
    self.tab_control(shapes, control, label, TabPlacement { x, width }, state)
  }

  fn tab_control(
    &mut self,
    shapes: &mut Vec<Shape>,
    control: Control,
    label: &str,
    place: TabPlacement,
    state: strip::Control,
  ) -> Result<(), Never> {
    let Ok(frame) = tab_button(place);
    let Ok(()) = strip::paint_control(shapes, frame, label, state, &self.palette);

    match state {
      strip::Control::Ready | strip::Control::Focused => {
        self.tab_controls.push(control);
        self.tab_layout.push(frame);
      }
      strip::Control::Unavailable => {}
    }

    Ok(())
  }

  fn paint_tab_labels(
    &mut self,
    shapes: &mut Vec<Shape>,
    focused: Option<Control>,
    visible: std::ops::Range<u32>,
    placement: TabPlacement,
  ) -> Result<(), Never> {
    for (column, position) in (0_i32..).zip(visible) {
      let Ok(index) = console_core_number_conversion::index(position);
      let tab = match self.tabs.get(index) {
        Some(tab) => tab,
        None => break,
      };
      let x = placement
        .x
        .saturating_add(column.saturating_mul(placement.width));
      let Ok(frame) = tab_button(TabPlacement {
        x,
        width: placement.width,
      });
      let control = Control::Tab(tab.id);
      let state = match (self.active_tab == tab.id, focused == Some(control)) {
        (true, _) => TabFace::Selected,
        (false, true) => TabFace::Focused,
        (false, false) => TabFace::Other,
      };
      let Ok(()) = strip::paint_tab(shapes, frame, &tab.title, state, &self.palette);
      self.tab_controls.push(control);
      self.tab_layout.push(frame);
    }

    Ok(())
  }

  fn retain_tab_focus(&mut self, focused: Option<Control>) -> Result<(), Never> {
    match focused {
      Some(control) => {
        let Ok(first) = fitted::<_, u32>(CONTROLS.len());
        let tabs = (first..).zip(&self.tab_controls);
        self.focus = tabs
          .clone()
          .find_map(|(id, target)| (*target == control).then_some(id))
          .or_else(|| {
            tabs
              .filter_map(|(id, target)| (*target == Control::Tab(self.active_tab)).then_some(id))
              .next()
          });
      }
      None => {}
    }

    Ok(())
  }

  fn tab_count(&self) -> Result<u64, Never> {
    console_core_number_conversion::fixed_index(self.tabs.len())
  }

  fn paint_address(&self, shapes: &mut Vec<Shape>) -> Result<(), Never> {
    let Ok(field) = self.at(Control::Address);
    let focused = match self.focus {
      Some(ADDRESS) => FocusMark::Focused,
      Some(_) | None => FocusMark::Passive,
    };
    let Ok(edge) = self.focus_edge(focused);

    {
      let Ok(shape) = panel(field, CORNER_RADIUS, self.palette.night, edge);
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
    let said = match (text, self.editing, self.prompt) {
      ("", _, Some(prompt)) => prompt,
      ("", Editing::Inactive, None) => "Address or search",
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

struct TabBar {
  right: i32,
  next: i32,
  start: i32,
  size: i32,
  available: i32,
}

fn tab_bar(width: i32) -> Result<TabBar, Never> {
  let right = width.saturating_sub(BROWSER_MARGIN);
  let close = right.saturating_sub(strip::HEIGHT);
  let size = 34.min(
    width
      .saturating_sub(BROWSER_MARGIN.saturating_mul(2))
      .max(0)
      .saturating_div(7),
  );
  let next = close
    .saturating_sub(size.saturating_mul(3))
    .saturating_sub(strip::GAP);
  let start = BROWSER_MARGIN
    .saturating_add(size)
    .saturating_add(strip::GAP);
  let available = next.saturating_sub(start).saturating_sub(strip::GAP).max(0);
  Ok(TabBar {
    right,
    next,
    start,
    size,
    available,
  })
}

struct TabPlacement {
  x: i32,
  width: i32,
}

fn tab_button(TabPlacement { x, width }: TabPlacement) -> Result<Rectangle<i32>, Never> {
  Ok(Rectangle {
    origin: Point { x, y: TAB_TOP },
    size: Size {
      width: width.max(0),
      height: strip::HEIGHT,
    },
  })
}

fn layout(size: Size<u32>) -> Result<[Rectangle<i32>; CONTROLS.len()], Never> {
  let Ok(width) = positive(size.width);

  let spacing: i32 = match width {
    ..490 => 32,
    _ => 40,
  };
  let button_width = spacing.saturating_sub(4);
  let right = width.saturating_sub(BROWSER_MARGIN);
  let address_left = 8i32.saturating_add(3i32.saturating_mul(spacing));
  let settings_left = right
    .saturating_sub(32)
    .saturating_sub(button_width)
    .saturating_sub(8);
  let placed = |x, width_2| Rectangle {
    origin: Point { x, y: ADDRESS_TOP },
    size: Size {
      width: width_2,
      height: ADDRESS_HEIGHT,
    },
  };
  let address_width = settings_left
    .saturating_sub(address_left)
    .saturating_sub(8)
    .max(0);

  let Ok(close) = tab_button(TabPlacement {
    x: right.saturating_sub(strip::HEIGHT),
    width: strip::HEIGHT,
  });

  Ok([
    placed(8, button_width),
    placed(8i32.saturating_add(spacing), button_width),
    placed(
      8i32.saturating_add(2i32.saturating_mul(spacing)),
      button_width,
    ),
    placed(address_left, address_width),
    placed(settings_left, button_width),
    close,
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
        PageLoad::Ready,
        ButtonAvailability::Enabled,
        Action::Back,
      ),
      (
        Control::Forward,
        2,
        PageLoad::Ready,
        ButtonAvailability::Enabled,
        Action::Forward,
      ),
      (
        Control::Reload,
        1,
        PageLoad::Ready,
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
        PageLoad::Ready,
        ButtonAvailability::Disabled,
        Action::Handled,
      ),
      (
        Control::Forward,
        2,
        PageLoad::Ready,
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
    assert_eq!(
      toolbar.settings_visibility(),
      Ok(SettingsVisibility::Visible)
    );
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
    assert_eq!(
      toolbar.settings_visibility(),
      Ok(SettingsVisibility::Visible)
    );
    Ok(())
  }

  #[test]
  fn tab_titles_center_inside_their_tabs() -> Result<(), Box<dyn std::error::Error>> {
    let mut toolbar = Toolbar::with_renderers(Renderer::ALL.as_slice())?;
    let short = "Home";
    let long = "A page title too long to fit inside a narrow browser tab";
    let Ok(_changed) = toolbar.tabs(
      vec![
        TabLabel {
          id: 1,
          title: short.to_owned(),
        },
        TabLabel {
          id: 2,
          title: long.to_owned(),
        },
      ],
      1,
    );
    let mut shapes = Vec::new();
    let Ok(()) = toolbar.paint_tabs(&mut shapes, 440);
    check_tab_labels(&toolbar, &shapes, [(1, short), (2, long)])?;

    let Ok(close) = toolbar.at(Control::Close);
    let Ok(address) = toolbar.at(Control::Address);
    assert!(close.origin.y.saturating_add(close.size.height) <= address.origin.y);

    for mark in [marks::BEFORE, marks::AFTER] {
      assert!(
        shapes
          .iter()
          .any(|shape| matches!(shape, Shape::Text(text) if text.said == mark))
      );
    }

    let next = toolbar
      .tab_controls
      .iter()
      .position(|control| *control == Control::NextTab)
      .and_then(|index| toolbar.tab_layout.get(index))
      .ok_or("missing next-tab arrow")?;
    assert_eq!(toolbar.tap(next.origin, ""), Ok(Action::SelectTab(2)));
    assert_eq!(
      toolbar.key(Keysym::Page_Down, 0, ""),
      Ok(Action::SelectTab(2))
    );

    Ok(())
  }

  fn check_tab_labels(
    toolbar: &Toolbar,
    shapes: &[Shape],
    titles: [(u32, &str); 2],
  ) -> Result<(), Box<dyn std::error::Error>> {
    for (id, original) in titles {
      let place = toolbar
        .tab_controls
        .iter()
        .position(|control| *control == Control::Tab(id))
        .and_then(|index| toolbar.tab_layout.get(index))
        .ok_or("missing tab hitbox")?;
      let right = place.origin.x.saturating_add(place.size.width);
      let label = shapes
        .iter()
        .find_map(|shape| match shape {
          Shape::Text(text) => (place.origin.x..right).contains(&text.at.x).then_some(text),
          Shape::Panel(_)
          | Shape::Picture(_)
          | Shape::Cropped(_)
          | Shape::Line(_)
          | Shape::Clip(_) => None,
        })
        .ok_or("missing tab label")?;
      assert!(!label.said.is_empty());
      assert_eq!(label.said == original, id == 1);
      let Ok(size) = drawing::measure_text(
        Run {
          said: &label.said,
          weight: label.weight,
          width: label.width,
        },
        &label.font,
      );
      let Ok(horizontal) = positive(size.width);
      let Ok(vertical) = positive(size.height);
      assert_eq!(
        label.at.x,
        place
          .origin
          .x
          .saturating_add(place.size.width.saturating_sub(horizontal).wrapping_div(2))
      );
      assert_eq!(
        label.at.y,
        place
          .origin
          .y
          .saturating_add(place.size.height.saturating_sub(vertical).wrapping_div(2))
      );
    }

    Ok(())
  }

  #[test]
  fn downloads_are_reachable_by_click_and_keyboard() -> Result<(), palette::PaletteError> {
    let mut toolbar = Toolbar::with_renderers(Renderer::ALL.as_slice())?;
    let address = "https://example.org/";
    let mut shapes = Vec::new();
    let Ok(()) = toolbar.paint_tabs(&mut shapes, 1100);
    let Ok(button) = toolbar.at(Control::Downloads);
    assert_eq!(button.size.width, 32);
    assert_eq!(
      shapes
        .iter()
        .filter(|shape| matches!(shape, Shape::Line(_)))
        .count(),
      6
    );
    assert!(
      !shapes
        .iter()
        .any(|shape| matches!(shape, Shape::Text(label) if label.said == "Downloads"))
    );

    assert_eq!(
      toolbar.key(Keysym::j, CONTROL, address),
      Ok(Action::Downloads)
    );
    assert_eq!(toolbar.tap(button.origin, address), Ok(Action::Downloads));
    let Ok(download_notice) = toolbar.download_notice(Some((DownloadState::Saved, "report.txt")));
    assert_eq!(download_notice, ToolbarChange::Different,);
    let Ok(hover) = toolbar.hover(Some(button.origin));
    assert_eq!(hover, ToolbarChange::Different);
    shapes.clear();
    let Ok(()) = toolbar.paint_tooltip(&mut shapes, 1100);
    assert!(
      shapes
        .iter()
        .any(|shape| matches!(shape, Shape::Text(label) if label.said == "Saved report.txt"))
    );
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
    let Ok(()) = toolbar.paint_tooltip(&mut shapes, 1100);
    assert!(shapes.is_empty());
    let Ok(button) = toolbar.at(Control::Settings);
    let Ok(hover) = toolbar.hover(Some(button.origin));
    assert_eq!(hover, ToolbarChange::Different);
    let Ok(()) = toolbar.paint_tooltip(&mut shapes, 1100);
    let label = shapes.iter().find_map(|shape| match shape {
      Shape::Text(label) => Some(label.said.as_str()),
      Shape::Panel(_)
      | Shape::Picture(_)
      | Shape::Cropped(_)
      | Shape::Line(_)
      | Shape::Clip(_) => None,
    });
    let tooltip = shapes.iter().find_map(|shape| match shape {
      Shape::Panel(panel) => Some(panel),
      Shape::Text(_)
      | Shape::Picture(_)
      | Shape::Cropped(_)
      | Shape::Line(_)
      | Shape::Clip(_) => None,
    });
    assert!(tooltip.is_some_and(|panel| {
      let Ok(room) = positive(panel.size.height);
      panel.at.y >= 0 && panel.at.y.saturating_add(room) <= button.origin.y
    }));
    assert_eq!(label, Some("Settings"));
    let Ok(toolbar_hover) = toolbar.hover(None);
    assert_eq!(toolbar_hover, ToolbarChange::Different);
    shapes.clear();
    let Ok(()) = toolbar.paint_tooltip(&mut shapes, 1100);
    assert!(shapes.is_empty());
    Ok(())
  }

  #[test]
  fn the_close_button_uses_the_same_action_for_navigation_and_a_tap()
  -> Result<(), palette::PaletteError> {
    let mut toolbar = Toolbar::with_renderers(&[Renderer::WebKit, Renderer::Chromium])?;
    assert_eq!(toolbar.key(Keysym::F6, 0, ""), Ok(Action::Handled));
    assert_eq!(toolbar.key(Keysym::Right, 0, ""), Ok(Action::Handled));
    assert_eq!(toolbar.key(Keysym::Up, 0, ""), Ok(Action::Handled));
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
