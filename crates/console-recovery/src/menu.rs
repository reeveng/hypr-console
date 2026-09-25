//! What a person can do from here, and nothing that needs the desktop to do it.
//!
//! Four choices, and each is one `systemctl`: start the desktop again, give
//! the screen to a terminal, restart, shut down. Starting something again and
//! giving the screen away both leave this program with nothing to do, so it
//! stops once systemd has said yes -- and not before, because a start that was
//! refused is a person who pressed a button and is still looking at the same
//! menu, and the menu should say so. Restarting and shutting down need no
//! stop: the machine is going.
//!
//! The words are Apple's, from the screen macOS shows when it cannot start.

use console_core_external_programs::Program as ExternalProgram;
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_walking::{Ring, Step};
use console_core_words::Words;
use console_program_contract::{Arguments, Command, Effect, Event, Exit, ExitStatus, Initial, Program, Update};

use console_input_event_devices::presses::ButtonPress;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Choice {
    #[words(title = "Start the Desktop Again")]
    TryAgain,
    #[words(title = "Terminal")]
    Terminal,
    #[words(title = "Restart")]
    Restart,
    #[words(title = "Shut Down")]
    ShutDown,
}

pub const CHOICES: [Choice; 4] = [Choice::TryAgain, Choice::Terminal, Choice::Restart, Choice::ShutDown];

pub const DESKTOP: &str = "display-manager.service";

pub const TERMINAL: &str = "getty@tty1.service";

impl Choice {
    fn arguments(self) -> Result<Vec<&'static str>, Never> {
        Ok(match self {
            Choice::TryAgain => vec!["--no-block", "start", DESKTOP],
            Choice::Terminal => vec!["--no-block", "start", TERMINAL],
            Choice::Restart => vec!["reboot"],
            Choice::ShutDown => vec!["poweroff"],
        })
    }

    fn leaves(self) -> Result<Leaving, Never> {
        Ok(match self {
            Choice::TryAgain | Choice::Terminal => Leaving::Stop,
            Choice::Restart | Choice::ShutDown => Leaving::MachineGoes,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Leaving {
    Stop,
    MachineGoes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Idle,
    Running(Choice),
    Failed(Choice),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Menu {
    pub at: u32,
    pub status: Status,
}

pub struct Recovery;

impl Program for Recovery {
    type State = Menu;
    type Event = ButtonPress;
    type Effect = Never;

    fn init(_arguments: &Arguments) -> Initial<Menu> {
        let Ok(opening) = Initial::new(Menu { at: 0, status: Status::Idle });

        opening
    }

    fn update(menu: &Menu, event: &Event<ButtonPress>) -> Update<Menu, Never> {
        let Ok(update) = match (menu.status, event) {
            (Status::Running(choice), Event::Replied(answer)) => answered(menu, choice, answer.status),
            (Status::Running(_), _) => Update::none(*menu),
            (Status::Idle | Status::Failed(_), Event::Custom(press)) => pressed(menu, *press),
            (Status::Idle | Status::Failed(_), _) => Update::none(*menu),
        };

        update
    }
}

fn pressed(menu: &Menu, press: ButtonPress) -> Result<Update<Menu, Never>, Never> {
    match press {
        ButtonPress::Up => moved(menu, Step::Back),
        ButtonPress::Down => moved(menu, Step::Forward),
        ButtonPress::Left | ButtonPress::Right | ButtonPress::Back => Update::none(*menu),
        ButtonPress::Choose => chose(menu),
    }
}

fn moved(menu: &Menu, step: Step) -> Result<Update<Menu, Never>, Never> {
    let Ok(ring) = Ring::of(&CHOICES);

    match ring {
        Some(ring) => {
            let Ok(at) = ring.stepped(menu.at, step);

            Update::none(Menu { at, status: Status::Idle })
        }
        None => Update::none(*menu),
    }
}

fn chose(menu: &Menu) -> Result<Update<Menu, Never>, Never> {
    let Ok(at) = index(menu.at);

    let choice = match CHOICES.get(at) {
        Some(choice) => *choice,
        None => return Update::none(*menu),
    };
    let Ok(arguments) = choice.arguments();
    let Ok(command) = Command::external(ExternalProgram::Systemctl, &arguments);

    Update::new(Menu { at: menu.at, status: Status::Running(choice) }, vec![Effect::Run(command)])
}

fn answered(menu: &Menu, choice: Choice, status: ExitStatus) -> Result<Update<Menu, Never>, Never> {
    let Ok(leaves) = choice.leaves();

    match (status, leaves) {
        (ExitStatus::Success, Leaving::Stop) => Update::new(*menu, vec![Effect::Stop(Exit::Success)]),
        (ExitStatus::Success, Leaving::MachineGoes) => Update::none(*menu),
        (ExitStatus::Failure(_), _) => Update::none(Menu { at: menu.at, status: Status::Failed(choice) }),
    }
}

const CLEARED: &str = "\x1b[2J\x1b[H\x1b[?25l";
const LIT: &str = "\x1b[7m";
const PLAIN: &str = "\x1b[0m";

pub fn screen(menu: &Menu) -> Result<String, Never> {
    let mut written = String::from(CLEARED);

    written.push_str("\n  The desktop did not start.\n\n");

    let Ok(lit) = index(menu.at);

    for (at, choice) in CHOICES.iter().enumerate() {
        let Ok(title) = choice.title();

        let line = match at == lit {
            true => format!("  {LIT} {title} {PLAIN}\n"),
            false => format!("   {title}\n"),
        };

        written.push_str(&line);
    }

    let Ok(message) = message(menu.status);

    written.push_str(&format!("\n  {message}\n\n  Up and down to move, A to choose.\n"));

    Ok(written)
}

fn message(status: Status) -> Result<String, Never> {
    Ok(match status {
        Status::Idle => String::new(),
        Status::Running(choice) => {
            let Ok(title) = choice.title();

            format!("{title}...")
        }
        Status::Failed(choice) => {
            let Ok(title) = choice.title();

            format!("{title} did not work. Try another.")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_program_contract::{Answer, run};

    fn arguments() -> Arguments {
        let Ok(arguments) = Arguments::of(&[]);

        arguments
    }

    fn systemctl(arguments: &[&str]) -> Command {
        let Ok(command) = Command::external(ExternalProgram::Systemctl, arguments);

        command
    }

    fn answer(arguments: &[&str], status: ExitStatus) -> Event<ButtonPress> {
        Event::Replied(Answer { command: systemctl(arguments), output: String::new(), status })
    }

    #[test]
    fn choosing_the_desktop_starts_it_and_stops_once_systemd_agrees() {
        let starts = ["--no-block", "start", DESKTOP];
        let Ok(trace) = run::<Recovery>(
            &arguments(),
            &[Event::Custom(ButtonPress::Choose), answer(&starts, ExitStatus::Success)],
        );
        let Ok(effects) = trace.effects();

        assert_eq!(effects, vec![Effect::Run(systemctl(&starts)), Effect::Stop(Exit::Success)]);
    }

    #[test]
    fn a_refused_start_stays_on_the_screen_and_says_so() {
        let starts = ["--no-block", "start", DESKTOP];
        let Ok(trace) = run::<Recovery>(
            &arguments(),
            &[Event::Custom(ButtonPress::Choose), answer(&starts, ExitStatus::Failure(Some(1)))],
        );
        let Ok(effects) = trace.effects();
        let Ok(shown) = screen(&trace.state);

        assert_eq!(effects, vec![Effect::Run(systemctl(&starts))]);
        assert!(shown.contains("did not work"), "{shown}");
    }

    #[test]
    fn up_from_the_top_is_the_bottom() {
        let Ok(trace) = run::<Recovery>(&arguments(), &[Event::Custom(ButtonPress::Up), Event::Custom(ButtonPress::Choose)]);
        let Ok(effects) = trace.effects();

        assert_eq!(effects, vec![Effect::Run(systemctl(&["poweroff"]))]);
    }

    #[test]
    fn a_press_while_something_is_underway_is_not_a_second_choice() {
        let Ok(trace) = run::<Recovery>(
            &arguments(),
            &[Event::Custom(ButtonPress::Down), Event::Custom(ButtonPress::Choose), Event::Custom(ButtonPress::Down), Event::Custom(ButtonPress::Choose)],
        );
        let Ok(effects) = trace.effects();

        assert_eq!(effects, vec![Effect::Run(systemctl(&["--no-block", "start", TERMINAL]))]);
    }

    #[test]
    fn the_screen_lights_the_choice_it_is_on() {
        let Ok(shown) = screen(&Menu { at: 2, status: Status::Idle });

        assert!(shown.contains(&format!("{LIT} Restart {PLAIN}")), "{shown}");
        assert!(shown.contains("   Shut Down\n"), "{shown}");
    }
}
