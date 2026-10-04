//! What the greeter decides: a press moves the cursor or draws the pattern, a
//! finger draws it across the dots, Login or a lifted finger hands the letters
//! to the login window, and what the window answers is what the screen says
//! next.
//!
//! While a pattern is being checked a press or a touch does nothing, so a
//! second Login cannot race the first. A refusal clears the pattern and keeps the cursor
//! where it was, because the hand that drew it is about to draw it again.
//!
//! The bar over the ring is a machine of its own, `console_status_bar::lock_screen`,
//! held beside the greeting and handed what is its: what the machine read, a tap
//! that landed on it, and the pad while the pad is on it. Up from a dot with
//! nothing above it is the pad going onto the bar, and a finger put down on the
//! ring takes away a row the bar had down.

use console_core_never::Never;
use console_input_event_devices::presses::ButtonPress;
use console_login_pattern::{Clicked, Direction, Pattern, Touch};
use console_login_window::protocol::{FromGreeter, ToGreeter};
use console_core_state_machine::{Machine, Queue, Transition};
use console_program_contract::{Effect, Event, Exit};
use console_status_bar::lock_screen::{LockScreenBar, LockScreenBarEffect, LockScreenBarEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Waiting,
    Fell,
    Checking,
    Failed(String),
    Message(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Greeting {
    pub pattern: Pattern,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GreeterState {
    pub greeting: Greeting,
    pub bar: LockScreenBar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GreeterEvent {
    Pressed(ButtonPress),
    Touched(Touch),
    Received(ToGreeter),
    Bar(LockScreenBarEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GreeterEffect {
    Send(FromGreeter),
    Bar(LockScreenBarEffect),
}

pub struct Greeter;

impl Machine for Greeter {
    type Input = Status;
    type State = GreeterState;
    type Request = Event<GreeterEvent>;
    type Effect = Effect<GreeterEffect>;

    fn initialize(status: &Status, _previous: Option<GreeterState>, _effects: &mut Effects) -> Result<GreeterState, Never> {
        let Ok(pattern) = Pattern::new();

        Ok(GreeterState { greeting: Greeting { pattern, status: status.clone() }, bar: LockScreenBar::default() })
    }

    fn handle(state: GreeterState, event: Event<GreeterEvent>, effects: &mut Effects) -> Result<GreeterState, Never> {
        let GreeterState { greeting, bar } = state;

        match (&greeting.status, event) {
            (_, Event::Custom(GreeterEvent::Received(received))) => {
                let Ok(greeting) = on_reply(greeting, received, effects);

                Ok(GreeterState { greeting, bar })
            }
            (_, Event::Custom(GreeterEvent::Bar(asked))) => barred(GreeterState { greeting, bar }, asked, effects),
            (Status::Checking, Event::Custom(GreeterEvent::Pressed(_) | GreeterEvent::Touched(_))) => Ok(GreeterState { greeting, bar }),
            (
                Status::Waiting | Status::Fell | Status::Failed(_) | Status::Message(_),
                Event::Custom(GreeterEvent::Pressed(press)),
            ) => on_pad(GreeterState { greeting, bar }, press, effects),
            (
                Status::Waiting | Status::Fell | Status::Failed(_) | Status::Message(_),
                Event::Custom(GreeterEvent::Touched(touch)),
            ) => {
                let Ok(state) = match touch {
                    Touch::Down(_) => barred(GreeterState { greeting, bar }, LockScreenBarEvent::Dismissed, effects),
                    Touch::Moved(_) | Touch::Up => Ok(GreeterState { greeting, bar }),
                };
                let Ok(clicked) = state.greeting.pattern.touch(touch);
                let Ok(greeting) = on_click(state.greeting, clicked, effects);

                Ok(GreeterState { greeting, ..state })
            }
            (_, Event::Opened | Event::Changed(_) | Event::Tick(..) | Event::Replied(_) | Event::Chosen(_) | Event::Stopping) => {
                Ok(GreeterState { greeting, bar })
            }
        }
    }
}

type Effects = Queue<Effect<GreeterEffect>>;

fn barred(state: GreeterState, asked: LockScreenBarEvent, effects: &mut Effects) -> Result<GreeterState, Never> {
    let Ok(Transition { state: bar, effects: wanted }) = LockScreenBar::transition(state.bar, asked);

    for effect in wanted {
        let Ok(()) = effects.offer(Effect::Custom(GreeterEffect::Bar(effect)));
    }

    Ok(GreeterState { bar, ..state })
}

fn on_pad(state: GreeterState, press: ButtonPress, effects: &mut Effects) -> Result<GreeterState, Never> {
    match (state.bar.focus, press) {
        (Some(_), _) => barred(state, LockScreenBarEvent::Pressed(press), effects),
        (None, ButtonPress::Up) => {
            let Ok(pattern) = state.greeting.pattern.moved(Direction::Up);

            match pattern.at == state.greeting.pattern.at {
                true => barred(state, LockScreenBarEvent::Entered, effects),
                false => Ok(GreeterState { greeting: Greeting { pattern, ..state.greeting }, ..state }),
            }
        }
        (None, ButtonPress::Down | ButtonPress::Left | ButtonPress::Right | ButtonPress::Back | ButtonPress::Choose) => {
            let Ok(greeting) = on_press(state.greeting, press, effects);

            Ok(GreeterState { greeting, ..state })
        }
    }
}

fn moved(greeting: Greeting, direction: Direction) -> Result<Greeting, Never> {
    let Ok(pattern) = greeting.pattern.moved(direction);

    Ok(Greeting { pattern, ..greeting })
}

fn on_press(greeting: Greeting, press: ButtonPress, effects: &mut Effects) -> Result<Greeting, Never> {
    match press {
        ButtonPress::Up => moved(greeting, Direction::Up),
        ButtonPress::Down => moved(greeting, Direction::Down),
        ButtonPress::Left => moved(greeting, Direction::Left),
        ButtonPress::Right => moved(greeting, Direction::Right),
        ButtonPress::Back => {
            let Ok(pattern) = greeting.pattern.undone();

            Ok(Greeting { pattern, ..greeting })
        }
        ButtonPress::Choose => {
            let Ok(clicked) = greeting.pattern.click();

            on_click(greeting, clicked, effects)
        }
    }
}

fn on_click(greeting: Greeting, clicked: Clicked, effects: &mut Effects) -> Result<Greeting, Never> {
    match clicked {
        Clicked::Traced(pattern) => Ok(Greeting { pattern, ..greeting }),
        Clicked::Submitted(secret) => {
            let Ok(spelled) = secret.as_str();
            let login = FromGreeter::Login(spelled.to_string());
            let Ok(pattern) = greeting.pattern.lifted();
            let Ok(()) = effects.offer(Effect::Custom(GreeterEffect::Send(login)));

            Ok(Greeting { pattern, status: Status::Checking })
        }
    }
}

fn on_reply(greeting: Greeting, received: ToGreeter, effects: &mut Effects) -> Result<Greeting, Never> {
    match received {
        ToGreeter::Welcome => {
            let Ok(()) = effects.offer(Effect::Stop(Exit::Success));

            Ok(greeting)
        }
        ToGreeter::Failed(why) => {
            let Ok(cleared) = greeting.pattern.cleared();

            Ok(Greeting { pattern: cleared, status: Status::Failed(why) })
        }
        ToGreeter::Message(text) => Ok(Greeting { status: Status::Message(text), ..greeting }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_login_pattern::{DOTS, Target};
    use console_core_state_machine::{run, run_from};
    use console_status_bar::lock_screen::{Menu, Power};
    use console_status_bar::showing::BarAction;

    fn press_events(presses: &[ButtonPress]) -> Result<Vec<Event<GreeterEvent>>, Never> {
        Ok(presses.iter().map(|press| Event::Custom(GreeterEvent::Pressed(*press))).collect())
    }

    #[test]
    fn a_pattern_ending_at_login_is_handed_to_the_window_as_its_letters() {
        let Ok(events) = press_events(&[
            ButtonPress::Choose,
            ButtonPress::Right,
            ButtonPress::Choose,
            ButtonPress::Down,
            ButtonPress::Down,
            ButtonPress::Down,
            ButtonPress::Down,
            ButtonPress::Choose,
        ]);
        let Ok(trace) = run::<Greeter>(&Status::Waiting, &events);
        let Ok(effects) = trace.effects();

        assert_eq!(trace.state.greeting.status, Status::Checking);
        assert_eq!(effects, vec![Effect::Custom(GreeterEffect::Send(FromGreeter::Login(String::from("ab"))))]);
    }

    #[test]
    fn a_finger_drawn_over_the_dots_and_lifted_is_handed_to_the_window() -> Result<(), &'static str> {
        let over = |key: char| DOTS.iter().find(|dot| dot.key == key).map(|dot| dot.centre);
        let a = over('a').ok_or("no dot for a")?;
        let e = over('e').ok_or("no dot for e")?;
        let c = over('c').ok_or("no dot for c")?;
        let touches = [Touch::Down(a), Touch::Moved(e), Touch::Moved(c), Touch::Up];
        let events: Vec<Event<GreeterEvent>> = touches.iter().map(|touch| Event::Custom(GreeterEvent::Touched(*touch))).collect();
        let Ok(trace) = run::<Greeter>(&Status::Waiting, &events);
        let Ok(effects) = trace.effects();

        assert_eq!(trace.state.greeting.status, Status::Checking);
        assert_eq!(trace.state.greeting.pattern.finger, None);
        assert_eq!(effects, vec![Effect::Custom(GreeterEffect::Send(FromGreeter::Login(String::from("aec"))))]);

        Ok(())
    }

    #[test]
    fn a_press_while_checking_does_nothing() {
        let Ok(start) = Pattern::new();
        let checking = GreeterState { greeting: Greeting { pattern: start.clone(), status: Status::Checking }, bar: LockScreenBar::default() };
        let Ok(events) = press_events(&[ButtonPress::Up, ButtonPress::Choose]);
        let Ok(trace) = run_from::<Greeter>(checking.clone(), &events);

        assert_eq!(trace.state, checking);
    }

    #[test]
    fn a_refusal_clears_the_pattern_and_keeps_the_cursor() {
        let drawn = GreeterState {
            greeting: Greeting { pattern: Pattern { at: Target::Login, path: vec![1, 2], finger: None }, status: Status::Checking },
            bar: LockScreenBar::default(),
        };
        let events = [Event::Custom(GreeterEvent::Received(ToGreeter::Failed(String::from("no"))))];
        let Ok(trace) = run_from::<Greeter>(drawn, &events);

        assert_eq!(trace.state.greeting.pattern, Pattern { at: Target::Login, path: Vec::new(), finger: None });
        assert_eq!(trace.state.greeting.status, Status::Failed(String::from("no")));
    }

    #[test]
    fn a_welcome_stops_the_greeter() {
        let Ok(start) = Pattern::new();
        let checking = GreeterState { greeting: Greeting { pattern: start, status: Status::Checking }, bar: LockScreenBar::default() };
        let Ok(trace) = run_from::<Greeter>(checking, &[Event::Custom(GreeterEvent::Received(ToGreeter::Welcome))]);
        let Ok(effects) = trace.effects();

        assert_eq!(effects, vec![Effect::Stop(Exit::Success)]);
    }

    #[test]
    fn a_desktop_that_fell_is_said_on_the_first_screen() {
        let Ok(trace) = run::<Greeter>(&Status::Fell, &[]);

        assert_eq!(trace.state.greeting.status, Status::Fell);
    }

    fn at_the_top() -> Result<GreeterState, Never> {
        let Ok(trace) = run::<Greeter>(&Status::Waiting, &[]);

        Ok(trace.state)
    }

    #[test]
    fn up_from_the_top_of_the_ring_is_the_bar_and_the_ring_keeps_its_place() {
        let Ok(state) = at_the_top();
        let Ok(events) = press_events(&[ButtonPress::Up, ButtonPress::Right]);
        let Ok(trace) = run_from::<Greeter>(state.clone(), &events);

        assert_eq!(trace.state.bar.focus, Some(BarAction::Menu(Menu::Power)));
        assert_eq!(trace.state.greeting, state.greeting, "a press on the bar moved the ring as well");
    }

    #[test]
    fn up_from_lower_on_the_ring_is_still_the_ring() {
        let Ok(state) = at_the_top();
        let Ok(events) = press_events(&[ButtonPress::Down, ButtonPress::Down, ButtonPress::Up]);
        let Ok(trace) = run_from::<Greeter>(state, &events);

        assert_eq!(trace.state.bar.focus, None);
        assert_ne!(trace.state.greeting.pattern.at, Target::Login);
    }

    #[test]
    fn b_on_the_bar_hands_the_pad_back_to_the_ring_where_it_left_it() {
        let Ok(state) = at_the_top();
        let Ok(events) = press_events(&[ButtonPress::Up, ButtonPress::Back, ButtonPress::Choose]);
        let Ok(trace) = run_from::<Greeter>(state.clone(), &events);

        assert_eq!(trace.state.bar.focus, None);
        assert_eq!(trace.state.greeting.pattern.path, vec![0], "A after B did not join the dot the pad was on");
    }

    #[test]
    fn shut_down_chosen_with_the_pad_is_asked_for_and_nothing_is_sent_to_the_window() {
        let Ok(state) = at_the_top();
        let Ok(events) = press_events(&[ButtonPress::Up, ButtonPress::Right, ButtonPress::Choose, ButtonPress::Right, ButtonPress::Right, ButtonPress::Choose]);
        let Ok(trace) = run_from::<Greeter>(state, &events);
        let Ok(effects) = trace.effects();

        assert_eq!(effects, vec![Effect::Custom(GreeterEffect::Bar(LockScreenBarEffect::Power(Power::ShutDown)))]);
    }

    #[test]
    fn a_finger_put_down_on_the_ring_takes_a_row_away_and_still_draws() -> Result<(), &'static str> {
        let Ok(state) = at_the_top();
        let a = DOTS.first().map(|dot| dot.centre).ok_or("no dots")?;
        let events = [
            Event::Custom(GreeterEvent::Bar(LockScreenBarEvent::Tapped(BarAction::Menu(Menu::Power)))),
            Event::Custom(GreeterEvent::Touched(Touch::Down(a))),
        ];
        let Ok(trace) = run_from::<Greeter>(state, &events);

        assert_eq!(trace.state.bar.menu, None);
        assert_eq!(trace.state.greeting.pattern.path, vec![0]);

        Ok(())
    }
}
