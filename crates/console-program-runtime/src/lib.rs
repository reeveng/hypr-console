//! The loop that carries out what a program decided.
//!
//! `console_program_contract` is the half that thinks and this is the half
//! that acts, and the split is the whole point: everything in here touches a
//! machine and nothing in here decides anything. A fault in this crate is a
//! fault in one place rather than in twenty `main`s.
//!
//! **It is not the only loop, and cannot be.** GTK draws on one thread and
//! will not be driven from somebody else's loop, so every panel keeps its own
//! and drives the same contract from glib. What is here is the loop for the
//! programs that have no toolkit of their own: the small ones that were shell
//! scripts, and the daemons.
//!
//! **A program that has nothing left to wait for is finished.** There is no
//! `exit` at the end of `main` and nothing to remember: when the queue is
//! empty and the program has asked for no stretch of time, the loop tells it
//! [`Word::Stopping`], carries out whatever it decided about that, and ends.
//! `keyboard-toggle` is one turn long and says so by wanting nothing.
//!
//! **`Carrying::came` is asked before the program is told the stretch came
//! round**, because a daemon decides on the wake-up: anything that arrived
//! while it was asleep has to be in the state by the time it is asked.
//!
//! **What a program asked to be told is asked of the pool.** `Wants::Words` is
//! a topic, `console-events` is the one subscription to it on the machine, and
//! this is where the two meet: the loop holds one [`Listening`], `Doing::Listen`
//! adds a topic to it and `Doing::Deafen` takes one away, and what the pool
//! says arrives as [`Word::Changed`] the way a round arrives as
//! [`Word::CameRound`]. A program with no pool hears nothing and keeps
//! whatever `Wants::Round` it also asked for, which is the fallback the whole
//! design leans on: slower, and never wrong.
//!
//! **So there is one wait rather than a sleep.** The loop blocks on the pool's
//! channel until the next round falls due, which is the same wait for both
//! reasons a program can be woken and is why nothing in this crate has to
//! declare a sleep any more. A program that wants no topic still waits here:
//! the channel is open and quiet, and a `recv_timeout` on it that times out is
//! the round coming round. Getting into the pool is a word of its own there
//! and the loop does nothing with it, because what a program wants after a gap
//! is the replay that is already following it.
//!
//! **An answer arrives on a later turn.** [`Doing::Ask`] is run to completion
//! here and its [`Word::Answered`] goes on the queue rather than back into the
//! turn that asked for it, because a program that could see its own answer
//! inside `heard` would be a program that blocks -- and the first thing that
//! would block on is InputPlumber not being on the bus for a minute. It is run
//! on this thread for now, which is honest for a program with one thing to do
//! and is the thing to change first when a daemon here waits on something
//! slow: the doc's shape is one thread per ask in flight.

use std::collections::VecDeque;
use std::process::{Command, ExitCode, Stdio};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Instant;

use console_events::listening::{self, Heard, Listening, Wanting};
use console_program_lifetime::{LetGo, Still, let_go};
use console_core_external_programs::Program as Theirs;
use console_core_never::Never;
use console_program_contract::{
    Answer, Argv, Changed, Chose, Doing, Ending, Named, Opening, Program, Question, Round, Runs,
    Saying, Since, Turn, Wants, Went, Word, Writing,
};

pub trait Carrying {
    type Hears;
    type Does;

    fn its(&mut self, doing: &Self::Does) -> Vec<Word<Self::Hears>>;

    fn came(&mut self, _round: &Round, _since: Since) -> Result<Vec<Word<Self::Hears>>, Never> {
        Ok(Vec::new())
    }
}

pub struct Nothing;

impl Carrying for Nothing {
    type Hears = Never;
    type Does = Never;

    fn its(&mut self, doing: &Never) -> Vec<Word<Never>> {
        match *doing {}
    }
}

struct Waiting {
    round: Round,
    due: Instant,
}

enum Woke {
    Came(Round),
    Told(Changed),
    Nothing,
}

pub fn run<P, C>(called: &str, argv: &Argv, carrying: &mut C) -> Result<ExitCode, Never>
where
    P: Program,
    C: Carrying<Hears = P::Hears, Does = P::Does>,
{
    let Opening { state, wants } = P::opening(argv);
    let mut held = state;
    let mut rounds: Vec<Waiting> = Vec::new();
    let mut queue: VecDeque<Word<P::Hears>> = VecDeque::new();
    let mut running: Vec<LetGo> = Vec::new();
    let Ok(words) = listening::listen(&[]);

    let began = Instant::now();

    for want in &wants {
        let Ok(()) = listen(&mut rounds, &words, want, began);
    }

    queue.push_back(Word::Opened);

    loop {
        let word = match queue.pop_front() {
            Some(word) => word,
            None => {
                let Ok(woke) = woke(&mut rounds, &words);

                match woke {
                    Woke::Came(round) => {
                        let since = began.elapsed();

                        let Ok(came) = carrying.came(&round, since);

                        queue.extend(came);
                        queue.push_back(Word::CameRound(round, since));

                        continue;
                    }
                    Woke::Told(changed) => {
                        queue.push_back(Word::Changed(changed));

                        continue;
                    }
                    Woke::Nothing => Word::Stopping,
                }
            },
        };

        let last = matches!(word, Word::Stopping);
        let Turn { now, doings } = P::heard(&held, &word);
        held = now;
        let mut ending: Option<Ending> = None;

        for doing in &doings {
            match doing {
                Doing::Ask(runs) => {
                    let Ok(answer) = asked(called, runs);

                    queue.push_back(Word::Answered(answer));
                }
                Doing::Watch(runs) => {
                    let Ok(answer) = watched(called, runs);

                    queue.push_back(Word::Answered(answer));
                }
                Doing::AskWhoever(question) => {
                    let Ok(chose) = chose(question);

                    queue.push_back(Word::Chose(chose));
                }
                Doing::Start(runs) => {
                    let Ok(child) = started(called, runs);

                    running.extend(child);
                }
                Doing::Listen(want) => {
                    let Ok(()) = listen(&mut rounds, &words, want, Instant::now());
                }
                Doing::Deafen(want) => {
                    let Ok(()) = deafen(&mut rounds, &words, want);
                }
                Doing::Write(writing) => {
                    let Ok(()) = wrote(called, writing);
                }
                Doing::Say(saying) => {
                    let Ok(()) = say(called, saying);
                }
                Doing::Print(line) => println!("{line}"),
                Doing::Stop(how) => ending = Some(how.clone()),
                Doing::Its(its) => queue.extend(carrying.its(its)),
            }
        }

        let Ok(still) = reaped(running);

        running = still;

        match (ending, last) {
            (Some(Ending::Done), _) | (None, true) => return Ok(ExitCode::SUCCESS),
            (Some(Ending::Badly(fault)), _) => {
                eprintln!("{called}: {fault}");

                return Ok(ExitCode::FAILURE);
            }
            (None, false) => {},
        }
    }
}

fn woke(rounds: &mut [Waiting], words: &Listening) -> Result<Woke, Never> {
    loop {
        let Ok(waited) = waited(rounds, words);

        match waited {
            Some(woke) => return Ok(woke),
            None => {},
        }
    }
}

fn waited(rounds: &mut [Waiting], words: &Listening) -> Result<Option<Woke>, Never> {
    let soonest = rounds.iter().map(|waiting| waiting.due).min();
    let Ok(heard) = words.heard();
    let Ok(wanting) = words.wanting();

    match (soonest, wanting) {
        (None, Wanting::Nothing) => Ok(Some(Woke::Nothing)),
        (None, Wanting::Something) => Ok(match heard.recv() {
            Ok(Heard::Said(changed)) => Some(Woke::Told(changed)),
            Ok(Heard::GotIn) => None,
            Err(_) => Some(Woke::Nothing),
        }),
        (Some(soonest), Wanting::Something | Wanting::Nothing) => {
            let waiting = soonest.saturating_duration_since(Instant::now());

            match heard.recv_timeout(waiting) {
                Ok(Heard::Said(changed)) => Ok(Some(Woke::Told(changed))),
                Ok(Heard::GotIn) => Ok(None),
                Err(RecvTimeoutError::Timeout) => {
                    let Ok(came) = came(rounds);

                    Ok(Some(came))
                }
                Err(RecvTimeoutError::Disconnected) => Ok(Some(Woke::Nothing)),
            }
        }
    }
}

fn came(rounds: &mut [Waiting]) -> Result<Woke, Never> {
    let woken = Instant::now();

    let Some(waiting) = rounds.iter_mut().find(|waiting| waiting.due <= woken) else {
        return Ok(Woke::Nothing);
    };

    let round = waiting.round;

    waiting.due = woken.checked_add(round.every).unwrap_or(woken);

    Ok(Woke::Came(round))
}

fn listen(
    rounds: &mut Vec<Waiting>,
    words: &Listening,
    want: &Wants,
    now: Instant,
) -> Result<(), Never> {
    match want {
        Wants::Round(round) => {
            let due = now.checked_add(round.every).unwrap_or(now);

            rounds.push(Waiting { round: *round, due });
        }
        Wants::Words(topic) => {
            let Ok(()) = words.also(topic);
        }
    }

    Ok(())
}

fn deafen(rounds: &mut Vec<Waiting>, words: &Listening, want: &Wants) -> Result<(), Never> {
    match want {
        Wants::Round(round) => rounds.retain(|waiting| waiting.round != *round),
        Wants::Words(topic) => {
            let Ok(()) = words.not(topic);
        }
    }

    Ok(())
}

fn asked(called: &str, runs: &Runs) -> Result<Answer, Never> {
    let Ok(mut starting) = starting(runs);

    let said = starting.stderr(Stdio::inherit()).output();

    Ok(match said {
        Ok(said) => Answer {
            ran: runs.clone(),
            said: String::from_utf8_lossy(&said.stdout).to_string(),
            went: match said.status.success() {
                true => Went::Well,
                false => Went::Badly(said.status.code()),
            },
        },
        Err(fault) => {
            let Ok(name) = runs.program.name();

            eprintln!("{called}: {name} did not run: {fault}");

            Answer { ran: runs.clone(), said: String::new(), went: Went::Badly(None) }
        }
    })
}

fn watched(called: &str, runs: &Runs) -> Result<Answer, Never> {
    let Ok(mut starting) = starting(runs);

    let went = starting.stdout(Stdio::inherit()).stderr(Stdio::inherit()).status();

    Ok(match went {
        Ok(went) => Answer {
            ran: runs.clone(),
            said: String::new(),
            went: match went.success() {
                true => Went::Well,
                false => Went::Badly(went.code()),
            },
        },
        Err(fault) => {
            let Ok(name) = runs.program.name();

            eprintln!("{called}: {name} did not run: {fault}");

            Answer { ran: runs.clone(), said: String::new(), went: Went::Badly(None) }
        }
    })
}

fn chose(question: &Question) -> Result<Chose, Never> {
    print!("{} ", question.asks);

    let _ = std::io::Write::flush(&mut std::io::stdout());

    let mut said = String::new();
    let terminal = std::io::stdin();
    let got = std::io::BufRead::read_line(&mut terminal.lock(), &mut said);

    match got {
        Ok(0) | Err(_) => Ok(question.silence),
        Ok(_) => meant(said.trim(), question.silence),
    }
}

fn meant(said: &str, silence: Chose) -> Result<Chose, Never> {
    Ok(match said.to_lowercase().as_str() {
        "y" | "yes" => Chose::Yes,
        "n" | "no" => Chose::No,
        _ => silence,
    })
}

fn started(called: &str, runs: &Runs) -> Result<Option<LetGo>, Never> {
    let Ok(mut starting) = Theirs::SystemdRun.command();
    let Ok(name) = runs.program.name();

    starting
        .args(["--user", "--scope", "--quiet", "--collect", "--"])
        .arg(name)
        .args(&runs.argv)
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

fn reaped(running: Vec<LetGo>) -> Result<Vec<LetGo>, Never> {
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

fn starting(runs: &Runs) -> Result<Command, Never> {
    let mut starting = match runs.program {
        Named::Theirs(program) => {
            let Ok(command) = program.command();

            command
        }
        Named::Ours(name) => Command::new(name),
    };

    starting.args(&runs.argv);

    Ok(starting)
}

fn wrote(called: &str, writing: &Writing) -> Result<(), Never> {
    match std::fs::write(&writing.at, &writing.what) {
        Ok(()) => {},
        Err(fault) => eprintln!("{called}: {}: {fault}", writing.at.display()),
    }

    Ok(())
}

fn say(called: &str, saying: &Saying) -> Result<(), Never> {
    let Ok(mut telling) = Theirs::NotifySend.command();

    telling.arg(&saying.says);

    match &saying.more {
        Some(more) => {
            let _ = telling.arg(more);
        }
        None => {},
    }

    match telling.status() {
        Ok(_) => {},
        Err(fault) => eprintln!("{called}: nothing was said on the screen: {fault}"),
    }

    Ok(())
}
