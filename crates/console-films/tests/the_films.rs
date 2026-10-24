use std::error::Error;
use std::path::{Path, PathBuf};

use console_films::{Bitmap, Facts, Film, Sound, open};
use console_core_never::Never;
use console_core_number_conversion::toward_zero_i64;
use console_core_subtitle_files::Written;
use console_concurrency::Cores;
use console_core_geometry::Size;

const EVERY_CODEC: [(&str, Sound); 4] = [
    ("red-then-blue.h264.mp4", Sound::Silent),
    ("red-then-blue.hevc.mp4", Sound::Silent),
    ("red-then-blue.vp9.webm", Sound::Audible),
    ("red-then-blue.av1.mkv", Sound::Silent),
];

const STORED: Size<u32> = Size { width: 96, height: 64 };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Colour {
    Red,
    Blue,
    Neither,
}

fn film(named: &str) -> Result<Film, Box<dyn Error>> {
    let at: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/films").join(named);
    let opened = open(&at)?;
    let film = opened.ok_or(format!("{named} was not read as a film"))?;

    Ok(film)
}

fn middle_of(bitmap: &Bitmap) -> Result<Colour, Box<dyn Error>> {
    let across = u64::from(bitmap.size.width);
    let row = u64::from(bitmap.size.height.saturating_div(2));
    let pixel = row.saturating_mul(across).saturating_add(across.saturating_div(2));
    let Ok(pixel) = console_core_number_conversion::index(pixel);
    let said = bitmap.rgba.as_chunks::<4>().0.get(pixel).ok_or("the middle is past the end of the picture")?;

    Ok(match said {
        [red, green, blue, u8::MAX] => match (*red > 200, *green < 60, *blue > 200) {
            (true, true, false) => Colour::Red,
            (false, true, true) => Colour::Blue,
            (_, _, _) => Colour::Neither,
        },
        _ => Colour::Neither,
    })
}

#[test]
fn every_codec_says_how_long_how_big_and_whether_it_sounds() -> Result<(), Box<dyn Error>> {
    for (named, sound) in EVERY_CODEC {
        let film = film(named)?;
        let Ok(facts) = film.facts();
        let Facts { seconds, rate, size, sound: heard, subtitles } = facts.ok_or(format!("{named} has no pictures"))?;

        assert!((seconds - 2.0).abs() < 0.1, "{named} lasts {seconds}");
        assert!((rate - 25.0).abs() < 0.5, "{named} runs at {rate}");
        assert_eq!(size, STORED, "{named}");
        assert_eq!(heard, sound, "{named}");
        assert!(subtitles.is_empty(), "{named}");
    }

    Ok(())
}

#[test]
fn every_codec_starts_red_and_ends_blue_and_shows_every_picture() -> Result<(), Box<dyn Error>> {
    for (named, _) in EVERY_CODEC {
        let film = film(named)?;
        let pictures = film.pictures(0.0)?;
        let mut pictures = pictures.ok_or(format!("{named} has no pictures"))?;
        let mut seen = Vec::new();

        'over_pictures: for _each in 0..60 {
            let next = pictures.next_picture()?;

            match next {
                Some(picture) => seen.push(picture),
                None => break 'over_pictures,
            }
        }

        assert_eq!(seen.len(), 50, "{named}");

        let first = seen.first().ok_or("no first")?;
        let last = seen.last().ok_or("no last")?;
        let (Ok(first), Ok(last)) = (first.rgba(), last.rgba());
        let starting = middle_of(&first)?;
        let ending = middle_of(&last)?;

        assert_eq!(first.size, STORED, "{named}");
        assert_eq!(starting, Colour::Red, "{named}");
        assert_eq!(ending, Colour::Blue, "{named}");

        let moments: Vec<f64> = seen.iter().flat_map(|picture| picture.at()).collect();

        assert!(moments.is_sorted(), "{named} came back out of order: {moments:?}");
    }

    Ok(())
}

#[test]
fn a_film_started_from_the_middle_starts_at_that_moment() -> Result<(), Box<dyn Error>> {
    for (named, _) in EVERY_CODEC {
        let film = film(named)?;
        let pictures = film.pictures(1.5)?;
        let mut pictures = pictures.ok_or(format!("{named} has no pictures"))?;
        let next = pictures.next_picture()?;
        let picture = next.ok_or(format!("{named} has nothing after a second and a half"))?;
        let Ok(at) = picture.at();
        let Ok(drawn) = picture.rgba();
        let seen = middle_of(&drawn)?;

        assert!((at - 1.5).abs() < 0.03, "{named} started at {at}");
        assert_eq!(seen, Colour::Blue, "{named}");
    }

    Ok(())
}

#[test]
fn a_film_shot_on_its_side_is_measured_and_drawn_upright() -> Result<(), Box<dyn Error>> {
    let film = film("turned.h264.mp4")?;
    let Ok(facts) = film.facts();
    let upright = Size { width: STORED.height, height: STORED.width };

    assert_eq!(facts.map(|facts| facts.size), Some(upright));

    let first = film.first_picture()?;
    let picture = first.ok_or("no first picture")?;
    let Ok(drawn) = picture.rgba();
    let seen = middle_of(&drawn)?;

    assert_eq!(drawn.size, upright);
    assert_eq!(seen, Colour::Red);

    Ok(())
}

#[test]
fn a_file_that_is_not_a_film_is_no_film() -> Result<(), Box<dyn Error>> {
    let at = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");

    let opened = open(&at)?;

    assert!(opened.is_none());

    Ok(())
}

#[test]
fn a_subtitle_track_in_a_film_is_named_and_read_with_its_moments() -> Result<(), Box<dyn Error>> {
    let film = film("with-subtitles.av1.mkv")?;
    let Ok(facts) = film.facts();
    let facts = facts.ok_or("no pictures")?;
    let subtitles = facts.subtitles;
    let only = subtitles.first().ok_or("no subtitle track")?;

    assert_eq!(subtitles.len(), 1);
    assert_eq!((only.language.0.as_str(), only.name.as_str(), only.written), ("nld", "Nederlands", Written::SubRip));

    let cues = film.cues(only)?;
    let said: Vec<(i64, i64, Vec<String>)> = cues.iter().map(|cue| {
        let (Ok(from), Ok(until)) = (whole_milliseconds(cue.from), whole_milliseconds(cue.until));

        (from, until, cue.lines.clone())
    }).collect();

    assert_eq!(said, vec![(500, 1200, vec![String::from("Red, at first")]), (1300, 1900, vec![String::from("then blue")])]);

    Ok(())
}

fn whole_milliseconds(seconds: f64) -> Result<i64, Never> {
    toward_zero_i64((seconds * 1000.0).round())
}

#[test]
fn a_picture_drawn_smaller_is_its_own_colours_upright_at_that_size() -> Result<(), Box<dyn Error>> {
    let Ok(cores) = Cores::counted();

    for (named, _) in EVERY_CODEC {
        let film = film(named)?;
        let first = film.first_picture()?;
        let picture = first.ok_or(format!("{named} has no first picture"))?;
        let Ok(drawn) = picture.rgba_at(Size { width: 48, height: 32 }, cores);
        let seen = middle_of(&drawn)?;

        assert_eq!((drawn.size, drawn.rgba.len()), (Size { width: 48, height: 32 }, 6144), "{named}");
        assert_eq!(seen, Colour::Red, "{named}");
    }

    let film = film("turned.h264.mp4")?;
    let first = film.first_picture()?;
    let picture = first.ok_or("no first picture")?;
    let Ok(drawn) = picture.rgba_at(Size { width: 32, height: 48 }, cores);
    let seen = middle_of(&drawn)?;

    assert_eq!((drawn.size, drawn.rgba.len()), (Size { width: 32, height: 48 }, 6144));
    assert_eq!(seen, Colour::Red);

    Ok(())
}
