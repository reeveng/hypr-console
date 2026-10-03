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
//! **A program can be woken by what only its interpreter can reach.** A
//! device node, a socket someone else opened -- the runtime has no way to wait
//! on either, and recovery and the wallpaper each wrote the whole loop again
//! rather than go without. [`Interpreter::listen`] is handed a [`Tell`] once,
//! before the first turn, and whatever it sends arrives as [`Event::Custom`]
//! through the same wait the pool's changes come through: the pool is carried
//! onto that one channel by a thread of its own, so there is still one wait
//! and not a poll across two. An interpreter that listens keeps the program
//! alive with no timer and no topic, and [`Tell::end`] is how the thread it
//! started says the thing it was waiting on has gone -- a program waiting on a
//! pad that can no longer be read is finished, and saying so is the only way
//! it stops rather than waiting on a channel that will never speak again.
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
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Instant;

use console_events::subscription::{self, Desired, Received, Subscriptions};
use console_program_lifetime::{Detached, Still, let_go, threads};
use console_core_external_programs::Program as ExternalProgram;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_state_machine::{Machine, Transition};
use console_program_contract::{
    Answer, Arguments, Change, Choice, Effect, Exit, Executable, Prompt, Timer, Command,
    Notification, Elapsed, Subscription, ExitStatus, Event, FileWrite,
};

pub trait Interpreter {
    type Event;
    type Effect;

    fn interpret(&mut self, effect: &Self::Effect) -> Vec<Event<Self::Event>>;

    fn tick(&mut self, _timer: &Timer, _since: Elapsed) -> Result<Vec<Event<Self::Event>>, Never> {
        Ok(Vec::new())
    }

    fn listen(&mut self, _tell: Tell<Self::Event>) -> Result<Subscribed, Never> {
        Ok(Subscribed::No)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subscribed {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    Yes,
    Gone,
}

enum Arrived<E> {
    Pool(Received),
    Custom(E),
    Ended(Exit),
}

pub struct Tell<E>(Sender<Arrived<E>>);

impl<E> Tell<E> {
    pub fn tell(&self, event: E) -> Result<Delivery, Never> {
        Ok(match self.0.send(Arrived::Custom(event)) {
            Ok(()) => Delivery::Yes,
            Err(_the_loop_has_ended) => Delivery::Gone,
        })
    }

    pub fn end(&self, how: Exit) -> Result<Delivery, Never> {
        Ok(match self.0.send(Arrived::Ended(how)) {
            Ok(()) => Delivery::Yes,
            Err(_the_loop_has_ended) => Delivery::Gone,
        })
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

enum Woke<E> {
    Tick(Timer),
    Changed(Change),
    Custom(E),
    Ended(Exit),
    Finished,
}

struct Waits<'a, E> {
    arrived: &'a Receiver<Arrived<E>>,
    subscriptions: &'a Subscriptions,
    listening: Subscribed,
}

pub fn run<M, C>(called: &str, arguments: &Arguments, interpreter: &mut C) -> Result<ExitCode, Never>
where
    M: Machine<Input = Arguments, Request = Event<C::Event>, Effect = Effect<C::Effect>>,
    C: Interpreter<Event: Send + 'static>,
{
    let Ok(Transition { state, effects: initialized }) = M::initial_transition(arguments, None);
    let Ok(subscriber) = subscription::connect(&[]);
    let Ok((topics, pool)) = subscriber.split();
    let (telling, arrived) = channel::<Arrived<C::Event>>();
    let carrying = telling.clone();

    let Ok(()) = threads::let_go(std::thread::spawn(move || {
        for received in pool.iter() {
            match carrying.send(Arrived::Pool(received)) {
                Ok(()) => {},
                Err(_the_loop_has_ended) => return,
            }
        }
    }));

    let Ok(listening) = interpreter.listen(Tell(telling));
    let waits = Waits { arrived: &arrived, subscriptions: &topics, listening };

    #[cfg_attr(
        dylint_lib = "explicit039_no_reading_the_clock",
        allow(
            explicit039_no_reading_the_clock,
            reason = "this is the half that acts: the program it runs decides nothing from the clock, and the moment its timers are counted from has to be read somewhere"
        )
    )]
    let began = Instant::now();
    let mut turning = Turning { held: state, timers: Vec::new(), queue: VecDeque::new(), running: Vec::new(), interpreter };
    let asked = Context { called, topics: &topics, waits: &waits, began };
    let Ok(ending) = carried_out(&initialized, &mut turning, &asked);

    match ending {
        Some(how) => return ended(called, how),
        None => {}
    }

    turning.queue.push_back(Event::Opened);

    Ok(match console_core_iteration::iterate(turning, |turning| turned::<M, C>(turning, &asked)) {
        Ok(ended) => ended,
        Err(endless) => {
            eprintln!("{called}: {endless}");

            ExitCode::FAILURE
        }
    })
}

struct Turning<'i, S, E, C> {
    held: S,
    timers: Vec<Waiting>,
    queue: VecDeque<Event<E>>,
    running: Vec<Detached>,
    interpreter: &'i mut C,
}

struct Context<'a, E> {
    called: &'a str,
    topics: &'a Subscriptions,
    waits: &'a Waits<'a, E>,
    began: Instant,
}

type Turned<'i, M, C> = Step<Turning<'i, <M as Machine>::State, <C as Interpreter>::Event, C>, ExitCode>;

fn turned<'i, M, C>(
    mut turning: Turning<'i, M::State, C::Event, C>,
    asked: &Context<'_, C::Event>,
) -> Result<Turned<'i, M, C>, Never>
where
    M: Machine<Request = Event<C::Event>, Effect = Effect<C::Effect>>,
    C: Interpreter,
{
    let word = match turning.queue.pop_front() {
        Some(word) => word,
        None => {
            let Ok(woke) = woke(asked.called, &mut turning.timers, asked.waits);

            match woke {
                Woke::Tick(timer) => {
                    let since = asked.began.elapsed();

                    let Ok(events) = turning.interpreter.tick(&timer, since);

                    turning.queue.extend(events);
                    turning.queue.push_back(Event::Tick(timer, since));

                    return Ok(Step::Again(turning));
                }
                Woke::Changed(change) => {
                    turning.queue.push_back(Event::Changed(change));

                    return Ok(Step::Again(turning));
                }
                Woke::Custom(event) => {
                    turning.queue.push_back(Event::Custom(event));

                    return Ok(Step::Again(turning));
                }
                Woke::Ended(how) => {
                    let Ok(ended) = ended(asked.called, how);

                    return Ok(Step::Halt(ended));
                }
                Woke::Finished => Event::Stopping,
            }
        },
    };

    let last = matches!(word, Event::Stopping);
    let Ok(Transition { state: next, effects }) = M::transition(turning.held, word);

    turning.held = next;

    let Ok(ending) = carried_out(&effects, &mut turning, asked);

    let Ok(still) = reap(turning.running);

    turning.running = still;

    Ok(match (ending, last) {
        (Some(how), _) => {
            let Ok(ended) = ended(asked.called, how);

            Step::Halt(ended)
        }
        (None, true) => Step::Halt(ExitCode::SUCCESS),
        (None, false) => Step::Again(turning),
    })
}

fn carried_out<S, C: Interpreter>(
    effects: &[Effect<C::Effect>],
    turning: &mut Turning<'_, S, C::Event, C>,
    asked: &Context<'_, C::Event>,
) -> Result<Option<Exit>, Never> {
    let called = asked.called;
    let mut ending: Option<Exit> = None;

    for effect in effects {
        match effect {
            Effect::Run(command) => {
                let Ok(answer) = run_captured(called, command);

                turning.queue.push_back(Event::Replied(answer));
            }
            Effect::Stream(command) => {
                let Ok(answer) = run_attached(called, command);

                turning.queue.push_back(Event::Replied(answer));
            }
            Effect::Prompt(prompt) => {
                let Ok(chose) = chose(prompt);

                turning.queue.push_back(Event::Chosen(chose));
            }
            Effect::Spawn(command) => {
                let Ok(child) = spawn_scoped(called, command);

                turning.running.extend(child);
            }
            Effect::Subscribe(want) => {
                #[cfg_attr(
                    dylint_lib = "explicit039_no_reading_the_clock",
                    allow(
                        explicit039_no_reading_the_clock,
                        reason = "a timer asked for part way through a life falls due from when it was asked, and the loop is the only thing that knows when that was"
                    )
                )]
                let Ok(()) = subscribe(called, &mut turning.timers, asked.topics, want, Instant::now());
            }
            Effect::Unsubscribe(want) => {
                let Ok(()) = unsubscribe(&mut turning.timers, asked.topics, want);
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
            Effect::Custom(effect) => turning.queue.extend(turning.interpreter.interpret(effect)),
        }
    }

    Ok(ending)
}

fn ended(called: &str, how: Exit) -> Result<ExitCode, Never> {
    Ok(match how {
        Exit::Success => ExitCode::SUCCESS,
        Exit::Failure(fault) => {
            eprintln!("{called}: {fault}");

            ExitCode::FAILURE
        }
    })
}

fn woke<E>(called: &str, timers: &mut Vec<Waiting>, waits: &Waits<'_, E>) -> Result<Woke<E>, Never> {
    let woke = console_core_iteration::iterate(timers, |timers| {
        let Ok(waited) = wait(called, timers, waits);

        Ok(match waited {
            Some(woke) => Step::Halt(woke),
            None => Step::Again(timers),
        })
    });

    Ok(match woke {
        Ok(woke) => woke,
        Err(_endless) => Woke::Finished,
    })
}

fn wait<E>(called: &str, timers: &mut Vec<Waiting>, waits: &Waits<'_, E>) -> Result<Option<Woke<E>>, Never> {
    let soonest = timers.iter().map(|waiting| waiting.due).min();
    let Ok(wanting) = waits.subscriptions.desired();

    match (soonest, wanting, waits.listening) {
        (None, Desired::None, Subscribed::No) => Ok(Some(Woke::Finished)),
        (None, Desired::Some, _) | (None, Desired::None, Subscribed::Yes) => Ok(match waits.arrived.recv() {
            Ok(arrived) => {
                let Ok(woke) = arrival(arrived);

                woke
            }
            Err(_the_sender_has_gone) => Some(Woke::Finished),
        }),
        (Some(soonest), Desired::Some | Desired::None, _) => {
            #[cfg_attr(
                dylint_lib = "explicit039_no_reading_the_clock",
                allow(
                    explicit039_no_reading_the_clock,
                    reason = "how long is left of the nearest timer, which is the one wait this loop makes rather than a decision anything takes from it"
                )
            )]
            let waiting = soonest.saturating_duration_since(Instant::now());

            match waits.arrived.recv_timeout(waiting) {
                Ok(arrived) => arrival(arrived),
                Err(RecvTimeoutError::Timeout) => {
                    let Ok(fell) = fell_due(called, timers);

                    Ok(Some(fell))
                }
                Err(RecvTimeoutError::Disconnected) => Ok(Some(Woke::Finished)),
            }
        }
    }
}

fn arrival<E>(arrived: Arrived<E>) -> Result<Option<Woke<E>>, Never> {
    Ok(match arrived {
        Arrived::Pool(Received::Event(change)) => Some(Woke::Changed(change)),
        Arrived::Pool(Received::Connected) => None,
        Arrived::Custom(event) => Some(Woke::Custom(event)),
        Arrived::Ended(how) => Some(Woke::Ended(how)),
    })
}

fn fell_due<E>(called: &str, timers: &mut Vec<Waiting>) -> Result<Woke<E>, Never> {
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
    subscriptions: &Subscriptions,
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
            let Ok(()) = subscriptions.subscribe(topic);
        }
    }

    Ok(())
}

fn unsubscribe(timers: &mut Vec<Waiting>, subscriptions: &Subscriptions, want: &Subscription) -> Result<(), Never> {
    match want {
        Subscription::Timer(timer) => timers.retain(|waiting| waiting.timer != *timer),
        Subscription::Topic(topic) => {
            let Ok(()) = subscriptions.unsubscribe(topic);
        }
    }

    Ok(())
}

fn run_captured(called: &str, command: &Command) -> Result<Answer, Never> {
    let Ok(mut starting) = build_command(command);

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

fn run_attached(called: &str, command: &Command) -> Result<Answer, Never> {
    let Ok(mut starting) = build_command(command);

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
        Ok(0) => Ok(prompt.default),
        Err(_the_terminal_failed) => Ok(prompt.default),
        Ok(_) => parse_choice(said.trim(), prompt.default),
    }
}

fn parse_choice(said: &str, default: Choice) -> Result<Choice, Never> {
    Ok(match said.to_lowercase().as_str() {
        "y" | "yes" => Choice::Yes,
        "n" | "no" => Choice::No,
        _ => default,
    })
}

fn spawn_scoped(called: &str, command: &Command) -> Result<Option<Detached>, Never> {
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

fn reap(running: Vec<Detached>) -> Result<Vec<Detached>, Never> {
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

fn build_command(command: &Command) -> Result<process::Command, Never> {
    let mut starting = match command.program {
        Executable::External(program) => {
            let Ok(starting) = program.command();

            starting
        }
        Executable::Internal(program) => {
            let Ok(starting) = program.command();

            starting
        }
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
