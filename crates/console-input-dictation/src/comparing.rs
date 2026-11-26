//! Which hearing this device should be using, measured on this device.
//!
//! The model was chosen because it worked, which is not the same as having
//! been compared against anything, and the `voice.md` note in Orbit names two
//! things it was never asked. Turbo is large-v3 with the decoder cut from
//! thirty-two layers to four, and OpenAI's own note on it is that it holds up
//! everywhere except a few languages, of which Thai is one -- and Thai is a
//! third of what is said to this device. Qwen3-ASR is the first local model
//! since whisper that hears all three of these languages, and llama.cpp can run
//! it on this machine's graphics. Both are answered by saying sixteen things
//! into it once.
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
//! The program contract sends UTF-8 command arguments, so a comparison checks
//! its directory once and preserves the spelling in each derived argument.

use std::path::{Path, PathBuf};

use console_core_external_programs::Program as ExternalProgram;
use console_core_bytes::Bytes;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};

const NOTHING_NAMED: &str = "";

const NOTHING_GUESSED: &str = "none";

use console_core_arguments::{
  CommandLine, Flag, NoSubcommand, Operands, Reason, Takes, ValidationError,
};
use console_core_state_machine::{EffectDependencies, Machine, Queue, Service, Transition};
use console_program_contract::{
  Choice, Command, Effect, Event, Exit, ExitStatus, FileWrite, Prompt,
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
  Clip {
    name: "en-word",
    language: "en",
    says: "ONE English word, as into a search box -- \"Settings\"",
  },
  Clip {
    name: "nl-word",
    language: "nl",
    says: "ONE Dutch word with an English word behind it -- \"zeven\". If \"seven\" comes back, detection went to English where you can watch it happen",
  },
  Clip {
    name: "th-word",
    language: "th",
    says: "ONE short Thai thing -- a greeting, a thank you",
  },
  Clip {
    name: "en-line",
    language: "en",
    says: "an English sentence, ten words or so, at ordinary speed",
  },
  Clip {
    name: "nl-line",
    language: "nl",
    says: "a Dutch sentence, ten words or so, at ordinary speed",
  },
  Clip {
    name: "th-line",
    language: "th",
    says: "a Thai sentence, ten words or so, at ordinary speed",
  },
  Clip {
    name: "th-tones",
    language: "th",
    says: "three words differing only in tone, with a gap between them -- dog, horse, come. The clip turbo is expected to lose on",
  },
  Clip {
    name: "th-marks",
    language: "th",
    says: "two or three words carrying the marks that are not letters: a silent-letter word, a sara am word",
  },
  Clip {
    name: "nl-hard",
    language: "nl",
    says: "a Dutch sentence heavy in ui, eu and oo -- \"Het huis heeft een nieuwe voordeur\"",
  },
  Clip {
    name: "nl-mixed",
    language: "nl",
    says: "a Dutch sentence with the English words left in, the way it is really spoken -- \"Ik heb de update gedownload naar mijn desktop\". This is what pinning the language costs",
  },
  Clip {
    name: "th-mixed",
    language: "th",
    says: "the same the other way: a Thai sentence with an English word or two in it",
  },
  Clip {
    name: "en-name",
    language: "en",
    says: "a name or a filename rather than a sentence -- what this paddle is mostly pressed for",
  },
  Clip {
    name: "nl-number",
    language: "nl",
    says: "a number and a date said aloud in Dutch. Models disagree wildly about digits",
  },
  Clip {
    name: "quiet",
    language: "en",
    says: "anything, said quietly, at arm length. This is the guard that decides whether a recording is speech at all",
  },
  Clip {
    name: "pause",
    language: "en",
    says: "a sentence with a two-second silence through the middle of it",
  },
  Clip {
    name: "nothing",
    language: "en",
    says: "say NOTHING. Start, wait three seconds, stop. Whisper answers an empty room with \"Thank you.\" and this is the guard that stops it",
  },
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
  Model {
    name: "turbo-q5",
    engine: Engine::Whisper,
    file: "ggml-large-v3-turbo-q5_0.bin",
  },
  Model {
    name: "turbo",
    engine: Engine::Whisper,
    file: "ggml-large-v3-turbo.bin",
  },
  Model {
    name: "large-v3",
    engine: Engine::Whisper,
    file: "ggml-large-v3.bin",
  },
  Model {
    name: "qwen3-asr",
    engine: Engine::Llama,
    file: "Qwen3-ASR-1.7B-Q8_0.gguf",
  },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Where {
  pub kept: CommandPath,
  pub host: String,
  pub stamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPath(String);

#[derive(Debug)]
pub struct NonTextPath(PathBuf);

impl std::fmt::Display for NonTextPath {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      to,
      "the comparison directory needs a UTF-8 name: {:?}",
      self.0
    )
  }
}

impl std::error::Error for NonTextPath {}

impl CommandPath {
  pub fn new(at: PathBuf) -> Result<Self, NonTextPath> {
    let text = at
      .into_os_string()
      .into_string()
      .map_err(|native| NonTextPath(PathBuf::from(native)))?;

    Ok(Self(text))
  }

  fn below(&self, relative: &str) -> Result<Self, Never> {
    let separator = match self.0.as_bytes().last() {
      Some(b'/') | None => "",
      Some(_) => "/",
    };

    Ok(Self(format!("{}{separator}{relative}", self.0)))
  }
}

impl AsRef<Path> for CommandPath {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

impl From<CommandPath> for PathBuf {
  fn from(path: CommandPath) -> Self {
    PathBuf::from(path.0)
  }
}

impl Where {
  fn compare_dir(&self) -> Result<CommandPath, Never> {
    self.kept.below("compare")
  }

  fn clips(&self) -> Result<CommandPath, Never> {
    let Ok(made) = self.compare_dir();

    made.below("clips")
  }

  fn clip(&self, name: &str) -> Result<CommandPath, Never> {
    let Ok(clips) = self.clips();
    let Ok(file) = leaf(&format!("{name}.wav"));

    clips.below(&file)
  }

  fn whisper(&self) -> Result<CommandPath, Never> {
    self.kept.below("whisper-cli")
  }

  fn llama(&self) -> Result<CommandPath, Never> {
    self.kept.below("llama-mtmd-cli")
  }

  fn model(&self, file: &str) -> Result<CommandPath, Never> {
    let Ok(leaf) = leaf(file);

    self.kept.below(&leaf)
  }

  fn report(&self) -> Result<CommandPath, Never> {
    let Ok(made) = self.compare_dir();
    let Ok(file) = leaf(&format!("said-{}.txt", self.stamp));

    made.below(&file)
  }
}

fn leaf(name: &str) -> Result<String, Never> {
  Ok(match name {
    "" => "%".to_string(),
    "." => "%2E".to_string(),
    ".." => "%2E%2E".to_string(),
    _ => name
      .replace('%', "%25")
      .replace('/', "%2F")
      .replace('\0', "%00"),
  })
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
  Look(Vec<CommandPath>),
  Ran(Vec<String>),
  Timed(Vec<String>),
  StartRecording(PathBuf),
  StopRecording,
}

impl EffectDependencies for CompareEffect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Look(_) | Self::Ran(_) => &[Service::Process],
      Self::Timed(_) => &[Service::Process, Service::Clock],
      Self::StartRecording(_) => &[Service::Audio, Service::Process, Service::FileSystem],
      Self::StopRecording => &[Service::Audio, Service::Process],
    })
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareEvent {
  Looked(Vec<Candidate>),
  Output(String),
  Timed(Timing),
  RecordingStarted,
  RecordingStopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
  pub at: CommandPath,
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
  Starting,
  Recording,
  Stopping,
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
  Initial,
  Sizing,
  Fetching(Vec<(String, CommandPath)>),
  Getting(Vec<(String, CommandPath)>),
  Moving(Vec<(String, CommandPath)>),
  Locating,
  Clearing,
  Cloning,
  Setting,
  Compiling,
  Copying,
  Naming,
  Sweeping,
  PreparingRecording,
  Recording(Vec<u32>, RecordingPhase),
  Looking,
  DetectingBackend,
  Working(Vec<Work>),
  WritingReport,
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

pub const RECORD: Flag = Flag {
  spelling: "--record",
  takes: Takes::None,
  about: "say the clips, once",
};

pub const FETCH_MODELS: Flag = Flag {
  spelling: "--models",
  takes: Takes::None,
  about: "fetch the models",
};

pub const BUILD: Flag = Flag {
  spelling: "--build",
  takes: Takes::None,
  about: "build the second engine, which is slow",
};

pub const FETCH: Flag = Flag {
  spelling: "--fetch",
  takes: Takes::None,
  about: "fetch the models and build the second engine",
};

pub const COMMAND: console_core_arguments::Command = console_core_arguments::Command {
  name: "voice-compare",
  about: "which hearing this device should use, measured on this device: every clip read with every model, unless one of these says otherwise",
  flags: &[RECORD, FETCH_MODELS, BUILD, FETCH],
  operands: Operands::None,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
  pub at: Where,
  pub job: Task,
}

impl Task {
  pub fn of(line: &CommandLine<NoSubcommand>) -> Result<Task, ValidationError> {
    let chosen = line.one_of(&[RECORD, FETCH_MODELS, BUILD, FETCH]);

    match chosen {
      Ok(flag) => {
        let Ok(task) = Task::chosen_by(flag);

        Ok(task)
      }
      Err(ValidationError {
        reason: Reason::MissingFlag(_),
        ..
      }) => Ok(Task::Compare),
      Err(refusal) => Err(refusal),
    }
  }

  fn chosen_by(flag: Flag) -> Result<Task, Never> {
    Ok(match flag {
      RECORD => Task::Record,
      FETCH_MODELS => Task::Models,
      BUILD => Task::Build,
      _fetch => Task::Fetch,
    })
  }
}

pub struct Compare;

impl Machine for Compare {
  type Input = Comparison;
  type State = Comparing;
  type Request = Event<CompareEvent>;
  type Effect = Effect<CompareEffect>;

  fn initialize(
    comparison: &Comparison,
    _previous: Option<Comparing>,
    _effects: &mut Effects,
  ) -> Result<Comparing, Never> {
    Ok(Comparing {
      job: comparison.job,
      step: Step::Initial,
      at: comparison.at.clone(),
      there: Vec::new(),
      report: Vec::new(),
    })
  }

  fn handle(
    state: Comparing,
    event: Event<CompareEvent>,
    effects: &mut Effects,
  ) -> Result<Comparing, Never> {
    let Ok(decided) = decide(&state, &event);

    decided.offered(effects)
  }
}

type Effects = Queue<Effect<CompareEffect>>;

fn decide(
  state: &Comparing,
  event: &Event<CompareEvent>,
) -> Result<Transition<Comparing, Effect<CompareEffect>>, Never> {
  match (&state.step, event) {
    (Step::Initial, Event::Opened) => begun(state),

    (Step::Sizing, Event::Custom(CompareEvent::Looked(seen))) => {
      let state_2 = Comparing {
        there: seen.to_vec(),
        ..state.clone()
      };
      let Ok(left) = missing_downloads(&state_2);

      fetching(&state_2, &left)
    }

    (Step::Fetching(left), _) => fetching(state, left),

    (Step::Getting(left), Event::Replied(answer)) => match answer.status {
      ExitStatus::Success => {
        let Ok(stepped) = with_step(state, Step::Moving(left.clone()));
        let Ok(moved) = moved(left);

        Transition::new(stepped, vec![Effect::Run(moved)])
      }
      ExitStatus::Failure(_) | ExitStatus::Interrupt => {
        badly(state, "a model would not come down")
      }
    },

    (Step::Moving(left), Event::Replied(_)) => {
      fetching(state, &left.iter().skip(1).cloned().collect::<Vec<_>>())
    }

    (Step::Locating, Event::Custom(CompareEvent::Looked(seen))) => built(state, seen),

    (Step::Clearing, Event::Replied(_)) => {
      let Ok(stepped) = with_step(state, Step::Cloning);
      let Ok(making) = making(&state.at);
      let Ok(shown) = shown(&making);
      let Ok(making_2) = Command::external(ExternalProgram::Mkdir, &["-p", &shown]);

      Transition::new(stepped, vec![Effect::Stream(making_2)])
    }

    (Step::Cloning, Event::Replied(_)) => {
      let Ok(stepped) = with_step(state, Step::Setting);
      let Ok(cloning) = cloning(&state.at);

      Transition::new(stepped, vec![Effect::Stream(cloning)])
    }

    (Step::Setting, Event::Replied(answer)) => match answer.status {
      ExitStatus::Failure(_) | ExitStatus::Interrupt => {
        badly(state, "llama.cpp would not come down")
      }
      ExitStatus::Success => {
        let Ok(stepped) = with_step(state, Step::Compiling);
        let Ok(configuring) = configuring(&state.at);

        Transition::new(stepped, vec![Effect::Stream(configuring)])
      }
    },

    (Step::Compiling, Event::Replied(answer)) => match answer.status {
      ExitStatus::Failure(_) | ExitStatus::Interrupt => {
        badly(state, "the second engine would not configure")
      }
      ExitStatus::Success => {
        let Ok(stepped) = with_step(state, Step::Copying);
        let Ok(compiling) = compiling(&state.at);

        Transition::new(stepped, vec![Effect::Stream(compiling)])
      }
    },

    (Step::Copying, Event::Replied(answer)) => match answer.status {
      ExitStatus::Failure(_) | ExitStatus::Interrupt => {
        badly(state, "the second engine would not build")
      }
      ExitStatus::Success => {
        let Ok(stepped) = with_step(state, Step::Naming);
        let Ok(made) = llama_binary(&state.at);
        let Ok(from) = shown(&made);
        let Ok(llama) = state.at.llama();
        let Ok(into) = coming(&llama);
        let Ok(copying) = Command::external(ExternalProgram::Cp, &[&from, &into]);

        Transition::new(stepped, vec![Effect::Run(copying)])
      }
    },

    (Step::Naming, Event::Replied(_)) => {
      let Ok(stepped) = with_step(state, Step::Sweeping);
      let Ok(llama) = state.at.llama();
      let Ok(from) = coming(&llama);
      let Ok(into) = shown(&llama);
      let Ok(naming) = Command::external(ExternalProgram::Mv, &[&from, &into]);

      Transition::new(stepped, vec![Effect::Run(naming)])
    }

    (Step::Sweeping, Event::Replied(_)) => {
      let Ok(stepped) = with_step(state, Step::Finished);
      let Ok(making) = making(&state.at);
      let Ok(shown) = shown(&making);
      let Ok(sweeping) = Command::external(ExternalProgram::Rm, &["-rf", &shown]);

      Transition::new(
        stepped,
        vec![Effect::Run(sweeping), Effect::Print("   built".to_string())],
      )
    }

    (Step::Finished, Event::Replied(_)) => after(state),

    (Step::PreparingRecording, Event::Replied(answer)) => match answer.status {
      ExitStatus::Success => ready_to_record(state),
      ExitStatus::Failure(_) | ExitStatus::Interrupt => {
        let Ok(clips) = state.at.clips();
        let Ok(path) = shown(&clips);

        badly(
          state,
          &format!("could not create the clips directory {path}"),
        )
      }
    },

    (Step::Recording(left, ear), word) => recording(state, left, ear, word),

    (Step::Looking, Event::Custom(CompareEvent::Looked(seen))) => looked(state, seen),

    (Step::DetectingBackend, Event::Custom(CompareEvent::Output(said))) => {
      backend_detected(state, said)
    }

    (Step::Working(left), word) => working(state, left, word),

    (Step::WritingReport, Event::Written(answer)) => {
      let Ok(at) = state.at.report();

      match (answer.path == at.as_ref(), &answer.result) {
        (false, _) => Transition::without_effects(state.clone()),
        (true, Err(fault)) => badly(
          state,
          &format!("could not write the report: {}", fault.detail),
        ),
        (true, Ok(())) => {
          let Ok(shown) = shown(&at);
          let Ok(stepped) = with_step(state, Step::Finished);

          Transition::new(
            stepped,
            vec![
              Effect::Print(format!("\nWritten to {shown}")),
              Effect::Print(CLOSING.to_string()),
              Effect::Stop(Exit::Success),
            ],
          )
        }
      }
    }

    (_, _) => Transition::without_effects(state.clone()),
  }
}

fn begun(state: &Comparing) -> Result<Transition<Comparing, Effect<CompareEffect>>, Never> {
  match state.job {
    Task::Record => {
      let Ok(stepped) = with_step(state, Step::PreparingRecording);
      let Ok(clips) = state.at.clips();
      let Ok(shown) = shown(&clips);
      let Ok(making) = Command::external(ExternalProgram::Mkdir, &["-p", &shown]);

      Transition::new(stepped, vec![Effect::Run(making)])
    }

    Task::Models | Task::Fetch => {
      let Ok(stepped) = with_step(state, Step::Sizing);
      let Ok(shown) = shown(&state.at.kept);
      let Ok(fetches) = fetches(&state.at);
      let Ok(making) = Command::external(ExternalProgram::Mkdir, &["-p", &shown]);

      Transition::new(
        stepped,
        vec![
          Effect::Run(making),
          Effect::Print("== the models".to_string()),
          Effect::Custom(CompareEffect::Look(
            fetches.iter().map(|(_, into)| into.clone()).collect(),
          )),
        ],
      )
    }

    Task::Build => locate_llama(state),

    Task::Compare => {
      let Ok(stepped) = with_step(state, Step::Looking);
      let Ok(everything) = everything(&state.at);

      Transition::new(
        stepped,
        vec![Effect::Custom(CompareEffect::Look(everything))],
      )
    }
  }
}

fn after(state: &Comparing) -> Result<Transition<Comparing, Effect<CompareEffect>>, Never> {
  Transition::new(state.clone(), vec![Effect::Stop(Exit::Success)])
}

fn fetching(
  state: &Comparing,
  left: &[(String, CommandPath)],
) -> Result<Transition<Comparing, Effect<CompareEffect>>, Never> {
  match left.first() {
    None => match state.job {
      Task::Fetch => {
        let Ok(stepped) = with_step(state, Step::Locating);
        let Ok(llama) = state.at.llama();

        Transition::new(
          stepped,
          vec![
            Effect::Print("\n== the second engine".to_string()),
            Effect::Custom(CompareEffect::Look(vec![llama])),
          ],
        )
      }
      Task::Record | Task::Models | Task::Build | Task::Compare => after(state),
    },
    Some((from, into)) => {
      let Ok(stepped) = with_step(state, Step::Getting(left.to_vec()));
      let Ok(named) = file_name(into);
      let Ok(getting) = getting(from, into);

      Transition::new(
        stepped,
        vec![
          Effect::Print(format!("   fetching {named}")),
          Effect::Stream(getting),
        ],
      )
    }
  }
}

fn locate_llama(
  state: &Comparing,
) -> Result<Transition<Comparing, Effect<CompareEffect>>, Never> {
  let Ok(stepped) = with_step(state, Step::Locating);
  let Ok(llama) = state.at.llama();

  Transition::new(
    stepped,
    vec![
      Effect::Print("== the second engine".to_string()),
      Effect::Custom(CompareEffect::Look(vec![llama])),
    ],
  )
}

fn built(
  state: &Comparing,
  seen: &[Candidate],
) -> Result<Transition<Comparing, Effect<CompareEffect>>, Never> {
  let is = seen.first().map(|seen_2| seen_2.is);

  match is {
    Some(Found::Runnable) => {
      let Ok(stepped) = with_step(state, Step::Finished);

      Transition::new(
        stepped,
        vec![
          Effect::Print("   already built".to_string()),
          Effect::Stop(Exit::Success),
        ],
      )
    }
    Some(Found::Absent | Found::There) | None => {
      let Ok(stepped) = with_step(state, Step::Clearing);
      let Ok(making) = making(&state.at);
      let Ok(shown) = shown(&making);
      let Ok(clearing) = Command::external(ExternalProgram::Rm, &["-rf", &shown]);

      Transition::new(
        stepped,
        vec![
          Effect::Print(format!(
            "   llama.cpp {LLAMA_AT}, pointed at this machine's graphics.\n   \
                     This is a C++ project and a handheld. It takes a while."
          )),
          Effect::Run(clearing),
        ],
      )
    }
  }
}

fn ready_to_record(
  state: &Comparing,
) -> Result<Transition<Comparing, Effect<CompareEffect>>, Never> {
  let Ok(clips) = fitted::<_, u32>(CLIPS.len());
  let Ok(stepped) = with_step(
    state,
    Step::Recording((0..clips).collect(), RecordingPhase::Waiting),
  );
  let Ok(asking) = prompt_for_clip(0);

  Transition::new(
    stepped,
    PREAMBLE
      .iter()
      .map(|line| Effect::Print((*line).to_string()))
      .chain(asking)
      .collect(),
  )
}

fn recording(
  state: &Comparing,
  left: &[u32],
  ear: &RecordingPhase,
  event: &Event<CompareEvent>,
) -> Result<Transition<Comparing, Effect<CompareEffect>>, Never> {
  let at = left.first().copied();

  match (at, ear, event) {
    (Some(at_2), RecordingPhase::Waiting, Event::Chosen(_)) => {
      let Ok(stepped) = with_step(
        state,
        Step::Recording(left.to_vec(), RecordingPhase::Starting),
      );
      let Ok(name) = name(at_2);
      let Ok(clip) = state.at.clip(name);
      Transition::new(
        stepped,
        vec![Effect::Custom(CompareEffect::StartRecording(
          PathBuf::from(clip),
        ))],
      )
    }

    (Some(_), RecordingPhase::Starting, Event::Custom(CompareEvent::RecordingStarted)) => {
      let Ok(stepped) = with_step(
        state,
        Step::Recording(left.to_vec(), RecordingPhase::Recording),
      );
      let Ok(asking) = Prompt::unless("  listening, ENTER to stop", Choice::Yes);

      Transition::new(stepped, vec![Effect::Prompt(asking)])
    }

    (Some(_), RecordingPhase::Recording, Event::Chosen(_)) => {
      let Ok(stepped) = with_step(
        state,
        Step::Recording(left.to_vec(), RecordingPhase::Stopping),
      );
      Transition::new(stepped, vec![Effect::Custom(CompareEffect::StopRecording)])
    }

    (Some(_), RecordingPhase::Stopping, Event::Custom(CompareEvent::RecordingStopped)) => {
      let rest: Vec<u32> = left.iter().skip(1).copied().collect();
      let Ok(stepped) = with_step(
        state,
        Step::Recording(rest.clone(), RecordingPhase::Waiting),
      );
      let next = match rest.first() {
        Some(next) => {
          let Ok(asking) = prompt_for_clip(*next);

          asking
        }
        None => vec![
          Effect::Print("\nRecorded. Now: cargo run --bin voice-compare".to_string()),
          Effect::Stop(Exit::Success),
        ],
      };

      Transition::new(
        stepped,
        std::iter::once(Effect::Print("  kept".to_string()))
          .chain(next)
          .collect(),
      )
    }

    (_, _, _) => Transition::without_effects(state.clone()),
  }
}

fn prompt_for_clip(at: u32) -> Result<Vec<Effect<CompareEffect>>, Never> {
  let Ok(at_2) = index(at);

  Ok(match CLIPS.get(at_2) {
    Some(clip) => {
      let Ok(starting) = Prompt::unless("  ENTER to start", Choice::Yes);

      vec![
        Effect::Print(format!(
          "\n{} ({})\n  {}",
          clip.name, clip.language, clip.says
        )),
        Effect::Prompt(starting),
      ]
    }
    None => Vec::new(),
  })
}

fn looked(
  state: &Comparing,
  seen: &[Candidate],
) -> Result<Transition<Comparing, Effect<CompareEffect>>, Never> {
  let state_2 = Comparing {
    there: seen.to_vec(),
    ..state.clone()
  };
  let Ok(at) = state_2.at.whisper();
  let Ok(whisper) = is(&state_2, &at);

  match whisper {
    Found::Absent | Found::There => {
      let Ok(shown) = shown(&at);

      badly(
        &state_2,
        &format!(
          "The hearing this machine built for itself is not there:\n  {shown}\n\
                     ButtonPress the paddle once, or run: console-dictate --fetch\n\
                     The packaged whisper-cli speaks nothing but the processor, so measuring \
                     against it would be measuring the wrong thing."
        ),
      )
    }
    Found::Runnable => {
      let Ok(recorded) = recorded(&state_2);

      match ::core::primitive::bool::then_some(recorded.is_empty(), ()) {
        ::core::option::Option::Some(()) => badly(
          &state_2,
          "Nothing has been recorded yet: cargo run --bin voice-compare -- --record",
        ),
        ::core::option::Option::None => {
          let Ok(stepped) = with_step(&state_2, Step::DetectingBackend);
          let Ok(probe) = backend_probe(&state_2);

          Transition::new(stepped, vec![Effect::Custom(CompareEffect::Ran(probe))])
        }
      }
    }
  }
}

fn backend_detected(
  state: &Comparing,
  said: &str,
) -> Result<Transition<Comparing, Effect<CompareEffect>>, Never> {
  let Ok(card) = graphics(said);
  let opening = vec![
    format!("voice-compare, {}, on {}", state.at.stamp, state.at.host),
    match card {
      Some(graphics_line) => format!("graphics: {graphics_line}"),
      None => "graphics: NONE FOUND -- this ran on the processor, so the numbers below \
                     are about the wrong program"
        .to_string(),
    },
  ];
  let Ok(left) = walking(state);
  let state_2 = Comparing {
    report: opening.clone(),
    ..state.clone()
  };
  let Ok(stepped) = with_step(&state_2, Step::Working(left.clone()));
  let Ok(going) = next_effects(&state_2, &left);

  Transition::new(
    stepped,
    opening
      .iter()
      .map(|line| Effect::Print(line.clone()))
      .chain(going)
      .collect(),
  )
}

fn working(
  state: &Comparing,
  left: &[Work],
  event: &Event<CompareEvent>,
) -> Result<Transition<Comparing, Effect<CompareEffect>>, Never> {
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
    None => Transition::without_effects(state.clone()),
    Some(line_2) => {
      let mut report = state.report.clone();
      report.push(line_2.clone());

      let state_2 = Comparing {
        report,
        ..state.clone()
      };
      let (step, next) = match ::core::primitive::bool::then_some(rest.is_empty(), ()) {
        ::core::option::Option::None => {
          let Ok(going) = next_effects(&state_2, &rest);

          (Step::Working(rest), going)
        }
        ::core::option::Option::Some(()) => {
          let Ok(written) = write_report(&state_2);

          (Step::WritingReport, vec![written])
        }
      };

      let Ok(stepped) = with_step(&state_2, step);

      Transition::new(
        stepped,
        std::iter::once(Effect::Print(line_2)).chain(next).collect(),
      )
    }
  }
}

fn next_effects(state: &Comparing, left: &[Work]) -> Result<Vec<Effect<CompareEffect>>, Never> {
  Ok(match left.first() {
    Some(Work::Detect(clip)) => {
      let Ok(detecting) = detecting(state, *clip);

      vec![Effect::Custom(CompareEffect::Ran(detecting))]
    }
    Some(Work::Hear(hear)) => {
      let Ok(hearing) = whisper_arguments(state, *hear);

      match hearing {
        Some(arguments) => vec![Effect::Custom(CompareEffect::Timed(arguments))],
        None => Vec::new(),
      }
    }
    None => Vec::new(),
  })
}

fn write_report(state: &Comparing) -> Result<Effect<CompareEffect>, Never> {
  let Ok(at) = state.at.report();

  Ok(Effect::Write(FileWrite {
    path: PathBuf::from(at),
    contents: Bytes::from(format!("{}\n", state.report.join("\n"))),
  }))
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

pub fn fetches(at: &Where) -> Result<Vec<(String, CommandPath)>, Never> {
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

pub fn missing_downloads(state: &Comparing) -> Result<Vec<(String, CommandPath)>, Never> {
  let Ok(fetches) = fetches(&state.at);

  Ok(
    fetches
      .into_iter()
      .filter(|(_, into)| {
        let Ok(is) = is(state, into);

        match is {
          Found::Absent => true,
          Found::There | Found::Runnable => false,
        }
      })
      .collect(),
  )
}

pub fn everything(at: &Where) -> Result<Vec<CommandPath>, Never> {
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
    let Ok(clip_2) = at.clip(clip.name);

    every.push(clip_2);
  }

  Ok(every)
}

pub fn walking(state: &Comparing) -> Result<Vec<Work>, Never> {
  let mut every = Vec::new();
  let Ok(recorded) = recorded(state);

  for clip in recorded {
    every.push(Work::Detect(clip));

    for (model, _) in MODELS.iter().enumerate() {
      let Ok(model_2) = fitted(model);

      every.push(Work::Hear(Hear {
        clip,
        model: model_2,
      }));
    }
  }

  Ok(every)
}

fn recorded(state: &Comparing) -> Result<Vec<u32>, Never> {
  let mut every = Vec::new();

  for (at, clip) in CLIPS.iter().enumerate() {
    let Ok(at_2) = fitted(at);
    let Ok(clip_2) = state.at.clip(clip.name);
    let Ok(is) = is(state, &clip_2);

    match is {
      Found::Absent => {}
      Found::There | Found::Runnable => every.push(at_2),
    }
  }

  Ok(every)
}

fn is(state: &Comparing, at: &CommandPath) -> Result<Found, Never> {
  let seen = state
    .there
    .iter()
    .find(|seen| &seen.at == at)
    .map(|seen| seen.is);

  Ok(match seen {
    Some(is) => is,
    None => Found::Absent,
  })
}

pub fn graphics(said: &str) -> Result<Option<String>, Never> {
  Ok(
    said
      .lines()
      .find(|line| {
        line.starts_with(CARD)
          && line.get(CARD.len()..).is_some_and(|rest| {
            rest
              .chars()
              .next()
              .is_some_and(|first| first.is_ascii_digit())
          })
      })
      .map(str::to_string),
  )
}

pub fn detected(said: &str) -> Result<Option<String>, Never> {
  Ok(
    said
      .lines()
      .filter_map(|line| line.split_once(DETECTING))
      .map(|(_, rest)| {
        rest
          .chars()
          .take_while(char::is_ascii_lowercase)
          .collect::<String>()
      })
      .rfind(|word| !word.is_empty()),
  )
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
  let wrong = match ::core::primitive::bool::then_some(guessed == spoken, ()) {
    ::core::option::Option::Some(()) => "",
    ::core::option::Option::None => "   <-- wrong",
  };

  let Ok(named) = name(clip);

  Ok(format!(
    "\n== {named}   spoken: {spoken}   auto heard: {guessed}{wrong}"
  ))
}

fn row(model: u32, took: &Timing) -> Result<String, Never> {
  let Ok(plainly) = plainly(&took.said);
  let Ok(model_2) = index(model);

  let named = match MODELS.get(model_2).map(|model_3| model_3.name) {
    Some(named) => named,
    None => NOTHING_NAMED,
  };

  Ok(format!(
    "   {named:<11} {:>5} ms  (first {:>5} ms)  {plainly}",
    took.best, took.first
  ))
}

fn detecting(state: &Comparing, clip: u32) -> Result<Vec<String>, Never> {
  let Ok(whisper) = state.at.whisper();
  let Ok(turbo) = turbo();
  let Ok(model) = state.at.model(turbo);
  let Ok(name) = name(clip);
  let Ok(heard) = state.at.clip(name);
  let Ok(whisper_2) = shown(&whisper);
  let Ok(model_file) = shown(&model);
  let Ok(heard_2) = shown(&heard);
  let mut arguments = vec![
    whisper_2,
    "--model".to_string(),
    model_file,
    "--file".to_string(),
    heard_2,
  ];

  arguments.push("--detect-language".to_string());

  Ok(arguments)
}

fn backend_probe(state: &Comparing) -> Result<Vec<String>, Never> {
  let Ok(whisper) = state.at.whisper();
  let Ok(turbo) = turbo();
  let Ok(model) = state.at.model(turbo);
  let Ok(heard) = state.at.clip("en-word");
  let Ok(whisper_2) = shown(&whisper);
  let Ok(model_file) = shown(&model);
  let Ok(heard_2) = shown(&heard);

  Ok(vec![
    whisper_2,
    "--model".to_string(),
    model_file,
    "--file".to_string(),
    heard_2,
    "--language".to_string(),
    "en".to_string(),
  ])
}

pub fn whisper_arguments(state: &Comparing, hear: Hear) -> Result<Option<Vec<String>>, Never> {
  let Hear {
    clip,
    model: model_number,
  } = hear;
  let Ok(model_index) = index(model_number);

  let model = match MODELS.get(model_index) {
    Some(model_2) => model_2,
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
  let Ok(model_file) = shown(&file);
  let Ok(heard_2) = shown(&heard);

  Ok(match (has_file, model.engine, has_llama) {
    (Found::Absent, _, _) => None,
    (_, Engine::Llama, Found::Absent | Found::There) => None,
    (_, Engine::Whisper, _) => {
      let Ok(whisper_2) = shown(&whisper);

      Some(vec![
        whisper_2,
        "--model".to_string(),
        model_file,
        "--file".to_string(),
        heard_2,
        "--language".to_string(),
        language.to_string(),
        "--threads".to_string(),
        THREADS.to_string(),
        "--no-timestamps".to_string(),
        "--no-prints".to_string(),
      ])
    }
    (_, Engine::Llama, Found::Runnable) => {
      let Ok(llama_2) = shown(&llama);
      let Ok(mmproj) = state.at.model(QWEN_MMPROJ);
      let Ok(projector_file) = shown(&mmproj);

      Some(vec![
        llama_2,
        "-m".to_string(),
        model_file,
        "--mmproj".to_string(),
        projector_file,
        "--audio".to_string(),
        heard_2,
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

fn getting(from: &str, into: &CommandPath) -> Result<Command, Never> {
  let Ok(coming) = coming(into);

  Command::external(
    ExternalProgram::Curl,
    &[
      "--location",
      "--fail",
      "--progress-bar",
      "--output",
      &coming,
      from,
    ],
  )
}

fn moved(left: &[(String, CommandPath)]) -> Result<Command, Never> {
  let into = match left.first() {
    Some((_named, into)) => into.clone(),
    None => CommandPath(String::new()),
  };
  let Ok(coming) = coming(&into);
  let Ok(shown) = shown(&into);

  Command::external(ExternalProgram::Mv, &[&coming, &shown])
}

fn making(at: &Where) -> Result<CommandPath, Never> {
  let Ok(made) = at.compare_dir();

  made.below("llama.cpp")
}

fn llama_binary(at: &Where) -> Result<CommandPath, Never> {
  let Ok(making) = making(at);

  making.below("build/bin/llama-mtmd-cli")
}

fn cloning(at: &Where) -> Result<Command, Never> {
  let Ok(making) = making(at);
  let Ok(shown) = shown(&making);

  Command::external(
    ExternalProgram::Git,
    &[
      "clone", "--quiet", "--depth", "1", "--branch", LLAMA_AT, LLAMA_FROM, &shown,
    ],
  )
}

fn configuring(at: &Where) -> Result<Command, Never> {
  let Ok(making) = making(at);
  let Ok(source) = shown(&making);
  let Ok(build_path) = making.below("build");
  let Ok(build) = shown(&build_path);

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
  let Ok(build_path) = making.below("build");
  let Ok(build) = shown(&build_path);

  Command::external(
    ExternalProgram::Cmake,
    &[
      "--build",
      &build,
      "--target",
      "llama-mtmd-cli",
      "--parallel",
    ],
  )
}

fn name(clip: u32) -> Result<&'static str, Never> {
  let Ok(clip_2) = index(clip);

  Ok(match CLIPS.get(clip_2).map(|clip_3| clip_3.name) {
    Some(named) => named,
    None => NOTHING_NAMED,
  })
}

fn language(clip: u32) -> Result<&'static str, Never> {
  let Ok(clip_2) = index(clip);

  Ok(match CLIPS.get(clip_2).map(|clip_3| clip_3.language) {
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

fn shown(at: &CommandPath) -> Result<String, Never> {
  Ok(at.0.clone())
}

fn coming(at: &CommandPath) -> Result<String, Never> {
  let Ok(shown) = shown(at);

  Ok(format!("{shown}.coming"))
}

fn file_name(at: &CommandPath) -> Result<String, Never> {
  Ok(
    at.as_ref()
      .file_name()
      .and_then(std::ffi::OsStr::to_str)
      .map_or_else(String::new, str::to_string),
  )
}

fn with_step(state: &Comparing, step: Step) -> Result<Comparing, Never> {
  Ok(Comparing {
    step,
    ..state.clone()
  })
}

fn badly(
  state: &Comparing,
  why: &str,
) -> Result<Transition<Comparing, Effect<CompareEffect>>, Never> {
  Transition::new(
    state.clone(),
    vec![Effect::Stop(Exit::Failure(why.to_string()))],
  )
}

#[cfg(test)]
mod tests {
  use console_core_state_machine::run;
  use console_program_contract::Answer;

  use super::*;

  #[test]
  fn a_report_is_announced_only_after_the_write_succeeds() -> Result<(), Never> {
    use console_program_contract::{WriteAnswer, WriteFailure, WriteStage};

    let comparing = holding(Vec::new())?;
    let at = comparing.at.report()?;
    let writing = with_step(&comparing, Step::WritingReport)?;
    let failed = decide(
      &writing,
      &Event::Written(WriteAnswer {
        path: PathBuf::from(at.clone()),
        result: Err(WriteFailure {
          stage: WriteStage::Fill,
          detail: "disk full".to_string(),
        }),
      }),
    )?;
    assert_eq!(
      failed.effects,
      [Effect::Stop(Exit::Failure(
        "could not write the report: disk full".to_string()
      ))]
    );

    let made = decide(
      &writing,
      &Event::Written(WriteAnswer {
        path: PathBuf::from(at),
        result: Ok(()),
      }),
    )?;
    assert!(
      matches!(made.effects.first(), Some(Effect::Print(message)) if message.contains("Written to"))
    );
    assert_eq!(made.effects.last(), Some(&Effect::Stop(Exit::Success)));

    Ok(())
  }

  fn place() -> Result<Where, Never> {
    Ok(Where {
      kept: CommandPath("/home/someone/.local/share/console/voice".to_string()),
      host: "handheld".to_string(),
      stamp: "2026-09-05-1730".to_string(),
    })
  }

  fn holding(there: Vec<Candidate>) -> Result<Comparing, Never> {
    let Ok(at) = place();

    Ok(Comparing {
      job: Task::Compare,
      step: Step::Looking,
      at,
      there,
      report: Vec::new(),
    })
  }

  fn all_of(every: &[CommandPath], is: Found) -> Result<Vec<Candidate>, Never> {
    Ok(
      every
        .iter()
        .map(|at| Candidate { at: at.clone(), is })
        .collect(),
    )
  }

  fn everything_there() -> Result<Vec<Candidate>, Never> {
    let Ok(at) = place();
    let Ok(whisper) = at.whisper();
    let Ok(llama) = at.llama();
    let Ok(mut there) = all_of(&[whisper, llama], Found::Runnable);
    let Ok(projector) = at.model(QWEN_MMPROJ);

    for one in &MODELS {
      let Ok(model) = at.model(one.file);

      there.push(Candidate {
        at: model,
        is: Found::There,
      });
    }

    there.push(Candidate {
      at: projector,
      is: Found::There,
    });

    for one in &CLIPS {
      let Ok(clip) = at.clip(one.name);

      there.push(Candidate {
        at: clip,
        is: Found::There,
      });
    }

    Ok(there)
  }

  fn given(job: Task) -> Result<Comparison, Never> {
    let Ok(at) = place();

    Ok(Comparison { at, job })
  }

  #[test]
  fn a_word_this_does_not_take_is_refused_rather_than_read_as_a_comparison() {
    let everything = console_core_arguments::read(&COMMAND, &["--everything"]);

    assert_eq!(
      everything.map(drop).map_err(|refusal| refusal.reason),
      Err(Reason::NoSuchFlag("--everything".to_string()))
    );
  }

  #[test]
  fn one_flag_chooses_the_job_none_compares_and_two_are_refused() -> Result<(), ValidationError> {
    let nothing: [&str; 0] = [];
    let models = console_core_arguments::read(&COMMAND, &["--models"])?;
    let plain = console_core_arguments::read(&COMMAND, &nothing)?;
    let both = console_core_arguments::read(&COMMAND, &["--record", "--build"])?;

    assert_eq!(Task::of(&models), Ok(Task::Models));
    assert_eq!(Task::of(&plain), Ok(Task::Compare));
    assert_eq!(
      Task::of(&both).map_err(|refusal| refusal.reason),
      Err(Reason::ExtraArgument("--build".to_string()))
    );

    Ok(())
  }

  #[test]
  fn every_clip_is_asked_for_by_name_and_by_what_it_is_for() {
    for clip in &CLIPS {
      assert!(
        !clip.says.is_empty(),
        "{} says nothing about what to say",
        clip.name
      );
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
    let Ok(at) = place();
    let Ok(given) = given(Task::Compare);
    let Ok(whisper) = at.whisper();

    let Ok(said) = run::<Compare>(
      &given,
      &[
        Event::Opened,
        Event::Custom(CompareEvent::Looked(vec![Candidate {
          at: whisper,
          is: Found::Absent,
        }])),
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
    let Ok(at) = place();
    let Ok(given) = given(Task::Compare);
    let Ok(whisper) = at.whisper();

    let Ok(said) = run::<Compare>(
      &given,
      &[
        Event::Opened,
        Event::Custom(CompareEvent::Looked(vec![Candidate {
          at: whisper,
          is: Found::Runnable,
        }])),
      ],
    );

    let Ok(effects) = said.effects();

    assert!(
      effects
        .iter()
        .all(|effect| !matches!(effect, Effect::Custom(CompareEffect::Timed(_)))),
      "it measured something with nothing recorded"
    );
    assert!(matches!(
      effects.last(),
      Some(Effect::Stop(Exit::Failure(_)))
    ));
  }

  #[test]
  fn every_clip_is_read_by_every_model_and_the_language_is_guessed_once_per_clip() {
    let Ok(everything_there) = everything_there();
    let Ok(state) = holding(everything_there);
    let Ok(left) = walking(&state);
    assert_eq!(
      left
        .iter()
        .filter(|work| matches!(work, Work::Detect(_)))
        .count(),
      CLIPS.len()
    );
    assert_eq!(
      left
        .iter()
        .filter(|work| matches!(work, Work::Hear(_)))
        .count(),
      CLIPS.len().saturating_mul(MODELS.len())
    );
  }

  #[test]
  fn a_clip_that_was_never_recorded_is_skipped_rather_than_measured_as_silence() {
    let Ok(at) = place();
    let Ok(mut there) = everything_there();

    let Ok(tones) = at.clip("th-tones");

    there.retain(|seen| seen.at != tones);

    let Ok(holding) = holding(there);
    let Ok(left) = walking(&holding);

    assert!(
      left.iter().all(|work| !matches!(work, Work::Detect(6))),
      "a clip no one recorded was read anyway"
    );
  }

  #[test]
  fn a_model_that_was_never_fetched_is_not_run() {
    let Ok(at) = place();
    let Ok(mut there) = everything_there();

    let Ok(large) = at.model("ggml-large-v3.bin");

    there.retain(|seen| seen.at != large);

    let Ok(state) = holding(there);
    let Ok(unheard) = whisper_arguments(&state, Hear { clip: 0, model: 2 });

    assert!(unheard.is_none(), "a model that is not there was run");

    let Ok(hearing) = whisper_arguments(&state, Hear { clip: 0, model: 0 });

    assert!(hearing.is_some());
  }

  #[test]
  fn the_second_model_is_not_run_when_the_engine_for_it_was_never_built() {
    let Ok(at) = place();
    let Ok(mut there) = everything_there();

    let Ok(llama) = at.llama();

    there.retain(|seen| seen.at != llama);

    let Ok(holding) = holding(there);
    let Ok(hearing) = whisper_arguments(&holding, Hear { clip: 0, model: 3 });

    assert!(hearing.is_none());
  }

  #[test]
  fn whisper_is_given_the_language_the_clip_was_spoken_in_and_the_threads_dictate_gives_it()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(everything_there) = everything_there();
    let Ok(holding) = holding(everything_there);
    let Ok(hearing) = whisper_arguments(&holding, Hear { clip: 2, model: 0 });
    let arguments = hearing.ok_or("whisper is not asked to hear the clip")?;

    assert!(
      arguments
        .windows(2)
        .any(|pair| pair == ["--language".to_string(), "th".to_string()])
    );
    assert!(
      arguments
        .windows(2)
        .any(|pair| pair == ["--threads".to_string(), THREADS.to_string()])
    );

    Ok(())
  }

  #[test]
  fn what_whisper_guessed_is_the_last_thing_it_said_about_a_language() {
    let said = "whisper_init: hello\nauto-detected language: nl (p = 0.9)\n";

    assert_eq!(detected(said), Ok(Some("nl".to_string())));
    assert_eq!(detected("nothing about it"), Ok(None));
  }

  #[test]
  fn a_run_with_no_graphics_line_is_said_to_have_had_none() {
    assert_eq!(
      graphics("ggml_vulkan: 0 = AMD Radeon"),
      Ok(Some("ggml_vulkan: 0 = AMD Radeon".to_string()))
    );
    assert_eq!(graphics("ggml_vulkan: found no devices"), Ok(None));
    assert_eq!(graphics("whisper: nothing to say"), Ok(None));
  }

  #[test]
  fn every_model_it_would_fetch_has_somewhere_to_fetch_it_from() {
    let Ok(at) = place();
    let Ok(every) = fetches(&at);

    for (from, into) in &every {
      assert!(
        from.starts_with("https://"),
        "{from} is not somewhere to fetch from"
      );
      assert!(into.as_ref().starts_with(&at.kept));
    }

    assert!(
      every.iter().any(|(from, _)| from.contains(QWEN_MMPROJ)),
      "the second engine's projector was never fetched"
    );
  }

  #[test]
  fn a_model_lands_beside_its_name_and_is_moved_onto_it_only_once_it_is_whole() {
    let Ok(at) = place();
    let Ok(every) = fetches(&at);
    let Ok(models) = given(Task::Models);

    let Ok(said) = run::<Compare>(
      &models,
      &[
        Event::Opened,
        Event::Custom(CompareEvent::Looked(
          every
            .iter()
            .map(|(_, into)| Candidate {
              at: into.clone(),
              is: Found::Absent,
            })
            .collect(),
        )),
      ],
    );

    let Ok(effects) = said.effects();
    let set_aside = effects
      .iter()
      .flat_map(|effect| match effect {
        Effect::Stream(runs) | Effect::Run(runs) => runs.arguments.as_slice(),
        Effect::Prompt(_)
        | Effect::Fork(_)
        | Effect::Interrupt(_)
        | Effect::Spawn(_)
        | Effect::Subscribe(_)
        | Effect::Unsubscribe(_)
        | Effect::Write(_)
        | Effect::Notify(_)
        | Effect::Print(_)
        | Effect::Stop(_)
        | Effect::Custom(_) => &[],
      })
      .any(|word| word.ends_with(".coming"));

    assert!(set_aside, "a model was written straight onto its own name");
  }

  #[test]
  fn a_model_already_there_is_not_fetched_again() {
    let Ok(at) = place();
    let Ok(every) = fetches(&at);
    let Ok(models) = given(Task::Models);

    let Ok(said) = run::<Compare>(
      &models,
      &[
        Event::Opened,
        Event::Custom(CompareEvent::Looked(
          every
            .iter()
            .map(|(_, into)| Candidate {
              at: into.clone(),
              is: Found::There,
            })
            .collect(),
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
    let Ok(at) = place();
    let Ok(build) = given(Task::Build);

    let Ok(llama) = at.llama();

    let Ok(said) = run::<Compare>(
      &build,
      &[
        Event::Opened,
        Event::Custom(CompareEvent::Looked(vec![Candidate {
          at: llama,
          is: Found::Runnable,
        }])),
      ],
    );

    let Ok(effects) = said.effects();

    assert!(
      effects
        .iter()
        .all(|effect| !matches!(effect, Effect::Stream(_))),
      "it built an engine that was already there"
    );
  }

  #[test]
  fn the_second_engine_is_built_at_a_tag_and_never_at_a_branch() {
    let Ok(at) = place();
    let Ok(cloning) = cloning(&at);

    let arguments = cloning.arguments;

    assert!(
      arguments
        .windows(2)
        .any(|pair| pair == ["--branch".to_string(), LLAMA_AT.to_string()])
    );
    assert!(LLAMA_AT.starts_with('v'), "{LLAMA_AT} does not name a tag");
  }

  #[test]
  fn the_second_engine_is_built_against_this_machines_graphics_and_links_nothing_it_might_find() {
    let Ok(at) = place();
    let Ok(configuring) = configuring(&at);

    let arguments = configuring.arguments;

    assert!(arguments.contains(&"-DGGML_VULKAN=ON".to_string()));
    assert!(arguments.contains(&"-DBUILD_SHARED_LIBS=OFF".to_string()));
  }

  #[test]
  fn recording_waits_for_each_press_and_for_the_recorder_to_reply()
  -> Result<(), Box<dyn std::error::Error>> {
    let Ok(record) = given(Task::Record);
    let Ok(initial) = Compare::initial_transition(&record, None);
    let Ok(opened) = Compare::transition(initial.state, Event::Opened);
    let making = match opened.effects.as_slice() {
      [Effect::Run(command)] => command.clone(),
      _other => {
        return Err(Box::<dyn std::error::Error>::from(
          "expected only the directory creation effect",
        ));
      }
    };
    let reply = Answer {
      job: None,
      command: making,
      output: String::new(),
      error_output: String::new(),
      status: ExitStatus::Success,
    };
    let Ok(refused) = Compare::transition(
      opened.state.clone(),
      Event::Replied(Answer {
        status: ExitStatus::Failure(Some(1)),
        ..reply.clone()
      }),
    );
    assert!(matches!(
      refused.effects.as_slice(),
      [Effect::Stop(Exit::Failure(_))]
    ));
    let Ok(ready) = Compare::transition(opened.state, Event::Replied(reply));
    assert!(matches!(ready.effects.last(), Some(Effect::Prompt(_))));

    let Ok(started) = Compare::transition(ready.state, Event::Chosen(Choice::Yes));
    assert!(matches!(
      started.effects.as_slice(),
      [Effect::Custom(CompareEffect::StartRecording(_))]
    ));

    let Ok(listening) =
      Compare::transition(started.state, Event::Custom(CompareEvent::RecordingStarted));
    assert!(
      matches!(listening.effects.as_slice(), [Effect::Prompt(prompt)] if prompt.message.contains("listening"))
    );

    let Ok(stopping) = Compare::transition(listening.state, Event::Chosen(Choice::Yes));
    assert_eq!(
      stopping.effects,
      [Effect::Custom(CompareEffect::StopRecording)]
    );

    let Ok(stopped) = Compare::transition(
      stopping.state,
      Event::Custom(CompareEvent::RecordingStopped),
    );
    assert_eq!(
      stopped.effects.first(),
      Some(&Effect::Print("  kept".to_string()))
    );
    assert!(matches!(stopped.effects.last(), Some(Effect::Prompt(_))));

    Ok(())
  }

  #[test]
  fn every_clip_is_recorded_into_its_own_name_under_the_clips_directory() {
    let Ok(at) = place();

    for one in &CLIPS {
      let Ok(clips) = at.clips();
      let Ok(path) = at.clip(one.name);
      let file = format!("{}.wav", one.name);

      assert_eq!(path.as_ref().parent(), Some(clips.as_ref()));
      assert_eq!(path.as_ref().file_name(), Some(std::ffi::OsStr::new(&file)));
    }
  }

  #[test]
  fn a_comparison_refuses_a_directory_that_the_command_contract_cannot_spell() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let directory = PathBuf::from(OsStr::from_bytes(b"/kept/voice-\xff"));

    let requested = directory.clone();

    assert!(matches!(
      CommandPath::new(directory),
      Err(NonTextPath(rejected)) if rejected == requested
    ));
  }

  #[test]
  fn a_row_says_the_best_and_the_first_because_the_first_is_a_cost_paid_once_a_boot() {
    let Ok(line) = row(
      0,
      &Timing {
        first: 3400,
        best: 900,
        said: "Settings".to_string(),
      },
    );

    assert!(line.contains("900 ms"));
    assert!(line.contains("first  3400 ms"));
    assert!(line.contains("Settings"));
  }

  #[test]
  fn a_language_guessed_wrong_is_marked_where_a_person_reading_will_see_it() {
    let Ok(english) = header(1, "auto-detected language: en");

    assert!(english.contains("<-- wrong"));

    let Ok(dutch) = header(1, "auto-detected language: nl");

    assert!(!dutch.contains("<-- wrong"));
  }
}
