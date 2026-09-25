//! Which hearing this device should be using, measured on this device.
//!
//! The model was chosen because it worked, which is not the same as having
//! been compared against anything, and `docs/voice.md` names two things it was
//! never asked. Turbo is large-v3 with the decoder cut from thirty-two layers
//! to four, and OpenAI's own note on it is that it holds up everywhere except
//! a few languages, of which Thai is one -- and Thai is a third of what is said
//! to this device. Qwen3-ASR is the first local model since whisper that hears
//! all three of these languages, and llama.cpp can run it on this machine's
//! graphics. Both are answered by saying sixteen things into it once.
//!
//! ## What is measured, and what is not
//!
//! Not a benchmark. A benchmark is a corpus someone else recorded, read by
//! someone being careful, and what it measures is not this. What is here is
//! the few things this paddle is actually asked to do and the few places it is
//! known to be able to fail. The word clips matter more than the sentences,
//! which is the opposite of how transcription is usually measured, because
//! nearly everything said to this button is one or two words into a search box.
//!
//! Three runs, and the best of them kept beside the first. The first run of
//! anything here pays for a model coming off the disk and shaders being
//! compiled, which is a real cost exactly once per boot and not the cost of a
//! press: the first is what the first press of the day feels like and the best
//! is what every press after it feels like.
//!
//! ## Why so much of it is [`CompareEffect`]
//!
//! Three of these effects are no one else's. The engines are binaries at paths
//! this program worked out rather than programs on the machine, which
//! [`console_program_contract::Command`] has no way to name; a run has to be
//! *timed*, and an answer carries how it went rather than how long it took;
//! and a recording is a child that is started by one press and ended by the
//! next. Every one of those is still a value decided before it happens, so a
//! transcript still says what a run would do -- which is the whole of what this
//! split buys, and it is why the parsing below is a function rather than a
//! `sed` in a pipeline.
//!
//! Nothing here is part of the desktop. It fetches into the same directory
//! `console-dictate` keeps its model in, it is run by hand, and what it leaves
//! behind can be deleted.

use std::path::{Path, PathBuf};

use console_core_external_programs::Program as ExternalProgram;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const NOTHING_NAMED: &str = "";

const NOTHING_GUESSED: &str = "none";

use console_program_contract::{
    Arguments, Choice, Effect, Exit, Initial, Program, Prompt, Command, Update, ExitStatus, Event, FileWrite,
};

pub const THREADS: &str = crate::THREADS;

pub const WHISPER_FROM: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

pub const QWEN_FROM: &str = "https://huggingface.co/ggml-org/Qwen3-ASR-1.7B-GGUF/resolve/main";

pub const QWEN_MMPROJ: &str = "mmproj-Qwen3-ASR-1.7B-Q8_0.gguf";

pub const LLAMA_FROM: &str = "https://github.com/ggml-org/llama.cpp";

pub const LLAMA_AT: &str = "v0.3.0";

pub const DETECTING: &str = "auto-detected language: ";

pub const CARD: &str = "ggml_vulkan: ";

pub struct Clip {
    pub name: &'static str,
    pub language: &'static str,
    pub says: &'static str,
}

pub const CLIPS: [Clip; 16] = [
    Clip { name: "en-word", language: "en", says: "ONE English word, as into a search box -- \"Settings\"" },
    Clip { name: "nl-word", language: "nl", says: "ONE Dutch word with an English word behind it -- \"zeven\". If \"seven\" comes back, detection went to English where you can watch it happen" },
    Clip { name: "th-word", language: "th", says: "ONE short Thai thing -- a greeting, a thank you" },
    Clip { name: "en-line", language: "en", says: "an English sentence, ten words or so, at ordinary speed" },
    Clip { name: "nl-line", language: "nl", says: "a Dutch sentence, ten words or so, at ordinary speed" },
    Clip { name: "th-line", language: "th", says: "a Thai sentence, ten words or so, at ordinary speed" },
    Clip { name: "th-tones", language: "th", says: "three words differing only in tone, with a gap between them -- dog, horse, come. The clip turbo is expected to lose on" },
    Clip { name: "th-marks", language: "th", says: "two or three words carrying the marks that are not letters: a silent-letter word, a sara am word" },
    Clip { name: "nl-hard", language: "nl", says: "a Dutch sentence heavy in ui, eu and oo -- \"Het huis heeft een nieuwe voordeur\"" },
    Clip { name: "nl-mixed", language: "nl", says: "a Dutch sentence with the English words left in, the way it is really spoken -- \"Ik heb de update gedownload naar mijn desktop\". This is what pinning the language costs" },
    Clip { name: "th-mixed", language: "th", says: "the same the other way: a Thai sentence with an English word or two in it" },
    Clip { name: "en-name", language: "en", says: "a name or a filename rather than a sentence -- what this paddle is mostly pressed for" },
    Clip { name: "nl-number", language: "nl", says: "a number and a date said aloud in Dutch. Models disagree wildly about digits" },
    Clip { name: "quiet", language: "en", says: "anything, said quietly, at arm length. This is the guard that decides whether a recording is speech at all" },
    Clip { name: "pause", language: "en", says: "a sentence with a two-second silence through the middle of it" },
    Clip { name: "nothing", language: "en", says: "say NOTHING. Start, wait three seconds, stop. Whisper answers an empty room with \"Thank you.\" and this is the guard that stops it" },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Whisper,
    Llama,
}

pub struct Model {
    pub name: &'static str,
    pub engine: Engine,
    pub file: &'static str,
}

pub const MODELS: [Model; 4] = [
    Model { name: "turbo-q5", engine: Engine::Whisper, file: "ggml-large-v3-turbo-q5_0.bin" },
    Model { name: "turbo", engine: Engine::Whisper, file: "ggml-large-v3-turbo.bin" },
    Model { name: "large-v3", engine: Engine::Whisper, file: "ggml-large-v3.bin" },
    Model { name: "qwen3-asr", engine: Engine::Llama, file: "Qwen3-ASR-1.7B-Q8_0.gguf" },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Where {
    pub kept: PathBuf,
    pub host: String,
    pub stamp: String,
}

impl Where {
    pub fn made(&self) -> Result<PathBuf, Never> {
        Ok(self.kept.join("compare"))
    }

    pub fn clips(&self) -> Result<PathBuf, Never> {
        let Ok(made) = self.made();

        Ok(made.join("clips"))
    }

    pub fn clip(&self, name: &str) -> Result<PathBuf, Never> {
        let Ok(clips) = self.clips();

        Ok(clips.join(format!("{name}.wav")))
    }

    pub fn whisper(&self) -> Result<PathBuf, Never> {
        Ok(self.kept.join("whisper-cli"))
    }

    pub fn llama(&self) -> Result<PathBuf, Never> {
        Ok(self.kept.join("llama-mtmd-cli"))
    }

    pub fn model(&self, file: &str) -> Result<PathBuf, Never> {
        Ok(self.kept.join(file))
    }

    pub fn report(&self) -> Result<PathBuf, Never> {
        let Ok(made) = self.made();

        Ok(made.join(format!("said-{}.txt", self.stamp)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    Record,
    Models,
    Build,
    Fetch,
    Compare,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareEffect {
    Look(Vec<PathBuf>),
    Ran(Vec<String>),
    Timed(Vec<String>),
    Record(PathBuf),
    Enough,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareEvent {
    Looked(Vec<Candidate>),
    Output(String),
    Timed(Timing),
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub at: PathBuf,
    pub is: Found,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    Absent,
    There,
    Runnable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timing {
    pub first: u128,
    pub best: u128,
    pub said: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordingPhase {
    Waiting,
    Recording,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Work {
    Detect(u32),
    Hear(Hear),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hear {
    pub clip: u32,
    pub model: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Error(String),
    Initial,
    Sizing,
    Fetching(Vec<(String, PathBuf)>),
    Getting(Vec<(String, PathBuf)>),
    Moving(Vec<(String, PathBuf)>),
    Locating,
    Clearing,
    Cloning,
    Setting,
    Compiling,
    Copying,
    Naming,
    Sweeping,
    Recording(Vec<u32>, RecordingPhase),
    Looking,
    DetectingBackend,
    Working(Vec<Work>),
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparing {
    pub job: Task,
    pub step: Step,
    pub at: Where,
    pub there: Vec<Candidate>,
    pub report: Vec<String>,
}

pub struct Compare;

impl Program for Compare {
    type State = Comparing;
    type Event = CompareEvent;
    type Effect = CompareEffect;

    fn init(arguments: &Arguments) -> Initial<Comparing> {
        let Ok(words) = arguments.words();

        let said = |at: u32| {
            let Ok(at) = index(at);

            match words.get(at).cloned() {
                Some(said) => said,
                None => String::new(),
            }
        };
        let at = Where { kept: PathBuf::from(said(0)), host: said(1), stamp: said(2) };

        let Ok(job) = asked_for(words.get(3).map(String::as_str));
        let Ok(step) = work(words.get(3).map(String::as_str));
        let Ok(opening) =
            Initial::new(Comparing { job, step, at, there: Vec::new(), report: Vec::new() });

        opening
    }

    fn update(state: &Comparing, event: &Event<CompareEvent>) -> Update<Comparing, CompareEffect> {
        let turn = match (&state.step, event) {
            (Step::Error(said), Event::Opened) => badly(
                state,
                &format!("{said} is not a word this takes: --record, --models, --build, --fetch"),
            ),

            (Step::Initial, Event::Opened) => begun(state),

            (Step::Sizing, Event::Custom(CompareEvent::Looked(seen))) => {
                let state = Comparing { there: seen.to_vec(), ..state.clone() };
                let Ok(left) = missing(&state);

                fetching(&state, &left)
            }

            (Step::Fetching(left), _) => fetching(state, left),

            (Step::Getting(left), Event::Replied(answer)) => match answer.status {
                ExitStatus::Success => {
                    let Ok(stepped) = stepped(state, Step::Moving(left.clone()));
                    let Ok(moved) = moved(left);

                    Update::new(stepped, vec![Effect::Run(moved)])
                }
                ExitStatus::Failure(_) => badly(state, "a model would not come down"),
            },

            (Step::Moving(left), Event::Replied(_)) => {
                fetching(state, &left.iter().skip(1).cloned().collect::<Vec<_>>())
            }

            (Step::Locating, Event::Custom(CompareEvent::Looked(seen))) => built(state, seen),

            (Step::Clearing, Event::Replied(_)) => {
                let Ok(stepped) = stepped(state, Step::Cloning);
                let Ok(making) = making(&state.at);
                let Ok(shown) = shown(&making);
                let Ok(making) = Command::external(ExternalProgram::Mkdir, &["-p", &shown]);

                Update::new(stepped, vec![Effect::Stream(making)])
            }

            (Step::Cloning, Event::Replied(_)) => {
                let Ok(stepped) = stepped(state, Step::Setting);
                let Ok(cloning) = cloning(&state.at);

                Update::new(stepped, vec![Effect::Stream(cloning)])
            }

            (Step::Setting, Event::Replied(answer)) => match answer.status {
                ExitStatus::Failure(_) => badly(state, "llama.cpp would not come down"),
                ExitStatus::Success => {
                    let Ok(stepped) = stepped(state, Step::Compiling);
                    let Ok(configuring) = configuring(&state.at);

                    Update::new(stepped, vec![Effect::Stream(configuring)])
                }
            },

            (Step::Compiling, Event::Replied(answer)) => match answer.status {
                ExitStatus::Failure(_) => badly(state, "the second engine would not configure"),
                ExitStatus::Success => {
                    let Ok(stepped) = stepped(state, Step::Copying);
                    let Ok(compiling) = compiling(&state.at);

                    Update::new(stepped, vec![Effect::Stream(compiling)])
                }
            },

            (Step::Copying, Event::Replied(answer)) => match answer.status {
                ExitStatus::Failure(_) => badly(state, "the second engine would not build"),
                ExitStatus::Success => {
                    let Ok(stepped) = stepped(state, Step::Naming);
                    let Ok(made) = made(&state.at);
                    let Ok(from) = shown(&made);
                    let Ok(llama) = state.at.llama();
                    let Ok(into) = coming(&llama);
                    let Ok(copying) = Command::external(ExternalProgram::Cp, &[&from, &into]);

                    Update::new(stepped, vec![Effect::Run(copying)])
                }
            },

            (Step::Naming, Event::Replied(_)) => {
                let Ok(stepped) = stepped(state, Step::Sweeping);
                let Ok(llama) = state.at.llama();
                let Ok(from) = coming(&llama);
                let Ok(into) = shown(&llama);
                let Ok(naming) = Command::external(ExternalProgram::Mv, &[&from, &into]);

                Update::new(stepped, vec![Effect::Run(naming)])
            }

            (Step::Sweeping, Event::Replied(_)) => {
                let Ok(stepped) = stepped(state, Step::Finished);
                let Ok(making) = making(&state.at);
                let Ok(shown) = shown(&making);
                let Ok(sweeping) = Command::external(ExternalProgram::Rm, &["-rf", &shown]);

                Update::new(stepped, vec![
                    Effect::Run(sweeping),
                    Effect::Print("   built".to_string()),
                ])
            }

            (Step::Finished, Event::Replied(_)) => after(state),

            (Step::Recording(left, ear), word) => recording(state, left, ear, word),

            (Step::Looking, Event::Custom(CompareEvent::Looked(seen))) => looked(state, seen),

            (Step::DetectingBackend, Event::Custom(CompareEvent::Output(said))) => backend_detected(state, said),

            (Step::Working(left), word) => working(state, left, word),

            (_, _) => Update::none(state.clone()),
        };

        let Ok(turn) = turn;

        turn
    }
}

fn work(said: Option<&str>) -> Result<Step, Never> {
    Ok(match said {
        None => Step::Initial,
        Some("--record" | "--models" | "--build" | "--fetch") => Step::Initial,
        Some(said) => Step::Error(said.to_string()),
    })
}

pub fn asked_for(said: Option<&str>) -> Result<Task, Never> {
    Ok(match said {
        Some("--record") => Task::Record,
        Some("--models") => Task::Models,
        Some("--build") => Task::Build,
        Some("--fetch") => Task::Fetch,
        Some(_) | None => Task::Compare,
    })
}

fn begun(state: &Comparing) -> Result<Update<Comparing, CompareEffect>, Never> {
    match state.job {
        Task::Record => {
            let Ok(clips) = fitted::<_, u32>(CLIPS.len());
            let Ok(stepped) = stepped(state, Step::Recording((0..clips).collect(), RecordingPhase::Waiting));
            let Ok(clips) = state.at.clips();
            let Ok(shown) = shown(&clips);
            let Ok(asking) = asking(0);
            let Ok(making) = Command::external(ExternalProgram::Mkdir, &["-p", &shown]);

            Update::new(
                stepped,
                std::iter::once(Effect::Run(making))
                    .chain(PREAMBLE.iter().map(|line| Effect::Print((*line).to_string())))
                    .chain(asking)
                    .collect(),
            )
        }

        Task::Models | Task::Fetch => {
            let Ok(stepped) = stepped(state, Step::Sizing);
            let Ok(shown) = shown(&state.at.kept);
            let Ok(fetches) = fetches(&state.at);
            let Ok(making) = Command::external(ExternalProgram::Mkdir, &["-p", &shown]);

            Update::new(stepped, vec![
                Effect::Run(making),
                Effect::Print("== the models".to_string()),
                Effect::Custom(CompareEffect::Look(fetches.iter().map(|(_, into)| into.clone()).collect())),
            ])
        }

        Task::Build => asked(state),

        Task::Compare => {
            let Ok(stepped) = stepped(state, Step::Looking);
            let Ok(everything) = everything(&state.at);

            Update::new(stepped, vec![Effect::Custom(CompareEffect::Look(everything))])
        }
    }
}

fn after(state: &Comparing) -> Result<Update<Comparing, CompareEffect>, Never> {
    Update::new(state.clone(), vec![Effect::Stop(Exit::Success)])
}

fn fetching(
    state: &Comparing,
    left: &[(String, PathBuf)],
) -> Result<Update<Comparing, CompareEffect>, Never> {
    match left.first() {
        None => match state.job {
            Task::Fetch => {
                let Ok(stepped) = stepped(state, Step::Locating);
                let Ok(llama) = state.at.llama();

                Update::new(stepped, vec![
                    Effect::Print("\n== the second engine".to_string()),
                    Effect::Custom(CompareEffect::Look(vec![llama])),
                ])
            }
            Task::Record | Task::Models | Task::Build | Task::Compare => after(state),
        },
        Some((from, into)) => {
            let Ok(stepped) = stepped(state, Step::Getting(left.to_vec()));
            let Ok(named) = named(into);
            let Ok(getting) = getting(from, into);

            Update::new(stepped, vec![
                Effect::Print(format!("   fetching {named}")),
                Effect::Stream(getting),
            ])
        }
    }
}

fn asked(state: &Comparing) -> Result<Update<Comparing, CompareEffect>, Never> {
    let Ok(stepped) = stepped(state, Step::Locating);
    let Ok(llama) = state.at.llama();

    Update::new(stepped, vec![
        Effect::Print("== the second engine".to_string()),
        Effect::Custom(CompareEffect::Look(vec![llama])),
    ])
}

fn built(state: &Comparing, seen: &[Candidate]) -> Result<Update<Comparing, CompareEffect>, Never> {
    let is = seen.first().map(|seen| seen.is);

    match is {
        Some(Found::Runnable) => {
            let Ok(stepped) = stepped(state, Step::Finished);

            Update::new(stepped, vec![
                Effect::Print("   already built".to_string()),
                Effect::Stop(Exit::Success),
            ])
        }
        Some(Found::Absent | Found::There) | None => {
            let Ok(stepped) = stepped(state, Step::Clearing);
            let Ok(making) = making(&state.at);
            let Ok(shown) = shown(&making);
            let Ok(clearing) = Command::external(ExternalProgram::Rm, &["-rf", &shown]);

            Update::new(stepped, vec![
                Effect::Print(format!(
                    "   llama.cpp {LLAMA_AT}, pointed at this machine's graphics.\n   \
                     This is a C++ project and a handheld. It takes a while."
                )),
                Effect::Run(clearing),
            ])
        }
    }
}

fn recording(
    state: &Comparing,
    left: &[u32],
    ear: &RecordingPhase,
    event: &Event<CompareEvent>,
) -> Result<Update<Comparing, CompareEffect>, Never> {
    let at = left.first().copied();

    match (at, ear, event) {
        (Some(at), RecordingPhase::Waiting, Event::Chosen(_)) => {
            let Ok(stepped) = stepped(state, Step::Recording(left.to_vec(), RecordingPhase::Recording));
            let Ok(name) = name(at);
            let Ok(clip) = state.at.clip(name);
            let Ok(asking) = Prompt::unless("  listening, ENTER to stop", Choice::Yes);

            Update::new(stepped, vec![
                Effect::Custom(CompareEffect::Record(clip)),
                Effect::Prompt(asking),
            ])
        }

        (Some(_), RecordingPhase::Recording, Event::Chosen(_)) => {
            let rest: Vec<u32> = left.iter().skip(1).copied().collect();
            let Ok(stepped) = stepped(state, Step::Recording(rest.clone(), RecordingPhase::Waiting));
            let next = match rest.first() {
                Some(next) => {
                    let Ok(asking) = asking(*next);

                    asking
                }
                None => vec![
                    Effect::Print("\nRecorded. Now: cargo run --bin voice-compare".to_string()),
                    Effect::Stop(Exit::Success),
                ],
            };

            Update::new(
                stepped,
                std::iter::once(Effect::Custom(CompareEffect::Enough))
                    .chain(std::iter::once(Effect::Print("  kept".to_string())))
                    .chain(next)
                    .collect(),
            )
        }

        (_, _, _) => Update::none(state.clone()),
    }
}

fn asking(at: u32) -> Result<Vec<Effect<CompareEffect>>, Never> {
    let Ok(at) = index(at);

    Ok(match CLIPS.get(at) {
        Some(clip) => {
            let Ok(starting) = Prompt::unless("  ENTER to start", Choice::Yes);

            vec![
                Effect::Print(format!("\n{} ({})\n  {}", clip.name, clip.language, clip.says)),
                Effect::Prompt(starting),
            ]
        }
        None => Vec::new(),
    })
}

fn looked(state: &Comparing, seen: &[Candidate]) -> Result<Update<Comparing, CompareEffect>, Never> {
    let state = Comparing { there: seen.to_vec(), ..state.clone() };
    let Ok(at) = state.at.whisper();
    let Ok(whisper) = is(&state, &at);

    match whisper {
        Found::Absent | Found::There => {
            let Ok(shown) = shown(&at);

            badly(
                &state,
                &format!(
                    "The hearing this machine built for itself is not there:\n  {shown}\n\
                     ButtonPress the paddle once, or run: console-dictate --fetch\n\
                     The packaged whisper-cli speaks nothing but the processor, so measuring \
                     against it would be measuring the wrong thing."
                ),
            )
        }
        Found::Runnable => {
            let Ok(recorded) = recorded(&state);

            match recorded.is_empty() {
                true => badly(
                    &state,
                    "Nothing has been recorded yet: cargo run --bin voice-compare -- --record",
                ),
                false => {
                    let Ok(stepped) = stepped(&state, Step::DetectingBackend);
                    let Ok(probe) = backend_probe(&state);

                    Update::new(stepped, vec![Effect::Custom(CompareEffect::Ran(probe))])
                }
            }
        }
    }
}

fn backend_detected(state: &Comparing, said: &str) -> Result<Update<Comparing, CompareEffect>, Never> {
    let Ok(card) = graphics(said);
    let opening = vec![
        format!("voice-compare, {}, on {}", state.at.stamp, state.at.host),
        match card {
            Some(card) => format!("graphics: {card}"),
            None => "graphics: NONE FOUND -- this ran on the processor, so the numbers below \
                     are about the wrong program"
                .to_string(),
        },
    ];
    let Ok(left) = walking(state);
    let state = Comparing { report: opening.clone(), ..state.clone() };
    let Ok(stepped) = stepped(&state, Step::Working(left.clone()));
    let Ok(going) = going(&state, &left);

    Update::new(
        stepped,
        opening.iter().map(|line| Effect::Print(line.clone())).chain(going).collect(),
    )
}

fn working(
    state: &Comparing,
    left: &[Work],
    event: &Event<CompareEvent>,
) -> Result<Update<Comparing, CompareEffect>, Never> {
    let rest: Vec<Work> = left.iter().skip(1).cloned().collect();

    let line = match (left.first(), event) {
        (Some(Work::Detect(clip)), Event::Custom(CompareEvent::Output(said))) => {
            let Ok(header) = header(*clip, said);

            Some(header)
        }
        (Some(Work::Hear(hear)), Event::Custom(CompareEvent::Timed(took))) => {
            let Ok(row) = row(hear.model, took);

            Some(row)
        }
        (_, _) => None,
    };

    match line {
        None => Update::none(state.clone()),
        Some(line) => {
            let mut report = state.report.clone();
            report.push(line.clone());

            let state = Comparing { report, ..state.clone() };
            let Ok(stepped) = stepped(&state, Step::Working(rest.clone()));
            let next = match rest.is_empty() {
                false => {
                    let Ok(going) = going(&state, &rest);

                    going
                }
                true => {
                    let Ok(written) = written(&state);

                    written
                }
            };

            Update::new(stepped, std::iter::once(Effect::Print(line)).chain(next).collect())
        }
    }
}

fn going(state: &Comparing, left: &[Work]) -> Result<Vec<Effect<CompareEffect>>, Never> {
    Ok(match left.first() {
        Some(Work::Detect(clip)) => {
            let Ok(detecting) = detecting(state, *clip);

            vec![Effect::Custom(CompareEffect::Ran(detecting))]
        }
        Some(Work::Hear(hear)) => {
            let Ok(hearing) = hearing(state, *hear);

            match hearing {
                Some(arguments) => vec![Effect::Custom(CompareEffect::Timed(arguments))],
                None => Vec::new(),
            }
        }
        None => Vec::new(),
    })
}

fn written(state: &Comparing) -> Result<Vec<Effect<CompareEffect>>, Never> {
    let Ok(at) = state.at.report();
    let Ok(shown) = shown(&at);

    Ok(vec![
        Effect::Write(FileWrite { path: at, contents: format!("{}\n", state.report.join("\n")) }),
        Effect::Print(format!("\nWritten to {shown}")),
        Effect::Print(CLOSING.to_string()),
        Effect::Stop(Exit::Success),
    ])
}

pub const PREAMBLE: [&str; 4] = [
    "Sixteen things, once each. ENTER starts, ENTER stops.",
    "Say them the way you would say them at the paddle: no slower, and no",
    "clearer. A benchmark read carefully is a benchmark of someone being",
    "careful.",
];

pub const CLOSING: &str = "\nWhat to read out of it. The times are the easy half and mostly will\n\
                           not decide anything: anything under about two seconds is a paddle\n\
                           that feels immediate. The half that decides it is the Thai lines,\n\
                           read by someone who speaks Thai. If large-v3 is better there and\n\
                           costs a few hundred milliseconds, it is the one to keep.";

pub fn fetches(at: &Where) -> Result<Vec<(String, PathBuf)>, Never> {
    let mut every = Vec::new();

    for model in &MODELS {
        let Ok(file) = at.model(model.file);

        match model.engine {
            Engine::Whisper => {
                every.push((format!("{WHISPER_FROM}/{}", model.file), file));
            }
            Engine::Llama => {
                let Ok(mmproj) = at.model(QWEN_MMPROJ);

                every.push((format!("{QWEN_FROM}/{}", model.file), file));
                every.push((format!("{QWEN_FROM}/{QWEN_MMPROJ}"), mmproj));
            }
        }
    }

    Ok(every)
}

pub fn missing(state: &Comparing) -> Result<Vec<(String, PathBuf)>, Never> {
    let Ok(fetches) = fetches(&state.at);

    Ok(fetches
        .into_iter()
        .filter(|(_, into)| {
            let Ok(is) = is(state, into);

            match is {
                Found::Absent => true,
                Found::There | Found::Runnable => false,
            }
        })
        .collect())
}

pub fn everything(at: &Where) -> Result<Vec<PathBuf>, Never> {
    let Ok(whisper) = at.whisper();
    let Ok(llama) = at.llama();
    let mut every = vec![whisper, llama];

    for model in &MODELS {
        let Ok(file) = at.model(model.file);

        every.push(file);
    }

    let Ok(mmproj) = at.model(QWEN_MMPROJ);

    every.push(mmproj);

    for clip in &CLIPS {
        let Ok(clip) = at.clip(clip.name);

        every.push(clip);
    }

    Ok(every)
}

pub fn walking(state: &Comparing) -> Result<Vec<Work>, Never> {
    let mut every = Vec::new();
    let Ok(recorded) = recorded(state);

    for clip in recorded {
        every.push(Work::Detect(clip));

        for (model, _) in MODELS.iter().enumerate() {
            let Ok(model) = fitted(model);

            every.push(Work::Hear(Hear { clip, model }));
        }
    }

    Ok(every)
}

fn recorded(state: &Comparing) -> Result<Vec<u32>, Never> {
    let mut every = Vec::new();

    for (at, clip) in CLIPS.iter().enumerate() {
        let Ok(at) = fitted(at);
        let Ok(clip) = state.at.clip(clip.name);
        let Ok(is) = is(state, &clip);

        match is {
            Found::Absent => {}
            Found::There | Found::Runnable => every.push(at),
        }
    }

    Ok(every)
}

fn is(state: &Comparing, at: &Path) -> Result<Found, Never> {
    let seen = state.there.iter().find(|seen| seen.at == at).map(|seen| seen.is);

    Ok(match seen {
        Some(is) => is,
        None => Found::Absent,
    })
}

pub fn graphics(said: &str) -> Result<Option<String>, Never> {
    Ok(said
        .lines()
        .find(|line| {
            line.starts_with(CARD)
                && line.get(CARD.len()..).is_some_and(|rest| {
                    rest.chars().next().is_some_and(|first| first.is_ascii_digit())
                })
        })
        .map(str::to_string))
}

pub fn detected(said: &str) -> Result<Option<String>, Never> {
    Ok(said
        .lines()
        .filter_map(|line| line.split_once(DETECTING))
        .map(|(_, rest)| rest.chars().take_while(char::is_ascii_lowercase).collect::<String>())
        .rfind(|word| !word.is_empty()))
}

pub fn plainly(said: &str) -> Result<String, Never> {
    Ok(said.split_whitespace().collect::<Vec<&str>>().join(" "))
}

fn header(clip: u32, said: &str) -> Result<String, Never> {
    let Ok(detected) = detected(said);
    let guessed = match detected {
        Some(guessed) => guessed,
        None => NOTHING_GUESSED.to_string(),
    };

    let Ok(spoken) = language(clip);
    let wrong = match guessed == spoken {
        true => "",
        false => "   <-- wrong",
    };

    let Ok(named) = name(clip);

    Ok(format!("\n== {named}   spoken: {spoken}   auto heard: {guessed}{wrong}"))
}

fn row(model: u32, took: &Timing) -> Result<String, Never> {
    let Ok(plainly) = plainly(&took.said);
    let Ok(model) = index(model);

    let named = match MODELS.get(model).map(|model| model.name) {
        Some(named) => named,
        None => NOTHING_NAMED,
    };

    Ok(format!("   {named:<11} {:>5} ms  (first {:>5} ms)  {plainly}", took.best, took.first))
}

fn detecting(state: &Comparing, clip: u32) -> Result<Vec<String>, Never> {
    let Ok(whisper) = state.at.whisper();
    let Ok(turbo) = turbo();
    let Ok(model) = state.at.model(turbo);
    let Ok(name) = name(clip);
    let Ok(heard) = state.at.clip(name);
    let Ok(whisper) = shown(&whisper);
    let Ok(model) = shown(&model);
    let Ok(heard) = shown(&heard);
    let mut arguments = vec![
        whisper,
        "--model".to_string(),
        model,
        "--file".to_string(),
        heard,
    ];

    arguments.push("--detect-language".to_string());

    Ok(arguments)
}

fn backend_probe(state: &Comparing) -> Result<Vec<String>, Never> {
    let Ok(whisper) = state.at.whisper();
    let Ok(turbo) = turbo();
    let Ok(model) = state.at.model(turbo);
    let Ok(heard) = state.at.clip("en-word");
    let Ok(whisper) = shown(&whisper);
    let Ok(model) = shown(&model);
    let Ok(heard) = shown(&heard);

    Ok(vec![
        whisper,
        "--model".to_string(),
        model,
        "--file".to_string(),
        heard,
        "--language".to_string(),
        "en".to_string(),
    ])
}

pub fn hearing(state: &Comparing, hear: Hear) -> Result<Option<Vec<String>>, Never> {
    let Hear { clip, model } = hear;
    let Ok(model) = index(model);

    let model = match MODELS.get(model) {
        Some(model) => model,
        None => return Ok(None),
    };

    let Ok(file) = state.at.model(model.file);
    let Ok(name) = name(clip);
    let Ok(heard) = state.at.clip(name);
    let Ok(language) = language(clip);
    let Ok(llama) = state.at.llama();
    let Ok(whisper) = state.at.whisper();
    let Ok(has_file) = is(state, &file);
    let Ok(has_llama) = is(state, &llama);
    let Ok(file) = shown(&file);
    let Ok(heard) = shown(&heard);

    Ok(match (has_file, model.engine, has_llama) {
        (Found::Absent, _, _) => None,
        (_, Engine::Llama, Found::Absent | Found::There) => None,
        (_, Engine::Whisper, _) => {
            let Ok(whisper) = shown(&whisper);

            Some(vec![
                whisper,
                "--model".to_string(),
                file,
                "--file".to_string(),
                heard,
                "--language".to_string(),
                language.to_string(),
                "--threads".to_string(),
                THREADS.to_string(),
                "--no-timestamps".to_string(),
                "--no-prints".to_string(),
            ])
        }
        (_, Engine::Llama, Found::Runnable) => {
            let Ok(llama) = shown(&llama);
            let Ok(mmproj) = state.at.model(QWEN_MMPROJ);
            let Ok(mmproj) = shown(&mmproj);

            Some(vec![
                llama,
                "-m".to_string(),
                file,
                "--mmproj".to_string(),
                mmproj,
                "--audio".to_string(),
                heard,
                "-p".to_string(),
                "Transcribe the audio.".to_string(),
                "-ngl".to_string(),
                "99".to_string(),
                "-t".to_string(),
                THREADS.to_string(),
                "--temp".to_string(),
                "0".to_string(),
            ])
        }
    })
}

fn getting(from: &str, into: &Path) -> Result<Command, Never> {
    let Ok(coming) = coming(into);

    Command::external(
        ExternalProgram::Curl,
        &["--location", "--fail", "--progress-bar", "--output", &coming, from],
    )
}

fn moved(left: &[(String, PathBuf)]) -> Result<Command, Never> {
    let into = match left.first() {
        Some((_named, into)) => into.clone(),
        None => PathBuf::new(),
    };
    let Ok(coming) = coming(&into);
    let Ok(shown) = shown(&into);

    Command::external(ExternalProgram::Mv, &[&coming, &shown])
}

fn making(at: &Where) -> Result<PathBuf, Never> {
    let Ok(made) = at.made();

    Ok(made.join("llama.cpp"))
}

fn made(at: &Where) -> Result<PathBuf, Never> {
    let Ok(making) = making(at);

    Ok(making.join("build/bin/llama-mtmd-cli"))
}

fn cloning(at: &Where) -> Result<Command, Never> {
    let Ok(making) = making(at);
    let Ok(shown) = shown(&making);

    Command::external(
        ExternalProgram::Git,
        &["clone", "--quiet", "--depth", "1", "--branch", LLAMA_AT, LLAMA_FROM, &shown],
    )
}

fn configuring(at: &Where) -> Result<Command, Never> {
    let Ok(making) = making(at);
    let Ok(source) = shown(&making);
    let Ok(build) = shown(&making.join("build"));

    Command::external(
        ExternalProgram::Cmake,
        &[
            "-S",
            &source,
            "-B",
            &build,
            "-DGGML_VULKAN=ON",
            "-DBUILD_SHARED_LIBS=OFF",
            "-DCMAKE_BUILD_TYPE=Release",
            "-DLLAMA_CURL=OFF",
            "-DLLAMA_BUILD_TESTS=OFF",
            "-DLLAMA_BUILD_EXAMPLES=OFF",
        ],
    )
}

fn compiling(at: &Where) -> Result<Command, Never> {
    let Ok(making) = making(at);
    let Ok(build) = shown(&making.join("build"));

    Command::external(
        ExternalProgram::Cmake,
        &["--build", &build, "--target", "llama-mtmd-cli", "--parallel"],
    )
}

fn name(clip: u32) -> Result<&'static str, Never> {
    let Ok(clip) = index(clip);

    Ok(match CLIPS.get(clip).map(|clip| clip.name) {
        Some(named) => named,
        None => NOTHING_NAMED,
    })
}

fn language(clip: u32) -> Result<&'static str, Never> {
    let Ok(clip) = index(clip);

    Ok(match CLIPS.get(clip).map(|clip| clip.language) {
        Some(language) => language,
        None => NOTHING_NAMED,
    })
}

fn turbo() -> Result<&'static str, Never> {
    Ok(match MODELS.first().map(|model| model.file) {
        Some(file) => file,
        None => NOTHING_NAMED,
    })
}

fn shown(at: &Path) -> Result<String, Never> {
    Ok(at.to_string_lossy().to_string())
}

fn coming(at: &Path) -> Result<String, Never> {
    let Ok(shown) = shown(at);

    Ok(format!("{shown}.coming"))
}

fn named(at: &Path) -> Result<String, Never> {
    Ok(match at.file_name() {
        Some(named) => named.to_string_lossy().to_string(),
        None => String::new(),
    })
}

fn stepped(state: &Comparing, step: Step) -> Result<Comparing, Never> {
    Ok(Comparing { step, ..state.clone() })
}

fn badly(state: &Comparing, why: &str) -> Result<Update<Comparing, CompareEffect>, Never> {
    Update::new(state.clone(), vec![Effect::Stop(Exit::Failure(why.to_string()))])
}

#[cfg(test)]
mod tests {
    use console_program_contract::run;

    use super::*;

    fn whisper(at: &Where) -> PathBuf {
        let Ok(whisper) = at.whisper();

        whisper
    }

    fn llama(at: &Where) -> PathBuf {
        let Ok(llama) = at.llama();

        llama
    }

    fn model(at: &Where, file: &str) -> PathBuf {
        let Ok(model) = at.model(file);

        model
    }

    fn clip(at: &Where, name: &str) -> PathBuf {
        let Ok(clip) = at.clip(name);

        clip
    }

    fn clips(at: &Where) -> PathBuf {
        let Ok(clips) = at.clips();

        clips
    }

    fn walking(state: &Comparing) -> Vec<Work> {
        let Ok(walking) = super::walking(state);

        walking
    }

    fn hearing(state: &Comparing, hear: Hear) -> Option<Vec<String>> {
        let Ok(hearing) = super::hearing(state, hear);

        hearing
    }

    fn detected(said: &str) -> Option<String> {
        let Ok(detected) = super::detected(said);

        detected
    }

    fn graphics(said: &str) -> Option<String> {
        let Ok(graphics) = super::graphics(said);

        graphics
    }

    fn fetches(at: &Where) -> Vec<(String, PathBuf)> {
        let Ok(fetches) = super::fetches(at);

        fetches
    }

    fn cloning(at: &Where) -> Command {
        let Ok(cloning) = super::cloning(at);

        cloning
    }

    fn configuring(at: &Where) -> Command {
        let Ok(configuring) = super::configuring(at);

        configuring
    }

    fn row(model: u32, took: &Timing) -> String {
        let Ok(row) = super::row(model, took);

        row
    }

    fn header(clip: u32, said: &str) -> String {
        let Ok(header) = super::header(clip, said);

        header
    }

    fn at() -> Where {
        Where {
            kept: PathBuf::from("/home/someone/.local/share/console/voice"),
            host: "handheld".to_string(),
            stamp: "2026-09-05-1730".to_string(),
        }
    }

    fn holding(there: Vec<Candidate>) -> Comparing {
        Comparing {
            job: Task::Compare,
            step: Step::Looking,
            at: at(),
            there,
            report: Vec::new(),
        }
    }

    fn all_of(every: &[PathBuf], is: Found) -> Vec<Candidate> {
        every.iter().map(|at| Candidate { at: at.clone(), is }).collect()
    }

    fn everything_there() -> Vec<Candidate> {
        let at = at();
        let mut there = all_of(&[whisper(&at), llama(&at)], Found::Runnable);

        there.extend(all_of(
            &MODELS.iter().map(|one| model(&at, one.file)).collect::<Vec<PathBuf>>(),
            Found::There,
        ));
        there.push(Candidate { at: model(&at, QWEN_MMPROJ), is: Found::There });
        there.extend(all_of(
            &CLIPS.iter().map(|one| clip(&at, one.name)).collect::<Vec<PathBuf>>(),
            Found::There,
        ));
        there
    }

    fn given(job: &str) -> Arguments {
        let at = at();
        let mut words = vec![
            at.kept.to_string_lossy().to_string(),
            at.host.clone(),
            at.stamp.clone(),
        ];

        match job.is_empty() {
            true => {},
            false => words.push(job.to_string()),
        }

        let Ok(arguments) = Arguments::of(&words.iter().map(String::as_str).collect::<Vec<&str>>());

        arguments
    }

    #[test]
    fn a_word_this_does_not_take_is_refused_rather_than_read_as_a_comparison() {
        let Ok(said) = run::<Compare>(&given("--everything"), &[Event::Opened]);
        let Ok(effects) = said.effects();

        assert!(matches!(effects.last(), Some(Effect::Stop(Exit::Failure(_)))));
    }

    #[test]
    fn every_clip_is_asked_for_by_name_and_by_what_it_is_for() {
        for clip in &CLIPS {
            assert!(!clip.says.is_empty(), "{} says nothing about what to say", clip.name);
            assert!(
                ["en", "nl", "th"].contains(&clip.language),
                "{} is in a language nothing here hears",
                clip.name
            );
        }
    }

    #[test]
    fn the_first_model_is_the_one_this_desktop_runs_today() {
        assert_eq!(MODELS.first().map(|model| model.file), Some(crate::MODEL));
    }

    #[test]
    fn a_machine_with_no_hearing_of_its_own_stops_rather_than_measuring_the_packaged_one() {
        let at = at();
        let Ok(said) = run::<Compare>(
            &given(""),
            &[
                Event::Opened,
                Event::Custom(CompareEvent::Looked(vec![Candidate { at: whisper(&at), is: Found::Absent }])),
            ],
        );

        let Ok(effects) = said.effects();

        assert!(
            matches!(effects.last(), Some(Effect::Stop(Exit::Failure(why)))
                if why.contains("console-dictate --fetch"))
        );
    }

    #[test]
    fn nothing_recorded_stops_before_a_single_model_is_loaded() {
        let at = at();
        let Ok(said) = run::<Compare>(
            &given(""),
            &[
                Event::Opened,
                Event::Custom(CompareEvent::Looked(vec![Candidate { at: whisper(&at), is: Found::Runnable }])),
            ],
        );

        let Ok(effects) = said.effects();

        assert!(
            effects.iter().all(|effect| !matches!(effect, Effect::Custom(CompareEffect::Timed(_)))),
            "it measured something with nothing recorded"
        );
        assert!(matches!(effects.last(), Some(Effect::Stop(Exit::Failure(_)))));
    }

    #[test]
    fn every_clip_is_read_by_every_model_and_the_language_is_guessed_once_per_clip() {
        let state = holding(everything_there());
        let left = walking(&state);
        assert_eq!(left.iter().filter(|work| matches!(work, Work::Detect(_))).count(), CLIPS.len());
        assert_eq!(left.iter().filter(|work| matches!(work, Work::Hear(_))).count(), CLIPS.len().saturating_mul(MODELS.len()));
    }

    #[test]
    fn a_clip_that_was_never_recorded_is_skipped_rather_than_measured_as_silence() {
        let at = at();
        let mut there = everything_there();

        there.retain(|seen| seen.at != clip(&at, "th-tones"));

        let left = walking(&holding(there));

        assert!(
            left.iter().all(|work| !matches!(work, Work::Detect(6))),
            "a clip no one recorded was read anyway"
        );
    }

    #[test]
    fn a_model_that_was_never_fetched_is_not_run() {
        let at = at();
        let mut there = everything_there();

        there.retain(|seen| seen.at != model(&at, "ggml-large-v3.bin"));

        let state = holding(there);

        assert!(
            hearing(&state, Hear { clip: 0, model: 2 }).is_none(),
            "a model that is not there was run"
        );
        assert!(hearing(&state, Hear { clip: 0, model: 0 }).is_some());
    }

    #[test]
    fn the_second_model_is_not_run_when_the_engine_for_it_was_never_built() {
        let at = at();
        let mut there = everything_there();

        there.retain(|seen| seen.at != llama(&at));

        assert!(hearing(&holding(there), Hear { clip: 0, model: 3 }).is_none());
    }

    #[test]
    fn whisper_is_given_the_language_the_clip_was_spoken_in_and_the_threads_dictate_gives_it() {
        let arguments =
            hearing(&holding(everything_there()), Hear { clip: 2, model: 0 }).unwrap_or_default();

        assert!(arguments.windows(2).any(|pair| pair == ["--language".to_string(), "th".to_string()]));
        assert!(arguments.windows(2).any(|pair| pair == ["--threads".to_string(), THREADS.to_string()]));
    }

    #[test]
    fn what_whisper_guessed_is_the_last_thing_it_said_about_a_language() {
        let said = "whisper_init: hello\nauto-detected language: nl (p = 0.9)\n";

        assert_eq!(detected(said), Some("nl".to_string()));
        assert_eq!(detected("nothing about it"), None);
    }

    #[test]
    fn a_run_with_no_graphics_line_is_said_to_have_had_none() {
        assert_eq!(graphics("ggml_vulkan: 0 = AMD Radeon"), Some("ggml_vulkan: 0 = AMD Radeon".to_string()));
        assert_eq!(graphics("ggml_vulkan: found no devices"), None);
        assert_eq!(graphics("whisper: nothing to say"), None);
    }

    #[test]
    fn every_model_it_would_fetch_has_somewhere_to_fetch_it_from() {
        let every = fetches(&at());

        for (from, into) in &every {
            assert!(from.starts_with("https://"), "{from} is not somewhere to fetch from");
            assert!(into.starts_with(&at().kept));
        }

        assert!(
            every.iter().any(|(from, _)| from.contains(QWEN_MMPROJ)),
            "the second engine's projector was never fetched"
        );
    }

    #[test]
    fn a_model_lands_beside_its_name_and_is_moved_onto_it_only_once_it_is_whole() {
        let every = fetches(&at());
        let Ok(said) = run::<Compare>(
            &given("--models"),
            &[
                Event::Opened,
                Event::Custom(CompareEvent::Looked(
                    every.iter().map(|(_, into)| Candidate { at: into.clone(), is: Found::Absent }).collect(),
                )),
            ],
        );
        let Ok(effects) = said.effects();
        let watched: Vec<Command> = effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::Stream(runs) | Effect::Run(runs) => Some(runs.clone()),
                _ => None,
            })
            .collect();

        assert!(
            watched.iter().any(|runs| runs.arguments.iter().any(|word| word.ends_with(".coming"))),
            "a model was written straight onto its own name"
        );
    }

    #[test]
    fn a_model_already_there_is_not_fetched_again() {
        let every = fetches(&at());
        let Ok(said) = run::<Compare>(
            &given("--models"),
            &[
                Event::Opened,
                Event::Custom(CompareEvent::Looked(
                    every.iter().map(|(_, into)| Candidate { at: into.clone(), is: Found::There }).collect(),
                )),
            ],
        );

        let Ok(effects) = said.effects();

        assert!(
            effects.iter().all(|effect| !matches!(effect, Effect::Stream(runs)
                if runs.program == console_program_contract::Executable::External(ExternalProgram::Curl))),
            "it fetched a model it already had"
        );
        assert!(matches!(effects.last(), Some(Effect::Stop(Exit::Success))));
    }

    #[test]
    fn an_engine_already_built_is_not_built_again() {
        let at = at();
        let Ok(said) = run::<Compare>(
            &given("--build"),
            &[
                Event::Opened,
                Event::Custom(CompareEvent::Looked(vec![Candidate { at: llama(&at), is: Found::Runnable }])),
            ],
        );

        let Ok(effects) = said.effects();

        assert!(
            effects.iter().all(|effect| !matches!(effect, Effect::Stream(_))),
            "it built an engine that was already there"
        );
    }

    #[test]
    fn the_second_engine_is_built_at_a_tag_and_never_at_a_branch() {
        let arguments = cloning(&at()).arguments;

        assert!(arguments.windows(2).any(|pair| pair == ["--branch".to_string(), LLAMA_AT.to_string()]));
        assert!(LLAMA_AT.starts_with('v'), "{LLAMA_AT} does not name a tag");
    }

    #[test]
    fn the_second_engine_is_built_against_this_machines_graphics_and_links_nothing_it_might_find() {
        let arguments = configuring(&at()).arguments;

        assert!(arguments.contains(&"-DGGML_VULKAN=ON".to_string()));
        assert!(arguments.contains(&"-DBUILD_SHARED_LIBS=OFF".to_string()));
    }

    #[test]
    fn recording_waits_for_a_press_before_it_listens_and_for_another_before_it_stops() {
        let Ok(said) = run::<Compare>(
            &given("--record"),
            &[Event::Opened, Event::Chosen(Choice::Yes), Event::Chosen(Choice::Yes)],
        );
        let Ok(effects) = said.effects();

        let before_the_end: Vec<&Effect<CompareEffect>> = effects
            .iter()
            .take_while(|effect| !matches!(effect, Effect::Custom(CompareEffect::Enough)))
            .collect();

        assert!(
            effects.iter().filter(|effect| matches!(effect, Effect::Prompt(_))).count() >= 3,
            "it did not wait to be told to start and to stop"
        );
        assert!(effects.iter().any(|effect| matches!(effect, Effect::Custom(CompareEffect::Enough))));
        assert!(before_the_end.iter().any(|effect| matches!(effect, Effect::Custom(CompareEffect::Record(_)))));
    }

    #[test]
    fn every_clip_is_recorded_into_its_own_name_under_the_clips_directory() {
        let at = at();

        for one in &CLIPS {
            assert_eq!(clip(&at, one.name), clips(&at).join(format!("{}.wav", one.name)));
        }
    }

    #[test]
    fn a_row_says_the_best_and_the_first_because_the_first_is_a_cost_paid_once_a_boot() {
        let line = row(0, &Timing { first: 3400, best: 900, said: "Settings".to_string() });

        assert!(line.contains("900 ms"));
        assert!(line.contains("first  3400 ms"));
        assert!(line.contains("Settings"));
    }

    #[test]
    fn a_language_guessed_wrong_is_marked_where_a_person_reading_will_see_it() {
        assert!(header(1, "auto-detected language: en").contains("<-- wrong"));
        assert!(!header(1, "auto-detected language: nl").contains("<-- wrong"));
    }
}
