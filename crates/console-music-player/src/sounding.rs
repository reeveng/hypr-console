//! Sound, as a pipe with this program at the head of it.
//!
//! [`decoding`] reads one song out as plain samples and pw-cat plays what it
//! is handed. Between them is a loop in a thread of its own, and that loop is
//! every control this player has: pausing ends the pipe, position is how much
//! has been written, and stopping is a pipe that closes. None of the three is a
//! question asked of someone else's program, which is what made every one of
//! kew's answers a thing to be waited for rather than read.
//!
//! **A pause that keeps the pipe keeps the machine awake.** Not writing was
//! the cheaper pause and it left pw-cat standing with the stream open, which
//! is a sink PipeWire will not suspend: the graph goes on running, the codec
//! stays powered, and a handheld with the music paused draws what a handheld
//! playing music draws. So a pause ends the sink, lets the song go, and keeps
//! the one thing that cannot be worked out again, which is where in the song
//! it got to. Starting again is a seek, which this file has to do anyway.
//!
//! **A song also keeps the machine from sleeping**, which is the other sense of
//! the word and a different mechanism entirely. The desktop suspends itself ten
//! minutes after the screen goes dark, and somebody who put the device down
//! with a track running asked for the dark and not for the silence. So playing
//! takes a logind `sleep` lock and pausing or stopping lets it go, held from
//! here because this is the one place that knows which of the three the player
//! is in. It is deliberately not an `idle` lock: hypridle reads those once for
//! the whole of its config, so an idle lock would hold the panel lit as well,
//! and the panel is what a handheld actually spends its battery on.
//! `console-awake` is the rest of that argument.
//!
//! What is kept is what was heard rather than what was written. The samples in
//! the pipe and in pw-cat's own buffer go with pw-cat, so folding the raw
//! count into the offset would step over [`AHEAD`] of music every time someone
//! pressed pause. Taking it off costs at worst a third of a second heard twice,
//! which is the direction to be wrong in.
//!
//! A seek is a run begun again: the song opened and its container asked for
//! the second, and a sink started for it. Opening a file costs little now that
//! no program is started to read it, so a run is one shape however it began
//! rather than a running decoder told to go somewhere else.
//!
//! What position is, exactly. Frames are counted as they are written, a frame
//! being one sample for each channel, at the rate the song was made at. What
//! has been written is not what has been heard: the pipe to pw-cat holds what a
//! pipe holds and pw-cat holds its own latency on top, so the count runs ahead
//! of the speaker by an amount that is bounded rather than known. [`AHEAD`] is
//! that amount, named once and taken off, and the answer is held at the offset
//! the run began from rather than allowed to walk back behind it in the first
//! moments, when the pipe is not yet full and the count is honest. A player
//! that reported the count raw would draw a scrub bar a third of a second ahead
//! of the music for the whole song, which is visible on a bar the width of a
//! handheld; one that took it off with nothing yet in the pipe would step back
//! a third of a second at every pause, every seek and every song. Paused there
//! is no pipe to be ahead of at all, so what is reported is the offset itself.
//!
//! **A tempo is the file's time and not the speaker's.** A film at twice its
//! speed is [`stretching`] between the decoder and the pipe, so what is
//! written is ordinary samples at an ordinary rate and a second of them is two
//! seconds of the film. The position multiplies what was heard by the tempo
//! and nothing else changes, which is why the music player, which never asks
//! for one, reads the same numbers it always read.
//!
//! **A song can be loaded without being played.** [`Sounding::ready`] is
//! [`Sounding::play`] with the wanting the other way round and no one woken:
//! the thread is left waiting, the offset is held, and the next press is a
//! resume rather than a start. That is what the player comes up holding when
//! the last song it played is remembered, because a device switched on in
//! someone's bag must not begin making a noise on its own.
//!
//! **What a press costs is written down here rather than worked out later.** The
//! stretch someone feels is between asking for a song and hearing one, and it
//! is spent in three places this thread can see: opening the song, starting
//! the sink, and the first samples decoded out of one and taken by the other.
//! So the stopwatch is [`one`]'s own and it ends at the first write, not at the
//! end of the song, and a run that never reaches a first write -- a song that
//! would not open, a press overtaken by the next one --
//! writes nothing, because no one waited for a sound that never came.
//!
//! The thread decides nothing about music. It is told a song and an offset, and
//! it carries samples until someone says otherwise or the song ends. When a
//! song ends it says so and stops there, and each whole second it carries it
//! says that too, because the clock under a song is the one thing nobody else
//! can see moving: what plays next is the playlist's business, the playlist belongs to the main thread, and a thread that reached
//! across to it would be the shape of fault kew had -- one list walked by
//! whoever got there first.

use crate::Seconds;
use crate::decoding::{self, Block, Decoding, Format};
use crate::stretching::Stretching;
use console_awake::{InhibitReason, InhibitResult, Staying};
use console_core_external_programs::Program;
use console_core_iteration::{Endless, Step};
use console_core_never::Never;
use console_core_number_conversion::{Float, index};
use console_operating_system_linux::program_lifetime::{BoundToParent, TimedPipe, alongside};
use console_response_times::{Wait, Waiting};
use console_sound_effects::balance::Balance;
use std::io::ErrorKind;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Weak;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

pub const SAMPLE: &str = "f32";

pub const LATENCY: &str = "50ms";

const AHEAD: Seconds = Seconds(0.383);

const PIPE_WAIT: Duration = Duration::from_millis(100);

const SHUTDOWN_WAIT: Duration = Duration::from_secs(2);

const UNTOLD: u32 = 48_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Wanted {
  Playing,
  Paused,
  #[default]
  Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ready {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Payload {
  On,
  Hold,
  Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
  Ended,
  ASecondPlayed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Crossed {
  ASecond,
  None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reached {
  End,
  Requested,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "tempo is a dimensionless playback rate ratio"
  )
)]
pub struct Tempo(pub f64);

impl Default for Tempo {
  fn default() -> Tempo {
    Tempo(1.0)
  }
}

#[derive(Debug, Clone, Default)]
struct Task {
  song: Option<PathBuf>,
  from: Seconds,
  tempo: Tempo,
  turn: u64,
}

#[derive(Debug)]
struct State {
  job: Task,
  wanted: Wanted,
  rate: u32,
  written: u64,
  ended: Option<u64>,
  lifetime: Lifetime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifetime {
  Running,
  Closing,
  Closed,
}

impl Default for State {
  fn default() -> State {
    State {
      job: Task::default(),
      wanted: Wanted::default(),
      rate: UNTOLD,
      written: 0,
      ended: None,
      lifetime: Lifetime::Running,
    }
  }
}

struct Playback {
  state: Mutex<State>,
  woken: Condvar,
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Sounding owns the worker and closes its rendezvous before joining; both endpoints need to observe playback position and requests until shutdown"
  )
)]
pub struct Sounding {
  telling: Arc<Playback>,
  awake: Mutex<Option<Staying>>,
  worker: Option<thread::JoinHandle<()>>,
}

#[derive(Clone)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "the video clock reads a weak view of playback and cannot keep its Sounding owner or worker alive"
  )
)]
pub struct PositionReader {
  playback: Weak<Playback>,
}

impl PositionReader {
  pub fn position(&self) -> Result<Option<f64>, Never> {
    let playback = match self.playback.upgrade() {
      Some(playback) => playback,
      None => return Ok(None),
    };
    let Ok(state) = lock(&playback.state);

    match state.lifetime {
      Lifetime::Running => {
        let Ok(Seconds(position)) = position_of(&state);

        Ok(Some(position))
      }
      Lifetime::Closing | Lifetime::Closed => Ok(None),
    }
  }
}

impl Sounding {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "only the Sounding owner and its joined worker hold the two endpoints of this private rendezvous"
    )
  )]
  pub fn new<E>(telling_progress: E) -> Result<Sounding, Never>
  where
    E: Fn(Progress) -> Result<(), Never> + Send + 'static,
  {
    let telling = Arc::new(Playback {
      state: Mutex::new(State::default()),
      woken: Condvar::new(),
    });
    let carrying = Arc::clone(&telling);

    let worker = thread::Builder::new()
      .name("music samples".to_string())
      .spawn(move || {
        let Ok(()) = pumping(&carrying, &telling_progress);

        drop(telling_progress);

        let Ok(mut state) = lock(&carrying.state);

        state.lifetime = Lifetime::Closed;
        carrying.woken.notify_all();
      });
    let worker_2 = match worker {
      Ok(worker_3) => Some(worker_3),
      Err(fault) => {
        console_core_logging::warn!("could not start the audio worker: {fault}");

        let Ok(mut state) = lock(&telling.state);

        state.lifetime = Lifetime::Closed;

        None
      }
    };

    Ok(Sounding {
      telling,
      awake: Mutex::new(None),
      worker: worker_2,
    })
  }

  pub fn play<T>(&self, song: &Path, from: T) -> Result<(), Never>
  where
    Seconds: From<T>,
  {
    self.play_at(song, from, Tempo::default())
  }

  pub fn play_at<T>(&self, song: &Path, from: T, tempo: Tempo) -> Result<(), Never>
  where
    Seconds: From<T>,
  {
    let Ok(()) = self.taking(song, Seconds::from(from), tempo, Wanted::Playing);

    self.telling.woken.notify_all();

    Ok(())
  }

  pub fn ready<T>(&self, song: &Path, from: T) -> Result<(), Never>
  where
    Seconds: From<T>,
  {
    self.taking(song, Seconds::from(from), Tempo::default(), Wanted::Paused)
  }

  fn taking(
    &self,
    song: &Path,
    from: Seconds,
    tempo: Tempo,
    wanted: Wanted,
  ) -> Result<(), Never> {
    let Ok(mut state) = lock(&self.telling.state);

    state.job = Task {
      song: Some(song.to_path_buf()),
      from,
      tempo,
      turn: state.job.turn.saturating_add(1),
    };
    state.wanted = wanted;
    state.written = 0;
    state.ended = None;

    drop(state);

    self.minding()
  }

  pub fn seek<T>(&self, to: T) -> Result<(), Never>
  where
    Seconds: From<T>,
  {
    let Seconds(destination) = Seconds::from(to);
    let Ok(state) = lock(&self.telling.state);
    let song = state.job.song.clone();
    let tempo = state.job.tempo;

    drop(state);

    match song {
      Some(song_2) => self.play_at::<Seconds>(&song_2, Seconds(destination.max(0.0)), tempo),
      None => Ok(()),
    }
  }

  pub fn wanting(&self, wanted: Wanted) -> Result<(), Never> {
    let Ok(mut state) = lock(&self.telling.state);

    let Ok(()) = request(&mut state, wanted);

    drop(state);
    self.telling.woken.notify_all();

    self.minding()
  }

  fn minding(&self) -> Result<(), Never> {
    let Ok(state) = lock(&self.telling.state);
    let Ok(mut awake) = lock(&self.awake);

    match (state.wanted, awake.is_some()) {
      (Wanted::Playing, true) => return Ok(()),
      (Wanted::Paused | Wanted::Stopped, _) => {
        let no_longer_needed = awake.take();

        drop(awake);
        drop(state);
        drop(no_longer_needed);

        return Ok(());
      }
      (Wanted::Playing, false) => {}
    }

    drop(awake);
    drop(state);

    let Ok(asked) = console_awake::inhibit(InhibitReason::FromSleeping);

    match asked {
      InhibitResult::Acquired(staying) => {
        let Ok(now) = lock(&self.telling.state);
        let Ok(mut current_lock) = lock(&self.awake);

        match (now.wanted, current_lock.is_none()) {
          (Wanted::Playing, true) => *current_lock = Some(staying),
          _no_lock_is_needed => {}
        }
      }
      InhibitResult::Failed(said) => console_core_logging::warn!("{said}"),
    }

    Ok(())
  }

  pub fn desired(&self) -> Result<Wanted, Never> {
    let Ok(state) = lock(&self.telling.state);

    Ok(state.wanted)
  }

  pub fn song(&self) -> Result<Option<PathBuf>, Never> {
    let Ok(state) = lock(&self.telling.state);

    Ok(state.job.song.clone())
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "downgrading the playback rendezvous gives video a clock while keeping shutdown with Sounding"
    )
  )]
  pub fn position_reader(&self) -> Result<PositionReader, Never> {
    Ok(PositionReader {
      playback: Arc::downgrade(&self.telling),
    })
  }

  pub fn position(&self) -> Result<f64, Never> {
    let Ok(Seconds(position)) = self.telling.position();

    Ok(position)
  }

  pub fn completed(&self) -> Result<Completion, Never> {
    let Ok(state) = lock(&self.telling.state);

    Ok(match (state.wanted, state.ended == Some(state.job.turn)) {
      (Wanted::Playing, true) => Completion::Ended,
      _still_active => Completion::Pending,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
  Ended,
  Pending,
}

impl Playback {
  fn position(&self) -> Result<Seconds, Never> {
    let Ok(state) = lock(&self.state);
    position_of(&state)
  }
}

fn position_of(state: &State) -> Result<Seconds, Never> {
  let latency = match state.wanted {
    Wanted::Playing => AHEAD,
    Wanted::Paused | Wanted::Stopped => Seconds::default(),
  };

  played_position(state, latency)
}

impl Drop for Sounding {
  #[cfg_attr(
    dylint_lib = "explicit057_no_unbounded_wait",
    expect(
      explicit057_no_unbounded_wait,
      reason = "the owned worker has acknowledged shutdown after its callbacks and pipe have closed before it is joined"
    )
  )]
  fn drop(&mut self) {
    let worker = match self.worker.take() {
      Some(worker) => worker,
      None => return,
    };
    let Ok(mut state) = lock(&self.telling.state);

    match state.lifetime {
      Lifetime::Running => state.lifetime = Lifetime::Closing,
      Lifetime::Closing | Lifetime::Closed => {}
    }

    self.telling.woken.notify_all();

    let stopped = self
      .telling
      .woken
      .wait_timeout_while(state, SHUTDOWN_WAIT, |current| {
        current.lifetime != Lifetime::Closed
      });
    let (state_2, _elapsed) = match stopped {
      Ok(held) => held,
      Err(poisoned) => poisoned.into_inner(),
    };
    let lifetime = state_2.lifetime;

    drop(state_2);

    match lifetime {
      Lifetime::Closed => match worker.join() {
        Ok(()) => {}
        Err(_panicked) => console_core_logging::warn!("the audio worker failed during shutdown"),
      },
      Lifetime::Running | Lifetime::Closing => {
        console_core_logging::warn!(
          "the audio worker did not acknowledge shutdown within two seconds"
        );
      }
    }
  }
}

fn request(state: &mut State, wanted: Wanted) -> Result<(), Never> {
  match (state.wanted, wanted) {
    (Wanted::Playing, Wanted::Paused) => {
      let Ok(()) = rebase_position(state);

      state.job.turn = state.job.turn.saturating_add(1);
      state.ended = None;
    }
    (_, Wanted::Stopped) => {
      state.job.song = None;
      state.job.turn = state.job.turn.saturating_add(1);
      state.ended = None;
    }
    _same_job => {}
  }

  state.wanted = wanted;

  Ok(())
}

fn played_position(state: &State, Seconds(ahead): Seconds) -> Result<Seconds, Never> {
  let Ok(written) = state.written.float();
  let carried = (written / f64::from(state.rate.max(1)) - ahead) * state.job.tempo.0;

  Ok(Seconds(
    (state.job.from.0 + carried).max(state.job.from.0).max(0.0),
  ))
}

#[cfg_attr(
  dylint_lib = "explicit057_no_unbounded_wait",
  expect(
    explicit057_no_unbounded_wait,
    reason = "the mutex guards only the short playback state updates and is released before callbacks, blocking I/O, and waiting for the worker"
  )
)]
fn lock<T>(what: &Mutex<T>) -> Result<MutexGuard<'_, T>, Never> {
  Ok(match what.lock() {
    Ok(held) => held,
    Err(poisoned) => poisoned.into_inner(),
  })
}

fn pumping(
  telling: &Playback,
  progress: &dyn Fn(Progress) -> Result<(), Never>,
) -> Result<(), Never> {
  let pumped = console_core_iteration::iterate((), |()| {
    let Ok(job) = wait_for_task(telling);
    let current = match job {
      Some(current) => current,
      None => return Ok(Step::Halt(())),
    };

    match current.song.as_deref() {
      None => {}
      Some(song) => {
        let Ok(reached) = one(telling, &current, song, progress);

        match reached {
          Reached::End => {
            let Ok(completed) = mark_ended(telling, current.turn);

            match completed {
              Completion::Ended => {
                let Ok(()) = progress(Progress::Ended);
              }
              Completion::Pending => {}
            }
          }
          Reached::Requested => {}
        }
      }
    }

    Ok(Step::Again(()))
  });

  match pumped {
    Ok(()) | Err(Endless) => Ok(()),
  }
}

fn wait_for_task(telling: &Playback) -> Result<Option<Task>, Never> {
  let Ok(state) = lock(&telling.state);
  let ready_to_run = telling.woken.wait_while(state, |current| {
    let Ok(available) = ready(current);

    current.lifetime == Lifetime::Running && available == Ready::No
  });
  let held = match ready_to_run {
    Ok(held) => held,
    Err(poisoned) => poisoned.into_inner(),
  };

  Ok(match held.lifetime {
    Lifetime::Running => Some(held.job.clone()),
    Lifetime::Closing | Lifetime::Closed => None,
  })
}

fn ready(state: &State) -> Result<Ready, Never> {
  Ok(match state.job.song {
    None => Ready::No,
    Some(ref _song) => match state.wanted {
      Wanted::Playing => {
        match ::core::primitive::bool::then_some(state.ended == Some(state.job.turn), ()) {
          ::core::option::Option::Some(()) => Ready::No,
          ::core::option::Option::None => Ready::Yes,
        }
      }
      Wanted::Paused | Wanted::Stopped => Ready::No,
    },
  })
}

fn rebase_position(state: &mut State) -> Result<(), Never> {
  let Ok(got) = played_position(state, AHEAD);

  state.job.from = got;
  state.written = 0;

  Ok(())
}

fn mark_ended(telling: &Playback, turn: u64) -> Result<Completion, Never> {
  let Ok(mut state) = lock(&telling.state);
  let Ok(carrying) = carry(&state, turn);

  match carrying {
    Payload::On => {
      state.ended = Some(turn);

      Ok(Completion::Ended)
    }
    Payload::Hold | Payload::Stop => Ok(Completion::Pending),
  }
}

fn one(
  telling: &Playback,
  job: &Task,
  song: &Path,
  progress: &dyn Fn(Progress) -> Result<(), Never>,
) -> Result<Reached, Never> {
  let Ok(mut waiting) = Waiting::here(Wait {
    who: "music-player",
    what: "sounding",
  });

  let mut decoding = match decoding::opened(song, job.from) {
    Ok(decoding) => decoding,
    Err(fault) => {
      console_core_logging::warn!("{}: {fault}", song.display());

      return Ok(Reached::End);
    }
  };

  let Ok(first) = decoding.decoded();

  let first_3 = match first {
    Some(first_2) => first_2,
    None => return Ok(Reached::End),
  };

  let Ok(()) = waiting.mark("decoder");
  let Ok(carrying) = began(telling, job.turn, first_3.format.rate);

  match carrying {
    Payload::On => {}
    Payload::Hold | Payload::Stop => return Ok(Reached::Requested),
  }

  let balance = match Balance::new(first_3.format.rate, first_3.format.channels) {
    Ok(balance) => balance,
    Err(fault) => {
      console_core_logging::warn!("could not prepare the volume reference: {fault}");
      return Ok(Reached::Requested);
    }
  };
  let Ok(playing) = sink(first_3.format);

  let mut playing_3 = match playing {
    Some(playing_2) => playing_2,
    None => return Ok(Reached::Requested),
  };

  let Ok(into) = playing_3.take_stdin();

  let stdin = match into {
    Some(stdin) => stdin,
    None => return Ok(Reached::Requested),
  };
  let input = match TimedPipe::new(OwnedFd::from(stdin)) {
    Ok(input) => input,
    Err(fault) => {
      console_core_logging::warn!("could not prepare the audio pipe: {fault}");

      return Ok(Reached::Requested);
    }
  };

  let Ok(()) = waiting.mark("sink");
  let Ok(stretching) = stretching(&first_3, job.tempo);
  let channels = first_3.format.channels.max(1);
  let pouring = Pouring {
    decoding,
    into: input,
    balance,
    stretching,
    channels,
    waiting: Some(first_3),
    sounded: Some(waiting),
  };

  let poured = console_core_iteration::iterate(pouring, |pouring_2| {
    poured(telling, job.turn, progress, pouring_2)
  });

  Ok(match poured {
    Ok(reached) => reached,
    Err(_endless) => Reached::Requested,
  })
}

fn stretching(first: &Block, tempo: Tempo) -> Result<Option<Stretching>, Never> {
  match ::core::primitive::bool::then_some(tempo == Tempo::default(), ()) {
    ::core::option::Option::Some(()) => Ok(None),
    ::core::option::Option::None => {
      let Ok(stretching) = Stretching::new(first.format, tempo);

      Ok(Some(stretching))
    }
  }
}

fn began(telling: &Playback, turn: u64, rate: u32) -> Result<Payload, Never> {
  let Ok(mut state) = lock(&telling.state);
  let Ok(carrying) = carry(&state, turn);

  match carrying {
    Payload::On => state.rate = rate,
    Payload::Hold | Payload::Stop => {}
  }

  Ok(carrying)
}

struct Pouring {
  decoding: Decoding,
  into: TimedPipe,
  balance: Balance,
  stretching: Option<Stretching>,
  channels: u32,
  waiting: Option<Block>,
  sounded: Option<Waiting>,
}

impl Pouring {
  fn heard(&mut self) -> Result<Option<Vec<f32>>, Never> {
    let next = match self.waiting.take() {
      Some(block) => Some(block),
      None => {
        let Ok(next) = self.decoding.decoded();

        next
      }
    };

    Ok(match (next, self.stretching.as_mut()) {
      (Some(block), None) => Some(block.samples),
      (Some(block), Some(stretching)) => {
        let Ok(laid) = stretching.stretched(&block.samples);

        Some(laid)
      }
      (None, _) => None,
    })
  }
}

fn poured(
  telling: &Playback,
  turn: u64,
  progress: &dyn Fn(Progress) -> Result<(), Never>,
  mut pouring: Pouring,
) -> Result<Step<Pouring, Reached>, Never> {
  let Ok(carry) = carrying_on(telling, turn);

  match carry {
    Payload::Hold | Payload::Stop => return Ok(Step::Halt(Reached::Requested)),
    Payload::On => {}
  }

  let Ok(heard) = pouring.heard();

  let mut samples = match heard {
    Some(samples) => samples,
    None => return Ok(Step::Halt(Reached::End)),
  };
  let Ok(()) = pouring.balance.apply(&mut samples);

  let bytes: Vec<u8> = samples
    .iter()
    .flat_map(|sample| sample.to_le_bytes())
    .collect();

  let Ok(delivered) = write_samples(telling, turn, &pouring.into, &bytes);

  match delivered {
    Delivered::Yes => {}
    Delivered::Interrupted => return Ok(Step::Halt(Reached::Requested)),
  }

  let Ok(written) = console_core_number_conversion::fitted::<_, u64>(samples.len());

  let frames = match written.checked_div(u64::from(pouring.channels)) {
    Some(frames) => frames,
    None => 0,
  };

  let Ok(crossed) = wrote(telling, turn, Frames(frames));

  match crossed {
    Crossed::ASecond => {
      let Ok(()) = progress(Progress::ASecondPlayed);
    }
    Crossed::None => {}
  }

  match pouring.sounded.take() {
    Some(mut waiting) => {
      let Ok(()) = waiting.mark("first");
      let Ok(()) = waiting.done_if_felt();
    }
    None => {}
  }

  Ok(Step::Again(pouring))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delivered {
  Yes,
  Interrupted,
}

fn write_samples(
  telling: &Playback,
  turn: u64,
  into: &TimedPipe,
  bytes: &[u8],
) -> Result<Delivered, Never> {
  let written = console_core_iteration::iterate(bytes, |pending| {
    let Ok(carrying) = carrying_on(telling, turn);

    match (carrying, pending) {
      (Payload::Hold | Payload::Stop, _) => return Ok(Step::Halt(Delivered::Interrupted)),
      (Payload::On, []) => return Ok(Step::Halt(Delivered::Yes)),
      (Payload::On, [_, ..]) => {}
    }

    Ok(match into.write_some(pending, PIPE_WAIT) {
      Ok(0) => Step::Halt(Delivered::Interrupted),
      Ok(count) => {
        let Ok(length) = index(count);

        match pending.get(length..) {
          Some(rest) => Step::Again(rest),
          None => Step::Halt(Delivered::Interrupted),
        }
      }
      Err(fault) => match bool::then_some(
        matches!(
          fault.kind(),
          ErrorKind::Interrupted | ErrorKind::WouldBlock | ErrorKind::TimedOut
        ),
        (),
      ) {
        Some(()) => Step::Again(pending),
        None => {
          console_core_logging::warn!("could not write audio samples: {fault}");

          Step::Halt(Delivered::Interrupted)
        }
      },
    })
  });

  Ok(match written {
    Ok(delivered) => delivered,
    Err(_endless) => Delivered::Interrupted,
  })
}

struct Frames(u64);

fn wrote(telling: &Playback, turn: u64, Frames(frames): Frames) -> Result<Crossed, Never> {
  let Ok(mut state) = lock(&telling.state);
  let Ok(carrying) = carry(&state, turn);

  match carrying {
    Payload::On => {}
    Payload::Hold | Payload::Stop => return Ok(Crossed::None),
  }

  let a_second = u64::from(state.rate);
  let before = state.written.checked_div(a_second);

  state.written = state.written.saturating_add(frames);

  Ok(
    match ::core::primitive::bool::then_some(before == state.written.checked_div(a_second), ()) {
      ::core::option::Option::Some(()) => Crossed::None,
      ::core::option::Option::None => Crossed::ASecond,
    },
  )
}

fn carrying_on(telling: &Playback, turn: u64) -> Result<Payload, Never> {
  let Ok(state) = lock(&telling.state);

  carry(&state, turn)
}

fn carry(state: &State, turn: u64) -> Result<Payload, Never> {
  match bool::then_some(
    state.lifetime == Lifetime::Running && state.job.turn == turn,
    (),
  ) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(Payload::Stop),
  }

  Ok(match state.wanted {
    Wanted::Stopped => Payload::Stop,
    Wanted::Playing => Payload::On,
    Wanted::Paused => Payload::Hold,
  })
}

fn sink(format: Format) -> Result<Option<BoundToParent>, Never> {
  let Ok(mut asking) = Program::PwCat.command();

  asking
    .args([
      "--playback",
      "--raw",
      "--format",
      SAMPLE,
      "--rate",
      &format.rate.to_string(),
    ])
    .args([
      "--channels",
      &format.channels.to_string(),
      "--latency",
      LATENCY,
      "-",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::null());

  Ok(match alongside(&mut asking) {
    Ok(playing) => Some(playing),
    Err(fault) => {
      console_core_logging::warn!("nothing to play through: {fault}");

      None
    }
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  const RATE: u32 = 44_100;

  const A_SECOND: u64 = 44_100;

  fn at(from: Seconds, wanted: Wanted, written: u64) -> Result<Playback, Never> {
    Ok(Playback {
      state: Mutex::new(State {
        job: Task {
          song: Some(PathBuf::from("a-song.flac")),
          from,
          tempo: Tempo::default(),
          turn: 1,
        },
        wanted,
        rate: RATE,
        written,
        ended: None,
        lifetime: Lifetime::Running,
      }),
      woken: Condvar::new(),
    })
  }

  #[test]
  fn a_write_that_carries_the_song_past_a_whole_second_says_so_and_one_within_it_does_not() {
    let Ok(telling) = at(Seconds(0.0), Wanted::Playing, A_SECOND.saturating_sub(10));

    assert_eq!(wrote(&telling, 1, Frames(5)), Ok(Crossed::None));
    assert_eq!(wrote(&telling, 1, Frames(10)), Ok(Crossed::ASecond));
    assert_eq!(wrote(&telling, 1, Frames(10)), Ok(Crossed::None));
  }

  #[test]
  fn a_pause_stops_carrying_rather_than_standing_there_holding_the_pipe() {
    let Ok(telling) = at(Seconds(0.0), Wanted::Paused, 0);
    let Ok(carry) = carrying_on(&telling, 1);

    assert_eq!(carry, Payload::Hold);
  }

  #[test]
  fn what_is_kept_at_a_pause_is_what_was_heard_and_not_what_was_written() {
    let Ok(telling) = at(Seconds(30.0), Wanted::Playing, A_SECOND.saturating_mul(10));
    let Ok(mut state) = lock(&telling.state);
    let Ok(()) = request(&mut state, Wanted::Paused);

    assert!(
      (state.job.from.0 - (40.0 - AHEAD.0)).abs() < 0.001,
      "{}",
      state.job.from.0
    );
    assert_eq!(state.written, 0);
  }

  #[test]
  fn the_position_reader_ends_with_the_audio_owner() {
    let Ok(sound) = Sounding::new(|_progress| Ok(()));
    let Ok(position) = sound.position_reader();

    assert_eq!(position.position(), Ok(Some(0.0)));
    drop(sound);
    assert_eq!(position.position(), Ok(None));
  }

  #[test]
  fn a_paused_position_is_the_offset_itself_rather_than_a_third_of_a_second_before_it() {
    let Ok(telling) = at(Seconds(30.0), Wanted::Paused, 0);
    let Ok(Seconds(where_it_is)) = telling.position();

    assert!((where_it_is - 30.0).abs() < 0.001, "{where_it_is}");
  }

  #[test]
  fn pausing_twice_over_does_not_walk_back_through_the_song() {
    let Ok(telling) = at(Seconds(30.0), Wanted::Playing, A_SECOND.saturating_mul(10));
    let Ok(mut state) = lock(&telling.state);
    let Ok(()) = request(&mut state, Wanted::Paused);
    let once = state.job.from.0;
    let Ok(()) = request(&mut state, Wanted::Paused);

    assert!(
      (state.job.from.0 - once).abs() < 0.001,
      "{once} then {}",
      state.job.from.0
    );
  }

  #[test]
  fn a_playing_position_still_stands_where_the_speaker_is() {
    let Ok(telling) = at(Seconds(30.0), Wanted::Playing, A_SECOND.saturating_mul(10));
    let Ok(Seconds(where_it_is)) = telling.position();

    assert!(
      (where_it_is - (40.0 - AHEAD.0)).abs() < 0.001,
      "{where_it_is}"
    );
  }

  #[test]
  fn at_twice_the_speed_a_second_heard_is_two_seconds_of_the_film() {
    let Ok(telling) = at(Seconds(30.0), Wanted::Playing, A_SECOND.saturating_mul(10));
    let Ok(mut state) = lock(&telling.state);

    state.job.tempo = Tempo(2.0);
    drop(state);

    let Ok(Seconds(where_it_is)) = telling.position();

    assert!(
      (where_it_is - (30.0 + (10.0 - AHEAD.0) * 2.0)).abs() < 0.001,
      "{where_it_is}"
    );
  }

  #[test]
  fn a_cancelled_run_cannot_write_into_the_resumed_run() {
    let Ok(telling) = at(Seconds(30.0), Wanted::Playing, A_SECOND);
    let Ok(mut state) = lock(&telling.state);
    let Ok(()) = request(&mut state, Wanted::Paused);
    let Ok(()) = request(&mut state, Wanted::Playing);

    drop(state);

    assert_eq!(wrote(&telling, 1, Frames(A_SECOND)), Ok(Crossed::None));
    assert_eq!(mark_ended(&telling, 1), Ok(Completion::Pending));

    let Ok(current) = lock(&telling.state);

    assert_eq!(current.written, 0);
    assert_eq!(current.ended, None);
  }

  #[test]
  fn dropping_an_idle_player_closes_its_worker_callback() {
    let (notify, closed) = std::sync::mpsc::sync_channel::<()>(1);
    let Ok(sounding) = Sounding::new(move |_progress| {
      let _held_until_the_worker_exits = &notify;

      Ok(())
    });

    assert!(sounding.worker.is_some());
    drop(sounding);

    assert!(matches!(
      closed.try_recv(),
      Err(std::sync::mpsc::TryRecvError::Disconnected)
    ));
  }
}
