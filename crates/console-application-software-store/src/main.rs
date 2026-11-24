//! A native Console window for finding and installing Nix packages.

#![forbid(unsafe_code)]

mod catalogue;
mod packages;
mod view;

use std::cmp::Ordering;
use std::convert::Infallible;
use std::io;
use std::process::ExitCode;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};

use catalogue::{Category, Package};
use console_core_external_programs::Installed;
use console_core_geometry::{Containment, Point, Size};
use console_core_iteration::{Step, iterate};
use console_core_number_conversion::toward_zero_i32;
use console_draw_surface::{
  Closed, InputContent, KeyPress, Keysym, PointerEvent, Scale, Surface, Surrounding,
  TextInputChange, TextInputState, Window,
};
use console_input_text::{InputDeletion, TextField, Typed};
use console_package_manager::user::{self, Profile};
use console_panel::canvas::{AreaLayout, FocusDirection};
use console_program_lifetime::threads::let_go;
use packages::{Local, Matches};
use view::{Frame, Theme};

const APP_ID: &str = "console-application-software-store";
const WINDOW: Size<u32> = Size {
  width: 1080,
  height: 740,
};
const DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
  Browse,
  Installed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
  Page(Page),
  Category(Category),
  Search,
  ClearSearch,
  Details(u32),
  Featured,
  Back,
  Install,
  Open,
  Remove,
  ConfirmRemove,
  ErrorDetails,
  SearchDetails,
  Dismiss,
  RetrySearch,
  Refresh,
  Help,
  Close,
}

#[derive(Clone, Debug)]
enum Overlay {
  Package(Package),
  Remove(Local),
  Error { summary: String, text: String },
  Help,
}

struct Notice {
  title: String,
  detail: Option<String>,
  error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SearchId(u64);

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProfileAvailability {
  Ready,
  Unavailable,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProfileStatus {
  Unavailable,
  Ready,
  Reading(ProfileAvailability),
  Changing,
}

impl ProfileStatus {
  fn availability(self) -> Result<ProfileAvailability, Infallible> {
    Ok(match self {
      Self::Unavailable => ProfileAvailability::Unavailable,
      Self::Ready | Self::Changing => ProfileAvailability::Ready,
      Self::Reading(previous) => previous,
    })
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LaunchStatus {
  Idle,
  Opening,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SearchStatus {
  Searching,
  Settled,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Editing {
  Updated,
  Unchanged,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WindowFlow {
  Stay,
  Close,
}

enum PackageChange {
  Install(Package),
  Remove(Local),
}

#[derive(Clone, Copy)]
enum Operation {
  Install,
  Remove,
}

struct ChangedPackage {
  name: String,
  operation: Operation,
  outcome: io::Result<()>,
  profile: io::Result<Vec<Local>>,
}

enum Reply {
  Search(SearchId, io::Result<Matches>),
  Profile(io::Result<Vec<Local>>),
  Change(ChangedPackage),
  Open(io::Result<()>),
}

struct Shop {
  page: Page,
  category: Category,
  query: TextField,
  preedit: String,
  deadline: Option<Instant>,
  request: SearchId,
  search_in_flight: Option<SearchId>,
  search_problem: Option<Notice>,
  matches: Vec<Package>,
  match_count: u32,
  installed: Vec<Local>,
  profile: Profile,
  profile_status: ProfileStatus,
  launching: LaunchStatus,
  nix_available: Installed,
  scroll: i32,
  focus: Option<Action>,
  hover: Option<Action>,
  overlay: Option<Overlay>,
  notice: Option<Notice>,
  send: SyncSender<Reply>,
  replies: Receiver<Reply>,
}

impl Shop {
  fn new() -> io::Result<Self> {
    let (send, replies) = mpsc::sync_channel(4);
    let profile = packages::profile()?;
    let Ok(nix_available) = user::availability();
    let mut shop = Self {
      page: Page::Browse,
      category: Category::All,
      query: TextField::default(),
      preedit: String::new(),
      deadline: None,
      request: SearchId(0),
      search_in_flight: None,
      search_problem: None,
      matches: Vec::new(),
      match_count: 0,
      installed: Vec::new(),
      profile,
      profile_status: ProfileStatus::Unavailable,
      launching: LaunchStatus::Idle,
      nix_available,
      scroll: 0,
      focus: None,
      hover: None,
      overlay: None,
      notice: None,
      send,
      replies,
    };
    let Ok(()) = shop.refresh();
    Ok(shop)
  }

  #[must_use]
  fn query(&self) -> core::result::Result<&str, core::convert::Infallible> {
    let Ok(text) = self.query.text();

    Ok(text)
  }

  #[must_use]
  fn searching(&self) -> Result<SearchStatus, Infallible> {
    let Ok(query) = self.query();
    let pending = !query.trim().is_empty()
      && (self.deadline.is_some() || self.search_in_flight.is_some())
      && self.search_problem.is_none();

    Ok(match pending.then_some(()) {
      Some(()) => SearchStatus::Searching,
      None => SearchStatus::Settled,
    })
  }

  #[must_use]
  fn rows(&self) -> core::result::Result<Vec<Package>, core::convert::Infallible> {
    let Ok(query) = self.query();

    match bool::then_some(!query.trim().is_empty(), ()) {
      Some(()) => {
        return Ok(self.matches.clone());
      }
      None => {}
    }

    Ok(match self.page {
      Page::Browse => {
        let Ok(explicit_value) = catalogue::featured(self.category);
        explicit_value
      }
      Page::Installed => self
        .installed
        .iter()
        .map(|entry| entry.package.clone())
        .collect(),
    })
  }

  #[must_use]
  fn local(
    &self,
    package: &Package,
  ) -> core::result::Result<Option<&Local>, core::convert::Infallible> {
    Ok(
      self
        .installed
        .iter()
        .find(|row| row.entry.attribute == package.installable.attribute),
    )
  }

  #[must_use]
  fn availability(&self) -> Result<ProfileAvailability, Infallible> {
    self.profile_status.availability()
  }

  fn can_change(&self) -> Result<ProfileAvailability, Infallible> {
    Ok(match (self.nix_available, self.profile_status) {
      (Installed::Yes, ProfileStatus::Ready) => ProfileAvailability::Ready,
      _ => ProfileAvailability::Unavailable,
    })
  }

  fn edit_query(&mut self) -> core::result::Result<(), core::convert::Infallible> {
    {
      self.scroll = 0;
      self.request = SearchId(self.request.0.wrapping_add(1));
      self.search_problem = None;
      self.match_count = 0;
      self.deadline = None;
      let Ok(own_query) = self.query();
      let query = own_query.trim();

      match query {
        "" => self.matches.clear(),
        pattern => {
          self.matches = {
            let Ok(explicit_value) = catalogue::suggested(pattern);
            explicit_value
          };

          let now = Instant::now();
          let next_search = match now.checked_add(DEBOUNCE) {
            Some(deadline) => deadline,
            None => now,
          };
          self.deadline = match self.nix_available {
            Installed::Yes => Some(next_search),
            Installed::No => None,
          };
        }
      }
    };

    Ok(())
  }

  fn clear_query(&mut self) -> core::result::Result<(), core::convert::Infallible> {
    {
      self.query = TextField::default();
      self.preedit.clear();
      let Ok(()) = self.edit_query();
    };

    Ok(())
  }

  #[must_use]
  fn failure(
    title: String,
    fault: &io::Error,
  ) -> core::result::Result<Notice, core::convert::Infallible> {
    let message = fault.to_string();
    console_core_logging::warn!("console-application-software-store: {title} {message}");
    Ok(Notice {
      title,
      detail: Some(
        ({
          let Ok(explicit_value) = packages::problem(fault);
          explicit_value
        })
        .to_owned(),
      ),
      error: Some(message),
    })
  }

  fn refresh(&mut self) -> Result<(), Infallible> {
    match (self.nix_available, self.profile_status) {
      (Installed::Yes, ProfileStatus::Ready | ProfileStatus::Unavailable) => {}
      _ => return Ok(()),
    }

    let Ok(previous) = self.availability();
    let profile = self.profile.clone();

    match spawn_reply(&self.send, "store-profile", move || {
      Reply::Profile(packages::list(&profile))
    }) {
      Ok(()) => self.profile_status = ProfileStatus::Reading(previous),
      Err(fault) => {
        let Ok(notice) = Self::failure(
          String::from("Installed packages could not be read."),
          &fault,
        );
        self.notice = Some(notice);
      }
    }

    Ok(())
  }

  fn update_profile(&mut self, result: io::Result<Vec<Local>>) -> Result<(), Infallible> {
    match result {
      Ok(rows) => {
        self.profile_status = ProfileStatus::Ready;
        self.installed = rows;
      }
      Err(fault) => {
        self.profile_status = ProfileStatus::Unavailable;
        let Ok(notice) = Self::failure(
          String::from("Installed packages could not be read."),
          &fault,
        );
        self.notice = Some(notice);
      }
    }

    Ok(())
  }

  fn advance(&mut self) -> Result<Editing, Infallible> {
    let Ok(()) = self.start_search(Instant::now());
    let mut changed = Editing::Unchanged;

    for _ in 0..4 {
      let reply = match self.replies.try_recv() {
        Ok(reply) => reply,
        Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => break,
      };
      let Ok(()) = self.receive(reply);
      changed = Editing::Updated;
    }

    Ok(changed)
  }

  fn start_search(&mut self, now: Instant) -> Result<(), Infallible> {
    let pending =
      self.search_in_flight.is_none() && self.deadline.is_some_and(|deadline| now >= deadline);

    match pending.then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    let Ok(written) = self.query();
    let query = written.split_whitespace().collect::<Vec<_>>().join(" ");
    let request = self.request;
    self.deadline = None;

    match spawn_reply(&self.send, "store-search", move || {
      Reply::Search(request, packages::search(&query))
    }) {
      Ok(()) => self.search_in_flight = Some(request),
      Err(fault) => {
        let Ok(notice) = Self::failure(String::from("Nixpkgs search failed."), &fault);
        self.search_problem = Some(notice);
      }
    }

    Ok(())
  }

  fn receive(&mut self, reply: Reply) -> Result<(), Infallible> {
    match reply {
      Reply::Search(request, result) => self.found(request, result),
      Reply::Profile(result) => self.update_profile(result),
      Reply::Change(changed) => self.changed_package(changed),
      Reply::Open(result) => {
        self.launching = LaunchStatus::Idle;

        match result {
          Ok(()) => {}
          Err(fault) => {
            console_core_logging::warn!(
              "console-application-software-store could not open the app: {fault}"
            );
            self.notice = Some(Notice {
              title: String::from("The application could not be opened."),
              detail: Some(String::from("Open Details to see the error.")),
              error: Some(fault.to_string()),
            });
          }
        }

        Ok(())
      }
    }
  }

  fn changed_package(&mut self, changed: ChangedPackage) -> Result<(), Infallible> {
    let ChangedPackage {
      name,
      operation,
      outcome,
      profile,
    } = changed;
    let Ok(()) = self.update_profile(profile);
    let Ok(availability) = self.availability();

    match (outcome, availability) {
      (Ok(()), ProfileAvailability::Ready) => {
        let title = match operation {
          Operation::Install => format!("{name} is installed. Find it under Installed."),
          Operation::Remove => format!("{name} was removed."),
        };
        self.notice = Some(Notice {
          title,
          detail: None,
          error: None,
        });
      }
      (Ok(()), ProfileAvailability::Unavailable) => {}
      (Err(fault), _) => {
        let title = match operation {
          Operation::Install => format!("{name} could not be installed."),
          Operation::Remove => format!("{name} could not be removed."),
        };
        let Ok(notice) = Self::failure(title, &fault);
        self.notice = Some(notice);
      }
    }

    Ok(())
  }

  fn found(&mut self, request: SearchId, result: io::Result<Matches>) -> Result<(), Infallible> {
    match (self.search_in_flight == Some(request)).then_some(()) {
      Some(()) => self.search_in_flight = None,
      None => return Ok(()),
    }

    match (request == self.request).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    match self.focus {
      Some(Action::Details(_)) => self.focus = None,
      Some(_) | None => {}
    }

    match result {
      Ok(found) => {
        self.matches = found.packages;
        self.match_count = found.total;
      }
      Err(fault) => {
        let Ok(notice) = Self::failure(String::from("Nixpkgs search failed."), &fault);
        self.search_problem = Some(notice);
      }
    }

    Ok(())
  }

  fn change(&mut self, change: PackageChange) -> Result<(), Infallible> {
    match self.can_change() {
      Ok(ProfileAvailability::Ready) => {}
      Ok(ProfileAvailability::Unavailable) => return Ok(()),
    }

    let (name, operation, progress) = match &change {
      PackageChange::Install(package) => (
        package.title.clone(),
        Operation::Install,
        Notice {
          title: format!("Installing {}…", package.title),
          detail: Some(String::from(
            "Nix may need to download or build some files.",
          )),
          error: None,
        },
      ),
      PackageChange::Remove(package) => (
        package.package.title.clone(),
        Operation::Remove,
        Notice {
          title: format!("Removing {}…", package.package.title),
          detail: None,
          error: None,
        },
      ),
    };
    let profile = self.profile.clone();
    let started = spawn_reply(&self.send, "store-change", move || {
      let outcome = match change {
        PackageChange::Install(package) => profile.install(&package.installable),
        PackageChange::Remove(package) => profile.remove(&package.entry),
      };
      Reply::Change(ChangedPackage {
        name,
        operation,
        outcome,
        profile: packages::list(&profile),
      })
    });

    match started {
      Ok(()) => {
        self.profile_status = ProfileStatus::Changing;
        self.overlay = None;
        self.focus = None;
        self.notice = Some(progress);
      }
      Err(fault) => {
        let Ok(notice) =
          Self::failure(String::from("The package change could not start."), &fault);
        self.notice = Some(notice);
      }
    }

    Ok(())
  }

  fn selected_local(&self) -> Result<Option<Local>, Infallible> {
    Ok(match self.overlay.as_ref() {
      Some(Overlay::Package(package)) => {
        let Ok(local) = self.local(package);
        local.cloned()
      }
      Some(Overlay::Remove(_) | Overlay::Error { .. } | Overlay::Help) | None => None,
    })
  }

  fn launch(&mut self) -> Result<(), Infallible> {
    let Ok(selected) = self.selected_local();
    let local = match (self.launching, selected) {
      (LaunchStatus::Idle, Some(local)) => local,
      (LaunchStatus::Idle, None) | (LaunchStatus::Opening, _) => return Ok(()),
    };

    match spawn_reply(&self.send, "store-open", move || {
      Reply::Open(local.launch())
    }) {
      Ok(()) => {
        self.launching = LaunchStatus::Opening;
        self.overlay = None;
        self.focus = None;
      }
      Err(fault) => {
        let Ok(()) = self.receive(Reply::Open(Err(fault)));
      }
    }

    Ok(())
  }

  fn show(&mut self, overlay: Overlay) -> core::result::Result<(), core::convert::Infallible> {
    {
      self.overlay = Some(overlay);
      self.focus = None;
      self.hover = None;
      self.preedit.clear();
    };

    Ok(())
  }

  fn action(&mut self, action: Action) -> Result<WindowFlow, Infallible> {
    match action {
      Action::Page(page) => {
        self.page = page;
        let Ok(()) = self.clear_query();

        match bool::then_some(page == Page::Installed, ()) {
          Some(()) => {
            let Ok(()) = self.refresh();
          }
          None => {}
        }
      }
      Action::Category(category) => {
        self.category = category;
        self.scroll = 0;
      }
      Action::Search => self.focus = Some(Action::Search),
      Action::ClearSearch => {
        let Ok(()) = self.clear_query();
        self.focus = Some(Action::Search);
      }
      Action::Details(index) => {
        let Ok(rows) = self.rows();

        let Ok(position) = console_core_number_conversion::index(index);

        match rows.into_iter().nth(position) {
          Some(package) => {
            let Ok(()) = self.show(Overlay::Package(package));
          }
          None => {}
        };
      }
      Action::Featured => {
        let Ok(show) = self.show(Overlay::Package({
          let Ok(explicit_value) = catalogue::package("obs-studio");
          explicit_value
        }));

        show
      }
      Action::Back => {
        self.overlay = match self.overlay.take() {
          Some(Overlay::Remove(local)) => Some(Overlay::Package(local.package)),
          None | Some(Overlay::Package(_) | Overlay::Error { .. } | Overlay::Help) => None,
        };
        self.focus = None;
      }
      Action::Install => {
        match self.overlay.clone() {
          Some(Overlay::Package(package)) => {
            let Ok(()) = self.change(PackageChange::Install(package));
          }
          None | Some(Overlay::Remove(_) | Overlay::Error { .. } | Overlay::Help) => {}
        };
      }
      Action::Open => {
        let Ok(()) = self.launch();
      }
      Action::Remove => {
        let Ok(selected) = self.selected_local();

        match selected {
          Some(local) => {
            let Ok(()) = self.show(Overlay::Remove(local));
          }
          None => {}
        }
      }
      Action::ConfirmRemove => {
        match self.overlay.clone() {
          Some(Overlay::Remove(local)) => {
            let Ok(()) = self.change(PackageChange::Remove(local));
          }
          None | Some(Overlay::Package(_) | Overlay::Error { .. } | Overlay::Help) => {}
        };
      }
      Action::ErrorDetails | Action::SearchDetails => {
        let notice = match bool::then_some(action == Action::SearchDetails, ()) {
          Some(()) => self.search_problem.as_ref(),
          None => self.notice.as_ref(),
        };

        let details = notice.and_then(|entry| {
          let text = entry.error.as_ref()?;
          Some(Overlay::Error {
            summary: entry.title.clone(),
            text: text.clone(),
          })
        });

        match details {
          Some(overlay) => {
            let Ok(()) = self.show(overlay);
          }
          None => {}
        }
      }
      Action::Dismiss => self.notice = None,
      Action::RetrySearch => {
        let Ok(edit_query) = self.edit_query();

        edit_query
      }
      Action::Refresh => {
        let Ok(refresh) = self.refresh();

        refresh
      }
      Action::Help => {
        let Ok(show) = self.show(Overlay::Help);

        show
      }
      Action::Close => {
        return Ok(match self.profile_status {
          ProfileStatus::Changing => WindowFlow::Stay,
          ProfileStatus::Unavailable | ProfileStatus::Ready | ProfileStatus::Reading(_) => {
            WindowFlow::Close
          }
        });
      }
    }

    Ok(WindowFlow::Stay)
  }

  fn keyboard(&mut self, press: KeyPress, frame: &Frame) -> Result<WindowFlow, Infallible> {
    let key = press.key;
    const CONTROL: u32 = 4;
    let control = press.modifiers & CONTROL != 0;
    let unobstructed = self.overlay.is_none();

    match key {
      Keysym::k | Keysym::f => match (control && unobstructed).then_some(()) {
        Some(()) => return self.action(Action::Search),
        None => {}
      },
      Keysym::u => match (control && self.focus == Some(Action::Search)).then_some(()) {
        Some(()) => return self.action(Action::ClearSearch),
        None => {}
      },
      Keysym::Escape => return self.escape(),
      Keysym::Tab | Keysym::ISO_Left_Tab => {
        let backwards = key == Keysym::ISO_Left_Tab || press.modifiers & 1 != 0;
        let direction = match backwards.then_some(()) {
          Some(()) => FocusDirection::Previous,
          None => FocusDirection::Next,
        };
        let Ok(tab) = frame.canvas.targets.tab(self.focus, direction);
        self.focus = tab;
        return Ok(WindowFlow::Stay);
      }
      Keysym::Return | Keysym::KP_Enter | Keysym::space => match self.focus {
        Some(Action::Search) => match key {
          Keysym::Return | Keysym::KP_Enter => return self.submit_search(),
          _ => {}
        },
        Some(action) => return self.action(action),
        None => {}
      },
      Keysym::Up | Keysym::Down | Keysym::Page_Up | Keysym::Page_Down => {
        match unobstructed.then_some(()) {
          Some(()) => {
            let by = match key {
              Keysym::Up => -75,
              Keysym::Down => 75,
              Keysym::Page_Up => -340,
              _ => 340,
            };
            let Ok(()) = self.scroll_by(by, frame);
            return Ok(WindowFlow::Stay);
          }
          None => {}
        }
      }
      Keysym::question => {
        match (unobstructed && self.focus != Some(Action::Search)).then_some(()) {
          Some(()) => return self.action(Action::Help),
          None => {}
        }
      }
      _ => {}
    }

    let Ok(()) = self.type_search(press);
    Ok(WindowFlow::Stay)
  }

  fn escape(&mut self) -> Result<WindowFlow, Infallible> {
    let Ok(query) = self.query();

    match (self.overlay.is_some(), query.is_empty()) {
      (true, _) => self.action(Action::Back),
      (false, false) => self.action(Action::ClearSearch),
      (false, true) => {
        self.focus = None;
        Ok(WindowFlow::Stay)
      }
    }
  }

  fn submit_search(&mut self) -> Result<WindowFlow, Infallible> {
    match (self.searching(), self.matches.as_slice()) {
      (Ok(SearchStatus::Searching), []) => {
        self.deadline = Some(Instant::now());
        Ok(WindowFlow::Stay)
      }
      (Ok(SearchStatus::Searching), [_, ..]) | (Ok(SearchStatus::Settled), _) => {
        self.action(Action::Details(0))
      }
    }
  }

  fn type_search(&mut self, press: KeyPress) -> Result<(), Infallible> {
    const COMMAND_MODIFIERS: u32 = (1 << 2) | (1 << 3) | (1 << 6);

    match (self.overlay.is_none() && press.modifiers & COMMAND_MODIFIERS == 0).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    let Ok(letter) = press.key.char();
    let editable =
      self.focus == Some(Action::Search) || letter.is_some_and(|value| !value.is_control());

    match editable.then_some(()) {
      Some(()) => {
        self.focus = Some(Action::Search);
        let Ok(typed) = self.query.typed(press.key);

        match (typed == Typed::Edited).then_some(()) {
          Some(()) => self.edit_query(),
          None => Ok(()),
        }
      }
      None => Ok(()),
    }
  }

  fn input(
    &mut self,
    change: TextInputChange,
  ) -> core::result::Result<(), core::convert::Infallible> {
    {
      match bool::then_some(self.focus != Some(Action::Search), ()) {
        Some(()) => {
          return Ok(());
        }
        None => {}
      }

      self.preedit = change
        .preedit
        .map_or_else(String::new, |preedit| preedit.text);
      let Ok(typed) = self.query.apply_input(
        InputDeletion {
          before: change.delete_before,
          after: change.delete_after,
        },
        match change.commit.as_deref() {
          Some(present) => present,
          None => "",
        },
      );

      match bool::then_some(typed == Typed::Edited, ()) {
        Some(()) => {
          let Ok(()) = self.edit_query();
        }
        None => {}
      }
    };

    Ok(())
  }

  fn scroll_by(
    &mut self,
    by: i32,
    frame: &Frame,
  ) -> core::result::Result<(), core::convert::Infallible> {
    {
      self.scroll = self
        .scroll
        .saturating_add(by)
        .clamp(0, frame.canvas.max_scroll);
    };

    Ok(())
  }

  #[must_use]
  fn input_state(
    &self,
    frame: &Frame,
  ) -> core::result::Result<Option<TextInputState>, core::convert::Infallible> {
    match bool::then_some(
      self.focus != Some(Action::Search) || self.overlay.is_some(),
      (),
    ) {
      Some(()) => {
        return Ok(None);
      }
      None => {}
    }

    let Ok(before) = self.query.before_caret();
    let cursor = match u32::try_from(before.len()) {
      Ok(offset) => offset,
      Err(_offset_out_of_range) => u32::MAX,
    };
    let Ok(query) = self.query();

    Ok(Some(TextInputState {
      cursor: frame.caret,
      identity: 1,
      content: InputContent::Plain(Some(Surrounding {
        text: query.to_owned(),
        cursor,
      })),
    }))
  }
}

fn spawn_reply(
  sender: &SyncSender<Reply>,
  job: &str,
  work: impl FnOnce() -> Reply + Send + 'static,
) -> io::Result<()> {
  let send = sender.clone();
  let worker = std::thread::Builder::new()
    .name(job.to_owned())
    .spawn(move || match send.send(work()) {
      Ok(()) => {}
      Err(_window_closed) => {
        console_core_logging::debug!("software store worker finished after the window closed")
      }
    })?;
  let Ok(()) = let_go(worker);
  Ok(())
}

fn open() -> Result<(), Box<dyn std::error::Error>> {
  let shop = Shop::new()?;
  let theme = Theme::new()?;
  let mut surface = Surface::connect()?;
  surface.show_window_sized(
    &Window {
      app_id: APP_ID.to_owned(),
      title: "Software Store".to_owned(),
    },
    WINDOW,
  )?;
  let initial = StoreWindow {
    shop,
    theme,
    surface,
    frame: Frame::default(),
    drawing: Drawing::Pending,
    gesture: None,
    closed: Closed::No,
  };
  let outcome = iterate(initial, |mut window| {
    Ok(match window.turn() {
      Ok(WindowFlow::Stay) => Step::Again(window),
      Ok(WindowFlow::Close) => Step::Halt(Ok(())),
      Err(fault) => Step::Halt(Err(fault)),
    })
  })?;
  outcome
}

#[derive(Clone, Copy, PartialEq)]
enum Drawing {
  Pending,
  Shown { room: Size<u32>, scale: Scale },
}

#[derive(Clone, Copy)]
enum Drag {
  Click,
  Scroll,
}

struct Gesture {
  start: Point<f64>,
  last: Point<f64>,
  target: Option<Action>,
  drag: Drag,
}

struct StoreWindow {
  shop: Shop,
  theme: Theme,
  surface: Surface,
  frame: Frame,
  drawing: Drawing,
  gesture: Option<Gesture>,
  closed: Closed,
}

impl StoreWindow {
  fn turn(&mut self) -> Result<WindowFlow, Box<dyn std::error::Error>> {
    let Ok(advance) = self.shop.advance();

    match advance {
      Editing::Updated => {
        self.gesture = None;
        self.drawing = Drawing::Pending;
      }
      Editing::Unchanged => {}
    }

    match (self.closed, self.shop.profile_status) {
      (Closed::Yes, ProfileStatus::Changing) => {}
      (
        Closed::Yes,
        ProfileStatus::Unavailable | ProfileStatus::Ready | ProfileStatus::Reading(_),
      ) => {
        return Ok(WindowFlow::Close);
      }
      (Closed::No, _) => self.paint()?,
    }

    self.surface.wait(&[], Some(Duration::from_millis(80)))?;
    let closed = self.surface.closed()?;

    match closed {
      Closed::Yes => self.closed = Closed::Yes,
      Closed::No => {}
    }

    let keys = self.surface.key_presses()?;

    for key in keys {
      let Ok(result) = self.shop.keyboard(key, &self.frame);

      match result {
        WindowFlow::Close => return Ok(WindowFlow::Close),
        WindowFlow::Stay => self.drawing = Drawing::Pending,
      }
    }

    let committed = self.surface.text_input_changes()?;

    for change in committed {
      let Ok(()) = self.shop.input(change);
      self.drawing = Drawing::Pending;
    }

    let pointers = self.surface.pointer_events()?;

    for event in pointers {
      let result = self.pointer(event)?;

      match result {
        WindowFlow::Close => return Ok(WindowFlow::Close),
        WindowFlow::Stay => {}
      }
    }

    Ok(WindowFlow::Stay)
  }

  fn paint(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    let Ok(logical) = self.surface.logical();
    let room = match logical {
      Some(room) => room,
      None => return Ok(()),
    };
    let Ok(scale) = self.surface.scale();
    let drawing = Drawing::Shown { room, scale };

    match (self.drawing == drawing).then_some(()) {
      Some(()) => return Ok(()),
      None => {}
    }

    let Ok(frame) = view::draw(&self.shop, room, &self.theme);
    self.frame = frame;

    match self.shop.scroll.cmp(&self.frame.canvas.max_scroll) {
      Ordering::Greater => {
        self.shop.scroll = self.frame.canvas.max_scroll;
        let Ok(scrolled) = view::draw(&self.shop, room, &self.theme);
        self.frame = scrolled;
      }
      Ordering::Less | Ordering::Equal => {}
    }

    let painted = self.frame.canvas.paint(&mut self.surface, room);
    painted?;
    let Ok(state) = self.shop.input_state(&self.frame);
    self.surface.set_text_input(state)?;
    self.drawing = drawing;
    Ok(())
  }

  fn pointer(&mut self, event: PointerEvent) -> Result<WindowFlow, Box<dyn std::error::Error>> {
    match event {
      PointerEvent::Down { at: (x, y) } => self.pointer_down(Point { x, y })?,
      PointerEvent::Moved { at: (x, y) } => {
        let Ok(()) = self.pointer_moved(Point { x, y });
      }
      PointerEvent::Up => {
        let Ok(next) = self.pointer_up();
        return Ok(next);
      }
      PointerEvent::Scrolled { by } => match self.shop.overlay.as_ref() {
        Some(_) => {}
        None => {
          let Ok(pixels) = toward_zero_i32(by);
          let Ok(()) = self.shop.scroll_by(pixels, &self.frame);
          self.drawing = Drawing::Pending;
        }
      },
      PointerEvent::Left => {
        self.gesture = None;

        match self.shop.hover.take() {
          Some(_) => self.drawing = Drawing::Pending,
          None => {}
        }
      }
      PointerEvent::Pinched { .. } => self.gesture = None,
    }

    Ok(WindowFlow::Stay)
  }

  fn pointer_down(&mut self, point: Point<f64>) -> Result<(), Box<dyn std::error::Error>> {
    let Ok(target) = self.frame.canvas.targets.action(point);
    self.gesture = Some(Gesture {
      start: point,
      last: point,
      target,
      drag: Drag::Click,
    });
    let move_window = target.is_none() && self.shop.overlay.is_none() && point.y < 53.0;

    match move_window.then_some(()) {
      Some(()) => self.surface.move_window()?,
      None => {}
    }

    Ok(())
  }

  fn pointer_moved(&mut self, point: Point<f64>) -> Result<(), Infallible> {
    let Ok(target) = self.frame.canvas.targets.action(point);

    match (target != self.shop.hover).then_some(()) {
      Some(()) => self.drawing = Drawing::Pending,
      None => {}
    }

    self.shop.hover = target;
    let gesture = match self.gesture.as_mut() {
      Some(gesture) => gesture,
      None => return Ok(()),
    };
    let scrollable = self.shop.overlay.is_none()
      && self.frame.scroll_area.contains_point(gesture.start) == Ok(Containment::Inside);
    let distant = (gesture.start.y - point.y).abs() > 10.0;

    match (scrollable, gesture.drag, distant) {
      (true, Drag::Scroll, _) | (true, Drag::Click, true) => {
        let Ok(pixels) = toward_zero_i32(gesture.last.y - point.y);
        let Ok(()) = self.shop.scroll_by(pixels, &self.frame);
        gesture.target = None;
        gesture.drag = Drag::Scroll;
        self.drawing = Drawing::Pending;
      }
      (false, _, _) | (true, Drag::Click, false) => {}
    }

    gesture.last = point;
    Ok(())
  }

  fn pointer_up(&mut self) -> Result<WindowFlow, Infallible> {
    let gesture = match self.gesture.take() {
      Some(Gesture {
        drag: Drag::Click,
        target: Some(target),
        last,
        ..
      }) => (target, last),
      Some(
        Gesture {
          drag: Drag::Scroll, ..
        }
        | Gesture { target: None, .. },
      )
      | None => {
        return Ok(WindowFlow::Stay);
      }
    };
    let (target, last) = gesture;
    let Ok(hit) = self.frame.canvas.targets.action(last);

    match (hit == Some(target)).then_some(()) {
      Some(()) => {
        self.shop.focus = Some(target);
        let Ok(flow) = self.shop.action(target);
        self.drawing = Drawing::Pending;
        Ok(flow)
      }
      None => Ok(WindowFlow::Stay),
    }
  }
}

fn main() -> ExitCode {
  match bool::then_some(
    std::env::args().any(|arg| arg == "--help" || arg == "-h"),
    (),
  ) {
    Some(()) => {
      println!(
        "console-application-software-store\n\nOpen the Console package store with console-application-software-store."
      );
      return ExitCode::SUCCESS;
    }
    None => {}
  }

  match open() {
    Ok(()) => ExitCode::SUCCESS,
    Err(fault) => {
      console_core_logging::warn!("console-application-software-store: {fault}");
      ExitCode::FAILURE
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::{Component, Path, PathBuf};

  use console_core_places::Base;
  use console_waiting::{Outcome, Ready, Schedule, until_handed};

  #[test]
  fn browse_view_draws_and_opens_the_obs_feature() -> Result<(), Box<dyn std::error::Error>> {
    let mut shop = Shop::new()?;
    shop.nix_available = Installed::Yes;
    let theme = Theme::new()?;
    let Ok(frame) = view::draw(&shop, WINDOW, &theme);
    let Ok(hit) = frame.canvas.targets.action(Point { x: 264.0, y: 376.0 });
    let featured = hit.ok_or("Missing OBS feature")?;
    assert_eq!(featured, Action::Featured);
    let Ok(tab) = Frame::default()
      .canvas
      .targets
      .tab(None, FocusDirection::Next);
    assert_eq!(tab, None);
    let Ok(frame_tab) = frame.canvas.targets.tab(None, FocusDirection::Next);
    let first = frame_tab.ok_or("Missing first focusable control")?;
    let Ok(frame_end) = frame.canvas.targets.tab(None, FocusDirection::Previous);
    let last = frame_end.ok_or("Missing last focusable control")?;
    let Ok(wrapped_next) = frame.canvas.targets.tab(Some(last), FocusDirection::Next);
    let Ok(wrapped_previous) = frame
      .canvas
      .targets
      .tab(Some(first), FocusDirection::Previous);
    assert_eq!(wrapped_next, Some(first));
    assert_eq!(wrapped_previous, Some(last));
    check_painting(&frame)?;

    let Ok(_flow) = shop.action(featured);
    assert!(
      matches!(&shop.overlay, Some(Overlay::Package(row)) if row.installable.attribute == "obs-studio")
    );
    let Ok(dialog) = view::draw(&shop, WINDOW, &theme);
    let Ok(back) = dialog.canvas.targets.action(Point { x: 780.0, y: 280.0 });
    assert_eq!(back, Some(Action::Back));
    Ok(())
  }

  #[test]
  fn focused_search_keeps_its_caret_and_clear_target() -> Result<(), Box<dyn std::error::Error>> {
    let mut shop = Shop::new()?;
    let Ok(query) = TextField::holding("obs");
    shop.query = query;
    shop.focus = Some(Action::Search);
    shop.preedit = " studio".to_owned();
    let theme = Theme::new()?;
    let Ok(frame) = view::draw(&shop, WINDOW, &theme);
    let Ok(input) = shop.input_state(&frame);
    let state = input.ok_or("Search text input is inactive")?;
    assert!(state.cursor.is_some());

    let Ok(search) = frame.canvas.targets.action(Point { x: 265.0, y: 40.0 });
    let Ok(clear) = frame.canvas.targets.action(Point { x: 840.0, y: 40.0 });
    assert_eq!(search, Some(Action::Search));
    assert_eq!(clear, Some(Action::ClearSearch));
    Ok(())
  }

  #[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    expect(
      explicit026_env_read_once,
      reason = "this fixture owns the optional CONSOLE_STORE_TEST_SCREENSHOT destination"
    )
  )]
  fn screenshot_destination() -> Result<Option<PathBuf>, Infallible> {
    Ok(std::env::var_os("CONSOLE_STORE_TEST_SCREENSHOT").map(PathBuf::from))
  }

  fn check_painting(frame: &Frame) -> Result<(), Box<dyn std::error::Error>> {
    const BYTES: u32 = WINDOW.width.saturating_mul(WINDOW.height).saturating_mul(4);
    let Ok(length) = console_core_number_conversion::index(BYTES);
    let mut pixels = vec![0; length.min(const { 4 * 1024 * 1024 })];
    console_draw_painting::onto(
      &mut pixels,
      console_draw_painting::Frame {
        device: WINDOW,
        points: WINDOW,
      },
      &frame.canvas.shapes,
    )?;
    let (rgba, _) = pixels.as_chunks::<4>();
    assert!(rgba.iter().all(|[_, _, _, alpha]| *alpha == 255));
    let Ok(destination) = screenshot_destination();

    match destination {
      Some(path) => {
        let (bgra, _) = pixels.as_chunks_mut::<4>();

        for [blue, _, red, _alpha] in bgra {
          std::mem::swap(blue, red);
        }

        let encoded = console_core_png_files::encoded_rgba(WINDOW, &pixels)?;
        console_core_atomic_writes::whole(&path, &encoded)?;
      }
      None => {}
    }

    Ok(())
  }

  struct NixFixture {
    root: PathBuf,
    store: PathBuf,
    nix_state: PathBuf,
    xdg_state: PathBuf,
  }

  impl NixFixture {
    #[cfg_attr(
      dylint_lib = "explicit026_env_read_once",
      expect(
        explicit026_env_read_once,
        reason = "the Nix integration fixture owns the isolation checks for CONSOLE_STORE_TEST_ROOT, NIX_STORE_DIR, and NIX_STATE_DIR"
      )
    )]
    fn configured() -> Result<Self, Box<dyn std::error::Error>> {
      let root =
        std::env::var_os("CONSOLE_STORE_TEST_ROOT").ok_or("Missing CONSOLE_STORE_TEST_ROOT")?;
      let store = std::env::var_os("NIX_STORE_DIR").ok_or("Missing NIX_STORE_DIR")?;
      let nix_state = std::env::var_os("NIX_STATE_DIR").ok_or("Missing NIX_STATE_DIR")?;
      let Ok(state) = Base::configured(Base::State);
      let xdg_state = state.ok_or("Missing XDG_STATE_HOME")?;
      Ok(Self {
        root: PathBuf::from(root),
        store: PathBuf::from(store),
        nix_state: PathBuf::from(nix_state),
        xdg_state,
      })
    }

    fn ensure_isolation(&self) -> Result<(), Infallible> {
      assert!(self.root.is_absolute());
      assert!(
        self
          .root
          .components()
          .all(|part| part != Component::ParentDir)
      );

      for path in [&self.store, &self.nix_state, &self.xdg_state] {
        let remainder = path.strip_prefix(&self.root);
        assert!(matches!(remainder, Ok(relative) if relative != Path::new("")));
        assert!(path.components().all(|part| part != Component::ParentDir));
      }

      Ok(())
    }
  }

  #[test]
  #[ignore = "requires an isolated local Nix store and test catalogue"]
  fn searches_installs_opens_and_removes_using_nix() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = NixFixture::configured()?;
    let Ok(()) = fixture.ensure_isolation();
    let mut shop = Shop::new()?;
    assert_eq!(shop.nix_available, Installed::Yes);
    wait_for(&mut shop, FixtureWait::Profile)?;
    assert!(shop.availability() == Ok(ProfileAvailability::Ready));
    assert!(shop.installed.is_empty());

    let Ok(query) = TextField::holding("obs");
    shop.query = query;
    let Ok(()) = shop.edit_query();
    wait_for(&mut shop, FixtureWait::Search)?;
    assert!(shop.search_problem.is_none());
    assert_eq!(shop.match_count, 1);

    let Ok(_details) = shop.action(Action::Details(0));
    let Ok(_install) = shop.action(Action::Install);
    wait_for(&mut shop, FixtureWait::Change)?;
    assert!(
      shop
        .notice
        .as_ref()
        .is_some_and(|notice| notice.error.is_none())
    );
    let installed = shop
      .installed
      .first()
      .ok_or("OBS Studio was not installed")?;
    assert_eq!(installed.package.title, "OBS Studio");
    let Ok(launch_label) = installed.launch_label();
    assert_eq!(launch_label, Some("Open"));

    let Ok(_installed_page) = shop.action(Action::Page(Page::Installed));
    wait_for(&mut shop, FixtureWait::Profile)?;
    let Ok(_installed_details) = shop.action(Action::Details(0));
    let Ok(_open) = shop.action(Action::Open);
    wait_for(&mut shop, FixtureWait::Launch)?;
    assert!(
      shop
        .notice
        .as_ref()
        .is_some_and(|notice| notice.error.is_none())
    );

    let Ok(_removal_details) = shop.action(Action::Details(0));
    let Ok(_remove) = shop.action(Action::Remove);
    let Ok(_confirm) = shop.action(Action::ConfirmRemove);
    wait_for(&mut shop, FixtureWait::Change)?;
    assert!(shop.availability() == Ok(ProfileAvailability::Ready) && shop.installed.is_empty());
    Ok(())
  }

  #[derive(Debug)]
  enum FixtureWait {
    Profile,
    Search,
    Change,
    Launch,
  }

  fn wait_for(shop: &mut Shop, event: FixtureWait) -> io::Result<()> {
    let Ok(patience) = Schedule::asking_every(Duration::from_secs(20), Duration::from_millis(20));
    let Ok(outcome) = until_handed(patience, shop, |fixture| {
      let Ok(_advance) = fixture.advance();
      let settled = match event {
        FixtureWait::Profile => !matches!(fixture.profile_status, ProfileStatus::Reading(_)),
        FixtureWait::Search => fixture.searching() == Ok(SearchStatus::Settled),
        FixtureWait::Change => fixture.profile_status != ProfileStatus::Changing,
        FixtureWait::Launch => fixture.launching == LaunchStatus::Idle,
      };
      Ok(match settled.then_some(()) {
        Some(()) => Ready::Yes,
        None => Ready::NotYet,
      })
    });

    match outcome {
      Outcome::Happened => Ok(()),
      Outcome::RanOut => Err(io::Error::new(
        io::ErrorKind::TimedOut,
        format!("Waiting for Nix {event:?} timed out"),
      )),
    }
  }
}
