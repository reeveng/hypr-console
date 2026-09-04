//! The wallpaper is the one surface that cannot be read for its colours.
//!
//! A stylesheet can be searched for a hex that should not be there; a picture
//! is pixels and gives nothing up. So it is held to its palette from both
//! ends: a stamp saying what it was drawn from, and the file itself read back
//! for the things the palette decided.

use std::path::{Path, PathBuf};

use console_garden::garden::{Garden, Spec};
use console_garden::scene::sheet;
use console_garden::{SEED, air, palette, probe, scene, stamp};
use console_screen::{CONFIG, Screen};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository")
}

fn read(at: &str) -> String {
    std::fs::read_to_string(root().join(at)).unwrap_or_else(|fault| panic!("{at}: {fault}"))
}

fn spec() -> Spec {
    toml::from_str(&read("theme/palette.toml")).expect("theme/palette.toml parses")
}

fn declared() -> toml::Table {
    read("theme/palette.toml")
        .parse()
        .expect("theme/palette.toml parses")
}

fn stamped() -> toml::Table {
    read("theme/garden.stamp")
        .parse()
        .expect("theme/garden.stamp parses")
}

fn screen() -> Screen {
    Screen::read(&read(CONFIG)).expect("the compositor declares a screen")
}

fn canvas() -> PathBuf {
    root().join("files/usr/share/backgrounds/console.webp")
}

/// One frame of the animation: where it sits, and how long it lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shown {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    milliseconds: u32,
}

/// Every frame of an animated WebP.
///
/// The container is read here rather than asked of a library, because what is
/// being checked is the thing this repository wrote into the file.
fn frames(path: &Path) -> Vec<Shown> {
    let data = std::fs::read(path).expect("the wallpaper has been drawn");
    assert_eq!(&data[..4], b"RIFF");
    assert_eq!(&data[8..12], b"WEBP");
    let three = |at: usize| u32::from_le_bytes([data[at], data[at + 1], data[at + 2], 0]);

    let mut out = Vec::new();
    let mut at = 12;
    while at + 8 <= data.len() {
        let tag = &data[at..at + 4];
        let size =
            u32::from_le_bytes(data[at + 4..at + 8].try_into().expect("four bytes")) as usize;
        if tag == b"ANMF" {
            let body = at + 8;
            out.push(Shown {
                x: three(body) * 2,
                y: three(body + 3) * 2,
                width: three(body + 6) + 1,
                height: three(body + 9) + 1,
                milliseconds: three(body + 12),
            });
        }
        at += 8 + size + (size & 1);
    }
    out
}

/// Change a colour and the picture is a lie until it is drawn again.
///
/// Every other surface is checked by reading the colour back out of it. This
/// one is checked by the stamp the drawing left behind, which is the same
/// promise made the only way a picture can make it.
#[test]
fn the_garden_is_drawn_from_the_palette_as_it_stands() {
    let held = stamped();
    let size = screen().pixels();
    let colours = palette::read(&read("theme/report.md")).expect("a solved palette");
    assert_eq!(
        held["palette"].as_str(),
        Some(stamp::wanted(&colours, &spec().garden, size).as_str()),
        "the wallpaper is older than the palette; run `just garden`"
    );
    assert_eq!(held["width"].as_integer(), Some(i64::from(size.0)));
    assert_eq!(held["height"].as_integer(), Some(i64::from(size.1)));
    let resting = held["resting"].as_str().expect("a resting colour");
    assert_eq!(resting.len(), 7);
    assert!(
        resting
            .strip_prefix('#')
            .is_some_and(|code| code.chars().all(|d| d.is_ascii_hexdigit()))
    );
}

#[test]
fn every_colour_the_garden_paints_with_is_in_the_palette() {
    let said = declared();
    let colours = said["colour"].as_table().expect("a table of colours");
    for (part, dipped) in spec().garden.paint {
        assert!(
            colours.contains_key(&dipped.colour),
            "the garden paints {part} with {}, which is not a colour",
            dipped.colour
        );
        assert!(
            dipped.alpha > 0.0 && dipped.alpha <= 1.0,
            "{part} is dipped at {}",
            dipped.alpha
        );
    }
}

/// The panel is mounted portrait and the compositor turns it, so the wallpaper
/// is landscape and the mode in the compositor's file is not.
///
/// Read off the picture and not off the tool that drew it. The tool takes its
/// size from the same place this does, so asking it would only prove that a
/// number equals itself; what is worth knowing is whether the file somebody
/// would install is that shape. A screen that changes and a wallpaper nobody
/// redrew is exactly the way this comes back.
///
/// It is here because it was wrong. The picture was drawn the shape of the
/// panel, the daemon cropped it to the shape of the desktop, and because what
/// it held was a gradient there was nothing on screen to say so.
#[test]
fn the_garden_is_the_shape_of_the_screen() {
    let first = frames(&canvas())[0];
    assert_eq!((first.width, first.height), screen().pixels());
}

/// The whole reason this costs nothing is the first frame's duration.
///
/// If it were ever written as milliseconds where seconds were meant, the
/// wallpaper would run a wind every few seconds on a machine held in two
/// hands.
#[test]
fn the_garden_rests_for_as_long_as_it_says() {
    let said = spec().garden;
    let shown = frames(&canvas());
    assert_eq!(
        shown[0].milliseconds,
        (said.rest_seconds * 1000.0).round() as u32
    );
    assert_eq!(
        shown.len() - 1,
        (said.gust_seconds * said.frames_per_second).round() as usize
    );
    let blowing: f64 = shown[1..]
        .iter()
        .map(|frame| f64::from(frame.milliseconds))
        .sum();
    assert!(
        (blowing / 1000.0 - said.gust_seconds).abs() < 0.1,
        "{blowing}ms of wind"
    );
}

/// A gust that redraws the whole picture every frame is a wallpaper that costs
/// its own size over and over. The first frame is the picture; every frame
/// after it is a strip of the picture and nothing else.
#[test]
fn the_wind_redraws_a_band_and_not_the_picture() {
    let (width, height) = screen().pixels();
    let shown = frames(&canvas());
    assert_eq!((shown[0].width, shown[0].height), (width, height));
    for frame in &shown[1..] {
        assert_eq!(frame.width, width);
        assert!(frame.height < height, "a gust frame is the whole picture");
        assert!(frame.y + frame.height <= height);
    }
}

/// The last frame of a gust holds no petals, so what loops round to the
/// resting picture is the resting picture.
///
/// Without it the wallpaper would creep: every wind would leave its blossom
/// somewhere it had blown to.
#[test]
fn the_wind_puts_the_picture_back() {
    let size = screen().pixels();
    let said = spec().garden;
    let colours = palette::read(&read("theme/report.md")).expect("a solved palette");
    let garden = Garden {
        width: f64::from(size.0),
        height: f64::from(size.1),
        paint: said
            .paints(&|name| colours.get(name).cloned())
            .expect("a table of paints"),
        rest_seconds: said.rest_seconds,
        gust_seconds: said.gust_seconds,
        frames_per_second: said.frames_per_second,
    };

    let painted = |wind: bool| {
        let (mut surface, ctx) = sheet(size.0 as i32, size.1 as i32, 0.0).expect("a sheet");
        let tips = scene::scene(&ctx, &garden, SEED).expect("the scene draws");
        if wind {
            let mut rng = console_random::Random::seeded(SEED + 11);
            air::blown(
                &ctx,
                &garden,
                &air::flight(&garden, &tips, &mut rng, 170),
                1.0,
            )
            .expect("the wind draws");
        }
        drop(ctx);
        probe::Pixels::of(&mut surface).expect("the picture is drawn").data
    };
    assert_eq!(
        painted(true),
        painted(false),
        "the wind left its blossom behind"
    );
}

/// The compositor's own background is the picture's darkest colour, so that a
/// wallpaper daemon dying costs the right colour rather than a grey nobody
/// chose. That kindness is also a blindness: a check that samples the dark
/// part of the sky reads the same thing whether the picture is there or not.
///
/// So every probe has to sit somewhere the picture has a colour of its own.
/// This is a test rather than only a guard in the drawing, because the way it
/// comes back is somebody moving the composition and not redrawing, and then
/// the check that is meant to prove the wallpaper is painting proves nothing
/// at all while staying green.
#[test]
fn no_probe_could_pass_against_a_bare_screen() {
    let held = stamped();
    let probes: Vec<((f64, f64), String)> = held["probe"]
        .as_array()
        .expect("the stamp holds probes")
        .iter()
        .map(|one| {
            let at = one["at"].as_array().expect("a place");
            let where_ = (
                at[0].as_float().expect("a fraction across"),
                at[1].as_float().expect("a fraction down"),
            );
            let colour = one["colour"]
                .as_str()
                .expect("a colour")
                .trim_start_matches('#');
            (where_, colour.to_string())
        })
        .collect();
    assert!(
        probes.len() >= 3,
        "one probe cannot say a picture is the right way up"
    );
    let colours = palette::read(&read("theme/report.md")).expect("a solved palette");
    let dark = probe::blind(&probes, &colours["night"]).expect("colours that read");
    assert!(dark.is_empty());
}
