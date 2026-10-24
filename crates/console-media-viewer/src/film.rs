//! A film, played: its sound through the speaker and its pictures onto the card.
//!
//! The sound and the pictures are read apart, from the same place in the film,
//! and kept together by one clock. The sound is `console_music_player`'s
//! `Sounding`, which is already the one place on this device that turns a file
//! into something heard, and it already knows how far through the file the
//! speaker has got. So the sound is the clock: each picture comes out of
//! `console_films` knowing which moment of the film it is, and is
//! handed to the card when the sound reaches that moment. A picture whose
//! moment has passed by the time it is decoded is not coloured or drawn, so a
//! slow machine shows fewer pictures of a film that stays in step with its
//! sound instead of every picture of one that falls behind it. A film with no
//! sound has no speaker to follow, and follows the wall instead.
//!
//! **Nothing is paused.** A pause lets the pictures go and keeps where the
//! film had got to, which is `Sounding`'s own argument for the music player
//! and it holds here for the same reasons: a decoder held open keeps the
//! machine awake for nobody. A seek, a speed and a choice of subtitles are the
//! same: the film is started again from where it is, with the new thing asked
//! for. [`next`] is that decision, and it is arithmetic, so it is what the
//! tests press.
//!
//! **The card is told, not asked.** Each picture goes to `console_panel::frames`,
//! which repaints the picture's own rectangle and nothing else, and once a
//! second of film the rows are asked for again so that the clock and the bar
//! under the picture move on. The words, the bar and the buttons are painted
//! once a second and the film as often as it has pictures.
//!
//! **Subtitles go with the picture they are said over.** libass drew them into
//! the frames when ffmpeg made the frames. Now each picture is handed to the
//! card with the lines due at its moment, and the card draws them over the
//! foot of the picture in its own letters -- so a line cannot fall out of step
//! with the picture, and it is as legible as every other word on the card. The
//! lines come from a track in the film or a file beside it, read by
//! `console_core_subtitle_files`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use console_films::Picture;
use console_concurrency::Cores;
use console_core_geometry::Size;
use console_core_iteration::Step;
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_u64};
use console_core_shapes::Pixels;
use console_core_subtitle_files::{Cue, Written};
use console_music_player::sounding::{Sounding, Tempo, Wanted};

pub use console_films::Sound;

use crate::playing::{self, Captions};

#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
    pub rate: f64,
    pub seconds: f64,
    pub sound: Sound,
    pub words: u32,
}

const ORDINARY_RATE: f64 = 30.0;

const FASTEST_RATE: f64 = 60.0;

pub fn facts(film: &Path) -> Result<Option<Facts>, Never> {
    let opened = console_films::open(film);

    let opened = match opened {
        Ok(Some(opened)) => opened,
        Ok(None) => return Ok(None),
        Err(fault) => {
            eprintln!("viewer: {fault}");

            return Ok(None);
        },
    };

    let Ok(facts) = opened.facts();

    let facts = match facts {
        Some(facts) => facts,
        None => return Ok(None),
    };

    let Ok(words) = fitted::<_, u32>(facts.subtitles.len());

    Ok(Some(Facts {
        rate: match facts.rate > 0.0 {
            true => facts.rate.min(FASTEST_RATE),
            false => ORDINARY_RATE,
        },
        seconds: facts.seconds.max(0.0),
        sound: facts.sound,
        words,
    }))
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackRequest {
    pub film: PathBuf,
    pub from: f64,
    pub speed: u32,
    pub captions: Captions,
    pub sought: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    None,
    Start,
    Stop,
}

pub fn next(playing: Option<&PlaybackRequest>, wanted: Option<&PlaybackRequest>) -> Result<Next, Never> {
    Ok(match (playing, wanted) {
        (None, None) => Next::None,
        (Some(_), None) => Next::Stop,
        (None, Some(_)) => Next::Start,
        (Some(playing), Some(wanted)) => {
            let same = playing.film == wanted.film
                && playing.speed == wanted.speed
                && playing.captions == wanted.captions
                && playing.sought == wanted.sought;

            match same {
                true => Next::None,
                false => Next::Start,
            }
        },
    })
}

pub fn sidecars(film: &Path) -> Result<Vec<PathBuf>, Never> {
    let name = match film.file_name().and_then(|name| name.to_str()) {
        Some(name) => name,
        None => return Ok(Vec::new()),
    };

    let folder = match film.parent() {
        Some(folder) => folder,
        None => return Ok(Vec::new()),
    };

    let Ok(said) = playing::beside(name);

    Ok(said.into_iter().map(|name| folder.join(name)).filter(|at| at.is_file()).collect())
}

fn cues(at: &Path, film: &console_films::Film, captions: Captions) -> Result<Vec<Cue>, Never> {
    let Ok(track) = captions.track();

    let track = match track {
        Some(track) => track,
        None => return Ok(Vec::new()),
    };

    let Ok(facts) = film.facts();

    let inside = match facts {
        Some(facts) => facts.subtitles,
        None => Vec::new(),
    };

    let Ok(wanted) = index(track);

    match inside.get(wanted) {
        Some(subtitles) => {
            return Ok(match film.cues(subtitles) {
                Ok(cues) => cues,
                Err(fault) => {
                    eprintln!("viewer: {fault}");

                    Vec::new()
                },
            });
        },
        None => {},
    }

    let Ok(inside) = fitted::<_, u32>(inside.len());
    let Ok(beyond) = index(track.saturating_sub(inside));
    let Ok(beside) = sidecars(at);

    let file = match beside.get(beyond) {
        Some(file) => file,
        None => return Ok(Vec::new()),
    };

    let Ok(written) = Written::of(file);

    let (written, bytes) = match (written, std::fs::read(file)) {
        (Some(written), Ok(bytes)) => (written, bytes),
        (None, _) => return Ok(Vec::new()),
        (Some(_), Err(fault)) => {
            eprintln!("viewer: {}: {fault}", file.display());

            return Ok(Vec::new());
        },
    };

    console_core_subtitle_files::read(&bytes, written)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Going {
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    Yes,
    No,
}

struct Shared {
    going: Mutex<Going>,
    woken: Condvar,
    ended: Mutex<Ended>,
}

enum Clock {
    Audio(Arc<Sounding>),
    Wall { from: f64, tempo: f64, began: Instant },
}

impl Clock {
    fn now(&self) -> Result<f64, Never> {
        match self {
            Clock::Audio(sound) => sound.position(),
            Clock::Wall { from, tempo, began } => Ok(from + began.elapsed().as_secs_f64() * tempo),
        }
    }
}

pub struct Film {
    pub asked: PlaybackRequest,
    clock: Arc<Clock>,
    shared: Arc<Shared>,
    sound: Arc<Sounding>,
}

impl Film {
    pub fn start(asked: PlaybackRequest, facts: &Facts, room: Size<u32>, sound: Arc<Sounding>) -> Result<Film, Never> {
        let Ok((_, tempo)) = playing::speed(asked.speed);

        let clock = match facts.sound {
            Sound::Audible => {
                let Ok(()) = sound.play_at(&asked.film, asked.from, Tempo(tempo));

                Clock::Audio(Arc::clone(&sound))
            },
            Sound::Silent => {
                let Ok(()) = sound.wanting(Wanted::Stopped);

                #[cfg_attr(
                    dylint_lib = "explicit039_no_reading_the_clock",
                    allow(
                        explicit039_no_reading_the_clock,
                        reason = "a film with no sound keeps time by the wall, and this is the one reading the whole of its playing is counted from"
                    )
                )]
                let began = Instant::now();

                Clock::Wall { from: asked.from, tempo, began }
            },
        };

        let clock = Arc::new(clock);
        let shared = Arc::new(Shared {
            going: Mutex::new(Going::On),
            woken: Condvar::new(),
            ended: Mutex::new(Ended::No),
        });

        let Ok(cores) = Cores::counted();
        let showing = Showing {
            film: asked.film.clone(),
            from: asked.from,
            tempo,
            rate: facts.rate,
            size: room,
            captions: asked.captions,
            cores,
        };
        let (carrying, timing) = (Arc::clone(&shared), Arc::clone(&clock));
        let pictures = std::thread::spawn(move || {
            let Ok(()) = shown(&showing, &timing, &carrying);
        });
        let Ok(()) = console_program_lifetime::threads::let_go(pictures);

        Ok(Film { asked, clock, shared, sound })
    }

    pub fn at(&self) -> Result<f64, Never> {
        self.clock.now()
    }

    pub fn ended(&self) -> Result<Ended, Never> {
        Ok(match self.shared.ended.lock() {
            Ok(ended) => *ended,
            Err(_the_pictures_gave_up) => Ended::Yes,
        })
    }
}

impl Drop for Film {
    fn drop(&mut self) {
        match self.shared.going.lock() {
            Ok(mut going) => *going = Going::Off,
            Err(_the_pictures_gave_up) => {},
        }

        self.shared.woken.notify_all();

        let Ok(()) = self.sound.wanting(Wanted::Stopped);
    }
}

struct Showing {
    film: PathBuf,
    from: f64,
    tempo: f64,
    rate: f64,
    size: Size<u32>,
    captions: Captions,
    cores: Cores,
}

fn state(shared: &Shared) -> Result<Going, Never> {
    Ok(match shared.going.lock() {
        Ok(going) => *going,
        Err(_the_film_gave_up) => Going::Off,
    })
}

const LONGEST_WAIT: f64 = 0.25;

fn wait(shared: &Shared, seconds: f64) -> Result<Going, Never> {
    let going = match shared.going.lock() {
        Ok(going) => going,
        Err(_the_film_gave_up) => return Ok(Going::Off),
    };

    match *going {
        Going::Off => return Ok(Going::Off),
        Going::On => {},
    }

    let long = Duration::from_secs_f64(seconds.clamp(0.0, LONGEST_WAIT));

    Ok(match shared.woken.wait_timeout(going, long) {
        Ok((going, _)) => *going,
        Err(_the_film_gave_up) => Going::Off,
    })
}

fn drawn(picture: &Picture, room: Size<u32>, cores: Cores) -> Result<Pixels, Never> {
    let Ok(shown) = picture.size();
    let Ok(made) = console_pictures::fit_within(shown, room);
    let Ok(drawn) = picture.rgba_at(made, cores);
    let stride = made.width.saturating_mul(4);

    Ok(Pixels { width: made.width, height: made.height, stride, bytes: Arc::new(drawn.rgba) })
}

fn shown(showing: &Showing, clock: &Clock, shared: &Shared) -> Result<(), Never> {
    let opened = console_films::open(&showing.film);

    let film = match opened {
        Ok(Some(film)) => film,
        Ok(None) => return ended(shared),
        Err(fault) => {
            eprintln!("viewer: {fault}");

            return ended(shared);
        },
    };

    let Ok(cues) = cues(&showing.film, &film, showing.captions);
    let pictures = film.pictures(showing.from);

    let pictures = match pictures {
        Ok(Some(pictures)) => pictures,
        Ok(None) => return ended(shared),
        Err(fault) => {
            eprintln!("viewer: {fault}");

            return ended(shared);
        },
    };

    let played = console_core_iteration::iterate((pictures, u64::MAX), |(mut pictures, said_second)| {
        let next = pictures.next_picture();

        let picture = match next {
            Ok(Some(picture)) => picture,
            Ok(None) => return Ok(Step::Halt(Stop::FilmOver)),
            Err(fault) => {
                eprintln!("viewer: {fault}");

                return Ok(Step::Halt(Stop::FilmOver));
            },
        };

        let Ok(put) = put(showing, (clock, shared), &picture, &cues);

        Ok(match put {
            Put::Off => Step::Halt(Stop::TurnedOff),
            Put::Late => Step::Again((pictures, said_second)),
            Put::Shown(due) => {
                let Ok(second) = toward_zero_u64(due);

                match second == said_second {
                    true => {},
                    false => {
                        let Ok(()) = console_panel::frames::tell(console_panel::frames::Notice::Rows);
                    },
                }

                Step::Again((pictures, second))
            },
        })
    });

    match played {
        Ok(Stop::FilmOver) => ended(shared),
        Ok(Stop::TurnedOff) => Ok(()),
        Err(_endless) => ended(shared),
    }
}

enum Put {
    Shown(f64),
    Late,
    Off,
}

fn put(showing: &Showing, keeping: (&Clock, &Shared), picture: &Picture, cues: &[Cue]) -> Result<Put, Never> {
    let (clock, shared) = keeping;
    let a_frame = 1.0 / showing.rate;
    let Ok(due) = picture.at();
    let Ok(still) = until_due(clock, shared, Due { at: due, early: a_frame / 2.0, tempo: showing.tempo });

    match still {
        Going::Off => return Ok(Put::Off),
        Going::On => {},
    }

    let Ok(still) = state(shared);

    match still {
        Going::Off => return Ok(Put::Off),
        Going::On => {},
    }

    let Ok(now) = clock.now();

    match now - due > a_frame * 2.0 {
        true => return Ok(Put::Late),
        false => {},
    }

    let Ok(pixels) = drawn(picture, showing.size, showing.cores);
    let Ok(said) = console_core_subtitle_files::showing(cues, due);
    let Ok(()) = console_panel::frames::insert(&showing.film, pixels, said);

    Ok(Put::Shown(due))
}

enum Stop {
    FilmOver,
    TurnedOff,
}

#[derive(Clone, Copy)]
struct Due {
    at: f64,
    early: f64,
    tempo: f64,
}

fn until_due(clock: &Clock, shared: &Shared, due: Due) -> Result<Going, Never> {
    let Due { at: due, early, tempo } = due;
    let waited = console_core_iteration::iterate((), |()| {
        let Ok(now) = clock.now();

        match now + early >= due {
            true => return Ok(Step::Halt(Going::On)),
            false => {},
        }

        let Ok(still) = wait(shared, (due - now) / tempo);

        Ok(match still {
            Going::Off => Step::Halt(Going::Off),
            Going::On => Step::Again(()),
        })
    });

    Ok(match waited {
        Ok(going) => going,
        Err(_endless) => Going::On,
    })
}

fn ended(shared: &Shared) -> Result<(), Never> {
    match shared.ended.lock() {
        Ok(mut ended) => *ended = Ended::Yes,
        Err(_the_film_gave_up) => {},
    }

    console_panel::frames::tell(console_panel::frames::Notice::Rows)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    fn request(film: &str) -> Result<PlaybackRequest, Never> {
        Ok(PlaybackRequest { film: PathBuf::from(film), from: 0.0, speed: 1, captions: Captions::Off, sought: None })
    }

    #[test]
    fn a_film_starts_when_it_is_wanted_and_stops_when_it_is_not() {
        let Ok(film) = request("/x/holiday.mp4");

        assert_eq!(next(None, None), Ok(Next::None));
        assert_eq!(next(None, Some(&film)), Ok(Next::Start));
        assert_eq!(next(Some(&film), None), Ok(Next::Stop));
        assert_eq!(next(Some(&film), Some(&film)), Ok(Next::None));
    }

    #[test]
    fn the_film_moving_on_by_itself_is_not_a_reason_to_start_it_again() {
        let Ok(playing) = request("/x/holiday.mp4");
        let later = PlaybackRequest { from: 42.0, ..playing.clone() };

        assert_eq!(next(Some(&playing), Some(&later)), Ok(Next::None));
    }

    #[test]
    fn a_seek_a_speed_subtitles_or_another_film_start_it_again() {
        let Ok(playing) = request("/x/holiday.mp4");

        for wanted in [
            PlaybackRequest { sought: Some(90), ..playing.clone() },
            PlaybackRequest { speed: 3, ..playing.clone() },
            PlaybackRequest { captions: Captions::Track(0), ..playing.clone() },
            PlaybackRequest { film: PathBuf::from("/x/other.mkv"), ..playing.clone() },
        ] {
            assert_eq!(next(Some(&playing), Some(&wanted)), Ok(Next::Start), "{wanted:?}");
        }
    }

    fn fixture(named: &str) -> Result<PathBuf, Never> {
        Ok(Path::new(env!("CARGO_MANIFEST_DIR")).join("../console-films/tests/films").join(named))
    }

    #[test]
    fn what_a_film_says_about_itself_is_what_playing_it_needs() -> Result<(), Box<dyn Error>> {
        let Ok(at) = fixture("red-then-blue.vp9.webm");
        let Ok(heard) = facts(&at);
        let heard = heard.ok_or("a film with pictures in it is a film")?;

        assert!((heard.rate - 25.0).abs() < 0.5, "{heard:?}");
        assert!((heard.seconds - 2.0).abs() < 0.1, "{heard:?}");
        assert_eq!((heard.sound, heard.words), (Sound::Audible, 0));

        let Ok(at) = fixture("with-subtitles.av1.mkv");
        let Ok(worded) = facts(&at);
        let worded = worded.ok_or("a film with words in it is a film")?;

        assert_eq!((worded.sound, worded.words), (Sound::Silent, 1));

        let not_a_film = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");

        assert_eq!(facts(&not_a_film), Ok(None));

        Ok(())
    }

    #[test]
    fn subtitles_come_from_the_film_first_and_then_from_a_file_beside_it() -> Result<(), Box<dyn Error>> {
        let folder = console_core_temporary_directories::fresh("viewer-words")?;
        let at = folder.join("holiday.mkv");
        let Ok(inside) = fixture("with-subtitles.av1.mkv");

        std::fs::copy(&inside, &at)?;
        console_core_atomic_writes::whole(&folder.join("holiday.srt"), b"1\n00:00:00,100 --> 00:00:00,900\nBeside it\n")?;

        let opened = console_films::open(&at)?;
        let film = opened.ok_or("the copy is a film")?;
        let Ok(off) = cues(&at, &film, Captions::Off);
        let Ok(inside) = cues(&at, &film, Captions::Track(0));
        let Ok(beside) = cues(&at, &film, Captions::Track(1));
        let Ok(past_the_end) = cues(&at, &film, Captions::Track(2));
        let Ok(said_inside) = console_core_subtitle_files::showing(&inside, 0.6);
        let Ok(said_beside) = console_core_subtitle_files::showing(&beside, 0.6);

        assert!(off.is_empty());
        assert_eq!(said_inside, vec![String::from("Red, at first")]);
        assert_eq!(said_beside, vec![String::from("Beside it")]);
        assert!(past_the_end.is_empty());

        Ok(())
    }

    #[test]
    fn a_film_plays_through_its_frames_onto_the_card_and_says_when_it_is_over() -> Result<(), Box<dyn Error>> {
        let Ok(at) = fixture("with-subtitles.av1.mkv");
        let Ok(facts) = facts(&at);
        let facts = facts.ok_or("the film it was handed is no film")?;

        assert_eq!(facts.sound, Sound::Silent, "this test must not make a noise on the machine running it");

        let room = Size { width: 32, height: 32 };
        let _looking = console_panel::frames::current(&at, room);
        let Ok(sound) = Sounding::new(|_progress| Ok(()));
        let asked = PlaybackRequest { film: at.clone(), from: 0.0, speed: 1, captions: Captions::Track(0), sought: None };
        let Ok(film) = Film::start(asked, &facts, room, Arc::new(sound));
        let Ok(patience) = console_waiting::Schedule::of(Duration::from_secs(20));
        let mut seen: (Vec<Arc<Vec<u8>>>, Vec<Vec<String>>) = (Vec::new(), Vec::new());

        let Ok(over) = console_waiting::until_handed(patience, &mut seen, |(seen, heard)| {
            match console_panel::frames::current(&at, room) {
                Ok(Some(frame)) => match seen.contains(&frame.bytes) {
                    true => {},
                    false => {
                        assert_eq!((frame.width, frame.height), (32, 21), "a frame is the film's shape inside the room");

                        seen.push(Arc::clone(&frame.bytes));
                    },
                },
                Ok(None) | Err(_) => {},
            }

            let Ok(said) = console_panel::frames::said(&at);

            match heard.contains(&said) {
                true => {},
                false => heard.push(said),
            }

            let Ok(ended) = film.ended();

            Ok(match ended {
                Ended::Yes => console_waiting::Ready::Yes,
                Ended::No => console_waiting::Ready::NotYet,
            })
        });
        let Ok(heard_at) = film.at();

        drop(film);

        let (seen, heard) = seen;

        assert_eq!(over, console_waiting::Outcome::Happened, "a two second film never ended");
        assert!(seen.len() >= 2, "the picture on the card never moved on: {} different frames reached it", seen.len());
        assert!(heard.contains(&vec![String::from("Red, at first")]) && heard.contains(&vec![String::from("then blue")]), "the subtitles were not said over the film: {heard:?}");
        assert!(heard_at > 0.5, "the film's clock stood still at {heard_at}");

        Ok(())
    }
}
