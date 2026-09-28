//! The bar with a font in the room, which is the half `showing` cannot answer.
//!
//! Everything about where a slab lands is arithmetic and is asserted without a
//! screen. What is not arithmetic is how wide a run of words comes out, and
//! every width on this bar is downstream of that: a slab is what it says plus
//! its padding, so an icon that measures wider than anyone expected is a bar
//! whose right-hand group walks off the screen and whose middle is no longer
//! the middle. That needs Pango and nothing else, which is what this is.
//!
//! It also draws, into a slab of memory with no compositor anywhere near it,
//! because the thing worth knowing about a shape list is whether the shapes
//! land where the numbers said. `CONSOLE_BAR_PICTURE` names a file to write the
//! frame into when someone wants to look at one.

use std::error::Error;
use std::path::PathBuf;

use console_compositor::Workspace;
use console_core_color::palette::PaletteError;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::index;
use console_draw_painting::{Frame, Run, measure_text, onto};
use console_status_bar::state::{BarState, Open};
use console_status_bar::reading::{Reading, Tone, StatusItem};
use console_status_bar::showing::{
    self, Bar, Face, Filling, Fitting, Measured, Slot, Wearing,
};
use console_onscreen::Up;

const SCREEN: Size<u32> = Size { width: 1024, height: 640 };

const SPENT: &str = "ground=2b2029\ntext=f7e7f3\nsoft=c9aec4\npink=ff9ecb\nnight=231b26\nbutter=f2d692\ncoral=ff8f8f\nleaf=9fd88f\nfill=723b5f";

fn sample_palette() -> Result<Wearing, PaletteError> {
    let Ok(spent) = console_core_color::palette::read(SPENT);

    Wearing::out_of(&spent)
}

fn says(icon: &str, beside: Option<&str>, tone: Tone) -> Result<Reading, Never> {
    Ok(Reading { icon: String::from(icon), beside: beside.map(String::from), tone })
}

fn sample_state() -> Result<BarState, Never> {
    let Ok(sound) = says("\u{f057e}", None, Tone::Plain);
    let Ok(bluetooth) = says("\u{f00b1}", None, Tone::Plain);
    let Ok(network) = says("\u{f0928}", None, Tone::Plain);
    let Ok(battery) = says("\u{f0079}", Some("64%\u{2007}"), Tone::Plain);
    let Ok(bell) = says("\u{f009c}", None, Tone::Secondary);
    let Ok(music) = says("\u{f075a}", None, Tone::Plain);

    Ok(BarState {
        readings: vec![
            (StatusItem::Sound, sound),
            (StatusItem::Bluetooth, bluetooth),
            (StatusItem::Network, network),
            (StatusItem::Battery, battery),
        ],
        bell,
        music,
        clock: String::from("14:30"),
        workspaces: vec![
            Workspace { id: 1, named: String::from("1"), windows: Some(0) },
            Workspace { id: 2, named: String::from("2"), windows: Some(0) },
            Workspace { id: 3, named: String::from("3"), windows: Some(0) },
        ],
        front: Some(2),
        open: Open {
            launcher: Up::OnScreen,
            keyboard: Up::NotThere,
            music: Up::NotThere,
            notifications: Up::NotThere,
            calendar: Up::NotThere,
            settings: Up::NotThere,
            tab: None,
        },
    })
}

fn measure(slots: &[Slot], fitting: Fitting) -> Result<Vec<Measured>, Never> {
    Ok(slots
        .iter()
        .map(|slot| {
            let runs = slot
                .spans
                .iter()
                .map(|span| {
                    let Ok(font) = fitting.font(span.face);
                    let Ok(weight) = span.face.weight();
                    let Ok(size) =
                        measure_text(Run { said: &span.text, weight, width: u32::MAX }, &font);

                    size
                })
                .collect();

            Measured { slot: slot.clone(), runs }
        })
        .collect())
}

fn bar(filling: Filling) -> Result<(Bar, Fitting), Never> {
    let Ok(fitting) = Fitting::of_em();
    let Ok(state) = sample_state();
    let Ok(layout) = state.layout(filling);
    let Ok(left) = measure(&layout.left, fitting);
    let Ok(middle) = measure(&layout.middle, fitting);
    let Ok(right) = measure(&layout.right, fitting);

    Ok((Bar { left, middle, right, filling }, fitting))
}

#[test]
fn nothing_measured_into_this_bar_makes_it_wider_than_the_screen() {
    let Ok((bar, fitting)) = bar(Filling::None);
    let Ok(left) = showing::group(&bar.left, fitting);
    let Ok(middle) = showing::group(&bar.middle, fitting);
    let Ok(right) = showing::group(&bar.right, fitting);

    assert!(
        left.saturating_add(middle).saturating_add(right) < SCREEN.width,
        "the three groups measure {left} + {middle} + {right} across a screen of {}",
        SCREEN.width
    );
}

#[test]
fn the_middle_clears_both_groups_beside_it() {
    let Ok((bar, fitting)) = bar(Filling::None);
    let Ok(left) = showing::group(&bar.left, fitting);
    let Ok(middle) = showing::group(&bar.middle, fitting);
    let Ok(right) = showing::group(&bar.right, fitting);
    let from = SCREEN.width.saturating_sub(middle).div_ceil(2);
    let ends = from.saturating_add(middle);

    assert!(from > left, "the clock starts at {from}, inside a left group {left} wide");
    assert!(
        ends < SCREEN.width.saturating_sub(right),
        "the clock ends at {ends}, inside a right group {right} wide"
    );
}

#[test]
fn every_icon_measures_to_about_one_cell_so_that_nothing_shifts_as_a_reading_changes() -> Result<(), Box<dyn Error>> {
    let Ok(fitting) = Fitting::of_em();
    let Ok(font) = fitting.font(Face::Icon);
    let widths: Vec<u32> = ["\u{f057e}", "\u{f00b1}", "\u{f0928}", "\u{f0079}", "\u{f009c}"]
        .into_iter()
        .map(|icon| {
            let Ok(size) = measure_text(
                Run { said: icon, weight: console_core_shapes::Weight::Plain, width: u32::MAX },
                &font,
            );

            size.width
        })
        .collect();
    let widest = widths.iter().copied().max().ok_or("no icon was measured")?;
    let narrowest = widths.iter().copied().min().ok_or("no icon was measured")?;

    assert!(widest > 0, "no icon measured at all: is the font installed?");
    assert_eq!(
        widest, narrowest,
        "the icons measure {widths:?}, so the right of the bar moves when a reading changes"
    );

    Ok(())
}

fn slab() -> Result<(Vec<u8>, Size<u32>), Never> {
    let Ok(fitting) = Fitting::of_em();
    let Ok(tall) = fitting.height();
    let device = Size { width: SCREEN.width, height: tall };
    let long = device.width.saturating_mul(device.height).saturating_mul(4);
    let Ok(long) = index(long);

    Ok((vec![0; long], device))
}

fn painted(filling: Filling) -> Result<(Vec<u8>, Size<u32>), Box<dyn Error>> {
    let Ok((bar, _fitting)) = bar(filling);
    let wearing = sample_palette()?;
    let Ok(drawn) = showing::along(&bar, &wearing, SCREEN);
    let Ok((mut pixels, device)) = slab();

    onto(&mut pixels, Frame { device, points: drawn.room }, &drawn.shapes)?;

    Ok((pixels, device))
}

fn at(pixels: &[u8], device: Size<u32>, on: Point<u32>) -> Result<[u8; 3], Never> {
    let Ok(position) = index(on.y.saturating_mul(device.width).saturating_add(on.x));

    Ok(match pixels.as_chunks::<4>().0.get(position) {
        Some([blue, green, red, _alpha]) => [*red, *green, *blue],
        None => [0, 0, 0],
    })
}

#[test]
fn the_surface_is_painted_over_the_strip_rows_as_well_as_the_bar() -> Result<(), Box<dyn Error>> {
    let (pixels, device) = painted(Filling::None)?;
    let Ok(fitting) = Fitting::of_em();
    let Ok(ground) = at(&pixels, device, Point { x: 0, y: 0 });
    let last = device.height.saturating_sub(1);
    let Ok(strip) = at(&pixels, device, Point { x: device.width.div_euclid(2), y: fitting.deep });
    let Ok(corner) = at(&pixels, device, Point { x: device.width.saturating_sub(1), y: last });

    assert_ne!(ground, [0, 0, 0], "the bar was not painted at all");
    assert_eq!(strip, ground, "the strip is a different color with no apply running");
    assert_eq!(corner, ground, "the last row of the surface was left unpainted");

    Ok(())
}

#[test]
fn a_running_apply_fills_the_strip_from_the_left_and_stops_where_it_has_got_to() -> Result<(), Box<dyn Error>> {
    let (pixels, device) = painted(Filling::At(500))?;
    let Ok(fitting) = Fitting::of_em();
    let Ok(ground) = at(&pixels, device, Point { x: 0, y: 0 });
    let row = fitting.deep;
    let half = device.width.div_euclid(2);
    let filled = |across| {
        let Ok(pixel) = at(&pixels, device, Point { x: across, y: row });

        pixel != ground
    };

    assert!(filled(0), "the strip is empty at the left with an apply half done");
    assert!(filled(half.saturating_sub(2)), "the strip stops short of half at half way");
    assert!(!filled(half.saturating_add(2)), "the strip runs past half at half way");
    assert!(!filled(device.width.saturating_sub(1)), "the strip is full at half way");

    Ok(())
}

#[cfg_attr(
    dylint_lib = "explicit026_env_read_once",
    allow(explicit026_env_read_once, reason = "the picture is asked for by whoever runs this by hand, and nothing else reads the name")
)]
#[test]
fn a_picture_of_the_bar_is_written_when_someone_asks_for_one() -> Result<(), Box<dyn Error>> {
    let into = match std::env::var("CONSOLE_BAR_PICTURE") {
        Ok(into) => PathBuf::from(into),
        Err(_no_one_wants_to_look_at_one) => return Ok(()),
    };
    let (pixels, device) = painted(Filling::At(380))?;

    console_core_atomic_writes::whole(&into, &pixels)?;

    println!("{}: {}x{} bgra", into.display(), device.width, device.height);

    Ok(())
}
