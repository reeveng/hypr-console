//! A native Console window for finding and managing packages.

#![forbid(unsafe_code)]

mod catalogue;
mod packages;
mod view;

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::io;
use std::process::ExitCode;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};

use catalogue::{Category, Package, Source};
use console_core_external_programs::Installed;
use console_core_geometry::{Containment, Point, Size};
use console_core_iteration::{Step, iterate};
use console_core_number_conversion::toward_zero_i32;
use console_core_state_machine::SingleEffectMachine;
use console_draw_surface::{
  Closed, InputContent, KeyPress, KeyboardFocus, Keysym, PhysicalKeyPress, PointerEvent, Scale,
  Surface, Surrounding, TextInputChange, TextInputState, Window,
};
use console_input_text::{InputDeletion, TextField, Typed};
use console_package_manager::catalogue::{Catalogue, Manager, Matches};
use console_package_manager::user::Profile;
use console_panel::canvas::{AreaLayout, FocusDirection};
use console_panel::keys::{
  Driving, Meaning, Navigation, NavigationEffect, NavigationInput, NavigationKeys, TabMotion,
};
use console_panel::scrolling::{self, LogicalPixels};
use console_panel::strip::{self, Stop};
use console_program_lifetime::threads::let_go;
use packages::{Local, sources};
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

impl Page {
  fn index(self) -> Result<u32, Infallible> {
    Ok(match self {
      Self::Browse => 0,
      Self::Installed => 1,
    })
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
  Page(Page),
  SelectTab(u32),
  PreviousTab,
  NextTab,
  Category(Category),
  Search,
  ClearSearch,
  Details(u32),
  Back,
  Install,
  Open,
  Remove,
  ConfirmRemove,
  SearchDetails,
  RetrySearch,
  Refresh,
  Close,
}

#[derive(Clone, Debug)]
enum Overlay {
  Package(Package),
  Remove(Local),
  Error { summary: String, text: String },
}

struct Notice {
  title: String,
  detail: Option<String>,
  error: Option<String>,
}

enum FailureSource {
  Application,
  PackageManager,
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
  inventory: sources::Inventory,
}

enum Reply {
  Search(Manager, SearchId, io::Result<Matches>),
  Profile(sources::Inventory),
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
  queued_searches: BTreeSet<Manager>,
  search_in_flight: BTreeMap<Manager, SearchId>,
  search_problem: Option<Notice>,
  suggestions: Vec<Package>,
  results: BTreeMap<Manager, Matches>,
  catalogue: Catalogue,
  cached_results: Matches,
  pacman_changes: Installed,
  terminal: Installed,
  installed: Vec<Local>,
  profile: Profile,
  profile_status: ProfileStatus,
  launching: LaunchStatus,
  nix_available: Installed,
  scroll: ScrollPositions,
  wheel: scrolling::Position,
  focus: Option<Action>,
  navigation: Navigation,
  hover: Option<Action>,
  overlay: Option<Overlay>,
  send: SyncSender<Reply>,
  replies: Receiver<Reply>,
}

#[derive(Default)]
struct ScrollPositions {
  browse: i32,
  installed: i32,
  search: i32,
}

impl ScrollPositions {
  fn current(&mut self, page: view::PageView) -> Result<&mut i32, Infallible> {
    Ok(match page {
      view::PageView::Browse => &mut self.browse,
      view::PageView::Installed => &mut self.installed,
      view::PageView::Search => &mut self.search,
    })
  }
}

impl Shop {
  fn new() -> io::Result<Self> {
    let (send, replies) = mpsc::sync_channel(4);
    let profile = packages::profile()?;
    let Ok(nix_available) = Manager::Nix.availability();
    let Ok(catalogue) = Catalogue::detecting();
    let mut shop = Self {
      page: Page::Browse,
      category: Category::All,
      query: TextField::default(),
      preedit: String::new(),
      deadline: None,
      request: SearchId(0),
      queued_searches: BTreeSet::new(),
      search_in_flight: BTreeMap::new(),
      search_problem: None,
      suggestions: Vec::new(),
      results: BTreeMap::new(),
      catalogue,
      cached_results: Matches::default(),
      pacman_changes: Installed::No,
      terminal: Installed::No,
      installed: Vec::new(),
      profile,
      profile_status: ProfileStatus::Unavailable,
      launching: LaunchStatus::Idle,
      nix_available,
      scroll: ScrollPositions::default(),
      wheel: scrolling::Position::default(),
      focus: None,
      navigation: Navigation::Ready,
      hover: None,
      overlay: None,
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
      && (!self.queued_searches.is_empty()
        || self
          .search_in_flight
          .values()
          .any(|request| *request == self.request));

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
        return self.results_for_view();
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

  fn results_for_view(&self) -> Result<Vec<Package>, Infallible> {
    let remote = self
      .results
      .values()
      .flat_map(|result| &result.packages)
      .filter(|package| {
        let Ok(included) = self.catalogue.includes(package);

        included.is_some()
      });
    let mut packages: Vec<_> = self
      .cached_results
      .packages
      .iter()
      .chain(remote)
      .cloned()
      .map(|raw| {
        let Ok(package) = catalogue::present(raw);

        package
      })
      .collect();
    let present: BTreeSet<_> = packages.iter().map(|package| &package.source).collect();
    let suggestions: Vec<_> = self
      .suggestions
      .iter()
      .filter(|suggestion| !present.contains(&suggestion.source))
      .cloned()
      .collect();
    packages.extend(suggestions);

    Ok(packages)
  }

  fn match_count(&self) -> Result<u32, Infallible> {
    Ok(
      self
        .results
        .values()
        .fold(self.cached_results.total, |total, result| {
          total.saturating_add(result.total)
        }),
    )
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
        .find(|row| row.package.source == package.source),
    )
  }

  #[must_use]
  fn availability(&self) -> Result<ProfileAvailability, Infallible> {
    self.profile_status.availability()
  }

  fn can_change(&self, source: &Source) -> Result<ProfileAvailability, Infallible> {
    match self.profile_status {
      ProfileStatus::Reading(_) | ProfileStatus::Changing => {
        return Ok(ProfileAvailability::Unavailable);
      }
      ProfileStatus::Unavailable | ProfileStatus::Ready => {}
    }

    let supported = match source {
      Source::Nix(_) => {
        self.nix_available == Installed::Yes && self.profile_status == ProfileStatus::Ready
      }
      Source::Pacman(_) => self.pacman_changes == Installed::Yes,
      Source::Aur(_) => self.terminal == Installed::Yes,
      Source::Flatpak(_) | Source::Mise { .. } => true,
    };

    Ok(match supported.then_some(()) {
      Some(()) => ProfileAvailability::Ready,
      None => ProfileAvailability::Unavailable,
    })
  }

  fn edit_query(&mut self) -> core::result::Result<(), core::convert::Infallible> {
    {
      self.scroll.search = 0;
      self.wheel = scrolling::Position::default();
      self.request = SearchId(self.request.0.wrapping_add(1));
      self.search_problem = None;
      self.results.clear();
      self.queued_searches.clear();
      self.deadline = None;
      let Ok(()) = self.match_catalogue();
      let Ok(own_query) = self.query();
      let query = own_query.trim();

      match query {
        "" => self.suggestions.clear(),
        pattern => {
          let Ok(features) = catalogue::suggested(pattern);
          self.suggestions = features;
          let Ok(managers) = self.catalogue.remote_managers();
          self.queued_searches.extend(managers);
          let now = Instant::now();
          self.deadline = match self.queued_searches.is_empty().then_some(()) {
            Some(()) => None,
            None => match now.checked_add(DEBOUNCE) {
              Some(deadline) => Some(deadline),
              None => Some(now),
            },
          };
        }
      }
    };

    Ok(())
  }

  fn match_catalogue(&mut self) -> Result<(), Infallible> {
    let Ok(query) = self.query();
    let Ok(matches) = self.catalogue.cached_matches(query.trim(), None);
    self.cached_results = matches;

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
    source: FailureSource,
  ) -> core::result::Result<Notice, core::convert::Infallible> {
    let message = fault.to_string();
    console_core_logging::warn!("console-application-software-store: {title} {message}");
    let detail = match source {
      FailureSource::Application => None,
      FailureSource::PackageManager => {
        let Ok(detail) = packages::problem(fault);
        detail.map(str::to_owned)
      }
    };
    let body = match detail.as_deref() {
      Some(explanation) => format!("{explanation}\n{message}"),
      None => message.clone(),
    };
    let Ok(()) = console_notifications::notify(
      console_notifications::Application("Console Store"),
      &title,
      Some(&body),
    );

    Ok(Notice {
      title,
      detail,
      error: Some(message),
    })
  }

  fn refresh(&mut self) -> Result<(), Infallible> {
    match self.profile_status {
      ProfileStatus::Ready | ProfileStatus::Unavailable => {}
      ProfileStatus::Reading(_) | ProfileStatus::Changing => return Ok(()),
    }

    let Ok(previous) = self.availability();
    let profile = self.profile.clone();

    match spawn_reply(&self.send, "store-profile", move || {
      let Ok(inventory) = sources::inventory(&profile);

      Reply::Profile(inventory)
    }) {
      Ok(()) => self.profile_status = ProfileStatus::Reading(previous),
      Err(fault) => {
        let Ok(_notice) = Self::failure(
          String::from("Reading installed packages could not start."),
          &fault,
          FailureSource::Application,
        );
      }
    }

    Ok(())
  }

  fn update_profile(&mut self, inventory: sources::Inventory) -> Result<(), Infallible> {
    let Ok(previous) = self.catalogue.remote_managers();
    let known: BTreeSet<_> = previous.collect();
    self.nix_available = inventory.nix;
    self.pacman_changes = inventory.pacman_changes;
    self.terminal = inventory.terminal;
    self.catalogue = inventory.catalogue;

    match inventory.profile {
      Ok(rows) => {
        self.profile_status = ProfileStatus::Ready;
        self.installed = rows;
      }
      Err(fault) => {
        self.profile_status = ProfileStatus::Unavailable;
        self
          .installed
          .retain(|local| matches!(&local.package.source, Source::Nix(_)));
        let Ok(_notice) = Self::failure(
          String::from("Installed packages could not be read."),
          &fault,
          FailureSource::PackageManager,
        );
      }
    }

    self
      .installed
      .extend(self.catalogue.installed.iter().map(|registered| {
        let Ok(local) = Local::packaged(registered.package.clone(), registered.launcher.clone());

        local
      }));
    self
      .installed
      .sort_by_cached_key(|local| local.package.title.to_lowercase());
    let Ok(()) = self.match_catalogue();
    let Ok(current) = self.catalogue.remote_managers();
    let active: BTreeSet<_> = current.collect();
    self.results.retain(|manager, _| active.contains(manager));
    self
      .queued_searches
      .retain(|manager| active.contains(manager));
    let Ok(query) = self.query();

    match (!query.trim().is_empty()).then_some(()) {
      Some(()) => self
        .queued_searches
        .extend(active.difference(&known).copied()),
      None => {}
    }

    match (self.deadline.is_none() && !self.queued_searches.is_empty()).then_some(()) {
      Some(()) => self.deadline = Some(Instant::now()),
      None => {}
    }

    Ok(())
  }

  fn advance(&mut self) -> Result<Editing, Infallible> {
    let Ok(mut changed) = self.start_search(Instant::now());

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

  fn start_search(&mut self, now: Instant) -> Result<Editing, Infallible> {
    let due = self.deadline.is_some_and(|deadline| now >= deadline);

    match due.then_some(()) {
      Some(()) => self.deadline = None,
      None => return Ok(Editing::Unchanged),
    }

    let Ok(written) = self.query();
    let query = written.split_whitespace().collect::<Vec<_>>().join(" ");
    let available: Vec<_> = self
      .queued_searches
      .iter()
      .copied()
      .filter(|manager| !self.search_in_flight.contains_key(manager))
      .collect();

    for manager in available {
      let Ok(()) = self.request_search(manager, &query);
    }

    Ok(Editing::Updated)
  }

  fn request_search(&mut self, manager: Manager, query: &str) -> Result<(), Infallible> {
    let _queued = self.queued_searches.remove(&manager);
    let request = self.request;
    let search = query.to_owned();

    match spawn_reply(&self.send, "store-search", move || {
      Reply::Search(manager, request, manager.search_remote(&search))
    }) {
      Ok(()) => {
        let _previous = self.search_in_flight.insert(manager, request);
      }
      Err(fault) => {
        let Ok(source) = manager.label();
        let Ok(notice) = Self::failure(
          format!("{source} search could not start."),
          &fault,
          FailureSource::Application,
        );
        self.search_problem = Some(notice);
      }
    }

    Ok(())
  }

  fn receive(&mut self, reply: Reply) -> Result<(), Infallible> {
    match reply {
      Reply::Search(manager, request, result) => self.found(manager, request, result),
      Reply::Profile(result) => self.update_profile(result),
      Reply::Change(changed) => self.changed_package(changed),
      Reply::Open(result) => {
        self.launching = LaunchStatus::Idle;

        match result {
          Ok(()) => {}
          Err(fault) => {
            let Ok(_notice) = Self::failure(
              String::from("The application could not be opened."),
              &fault,
              FailureSource::Application,
            );
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
      inventory,
    } = changed;
    let Ok(()) = self.update_profile(inventory);

    match outcome {
      Ok(()) => {
        let title = match operation {
          Operation::Install => format!("{name} is installed. Find it under Installed."),
          Operation::Remove => format!("{name} was removed."),
        };
        let Ok(()) = console_notifications::notify(
          console_notifications::Application("Console Store"),
          &title,
          None,
        );
      }
      Err(fault) => {
        match (fault.kind() == io::ErrorKind::Interrupted).then_some(()) {
          Some(()) => {
            console_core_logging::debug!("Store transaction canceled: {fault}");
            return Ok(());
          }
          None => {}
        }

        let title = match operation {
          Operation::Install => format!("{name} could not be installed."),
          Operation::Remove => format!("{name} could not be removed."),
        };
        let Ok(_notice) = Self::failure(title, &fault, FailureSource::PackageManager);
      }
    }

    Ok(())
  }

  fn found(
    &mut self,
    manager: Manager,
    request: SearchId,
    result: io::Result<Matches>,
  ) -> Result<(), Infallible> {
    match (self.search_in_flight.get(&manager) == Some(&request)).then_some(()) {
      Some(()) => {
        let _finished = self.search_in_flight.remove(&manager);
      }
      None => return Ok(()),
    }

    match (self.deadline.is_none() && self.queued_searches.contains(&manager)).then_some(()) {
      Some(()) => self.deadline = Some(Instant::now()),
      None => {}
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
        let _previous = self.results.insert(manager, found);
      }
      Err(fault) => {
        let Ok(source) = manager.label();
        let Ok(notice) = Self::failure(
          format!("{source} search failed."),
          &fault,
          FailureSource::PackageManager,
        );
        self.search_problem = Some(notice);
      }
    }

    Ok(())
  }

  fn change(&mut self, change: PackageChange) -> Result<(), Infallible> {
    let source = match &change {
      PackageChange::Install(package) => &package.source,
      PackageChange::Remove(local) => &local.package.source,
    };

    match self.can_change(source) {
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
            "The package manager may need to download or build some files.",
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
        PackageChange::Install(package) => sources::install(&package, &profile),
        PackageChange::Remove(package) => package.remove(&profile),
      };
      let Ok(inventory) = sources::inventory(&profile);

      Reply::Change(ChangedPackage {
        name,
        operation,
        outcome,
        inventory,
      })
    });

    match started {
      Ok(()) => {
        self.profile_status = ProfileStatus::Changing;
        self.overlay = None;
        self.focus = None;
        let Ok(()) = console_notifications::notify(
          console_notifications::Application("Console Store"),
          &progress.title,
          progress.detail.as_deref(),
        );
      }
      Err(fault) => {
        let Ok(_notice) = Self::failure(
          String::from("The package change could not start."),
          &fault,
          FailureSource::Application,
        );
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
      Some(Overlay::Remove(_) | Overlay::Error { .. }) | None => None,
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

  fn install_selected(&mut self) -> Result<(), Infallible> {
    let package = match self.overlay.clone() {
      Some(Overlay::Package(package)) => package,
      None | Some(Overlay::Remove(_) | Overlay::Error { .. }) => return Ok(()),
    };
    let Ok(local) = self.local(&package);

    match local {
      Some(_) => Ok(()),
      None => self.change(PackageChange::Install(package)),
    }
  }

  fn confirm_removal(&mut self) -> Result<(), Infallible> {
    let Ok(selected) = self.selected_local();
    let local = match selected {
      Some(local) => local,
      None => return Ok(()),
    };
    let Ok(removable) = local.removable();

    match removable {
      packages::Removal::Optional => self.show(Overlay::Remove(local)),
      packages::Removal::Required => Ok(()),
    }
  }

  fn action(&mut self, action: Action) -> Result<WindowFlow, Infallible> {
    match action {
      Action::SelectTab(position) => {
        let Ok(()) = self.select_tab(position);
      }
      Action::PreviousTab => return self.step_tab(-1),
      Action::NextTab => return self.step_tab(1),
      Action::Page(page) => {
        let Ok(()) = self.show_page(page);
      }
      Action::Category(category) => {
        self.category = category;
        self.focus = Some(Action::Category(category));
        self.scroll.browse = 0;
        self.wheel = scrolling::Position::default();
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
      Action::Back => {
        self.overlay = match self.overlay.take() {
          Some(Overlay::Remove(local)) => Some(Overlay::Package(local.package)),
          None | Some(Overlay::Package(_) | Overlay::Error { .. }) => None,
        };
        self.focus = None;
      }
      Action::Install => {
        let Ok(()) = self.install_selected();
      }
      Action::Open => {
        let Ok(()) = self.launch();
      }
      Action::Remove => {
        let Ok(()) = self.confirm_removal();
      }
      Action::ConfirmRemove => {
        match self.overlay.clone() {
          Some(Overlay::Remove(local)) => {
            let Ok(()) = self.change(PackageChange::Remove(local));
          }
          None | Some(Overlay::Package(_) | Overlay::Error { .. }) => {}
        };
      }
      Action::SearchDetails => {
        let notice = self.search_problem.as_ref();

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
      Action::RetrySearch => {
        let Ok(edit_query) = self.edit_query();

        edit_query
      }
      Action::Refresh => {
        let Ok(refresh) = self.refresh();

        refresh
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

  fn select_tab(&mut self, position: u32) -> Result<(), Infallible> {
    match position {
      0 => self.show_page(Page::Browse),
      1 => self.show_page(Page::Installed),
      2.. => Ok(()),
    }
  }

  fn show_page(&mut self, page: Page) -> Result<(), Infallible> {
    self.page = page;
    self.focus = None;
    self.hover = None;
    let Ok(()) = self.clear_query();

    match page {
      Page::Browse => Ok(()),
      Page::Installed => self.refresh(),
    }
  }

  fn tab_stop(&self) -> Result<Stop, Infallible> {
    let Ok(current) = self.page.index();
    Ok(match (self.focus == Some(Action::Close)).then_some(()) {
      Some(()) => Stop::Outside,
      None => Stop::Tab(current),
    })
  }

  fn step_tab(&mut self, step: i32) -> Result<WindowFlow, Infallible> {
    let Ok(from) = self.tab_stop();
    let Ok(next) = strip::along(2, from, step);

    match next {
      Stop::Outside => {
        self.focus = Some(Action::Close);
        self.preedit.clear();
        Ok(WindowFlow::Stay)
      }
      Stop::Tab(tab) => {
        match (next != from).then_some(()) {
          Some(()) => {
            let Ok(()) = self.select_tab(tab);
          }
          None => {}
        }

        Ok(WindowFlow::Stay)
      }
    }
  }

  fn keyboard(
    &mut self,
    press: PhysicalKeyPress,
    frame: &Frame,
  ) -> Result<WindowFlow, Infallible> {
    let KeyPress { key, modifiers, .. } = press.translated;
    let control = modifiers & KeyPress::CONTROL != 0;
    let driving = match (&self.overlay, self.focus) {
      (Some(Overlay::Remove(_)), _) => Driving::Sure,
      (None, Some(Action::Search)) => Driving::Search,
      (Some(Overlay::Package(_) | Overlay::Error { .. }), _) | (None, _) => Driving::Panel,
    };

    match (key, self.overlay.as_ref(), control, driving) {
      (Keysym::k | Keysym::f, None, true, _) | (Keysym::slash, None, false, Driving::Panel) => {
        return self.action(Action::Search);
      }
      (Keysym::u, None, true, Driving::Search) => return self.action(Action::ClearSearch),
      (Keysym::Tab | Keysym::ISO_Left_Tab, _, _, _) => {
        let direction = match (key == Keysym::ISO_Left_Tab || modifiers & 1 != 0).then_some(()) {
          Some(()) => FocusDirection::Previous,
          None => FocusDirection::Next,
        };
        let Ok(tab) = frame.canvas.targets.tab(self.focus, direction);
        self.focus = tab;
        return Ok(WindowFlow::Stay);
      }
      _ => {}
    }

    let Ok(transition) = NavigationKeys::handle(
      self.navigation,
      NavigationInput::Press {
        key: press,
        driving,
      },
    );
    self.navigation = transition.state;
    let meaning = match transition.effect {
      Some(NavigationEffect::Apply(meaning)) => meaning,
      Some(NavigationEffect::Tab(motion)) => return self.switch_tab(motion),
      None => return Ok(WindowFlow::Stay),
    };

    match (meaning, self.overlay.as_ref()) {
      (Meaning::Close | Meaning::Abandon, _) => return self.escape(),
      (Meaning::Tab(step), None) => return self.step_tab(step),
      (Meaning::Step(step), None) => {
        let Ok(()) = self.step_row(step, frame);
        return Ok(WindowFlow::Stay);
      }
      (Meaning::Step(step) | Meaning::Nudge(step), Some(_)) => {
        let Ok(()) = self.walk_controls(step, frame);
        return Ok(WindowFlow::Stay);
      }
      (Meaning::Nudge(step), None) => {
        let Ok(()) = self.nudge_category(step);
        return Ok(WindowFlow::Stay);
      }
      (Meaning::Choose, _) => return self.choose(frame),
      (Meaning::None | Meaning::More, None)
      | (Meaning::None | Meaning::More | Meaning::Tab(_), Some(_)) => {}
    }

    let Ok(()) = self.type_search(press.translated);
    Ok(WindowFlow::Stay)
  }

  fn switch_tab(&mut self, motion: TabMotion) -> Result<WindowFlow, Infallible> {
    match self.overlay {
      Some(_) => return Ok(WindowFlow::Stay),
      None => {}
    }

    let Ok(from) = self.tab_stop();
    let Ok(next) = strip::switched(2, from, motion);

    match next {
      Some(tab) => {
        let Ok(()) = self.select_tab(tab);
      }
      None => {}
    }

    Ok(WindowFlow::Stay)
  }

  fn walk_controls(&mut self, step: i32, frame: &Frame) -> Result<(), Infallible> {
    let direction = match (step < 0).then_some(()) {
      Some(()) => FocusDirection::Previous,
      None => FocusDirection::Next,
    };
    let Ok(target) = frame.canvas.targets.tab(self.focus, direction);
    self.focus = target;
    Ok(())
  }

  fn nudge_category(&mut self, step: i32) -> Result<(), Infallible> {
    let Ok(query) = self.query();

    match (self.page, query.trim(), self.focus) {
      (Page::Browse, "", None | Some(Action::Category(_) | Action::Details(_))) => {}
      (Page::Browse | Page::Installed, _, _) => return Ok(()),
    }

    let position = match (0_i32..)
      .zip(Category::EVERY)
      .find_map(|(position, category)| (category == self.category).then_some(position))
    {
      Some(position) => position,
      None => return Ok(()),
    };
    let Ok(last) =
      console_core_number_conversion::fitted::<_, i32>(Category::EVERY.len().saturating_sub(1));
    let Ok(next) = console_core_number_conversion::fitted::<_, u32>(
      position.saturating_add(step).clamp(0, last),
    );
    let Ok(index) = console_core_number_conversion::index(next);

    match Category::EVERY.get(index) {
      Some(category) => {
        let Ok(_) = self.action(Action::Category(*category));
      }
      None => {}
    }

    Ok(())
  }

  fn step_row(&mut self, step: i32, frame: &Frame) -> Result<(), Infallible> {
    let first = match frame.entries.first() {
      Some(first) => first,
      None => return self.walk_controls(step, frame),
    };
    let current = (0_i32..)
      .zip(&frame.entries)
      .find_map(|(row, (action, _))| (Some(*action) == self.focus).then_some(row));
    let entry = match current {
      Some(row) => {
        let Ok(last) =
          console_core_number_conversion::fitted::<_, i32>(frame.entries.len().saturating_sub(1));
        let Ok(next) = console_core_number_conversion::fitted::<_, u32>(
          row.saturating_add(step).clamp(0, last),
        );
        let Ok(index) = console_core_number_conversion::index(next);

        match frame.entries.get(index) {
          Some(entry) => entry,
          None => first,
        }
      }
      None => first,
    };
    let (action, area) = *entry;
    self.preedit.clear();
    let Ok(bottom) = area.bottom();
    let Ok(view_bottom) = frame.scroll_area.bottom();
    let needed = match area.origin.y.cmp(&frame.scroll_area.origin.y) {
      Ordering::Less => area.origin.y.saturating_sub(frame.scroll_area.origin.y),
      Ordering::Equal | Ordering::Greater => bottom.saturating_sub(view_bottom).max(0),
    };
    self.focus = Some(action);
    self.scroll_by(needed, frame)
  }

  fn choose(&mut self, frame: &Frame) -> Result<WindowFlow, Infallible> {
    match (self.focus, self.overlay.as_ref()) {
      (Some(Action::Search), _) => {
        match self.searching() {
          Ok(SearchStatus::Searching) => self.deadline = Some(Instant::now()),
          Ok(SearchStatus::Settled) => {}
        }

        let Ok(()) = self.step_row(1, frame);
        Ok(WindowFlow::Stay)
      }
      (Some(action), _) => self.action(action),
      (None, Some(_)) => Ok(WindowFlow::Stay),
      (None, None) => {
        let Ok(()) = self.step_row(1, frame);
        Ok(WindowFlow::Stay)
      }
    }
  }

  fn escape(&mut self) -> Result<WindowFlow, Infallible> {
    let Ok(query) = self.query();

    match (self.overlay.is_some(), query.is_empty(), self.focus) {
      (true, _, _) => self.action(Action::Back),
      (false, false, _) => {
        let Ok(()) = self.clear_query();
        self.focus = None;
        Ok(WindowFlow::Stay)
      }
      (false, true, Some(Action::Search)) => {
        self.focus = None;
        self.preedit.clear();
        Ok(WindowFlow::Stay)
      }
      (false, true, _) => self.action(Action::Close),
    }
  }

  fn type_search(&mut self, press: KeyPress) -> Result<(), Infallible> {
    const COMMAND_MODIFIERS: u32 = KeyPress::CONTROL | (1 << 3) | (1 << 6);

    match (
      self.overlay.as_ref(),
      press.modifiers & COMMAND_MODIFIERS,
      press.key,
    ) {
      (None, 0, _) | (None, KeyPress::CONTROL, Keysym::BackSpace) => {}
      _ => return Ok(()),
    }

    let Ok(letter) = press.key.char();
    let editable =
      self.focus == Some(Action::Search) || letter.is_some_and(|value| !value.is_control());

    match editable.then_some(()) {
      Some(()) => {
        self.focus = Some(Action::Search);
        let Ok(typed) = self.query.typed_press(press);

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
    let Ok(page) = view::PageView::current(self);
    let Ok(scroll) = self.scroll.current(page);
    *scroll = scroll.saturating_add(by).clamp(0, frame.canvas.max_scroll);
    self.wheel = scrolling::Position::default();

    Ok(())
  }

  fn scroll_position(&self) -> Result<i32, Infallible> {
    let Ok(page) = view::PageView::current(self);

    Ok(match page {
      view::PageView::Browse => self.scroll.browse,
      view::PageView::Installed => self.scroll.installed,
      view::PageView::Search => self.scroll.search,
    })
  }

  fn fit_scroll(&mut self, maximum: i32) -> Result<Editing, Infallible> {
    match self.overlay.as_ref() {
      Some(_) => return Ok(Editing::Unchanged),
      None => {}
    }

    let Ok(page) = view::PageView::current(self);
    let Ok(scroll) = self.scroll.current(page);
    let limit = maximum.max(0);

    Ok(match (*scroll).cmp(&limit) {
      Ordering::Greater => {
        *scroll = limit;
        Editing::Updated
      }
      Ordering::Less | Ordering::Equal => Editing::Unchanged,
    })
  }

  fn wheel(&mut self, by: LogicalPixels, limit: i32) -> Result<Editing, Infallible> {
    let Ok(page) = view::PageView::current(self);
    let Ok(scroll) = self.scroll.current(page);
    let Ok(turn) = scrolling::Wheel::handle(
      self.wheel,
      scrolling::Motion {
        at: *scroll,
        by,
        limit,
      },
    );
    self.wheel = turn.state;

    Ok(match turn.effect {
      Some(scrolling::ScrollTo(next)) => {
        *scroll = next;
        Editing::Updated
      }
      None => Editing::Unchanged,
    })
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
      title: "Console Store".to_owned(),
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

    let keys = self.surface.physical_key_presses()?;

    for (position, key) in (0_u32..).zip(keys) {
      match (position > 0).then_some(()) {
        Some(()) => {
          let Ok(logical) = self.surface.logical();

          match logical {
            Some(room) => {
              let Ok(()) = self.layout(room);
            }
            None => {}
          }
        }
        None => {}
      }

      let Ok(result) = self.shop.keyboard(key, &self.frame);

      match result {
        WindowFlow::Close => return Ok(WindowFlow::Close),
        WindowFlow::Stay => self.drawing = Drawing::Pending,
      }
    }

    let Ok(focus) = self.surface.keyboard_focus();

    match focus {
      KeyboardFocus::Elsewhere => self.shop.navigation = Navigation::Ready,
      KeyboardFocus::Here => {}
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

    let Ok(()) = self.layout(room);

    let painted = self.frame.canvas.paint(&mut self.surface, room);
    painted?;
    let Ok(state) = self.shop.input_state(&self.frame);
    self.surface.set_text_input(state)?;
    self.drawing = drawing;
    Ok(())
  }

  fn layout(&mut self, room: Size<u32>) -> Result<(), Infallible> {
    let Ok(frame) = view::draw(&self.shop, room, &self.theme);
    self.frame = frame;

    let Ok(changed) = self.shop.fit_scroll(self.frame.canvas.max_scroll);

    match changed {
      Editing::Updated => {
        let Ok(scrolled) = view::draw(&self.shop, room, &self.theme);
        self.frame = scrolled;
      }
      Editing::Unchanged => {}
    }

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
          let Ok(changed) = self
            .shop
            .wheel(LogicalPixels(by), self.frame.canvas.max_scroll);

          match changed {
            Editing::Updated => self.drawing = Drawing::Pending,
            Editing::Unchanged => {}
          }
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
    self.shop.navigation = Navigation::Ready;
    self.gesture = Some(Gesture {
      start: point,
      last: point,
      target,
      drag: Drag::Click,
    });
    let move_window = target.is_none()
      && self.shop.overlay.is_none()
      && point.y < f64::from(strip::MARGIN.saturating_add(strip::HEIGHT));

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
  use console_core_shapes::Shape;
  use console_waiting::{Outcome, Ready, Schedule, until_handed};

  fn key(key: Keysym) -> Result<PhysicalKeyPress, Infallible> {
    let Ok(translated) = KeyPress::unmodified(key);

    Ok(PhysicalKeyPress {
      code: 0,
      depressed_modifiers: 0,
      translated,
    })
  }

  #[test]
  fn store_details_and_other_tabs_keep_the_list_scroll() -> Result<(), Box<dyn std::error::Error>>
  {
    let mut shop = Shop::new()?;
    let Ok(_) = shop.wheel(LogicalPixels(128.5), 500);
    let Ok(_) = shop.action(Action::Details(0));
    assert!(matches!(shop.overlay, Some(Overlay::Package(_))));
    assert!(shop.fit_scroll(0) == Ok(Editing::Unchanged));
    let Ok(_) = shop.action(Action::Back);
    assert_eq!(shop.scroll_position(), Ok(128));

    let Ok(_) = shop.action(Action::Page(Page::Installed));
    assert_eq!(shop.scroll_position(), Ok(0));
    let Ok(_) = shop.wheel(LogicalPixels(72.0), 500);
    let Ok(_) = shop.action(Action::Page(Page::Browse));
    assert_eq!(shop.scroll_position(), Ok(128));
    let Ok(_) = shop.action(Action::Page(Page::Installed));
    assert_eq!(shop.scroll_position(), Ok(72));

    Ok(())
  }

  #[test]
  fn tabs_and_search_use_the_same_header_for_keys_and_clicks()
  -> Result<(), Box<dyn std::error::Error>> {
    let mut shop = Shop::new()?;
    let theme = Theme::new()?;
    let Ok(frame) = view::draw(&shop, WINDOW, &theme);

    for mark in [
      console_panel::marks::BEFORE,
      console_panel::marks::AFTER,
      console_panel::marks::SHUT,
    ] {
      assert!(
        frame
          .canvas
          .shapes
          .iter()
          .any(|shape| matches!(shape, Shape::Text(label) if label.said == mark))
      );
    }

    let Ok(next) = frame.canvas.targets.action(Point { x: 990.0, y: 32.0 });
    assert_eq!(next, Some(Action::NextTab));
    let Ok(press) = key(Keysym::Page_Down);
    let Ok(_flow) = shop.keyboard(press, &frame);
    assert_eq!(shop.page, Page::Installed);
    let Ok(installed) = view::draw(&shop, WINDOW, &theme);
    let Ok(previous) = installed.canvas.targets.action(Point { x: 32.0, y: 32.0 });
    assert_eq!(previous, Some(Action::PreviousTab));
    let Ok(_flow) = shop.action(Action::PreviousTab);
    assert_eq!(shop.page, Page::Browse);
    let Ok(slash) = key(Keysym::slash);
    let Ok(_flow) = shop.keyboard(slash, &installed);
    assert_eq!(shop.focus, Some(Action::Search));
    Ok(())
  }

  #[test]
  fn failed_search_shows_an_error_and_keeps_the_original_diagnostic()
  -> Result<(), Box<dyn std::error::Error>> {
    let mut shop = Shop::new()?;
    let Ok(query) = TextField::holding("no match");
    shop.query = query;
    let request = shop.request;
    let _previous = shop.search_in_flight.insert(Manager::Nix, request);
    let diagnostic = "error: opening lock file '/nix/var/nix/db/big-lock': Read-only file system";
    let Ok(()) = shop.found(Manager::Nix, request, Err(io::Error::other(diagnostic)));
    let notice = shop
      .search_problem
      .as_ref()
      .ok_or_else(|| io::Error::other("Missing search error"))?;
    assert_eq!(
      notice.detail.as_deref(),
      Some("The filesystem the store needs is read-only.")
    );

    let theme = Theme::new()?;
    let Ok(frame) = view::draw(&shop, WINDOW, &theme);
    let shows = |message| {
      frame
        .canvas
        .shapes
        .iter()
        .any(|shape| matches!(shape, Shape::Text(text) if text.said == message))
    };
    assert!(shows("Nixpkgs search failed."));
    assert!(!shows("No featured matches"));

    let Ok(_) = shop.action(Action::SearchDetails);
    assert!(matches!(&shop.overlay, Some(Overlay::Error { text, .. }) if text == diagnostic));

    Ok(())
  }

  #[test]
  fn zen_uses_its_pacman_actions_without_a_nix_profile() -> Result<(), Box<dyn std::error::Error>>
  {
    let mut shop = Shop::new()?;
    shop.nix_available = Installed::No;
    shop.pacman_changes = Installed::Yes;
    shop.profile_status = ProfileStatus::Ready;
    shop.page = Page::Installed;
    let source = Source::Pacman(String::from("zen-browser-bin"));
    let zen = Package {
      source: source.clone(),
      title: String::from("Zen Browser"),
      description: String::new(),
      version: String::from("1.24.2b-1"),
    };
    let Ok(local) = Local::packaged(zen, None);
    shop.installed = vec![local];
    let Ok(installable) = console_package_manager::user::Installable::nixpkgs("zen-browser-bin");
    let nix = Package {
      source: Source::Nix(installable),
      title: String::from("Zen Browser"),
      description: String::new(),
      version: String::new(),
    };
    let Ok(nix_copy) = shop.local(&nix);
    assert!(nix_copy.is_none());

    let theme = Theme::new()?;
    let Ok(_) = shop.action(Action::Details(0));
    let Ok(installed) = view::draw(&shop, WINDOW, &theme);
    assert!(
      installed
        .canvas
        .shapes
        .iter()
        .any(|shape| matches!(shape, Shape::Text(text) if text.said == "Remove"))
    );
    let Ok(_) = shop.action(Action::Remove);
    assert!(
      matches!(&shop.overlay, Some(Overlay::Remove(selected)) if selected.package.source == source)
    );

    shop.installed.clear();
    let Ok(_) = shop.action(Action::Back);
    let Ok(available) = view::draw(&shop, WINDOW, &theme);
    assert!(
      available
        .canvas
        .shapes
        .iter()
        .any(|shape| matches!(shape, Shape::Text(text) if text.said == "Install"))
    );

    Ok(())
  }

  #[test]
  fn a_stale_search_reply_releases_only_its_own_manager() -> io::Result<()> {
    let mut shop = Shop::new()?;
    let Ok(query) = TextField::holding("zen");
    shop.query = query;
    let previous = shop.request;
    let current = SearchId(1);
    shop.request = current;
    let _queued = shop.queued_searches.insert(Manager::Aur);
    let _aur = shop.search_in_flight.insert(Manager::Aur, previous);
    let _nix = shop.search_in_flight.insert(Manager::Nix, current);
    let Ok(()) = shop.found(Manager::Aur, previous, Ok(Matches::default()));

    assert!(!shop.search_in_flight.contains_key(&Manager::Aur));
    assert_eq!(shop.search_in_flight.get(&Manager::Nix), Some(&current));
    assert!(shop.deadline.is_some());
    assert!(shop.results.is_empty());
    assert!(matches!(shop.searching(), Ok(SearchStatus::Searching)));

    Ok(())
  }

  #[test]
  fn browse_lists_obs_as_a_regular_package_row() -> Result<(), Box<dyn std::error::Error>> {
    let mut shop = Shop::new()?;
    shop.nix_available = Installed::Yes;
    let theme = Theme::new()?;
    let Ok(frame) = view::draw(&shop, WINDOW, &theme);

    assert!(
      !frame
        .canvas
        .shapes
        .iter()
        .any(|shape| matches!(shape, Shape::Picture(_) | Shape::Cropped(_)))
    );
    let entry = frame.entries.first().ok_or("Missing OBS row")?;
    let (featured, area) = *entry;
    assert_eq!(featured, Action::Details(0));
    let point = Point {
      x: f64::from(area.origin.x.saturating_add(20)),
      y: f64::from(area.origin.y.saturating_add(20)),
    };
    let Ok(hit) = frame.canvas.targets.action(point);
    assert_eq!(hit, Some(featured));
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

    let Ok(press) = key(Keysym::Down);
    let Ok(_flow) = shop.keyboard(press, &frame);
    assert_eq!(shop.focus, Some(featured));
    let Ok(enter) = key(Keysym::Return);
    let Ok(_flow) = shop.keyboard(enter, &frame);
    assert!(
      matches!(&shop.overlay, Some(Overlay::Package(row)) if row.source.name() == Ok("obs-studio"))
    );
    let Ok(dialog) = view::draw(&shop, WINDOW, &theme);
    let Ok(back) = dialog.canvas.targets.action(Point { x: 790.0, y: 280.0 });
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

    let Ok(search) = frame.canvas.targets.action(Point { x: 265.0, y: 85.0 });
    let Ok(clear) = frame.canvas.targets.action(Point { x: 1030.0, y: 85.0 });
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
    assert_eq!(
      shop.results.get(&Manager::Nix).map(|matches| matches.total),
      Some(1)
    );

    let Ok(_details) = shop.action(Action::Details(0));
    let Ok(_install) = shop.action(Action::Install);
    wait_for(&mut shop, FixtureWait::Change)?;
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
