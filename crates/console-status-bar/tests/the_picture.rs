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


use console_compositor::Workspace;
use console_core_geometry::Size;
use console_draw_painting::{Frame, Run, measured, onto};
use console_status_bar::state::{BarState, Open};
use console_status_bar::reading::{Reading, Tone, StatusItem};
use console_status_bar::showing::{
    self, Bar, Face, Filling, Fitting, Measured, Slot, Wearing,
};
use console_onscreen::Up;

const SCREEN: Size<u32> = Size { width: 1024, height: 640 };

const SPENT: &str = "ground=2b2029\ntext=f7e7f3\nsoft=c9aec4\npink=ff9ecb\nnight=231b26\nbutter=f2d692\ncoral=ff8f8f\nleaf=9fd88f\nfill=723b5f";

fn wearing() -> Wearing {
    let Ok(spent) = console_core_color::palette::read(SPENT);

    match Wearing::out_of(&spent) {
        Ok(wearing) => wearing,
        Err(why) => panic!("a whole palette should dress a bar: {why}"),
    }
}

fn says(icon: &str, beside: Option<&str>, tone: Tone) -> Reading {
    Reading { icon: icon.to_string(), beside: beside.map(str::to_string), tone }
}

fn held() -> BarState {
    BarState {
        readings: vec![
            (StatusItem::Sound, says("\u{f057e}", None, Tone::Plain)),
            (StatusItem::Bluetooth, says("\u{f00b1}", None, Tone::Plain)),
            (StatusItem::Network, says("\u{f0928}", None, Tone::Plain)),
            (StatusItem::Battery, says("\u{f0079}", Some("64%\u{2007}"), Tone::Plain)),
        ],
        bell: says("\u{f009c}", None, Tone::Secondary),
        music: says("\u{f075a}", None, Tone::Plain),
        clock: "14:30".to_string(),
        workspaces: vec![
            Workspace { id: 1, named: "1".to_string(), windows: Some(0) },
            Workspace { id: 2, named: "2".to_string(), windows: Some(0) },
            Workspace { id: 3, named: "3".to_string(), windows: Some(0) },
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
    }
}

fn measure(slots: &[Slot], fitting: Fitting) -> Vec<Measured> {
    slots
        .iter()
        .map(|slot| {
            let runs = slot
                .spans
                .iter()
                .map(|span| {
                    let Ok(font) = fitting.font(span.face);
                    let Ok(weight) = span.face.weight();
                    let Ok(size) =
                        measured(Run { said: &span.text, weight, width: u32::MAX }, &font);

                    size
                })
                .collect();

            Measured { slot: slot.clone(), runs }
        })
        .collect()
}

fn bar(filling: Filling) -> (Bar, Fitting) {
    let Ok(fitting) = Fitting::of_em();
    let Ok(layout) = held().layout(filling);
    let bar = Bar {
        left: measure(&layout.left, fitting),
        middle: measure(&layout.middle, fitting),
        right: measure(&layout.right, fitting),
        filling,
    };

    (bar, fitting)
}

#[test]
fn nothing_measured_into_this_bar_makes_it_wider_than_the_screen() {
    let (bar, fitting) = bar(Filling::None);
    let Ok(left) = showing::group(&bar.left, fitting);
    let Ok(middle) = showing::group(&bar.middle, fitting);
    let Ok(right) = showing::group(&bar.right, fitting);

    assert!(
        left + middle + right < SCREEN.width,
        "the three groups measure {left} + {middle} + {right} across a screen of {}",
        SCREEN.width
    );
}

#[test]
fn the_middle_clears_both_groups_beside_it() {
    let (bar, fitting) = bar(Filling::None);
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
fn every_icon_measures_to_about_one_cell_so_that_nothing_shifts_as_a_reading_changes() {
    let Ok(fitting) = Fitting::of_em();
    let Ok(font) = fitting.font(Face::Icon);
    let widths: Vec<u32> = ["\u{f057e}", "\u{f00b1}", "\u{f0928}", "\u{f0079}", "\u{f009c}"]
        .into_iter()
        .map(|icon| {
            let Ok(size) = measured(
                Run { said: icon, weight: console_core_shapes::Weight::Plain, width: u32::MAX },
                &font,
            );

            size.width
        })
        .collect();
    let widest = widths.iter().copied().max().unwrap_or(0);
    let narrowest = widths.iter().copied().min().unwrap_or(0);

    assert!(widest > 0, "no icon measured at all: is the font installed?");
    assert_eq!(
        widest, narrowest,
        "the icons measure {widths:?}, so the right of the bar moves when a reading changes"
    );
}

fn slab() -> (Vec<u8>, Size<u32>) {
    let Ok(fitting) = Fitting::of_em();
    let Ok(tall) = fitting.height();
    let device = Size { width: SCREEN.width, height: tall };
    let long = device.width.saturating_mul(device.height).saturating_mul(4);

    (vec![0; long.try_into().unwrap()], device)
}

fn painted(filling: Filling) -> (Vec<u8>, Size<u32>) {
    let (bar, _fitting) = bar(filling);
    let Ok(drawn) = showing::along(&bar, &wearing(), SCREEN);
    let (mut pixels, device) = slab();

    match onto(&mut pixels, Frame { device, points: drawn.room }, &drawn.shapes) {
        Ok(()) => {}
        Err(why) => panic!("the bar should draw: {why}"),
    }

    (pixels, device)
}

fn at(pixels: &[u8], device: Size<u32>, across: u32, down: u32) -> [u8; 3] {
    let stride = device.width.saturating_mul(4);
    let start = down.saturating_mul(stride).saturating_add(across.saturating_mul(4));
    match pixels.get(start.try_into().unwrap()..start.saturating_add(3).try_into().unwrap()) {
        Some([blue, green, red]) => [*red, *green, *blue],
        Some(_) | None => [0, 0, 0],
    }
}

#[test]
fn the_surface_is_painted_over_the_strip_rows_as_well_as_the_bar() {
    let (pixels, device) = painted(Filling::None);
    let Ok(fitting) = Fitting::of_em();
    let ground = at(&pixels, device, 0, 0);
    let last = device.height.saturating_sub(1);

    assert_ne!(ground, [0, 0, 0], "the bar was not painted at all");
    assert_eq!(
        at(&pixels, device, device.width / 2, fitting.deep),
        ground,
        "the strip is a different color with no apply running"
    );
    assert_eq!(
        at(&pixels, device, device.width.saturating_sub(1), last),
        ground,
        "the last row of the surface was left unpainted"
    );
}

#[test]
fn a_running_apply_fills_the_strip_from_the_left_and_stops_where_it_has_got_to() {
    let (pixels, device) = painted(Filling::At(500));
    let Ok(fitting) = Fitting::of_em();
    let ground = at(&pixels, device, 0, 0);
    let row = fitting.deep;
    let filled = |across| at(&pixels, device, across, row) != ground;

    assert!(filled(0), "the strip is empty at the left with an apply half done");
    assert!(filled(device.width / 2 - 2), "the strip stops short of half at half way");
    assert!(!filled(device.width / 2 + 2), "the strip runs past half at half way");
    assert!(!filled(device.width.saturating_sub(1)), "the strip is full at half way");
}

#[test]
fn a_picture_of_the_bar_is_written_when_someone_asks_for_one() {
    let into = match std::env::var("CONSOLE_BAR_PICTURE") {
        Ok(into) => into,
        Err(_no_one_wants_to_look_at_one) => return,
    };
    let (pixels, device) = painted(Filling::At(380));

    match std::fs::write(&into, &pixels) {
        Ok(()) => println!("{into}: {}x{} bgra", device.width, device.height),
        Err(why) => panic!("{into}: {why}"),
    }
}
