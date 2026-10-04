//! What a person can do from here, and nothing that needs the desktop to do it.
//!
//! Start the desktop again, give the screen to a terminal, restart, shut down:
//! each is one `systemctl`. Starting something again and giving the screen
//! away both leave this program with nothing to do, so it stops once systemd
//! has said yes -- and not before, because a start that was refused is a
//! person who pressed a button and is still looking at the same menu, and the
//! menu should say so. Restarting and shutting down need no stop: the machine
//! is going.
//!
//! **Restarting from a snapshot is here because this is where the pad still
//! works.** Limine's menu holds the snapshots and nothing on the pad reaches
//! it, so the choice is made on this screen and Limine is only told the
//! answer: `bootctl set-oneshot` names the entry the next boot takes, and the
//! machine restarts into it. The list is `limine.conf` read when the choice is
//! made, because the identifiers Limine published at boot are its menu as it
//! stood then. A one-shot that was refused is not followed by the restart,
//! which would only be the machine that did not start, started again. Limine
//! remembers the entry it last booted, so the boot after the snapshot's is the
//! snapshot again until somebody chooses otherwise.
//!
//! A snapshot no apply took asks first. The installer's is in the same list as
//! last week's, and it starts a machine with none of this desktop on it, which
//! Limine would then keep starting. The question opens on Cancel, so a press
//! that landed twice cancels rather than agrees.
//!
//! What is on the screen is asked for rather than drawn beside the decision:
//! a turn that changed the menu, and the first turn of all, ends with
//! [`Screen::Show`], so a transcript says what a person was looking at when
//! they pressed and the loop that draws it is the runtime's rather than a
//! second one written here.
//!
//! The words are Apple's, from the screen macOS shows when it cannot start.

use console_boot_entries::{CONFIG, Snapshot, TakenBy};
use console_core_external_programs::Program as ExternalProgram;
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_walking::{Ring, Step, where_it_is};
use console_core_words::Words;
use console_core_state_machine::{Machine, Queue, Transition};
use console_program_contract::{Answer, Command, Effect, Event, Exit, ExitStatus};

use console_input_event_devices::presses::ButtonPress;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Choice {
    #[words(title = "Start the Desktop Again")]
    TryAgain,
    #[words(title = "Restart from a Snapshot")]
    Snapshots,
    #[words(title = "Terminal")]
    Terminal,
    #[words(title = "Restart")]
    Restart,
    #[words(title = "Shut Down")]
    ShutDown,
}

pub const CHOICES: [Choice; 5] = [Choice::TryAgain, Choice::Snapshots, Choice::Terminal, Choice::Restart, Choice::ShutDown];

pub const DESKTOP: &str = "display-manager.service";

pub const TERMINAL: &str = "getty@tty1.service";

impl Choice {
    fn command(self) -> Result<Command, Never> {
        match self {
            Choice::TryAgain => Command::external(ExternalProgram::Systemctl, &["--no-block", "start", DESKTOP]),
            Choice::Snapshots => Command::external(ExternalProgram::Cat, &[CONFIG]),
            Choice::Terminal => Command::external(ExternalProgram::Systemctl, &["--no-block", "start", TERMINAL]),
            Choice::Restart => Command::external(ExternalProgram::Systemctl, &["reboot"]),
            Choice::ShutDown => Command::external(ExternalProgram::Systemctl, &["poweroff"]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
pub enum Confirmation {
    #[words(title = "Cancel")]
    Cancel,
    #[words(title = "Restart")]
    Restart,
}

pub const CONFIRMATIONS: [Confirmation; 2] = [Confirmation::Cancel, Confirmation::Restart];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Chose(Choice),
    OneShot(u32),
}

impl Action {
    fn title(self) -> Result<String, Never> {
        Ok(match self {
            Action::Chose(choice) => {
                let Ok(title) = choice.title();

                title.to_string()
            }
            Action::OneShot(number) => format!("Restart from Snapshot {number}"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Idle,
    Running(Action),
    Failed(Action),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page {
    Choices,
    Snapshots(Vec<Snapshot>),
    Confirm { snapshots: Vec<Snapshot>, chosen: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu {
    pub page: Page,
    pub at: u32,
    pub status: Status,
}

pub struct Recovery;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Show(Menu),
}

impl Machine for Recovery {
    type Input = ();
    type State = Menu;
    type Request = Event<ButtonPress>;
    type Effect = Effect<Screen>;

    fn initialize(_input: &(), _previous: Option<Menu>, effects: &mut Effects) -> Result<Menu, Never> {
        let Ok(opening) = initial();

        opening.offered(effects)
    }

    fn handle(state: Menu, event: Event<ButtonPress>, effects: &mut Effects) -> Result<Menu, Never> {
        let Ok(decided) = decide(&state, &event);

        decided.offered(effects)
    }
}

type Effects = Queue<Effect<Screen>>;

type Turn = Transition<Menu, Effect<Screen>>;

fn initial() -> Result<Turn, Never> {
    let Ok(opening) = Transition::without_effects(Menu { page: Page::Choices, at: 0, status: Status::Idle });

    Ok(opening)
}

fn decide(menu: &Menu, event: &Event<ButtonPress>) -> Result<Turn, Never> {
    let Ok(Transition { state, effects }) = match (menu.status, event) {
        (Status::Running(action), Event::Replied(answer)) => replied(menu, action, answer),
        (Status::Running(_), _) => Transition::without_effects(menu.clone()),
        (Status::Idle | Status::Failed(_), Event::Custom(press)) => on_press(menu, *press),
        (Status::Idle | Status::Failed(_), _) => Transition::without_effects(menu.clone()),
    };
    let Ok(showing) = showing(menu, &state, event);

    let effects = match showing {
        Showing::Again => [vec![Effect::Custom(Screen::Show(state.clone()))], effects].concat(),
        Showing::AsItWas => effects,
    };

    let Ok(update) = Transition::new(state, effects);

    Ok(update)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Showing {
    Again,
    AsItWas,
}

fn showing(before: &Menu, after: &Menu, event: &Event<ButtonPress>) -> Result<Showing, Never> {
    Ok(match before == after {
        false => Showing::Again,
        true => match event {
            Event::Opened => Showing::Again,
            Event::Custom(_)
            | Event::Tick(_, _)
            | Event::Changed(_)
            | Event::Replied(_)
            | Event::Chosen(_)
            | Event::Stopping => Showing::AsItWas,
        },
    })
}

fn on_press(menu: &Menu, press: ButtonPress) -> Result<Turn, Never> {
    match press {
        ButtonPress::Up => step(menu, Step::Back),
        ButtonPress::Down => step(menu, Step::Forward),
        ButtonPress::Back => back(menu),
        ButtonPress::Left | ButtonPress::Right => Transition::without_effects(menu.clone()),
        ButtonPress::Choose => chose(menu),
    }
}

fn step(menu: &Menu, step: Step) -> Result<Turn, Never> {
    let Ok(ring) = match &menu.page {
        Page::Choices => Ring::of(&CHOICES),
        Page::Snapshots(snapshots) => Ring::of(snapshots),
        Page::Confirm { .. } => Ring::of(&CONFIRMATIONS),
    };

    match ring {
        Some(ring) => {
            let Ok(at) = ring.step(menu.at, step);

            Transition::without_effects(Menu { page: menu.page.clone(), at, status: Status::Idle })
        }
        None => Transition::without_effects(menu.clone()),
    }
}

fn back(menu: &Menu) -> Result<Turn, Never> {
    match &menu.page {
        Page::Choices => Transition::without_effects(menu.clone()),
        Page::Snapshots(_) => {
            let Ok(found) = where_it_is(&CHOICES, &Choice::Snapshots);

            let at = match found {
                Some(at) => at,
                None => 0,
            };

            Transition::without_effects(Menu { page: Page::Choices, at, status: Status::Idle })
        }
        Page::Confirm { snapshots, chosen } => {
            Transition::without_effects(Menu { page: Page::Snapshots(snapshots.clone()), at: *chosen, status: Status::Idle })
        }
    }
}

fn chose(menu: &Menu) -> Result<Turn, Never> {
    let Ok(at) = index(menu.at);

    match &menu.page {
        Page::Choices => match CHOICES.get(at) {
            Some(choice) => {
                let Ok(command) = choice.command();

                running(menu, Action::Chose(*choice), command)
            }
            None => Transition::without_effects(menu.clone()),
        },
        Page::Snapshots(snapshots) => match snapshots.get(at) {
            Some(snapshot) => {
                let Ok(taken_by) = snapshot.taken_by();

                match taken_by {
                    TakenBy::Apply => one_shot(menu, snapshot),
                    TakenBy::SomethingElse => {
                        let asking = Page::Confirm { snapshots: snapshots.clone(), chosen: menu.at };

                        Transition::without_effects(Menu { page: asking, at: 0, status: Status::Idle })
                    }
                }
            }
            None => Transition::without_effects(menu.clone()),
        },
        Page::Confirm { snapshots, chosen } => match CONFIRMATIONS.get(at) {
            Some(Confirmation::Cancel) => back(menu),
            Some(Confirmation::Restart) => {
                let Ok(which) = index(*chosen);

                match snapshots.get(which) {
                    Some(snapshot) => one_shot(menu, snapshot),
                    None => Transition::without_effects(menu.clone()),
                }
            }
            None => Transition::without_effects(menu.clone()),
        },
    }
}

fn one_shot(menu: &Menu, snapshot: &Snapshot) -> Result<Turn, Never> {
    let Ok(command) = Command::external(ExternalProgram::Bootctl, &["set-oneshot", &snapshot.identifier]);

    running(menu, Action::OneShot(snapshot.number), command)
}

fn running(menu: &Menu, action: Action, command: Command) -> Result<Turn, Never> {
    Transition::new(Menu { page: menu.page.clone(), at: menu.at, status: Status::Running(action) }, vec![Effect::Run(command)])
}

fn replied(menu: &Menu, action: Action, answer: &Answer) -> Result<Turn, Never> {
    match (action, answer.status) {
        (Action::Chose(Choice::TryAgain | Choice::Terminal), ExitStatus::Success) => Transition::new(menu.clone(), vec![Effect::Stop(Exit::Success)]),
        (Action::Chose(Choice::Restart | Choice::ShutDown), ExitStatus::Success) => Transition::without_effects(menu.clone()),
        (Action::Chose(Choice::Snapshots), ExitStatus::Success) => listed(&answer.output),
        (Action::OneShot(_), ExitStatus::Success) => {
            let Ok(command) = Choice::Restart.command();

            running(menu, Action::Chose(Choice::Restart), command)
        }
        (action, ExitStatus::Failure(_)) => {
            Transition::without_effects(Menu { page: menu.page.clone(), at: menu.at, status: Status::Failed(action) })
        }
    }
}

fn listed(config: &str) -> Result<Turn, Never> {
    let Ok(entries) = console_boot_entries::entries(config);
    let Ok(snapshots) = console_boot_entries::snapshots(&entries);

    Transition::without_effects(Menu { page: Page::Snapshots(snapshots), at: 0, status: Status::Idle })
}

const CLEARED: &str = "\x1b[2J\x1b[H\x1b[?25l";
const LIT: &str = "\x1b[7m";
const PLAIN: &str = "\x1b[0m";

struct Layout {
    heading: Vec<String>,
    rows: Vec<String>,
    hint: &'static str,
}

fn titles(every: impl Iterator<Item = Result<&'static str, Never>>) -> Result<Vec<String>, Never> {
    Ok(every
        .map(|said| {
            let Ok(title) = said;

            title.to_string()
        })
        .collect())
}

fn described(snapshot: &Snapshot) -> Result<String, Never> {
    Ok(format!("{}  {}", snapshot.taken, snapshot.comments.join("  ")))
}

fn laid_out(page: &Page) -> Result<Layout, Never> {
    Ok(match page {
        Page::Choices => {
            let Ok(rows) = titles(CHOICES.iter().map(|choice| choice.title()));

            Layout { heading: vec!["The desktop did not start.".to_string()], rows, hint: "Up and down to move, A to choose." }
        }
        Page::Snapshots(snapshots) => Layout {
            heading: vec![match snapshots.is_empty() {
                true => "There are no snapshots.".to_string(),
                false => "Choose a snapshot to restart from.".to_string(),
            }],
            rows: snapshots
                .iter()
                .map(|snapshot| {
                    let Ok(said) = described(snapshot);

                    format!("{:>4}  {said}", snapshot.number)
                })
                .collect(),
            hint: "Up and down to move, A to choose, B to go back.",
        },
        Page::Confirm { snapshots, chosen } => {
            let Ok(rows) = titles(CONFIRMATIONS.iter().map(|answer| answer.title()));
            let Ok(which) = index(*chosen);

            let heading = match snapshots.get(which) {
                Some(snapshot) => {
                    let Ok(said) = described(snapshot);

                    vec![
                        format!("Restart from snapshot {}?", snapshot.number),
                        String::new(),
                        said,
                        "It was not taken by an apply, so this desktop may not be on it.".to_string(),
                    ]
                }
                None => vec!["Restart from this snapshot?".to_string()],
            };

            Layout { heading, rows, hint: "Up and down to move, A to choose, B to cancel." }
        }
    })
}

pub fn screen(menu: &Menu) -> Result<String, Never> {
    let Ok(Layout { heading, rows, hint }) = laid_out(&menu.page);
    let Ok(lit) = index(menu.at);
    let mut written = format!("{CLEARED}\n");

    for line in &heading {
        written.push_str(&format!("  {line}\n"));
    }

    written.push('\n');

    for (at, row) in rows.iter().enumerate() {
        let line = match at == lit {
            true => format!("  {LIT} {row} {PLAIN}\n"),
            false => format!("   {row}\n"),
        };

        written.push_str(&line);
    }

    let Ok(message) = message(menu.status);

    written.push_str(&format!("\n  {message}\n\n  {hint}\n"));

    Ok(written)
}

fn message(status: Status) -> Result<String, Never> {
    Ok(match status {
        Status::Idle => String::new(),
        Status::Running(action) => {
            let Ok(title) = action.title();

            format!("{title}...")
        }
        Status::Failed(action) => {
            let Ok(title) = action.title();

            format!("{title} did not work. Try another.")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_core_state_machine::run;

    const LIMINE: &str = "/+CachyOS
//linux-cachyos-deckify
protocol: linux
//Snapshots
///184 \u{2502} 2026-09-23 06:54:01
comment: console apply e79c18f5
////linux-cachyos-deckify
protocol: linux
///6   \u{2502} 2026-08-27 14:47:13
comment: Fresh CachyOS Installation
////linux-cachyos-deckify
protocol: linux
";

    const NEWEST: &str = "CachyOS.Snapshots.184-----2026-09-23-06-54-01.linux-cachyos-deckify";

    fn systemctl(arguments: &[&str]) -> Result<Command, Never> {
        Command::external(ExternalProgram::Systemctl, arguments)
    }

    fn reply(command: Command, output: &str, status: ExitStatus) -> Result<Event<ButtonPress>, Never> {
        Ok(Event::Replied(Answer { command, output: output.to_string(), status }))
    }

    fn answer(arguments: &[&str], status: ExitStatus) -> Result<Event<ButtonPress>, Never> {
        let Ok(command) = systemctl(arguments);

        reply(command, "", status)
    }

    fn read_limine() -> Result<Command, Never> {
        Command::external(ExternalProgram::Cat, &[CONFIG])
    }

    fn one_shot(identifier: &str) -> Result<Command, Never> {
        Command::external(ExternalProgram::Bootctl, &["set-oneshot", identifier])
    }

    fn opened_snapshots() -> Result<Vec<Event<ButtonPress>>, Never> {
        let Ok(command) = read_limine();
        let Ok(listed) = reply(command, LIMINE, ExitStatus::Success);

        Ok(vec![Event::Custom(ButtonPress::Down), Event::Custom(ButtonPress::Choose), listed])
    }

    fn done(trace: &console_core_state_machine::Trace<Menu, Event<ButtonPress>, Effect<Screen>>) -> Result<Vec<Effect<Screen>>, Never> {
        let Ok(effects) = trace.effects();

        Ok(effects.into_iter().filter(|effect| !matches!(effect, Effect::Custom(Screen::Show(_)))).collect())
    }

    fn shown(trace: &console_core_state_machine::Trace<Menu, Event<ButtonPress>, Effect<Screen>>) -> Result<Vec<Menu>, Never> {
        let Ok(effects) = trace.effects();

        Ok(effects
            .into_iter()
            .filter_map(|effect| match effect {
                Effect::Custom(Screen::Show(menu)) => Some(menu),
                Effect::Run(_)
                | Effect::Stream(_)
                | Effect::Prompt(_)
                | Effect::Spawn(_)
                | Effect::Subscribe(_)
                | Effect::Unsubscribe(_)
                | Effect::Write(_)
                | Effect::Notify(_)
                | Effect::Print(_)
                | Effect::Stop(_) => None,
            })
            .collect())
    }

    #[test]
    fn choosing_the_desktop_starts_it_and_stops_once_systemd_agrees() {
        let starts = ["--no-block", "start", DESKTOP];
        let Ok(answered) = answer(&starts, ExitStatus::Success);

        let Ok(trace) = run::<Recovery>(
            &(),
            &[Event::Custom(ButtonPress::Choose), answered],
        );

        let Ok(effects) = done(&trace);
        let Ok(command) = systemctl(&starts);

        assert_eq!(effects, vec![Effect::Run(command), Effect::Stop(Exit::Success)]);
    }

    #[test]
    fn a_refused_start_stays_on_the_screen_and_says_so() {
        let starts = ["--no-block", "start", DESKTOP];
        let Ok(answered) = answer(&starts, ExitStatus::Failure(Some(1)));

        let Ok(trace) = run::<Recovery>(
            &(),
            &[Event::Custom(ButtonPress::Choose), answered],
        );

        let Ok(effects) = done(&trace);
        let Ok(shown) = screen(&trace.state);
        let Ok(command) = systemctl(&starts);

        assert_eq!(effects, vec![Effect::Run(command)]);
        assert!(shown.contains("did not work"), "{shown}");
    }

    #[test]
    fn up_from_the_top_is_the_bottom() {
        let Ok(trace) = run::<Recovery>(&(), &[Event::Custom(ButtonPress::Up), Event::Custom(ButtonPress::Choose)]);
        let Ok(effects) = done(&trace);
        let Ok(command) = systemctl(&["poweroff"]);

        assert_eq!(effects, vec![Effect::Run(command)]);
    }

    #[test]
    fn a_press_while_something_is_underway_is_not_a_second_choice() {
        let Ok(trace) = run::<Recovery>(
            &(),
            &[
                Event::Custom(ButtonPress::Down),
                Event::Custom(ButtonPress::Down),
                Event::Custom(ButtonPress::Choose),
                Event::Custom(ButtonPress::Down),
                Event::Custom(ButtonPress::Choose),
            ],
        );

        let Ok(effects) = done(&trace);
        let Ok(command) = systemctl(&["--no-block", "start", TERMINAL]);

        assert_eq!(effects, vec![Effect::Run(command)]);
    }

    #[test]
    fn the_menu_is_shown_when_it_opens_and_again_whenever_it_changes_and_not_otherwise() {
        let starts = ["--no-block", "start", DESKTOP];
        let Ok(answered) = answer(&starts, ExitStatus::Failure(Some(1)));

        let Ok(trace) = run::<Recovery>(
            &(),
            &[
                Event::Opened,
                Event::Custom(ButtonPress::Left),
                Event::Custom(ButtonPress::Choose),
                answered,
            ],
        );

        let Ok(shown) = shown(&trace);

        assert_eq!(shown, vec![
            Menu { page: Page::Choices, at: 0, status: Status::Idle },
            Menu { page: Page::Choices, at: 0, status: Status::Running(Action::Chose(Choice::TryAgain)) },
            Menu { page: Page::Choices, at: 0, status: Status::Failed(Action::Chose(Choice::TryAgain)) },
        ]);
    }

    #[test]
    fn the_screen_lights_the_choice_it_is_on() {
        let Ok(shown) = screen(&Menu { page: Page::Choices, at: 3, status: Status::Idle });

        assert!(shown.contains(&format!("{LIT} Restart {PLAIN}")), "{shown}");
        assert!(shown.contains("   Shut Down\n"), "{shown}");
    }

    #[test]
    fn restarting_from_a_snapshot_lists_what_limine_offers_with_the_apply_each_stands_before() {
        let Ok(events) = opened_snapshots();
        let Ok(trace) = run::<Recovery>(&(), &events);
        let Ok(effects) = done(&trace);
        let Ok(read) = read_limine();
        let Ok(shown) = screen(&trace.state);

        assert_eq!(effects, vec![Effect::Run(read)]);
        assert!(shown.contains(&format!("{LIT}  184  2026-09-23 06:54:01  console apply e79c18f5 {PLAIN}")), "{shown}");
        assert!(shown.contains("      6  2026-08-27 14:47:13  Fresh CachyOS Installation\n"), "{shown}");
    }

    #[test]
    fn a_chosen_snapshot_is_the_next_boot_and_then_the_machine_restarts() {
        let Ok(opened) = opened_snapshots();
        let Ok(set) = one_shot(NEWEST);
        let Ok(taken) = reply(set.clone(), "", ExitStatus::Success);
        let events = [opened, vec![Event::Custom(ButtonPress::Choose), taken]].concat();
        let Ok(trace) = run::<Recovery>(&(), &events);
        let Ok(effects) = done(&trace);
        let Ok(read) = read_limine();
        let Ok(restart) = systemctl(&["reboot"]);

        assert_eq!(effects, vec![Effect::Run(read), Effect::Run(set), Effect::Run(restart)]);
    }

    #[test]
    fn a_refused_one_shot_does_not_restart_and_says_so() {
        let Ok(opened) = opened_snapshots();
        let Ok(set) = one_shot(NEWEST);
        let Ok(refused) = reply(set.clone(), "", ExitStatus::Failure(Some(1)));
        let events = [opened, vec![Event::Custom(ButtonPress::Choose), refused]].concat();
        let Ok(trace) = run::<Recovery>(&(), &events);
        let Ok(effects) = done(&trace);
        let Ok(read) = read_limine();
        let Ok(shown) = screen(&trace.state);

        assert_eq!(effects, vec![Effect::Run(read), Effect::Run(set)]);
        assert!(shown.contains("Restart from Snapshot 184 did not work."), "{shown}");
    }

    #[test]
    fn back_from_the_snapshots_is_the_choice_that_opened_them() {
        let Ok(opened) = opened_snapshots();
        let events = [opened, vec![Event::Custom(ButtonPress::Down), Event::Custom(ButtonPress::Back)]].concat();
        let Ok(trace) = run::<Recovery>(&(), &events);

        assert_eq!(trace.state, Menu { page: Page::Choices, at: 1, status: Status::Idle });
    }

    const INSTALLED: &str = "CachyOS.Snapshots.6-------2026-08-27-14-47-13.linux-cachyos-deckify";

    fn asked_about_the_installers() -> Result<Vec<Event<ButtonPress>>, Never> {
        let Ok(opened) = opened_snapshots();

        Ok([opened, vec![Event::Custom(ButtonPress::Down), Event::Custom(ButtonPress::Choose)]].concat())
    }

    #[test]
    fn a_snapshot_no_apply_took_asks_first_and_opens_on_cancel() {
        let Ok(events) = asked_about_the_installers();
        let Ok(trace) = run::<Recovery>(&(), &events);
        let Ok(effects) = done(&trace);
        let Ok(read) = read_limine();
        let Ok(shown) = screen(&trace.state);

        assert_eq!(effects, vec![Effect::Run(read)]);
        assert!(shown.contains("Restart from snapshot 6?"), "{shown}");
        assert!(shown.contains("2026-08-27 14:47:13  Fresh CachyOS Installation"), "{shown}");
        assert!(shown.contains(&format!("{LIT} Cancel {PLAIN}")), "{shown}");
    }

    #[test]
    fn a_press_that_lands_twice_cancels_rather_than_agrees() {
        let Ok(asked) = asked_about_the_installers();
        let events = [asked, vec![Event::Custom(ButtonPress::Choose)]].concat();
        let Ok(trace) = run::<Recovery>(&(), &events);
        let Ok(effects) = done(&trace);
        let Ok(read) = read_limine();

        assert_eq!(effects, vec![Effect::Run(read)]);
        assert!(matches!(trace.state.page, Page::Snapshots(_)), "{:?}", trace.state);
        assert_eq!(trace.state.at, 1);
    }

    #[test]
    fn b_on_the_question_cancels() {
        let Ok(asked) = asked_about_the_installers();
        let events = [asked, vec![Event::Custom(ButtonPress::Back)]].concat();
        let Ok(trace) = run::<Recovery>(&(), &events);

        assert!(matches!(trace.state.page, Page::Snapshots(_)), "{:?}", trace.state);
        assert_eq!(trace.state.at, 1);
    }

    #[test]
    fn agreeing_restarts_from_the_snapshot_that_was_asked_about() {
        let Ok(asked) = asked_about_the_installers();
        let Ok(set) = one_shot(INSTALLED);
        let Ok(taken) = reply(set.clone(), "", ExitStatus::Success);
        let events = [asked, vec![Event::Custom(ButtonPress::Down), Event::Custom(ButtonPress::Choose), taken]].concat();
        let Ok(trace) = run::<Recovery>(&(), &events);
        let Ok(effects) = done(&trace);
        let Ok(read) = read_limine();
        let Ok(restart) = systemctl(&["reboot"]);

        assert_eq!(effects, vec![Effect::Run(read), Effect::Run(set), Effect::Run(restart)]);
    }

    #[test]
    fn a_machine_with_no_snapshots_says_so_and_choosing_does_nothing() {
        let Ok(read) = read_limine();
        let Ok(listed) = reply(read.clone(), "/Linux\nprotocol: linux\n", ExitStatus::Success);
        let events = [Event::Custom(ButtonPress::Down), Event::Custom(ButtonPress::Choose), listed, Event::Custom(ButtonPress::Choose)];
        let Ok(trace) = run::<Recovery>(&(), &events);
        let Ok(effects) = done(&trace);
        let Ok(shown) = screen(&trace.state);

        assert_eq!(effects, vec![Effect::Run(read)]);
        assert!(shown.contains("There are no snapshots."), "{shown}");
    }
}
