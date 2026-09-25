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

use std::sync::Arc;

use crate::Unmapped;
use crate::update::{FIRST, MappingEvent, MappingEffect, Setting, Setup, TABLE, WRITTEN};
use crate::rows::{Choice, PUT_BACK_SURE, PUT_BACK_YES, Part, choices, every, parts, row, rows};
use crate::table;
use console_button_guide::guide::{Section, in_front, reference};
use console_core_never::Never;
use console_core_number_conversion::index;
use console_input_controller::mode::{Woken, Mode};
use console_panel::page::{Aside, Handler, Page, Row, Rows, Showing, Subject};
use console_panel::card::{Card, Door};
use console_input_bindings::bound::{Binding, EVERY, Input};
use console_program_contract::{Arguments, Effect, Executable, Program, Update, Event, FileWrite};

const DOOR: &str = "buttons";

fn press(setting: &Setting, heard: MappingEvent, showing: &dyn Showing) -> Result<(), Never> {
    let Update { effects, .. } = Setup::update(setting, &Event::Custom(heard));

    for effect in &effects {
        let Ok(()) = carry(effect, showing);
    }

    Ok(())
}

fn carry(effect: &Effect<MappingEffect>, showing: &dyn Showing) -> Result<(), Never> {
    match effect {
        Effect::Custom(MappingEffect::Note(said)) => showing.note(said),

        Effect::Custom(MappingEffect::Sure) => {
            let Ok(putting) = putting_back();

            showing.sure(PUT_BACK_SURE, Subject(""), &[PUT_BACK_YES], Arc::new(move |showing, _| {
                let Ok(()) = press(&putting, MappingEvent::Sure, showing);
            }));
        }

        Effect::Run(runs) => match runs.program {
            Executable::Internal(name) => {
                let mut whole = vec![name.to_string()];

                whole.extend(runs.arguments.clone());
                showing.later(whole);
            }
            Executable::External(_) => {},
        },

        Effect::Write(writing) => match wrote(writing) {
            Ok(()) => {},
            Err(fault) => showing.note(&fault.to_string()),
        },

        Effect::Stream(_)
        | Effect::Prompt(_)
        | Effect::Spawn(_)
        | Effect::Subscribe(_)
        | Effect::Unsubscribe(_)
        | Effect::Notify(_)
        | Effect::Print(_)
        | Effect::Stop(_) => {},
    }

    Ok(())
}

fn wrote(writing: &FileWrite) -> Result<(), Unmapped> {
    match writing.path.parent() {
        Some(holding) => std::fs::create_dir_all(holding)
            .map_err(|fault| Unmapped::Holding(holding.to_path_buf(), fault))?,
        None => {},
    }

    console_core_atomic_writes::whole(&writing.path, writing.contents.as_bytes())
        .map_err(Unmapped::Writing)
}

fn putting_back() -> Result<Setting, Never> {
    let Ok(at) = table::at();

    Ok(match at {
        Some(at) => Setting::Set { at },
        None => Setting::Nowhere,
    })
}

fn changing(part: &Part) -> Result<Row, Never> {
    let Ok(row) = row(part);
    let part = part.clone();

    row.offering(move |showing| {
        let Ok(offered) = choices(&part);
        let words: Vec<String> = offered.iter().map(|(said, _)| said.clone()).collect();
        let said: Vec<&str> = words.iter().map(String::as_str).collect();
        let does = part.does.clone();
        let part = part.clone();

        showing.sure(&does, Subject(""), &said, Arc::new(move |showing, which| {
            let Ok(at) = index(which);
            let chosen = offered.get(at);

            match chosen {
                Some((_, Choice::Add)) => {
                    let Ok(putting) = putting_back();
                    let Ok(()) = press(&putting, MappingEvent::Requested(part.clone()), showing);
                }
                Some((_, Choice::Remove(binding))) => {
                    let Ok(()) = removed(&part, binding, showing);
                }
                None => {},
            }
        }));

        false
    })
}

fn removed(part: &Part, binding: &Binding, showing: &dyn Showing) -> Result<(), Never> {
    let Ok(mut jobs) = table::read();
    let Ok(table) = table::table();
    let Ok(every) = every(&table);
    let Ok(()) = jobs.removing(&every, &part.slug, binding);

    match table::write(&jobs) {
        Ok(()) => showing.refresh(),
        Err(fault) => showing.note(&fault.to_string()),
    }

    Ok(())
}

fn puts_it_all_back() -> Result<Handler, Never> {
    Handler::and_stay(|showing| {
        let Ok(putting) = putting_back();
        let Ok(()) = press(&putting, MappingEvent::Restore, showing);
    })
}

fn one_input(on: Input) -> Result<Vec<Row>, Never> {
    let Ok(table) = table::table();
    let Ok(front) = table::front();
    let Ok(parts) = parts(&table, &front, on);
    let Ok(back) = puts_it_all_back();

    rows(
        &parts,
        |part| {
            let Ok(row) = changing(part);

            row
        },
        back,
    )
}

fn pages() -> Result<Vec<Page>, Never> {
    let mut pages = Vec::new();

    for on in EVERY {
        let Ok(asked) = Rows::asked(move || {
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
            let Ok(row) = Row::said(&line.button, Aside(&line.does));

            row
        })
        .collect();

    Page::new(&section.title, Rows::Fixed(rows))
}

fn opening() -> Result<Arguments, Never> {
    let Ok(at) = table::at();

    let mut words: Vec<String> = match &at {
        Some(at) => vec![TABLE.to_string(), at.display().to_string()],
        None => Vec::new(),
    };

    match std::env::args().any(|word| word == FIRST) {
        true => words.push(FIRST.to_string()),
        false => {},
    }

    match at.is_some_and(|at| at.exists()) {
        true => words.push(WRITTEN.to_string()),
        false => {},
    }

    Arguments::of(&words.iter().map(String::as_str).collect::<Vec<&str>>())
}


pub const WHO: &str = "mapping-panel";

pub fn door(_argv: &[String]) -> Result<Door, Never> {
    Door::closing(DOOR)
}

pub fn card(_argv: &[String]) -> Result<Card, Never> {
    let Ok(arguments) = opening();

    let opened = Setup::init(&arguments);
    let Update { effects, .. } = Setup::update(&opened.state, &Event::Opened);

    let writings = effects.iter().filter_map(|effect| {
        let Ok(writing) = effect.written();

        writing
    });

    for writing in writings {
        match wrote(writing) {
            Ok(()) => {},
            Err(fault) => eprintln!("mapping-panel: {fault}"),
        }
    }

    let Ok(card) = Card::new(Arc::new(|| {
        let Ok(pages) = pages();

        pages
    }));
    let Ok(first) = opens_on();

    card.opening_at(Some(&first))
}

fn opens_on() -> Result<String, Never> {
    let Ok(home) = console_core_places::home();

    let Ok(on) = match home {
        Some(home) => console_input_bindings::active::read(&home),
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
    let Ok(awake) = Woken::asked();

    Mode::seen(&screens, awake)
}
