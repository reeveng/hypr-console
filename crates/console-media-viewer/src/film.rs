//! A film, played: its sound through the speaker and its frames onto the card.
//!
//! Two ffmpegs, one for each half, started at the same place in the film and
//! kept together by one clock. The sound is `console_music_player`'s
//! `Sounding`, which is already the one place on this device that turns a file
//! into something heard, and it already knows how far through the file the
//! speaker has got. So the sound is the clock: each frame is read off a pipe
//! of raw pictures at the film's own rate, knows which moment of the film it
//! is, and is handed to the card when the sound reaches that moment. A frame
//! that comes out after its moment has passed is dropped rather than shown
//! late, so a slow machine shows fewer frames of a film that stays in step with
//! its sound instead of every frame of one that falls behind it. A film with no
//! sound has no speaker to follow, and follows the wall instead.
//!
//! **Nothing is paused.** A pause ends both programs and keeps where the film
//! had got to, which is `Sounding`'s own argument for the music player and it
//! holds here for the same reasons: a stream held open keeps the machine awake,
//! and nothing can tell a running ffmpeg to go somewhere else anyway. A seek, a
//! speed and a choice of subtitles are the same: the film is started again from
//! where it is, with the new thing asked for. [`next`] is that decision, and it
//! is arithmetic, so it is what the tests press.
//!
//! **The card is told, not asked.** Each frame goes to `console_panel::frames`,
//! which repaints the picture's own rectangle and nothing else, and once a
//! second of film the rows are asked for again so that the clock and the bar
//! under the picture move on. The words, the bar and the buttons are painted
//! once a second and the film thirty times.
//!
//! **Subtitles are drawn into the frames.** libass is inside ffmpeg already,
//! and a line of text drawn by the program that knows when it is due is a line
//! that cannot fall out of step with the picture. A film seeked to the middle
//! starts its timestamps at nothing, so the frames are moved to where they are
//! in the film before the words are drawn over them and moved back after.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use console_core_external_programs::Program;
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{Float, toward_zero_u64};
use console_core_shapes::Pixels;
use console_music_player::sounding::{Sounding, Tempo, Wanted};
use console_program_lifetime::{BoundToParent, alongside};

use crate::playing::{self, Captions};

#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
    pub rate: f64,
    pub seconds: f64,
    pub sound: Sound,
    pub words: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    Audible,
    Silent,
}

const ORDINARY_RATE: f64 = 30.0;

const FASTEST_RATE: f64 = 60.0;

pub fn facts(film: &Path) -> Result<Option<Facts>, Never> {
    let Ok(mut asking) = Program::Ffprobe.command();

    let said = match asking
        .args(["-v", "error", "-show_entries", "stream=codec_type,r_frame_rate:format=duration"])
        .args(["-of", "default=nw=1"])
        .arg(film)
        .stdin(Stdio::null())
        .output()
    {
        Ok(said) => said,
        Err(fault) => {
            eprintln!("viewer: ffprobe: {fault}");

            return Ok(None);
        },
    };

    match said.status.success() {
        true => {},
        false => return Ok(None),
    }

    read(&String::from_utf8_lossy(&said.stdout))
}

pub fn read(said: &str) -> Result<Option<Facts>, Never> {
    let mut facts = Facts { rate: 0.0, seconds: 0.0, sound: Sound::Silent, words: 0 };
    let mut kind = "";
    let mut pictured = Pictured::No;

    for line in said.lines() {
        let (key, value) = match line.split_once('=') {
            Some(said) => said,
            None => continue,
        };

        match key {
            "codec_type" => {
                kind = value;

                match value {
                    "audio" => facts.sound = Sound::Audible,
                    "subtitle" => facts.words = facts.words.saturating_add(1),
                    "video" => pictured = Pictured::Yes,
                    _another_kind => {},
                }
            },
            "r_frame_rate" => match (kind, facts.rate > 0.0) {
                ("video", false) => {
                    let Ok(rate) = rate(value);

                    facts.rate = rate;
                },
                (_, _) => {},
            },
            "duration" => match value.parse::<f64>() {
                Ok(seconds) => facts.seconds = seconds.max(0.0),
                Err(_not_said) => {},
            },
            _another_key => {},
        }
    }

    Ok(match pictured {
        Pictured::Yes => Some(Facts {
            rate: match facts.rate > 0.0 {
                true => facts.rate.min(FASTEST_RATE),
                false => ORDINARY_RATE,
            },
            ..facts
        }),
        Pictured::No => None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pictured {
    Yes,
    No,
}

fn rate(said: &str) -> Result<f64, Never> {
    let (over, under) = match said.split_once('/') {
        Some(split) => split,
        None => (said, "1"),
    };

    Ok(match (over.parse::<f64>(), under.parse::<f64>()) {
        (Ok(over), Ok(under)) => match under > 0.0 {
            true => over / under,
            false => 0.0,
        },
        (Err(_), _) | (_, Err(_)) => 0.0,
    })
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

pub fn words_filter(film: &Path, captions: Captions, words: u32, from: f64) -> Result<Option<String>, Never> {
    let Ok(track) = captions.track();

    let track = match track {
        Some(track) => track,
        None => return Ok(None),
    };

    let Ok(beside) = sidecars(film);

    let Ok(beyond) = console_core_number_conversion::index(track.saturating_sub(words));

    let (file, stream) = match track < words {
        true => (film.to_path_buf(), Some(track)),
        false => match beside.get(beyond) {
            Some(file) => (file.clone(), None),
            None => return Ok(None),
        },
    };

    let Ok(named) = escaped(&file.to_string_lossy());
    let chosen = match stream {
        Some(stream) => format!(":si={stream}"),
        None => String::new(),
    };

    Ok(Some(format!("setpts=PTS+{from}/TB,subtitles=filename={named}{chosen},setpts=PTS-STARTPTS")))
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

pub fn escaped(path: &str) -> Result<String, Never> {
    let mut once = String::new();

    for letter in path.chars() {
        match letter {
            '\\' | '\'' | ':' => {
                once.push('\\');
                once.push(letter);
            },
            other => once.push(other),
        }
    }

    let mut twice = String::new();

    for letter in once.chars() {
        match letter {
            '\\' | '\'' | '[' | ']' | ',' | ';' => {
                twice.push('\\');
                twice.push(letter);
            },
            other => twice.push(other),
        }
    }

    Ok(twice)
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

        let Ok(filter) = words_filter(&asked.film, asked.captions, facts.words, asked.from);
        let showing = Showing {
            film: asked.film.clone(),
            from: asked.from,
            tempo,
            rate: facts.rate,
            size: room,
            words: filter,
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
    words: Option<String>,
}

fn decoding(showing: &Showing, size: Size<u32>) -> Result<Option<BoundToParent>, Never> {
    let Size { width: wide, height: tall } = size;
    let scaled = format!("scale={wide}:{tall}");
    let filter = match &showing.words {
        Some(words) => format!("{words},{scaled}"),
        None => scaled,
    };

    let Ok(mut asking) = Program::Ffmpeg.command();

    asking
        .args(["-v", "error", "-nostdin", "-ss", &format!("{}", showing.from), "-i"])
        .arg(&showing.film)
        .args(["-an", "-sn", "-vf", &filter, "-r", &format!("{}", showing.rate)])
        .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .stdout(Stdio::piped())
        .stdin(Stdio::null())
        .stderr(Stdio::null());

    Ok(match alongside(&mut asking) {
        Ok(decoding) => Some(decoding),
        Err(fault) => {
            eprintln!("viewer: {}: will not decode: {fault}", showing.film.display());

            None
        },
    })
}

fn going(shared: &Shared) -> Result<Going, Never> {
    Ok(match shared.going.lock() {
        Ok(going) => *going,
        Err(_the_film_gave_up) => Going::Off,
    })
}

const LONGEST_WAIT: f64 = 0.25;

fn waited(shared: &Shared, seconds: f64) -> Result<Going, Never> {
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

pub fn fitted_size(had: Size<u32>, room: Size<u32>) -> Result<Size<u32>, Never> {
    let Ok(fitted) = console_pictures::fitted(had, room);
    let even = |side: u32| side.saturating_sub(side.wrapping_rem(2)).max(2);

    Ok(Size { width: even(fitted.width), height: even(fitted.height) })
}

fn shown(showing: &Showing, clock: &Clock, shared: &Shared) -> Result<(), Never> {
    let had = match console_pictures::measured(&showing.film) {
        Ok(Some(had)) => had,
        Ok(None) | Err(_) => Size { width: 16, height: 9 },
    };
    let Ok(size) = fitted_size(had, showing.size);
    let Ok(decoder) = decoding(showing, size);

    let mut decoder = match decoder {
        Some(decoder) => decoder,
        None => return ended(shared),
    };

    let Ok(out) = decoder.reading();

    let mut out = match out {
        Some(out) => out,
        None => return ended(shared),
    };

    let long = u64::from(size.width).saturating_mul(u64::from(size.height)).saturating_mul(4);
    let Ok(long) = console_core_number_conversion::index(long);
    let stride = size.width.saturating_mul(4);
    let a_frame = 1.0 / showing.rate;
    let mut read: u64 = 0;
    let mut said_second = u64::MAX;

    'frames: loop {
        let mut frame = vec![0u8; long];

        match out.read_exact(&mut frame) {
            Ok(()) => {},
            Err(_the_film_is_over) => return ended(shared),
        }

        let Ok(counted) = read.float();
        let due = showing.from + counted * a_frame;

        read = read.saturating_add(1);

        'due: loop {
            let Ok(now) = clock.now();

            match now + a_frame / 2.0 >= due {
                true => break 'due,
                false => {},
            }

            let Ok(still) = waited(shared, (due - now) / showing.tempo);

            match still {
                Going::Off => return Ok(()),
                Going::On => {},
            }
        }

        let Ok(still) = going(shared);

        match still {
            Going::Off => return Ok(()),
            Going::On => {},
        }

        let Ok(now) = clock.now();

        match now - due > a_frame * 2.0 {
            true => continue 'frames,
            false => {},
        }

        let pixels = Pixels { width: size.width, height: size.height, stride, bytes: Arc::new(frame) };
        let Ok(()) = console_panel::frames::put(&showing.film, pixels);
        let Ok(second) = toward_zero_u64(due);

        match second == said_second {
            true => {},
            false => {
                said_second = second;

                let Ok(()) = console_panel::frames::tell(console_panel::frames::Notice::Rows);
            },
        }
    }
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
    use super::*;

    fn asked(film: &str) -> PlaybackRequest {
        PlaybackRequest { film: PathBuf::from(film), from: 0.0, speed: 1, captions: Captions::Off, sought: None }
    }

    #[test]
    fn a_film_starts_when_it_is_wanted_and_stops_when_it_is_not() {
        let film = asked("/x/holiday.mp4");

        assert_eq!(next(None, None), Ok(Next::None));
        assert_eq!(next(None, Some(&film)), Ok(Next::Start));
        assert_eq!(next(Some(&film), None), Ok(Next::Stop));
        assert_eq!(next(Some(&film), Some(&film)), Ok(Next::None));
    }

    #[test]
    fn the_film_moving_on_by_itself_is_not_a_reason_to_start_it_again() {
        let playing = asked("/x/holiday.mp4");
        let later = PlaybackRequest { from: 42.0, ..playing.clone() };

        assert_eq!(next(Some(&playing), Some(&later)), Ok(Next::None));
    }

    #[test]
    fn a_seek_a_speed_subtitles_or_another_film_start_it_again() {
        let playing = asked("/x/holiday.mp4");

        for wanted in [
            PlaybackRequest { sought: Some(90), ..playing.clone() },
            PlaybackRequest { speed: 3, ..playing.clone() },
            PlaybackRequest { captions: Captions::Track(0), ..playing.clone() },
            PlaybackRequest { film: PathBuf::from("/x/other.mkv"), ..playing.clone() },
        ] {
            assert_eq!(next(Some(&playing), Some(&wanted)), Ok(Next::Start), "{wanted:?}");
        }
    }

    #[test]
    fn what_ffprobe_says_about_a_film_is_read_into_what_playing_it_needs() {
        let said = "codec_type=video\nr_frame_rate=30000/1001\ncodec_type=audio\nr_frame_rate=0/0\n\
                    codec_type=subtitle\nr_frame_rate=0/0\ncodec_type=subtitle\nr_frame_rate=0/0\nduration=12.5\n";
        let Ok(Some(facts)) = read(said) else { panic!("a film with a picture in it is a film") };

        assert!((facts.rate - 29.97).abs() < 0.01, "{facts:?}");
        assert_eq!(facts.sound, Sound::Audible);
        assert_eq!(facts.words, 2);
        assert!((facts.seconds - 12.5).abs() < 0.001);
    }

    #[test]
    fn a_film_with_no_sound_or_no_rate_still_plays_and_a_song_is_not_a_film() {
        let Ok(Some(silent)) = read("codec_type=video\nr_frame_rate=0/0\nduration=3\n") else {
            panic!("a silent film is a film")
        };

        assert_eq!(silent.sound, Sound::Silent);
        assert!((silent.rate - ORDINARY_RATE).abs() < 0.001);
        assert_eq!(read("codec_type=audio\nr_frame_rate=0/0\nduration=3\n"), Ok(None));

        let Ok(Some(fast)) = read("codec_type=video\nr_frame_rate=240/1\n") else { panic!("fast is a film") };

        assert!((fast.rate - FASTEST_RATE).abs() < 0.001, "{fast:?}");
    }

    #[test]
    fn a_name_with_the_filter_syntax_in_it_is_said_so_the_filter_reads_it_whole() {
        assert_eq!(escaped("/x/plain.mkv"), Ok("/x/plain.mkv".to_string()));
        assert_eq!(escaped("it's a: test, [x].mkv"), Ok("it\\\\\\'s a\\\\: test\\, \\[x\\].mkv".to_string()));
    }

    #[test]
    fn subtitles_off_draw_nothing_into_the_frames() {
        assert_eq!(words_filter(Path::new("/x/holiday.mp4"), Captions::Off, 2, 0.0), Ok(None));
    }

    #[test]
    fn a_subtitle_track_inside_the_film_is_asked_for_by_its_number_and_starts_where_the_film_does() {
        let Ok(Some(filter)) = words_filter(Path::new("/x/holiday.mp4"), Captions::Track(1), 2, 30.0) else {
            panic!("a track the film has is drawn")
        };

        assert_eq!(filter, "setpts=PTS+30/TB,subtitles=filename=/x/holiday.mp4:si=1,setpts=PTS-STARTPTS");
        assert_eq!(words_filter(Path::new("/x/holiday.mp4"), Captions::Track(5), 2, 0.0), Ok(None));
    }

    #[test]
    fn a_frame_is_an_even_size_inside_the_room() {
        assert_eq!(fitted_size(Size { width: 1920, height: 1080 }, Size { width: 801, height: 801 }), Ok(Size { width: 800, height: 450 }));
        assert_eq!(fitted_size(Size { width: 1, height: 1 }, Size { width: 1, height: 1 }), Ok(Size { width: 2, height: 2 }));
    }

    #[test]
    fn a_film_plays_through_its_frames_onto_the_card_and_says_when_it_is_over() {
        let at = std::env::temp_dir().join(format!("console-viewer-film-{}.mkv", std::process::id()));
        let Ok(mut making) = Program::Ffmpeg.command();
        let made = making
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", "testsrc=s=64x48:r=5:d=2"])
            .arg(&at)
            .status();

        assert!(made.is_ok_and(|how| how.success()), "ffmpeg made no film to play");

        let facts = match facts(&at) {
            Ok(Some(facts)) => facts,
            Ok(None) | Err(_) => panic!("ffprobe says the film it was handed is no film"),
        };

        assert_eq!(facts.sound, Sound::Silent, "this test must not make a noise on the machine running it");

        let room = Size { width: 32, height: 32 };
        let _looking = console_panel::frames::current(&at, room);
        let Ok(sound) = Sounding::new(|_progress| Ok(()));
        let asked = PlaybackRequest { film: at.clone(), from: 0.0, speed: 1, captions: Captions::Off, sought: None };
        let Ok(film) = Film::start(asked, &facts, room, Arc::new(sound));
        let patience = console_waiting::Schedule::of(Duration::from_secs(20)).expect("a patience");
        let mut seen: Vec<Arc<Vec<u8>>> = Vec::new();

        let Ok(over) = console_waiting::until(patience, || {
            match console_panel::frames::current(&at, room) {
                Ok(Some(frame)) => match seen.contains(&frame.bytes) {
                    true => {},
                    false => {
                        assert_eq!((frame.width, frame.height), (32, 24), "a frame is the film's shape inside the room");

                        seen.push(Arc::clone(&frame.bytes));
                    },
                },
                Ok(None) | Err(_) => {},
            }

            let Ok(ended) = film.ended();

            Ok(match ended {
                Ended::Yes => console_waiting::Ready::Yes,
                Ended::No => console_waiting::Ready::NotYet,
            })
        });
        let Ok(heard_at) = film.at();

        drop(film);
        let _ = std::fs::remove_file(&at);

        assert_eq!(over, console_waiting::Outcome::Happened, "a two second film never ended");
        assert!(seen.len() > 5, "only {} different frames reached the card", seen.len());
        assert!(heard_at > 0.5, "the film's clock stood still at {heard_at}");
    }
}
