//! A command line, read against what the program said it takes.
//!
//! Every program here read the words it was started with by hand, and every
//! one of them spelled its flags twice: once in the code that looked for them
//! and once in a usage line written beside it, which is two readings of one
//! thing and the drift `CLAUDE.md` warns about. The drift was real. The guide
//! said `console-check --stage device --dry` while the program looked for
//! `--dry-run`, and a flag nobody declared was not refused but passed over in
//! silence -- so `--stage device --dyr-run --yes`, one slip of a finger, was a
//! run on someone's handheld rather than a list of what it would do.
//!
//! effect's command-line module builds the parser, the help and the errors
//! from one definition of what a command takes, and that is taken whole. A
//! program declares each `Flag` once, as a constant, and the `Command` that
//! lists them; `read` turns the words it starts with into a
//! `CommandLine` or a `ValidationError`, and the program asks the line about the same
//! constants it declares, so no call site spells a flag at all. The
//! declaration draws the usage, which cannot disagree with it, and `read`
//! refuses a word that names no flag in it and prints the usage under it.
//!
//! A program that does one of several things -- `up`, `down`, `get` -- says
//! so with the enum it already has, the one whose variants carry their words
//! under `#[derive(Words)]`, by naming it as its `Subcommand`. The list and
//! the sentence for each come off the enum, so a new variant is in the help
//! the moment it exists.
//!
//! What comes after the flags is declared the same way. `Named` is a fixed
//! list of operands, every one of them needed, and `read` refuses a line with
//! one missing or one over, so the usage says `ARCHIVE` rather than the
//! `[ARCHIVE...]` it had to say before there was a way to say one. `Optional`
//! and `Any` leave the count to the program, and `exactly` is where it asks
//! for one: `console-machine` takes one value or two by which setting it is
//! told, and `console-sound-effect` a sound or `--list`.
//!
//! `run_main` is a program's edge, written once. A program's `main` reads its
//! words, hands them to the function that decides what they ask, and runs
//! what that answers; a refusal is printed with the usage under it and is the
//! exit code, and a fault is printed under the program's name and is a
//! failure. Every program wrote those lines for itself, which made every
//! `main` the same function in another place -- effect's `runMain` is the
//! one place instead.
//!
//! `read` takes only a word that begins with two dashes for a flag, and `-h`. A
//! single dash is somebody's operand -- `-5` to a level, `-p` to a command
//! it hands on -- and `--` ends the flags for anything that has to start
//! with one. `read` does not take `--name=value`: one spelling for each thing is the
//! whole point, and every program here has always written the value as the
//! next word.

use std::collections::BTreeMap;
use std::fmt;
use std::process::{ExitCode, Termination};
use std::str::FromStr;

use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const LONG: &str = "--";

const HELPED: u8 = 0;

const MISUSED: u8 = 2;

const NO_ROWS: u32 = 0;

const HELP: Flag = Flag { spelling: "--help", takes: Takes::None, about: "what this program takes, which is this" };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Takes {
    None,
    Value(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flag {
    pub spelling: &'static str,
    pub takes: Takes,
    pub about: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operands {
    None,
    Optional(&'static str),
    Named(&'static [&'static str]),
    Any(&'static str),
    Verbatim(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    pub name: &'static str,
    pub about: &'static str,
    pub flags: &'static [Flag],
    pub operands: Operands,
}

pub trait Subcommand: Copy + 'static {
    fn variants() -> Result<impl Iterator<Item = Self>, Never>;

    fn spelling(self) -> Result<&'static str, Never>;

    fn about(self) -> Result<&'static str, Never>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoSubcommand {}

impl Subcommand for NoSubcommand {
    fn variants() -> Result<impl Iterator<Item = Self>, Never> {
        Ok(std::iter::empty())
    }

    fn spelling(self) -> Result<&'static str, Never> {
        match self {}
    }

    fn about(self) -> Result<&'static str, Never> {
        match self {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Present,
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    HelpRequest,
    NoSuchFlag(String),
    MissingValue(&'static str),
    ExtraArgument(String),
    NoSuchSubcommand(String),
    MissingSubcommand,
    MissingFlag(Vec<&'static str>),
    MissingOperands(Vec<&'static str>),
    InvalidValue { of: &'static str, value: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub reason: Reason,
    pub name: &'static str,
    pub usage: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLine<S> {
    subcommand: Option<S>,
    values: BTreeMap<&'static str, Vec<String>>,
    operands: Vec<String>,
    name: &'static str,
    usage: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expecting {
    Anything,
    ValueOf(&'static str),
    OnlyOperands,
}

struct Reading<'a, S> {
    command: &'a Command,
    flags: BTreeMap<&'static str, Flag>,
    subcommands: BTreeMap<&'static str, S>,
}

struct Progress<S> {
    expecting: Expecting,
    line: CommandLine<S>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Entry<'a> {
    flag: &'static str,
    value: &'a str,
}

pub fn read<W: AsRef<str>>(command: &Command, words: &[W]) -> Result<CommandLine<NoSubcommand>, ValidationError> {
    read_with(command, words)
}

pub fn read_with<S: Subcommand, W: AsRef<str>>(command: &Command, words: &[W]) -> Result<CommandLine<S>, ValidationError> {
    let Ok(usage) = usage::<S>(command);
    let Ok(every) = S::variants();
    let name = command.name;
    let subcommands = every
        .map(|subcommand| {
            let Ok(spelling) = subcommand.spelling();

            (spelling, subcommand)
        })
        .collect();
    let flags = command.flags.iter().map(|flag| (flag.spelling, *flag)).collect();
    let reading = Reading { command, flags, subcommands };
    let line = CommandLine { subcommand: None, values: BTreeMap::new(), operands: Vec::new(), name, usage: usage.clone() };
    let start = Progress { expecting: Expecting::Anything, line };
    let walk = words.iter().try_fold(start, |read, word| reading.word(read, word.as_ref()));
    let outcome = walk.and_then(|read| match read.expecting {
        Expecting::ValueOf(spelling) => Err(Reason::MissingValue(spelling)),
        Expecting::Anything | Expecting::OnlyOperands => reading.complete(read.line),
    });

    outcome.map_err(|reason| ValidationError { reason, name, usage })
}

impl<S: Subcommand> Reading<'_, S> {
    fn word(&self, read: Progress<S>, word: &str) -> Result<Progress<S>, Reason> {
        match read.expecting {
            Expecting::ValueOf(flag) => {
                let Ok(progress) = record(read, Entry { flag, value: word });

                Ok(progress)
            }
            Expecting::OnlyOperands => self.operand(read, word),
            Expecting::Anything => match word {
                "--" => Ok(Progress { expecting: Expecting::OnlyOperands, ..read }),
                "--help" | "-h" => Err(Reason::HelpRequest),
                _flag_or_operand => match word.starts_with(LONG) {
                    true => self.flag(read, word),
                    false => self.subcommand_or_operand(read, word),
                },
            },
        }
    }

    fn flag(&self, read: Progress<S>, word: &str) -> Result<Progress<S>, Reason> {
        let flag = match self.flags.get(word) {
            Some(flag) => flag,
            None => return Err(Reason::NoSuchFlag(word.to_string())),
        };

        Ok(match flag.takes {
            Takes::None => {
                let mut line = read.line;

                line.values.entry(flag.spelling).or_default();

                Progress { line, ..read }
            }
            Takes::Value(_placeholder) => Progress { expecting: Expecting::ValueOf(flag.spelling), ..read },
        })
    }

    fn subcommand_or_operand(&self, read: Progress<S>, word: &str) -> Result<Progress<S>, Reason> {
        match (self.subcommands.is_empty(), read.line.subcommand, read.line.operands.is_empty()) {
            (false, None, true) => {
                let subcommand = match self.subcommands.get(word) {
                    Some(subcommand) => *subcommand,
                    None => return Err(Reason::NoSuchSubcommand(word.to_string())),
                };
                let line = CommandLine { subcommand: Some(subcommand), ..read.line };
                let Ok(expecting) = self.after_the_first();

                Ok(Progress { expecting, line })
            }
            (true, _, _) | (false, Some(_), _) | (false, None, false) => self.operand(read, word),
        }
    }

    fn operand(&self, read: Progress<S>, word: &str) -> Result<Progress<S>, Reason> {
        match self.command.operands {
            Operands::None => Err(Reason::ExtraArgument(word.to_string())),
            Operands::Optional(_name) => match read.line.operands.is_empty() {
                true => self.keep(read, word),
                false => Err(Reason::ExtraArgument(word.to_string())),
            },
            Operands::Named(names) => match read.line.operands.len() < names.len() {
                true => self.keep(read, word),
                false => Err(Reason::ExtraArgument(word.to_string())),
            },
            Operands::Any(_name) | Operands::Verbatim(_name) => self.keep(read, word),
        }
    }

    fn complete(&self, line: CommandLine<S>) -> Result<CommandLine<S>, Reason> {
        match (self.subcommands.is_empty(), line.subcommand) {
            (false, None) => return Ok(line),
            (true, _) | (false, Some(_)) => {},
        }

        match self.command.operands {
            Operands::Named(names) => {
                let missing: Vec<&'static str> = names.iter().skip(line.operands.len()).copied().collect();

                match missing.is_empty() {
                    true => Ok(line),
                    false => Err(Reason::MissingOperands(missing)),
                }
            }
            Operands::None | Operands::Optional(_) | Operands::Any(_) | Operands::Verbatim(_) => Ok(line),
        }
    }

    fn keep(&self, read: Progress<S>, word: &str) -> Result<Progress<S>, Reason> {
        let mut line = read.line;

        line.operands.push(word.to_string());

        let Ok(after) = self.after_the_first();
        let expecting = match read.expecting {
            Expecting::OnlyOperands => Expecting::OnlyOperands,
            Expecting::Anything | Expecting::ValueOf(_) => after,
        };

        Ok(Progress { expecting, line })
    }

    fn after_the_first(&self) -> Result<Expecting, Never> {
        Ok(match self.command.operands {
            Operands::Verbatim(_name) => Expecting::OnlyOperands,
            Operands::None | Operands::Optional(_) | Operands::Named(_) | Operands::Any(_) => Expecting::Anything,
        })
    }
}

fn record<S>(read: Progress<S>, entry: Entry<'_>) -> Result<Progress<S>, Never> {
    let mut line = read.line;

    line.values.entry(entry.flag).or_default().push(entry.value.to_string());

    Ok(Progress { expecting: Expecting::Anything, line })
}

impl<S: Copy> CommandLine<S> {
    pub fn subcommand(&self) -> Result<Option<S>, Never> {
        Ok(self.subcommand)
    }

    pub fn presence(&self, flag: Flag) -> Result<Presence, Never> {
        Ok(match self.values.contains_key(flag.spelling) {
            true => Presence::Present,
            false => Presence::Absent,
        })
    }

    pub fn value(&self, flag: Flag) -> Result<Option<&str>, Never> {
        Ok(self.values.get(flag.spelling).and_then(|values| values.last()).map(String::as_str))
    }

    pub fn values(&self, flag: Flag) -> Result<&[String], Never> {
        Ok(match self.values.get(flag.spelling) {
            Some(values) => values.as_slice(),
            None => &[],
        })
    }

    pub fn operands(&self) -> Result<&[String], Never> {
        Ok(&self.operands)
    }

    pub fn require_subcommand(&self) -> Result<S, ValidationError> {
        match self.subcommand {
            Some(subcommand) => Ok(subcommand),
            None => {
                let Ok(refusal) = self.refusal(Reason::MissingSubcommand);

                Err(refusal)
            }
        }
    }

    pub fn one_of(&self, choices: &[Flag]) -> Result<Flag, ValidationError> {
        let mut present = choices.iter().filter(|flag| self.values.contains_key(flag.spelling));

        let choice = match (present.next(), present.next()) {
            (Some(flag), None) => Ok(*flag),
            (Some(_flag), Some(second)) => Err(Reason::ExtraArgument(second.spelling.to_string())),
            (None, _nothing) => Err(Reason::MissingFlag(choices.iter().map(|flag| flag.spelling).collect())),
        };

        choice.map_err(|reason| {
            let Ok(refusal) = self.refusal(reason);

            refusal
        })
    }

    pub fn exactly<const N: usize>(&self, named: [&'static str; N]) -> Result<&[String; N], ValidationError> {
        let count = match self.operands.split_first_chunk() {
            Some((exactly, [])) => Ok(exactly),
            Some((_exactly, [extra, ..])) => Err(Reason::ExtraArgument(extra.clone())),
            None => Err(Reason::MissingOperands(named.iter().skip(self.operands.len()).copied().collect())),
        };

        count.map_err(|reason| {
            let Ok(refusal) = self.refusal(reason);

            refusal
        })
    }

    pub fn parsed<T: FromStr>(&self, flag: Flag) -> Result<Option<T>, ValidationError> {
        let Ok(text) = self.value(flag);

        match text {
            None => Ok(None),
            Some(text) => match text.parse::<T>() {
                Ok(read) => Ok(Some(read)),
                Err(_invalid) => {
                    let Ok(refusal) = self.refusal(Reason::InvalidValue { of: flag.spelling, value: text.to_string() });

                    Err(refusal)
                }
            },
        }
    }

    pub fn refusal(&self, reason: Reason) -> Result<ValidationError, Never> {
        Ok(ValidationError { reason, name: self.name, usage: self.usage.clone() })
    }
}

impl ValidationError {
    pub fn print(&self) -> Result<u8, Never> {
        Ok(match self.reason {
            Reason::HelpRequest => {
                println!("{}", self.usage);

                HELPED
            }
            Reason::NoSuchFlag(_)
            | Reason::MissingValue(_)
            | Reason::ExtraArgument(_)
            | Reason::NoSuchSubcommand(_)
            | Reason::MissingSubcommand
            | Reason::MissingFlag(_)
            | Reason::MissingOperands(_)
            | Reason::InvalidValue { .. } => {
                eprintln!("{self}\n\n{}", self.usage);

                MISUSED
            }
        })
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.name;

        match &self.reason {
            Reason::HelpRequest => write!(to, "{name}: a request for the usage"),
            Reason::NoSuchFlag(word) => write!(to, "{name}: {word} is not a flag {name} takes"),
            Reason::MissingValue(flag) => write!(to, "{name}: {flag} takes a value, and nothing comes after it"),
            Reason::ExtraArgument(word) => write!(to, "{name}: {word} is one word more than {name} takes"),
            Reason::NoSuchSubcommand(word) => write!(to, "{name}: {word} is not one of the things {name} does"),
            Reason::MissingSubcommand => write!(to, "{name}: {name} needs to hear which of its things to do"),
            Reason::MissingFlag(choices) => write!(to, "{name}: {name} needs one of {}", choices.join(", ")),
            Reason::MissingOperands(named) => write!(to, "{name}: {name} needs {}", named.join(" and ")),
            Reason::InvalidValue { of, value } => write!(to, "{name}: {value} is not something {of} can be"),
        }
    }
}

impl std::error::Error for ValidationError {}

pub fn run_main<T, R: Termination, F: fmt::Display>(
    command: &Command,
    words: &[String],
    decide: impl FnOnce(&[String]) -> Result<T, ValidationError>,
    program: impl FnOnce(T) -> Result<R, F>,
) -> Result<ExitCode, Never> {
    let decided = decide(words);

    Ok(match decided {
        Err(refusal) => {
            let Ok(code) = refusal.print();

            ExitCode::from(code)
        }
        Ok(asked) => match program(asked) {
            Ok(ended) => ended.report(),
            Err(fault) => {
                eprintln!("{}: {fault}", command.name);

                ExitCode::FAILURE
            }
        },
    })
}

pub fn usage<S: Subcommand>(command: &Command) -> Result<String, Never> {
    let Ok(every) = S::variants();
    let subcommands: Vec<(String, &'static str)> = every
        .map(|subcommand| {
            let Ok(spelling) = subcommand.spelling();
            let Ok(about) = subcommand.about();

            (spelling.to_string(), about)
        })
        .collect();
    let flags: Vec<(String, &'static str)> = command
        .flags
        .iter()
        .chain([HELP].iter())
        .map(|flag| {
            let Ok(spelling) = spelling(*flag);

            (spelling, flag.about)
        })
        .collect();
    let Ok(synopsis) = synopsis(command, &subcommands);
    let Ok(widest) = widest(subcommands.iter().chain(flags.iter()));
    let Ok(subcommand_rows) = section("subcommands", &subcommands, widest);
    let Ok(flag_rows) = section("flags", &flags, widest);

    Ok([synopsis, command.about.to_string(), subcommand_rows, flag_rows]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<String>>()
        .join("\n\n"))
}

fn spelling(flag: Flag) -> Result<String, Never> {
    Ok(match flag.takes {
        Takes::None => flag.spelling.to_string(),
        Takes::Value(placeholder) => format!("{} {placeholder}", flag.spelling),
    })
}

fn synopsis(command: &Command, subcommands: &[(String, &'static str)]) -> Result<String, Never> {
    let subcommand = match subcommands.is_empty() {
        true => String::new(),
        false => " [SUBCOMMAND]".to_string(),
    };
    let flags: String = command
        .flags
        .iter()
        .map(|flag| {
            let Ok(spelling) = spelling(*flag);

            format!(" [{spelling}]")
        })
        .collect();
    let operands = match command.operands {
        Operands::None => String::new(),
        Operands::Optional(name) => format!(" [{name}]"),
        Operands::Named(names) => names.iter().map(|name| format!(" {name}")).collect(),
        Operands::Any(name) | Operands::Verbatim(name) => format!(" [{name}...]"),
    };

    Ok(format!("usage: {}{subcommand}{flags}{operands}", command.name))
}

fn widest<'a>(rows: impl Iterator<Item = &'a (String, &'static str)>) -> Result<u32, Never> {
    let widest = rows
        .map(|(left, _about)| {
            let Ok(wide) = fitted::<_, u32>(left.chars().count());

            wide
        })
        .max();

    Ok(match widest {
        Some(widest) => widest,
        None => NO_ROWS,
    })
}

fn section(heading: &str, rows: &[(String, &'static str)], widest: u32) -> Result<String, Never> {
    let Ok(width) = index(widest);
    let lines: Vec<String> = rows.iter().map(|(left, about)| format!("  {left:<width$}  {about}")).collect();

    Ok(match lines.is_empty() {
        true => String::new(),
        false => format!("{heading}:\n{}", lines.join("\n")),
    })
}

#[cfg(test)]
mod tests;
