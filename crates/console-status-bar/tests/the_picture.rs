//! The bar with a font in the room, which is the half `showing` cannot answer.
//!
//! Everything about where a slab lands is arithmetic and is asserted without a
//! screen. What is not arithmetic is how wide a run of words comes out, and
//! every width on this bar is downstream of that: a slab is what it says plus
//! its padding, so an icon that measures wider than anyone expected is a bar
//! whose right-hand group walks off the screen and whose middle is no longer
//! the middle. That needs the text engine and nothing else, which is what this is.
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
use console_core_number_conversion::{fitted, index};
use console_core_shapes::Shape;
use console_draw_painting::{Frame, Run, measure_text, onto};
use console_status_bar::lock_screen::{LockScreenBar, Menu};
use console_status_bar::measuring;
use console_status_bar::panels::{Panel, PanelEvent, Panels};
use console_status_bar::preferences::{Item, Placement, Preferences};
use console_status_bar::reading::{Reading, StatusItem, Tone};
use console_status_bar::settings::SettingsTab;
use console_status_bar::showing::{
  self, Bar, BarAction, Face, Filling, Fitting, Lit, Measured, Slot, Span, Wearing,
};
use console_status_bar::state::BarState;

const SCREEN: Size<u32> = Size {
  width: 1024,
  height: 640,
};

const SPENT: &str = "ground=2b2029\ntext=f7e7f3\nsoft=c9aec4\npink=ff9ecb\nnight=231b26\nbutter=f2d692\ncoral=ff8f8f\nleaf=9fd88f\nfill=723b5f";

#[test]
fn pointer_feedback_marks_the_click_target_and_leaves_the_progress_strip_plain()
-> Result<(), Box<dyn Error>> {
  let Ok((bar, _)) = bar(Filling::At(250));
  let wearing = sample_palette()?;
  let Ok(mut drawn) = showing::along(&bar, &wearing, SCREEN);
  let plain = drawn.clone();
  let region = plain
    .touching
    .iter()
    .find(|hit| hit.action == BarAction::Calendar)
    .ok_or("calendar click target is missing")?;
  let point = region.panel.at;
  assert_eq!(drawn.show_hover(Some(point), wearing), Ok(Some(*region)));
  assert_eq!(drawn.on(point), Ok(Some(region.action)));
  assert_eq!(drawn.touching, plain.touching);
  let marker = drawn
    .shapes
    .strip_prefix(plain.shapes.as_slice())
    .ok_or("bar changed behind the marker")?;
  assert!(matches!(marker, [Shape::Line(edge), Shape::Line(accent)]
    if edge.width > accent.width && accent.color == wearing.pink));

  let Ok(last_row) = fitted::<_, i32>(plain.room.height.saturating_sub(1));
  let progress = Point { x: 1, y: last_row };
  let mut cleared = plain.clone();
  assert_eq!(cleared.show_hover(Some(progress), wearing), Ok(None));
  assert_eq!(cleared.show_hover(None, wearing), Ok(None));
  assert_eq!(cleared, plain);

  Ok(())
}

#[test]
fn bottom_bar_keeps_the_horizontal_layout_with_progress_on_the_inner_edge()
-> Result<(), Box<dyn Error>> {
  let Ok(mut state) = sample_state();
  state.clock = "Sat 21 Nov  12:59 pm".to_owned();
  let mut preferences = Preferences::default();
  preferences.placement = Placement::Bottom;
  let Ok(layout) = state.configured(&preferences, Filling::At(250));
  let Ok(fitting) = Fitting::of_em();
  let Ok(bar) = measuring::sized(&layout, fitting);
  let wearing = sample_palette()?;
  let Ok(top) = showing::along_with_applets(&bar, &[], &wearing, SCREEN, Placement::Top);
  let Ok(bottom) = showing::along_with_applets(&bar, &[], &wearing, SCREEN, Placement::Bottom);

  assert_eq!(top.room, bottom.room);
  assert!(!bottom.touching.is_empty());
  assert_eq!(top.touching.len(), bottom.touching.len());
  let Ok(strip) = fitted::<_, i32>(fitting.thin);

  for (above, below) in top.touching.iter().zip(&bottom.touching) {
    assert_eq!(above.action, below.action);
    assert_eq!(above.panel.at.x, below.panel.at.x);
    assert_eq!(above.panel.at.y.saturating_add(strip), below.panel.at.y);
    let Ok(width) = fitted::<_, i32>(below.panel.size.width);
    let Ok(height) = fitted::<_, i32>(below.panel.size.height);
    let center = Point {
      x: below.panel.at.x.saturating_add(width.saturating_div(2)),
      y: below.panel.at.y.saturating_add(height.saturating_div(2)),
    };
    assert_eq!(bottom.on(center), Ok(Some(below.action)));
  }

  assert!(
    bottom
      .shapes
      .iter()
      .any(|shape| matches!(shape, Shape::Text(text) if text.said == state.clock))
  );
  assert!(matches!(bottom.shapes.last(), Some(Shape::Panel(panel))
    if panel.at == (Point { x: 0, y: 0 }) && panel.size.height == fitting.thin
    && panel.fill == wearing.fill));

  Ok(())
}

#[test]
fn sidebar_items_fit_and_use_their_drawn_hit_regions_on_both_edges() -> Result<(), Box<dyn Error>>
{
  let Ok((bar, applets, fitting, room)) = sidebar_fixture();
  let wearing = sample_palette()?;

  for placement in [Placement::Left, Placement::Right] {
    let Ok(drawn) = showing::sidebar(&bar, &applets, &wearing, room, placement);
    assert_eq!(drawn.room, room);
    assert_eq!(room.width, 40);

    let mut clock_rows = drawn.shapes.iter().filter_map(|shape| match shape {
      Shape::Text(text) => match text.said.as_str() {
        "12" | "59" | "pm" => Some(text),
        _ => None,
      },
      Shape::Panel(_)
      | Shape::Picture(_)
      | Shape::Cropped(_)
      | Shape::Line(_)
      | Shape::Clip(_) => None,
    });
    let mut previous_y = i32::MIN;

    for part in ["12", "59", "pm"] {
      let row = clock_rows.next().ok_or("missing sidebar clock row")?;
      assert_eq!(row.said, part);
      assert!(row.at.y > previous_y);
      previous_y = row.at.y;
    }

    assert!(
      drawn
        .touching
        .iter()
        .any(|hit| hit.action == BarAction::Applet(0))
    );
    assert!(
      drawn
        .touching
        .iter()
        .any(|hit| hit.action == BarAction::Settings(SettingsTab::Display))
    );
    assert!(
      drawn
        .touching
        .iter()
        .any(|hit| hit.action == BarAction::Clipboard)
    );
    assert!(!drawn.touching.iter().any(|hit| matches!(
      hit.action,
      BarAction::Workspace(_) | BarAction::Settings(SettingsTab::Battery)
    )));
    let Ok(()) = hit_regions_agree(&drawn);
    let progress = match drawn.shapes.last() {
      Some(Shape::Panel(progress)) => progress,
      Some(_) | None => {
        return Err(Box::<dyn Error>::from("missing sidebar progress strip"));
      }
    };

    let inner_edge = match ::core::primitive::bool::then_some(placement == Placement::Left, ()) {
      ::core::option::Option::Some(()) => room.width.saturating_sub(fitting.thin),
      ::core::option::Option::None => 0,
    };
    let Ok(progress_x) = fitted::<_, u32>(progress.at.x);
    assert_eq!(progress_x, inner_edge);
    assert_eq!(
      progress.size,
      Size {
        width: fitting.thin,
        height: ::core::primitive::u32::wrapping_div(SCREEN.height, 4)
      }
    );
  }

  Ok(())
}

fn sample_palette() -> Result<Wearing, PaletteError> {
  let Ok(spent) = console_core_color::palette::read(SPENT);

  Wearing::out_of(&spent)
}

fn says(icon: &str, beside: Option<&str>, tone: Tone) -> Result<Reading, Never> {
  Ok(Reading {
    icon: String::from(icon),
    beside: beside.map(String::from),
    tone,
  })
}

fn sample_state() -> Result<BarState, Never> {
  let Ok(sound) = says("\u{f057e}", None, Tone::Plain);
  let Ok(bluetooth) = says("\u{f00b1}", None, Tone::Plain);
  let Ok(network) = says("\u{f0928}", None, Tone::Plain);
  let Ok(battery) = says("\u{f0079}", Some("64%\u{2007}"), Tone::Plain);
  let Ok(bell) = says("\u{f009c}", None, Tone::Secondary);
  let Ok(music) = says("\u{f075a}", None, Tone::Plain);

  let mut held = BarState {
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
      Workspace {
        id: 1,
        named: String::from("1"),
        windows: Some(0),
      },
      Workspace {
        id: 2,
        named: String::from("2"),
        windows: Some(0),
      },
      Workspace {
        id: 3,
        named: String::from("3"),
        windows: Some(0),
      },
    ],
    front: Some(2),
    panels: Panels::default(),
  };
  let Ok(_starts_nothing) = held.told(PanelEvent::Shown(vec![Panel::Launcher]));

  Ok(held)
}

fn bar(filling: Filling) -> Result<(Bar, Fitting), Never> {
  let Ok(fitting) = Fitting::of_em();
  let Ok(state) = sample_state();
  let Ok(layout) = state.layout(filling);
  let Ok(bar) = measuring::sized(&layout, fitting);

  Ok((bar, fitting))
}

fn sidebar_fixture() -> Result<(Bar, Vec<Measured>, Fitting, Size<u32>), Never> {
  let Ok(mut state) = sample_state();
  state.clock = "Sat 21 Nov  12:59 pm".to_owned();
  let mut preferences = Preferences::default();
  preferences.placement = Placement::Left;
  let Ok(()) = preferences.toggle(Item::Workspaces);
  let Ok(()) = preferences.toggle(Item::Battery);
  let Ok(()) = preferences.toggle_settings(SettingsTab::Display);
  let Ok(layout) = state.configured(&preferences, Filling::At(250));
  let Ok(fitting) = Fitting::of_em();
  let Ok(mut bar) = measuring::sized(&layout, fitting);
  let Ok(width) = fitting.sidebar_width();
  let room = Size {
    width,
    height: SCREEN.height,
  };
  let Ok(text_width) = fitting.sidebar_content_width(width);

  for group in [&mut bar.left, &mut bar.middle, &mut bar.right] {
    let Ok(()) = measuring::compact(group, fitting, text_width);
  }

  let Ok(mut slots) = state.settings_applets(&preferences);
  slots.push(Slot {
    spans: vec![Span {
      text: "A very long applet label that must be shortened".to_owned(),
      face: Face::Reading,
    }],
    tone: Tone::Plain,
    lit: Lit::No,
    action: Some(BarAction::Applet(0)),
  });
  let Ok(mut applets) = measuring::every(&slots, fitting);
  let Ok(()) = measuring::compact(&mut applets, fitting, text_width);

  Ok((bar, applets, fitting, room))
}

fn hit_regions_agree(drawn: &showing::Rendered) -> Result<(), Never> {
  let mut previous_bottom = 0;

  for hit in &drawn.touching {
    let Ok(height) = fitted::<_, i32>(hit.panel.size.height);
    let Ok(width) = fitted::<_, i32>(hit.panel.size.width);
    assert!(hit.panel.at.y >= previous_bottom);
    previous_bottom = hit.panel.at.y.saturating_add(height);
    let center = Point {
      x: hit.panel.at.x.saturating_add(width.saturating_div(2)),
      y: hit.panel.at.y.saturating_add(height.saturating_div(2)),
    };
    assert_eq!(drawn.on(center), Ok(Some(hit.action)));
  }

  Ok(())
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

  assert!(
    from > left,
    "the clock starts at {from}, inside a left group {left} wide"
  );
  assert!(
    ends < SCREEN.width.saturating_sub(right),
    "the clock ends at {ends}, inside a right group {right} wide"
  );
}

#[test]
fn every_icon_measures_to_about_one_cell_so_that_nothing_shifts_as_a_reading_changes()
-> Result<(), Box<dyn Error>> {
  let Ok(fitting) = Fitting::of_em();
  let Ok(font) = fitting.font(Face::Icon);
  let widths: Vec<u32> = [
    "\u{f057e}",
    "\u{f00b1}",
    "\u{f0928}",
    "\u{f0079}",
    "\u{f009c}",
  ]
  .into_iter()
  .map(|icon| {
    let Ok(size) = measure_text(
      Run {
        said: icon,
        weight: console_core_shapes::Weight::Plain,
        width: u32::MAX,
      },
      &font,
    );

    size.width
  })
  .collect();
  let widest = widths.iter().copied().max().ok_or("no icon was measured")?;
  let narrowest = widths.iter().copied().min().ok_or("no icon was measured")?;

  assert!(
    widest > 0,
    "no icon measured at all: is the font installed?"
  );
  assert_eq!(
    widest, narrowest,
    "the icons measure {widths:?}, so the right of the bar moves when a reading changes"
  );

  Ok(())
}

fn slab() -> Result<(Vec<u8>, Size<u32>), Never> {
  let Ok(fitting) = Fitting::of_em();
  let Ok(tall) = fitting.height();
  let device = Size {
    width: SCREEN.width,
    height: tall,
  };
  let long = device.width.saturating_mul(device.height).saturating_mul(4);
  let Ok(long_2) = index(long);

  Ok((vec![0; long_2.min(4_194_304)], device))
}

fn painted(filling: Filling) -> Result<(Vec<u8>, Size<u32>), Box<dyn Error>> {
  let Ok((bar, _fitting)) = bar(filling);
  let wearing = sample_palette()?;
  let Ok(drawn) = showing::along(&bar, &wearing, SCREEN);
  let Ok((mut pixels, device)) = slab();

  onto(
    &mut pixels,
    Frame {
      device,
      points: drawn.room,
    },
    &drawn.shapes,
  )?;

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
  let Ok(strip) = at(
    &pixels,
    device,
    Point {
      x: device.width.div_euclid(2),
      y: fitting.deep,
    },
  );
  let Ok(corner) = at(
    &pixels,
    device,
    Point {
      x: device.width.saturating_sub(1),
      y: last,
    },
  );

  assert_ne!(ground, [0, 0, 0], "the bar was not painted at all");
  assert_eq!(
    strip, ground,
    "the strip is a different color with no apply running"
  );
  assert_eq!(
    corner, ground,
    "the last row of the surface was left unpainted"
  );

  Ok(())
}

#[test]
fn a_running_apply_fills_the_strip_from_the_left_and_stops_where_it_has_got_to()
-> Result<(), Box<dyn Error>> {
  let (pixels, device) = painted(Filling::At(500))?;
  let Ok(fitting) = Fitting::of_em();
  let Ok(ground) = at(&pixels, device, Point { x: 0, y: 0 });
  let row = fitting.deep;
  let half = device.width.div_euclid(2);
  let filled = |across| {
    let Ok(pixel) = at(&pixels, device, Point { x: across, y: row });

    pixel != ground
  };

  assert!(
    filled(0),
    "the strip is empty at the left with an apply half done"
  );
  assert!(
    filled(half.saturating_sub(2)),
    "the strip stops short of half at half way"
  );
  assert!(
    !filled(half.saturating_add(2)),
    "the strip runs past half at half way"
  );
  assert!(
    !filled(device.width.saturating_sub(1)),
    "the strip is full at half way"
  );

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  allow(
    explicit026_env_read_once,
    reason = "the picture is asked for by whoever runs this by hand, and nothing else reads the name"
  )
)]
#[test]
fn a_picture_of_the_bar_is_written_when_someone_asks_for_one() -> Result<(), Box<dyn Error>> {
  let into = match std::env::var("CONSOLE_BAR_PICTURE") {
    Ok(into) => PathBuf::from(into),
    Err(_no_one_wants_to_look_at_one) => return Ok(()),
  };
  let (pixels, device) = painted(Filling::At(380))?;

  console_core_atomic_writes::whole(&into, &pixels)?;

  println!(
    "{}: {}x{} bgra",
    into.display(),
    device.width,
    device.height
  );

  Ok(())
}

fn over_the_way_in(menu: Menu) -> Result<LockScreenBar, Never> {
  let Ok(sound) = says("\u{f0580}", Some("50%\u{2007}"), Tone::Plain);
  let Ok(battery) = says("\u{f007e}", Some("64%\u{2007}"), Tone::Plain);

  Ok(LockScreenBar {
    readings: [(StatusItem::Sound, sound), (StatusItem::Battery, battery)]
      .into_iter()
      .collect(),
    clock: String::from("Sun 20 Sep  02:15"),
    menu: Some(menu),
    focus: None,
  })
}

#[test]
fn a_row_hangs_under_the_bar_inside_the_screen_and_a_thumb_on_each_choice_lands_on_it()
-> Result<(), Box<dyn Error>> {
  let wearing = sample_palette()?;
  let Ok(fitting) = Fitting::of_em();
  let Ok(tall) = fitting.height();
  let Ok(across) = fitted::<u32, i32>(SCREEN.width);
  let Ok(down) = fitted::<u32, i32>(tall);

  for (menu, many) in [(Menu::Power, 3), (Menu::Sound, 3)] {
    let Ok(bar) = over_the_way_in(menu);
    let Ok(drawn) = bar.pictured(&wearing, SCREEN);
    let row: Vec<_> = drawn
      .touching
      .iter()
      .filter(|region| matches!(region.action, BarAction::Power(_) | BarAction::Volume(_)))
      .collect();

    assert_eq!(
      row.len(),
      many,
      "the {menu:?} row is not every choice it offers"
    );

    for region in row {
      let Ok(wide) = fitted::<u32, i32>(region.panel.size.width);
      let Ok(high) = fitted::<u32, i32>(region.panel.size.height);
      let middle = Point {
        x: region.panel.at.x.saturating_add(wide.div_euclid(2)),
        y: region.panel.at.y.saturating_add(high.div_euclid(2)),
      };
      let Ok(landed) = drawn.on(middle);

      assert!(
        region.panel.at.y >= down,
        "{:?} is drawn over the bar rather than under it",
        region.action
      );
      assert!(
        region.panel.at.x.saturating_add(wide) <= across,
        "{:?} runs off the right of the screen",
        region.action
      );
      assert_eq!(
        landed,
        Some(region.action),
        "a thumb on {:?} lands on something else",
        region.action
      );
    }
  }

  Ok(())
}

#[test]
fn the_volume_tiles_match_full_paints_after_reusing_a_buffer() -> Result<(), Box<dyn Error>> {
  use console_draw_surface::memory::Shared;
  use console_draw_surface::retained::{BufferRevision, Retained};
  use console_status_bar::retained::{Scene, Volume, VolumeTiles, pixels};

  let Ok(mut held) = sample_state();
  set_test_volume(&mut held, Volume::High)?;
  let Ok(fitting) = Fitting::of_em();
  let preferences = Preferences::default();
  let wearing = sample_palette()?;
  let scene = Scene {
    held: &held,
    preferences: &preferences,
    slots: &[],
    wearing,
    room: SCREEN,
    fitting,
    filling: Filling::None,
    focus: None,
  };
  let Ok(rendered) = scene.render();
  let device = Size {
    width: rendered.room.width.saturating_mul(5).div_ceil(2),
    height: rendered.room.height.saturating_mul(5).div_ceil(2),
  };
  let frame = Frame {
    points: rendered.room,
    device,
  };
  let prepared = VolumeTiles::prepare(scene, device)?;
  let tiles = prepared.ok_or("volume tiles should fit the slot")?;
  let initial = pixels(&rendered, device)?;
  let Ok(length) = fitted::<_, u64>(initial.len());
  let mut actual = vec![0; initial.len()];
  let mut retained = Retained::default();
  retained.replace(device, initial)?;
  let buffer = Shared::of(length)?;
  let mut revision = BufferRevision::default();
  retained.copy_to(&buffer, &mut revision)?;
  let mut changed = held.clone();
  let mut packed = PackedTileCapture::new(&tiles)?;

  for volume in [
    Volume::Low,
    Volume::Silent,
    Volume::Medium,
    Volume::High,
    Volume::Unavailable,
  ] {
    set_test_volume(&mut changed, volume)?;
    tiles.update(volume, &mut retained)?;
    let Ok(expected_frame) = (Scene {
      held: &changed,
      ..scene
    })
    .render();
    let expected = pixels(&expected_frame, device)?;
    retained.copy_to(&buffer, &mut revision)?;
    buffer.read_pixels_at(&mut actual, 0)?;
    assert!(actual == expected, "retained pixels differ for {volume:?}");

    packed.verify(&tiles, volume, frame, &expected)?;
  }

  Ok(())
}

fn set_test_volume(
  state: &mut BarState,
  volume: console_status_bar::retained::Volume,
) -> Result<(), Box<dyn Error>> {
  let sound = state
    .readings
    .iter_mut()
    .find(|(item, _)| *item == StatusItem::Sound);
  let (_, reading) = sound.ok_or("fixture needs a sound reading")?;
  let Ok(current) = volume.reading();
  *reading = current;

  Ok(())
}

struct PackedTileCapture {
  buffer: console_draw_surface::memory::Shared,
  pixels: Vec<u8>,
}

impl PackedTileCapture {
  fn new(tiles: &console_status_bar::retained::VolumeTiles) -> Result<Self, Box<dyn Error>> {
    use console_draw_surface::memory::Shared;
    use console_status_bar::retained::Volume;

    let Ok(tile) = tiles.tile(Volume::High);
    let (_, sample) = tile.ok_or("fixture needs a packed tile")?;
    let Ok(size) = sample.tile_size();
    let length = u64::from(size.width)
      .saturating_mul(u64::from(size.height))
      .saturating_mul(4);
    let buffer = Shared::of(length)?;
    let Ok(bytes) = index(length);

    Ok(Self {
      buffer,
      pixels: vec![0; bytes.min(2_147_483_647)],
    })
  }

  fn verify(
    &mut self,
    tiles: &console_status_bar::retained::VolumeTiles,
    volume: console_status_bar::retained::Volume,
    frame: Frame,
    expected: &[u8],
  ) -> Result<(), Box<dyn Error>> {
    let Ok(tile) = tiles.tile(volume);
    let (part, patch) = tile.ok_or("fixture needs a packed tile for every icon")?;
    patch.write_tile(&self.buffer)?;
    self.buffer.read_pixels_at(&mut self.pixels, 0)?;
    let Ok(region) = console_draw_surface::on_the_device(part, frame.points, frame.device);
    let wanted = tile_bytes_at(expected, frame.device.width, region)?;
    assert_eq!(self.pixels, wanted, "packed pixels differ for {volume:?}");

    Ok(())
  }
}

fn tile_bytes_at(
  frame: &[u8],
  width: u32,
  part: console_draw_surface::Damage,
) -> Result<Vec<u8>, Box<dyn Error>> {
  let Ok(row_bytes) = index(u64::from(width).saturating_mul(4));
  let Ok(x) = fitted::<_, u64>(part.at.x);
  let Ok(right) = fitted::<_, u64>(part.at.x.saturating_add(part.size.width));
  let Ok(first) = index(x.saturating_mul(4));
  let Ok(last) = index(right.saturating_mul(4));
  let Ok(top) = fitted::<_, u32>(part.at.y);
  let Ok(height) = fitted::<_, u32>(part.size.height);
  let Ok(start) = index(top);
  let Ok(rows) = index(height);
  let mut pixels = Vec::new();

  for row in frame.chunks_exact(row_bytes.max(1)).skip(start).take(rows) {
    let crop = row.get(first..last).ok_or("tile must fit within the bar")?;
    pixels.extend_from_slice(crop);
  }

  Ok(pixels)
}
