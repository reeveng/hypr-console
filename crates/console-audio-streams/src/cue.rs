use std::fmt;
use std::fs::File;
use std::io::{self, Write};

use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};
use console_program_contract::Effect;
use console_program_lifetime::{Detached, let_go, threads};

use crate::Pcm;
use crate::pipewire;

pub struct VolumePreview<'a> {
  pub target: &'a str,
  pub lock: File,
}

#[derive(Debug)]
pub enum CueError {
  Start(io::Error),
  Write(io::Error),
  MissingInput,
  Incomplete,
}

impl fmt::Display for CueError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Start(fault) => write!(to, "could not start cue playback: {fault}"),
      Self::Write(fault) => write!(to, "could not write cue samples: {fault}"),
      Self::MissingInput => to.write_str("cue playback has no input"),
      Self::Incomplete => to.write_str("cue playback did not finish setup"),
    }
  }
}

pub fn play_cue(
  format: Pcm,
  samples: &[u8],
  preview: Option<VolumePreview<'_>>,
) -> Result<(), CueError> {
  let (preview_target, mut lock) = match preview {
    Some(reservation) => (Some(reservation.target), Some(reservation.lock)),
    None => (None, None),
  };
  let Ok(first) = Cue::initialize(
    &Cue {
      format,
      samples,
      preview_target,
    },
    None,
  );

  match Cue::run_effects(first, 3, &mut lock, |reservation, effect| match effect {
    Effect::Custom(operation) => {
      let Ok(reply) = perform(reservation, operation);

      reply
    }
    Effect::Run(_)
    | Effect::Stream(_)
    | Effect::Fork(_)
    | Effect::Interrupt(_)
    | Effect::Prompt(_)
    | Effect::Spawn(_)
    | Effect::Subscribe(_)
    | Effect::Unsubscribe(_)
    | Effect::Write(_)
    | Effect::Notify(_)
    | Effect::Print(_)
    | Effect::Stop(_) => Reply::InvalidEffect,
  }) {
    Ok(Phase::Done(result)) => result,
    Ok(Phase::Starting(_) | Phase::Writing | Phase::Reaping(_)) | Err(_) => {
      Err(CueError::Incomplete)
    }
  }
}

enum Reply {
  Started(io::Result<Detached>),
  Written {
    child: Detached,
    result: Result<(), CueError>,
  },
  Reaping,
  InvalidEffect,
}

enum Phase<'a> {
  Starting(&'a [u8]),
  Writing,
  Reaping(Result<(), CueError>),
  Done(Result<(), CueError>),
}

enum Operation<'a> {
  Start {
    format: Pcm,
    preview_target: Option<&'a str>,
  },
  Write {
    child: Detached,
    samples: &'a [u8],
  },
  Reap(Detached),
}

impl EffectDependencies for Operation<'_> {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(match self {
      Self::Start { .. } => &[Service::Audio, Service::Process],
      Self::Write { .. } => &[Service::Audio],
      Self::Reap(_) => &[Service::Process],
    })
  }
}

struct Cue<'a> {
  format: Pcm,
  samples: &'a [u8],
  preview_target: Option<&'a str>,
}

impl<'a> SingleEffectMachine for Cue<'a> {
  type Input = Self;
  type State = Phase<'a>;
  type Request = Reply;
  type Effect = Effect<Operation<'a>>;

  fn initialize(
    cue: &Self::Input,
    _: Option<Self::State>,
  ) -> Result<SingleTransition<Self::State, Self::Effect>, Never> {
    Ok(SingleTransition {
      state: Phase::Starting(cue.samples),
      effect: Some(Effect::Custom(Operation::Start {
        format: cue.format,
        preview_target: cue.preview_target,
      })),
    })
  }

  fn handle(
    state: Self::State,
    reply: Self::Request,
  ) -> Result<SingleTransition<Self::State, Self::Effect>, Never> {
    let (next, operation) = match (state, reply) {
      (Phase::Starting(samples), Reply::Started(Ok(child))) => {
        (Phase::Writing, Some(Operation::Write { child, samples }))
      }
      (Phase::Starting(_), Reply::Started(Err(fault))) => {
        (Phase::Done(Err(CueError::Start(fault))), None)
      }
      (Phase::Writing, Reply::Written { child, result }) => {
        (Phase::Reaping(result), Some(Operation::Reap(child)))
      }
      (Phase::Reaping(result), Reply::Reaping) => (Phase::Done(result), None),
      (
        phase,
        Reply::Started(_) | Reply::Written { .. } | Reply::Reaping | Reply::InvalidEffect,
      ) => (phase, None),
    };

    Ok(SingleTransition {
      state: next,
      effect: operation.map(Effect::Custom),
    })
  }
}

fn perform(reservation: &mut Option<File>, operation: Operation<'_>) -> Result<Reply, Never> {
  Ok(match operation {
    Operation::Start {
      format,
      preview_target,
    } => {
      let Ok(mut command) = pipewire::cue(format, preview_target, reservation.take());

      Reply::Started(let_go(&mut command))
    }
    Operation::Write { mut child, samples } => {
      let Ok(input) = child.take_stdin();
      let result = match input {
        Some(mut stdin) => stdin.write_all(samples).map_err(CueError::Write),
        None => Err(CueError::MissingInput),
      };

      Reply::Written { child, result }
    }
    Operation::Reap(mut child) => {
      let Ok(()) = threads::let_go(std::thread::spawn(move || match child.wait() {
        Ok(_) => {}
        Err(fault) => console_core_logging::warn!("could not reap cue playback: {fault}"),
      }));

      Reply::Reaping
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Sample;

  #[test]
  fn failed_start_finishes_without_writing_or_reaping() {
    let cue = Cue {
      format: Pcm {
        rate_hz: 48_000,
        channels: 1,
        sample: Sample::I16,
      },
      samples: &[0, 0],
      preview_target: Some("speakers"),
    };
    let Ok(first) = Cue::initialize(&cue, None);
    assert!(matches!(
      first.effect,
      Some(Effect::Custom(Operation::Start {
        preview_target: Some("speakers"),
        ..
      }))
    ));
    let fault = io::Error::from(io::ErrorKind::NotFound);
    let Ok(after) = Cue::handle(first.state, Reply::Started(Err(fault)));

    assert!(after.effect.is_none());
    assert!(matches!(after.state, Phase::Done(Err(CueError::Start(_)))));
  }
}
