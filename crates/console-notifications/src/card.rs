//! What the desktop has said, drawn.
//!
//! ```text
//!     notifications-panel
//!     notifications-panel Earlier
//! ```
//!
//! The bell on the right of the bar opens it, and tapping the bell again puts
//! it away, which is how every other icon along there works.
//!
//! What is here is the reading of the file the daemon writes, and the drawing.
//! Both tabs come out of one reading, which is the point of it being a file:
//! the panel used to ask mako twice, once for what was waiting and once for
//! the history, and the two answers were two moments -- a tab opened between
//! them showed a notification in neither list or in both. What each tab holds
//! once it has been read is `crate::rows`; where a press
//! leaves you is `crate::notifications`, which is a
//! `console_program_contract::Program` and holds the whole of what this panel
//! decides. The actor is what makes that state reachable from a closure on
//! GTK's thread; it steps by asking `update`, and the effects are carried out in
//! the callback that has the surface in its hand.

use std::sync::Arc;

use crate::notifications::{Closes, NotificationsEvent, NotificationsEffect, Notifications, Destination, closes};
use crate::reading::Notification;
use crate::serving;
use crate::rows::{Chosen, earlier_rows, cleared_rows, one_rows, tab, waiting_rows};
use console_panel::actor::{self, Address, Answer};
use console_panel::card::{Card, Door};
use console_panel::page::{Handler, Page, Row, Rows, Showing};
use console_panel::running::said;
use console_core_never::Never;
use console_program_contract::{Arguments, Effect, Executable, Program as _, Topic, Update, Event};


fn waiting() -> Result<Vec<Notification>, Never> {
    let Ok(held) = serving::held();

    Ok(held.waiting)
}

fn earlier() -> Result<Vec<Notification>, Never> {
    let Ok(held) = serving::held();

    Ok(held.earlier)
}

struct Looking {
    onto: Destination,
}

enum Message {
    Event(NotificationsEvent, Answer<Vec<Effect<NotificationsEffect>>>),
    At(Answer<Destination>),
}

impl actor::Machine for Looking {
    type Message = Message;

    fn step(self, message: Message) -> Self {
        match message {
            Message::Event(heard, answer) => {
                let Update { state, effects } = Notifications::update(&self.onto, &Event::Custom(heard));
                let _ = answer.say(effects);

                Looking { onto: state }
            }
            Message::At(answer) => {
                let _ = answer.say(self.onto);

                self
            },
        }
    }
}

type ActorAddress = Address<Message>;

fn looking_at(held: &ActorAddress) -> Result<Destination, Never> {
    Ok(match held.ask(Message::At) {
        Ok(onto) => onto,
        Err(_) => Destination::List,
    })
}

fn press(held: &ActorAddress, heard: NotificationsEvent, showing: &dyn Showing) -> Result<(), Never> {
    let effects = match held.ask(|answer| Message::Event(heard, answer)) {
        Ok(effects) => effects,
        Err(_) => {
            eprintln!("notifications-panel: the panel's own state is missing, so the press did nothing");

            Vec::new()
        }
    };

    for effect in effects {
        let Ok(()) = carry(&effect, showing);
    }

    Ok(())
}

fn carry(effect: &Effect<NotificationsEffect>, showing: &dyn Showing) -> Result<(), Never> {
    match effect {
        Effect::Run(runs) => {
            let arguments: Vec<&str> = runs.arguments.iter().map(String::as_str).collect();

            match runs.program {
                Executable::External(program) => {
                    let _ = said(program, &arguments);
                }
                Executable::Internal(name) => {
                    let mut whole = vec![name.to_string()];

                    whole.extend(runs.arguments.clone());
                    showing.later(whole);
                }
            }
        }
        Effect::Custom(NotificationsEffect::Replace(row)) => {
            showing.replace(*row)
        }
        Effect::Custom(NotificationsEffect::Refresh) => showing.refresh(),
        Effect::Stream(_)
        | Effect::Prompt(_)
        | Effect::Spawn(_)
        | Effect::Subscribe(_)
        | Effect::Unsubscribe(_)
        | Effect::Write(_)
        | Effect::Notify(_)
        | Effect::Print(_)
        | Effect::Stop(_) => {},
    }

    Ok(())
}

fn back_up(held: &ActorAddress) -> Result<Chosen, Never> {
    let held = held.clone();

    Ok(Arc::new(move |showing: &dyn Showing| {
        let Ok(()) = press(&held, NotificationsEvent::Back, showing);
    }))
}

fn open(held: &ActorAddress, id: u32) -> Result<Handler, Never> {
    let held = held.clone();

    Handler::and_stay(move |showing| {
        let Ok(()) = press(&held, NotificationsEvent::Chosen(id), showing);
    })
}

fn dismiss(held: &ActorAddress, id: u32) -> Result<Handler, Never> {
    let held = held.clone();

    Handler::and_stay(move |showing| {
        let Ok(()) = press(&held, NotificationsEvent::Dismissed(id), showing);
    })
}

fn clear(held: &ActorAddress) -> Result<Handler, Never> {
    let held = held.clone();

    Handler::and_stay(move |showing| {
        let Ok(()) = press(&held, NotificationsEvent::ClearAll, showing);
    })
}

fn waiting_tab(looking: &ActorAddress) -> Result<Vec<Row>, Never> {
    let Ok(held) = waiting();
    let Ok(onto) = looking_at(looking);

    match onto {
        Destination::List => {
            let Ok(clears) = clear(looking);

            waiting_rows(
                &held,
                |notification| {
                    let Ok(does) = open(looking, notification.id);

                    does
                },
                clears,
            )
        }
        Destination::One(id) => match held.iter().find(|notification| notification.id == id) {
            Some(notification) => {
                let Ok(back) = back_up(looking);
                let Ok(dismisses) = dismiss(looking, id);

                one_rows(notification, &back, dismisses)
            }
            None => {
                let Ok(back) = back_up(looking);

                cleared_rows(&back)
            }
        },
    }
}

fn earlier_tab() -> Result<Vec<Row>, Never> {
    let Ok(held) = earlier();

    earlier_rows(&held)
}

fn pages(looking: &ActorAddress) -> Result<Vec<Page>, Never> {
    let drawing = looking.clone();
    let backing = looking.clone();
    let Ok(asked) = Rows::asked(move || {
        let Ok(rows) = waiting_tab(&drawing);

        rows
    });
    let Ok(first) = tab(0);
    let Ok(waiting) = Page::new(first, asked);
    let Ok(watching) = waiting.listening(Topic::Notifications, console_events::again::notifications);
    let Ok(waiting) = watching.on_back(move |showing| {
        let Ok(onto) = looking_at(&backing);
        let Ok(()) = press(&backing, NotificationsEvent::Back, showing);

        let Ok(closes) = closes(&onto);

        match closes {
            Closes::Yes => true,
            Closes::No => false,
        }
    });
    let Ok(read) = Rows::asked(|| {
        let Ok(rows) = earlier_tab();

        rows
    });
    let Ok(second) = tab(1);
    let Ok(cleared) = Page::new(second, read);

    Ok(vec![waiting, cleared])
}


pub const WHO: &str = "notifications-panel";

const UNDER: i32 = 250;

pub fn door(arguments: &[String]) -> Result<Door, Never> {
    Door::closing_at("notifications", arguments.first().map(String::as_str))
}

pub fn card(arguments: &[String]) -> Result<Card, Never> {
    let tab = arguments.first().cloned();
    let Ok(opened) = Arguments::of(&tab.as_deref().into_iter().collect::<Vec<&str>>());

    let init = Notifications::init(&opened);
    let Ok(looking) = actor::supervise(move || Looking { onto: init.state });
    let held = looking.addr.clone();

    let Ok(card) = Card::new(Arc::new(move || {
        let Ok(pages) = pages(&held);

        pages
    }));
    let Ok(card) = card.under(UNDER);
    let Ok(card) = card.opening_at(tab.as_deref());

    card.shutting(Box::new(move || looking.shutdown()))
}
