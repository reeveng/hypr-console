use std::error::Error;
use std::hint::black_box;
use std::time::{Duration, Instant};

use console_compositor::Workspace;
use console_core_color::palette;
use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, fixed_index, index};
use console_draw_painting::{Frame, onto};
use console_draw_surface::memory::Shared;
use console_status_bar::measuring::{compact, sized};
use console_status_bar::preferences::{Placement, Preferences};
use console_status_bar::reading::{Reading, StatusItem, Tone};
use console_status_bar::retained::{Scene, Volume, VolumeTiles};
use console_status_bar::settings::SettingsTab;
use console_status_bar::showing::{self, BarAction, Filling, Fitting, Layout, Rendered, Wearing};
use console_status_bar::state::BarState;
use criterion::measurement::WallTime;
use criterion::{BatchSize, BenchmarkGroup, BenchmarkId, Criterion};

struct Scenario {
  name: &'static str,
  placement: Placement,
  screen: Size<u32>,
  scale: (u32, u32),
}

const HANDHELD: Size<u32> = Size {
  width: 1024,
  height: 640,
};

const SCENARIOS: [Scenario; 3] = [
  Scenario {
    name: "top_desktop_1x",
    placement: Placement::Top,
    screen: Size {
      width: 1920,
      height: 1200,
    },
    scale: (1, 1),
  },
  Scenario {
    name: "top_handheld_2_5x",
    placement: Placement::Top,
    screen: HANDHELD,
    scale: (5, 2),
  },
  Scenario {
    name: "left_handheld_2_5x",
    placement: Placement::Left,
    screen: HANDHELD,
    scale: (5, 2),
  },
];

fn reading(icon: &str, beside: Option<&str>) -> Result<Reading, Never> {
  Ok(Reading {
    icon: icon.to_owned(),
    beside: beside.map(str::to_owned),
    tone: Tone::Plain,
  })
}

fn fixture() -> Result<BarState, Never> {
  let Ok(sound) = reading("\u{f057e}", None);
  let Ok(bluetooth) = reading("\u{f00b1}", None);
  let Ok(network) = reading("\u{f0928}", None);
  let Ok(battery) = reading("\u{f0079}", Some("64%"));
  let Ok(bell) = reading("\u{f009c}", None);
  let Ok(music) = reading("\u{f075a}", None);

  Ok(BarState {
    readings: vec![
      (StatusItem::Sound, sound),
      (StatusItem::Bluetooth, bluetooth),
      (StatusItem::Network, network),
      (StatusItem::Battery, battery),
    ],
    bell,
    music,
    clock: "Wed 25 Nov  12:34 pm".to_owned(),
    workspaces: [1, 4, 7]
      .map(|id| Workspace {
        id,
        named: id.to_string(),
        windows: Some(1),
      })
      .to_vec(),
    front: Some(4),
    panels: Default::default(),
  })
}

fn render(scenario: &Scenario, layout: &Layout, wearing: &Wearing) -> Result<Rendered, Never> {
  let Ok(fitting) = Fitting::of_em();
  let Ok(mut bar) = sized(layout, fitting);

  match scenario.placement {
    Placement::Top | Placement::Bottom => {
      showing::along_with_applets(&bar, &[], wearing, scenario.screen, scenario.placement)
    }
    Placement::Left | Placement::Right => {
      let Ok(width) = fitting.sidebar_width();
      let Ok(content) = fitting.sidebar_content_width(width);

      for group in [&mut bar.left, &mut bar.middle, &mut bar.right] {
        let Ok(()) = compact(group, fitting, content);
      }

      showing::sidebar(
        &bar,
        &[],
        wearing,
        Size {
          width,
          ..scenario.screen
        },
        scenario.placement,
      )
    }
  }
}

fn sound_button(drawn: &Rendered) -> Result<Point<i32>, Box<dyn Error>> {
  let hit = drawn
    .touching
    .iter()
    .find(|hit| hit.action == BarAction::Settings(SettingsTab::Sound))
    .ok_or("fixture must contain a Sound button")?;
  let Ok(x) = fitted::<_, i32>(hit.panel.size.width.div_euclid(2));
  let Ok(y) = fitted::<_, i32>(hit.panel.size.height.div_euclid(2));

  Ok(Point {
    x: hit.panel.at.x.saturating_add(x),
    y: hit.panel.at.y.saturating_add(y),
  })
}

fn frame(scenario: &Scenario, points: Size<u32>) -> Result<Frame, Never> {
  let (numerator, denominator) = scenario.scale;
  let scaled = |length: u32| length.saturating_mul(numerator).div_ceil(denominator);

  Ok(Frame {
    points,
    device: Size {
      width: scaled(points.width),
      height: scaled(points.height),
    },
  })
}

fn register(criterion: &mut Criterion) -> Result<(), Box<dyn Error>> {
  let Ok(colors) = palette::read(include_str!(
    "../../../files/usr/local/lib/console/palette.sh"
  ));
  let wearing = Wearing::out_of(&colors)?;
  let Ok(state) = fixture();
  let mut group = criterion.benchmark_group("status_bar");

  for scenario in SCENARIOS {
    register_scenario(&mut group, &scenario, &state, &wearing)?;
  }

  register_volume(&mut group, &state, wearing)?;
  group.finish();

  Ok(())
}

fn register_scenario(
  group: &mut BenchmarkGroup<'_, WallTime>,
  scenario: &Scenario,
  state: &BarState,
  wearing: &Wearing,
) -> Result<(), Box<dyn Error>> {
  let mut preferences = Preferences::default();
  preferences.placement = scenario.placement;
  let Ok(layout) = state.configured(&preferences, Filling::At(250));
  let Ok(drawn) = render(scenario, &layout, wearing);
  let position = sound_button(&drawn)?;
  let _ = group.bench_function(BenchmarkId::new("select_action", scenario.name), |bench| {
    bench.iter_batched_ref(
      || state.clone(),
      |held| {
        let Ok(hit) = drawn.on(black_box(position));

        match hit {
          Some(action) => {
            let Ok(effects) = held.pressed(action);
            let _ = black_box(effects);
          }
          None => {}
        }
      },
      BatchSize::NumIterations(1024),
    );
  });
  let _ = group.bench_function(BenchmarkId::new("layout", scenario.name), |bench| {
    bench.iter(|| {
      let Ok(rendered) = render(scenario, black_box(&layout), wearing);
      black_box(rendered)
    });
  });

  register_paint(group, scenario, &drawn)
}

fn register_paint(
  group: &mut BenchmarkGroup<'_, WallTime>,
  scenario: &Scenario,
  drawn: &Rendered,
) -> Result<(), Box<dyn Error>> {
  let Ok(frame) = frame(scenario, drawn.room);
  let bytes = u64::from(frame.device.width)
    .saturating_mul(u64::from(frame.device.height))
    .saturating_mul(4);
  let Ok(length) = index(bytes);
  let mut pixels = vec![0; length.min(4_194_304)];
  onto(&mut pixels, frame, &drawn.shapes)?;

  let _ = group.bench_function(BenchmarkId::new("paint", scenario.name), |bench| {
    bench.iter_batched_ref(
      || pixels.clone(),
      |buffer| match onto(black_box(buffer), frame, black_box(&drawn.shapes)) {
        Ok(()) => {}
        Err(fault) => {
          console_core_logging::warn!("status bar benchmark painting failed: {fault}")
        }
      },
      BatchSize::NumIterations(8),
    );
  });

  Ok(())
}

fn register_volume(
  group: &mut BenchmarkGroup<'_, WallTime>,
  held: &BarState,
  wearing: Wearing,
) -> Result<(), Box<dyn Error>> {
  let scenario = Scenario {
    name: "top_handheld_2_5x",
    placement: Placement::Top,
    screen: HANDHELD,
    scale: (5, 2),
  };
  let Ok(fitting) = Fitting::of_em();
  let preferences = Preferences::default();
  let scene = Scene {
    held,
    preferences: &preferences,
    slots: &[],
    wearing,
    room: scenario.screen,
    fitting,
    filling: Filling::None,
    focus: None,
  };
  let Ok(rendered) = scene.render();
  let Ok(frame) = frame(&scenario, rendered.room);
  let prepared = VolumeTiles::prepare(scene, frame.device)?;
  let tiles = prepared.ok_or("volume tiles must be reusable")?;
  let mut run = VolumeRun::new(tiles)?;

  #[cfg_attr(
    dylint_lib = "explicit047_no_captured_permission",
    expect(
      explicit047_no_captured_permission,
      reason = "Criterion invokes the benchmark synchronously, and the final report reads the accumulated samples after it returns."
    )
  )]
  let _registered = group.bench_function(
    BenchmarkId::new("volume_to_reusable_buffer", scenario.name),
    |bench| {
      bench.iter_custom(|iterations| {
        let Ok(total) = run.measure(iterations);
        total
      });
    },
  );

  match run.failure {
    Some(fault) => return Err(Box::<dyn Error>::from(fault)),
    None => {
      let Ok(()) = run.samples.report(scenario.name);
    }
  }

  Ok(())
}

#[doc = "Two tile buffers cycle behind three visual states."]
struct VolumeRun {
  tiles: VolumeTiles,
  buffers: [Shared; 2],
  volumes: [Volume; 3],
  samples: Latencies,
  failure: Option<std::io::Error>,
}

impl VolumeRun {
  fn new(tiles: VolumeTiles) -> std::io::Result<Self> {
    let Ok(tile) = tiles.tile(Volume::High);
    let (_, initial) = tile.ok_or(std::io::ErrorKind::InvalidData)?;
    let Ok(size) = initial.tile_size();
    let length = u64::from(size.width)
      .saturating_mul(u64::from(size.height))
      .saturating_mul(4);
    let first = Shared::of(length)?;
    let second = Shared::of(length)?;
    initial.write_tile(&first)?;
    initial.write_tile(&second)?;

    Ok(Self {
      tiles,
      buffers: [first, second],
      volumes: [Volume::Low, Volume::Medium, Volume::High],
      samples: Latencies::default(),
      failure: None,
    })
  }

  fn measure(&mut self, iterations: u64) -> Result<Duration, Never> {
    let mut total = Duration::ZERO;

    for _ in 0..iterations {
      let [volume, ..] = self.volumes;
      let [buffer, ..] = &self.buffers;
      let started = Instant::now();
      let Ok(tile) = self.tiles.tile(black_box(volume));
      let written = match tile {
        Some((_, patch)) => patch.write_tile(buffer),
        None => Err(std::io::Error::from(std::io::ErrorKind::InvalidData)),
      };
      let elapsed = started.elapsed();

      match written {
        Ok(()) => {}
        Err(fault) => {
          self.failure = Some(fault);
          break;
        }
      }

      let Ok(()) = self.samples.record(elapsed);
      total = total.saturating_add(elapsed);
      self.volumes.rotate_left(1);
      self.buffers.rotate_left(1);
    }

    Ok(total)
  }
}

struct Latencies {
  bins: Box<[u64]>,
  count: u64,
}

impl Default for Latencies {
  fn default() -> Self {
    Self {
      bins: vec![0; 10_001].into_boxed_slice(),
      count: 0,
    }
  }
}

impl Latencies {
  fn record(&mut self, elapsed: Duration) -> Result<(), Never> {
    let Ok(last) = fixed_index(self.bins.len().saturating_sub(1));
    let Ok(bin) = index(elapsed.as_nanos().div_ceil(100).min(u128::from(last)));

    match self.bins.get_mut(bin) {
      Some(count) => *count = count.saturating_add(1),
      None => {}
    }

    self.count = self.count.saturating_add(1);

    Ok(())
  }

  #[must_use]
  fn percentile(&self, percent: u64) -> Result<String, Never> {
    let rank = self.count.saturating_mul(percent).div_ceil(100);
    let mut seen = 0_u64;

    for (bin, count) in self.bins.iter().enumerate() {
      seen = seen.saturating_add(*count);

      match (seen >= rank).then_some(()) {
        Some(()) => {
          let Ok(bucket) = fixed_index(bin);
          let nanos = bucket.saturating_mul(100);

          return Ok(match (nanos >= 1_000_000).then_some(()) {
            Some(()) => String::from("over 1 ms"),
            None => format!("{nanos} ns"),
          });
        }
        None => {}
      }
    }

    Ok(String::from("over 1 ms"))
  }

  fn report(&self, scenario: &str) -> Result<(), Never> {
    match self.count {
      0 => {}
      count => {
        let Ok(p50) = self.percentile(50);
        let Ok(p99) = self.percentile(99);
        println!("volume_to_reusable_buffer/{scenario} p50={p50} p99={p99} ({count} updates)");
      }
    }

    Ok(())
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let mut criterion = Criterion::default().configure_from_args();
  register(&mut criterion)?;
  criterion.final_summary();

  Ok(())
}
