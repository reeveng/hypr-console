//! The buttons, and where they are on this device.
//!
//! ```text
//!     mapping-panel            open it
//!     mapping-panel --first    open it because no one has answered yet
//! ```
//!
//! One row per thing the desktop does, what plays it now beside it. A on a row
//! does the thing, the way pressing its button would -- which is what makes a
//! job reachable by a finger, closing a window whose program draws no close
//! button included. Y on a row is where it is changed: a button added beside
//! the ones it has, or one of them taken off. Adding asks for the button by
//! putting a card up and waiting for a press. The card is a program of its
//! own, because what tells the controller daemon to make the front of the
//! machine inert while the question is on screen is the card's own layer being
//! there.
//!
//! This is also the guide. It used to be a second panel reading the same table
//! read-only, one door along, and the two disagreed about what a tap did; the
//! guide's sections for what a menu, the Home Screen or the keyboard does with
//! a button come after the two pages here, and it opens on whichever of them
//! is true of the screen it was raised over.
//!
//! A tab per input, and it opens on the one last pressed. Both are always
//! here -- a keyboard's page is drawn on a machine with no keyboard attached,
//! because a job with nothing on it there is exactly what someone about to
//! plug one in wants to see -- so the last press decides only which is in
//! front. It is read from the file the daemon writes rather than heard live,
//! and `console_input_bindings::active` is the argument: a page that moved
//! under someone halfway through moving a job would be answering a question
//! no one asked.
//!
//! What is here is the machine: where the table is, what this device can send,
//! and a surface. `crate::rows` is the screen and
//! `crate::update` is what a press decides, and neither has
//! ever seen one.

use crate::Unmapped;
use crate::rows::{Choice, PUT_BACK_SURE, PUT_BACK_YES, Part, choices, every, parts, row, rows};
use crate::table;
use crate::update::{Empty, MappingEffect, MappingEvent, Setting, Setup};
use console_button_guide::guide::{Section, in_front, reference};
use console_core_arguments::{Command, Operands, Presence, Takes, read};
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_state_machine::{Machine, Transition};
use console_input_bindings::bound::{Binding, EVERY, Input};
use console_input_controller::actions::{MappedAction, Table};
use console_input_controller::mode::{Focused, Mode, WakeState};
use console_panel::card::{Card, Door};
use console_panel::page::{
  ActionOutcome, Answer, Aside, Handler, Initial, Page, Row, Rows, Showing, Subject,
};
use console_program_contract::{Effect, Event, ExitStatus, FileWrite, Notification};

const DOOR: &str = "buttons";

fn press(setting: &Setting, heard: MappingEvent, showing: &dyn Showing) -> Result<(), Never> {
  let Ok(Transition { state, effects }) =
    Setup::transition(setting.clone(), Event::Custom(heard));

  for effect in &effects {
    let Ok(()) = carry(effect, &state, showing);
  }

  Ok(())
}

fn carry(
  effect: &Effect<MappingEffect>,
  setting: &Setting,
  showing: &dyn Showing,
) -> Result<(), Never> {
  match effect {
    Effect::Custom(MappingEffect::Notify(said)) => {
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Controller Mapping"),
        said,
        None,
      );
    }
    Effect::Notify(notice) => {
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Controller Mapping"),
        &notice.summary,
        notice.body.as_deref(),
      );
    }

    Effect::Custom(MappingEffect::Sure) => {
      let Ok(putting) = putting_back();

      showing.sure(
        PUT_BACK_SURE,
        Subject(""),
        &[PUT_BACK_YES],
        Box::new(move |showing_2, _| {
          let Ok(()) = press(&putting, MappingEvent::Sure, showing_2);
        }),
      );
    }

    Effect::Custom(MappingEffect::AskForWorkspaceName) => {
      let waiting = setting.clone();
      #[cfg_attr(
        dylint_lib = "explicit081_no_shared_ownership",
        expect(
          explicit081_no_shared_ownership,
          reason = "the panel retains its text-prompt callback"
        )
      )]
      let answer: Answer = std::sync::Arc::new(move |page, name| {
        let Ok(()) = press(
          &waiting,
          MappingEvent::SpecialWorkspaceNamed(name.to_owned()),
          page,
        );
      });

      showing.ask_aloud("Special workspace name", Initial(""), answer);
    }

    Effect::Custom(MappingEffect::RegisterSpecialWorkspace(command)) => {
      let Ok(task) = console_panel::tasks::Task::of(command.clone());
      let Ok(attributed) = task.from_application(console_notifications::Application("Buttons"));
      let Ok(register) = attributed.then(|answer| {
        Ok(match answer.status {
          ExitStatus::Success | ExitStatus::Interrupt => None,
          ExitStatus::Failure(_) => Some(Notification {
            summary: "Could not add the special workspace shortcut".to_owned(),
            body: Some(answer.error_output.trim().to_owned()),
          }),
        })
      });

      showing.run(register);
    }

    Effect::Run(runs) => {
      let Ok(task) = console_panel::tasks::Task::of(runs.clone());

      showing.run(task);
    }

    Effect::Write(writing) => match table::write_contents(&writing.path, &writing.contents) {
      Ok(()) => {}
      Err(fault) => {
        let Ok(()) = console_notifications::notify(
          console_notifications::Application("Controller Mapping"),
          "Could not save controller mappings",
          Some(&fault.to_string()),
        );
      }
    },

    Effect::Stream(_)
    | Effect::Fork(_)
    | Effect::Interrupt(_)
    | Effect::Prompt(_)
    | Effect::Spawn(_)
    | Effect::Subscribe(_)
    | Effect::Unsubscribe(_)
    | Effect::Print(_)
    | Effect::Stop(_) => {}
  }

  Ok(())
}

fn wrote_initial(writing: &FileWrite) -> Result<(), Unmapped> {
  use console_core_atomic_writes::Unwritten;

  match console_core_atomic_writes::whole_in_new_file_with_folders(
    &writing.path,
    writing.contents.as_ref(),
  ) {
    Ok(()) => Ok(()),
    Err(Unwritten::Moving(at, fault)) => {
      match ::core::primitive::bool::then_some(
        fault.kind() == std::io::ErrorKind::AlreadyExists,
        (),
      ) {
        ::core::option::Option::Some(()) => Ok(()),
        ::core::option::Option::None => Err(Unmapped::Writing(Unwritten::Moving(at, fault))),
      }
    }
    Err(fault) => Err(Unmapped::Writing(fault)),
  }
}

fn putting_back() -> Result<Setting, Never> {
  let Ok(at) = table::at();

  Ok(match at {
    Some(at_2) => Setting::Set { at: at_2 },
    None => Setting::Nowhere,
  })
}

fn changing(part: &Part) -> Result<Row, Never> {
  let Ok(row) = row(part);
  let part_2 = part.clone();

  row.offering(move |showing| {
    let Ok(offered) = choices(&part_2);
    let words: Vec<String> = offered.iter().map(|(said, _)| said.clone()).collect();
    let said: Vec<&str> = words.iter().map(String::as_str).collect();
    let does = part_2.does.clone();
    let part_3 = part_2.clone();

    showing.sure(
      &does,
      Subject(""),
      &said,
      Box::new(move |showing_2, which| {
        let Ok(at) = index(which);
        let chosen = offered.get(at);

        match chosen {
          Some((_, Choice::Add)) => {
            let Ok(putting) = putting_back();
            let Ok(()) = press(&putting, MappingEvent::Requested(part_3), showing_2);
          }
          Some((_, Choice::Remove(binding))) => {
            let Ok(()) = removed(&part_3, binding, showing_2);
          }
          None => {}
        }
      }),
    );

    ActionOutcome::Stay
  })
}

fn removed(part: &Part, binding: &Binding, showing: &dyn Showing) -> Result<(), Never> {
  let changed = table::change(|jobs| {
    let Ok(current) = Table::of(jobs);
    let Ok(every) = every(&current);
    let Ok(()) = jobs.remove(&every, &part.slug, binding);

    Ok(table::Change::Save(()))
  });

  match changed {
    Ok(()) => showing.refresh(),
    Err(fault) => {
      let Ok(()) = console_notifications::notify(
        console_notifications::Application("Controller Mapping"),
        "Could not remove the controller binding",
        Some(&fault.to_string()),
      );
    }
  }

  Ok(())
}

fn puts_it_all_back() -> Result<Handler, Never> {
  Handler::and_stay(|showing| {
    let Ok(putting) = putting_back();
    let Ok(()) = press(&putting, MappingEvent::Restore, showing);
  })
}

fn add_special_workspace() -> Result<Row, Never> {
  let Ok(handler) = Handler::and_stay(|showing| {
    let Ok(setting) = putting_back();
    let Ok(()) = press(&setting, MappingEvent::AddSpecialWorkspace, showing);
  });

  Row::new("Add Special Workspace", Aside(""), handler)
}

fn one_input(on: Input) -> Result<Vec<Row>, Never> {
  let Ok(table) = table::table();
  let Ok(front) = table::front();
  let Ok(mut parts) = parts(&table, &front, on);
  let Ok(back) = puts_it_all_back();

  parts.sort_by_key(|part| match part.action {
    MappedAction::SpecialWorkspace(_) => 0,
    MappedAction::Builtin(_) => 1,
  });

  let Ok(mut listed) = rows(
    &parts,
    |part| {
      let Ok(row) = changing(part);

      row
    },
    back,
  );
  let Ok(add) = add_special_workspace();

  listed.insert(0, add);
  Ok(listed)
}

fn pages() -> Result<Vec<Page>, Never> {
  let mut pages = Vec::new();

  for on in EVERY {
    let Ok(asked) = Rows::computed(move || {
      let Ok(rows) = one_input(on);

      rows
    });
    let Ok(says) = on.says();
    let Ok(page) = Page::new(says, asked);

    pages.push(page);
  }

  let Ok(table) = table::table();
  let Ok(reference) = reference(&table);

  for section in reference {
    let Ok(page) = guided(&section);

    pages.push(page);
  }

  Ok(pages)
}

fn guided(section: &Section) -> Result<Page, Never> {
  let rows = section
    .lines
    .iter()
    .map(|line| {
      let Ok(row) = Row::text(&line.button, Aside(&line.does));

      row
    })
    .collect();

  Page::new(&section.title, Rows::Fixed(rows))
}

fn first_time(argv: &[String]) -> Result<Presence, Never> {
  match read(&COMMAND, argv) {
    Ok(line) => line.presence(FIRST_TIME),
    Err(refusal) => {
      eprintln!("{refusal}, so it opens the way it usually does");

      Ok(Presence::Absent)
    }
  }
}

fn opening(argv: &[String]) -> Result<Setting, Never> {
  let Ok(at) = table::at();
  let Ok(first) = first_time(argv);

  Ok(match at {
    Some(at_2) => {
      let Ok(empty) = Empty::of(first);

      Setting::Initial { at: at_2, empty }
    }
    None => Setting::Nowhere,
  })
}

pub const WHO: &str = "mapping-panel";

pub const FIRST_TIME: console_core_arguments::Flag = console_core_arguments::Flag {
  spelling: "--first",
  takes: Takes::None,
  about: "open it because no one has answered yet",
};

pub const COMMAND: Command = Command {
  name: WHO,
  about: "the buttons, and where they are on this device",
  flags: &[FIRST_TIME],
  operands: Operands::None,
};

pub fn door(_argv: &[String]) -> Result<Door, Never> {
  Door::closing(DOOR)
}

pub fn card(argv: &[String]) -> Result<Card, Never> {
  let Ok(setting) = opening(argv);

  let Ok(opened) = Setup::initial_transition(&setting, None);
  let Ok(Transition { state: _, effects }) = Setup::transition(opened.state, Event::Opened);

  let writings = effects.iter().filter_map(|effect| {
    let Ok(writing) = effect.as_file_write();

    writing
  });

  for writing in writings {
    match wrote_initial(writing) {
      Ok(()) => {}
      Err(fault) => eprintln!("mapping-panel: {fault}"),
    }
  }

  let Ok(card) = Card::new(|| {
    let Ok(pages) = pages();

    pages
  });
  let Ok(first) = opens_on();

  card.opening_at(Some(&first))
}

fn opens_on() -> Result<String, Never> {
  let Ok(home) = console_core_places::home();

  let Ok(on) = match home {
    Some(home_2) => console_input_bindings::active::read(&home_2),
    None => Ok(console_input_bindings::active::FIRST),
  };

  let Ok(front) = front();
  let Ok(in_front) = in_front(front);
  let Ok(hand) = on.says();

  Ok(match in_front {
    Some(title) => title.to_string(),
    None => hand.to_string(),
  })
}

fn front() -> Result<Mode, Never> {
  let screens = match console_onscreen::screens() {
    Ok(screens) => screens,
    Err(fault) => {
      eprintln!("mapping-panel: {fault}");

      return Ok(Mode::Desktop);
    }
  };
  let Ok(awake) = WakeState::detect();
  let focused = match console_onscreen::in_front() {
    Ok(focused) => focused,
    Err(_the_compositor_would_not_say_which_window_is_in_front) => Focused::SomethingElse,
  };

  Mode::detect(&screens, awake, focused)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_card_reads_the_first_opening_off_the_words_it_has() {
    let first = first_time(&[FIRST_TIME.spelling.to_string()]);
    let plain = first_time(&[]);

    assert_eq!(
      first,
      Ok(Presence::Present),
      "the host draws this panel, and the host's own command line has no --first"
    );
    assert_eq!(plain, Ok(Presence::Absent));
  }
}
