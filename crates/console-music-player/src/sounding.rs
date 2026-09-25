//! Sound, as a pipe with this program standing in the middle of it.
//!
//! ffmpeg writes one song out as plain samples and pw-cat plays what it is
//! handed. Between them is a loop in a thread of its own, and that loop is
//! every control this player has: pausing ends the pipe, position is how much
//! has been written, and stopping is a pipe that closes. None of the three is a
//! question asked of someone else's program, which is what made every one of
//! kew's answers a thing to be waited for rather than read.
//!
//! **A pause that keeps the pipe keeps the machine awake.** Not writing was
//! the cheaper pause and it left ffmpeg and pw-cat standing with the stream
//! open, which is a sink PipeWire will not suspend: the graph goes on running,
//! the codec stays powered, and a handheld with the music paused draws what a
//! handheld playing music draws. So a pause ends both programs and keeps the
//! one thing that cannot be worked out again, which is where in the song it
//! got to. Starting again is the seek this file already had to write, because
//! nothing can tell a running ffmpeg to go somewhere else anyway.
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
//! the pipe and in pw-cat's own buffer go with the programs, so folding the raw
//! count into the offset would step over [`AHEAD`] of music every time someone
//! pressed pause. Taking it off costs at worst a third of a second heard twice,
//! which is the direction to be wrong in.
//!
//! Seeking is the one that costs. Nothing can tell a running ffmpeg to go
//! somewhere else, so a seek ends the one that is running and starts another at
//! the offset. GStreamer would seek without opening the file again; `lib.rs`
//! says why that was not enough to take it.
//!
//! What position is, exactly. Samples are counted as they are written, at
//! forty-eight thousand a second, two channels of them, two bytes each. What
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
//! speed is ffmpeg's `atempo` between the decoder and the pipe, so what is
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
//! is spent in three places this thread can see: starting the decoder, starting
//! the sink, and waiting for the first samples to come out of one and be taken
//! by the other. So the stopwatch is [`one`]'s own and it ends at the first
//! write, not at the end of the song, and a run that never reaches a first
//! write -- a decoder that would not start, a press overtaken by the next one --
//! writes nothing, because no one waited for a sound that never came.
//!
//! The thread decides nothing about music. It is told a song and an offset, and
//! it carries samples until someone says otherwise or the song ends. When a
//! song ends it says so and stops there, and each whole second it carries it
//! says that too, because the clock under a song is the one thing nobody else
//! can see moving: what plays next is the playlist's business, the playlist belongs to the main thread, and a thread that reached
//! across to it would be the shape of fault kew had -- one list walked by
//! whoever got there first.

use console_awake::{InhibitResult, InhibitReason, Staying};
use console_core_external_programs::Program;
use console_core_never::Never;
use console_core_number_conversion::Float;
use console_program_lifetime::{BoundToParent, alongside};
use console_response_times::{Wait, Waiting};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;

pub const RATE: &str = "48000";

pub const CHANNELS: &str = "2";

pub const SAMPLE: &str = "s16";

pub const LATENCY: &str = "50ms";

const A_SECOND: u64 = 192_000;

const AHEAD: f64 = 0.383;

const CHUNK: u32 = 8_192;

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
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reached {
    End,
    PlayerState,
    Requested,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tempo(pub f64);

impl Default for Tempo {
    fn default() -> Tempo {
        Tempo(1.0)
    }
}

#[derive(Debug, Clone, Default)]
struct Task {
    song: Option<PathBuf>,
    from: f64,
    tempo: Tempo,
    turn: u64,
}

#[derive(Debug, Default)]
struct State {
    job: Task,
    wanted: Wanted,
    written: u64,
    ended: Option<u64>,
}

struct Playback {
    state: Mutex<State>,
    woken: Condvar,
}

pub struct Sounding {
    telling: Arc<Playback>,
    awake: Mutex<Option<Staying>>,
}

impl Sounding {
    pub fn new<E>(telling_progress: E) -> Result<Sounding, Never>
    where
        E: Fn(Progress) -> Result<(), Never> + Send + 'static,
    {
        let telling = Arc::new(Playback {
            state: Mutex::new(State::default()),
            woken: Condvar::new(),
        });
        let carrying = Arc::clone(&telling);

        let _the_thread_lives_as_long_as_this_program = thread::spawn(move || {
            let Ok(()) = pumping(&carrying, &telling_progress);
        });

        Ok(Sounding { telling, awake: Mutex::new(None) })
    }

    pub fn play(&self, song: &Path, from: f64) -> Result<(), Never> {
        self.play_at(song, from, Tempo::default())
    }

    pub fn play_at(&self, song: &Path, from: f64, tempo: Tempo) -> Result<(), Never> {
        let Ok(()) = self.taking(song, from, tempo, Wanted::Playing);

        self.telling.woken.notify_all();

        Ok(())
    }

    pub fn ready(&self, song: &Path, from: f64) -> Result<(), Never> {
        self.taking(song, from, Tempo::default(), Wanted::Paused)
    }

    fn taking(&self, song: &Path, from: f64, tempo: Tempo, wanted: Wanted) -> Result<(), Never> {
        let Ok(mut state) = held(&self.telling.state);

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

        self.minding(wanted)
    }

    pub fn seek(&self, to: f64) -> Result<(), Never> {
        let Ok(state) = held(&self.telling.state);
        let song = state.job.song.clone();
        let tempo = state.job.tempo;

        drop(state);

        match song {
            Some(song) => self.play_at(&song, to.max(0.0), tempo),
            None => Ok(()),
        }
    }

    pub fn wanting(&self, wanted: Wanted) -> Result<(), Never> {
        let Ok(mut state) = held(&self.telling.state);

        state.wanted = wanted;

        match wanted {
            Wanted::Stopped => state.job.song = None,
            Wanted::Playing | Wanted::Paused => {},
        }

        drop(state);
        self.telling.woken.notify_all();

        self.minding(wanted)
    }

    fn minding(&self, wanted: Wanted) -> Result<(), Never> {
        let Ok(mut awake) = held(&self.awake);

        match wanted {
            Wanted::Playing => match *awake {
                Some(_) => {}
                None => {
                    let Ok(asked) = console_awake::taking(InhibitReason::FromSleeping);

                    match asked {
                        InhibitResult::Acquired(staying) => *awake = Some(staying),
                        InhibitResult::Failed(said) => eprintln!("music-player: {said}"),
                    }
                }
            },
            Wanted::Paused | Wanted::Stopped => *awake = None,
        }

        Ok(())
    }

    pub fn wanted(&self) -> Result<Wanted, Never> {
        let Ok(state) = held(&self.telling.state);

        Ok(state.wanted)
    }

    pub fn song(&self) -> Result<Option<PathBuf>, Never> {
        let Ok(state) = held(&self.telling.state);

        Ok(state.job.song.clone())
    }

    pub fn position(&self) -> Result<f64, Never> {
        let Ok(state) = held(&self.telling.state);
        let Ok(ahead) = ahead(state.wanted);
        let Ok(got) = heard(&state, ahead);

        Ok(got)
    }
}

fn ahead(wanted: Wanted) -> Result<f64, Never> {
    Ok(match wanted {
        Wanted::Playing => AHEAD,
        Wanted::Paused | Wanted::Stopped => 0.0,
    })
}

fn heard(state: &State, ahead: f64) -> Result<f64, Never> {
    let Ok(written) = state.written.float();
    let Ok(a_second) = A_SECOND.float();
    let carried = (written / a_second - ahead) * state.job.tempo.0;

    Ok((state.job.from + carried).max(state.job.from).max(0.0))
}

fn held<T>(what: &Mutex<T>) -> Result<MutexGuard<'_, T>, Never> {
    Ok(match what.lock() {
        Ok(held) => held,
        Err(poisoned) => poisoned.into_inner(),
    })
}

fn pumping(telling: &Arc<Playback>, progress: &dyn Fn(Progress) -> Result<(), Never>) -> Result<(), Never> {
    loop {
        let Ok(job) = waited(telling);

        match job.song.clone() {
            None => {},
            Some(song) => {
                let Ok(reached) = one(telling, &job, &song, progress);

                match reached {
                    Reached::End => {
                        let Ok(()) = done(telling, job.turn);

                        let Ok(()) = progress(Progress::Ended);
                    },
                    Reached::PlayerState => {
                        let Ok(()) = holding(telling);
                    },
                    Reached::Requested => {},
                }
            },
        }
    }
}

fn waited(telling: &Arc<Playback>) -> Result<Task, Never> {
    let Ok(mut state) = held(&telling.state);

    loop {
        let Ok(ready) = ready(&state);

        match ready {
            Ready::Yes => return Ok(state.job.clone()),
            Ready::No => {
                state = match telling.woken.wait(state) {
                    Ok(state) => state,
                    Err(poisoned) => poisoned.into_inner(),
                };
            },
        }
    }
}

fn ready(state: &State) -> Result<Ready, Never> {
    Ok(match state.job.song {
        None => Ready::No,
        Some(ref _song) => match state.wanted {
            Wanted::Playing => match state.ended == Some(state.job.turn) {
                true => Ready::No,
                false => Ready::Yes,
            },
            Wanted::Paused | Wanted::Stopped => Ready::No,
        },
    })
}

fn holding(telling: &Arc<Playback>) -> Result<(), Never> {
    let Ok(mut state) = held(&telling.state);
    let Ok(got) = heard(&state, AHEAD);

    state.job.from = got;
    state.written = 0;

    Ok(())
}

fn done(telling: &Arc<Playback>, turn: u64) -> Result<(), Never> {
    let Ok(mut state) = held(&telling.state);

    state.ended = Some(turn);

    Ok(())
}

fn one(
    telling: &Arc<Playback>,
    job: &Task,
    song: &Path,
    progress: &dyn Fn(Progress) -> Result<(), Never>,
) -> Result<Reached, Never> {
    let Ok(mut waiting) = Waiting::here(Wait { who: "music-player", what: "sounding" });
    let Ok(decoding) = reading(song, job.from, job.tempo);

    let mut decoding = match decoding {
        Some(decoding) => decoding,
        None => return Ok(Reached::Requested),
    };

    let Ok(()) = waiting.mark("ffmpeg");
    let Ok(playing) = sink();

    let mut playing = match playing {
        Some(playing) => playing,
        None => return Ok(Reached::Requested),
    };

    let Ok(out) = decoding.reading();
    let Ok(into) = playing.writing();

    let mut out = match out {
        Some(out) => out,
        None => return Ok(Reached::Requested),
    };

    let mut into = match into {
        Some(into) => into,
        None => return Ok(Reached::Requested),
    };

    let Ok(()) = waiting.mark("sink");
    let Ok(chunk) = console_core_number_conversion::index(CHUNK);
    let mut buffer = vec![0; chunk];
    let mut sounded = Some(waiting);

    loop {
        let Ok(carry) = carrying_on(telling, job.turn);

        match carry {
            Payload::Stop => return Ok(Reached::Requested),
            Payload::Hold => return Ok(Reached::PlayerState),
            Payload::On => {},
        }

        let carried = match out.read(&mut buffer) {
            Ok(0) => return Ok(Reached::End),
            Ok(read) => match buffer.get(..read) {
                Some(carried) => carried,
                None => return Ok(Reached::Requested),
            },
            Err(_the_decoder_has_gone) => return Ok(Reached::Requested),
        };

        match into.write_all(carried) {
            Ok(()) => {},
            Err(_the_sink_has_gone) => return Ok(Reached::Requested),
        }

        let Ok(read) = console_core_number_conversion::fitted::<_, u64>(carried.len());
        let Ok(crossed) = wrote(telling, read);

        match crossed {
            Crossed::ASecond => {
                let Ok(()) = progress(Progress::ASecondPlayed);
            },
            Crossed::Nothing => {},
        }

        match sounded.take() {
            Some(mut waiting) => {
                let Ok(()) = waiting.mark("first");
                let Ok(()) = waiting.done_if_felt();
            }
            None => {},
        }
    }
}

fn wrote(telling: &Arc<Playback>, read: u64) -> Result<Crossed, Never> {
    let Ok(mut state) = held(&telling.state);
    let before = state.written.checked_div(A_SECOND);

    state.written = state.written.saturating_add(read);

    Ok(match before == state.written.checked_div(A_SECOND) {
        true => Crossed::Nothing,
        false => Crossed::ASecond,
    })
}

fn carrying_on(telling: &Arc<Playback>, turn: u64) -> Result<Payload, Never> {
    let Ok(state) = held(&telling.state);

    match state.job.turn == turn {
        true => {},
        false => return Ok(Payload::Stop),
    }

    Ok(match state.wanted {
        Wanted::Stopped => Payload::Stop,
        Wanted::Playing => Payload::On,
        Wanted::Paused => Payload::Hold,
    })
}

fn reading(song: &Path, from: f64, tempo: Tempo) -> Result<Option<BoundToParent>, Never> {
    let Ok(mut asking) = Program::Ffmpeg.command();

    asking
        .args(["-v", "quiet", "-nostdin", "-ss", &format!("{from}"), "-i"])
        .arg(song)
        .args(["-vn", "-af", &format!("atempo={}", tempo.0)])
        .args(["-f", "s16le", "-ar", RATE, "-ac", CHANNELS, "-"])
        .stdout(Stdio::piped())
        .stdin(Stdio::null())
        .stderr(Stdio::null());

    Ok(match alongside(&mut asking) {
        Ok(decoding) => Some(decoding),
        Err(fault) => {
            eprintln!("music-player: {}: will not decode: {fault}", song.display());

            None
        },
    })
}

fn sink() -> Result<Option<BoundToParent>, Never> {
    let Ok(mut asking) = Program::PwCat.command();

    asking
        .args(["--playback", "--raw", "--format", SAMPLE, "--rate", RATE])
        .args(["--channels", CHANNELS, "--latency", LATENCY, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    Ok(match alongside(&mut asking) {
        Ok(playing) => Some(playing),
        Err(fault) => {
            eprintln!("music-player: nothing to play through: {fault}");

            None
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(from: f64, wanted: Wanted, written: u64) -> Arc<Playback> {
        Arc::new(Playback {
            state: Mutex::new(State {
                job: Task { song: Some(PathBuf::from("a-song.flac")), from, tempo: Tempo::default(), turn: 1 },
                wanted,
                written,
                ended: None,
            }),
            woken: Condvar::new(),
        })
    }

    #[test]
    fn a_write_that_carries_the_song_past_a_whole_second_says_so_and_one_within_it_does_not() {
        let telling = at(0.0, Wanted::Playing, A_SECOND - 10);

        assert_eq!(wrote(&telling, 5), Ok(Crossed::Nothing));
        assert_eq!(wrote(&telling, 10), Ok(Crossed::ASecond));
        assert_eq!(wrote(&telling, 10), Ok(Crossed::Nothing));
    }

    #[test]
    fn a_pause_stops_carrying_rather_than_standing_there_holding_the_pipe() {
        let telling = at(0.0, Wanted::Paused, 0);
        let Ok(carry) = carrying_on(&telling, 1);

        assert_eq!(carry, Payload::Hold);
    }

    #[test]
    fn what_is_kept_at_a_pause_is_what_was_heard_and_not_what_was_written() {
        let telling = at(30.0, Wanted::Paused, A_SECOND * 10);
        let Ok(()) = holding(&telling);
        let Ok(state) = held(&telling.state);

        assert!((state.job.from - (40.0 - AHEAD)).abs() < 0.001, "{}", state.job.from);
        assert_eq!(state.written, 0);
    }

    #[test]
    fn a_paused_position_is_the_offset_itself_rather_than_a_third_of_a_second_before_it() {
        let sounding = Sounding { telling: at(30.0, Wanted::Paused, 0), awake: Mutex::new(None) };
        let Ok(where_it_is) = sounding.position();

        assert!((where_it_is - 30.0).abs() < 0.001, "{where_it_is}");
    }

    #[test]
    fn pausing_twice_over_does_not_walk_back_through_the_song() {
        let telling = at(30.0, Wanted::Paused, A_SECOND * 10);
        let Ok(()) = holding(&telling);
        let Ok(state) = held(&telling.state);
        let once = state.job.from;

        drop(state);

        let Ok(()) = holding(&telling);
        let Ok(state) = held(&telling.state);

        assert!((state.job.from - once).abs() < 0.001, "{once} then {}", state.job.from);
    }

    #[test]
    fn a_playing_position_still_stands_where_the_speaker_is() {
        let sounding =
            Sounding { telling: at(30.0, Wanted::Playing, A_SECOND * 10), awake: Mutex::new(None) };
        let Ok(where_it_is) = sounding.position();

        assert!((where_it_is - (40.0 - AHEAD)).abs() < 0.001, "{where_it_is}");
    }

    #[test]
    fn at_twice_the_speed_a_second_heard_is_two_seconds_of_the_film() {
        let telling = at(30.0, Wanted::Playing, A_SECOND * 10);
        let Ok(mut state) = held(&telling.state);

        state.job.tempo = Tempo(2.0);
        drop(state);

        let sounding = Sounding { telling, awake: Mutex::new(None) };
        let Ok(where_it_is) = sounding.position();

        assert!((where_it_is - (30.0 + (10.0 - AHEAD) * 2.0)).abs() < 0.001, "{where_it_is}");
    }
}
