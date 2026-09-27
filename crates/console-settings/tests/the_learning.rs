//! What the machine learns from a rocker press, asked without a machine.
//!
//! The whole of this feature is one table and three questions about it: which
//! band a reading falls in, what a band holds after being taught twice, and
//! what a band that was never taught answers. None of the three needs a
//! screen, a sensor or a person, which is why they are here rather than in a
//! check -- and the one that would need all three, whether the level it picks
//! is comfortable, is not a thing any test can answer and is why the table is
//! learned instead of written.

use console_settings::learned::{Band, Levels, band};
use console_settings::light::Lit;
use console_settings::screen::{BRIGHTEST, DARKEST};

use console_core_never::Never;

fn taught(pairs: &[(i64, i64)]) -> Result<Levels, Never> {
    let Ok(nothing) = Levels::empty();

    Ok(pairs.iter().fold(nothing, |levels, (raw, level)| {
        let Ok(band) = band(Lit(*raw));
        let Ok(taught) = levels.taught(band, *level);

        taught
    }))
}

fn level_for(levels: &Levels, raw: i64) -> Result<Option<i64>, Never> {
    let Ok(band) = band(Lit(raw));

    levels.level_for(band)
}

#[test]
fn a_dark_room_and_a_bright_one_are_not_the_same_band() {
    let Ok(dark) = band(Lit(0));
    let Ok(lamp) = band(Lit(400));
    let Ok(sun) = band(Lit(20000));

    assert_ne!(dark, lamp);
    assert_ne!(lamp, sun);
    assert!(dark < lamp);
    assert!(lamp < sun);
}

#[test]
fn a_reading_that_barely_moves_stays_in_its_band() {
    let Ok(one) = band(Lit(351));
    let Ok(other) = band(Lit(435));

    assert_eq!(one, other);
}

#[test]
fn a_machine_that_was_never_taught_answers_nothing() {
    let Ok(nothing) = Levels::empty();

    assert_eq!(level_for(&nothing, 400), Ok(None));
}

#[test]
fn a_band_taught_once_answers_what_it_was_told() {
    let Ok(levels) = taught(&[(400, 500)]);

    assert_eq!(level_for(&levels, 400), Ok(Some(500)));
}

#[test]
fn a_second_press_moves_the_band_rather_than_replacing_it() {
    let Ok(levels) = taught(&[(400, 500), (400, 900)]);

    let Ok(held) = level_for(&levels, 400);

    assert_ne!(held, Some(900));
    assert!(held > Some(500));
}

#[test]
fn between_two_taught_bands_it_climbs() {
    let Ok(levels) = taught(&[(1, 100), (20000, 900)]);

    let Ok(dim) = level_for(&levels, 20);
    let Ok(middling) = level_for(&levels, 2000);

    assert!(dim > Some(100));
    assert!(middling > dim);
    assert!(middling < Some(900));
}

#[test]
fn outside_what_it_knows_it_holds_the_nearest() {
    let Ok(levels) = taught(&[(400, 500)]);

    assert_eq!(level_for(&levels, 0), Ok(Some(500)));
    assert_eq!(level_for(&levels, 30000), Ok(Some(500)));
}

#[test]
fn a_level_is_never_past_what_this_panel_can_show() {
    let Ok(levels) = taught(&[(400, 100000), (0, -5000)]);

    assert_eq!(level_for(&levels, 400), Ok(Some(BRIGHTEST)));
    assert_eq!(level_for(&levels, 0), Ok(Some(DARKEST)));
}

#[test]
fn what_is_written_down_reads_back_the_same() {
    let Ok(levels) = taught(&[(1, 200), (400, 500), (20000, 900)]);

    let Ok(written) = levels.serialize();
    let Ok(again) = Levels::read(&written);

    assert_eq!(again, levels);
}

#[test]
fn a_file_somebody_has_been_editing_loses_only_the_line_they_broke() {
    let Ok(read) = Levels::read("3 400\nnonsense\n\n99 500\n5 600\n");

    assert_eq!(read.level_for(Band(3)), Ok(Some(400)));
    assert_eq!(read.level_for(Band(5)), Ok(Some(600)));
}
