//! What the notifications panel is looking at, and what a press does about it.
//!
//! The panel used to hold this in an actor whose whole job was to be reachable
//! from every closure that draws a row. What it holds is one of two things, so
//! the actor was the shape rather than the state, and it is here as a
//! `console_program_contract::Program`: a press goes in, a place and a list of
//! effects comes out, and none of it needs a daemon or a screen.
//!
//! Dismissing goes back to the list before the row it dismissed has gone. The
//! daemon answers the call before it writes the file the panel reads, so a
//! panel that stayed on the row would be drawing a notification no one can act on.
//!
//! Both presses are a call on the bus, which is what `makoctl` was doing in
//! mako's words and is now `busctl` in ours. The arguments is built by
//! `crate::serving`, which is the one place the interface is spelled: a panel
//! that wrote out the name, the path and the member itself would be a second
//! opinion about what this desktop's own daemon is called.

use console_core_external_programs::Program as ExternalProgram;
use console_core_never::Never;
use console_program_contract::{Arguments, Effect, Initial, Program, Command, Update, Event};

pub const UP: u32 = 0;

pub const DEEPER: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    List,
    One(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationsEvent {
    Chosen(u32),
    Back,
    Dismissed(u32),
    ClearAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationsEffect {
    Replace(u32),
    Refresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closes {
    Yes,
    No,
}

pub struct Notifications;

impl Program for Notifications {
    type State = Destination;
    type Event = NotificationsEvent;
    type Effect = NotificationsEffect;

    fn init(_argv: &Arguments) -> Initial<Destination> {
        let Ok(opening) = Initial::new(Destination::List);

        opening
    }

    fn update(state: &Destination, event: &Event<NotificationsEvent>) -> Update<Destination, NotificationsEffect> {
        let Ok(turn) = match event {
            Event::Custom(NotificationsEvent::Chosen(id)) => {
                Update::new(Destination::One(*id), vec![Effect::Custom(NotificationsEffect::Replace(DEEPER))])
            }

            Event::Custom(NotificationsEvent::Back) => match state {
                Destination::One(_) => Update::new(Destination::List, vec![Effect::Custom(NotificationsEffect::Replace(UP))]),
                Destination::List => Update::none(*state),
            },

            Event::Custom(NotificationsEvent::Dismissed(id)) => {
                let Ok(dismiss) = closing(*id);

                Update::new(
                    Destination::List,
                    vec![Effect::Run(dismiss), Effect::Custom(NotificationsEffect::Replace(UP))],
                )
            }

            Event::Custom(NotificationsEvent::ClearAll) => {
                let Ok(clear) = clearing();

                Update::new(Destination::List, vec![Effect::Run(clear), Effect::Custom(NotificationsEffect::Refresh)])
            }

            Event::Opened | Event::Changed(_) | Event::Tick(_, _) | Event::Replied(_)
            | Event::Chosen(_) | Event::Stopping => Update::none(*state),
        };

        turn
    }
}

fn closing(id: u32) -> Result<Command, Never> {
    let Ok(arguments) = crate::serving::closing_one(id);
    let said: Vec<&str> = arguments.iter().map(String::as_str).collect();

    Command::external(ExternalProgram::Busctl, &said)
}

fn clearing() -> Result<Command, Never> {
    let Ok(arguments) = crate::serving::asking("ClearAll");
    let said: Vec<&str> = arguments.iter().map(String::as_str).collect();

    Command::external(ExternalProgram::Busctl, &said)
}

pub fn closes(state: &Destination) -> Result<Closes, Never> {
    Ok(match state {
        Destination::List => Closes::Yes,
        Destination::One(_) => Closes::No,
    })
}

#[cfg(test)]
mod tests {
    use console_program_contract::run;

    use super::*;

    fn dismissing(id: u32) -> Effect<NotificationsEffect> {
        let Ok(runs) = closing(id);

        Effect::Run(runs)
    }

    #[test]
    fn a_row_opens_onto_the_notification_it_names() {
        let Ok(said) = run::<Notifications>(&Arguments::default(), &[Event::Custom(NotificationsEvent::Chosen(7))]);

        assert_eq!(said.state, Destination::One(7));
        assert_eq!(said.on(0), Ok(Some([Effect::Custom(NotificationsEffect::Replace(DEEPER))].as_slice())));
    }

    #[test]
    fn back_out_of_a_notification_is_the_list_and_back_out_of_the_list_is_the_way_out() {
        let Ok(one) = run::<Notifications>(
            &Arguments::default(),
            &[Event::Custom(NotificationsEvent::Chosen(7)), Event::Custom(NotificationsEvent::Back)],
        );

        assert_eq!(one.state, Destination::List);
        assert_eq!(one.on(1), Ok(Some([Effect::Custom(NotificationsEffect::Replace(UP))].as_slice())));
        assert_eq!(closes(&one.state), Ok(Closes::Yes));
        assert_eq!(closes(&Destination::One(7)), Ok(Closes::No));
    }

    #[test]
    fn dismissing_one_asks_the_daemon_and_comes_back_to_the_list() {
        let Ok(said) = run::<Notifications>(
            &Arguments::default(),
            &[Event::Custom(NotificationsEvent::Chosen(7)), Event::Custom(NotificationsEvent::Dismissed(7))],
        );

        assert_eq!(said.state, Destination::List);
        assert_eq!(
            said.on(1),
            Ok(Some([dismissing(7), Effect::Custom(NotificationsEffect::Replace(UP))].as_slice()))
        );
    }

    #[test]
    fn clearing_them_all_stays_where_it_is_and_draws_again() {
        let Ok(said) = run::<Notifications>(&Arguments::default(), &[Event::Custom(NotificationsEvent::ClearAll)]);
        let Ok(clear) = clearing();

        assert_eq!(
            said.on(0),
            Ok(Some([Effect::Run(clear), Effect::Custom(NotificationsEffect::Refresh)].as_slice()))
        );
    }
}
