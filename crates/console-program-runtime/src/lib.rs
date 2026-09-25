//! The loop that carries out what a program decided.
//!
//! `console_program_contract` is the half that thinks and this is the half
//! that acts, and the split is the whole point: everything in here touches a
//! machine and nothing in here decides anything. A fault in this crate is a
//! fault in one place rather than in twenty `main`s.
//!
//! **It is not the only loop, and cannot be.** GTK draws on one thread and
//! will not be driven from someone else's loop, so every panel keeps its own
//! and drives the same contract from glib. What is here is the loop for the
//! programs that have no toolkit of their own: the small ones that were shell
//! scripts, and the daemons.
//!
//! **A program that has nothing left to wait for is finished.** There is no
//! `exit` at the end of `main` and nothing to remember: when the queue is
//! empty and the program has asked for no stretch of time, the loop tells it
//! [`Event::Stopping`], carries out whatever it decided about that, and ends.
//! `keyboard-toggle` is one turn long and says so by wanting nothing.
//!
//! **`Interpreter::tick` is asked before the program is told the timer fell
//! due**, because a daemon decides on the wake-up: anything that arrived
//! while it was asleep has to be in the state by the time it is asked.
//!
//! **What a program subscribed to is asked of the pool.** `Subscription::Topic`
//! is a topic, `console-events` is the one subscription to it on the machine,
//! and this is where the two meet: the loop holds one [`Subscriber`],
//! `Effect::Subscribe` adds a topic to it and `Effect::Unsubscribe` takes one
//! away, and what the pool says arrives as [`Event::Changed`] the way a timer
//! falling due arrives as [`Event::Tick`]. A program with no pool hears nothing
//! and keeps whatever `Subscription::Timer` it also asked for, which is the
//! fallback the whole design leans on: slower, and never wrong.
//!
//! **So there is one wait rather than a sleep.** The loop blocks on the pool's
//! channel until the next timer falls due, which is the same wait for both
//! reasons a program can be woken and is why nothing in this crate has to
//! declare a sleep any more. A program subscribed to no topic still waits here:
//! the channel is open and quiet, and a `recv_timeout` on it that times out is
//! the timer falling due. Getting into the pool is an event of its own there
//! and the loop does nothing with it, because what a program subscribed to
//! after a gap is the replay that is already following it.
//!
//! **A timer that cannot fall due is not a timer.** `Instant::checked_add`
//! refuses when the answer would be hundreds of years out, so nothing on a
//! machine reaches it -- but the arm has to say something, and there are only
//! two things it can say. "Fall due at the instant you already had" is a timer
//! that fires, fires again and never waits, which is a busy loop wearing a
//! timer's name. "This one has stopped" is what is true, so it is dropped: the
//! program is told the timer it was waiting for fell due, and nothing waits for
//! it again.
//!
//! **An answer arrives on a later turn.** [`Effect::Run`] is run to completion
//! here and its [`Event::Answered`] goes on the queue rather than back into the
//! turn that asked for it, because a program that could see its own answer
//! inside `update` would be a program that blocks -- and the first thing that
//! would block on is InputPlumber not being on the bus for a minute. It is run
//! on this thread for now, which is honest for a program with one thing to do
//! and is the thing to change first when a daemon here waits on something
//! slow: the doc's shape is one thread per ask in flight.

use std::collections::VecDeque;
use std::process::{self, ExitCode, Stdio};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Instant;

use console_events::subscription::{self, Desired, Received, Subscriber};
use console_program_lifetime::{Detached, Still, let_go};
use console_core_external_programs::Program as ExternalProgram;
use console_core_never::Never;
use console_program_contract::{
    Answer, Arguments, Change, Choice, Effect, Exit, Executable, Initial, Program, Prompt, Timer, Command,
    Notification, Elapsed, Update, Subscription, ExitStatus, Event, FileWrite,
};

pub trait Interpreter {
    type Event;
    type Effect;

    fn interpret(&mut self, effect: &Self::Effect) -> Vec<Event<Self::Event>>;

    fn tick(&mut self, _timer: &Timer, _since: Elapsed) -> Result<Vec<Event<Self::Event>>, Never> {
        Ok(Vec::new())
    }
}

pub struct Pure;

impl Interpreter for Pure {
    type Event = Never;
    type Effect = Never;

    fn interpret(&mut self, effect: &Never) -> Vec<Event<Never>> {
        match *effect {}
    }
}

struct Waiting {
    timer: Timer,
    due: Instant,
}

enum Woke {
    Tick(Timer),
    Changed(Change),
    Finished,
}

pub fn run<P, C>(called: &str, arguments: &Arguments, interpreter: &mut C) -> Result<ExitCode, Never>
where
    P: Program,
    C: Interpreter<Event = P::Event, Effect = P::Effect>,
{
    let Initial { state, subscriptions } = P::init(arguments);
    let mut held = state;
    let mut timers: Vec<Waiting> = Vec::new();
    let mut queue: VecDeque<Event<P::Event>> = VecDeque::new();
    let mut running: Vec<Detached> = Vec::new();
    let Ok(subscriber) = subscription::connect(&[]);

    #[cfg_attr(
        dylint_lib = "explicit039_no_reading_the_clock",
        allow(
            explicit039_no_reading_the_clock,
            reason = "this is the half that acts: the program it runs decides nothing from the clock, and the moment its timers are counted from has to be read somewhere"
        )
    )]
    let began = Instant::now();

    for want in &subscriptions {
        let Ok(()) = subscribe(called, &mut timers, &subscriber, want, began);
    }

    queue.push_back(Event::Opened);

    loop {
        let word = match queue.pop_front() {
            Some(word) => word,
            None => {
                let Ok(woke) = woke(called, &mut timers, &subscriber);

                match woke {
                    Woke::Tick(timer) => {
                        let since = began.elapsed();

                        let Ok(events) = interpreter.tick(&timer, since);

                        queue.extend(events);
                        queue.push_back(Event::Tick(timer, since));

                        continue;
                    }
                    Woke::Changed(change) => {
                        queue.push_back(Event::Changed(change));

                        continue;
                    }
                    Woke::Finished => Event::Stopping,
                }
            },
        };

        let last = matches!(word, Event::Stopping);
        let Update { state: next, effects } = P::update(&held, &word);
        held = next;
        let mut ending: Option<Exit> = None;

        for effect in &effects {
            match effect {
                Effect::Run(command) => {
                    let Ok(answer) = asked(called, command);

                    queue.push_back(Event::Replied(answer));
                }
                Effect::Stream(command) => {
                    let Ok(answer) = watched(called, command);

                    queue.push_back(Event::Replied(answer));
                }
                Effect::Prompt(prompt) => {
                    let Ok(chose) = chose(prompt);

                    queue.push_back(Event::Chosen(chose));
                }
                Effect::Spawn(command) => {
                    let Ok(child) = started(called, command);

                    running.extend(child);
                }
                Effect::Subscribe(want) => {
                    #[cfg_attr(
                        dylint_lib = "explicit039_no_reading_the_clock",
                        allow(
                            explicit039_no_reading_the_clock,
                            reason = "a timer asked for part way through a life falls due from when it was asked, and the loop is the only thing that knows when that was"
                        )
                    )]
                    let Ok(()) = subscribe(called, &mut timers, &subscriber, want, Instant::now());
                }
                Effect::Unsubscribe(want) => {
                    let Ok(()) = unsubscribe(&mut timers, &subscriber, want);
                }
                Effect::Write(write) => {
                    let Ok(()) = wrote(called, write);
                }
                Effect::Notify(notification) => {
                    let Ok(()) = notify(called, notification);
                }
                #[cfg_attr(
                    dylint_lib = "explicit041_no_unsaid_printing",
                    allow(
                        explicit041_no_unsaid_printing,
                        reason = "this is what carries `Effect::Print` out, so something at the bottom has to be the thing that prints"
                    )
                )]
                Effect::Print(line) => println!("{line}"),
                Effect::Stop(how) => ending = Some(how.clone()),
                Effect::Custom(effect) => queue.extend(interpreter.interpret(effect)),
            }
        }

        let Ok(still) = reaped(running);

        running = still;

        match (ending, last) {
            (Some(Exit::Success), _) | (None, true) => return Ok(ExitCode::SUCCESS),
            (Some(Exit::Failure(fault)), _) => {
                eprintln!("{called}: {fault}");

                return Ok(ExitCode::FAILURE);
            }
            (None, false) => {},
        }
    }
}

fn woke(called: &str, timers: &mut Vec<Waiting>, subscriber: &Subscriber) -> Result<Woke, Never> {
    loop {
        let Ok(waited) = waited(called, timers, subscriber);

        match waited {
            Some(woke) => return Ok(woke),
            None => {},
        }
    }
}

fn waited(called: &str, timers: &mut Vec<Waiting>, subscriber: &Subscriber) -> Result<Option<Woke>, Never> {
    let soonest = timers.iter().map(|waiting| waiting.due).min();
    let Ok(received) = subscriber.received();
    let Ok(wanting) = subscriber.desired();

    match (soonest, wanting) {
        (None, Desired::None) => Ok(Some(Woke::Finished)),
        (None, Desired::Some) => Ok(match received.recv() {
            Ok(Received::Event(change)) => Some(Woke::Changed(change)),
            Ok(Received::Connected) => None,
            Err(_) => Some(Woke::Finished),
        }),
        (Some(soonest), Desired::Some | Desired::None) => {
            #[cfg_attr(
                dylint_lib = "explicit039_no_reading_the_clock",
                allow(
                    explicit039_no_reading_the_clock,
                    reason = "how long is left of the nearest timer, which is the one wait this loop makes rather than a decision anything takes from it"
                )
            )]
            let waiting = soonest.saturating_duration_since(Instant::now());

            match received.recv_timeout(waiting) {
                Ok(Received::Event(change)) => Ok(Some(Woke::Changed(change))),
                Ok(Received::Connected) => Ok(None),
                Err(RecvTimeoutError::Timeout) => {
                    let Ok(fell) = fell_due(called, timers);

                    Ok(Some(fell))
                }
                Err(RecvTimeoutError::Disconnected) => Ok(Some(Woke::Finished)),
            }
        }
    }
}

fn fell_due(called: &str, timers: &mut Vec<Waiting>) -> Result<Woke, Never> {
    #[cfg_attr(
        dylint_lib = "explicit039_no_reading_the_clock",
        allow(
            explicit039_no_reading_the_clock,
            reason = "which timers have fallen due by the moment the loop woke, which is a reading of the machine rather than a decision the program makes"
        )
    )]
    let woken = Instant::now();

    let waiting = match timers.iter_mut().find(|waiting| waiting.due <= woken) {
        Some(waiting) => waiting,
        None => return Ok(Woke::Finished),
    };

    let timer = waiting.timer;

    match woken.checked_add(timer.interval) {
        Some(due) => waiting.due = due,
        None => {
            eprintln!(
                "{called}: {} cannot fall due again, so this was the last tick",
                timer.name
            );

            timers.retain(|waiting| waiting.timer != timer);
        }
    }

    Ok(Woke::Tick(timer))
}

fn subscribe(
    called: &str,
    timers: &mut Vec<Waiting>,
    subscriber: &Subscriber,
    want: &Subscription,
    now: Instant,
) -> Result<(), Never> {
    match want {
        Subscription::Timer(timer) => {
            match now.checked_add(timer.interval) {
                Some(due) => timers.push(Waiting { timer: *timer, due }),
                None => eprintln!(
                    "{called}: {} cannot fall due, so nothing is waiting for it",
                    timer.name
                ),
            }
        }
        Subscription::Topic(topic) => {
            let Ok(()) = subscriber.subscribe(topic);
        }
    }

    Ok(())
}

fn unsubscribe(timers: &mut Vec<Waiting>, subscriber: &Subscriber, want: &Subscription) -> Result<(), Never> {
    match want {
        Subscription::Timer(timer) => timers.retain(|waiting| waiting.timer != *timer),
        Subscription::Topic(topic) => {
            let Ok(()) = subscriber.unsubscribe(topic);
        }
    }

    Ok(())
}

fn asked(called: &str, command: &Command) -> Result<Answer, Never> {
    let Ok(mut starting) = starting(command);

    let said = starting.stderr(Stdio::inherit()).output();

    Ok(match said {
        Ok(said) => Answer {
            command: command.clone(),
            output: String::from_utf8_lossy(&said.stdout).to_string(),
            status: match said.status.success() {
                true => ExitStatus::Success,
                false => ExitStatus::Failure(said.status.code()),
            },
        },
        Err(fault) => {
            let Ok(name) = command.program.name();

            eprintln!("{called}: {name} did not run: {fault}");

            Answer { command: command.clone(), output: String::new(), status: ExitStatus::Failure(None) }
        }
    })
}

fn watched(called: &str, command: &Command) -> Result<Answer, Never> {
    let Ok(mut starting) = starting(command);

    let went = starting.stdout(Stdio::inherit()).stderr(Stdio::inherit()).status();

    Ok(match went {
        Ok(went) => Answer {
            command: command.clone(),
            output: String::new(),
            status: match went.success() {
                true => ExitStatus::Success,
                false => ExitStatus::Failure(went.code()),
            },
        },
        Err(fault) => {
            let Ok(name) = command.program.name();

            eprintln!("{called}: {name} did not run: {fault}");

            Answer { command: command.clone(), output: String::new(), status: ExitStatus::Failure(None) }
        }
    })
}

#[cfg_attr(
    dylint_lib = "explicit041_no_unsaid_printing",
    allow(
        explicit041_no_unsaid_printing,
        reason = "a question asked of whoever is at the terminal, whose answer is read back off stdin on the same line -- there is no effect for the half of a conversation that is waiting for a person"
    )
)]
fn chose(prompt: &Prompt) -> Result<Choice, Never> {
    print!("{} ", prompt.message);

    let _ = std::io::Write::flush(&mut std::io::stdout());

    let mut said = String::new();
    let terminal = std::io::stdin();

    match terminal.read_line(&mut said) {
        Ok(0) | Err(_) => Ok(prompt.default),
        Ok(_) => meant(said.trim(), prompt.default),
    }
}

fn meant(said: &str, default: Choice) -> Result<Choice, Never> {
    Ok(match said.to_lowercase().as_str() {
        "y" | "yes" => Choice::Yes,
        "n" | "no" => Choice::No,
        _ => default,
    })
}

fn started(called: &str, command: &Command) -> Result<Option<Detached>, Never> {
    let Ok(mut starting) = ExternalProgram::SystemdRun.command();
    let Ok(name) = command.program.name();

    starting
        .args(["--user", "--scope", "--quiet", "--collect", "--"])
        .arg(name)
        .args(&command.arguments)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());

    Ok(match let_go(&mut starting) {
        Ok(child) => Some(child),
        Err(fault) => {
            eprintln!("{called}: {name} did not start: {fault}");

            None
        }
    })
}

fn reaped(running: Vec<Detached>) -> Result<Vec<Detached>, Never> {
    Ok(running
        .into_iter()
        .filter_map(|mut child| {
            let Ok(still) = child.still();

            match still {
                Still::Running => Some(child),
                Still::Ended => None,
            }
        })
        .collect())
}

fn starting(command: &Command) -> Result<process::Command, Never> {
    let mut starting = match command.program {
        Executable::External(program) => {
            let Ok(starting) = program.command();

            starting
        }
        Executable::Internal(name) => process::Command::new(name),
    };

    starting.args(&command.arguments);

    Ok(starting)
}

fn wrote(called: &str, write: &FileWrite) -> Result<(), Never> {
    match console_core_atomic_writes::whole(&write.path, write.contents.as_bytes()) {
        Ok(()) => {},
        Err(fault) => eprintln!("{called}: {}: {fault}", write.path.display()),
    }

    Ok(())
}

fn notify(called: &str, notification: &Notification) -> Result<(), Never> {
    let Ok(mut telling) = ExternalProgram::NotifySend.command();

    telling.arg(&notification.summary);

    match &notification.body {
        Some(body) => {
            let _ = telling.arg(body);
        }
        None => {},
    }

    match telling.status() {
        Ok(_) => {},
        Err(fault) => eprintln!("{called}: nothing was said on the screen: {fault}"),
    }

    Ok(())
}
