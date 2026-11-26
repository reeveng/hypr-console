use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Instant;

use console_core_bytes::Bytes;
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use console_core_state_machine::{
  EffectDependencies, Service, SingleEffectMachine, SingleTransition,
};
use rustix::event::PollFlags;

use super::{MAX_PNG_REQUEST, MAX_REQUEST, PNG_TIME, Reply, Request, TIMEOUT, decode_request};

pub(crate) struct Peer {
  stream: UnixStream,
  state: State,
}

enum State {
  Reading {
    accepted: Instant,
    deadline: Instant,
    bytes: Vec<u8>,
  },
  Handling,
  Writing {
    deadline: Instant,
    bytes: Bytes,
    offset: u64,
  },
  Finished,
  Oversized,
}

enum Update<'a> {
  Read { part: &'a [u8], room: u64 },
  End,
  Prepared { bytes: Bytes, deadline: Instant },
  Written(u64),
}

enum Effect {
  Answer(Bytes),
}

impl EffectDependencies for Effect {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(&[Service::Clipboard])
  }
}

struct Transfer<'a>(std::marker::PhantomData<&'a [u8]>);

type Turn = SingleTransition<State, Effect>;

impl<'a> SingleEffectMachine for Transfer<'a> {
  type Input = (Instant, Instant);
  type State = State;
  type Request = Update<'a>;
  type Effect = Effect;

  fn initialize(
    &(accepted, deadline): &Self::Input,
    _previous: Option<State>,
  ) -> Result<Turn, Never> {
    Ok(Turn {
      state: State::Reading {
        accepted,
        deadline,
        bytes: Vec::new(),
      },
      effect: None,
    })
  }

  fn handle(state: State, update: Update<'a>) -> Result<Turn, Never> {
    let next = match (state, update) {
      (
        State::Reading {
          accepted,
          deadline,
          bytes,
        },
        Update::Read { part, room },
      ) => {
        let Ok(next) = received(accepted, deadline, bytes, part, room);

        next
      }
      (State::Reading { bytes, .. }, Update::End) => {
        return Ok(Turn {
          state: State::Handling,
          effect: Some(Effect::Answer(Bytes::from(bytes))),
        });
      }
      (State::Handling, Update::Prepared { bytes, deadline }) => State::Writing {
        deadline,
        bytes,
        offset: 0,
      },
      (
        State::Writing {
          deadline,
          bytes,
          offset,
        },
        Update::Written(written),
      ) => {
        let Ok(length) = fixed_index(bytes.len());
        let advanced = offset.saturating_add(written);

        match (advanced >= length).then_some(()) {
          Some(()) => State::Finished,
          None => State::Writing {
            deadline,
            bytes,
            offset: advanced,
          },
        }
      }
      (previous, _) => previous,
    };

    Ok(Turn {
      state: next,
      effect: None,
    })
  }
}

fn received(
  accepted: Instant,
  deadline: Instant,
  mut bytes: Vec<u8>,
  part: &[u8],
  room: u64,
) -> Result<State, Never> {
  let (maximum, expires) = match bytes.first().or_else(|| part.first()) {
    Some(0x89) => match accepted.checked_add(PNG_TIME) {
      Some(extended) => (MAX_PNG_REQUEST, extended),
      None => (MAX_PNG_REQUEST, deadline),
    },
    Some(_) | None => (MAX_REQUEST, deadline),
  };
  let Ok(used) = fixed_index(bytes.len());
  let Ok(incoming) = fixed_index(part.len());

  Ok(
    match (incoming <= room && used.saturating_add(incoming) <= u64::from(maximum)).then_some(())
    {
      Some(()) => {
        bytes.extend_from_slice(part);

        State::Reading {
          accepted,
          deadline: expires,
          bytes,
        }
      }
      None => State::Oversized,
    },
  )
}

impl Peer {
  pub(crate) fn accept(listener: &UnixListener, now: Instant) -> io::Result<Self> {
    let (stream, _peer) = listener.accept()?;
    stream.set_nonblocking(true)?;
    let deadline = now.checked_add(TIMEOUT).ok_or(io::ErrorKind::TimedOut)?;
    let Ok(turn) = Transfer::initialize(&(now, deadline), None);

    Ok(Self {
      stream,
      state: turn.state,
    })
  }

  pub(crate) fn expires(&self) -> Result<Option<Instant>, Never> {
    Ok(match self.state {
      State::Reading { deadline, .. } | State::Writing { deadline, .. } => Some(deadline),
      State::Handling | State::Finished | State::Oversized => None,
    })
  }

  pub(crate) fn socket(&self) -> Result<(&UnixStream, PollFlags), Never> {
    let interest = match self.state {
      State::Reading { .. } => PollFlags::IN,
      State::Writing { .. } => PollFlags::OUT,
      State::Handling | State::Finished | State::Oversized => PollFlags::empty(),
    };

    Ok((&self.stream, interest))
  }

  pub(crate) fn request_bytes(&self) -> Result<u64, Never> {
    Ok(match &self.state {
      State::Reading { bytes, .. } => {
        let Ok(used) = fixed_index(bytes.len());

        used
      }
      State::Handling | State::Writing { .. } | State::Finished | State::Oversized => 0,
    })
  }

  pub(crate) fn ready<Context>(
    mut self,
    room: u64,
    now: Instant,
    context: &mut Context,
    answer: impl FnOnce(&mut Context, Request) -> Result<Reply, Never>,
  ) -> io::Result<Option<Self>> {
    let mut chunk = [0_u8; 64 * 1024];
    let observed = match &self.state {
      State::Reading { .. } => read(&mut self.stream, &mut chunk, room),
      State::Writing { bytes, offset, .. } => write(&mut self.stream, bytes, *offset),
      State::Handling | State::Finished | State::Oversized => return Ok(None),
    };
    let update = match observed {
      Ok(Some(update)) => update,
      Ok(None) => return Ok(Some(self)),
      Err(fault) => return Err(fault),
    };
    let Ok(turn) = Transfer::handle(self.state, update);
    self.state = match turn.effect {
      Some(Effect::Answer(bytes)) => {
        let request = decode_request(bytes)?;
        let Ok(reply) = answer(context, request);
        let encoded = serde_json::to_vec(&reply)?;
        let deadline = now.checked_add(TIMEOUT).ok_or(io::ErrorKind::TimedOut)?;
        let response = Update::Prepared {
          bytes: Bytes::from(encoded),
          deadline,
        };
        let Ok(prepared) = Transfer::handle(turn.state, response);

        prepared.state
      }
      None => turn.state,
    };

    match self.state {
      State::Reading { .. } | State::Writing { .. } => Ok(Some(self)),
      State::Handling | State::Finished => Ok(None),
      State::Oversized => Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "clipboard request exceeds its byte limit",
      )),
    }
  }
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the poll owner reads one chunk from this nonblocking peer per turn"
  )
)]
fn read<'a>(
  stream: &mut UnixStream,
  chunk: &'a mut [u8],
  room: u64,
) -> io::Result<Option<Update<'a>>> {
  match stream.read(chunk) {
    Ok(0) => Ok(Some(Update::End)),
    Ok(amount) => {
      let part = chunk.get(..amount).ok_or(io::ErrorKind::InvalidData)?;

      Ok(Some(Update::Read { part, room }))
    }
    Err(fault) => pending(fault),
  }
}

fn write(
  stream: &mut UnixStream,
  bytes: &[u8],
  offset: u64,
) -> io::Result<Option<Update<'static>>> {
  let Ok(position) = index(offset);
  let remaining = bytes.get(position..).ok_or(io::ErrorKind::InvalidData)?;

  match stream.write(remaining) {
    Ok(0) => Err(io::Error::from(io::ErrorKind::WriteZero)),
    Ok(sent) => {
      let Ok(amount) = fixed_index(sent);

      Ok(Some(Update::Written(amount)))
    }
    Err(fault) => pending(fault),
  }
}

fn pending<'a>(fault: io::Error) -> io::Result<Option<Update<'a>>> {
  match (fault.kind() == io::ErrorKind::WouldBlock || fault.kind() == io::ErrorKind::Interrupted)
    .then_some(())
  {
    Some(()) => Ok(None),
    None => Err(fault),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_answer_effect_waits_for_eof_and_partial_writes_keep_the_reply() {
    let now = Instant::now();
    let Ok(opened) = Transfer::initialize(&(now, now), None);
    let Ok(fragment) = Transfer::handle(
      opened.state,
      Update::Read {
        part: b"a",
        room: 2,
      },
    );
    assert!(fragment.effect.is_none());
    let Ok(received) = Transfer::handle(
      fragment.state,
      Update::Read {
        part: b"b",
        room: 1,
      },
    );
    assert!(received.effect.is_none());

    let Ok(ended) = Transfer::handle(received.state, Update::End);
    assert!(matches!(ended.effect, Some(Effect::Answer(bytes)) if bytes.as_ref() == b"ab"));
    let Ok(prepared) = Transfer::handle(
      ended.state,
      Update::Prepared {
        bytes: Bytes::from(b"done".to_vec()),
        deadline: now,
      },
    );
    let Ok(partial) = Transfer::handle(prepared.state, Update::Written(1));
    assert!(matches!(partial.state, State::Writing { offset: 1, .. }));
    let Ok(complete) = Transfer::handle(partial.state, Update::Written(3));
    assert!(matches!(complete.state, State::Finished));
  }

  #[test]
  fn a_peer_cannot_append_past_the_shared_request_budget() {
    let now = Instant::now();
    let Ok(opened) = Transfer::initialize(&(now, now), None);
    let Ok(refused) = Transfer::handle(
      opened.state,
      Update::Read {
        part: b"abc",
        room: 2,
      },
    );

    assert!(matches!(refused.state, State::Oversized));
    assert!(refused.effect.is_none());
  }
}
