//! The menu: the surface every application on this machine is opened from.
//!
//! Applications come out in the order you actually use them: the ones you open
//! most, most often, and everything else alphabetically after them.
//!
//! It is drawn as a panel, like the settings, the guide and the files. It was
//! wofi for a long time, and wofi cost four separate things: it listed itself
//! under its own name, so the bar could not say whether the menu was up; it
//! could not be told to shrink when the on-screen keyboard took the bottom of
//! the screen; it wanted one press to highlight a row and another to take it;
//! and the icon that opened it could not close it. None of the four is a fault
//! in wofi. They are one fact: the menu was the only surface on this machine
//! that was not ours.
//!
//! What is typed is a name to the machine and a question to the browser, and
//! it does not stop being the second because it was the first. The last row of
//! the list offers to ask it, under everything the machine answered with.
//!
//! ## And it is where the home screen is filled from
//!
//! The home screen is the handful of applications someone put where they want
//! them, and this is every application there is, in the order they are used,
//! found by typing. They are the same list read two ways, which is why they
//! read it out of the same place -- and it is why what goes on the home screen
//! is decided here rather than on a card of its own. That card existed, it was
//! this list with a word beside some of the rows, and the word is all that is
//! left of it: Y puts the row you are standing on on the home screen, or takes
//! it off, and the row says which it is.
//!
//! A is untouched by that. A row does one thing on A -- open the application --
//! because one thing is what a thumb should have to know, and everything else
//! it could be done to is behind Y. That is what Y is on the files, on the
//! music, on the downloads and on the home screen itself.
//!
//! ## Except when the home screen asked
//!
//! `--place` is the home screen opening this on one of its empty squares, and
//! then the whole card is that one question: A puts what it is standing on
//! there, on the square that asked, and there is no Y and no browser. Subject
//! square it was travels as `console_home_screen::Spot::serialize` and comes back through
//! `Spot::read`, so only the home screen's own model says how a square is
//! spelled.
//!
//! It lives above `console_applications` rather than in it because of that reading:
//! the menu crate is what applications this machine has, the home screen is
//! built on top of it, and a surface that has to know about both belongs over
//! the two of them.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use console_actor::{Actor, SendError};
use console_applications::{counts, entry, found, narrow};
use console_core_arguments::{
  Command, CommandLine, Flag, NoSubcommand, Operands, Presence, Takes, read,
};
use console_core_internal_programs::LAUNCHER_KEEP;
use console_core_never::Never;
use console_core_state_machine::{EffectDependencies, Machine, Queue, Service};
use console_default_applications::engines;
use console_home_screen::shape::Shape;
use console_home_screen::{HomeScreen, Spot};
use console_panel::card::{Card, Door};
use console_panel::page::{ActionOutcome, Aside, Handler, LoadedRows, Page, Picture, Row, Rows};
use console_panel::picker::Again;

pub const WHO: &str = "launcher";

const DOOR: &str = "menu";

pub const PLACE: Flag = Flag {
  spelling: "--place",
  takes: Takes::Value("SQUARE"),
  about: "put what is chosen on this square of the home screen, written pane.row.column",
};

pub const COMMAND: Command = Command {
  name: WHO,
  about: "the menu",
  flags: &[LAUNCHER_KEEP, PLACE],
  operands: Operands::None,
};

pub fn door(arguments: &[String]) -> Result<Door, Never> {
  let Ok(read) = command_line(arguments);
  let kept = match read {
    Some(line) => line.presence(LAUNCHER_KEEP),
    None => Ok(Presence::Absent),
  };

  let again = match kept {
    Ok(Presence::Present) => Again::Keeps,
    Ok(Presence::Absent) => Again::Closes,
  };

  Door::new(DOOR, again)
}

fn command_line(arguments: &[String]) -> Result<Option<CommandLine<NoSubcommand>>, Never> {
  Ok(match read(&COMMAND, arguments) {
    Ok(line) => Some(line),
    Err(refusal) => {
      eprintln!("{refusal}, so this is the menu as it usually opens");

      None
    }
  })
}

pub fn card(arguments: &[String]) -> Result<Card, Never> {
  let Ok(going) = asked_for(arguments);

  Card::supervised::<Menu, _>((), move |typed| {
    let Ok(page) = menu_page(typed, going);

    Ok(vec![page])
  })
}

struct Everything {
  applications: BTreeMap<String, entry::Application>,
  icon: BTreeMap<String, String>,
  order: Vec<String>,
}

struct Menu;

#[derive(Clone, Copy, Default)]
enum Catalog {
  #[default]
  Cached,
  Live,
}

#[derive(Clone, Default)]
struct View {
  word: String,
  catalog: Catalog,
}

enum Request {
  Typed(String),
  PreviewShown,
}

#[derive(PartialEq, Eq)]
enum Effect {
  FocusFirst,
  RefreshRows,
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::FocusFirst | Self::RefreshRows => &[Service::Panels],
    })
  }
}

impl Machine for Menu {
  type Input = ();
  type State = View;
  type Request = Request;
  type Effect = Effect;

  fn initialize(
    _input: &(),
    _previous: Option<View>,
    _effects: &mut Queue<Effect>,
  ) -> Result<View, Never> {
    Ok(View::default())
  }

  fn handle(
    mut view: View,
    request: Request,
    effects: &mut Queue<Effect>,
  ) -> Result<View, Never> {
    match request {
      Request::Typed(word) => match (view.word != word).then_some(()) {
        Some(()) => {
          view.word = word;
          let Ok(()) = effects.offer(Effect::FocusFirst);
        }
        None => {}
      },
      Request::PreviewShown => match view.catalog {
        Catalog::Cached => {
          view.catalog = Catalog::Live;
          let Ok(()) = effects.offer(Effect::RefreshRows);
        }
        Catalog::Live => {}
      },
    }

    Ok(view)
  }
}

type MenuActor = Actor<Menu>;

const ABOUT: &str = "Type to narrow the list";

const ON: &str = "on the home screen";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum For {
  Opening,
  Placing(Spot),
}

fn standing() -> Result<console_screen::Shape, Never> {
  Ok(match console_home_screen::standing() {
    Ok(standing) => standing,
    Err(fault) => {
      eprintln!("launcher: which way up the screen stands: {fault}");

      console_screen::Shape::Wider
    }
  })
}

fn shape() -> Result<Shape, Never> {
  let hers = console_core_places::home()?;
  let Ok(standing) = standing();
  let usual = Shape::usual(standing)?;

  let hers_3 = match hers {
    Some(hers_2) => hers_2,
    None => return Ok(usual),
  };

  match console_home_screen::grid(&hers_3, standing) {
    Ok(grid) => Ok(grid),
    Err(fault) => {
      eprintln!("launcher: {fault}");

      Ok(usual)
    }
  }
}

fn home() -> Result<HomeScreen, Never> {
  let Ok(known) = found::load_cached();

  home_with(&known.applications)
}

fn home_with(applications: &BTreeMap<String, entry::Application>) -> Result<HomeScreen, Never> {
  let Ok(found) = console_core_places::home();

  let hers = match found {
    Some(hers) => hers,
    None => return Ok(HomeScreen::default()),
  };

  let Ok(standing) = standing();
  let Ok(held) = console_home_screen::kept(&hers, standing);

  match held {
    console_core_atomic_writes::Stored::Text(said) => {
      let Ok(mut home) = HomeScreen::read(&said);
      let Ok(()) = home.update_application_names(|name| applications.contains_key(name));
      Ok(home)
    }
    console_core_atomic_writes::Stored::Absent => Ok(HomeScreen::default()),
    console_core_atomic_writes::Stored::Failed(fault) => {
      eprintln!("launcher: the home screen: {fault}");

      Ok(HomeScreen::default())
    }
  }
}

fn keep(home: &HomeScreen) -> Result<(), Never> {
  let Ok(found) = console_core_places::home();

  let hers = match found {
    Some(hers) => hers,
    None => {
      eprintln!("launcher: no home to keep the home screen in; leaving it as it is");

      return Ok(());
    }
  };

  let Ok(standing) = standing();

  match console_home_screen::keep(&hers, standing, home) {
    Ok(()) => {}
    Err(fault) => eprintln!("launcher: {fault}"),
  }

  Ok(())
}

fn toggle_on_home(name: &str) -> Result<(), Never> {
  let Ok(home) = home();
  let Ok(shape) = shape();
  let Ok(turned) = toggle_in(home, shape, name);
  let Ok(()) = keep(&turned);

  Ok(())
}

fn toggle_in(mut home: HomeScreen, shape: Shape, name: &str) -> Result<HomeScreen, Never> {
  let placed = home.where_(name)?;

  match placed {
    Some(_) => home.forget(name)?,
    None => {
      let spot = home.first_free(shape)?;

      home.place(spot, name)?;
    }
  }

  Ok(home)
}

fn place_on_home(spot: Spot, name: &str) -> Result<(), Never> {
  let Ok(home) = home();
  let Ok(placed) = place_in(home, spot, name);
  let Ok(()) = keep(&placed);

  Ok(())
}

fn place_in(mut home: HomeScreen, spot: Spot, name: &str) -> Result<HomeScreen, Never> {
  home.forget(name)?;

  home.place(spot, name)?;

  Ok(home)
}

fn everything() -> Result<Everything, Never> {
  let machine = found::machine()?;

  with_counts(machine)
}

fn with_counts(found: found::Found) -> Result<Everything, Never> {
  let names: Vec<String> = found.applications.keys().cloned().collect();

  let counted = found::load_counts()?;

  let order = counts::order(&names, &counted)?;

  Ok(Everything {
    applications: found.applications,
    icon: found.icon,
    order,
  })
}

fn application_row(
  all: &Everything,
  name: &str,
  going: For,
  on: &HomeScreen,
) -> Result<Row, Never> {
  let picture = all
    .icon
    .get(name)
    .map_or(Picture::Space, |at| Picture::At(PathBuf::from(at)));
  let named = name.to_string();

  match going {
    For::Placing(spot) => {
      let Ok(places) = Handler::call(move |_| {
        let Ok(()) = place_on_home(spot, &named);
        ActionOutcome::Close
      });
      let Ok(row) = Row::new(name, Aside(""), places);

      row.picturing(picture)
    }
    For::Opening => {
      let application = all.applications.get(name).cloned();
      let placed = on.where_(name)?;

      let aside = match placed {
        Some(_) => ON,
        None => "",
      };
      let switching = name.to_string();

      let Ok(starts) = Handler::call(move |_| {
        let Ok(()) = start(application.as_ref(), &named);
        ActionOutcome::Close
      });
      let Ok(row) = Row::new(name, Aside(aside), starts);
      let Ok(pictured) = row.picturing(picture);

      pictured.offering(move |showing| {
        let Ok(()) = toggle_on_home(&switching);
        showing.refresh();

        ActionOutcome::Stay
      })
    }
  }
}

fn looking_up_row(said: &str) -> Result<Row, Never> {
  let word = said.to_string();
  let Ok(looks) = Handler::call(move |_| {
    let Ok(()) = looked_up(&word);
    ActionOutcome::Close
  });
  let Ok(row) = Row::new(&format!("Look up {said:?}"), Aside(""), looks);

  row.picturing(Picture::Space)
}

fn rows(word: &str, all: &Everything, going: For) -> Result<Vec<Row>, Never> {
  let Ok(on) = home_with(&all.applications);
  let standing = narrow::matching(&all.order, word)?;
  let mut rows: Vec<Row> = standing
    .iter()
    .map(|name| {
      let Ok(row) = application_row(all, name, going, &on);

      row
    })
    .collect();
  let said = word.trim();

  match ::core::primitive::bool::then_some(!said.is_empty() && going == For::Opening, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(row) = looking_up_row(said);

      rows.push(row);
    }
    ::core::option::Option::None => {}
  }

  Ok(rows)
}

fn title(going: For) -> Result<&'static str, Never> {
  Ok(match going {
    For::Opening => "Menu",
    For::Placing(_) => "Put one on the home screen",
  })
}

#[derive(Default)]
struct Catalogs {
  cached: OnceLock<Everything>,
  live: OnceLock<Everything>,
}

impl Catalogs {
  fn cached(&self) -> Result<Option<&Everything>, Never> {
    let remembered = self.cached.get_or_init(|| {
      let Ok(found) = found::load_cached();
      let Ok(all) = with_counts(found);

      all
    });

    Ok((!remembered.order.is_empty()).then_some(remembered))
  }

  fn live(&self) -> Result<&Everything, Never> {
    Ok(self.live.get_or_init(|| {
      let Ok(all) = everything();

      all
    }))
  }
}

fn menu_rows(held: &MenuActor, catalogs: &Catalogs, going: For) -> Result<LoadedRows, Never> {
  let view = match held.get() {
    Ok(view) => view,
    Err(_closed) => View::default(),
  };
  let cache = match view.catalog {
    Catalog::Cached => catalogs.cached()?,
    Catalog::Live => None,
  };
  let Ok(all) = match cache {
    Some(cached) => Ok(cached),
    None => catalogs.live(),
  };
  let listed = rows(&view.word, all, going)?;
  let Ok(loaded) = LoadedRows::new(listed);

  match cache {
    Some(_) => {
      let after = held.clone();

      loaded.then(move |_| match after.send(Request::PreviewShown) {
        Ok(effects) | Err(SendError::MachineDefect(effects)) => {
          match effects.contains(&Effect::RefreshRows).then_some(()) {
            Some(()) => {
              let Ok(()) = console_panel::frames::tell(console_panel::frames::Notice::Rows);
            }
            None => {}
          }
        }
        Err(SendError::Closed) => {
          console_core_logging::debug!("launcher: the menu closed before its preview was read")
        }
      })
    }
    None => Ok(loaded),
  }
}

fn menu_page(typed: &MenuActor, going: For) -> Result<Page, Never> {
  let listing = typed.clone();
  let typing = typed.clone();
  let catalogs = Catalogs::default();

  let Ok(heading) = title(going);
  let identity = match going {
    For::Opening => "launcher loading",
    For::Placing(_) => "launcher placement loading",
  };

  let Ok(asked) = Rows::with_completion(move || {
    let Ok(loaded) = menu_rows(&listing, &catalogs, going);

    loaded
  });
  let Ok(page) = Page::new(heading, asked);
  let Ok(opening) = page.fixed_opening(identity);
  let Ok(with_preview) = opening.meanwhile(|| {
    let Ok(loading) = Row::placeholder("Loading apps…");

    vec![loading]
  });
  with_preview.searching(ABOUT, move |showing, word| {
    let narrowed = typing.send(Request::Typed(word.to_string()));

    match narrowed {
      Ok(effects) => match effects.as_slice() {
        [Effect::FocusFirst] => showing.replace(0),
        _ => {}
      },
      Err(SendError::Closed | SendError::MachineDefect(_)) => showing.replace(0),
    }
  })
}

fn asked_for(asked: &[String]) -> Result<For, Never> {
  let Ok(read) = command_line(asked);
  let placing = match &read {
    Some(line) => line.value(PLACE),
    None => Ok(None),
  };

  let said = match placing {
    Ok(Some(said)) => said,
    Ok(None) => return Ok(For::Opening),
  };

  let read_2 = Spot::read(said)?;

  Ok(match read_2 {
    Some(spot) => For::Placing(spot),
    None => {
      eprintln!("launcher: --place {said:?} is not a square, so this is the menu");

      For::Opening
    }
  })
}

fn start(application: Option<&entry::Application>, chosen: &str) -> Result<(), Never> {
  match application {
    Some(application_2) => {
      let command = found::command(application_2)?;

      match command {
        Some(arguments) => {
          let Ok(()) = console_panel::running::left_running(&arguments);
        }
        None => {}
      }
    }
    None => {
      let Ok(()) = looked_up(chosen);
    }
  }

  Ok(())
}

fn looked_up(said: &str) -> Result<(), Never> {
  let chosen = engines::current()?;
  let known = engines::one(&chosen)?;

  let engine = match known {
    Some(engine) => engine,
    None => return Ok(()),
  };

  let asked = engines::address(said, engine)?;

  let address = match asked {
    Some(address) => address,
    None => return Ok(()),
  };

  eprintln!("the menu was asked {said:?}: {address}");
  let Ok(opening) = console_default_applications::opening::arguments(&address);
  let Ok(()) = console_panel::running::left_running(&opening);

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn presenting_the_cache_requests_the_live_catalog_once() {
    let Ok(initial) = Menu::initial_transition(&(), None);
    let Ok(first) = Menu::transition(initial.state, Request::PreviewShown);
    assert!(matches!(first.effects.as_slice(), [Effect::RefreshRows]));
    let Ok(refresh) = Menu::transition(first.state, Request::PreviewShown);
    assert!(refresh.effects.is_empty());
  }

  const GRID: Shape = Shape::USUAL;

  fn occupied_home() -> Result<HomeScreen, Never> {
    let mut home = HomeScreen::default();
    let Ok(()) = home.place(
      Spot {
        pane: 0,
        row: 0,
        column: 0,
      },
      "Files",
    );
    let Ok(()) = home.place(
      Spot {
        pane: 0,
        row: 0,
        column: 1,
      },
      "Music",
    );

    Ok(home)
  }

  #[test]
  fn y_is_a_switch() {
    let Ok(home) = occupied_home();
    let Ok(on) = toggle_in(home.clone(), GRID, "Download");
    let first = Spot {
      pane: 0,
      row: 0,
      column: 0,
    };

    let free = Spot {
      pane: 0,
      row: 0,
      column: 2,
    };

    assert_eq!(
      on.where_("Download"),
      Ok(Some(free)),
      "the first free square"
    );
    assert_eq!(on.at(first), Ok(Some("Files")), "and nothing else moved");

    let Ok(off) = toggle_in(on, GRID, "Download");
    assert_eq!(off.where_("Download"), Ok(None), "pressed again, it is off");
    assert_eq!(off, home, "and the home screen is where it was");
  }

  #[test]
  fn a_square_that_was_asked_for_is_the_square_it_lands_on() {
    let asked = Spot {
      pane: 2,
      row: 1,
      column: 3,
    };
    let Ok(home) = occupied_home();
    let Ok(placed) = place_in(home.clone(), asked, "Download");
    let Ok(free) = home.first_free(GRID);

    assert_eq!(placed.where_("Download"), Ok(Some(asked)));
    assert_ne!(asked, free, "the point of asking");
  }

  #[test]
  fn one_that_is_already_on_it_is_moved() {
    let asked = Spot {
      pane: 1,
      row: 0,
      column: 0,
    };
    let Ok(home) = occupied_home();
    let Ok(placed) = place_in(home, asked, "Files");

    let first = Spot {
      pane: 0,
      row: 0,
      column: 0,
    };

    assert_eq!(
      placed.at(first),
      Ok(None),
      "and it left the square it was on"
    );
  }

  #[test]
  fn what_the_words_after_the_program_name_ask_for() {
    let asked = |words: &[&str]| {
      asked_for(
        &words
          .iter()
          .map(|word| (*word).to_string())
          .collect::<Vec<String>>(),
      )
    };

    let spot = Spot {
      pane: 2,
      row: 1,
      column: 3,
    };

    assert_eq!(asked(&["--place", "2.1.3"]), Ok(For::Placing(spot)));
    assert_eq!(
      asked(&[]),
      Ok(For::Opening),
      "the menu, opened the way it usually is"
    );
    assert_eq!(
      asked(&["--keep"]),
      Ok(For::Opening),
      "the paddles, which only open it"
    );
    assert_eq!(
      asked(&["--place"]),
      Ok(For::Opening),
      "a square that was not said"
    );
    assert_eq!(
      asked(&["--place", "sideways"]),
      Ok(For::Opening),
      "a square that is not one"
    );
  }
}
