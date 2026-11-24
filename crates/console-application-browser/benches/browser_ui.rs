#![expect(
  dead_code,
  unused_imports,
  reason = "the shared browser modules include worker code and unit tests outside this harness"
)]

#[path = "../src/downloads.rs"]
mod downloads;
#[path = "../src/reader.rs"]
mod reader;
#[path = "../src/renderer.rs"]
mod renderer;
#[path = "../src/settings.rs"]
mod settings;
#[path = "../src/toolbar.rs"]
mod toolbar;

use std::cell::OnceCell;
use std::error::Error;
use std::hint::black_box;
use std::io;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use console_core_color::palette::{self, Wearing};
use console_core_geometry::Size;
use console_core_never::Never;
use console_core_web_engine::FrameLayout;
use console_draw_painting::Frame;
use console_input_key_symbols::Keysym;
use criterion::Criterion;

use renderer::Renderer;
use toolbar::{Action, ButtonAvailability, HEIGHT, Status, TabLabel, Toolbar};

const ADDRESS: &str = "https://example.org/guide/browsing";
const POINTS: Size<u32> = Size {
  width: 1100,
  height: HEIGHT,
};
const SCALED: Size<u32> = Size {
  width: POINTS.width.saturating_mul(3).wrapping_div(2),
  height: POINTS.height.saturating_mul(3).wrapping_div(2),
};
const TITLES: [&str; 6] = [
  "Browsing guide",
  "Project notes",
  "Weather forecast",
  "Downloads",
  "Café menu",
  "Getting started",
];

type BenchFault = Box<dyn Error>;

const MAX_BUFFER: u64 = 4 * 1024 * 1024;

#[derive(Clone, Copy)]
enum Work {
  Paint,
  Edit,
}

enum AddressLength {
  Original,
  Extended,
}

struct Scenario<'a> {
  name: &'a str,
  titles: &'a [&'a str],
  device: Size<u32>,
  work: Work,
}

fn toolbar(colors: Wearing, titles: &[&str]) -> Result<Toolbar, Never> {
  let Ok(mut toolbar) = Toolbar::with_palette(&[Renderer::WebKit, Renderer::Chromium], colors);
  let tabs = (1u32..)
    .zip(titles)
    .map(|(id, &title)| TabLabel {
      id,
      title: title.to_owned(),
    })
    .collect();
  let Ok(_changed) = toolbar.tabs(tabs, 1);
  let Ok(_address) = toolbar.address(ADDRESS);
  let Ok(_status) = toolbar.status(Status {
    back: ButtonAvailability::Enabled,
    ..Status::default()
  });
  Ok(toolbar)
}

fn buffer(size: Size<u32>) -> Result<Vec<u8>, BenchFault> {
  let layout = FrameLayout::rgba(size)
    .map_err(|fault| io::Error::other(format!("invalid benchmark framebuffer: {fault:?}")))?;

  match (layout.bytes <= MAX_BUFFER).then_some(()) {
    Some(()) => {}
    None => {
      return Err(Box::from(io::Error::other(
        "oversized toolbar benchmark frame",
      )));
    }
  }

  let length = console_core_number_conversion::checked_index(layout.bytes)?;
  Ok(vec![0; length.min(4_194_304)])
}

fn start_edit(toolbar: &mut Toolbar) -> Result<(), BenchFault> {
  let Ok(action) = toolbar.key(Keysym::l, toolbar::CONTROL, ADDRESS);

  match (action == Action::Handled).then_some(()) {
    Some(()) => {}
    None => {
      return Err(Box::from(io::Error::other(
        "Ctrl+L must begin address editing",
      )));
    }
  }

  let Ok(()) = toolbar.text(ADDRESS);
  Ok(())
}

fn edit(toolbar: &mut Toolbar, length: AddressLength) -> Result<AddressLength, Never> {
  Ok(match length {
    AddressLength::Original => {
      let Ok(()) = toolbar.text(black_box("s"));
      AddressLength::Extended
    }
    AddressLength::Extended => {
      let Ok(action) = toolbar.key(Keysym::BackSpace, 0, ADDRESS);
      let _observed = black_box(action);
      AddressLength::Original
    }
  })
}

fn measure(
  scenario: &Scenario<'_>,
  colors: Wearing,
  iterations: u64,
) -> Result<Duration, BenchFault> {
  let Ok(mut toolbar) = toolbar(colors, scenario.titles);
  let frame = Frame {
    points: POINTS,
    device: scenario.device,
  };
  let mut pixels = buffer(frame.device)?;
  let mut length = AddressLength::Original;

  match scenario.work {
    Work::Paint => {}
    Work::Edit => start_edit(&mut toolbar)?,
  }

  let start = Instant::now();

  for _ in 0..iterations {
    match scenario.work {
      Work::Paint => {}
      Work::Edit => {
        let Ok(next) = edit(&mut toolbar, length);
        length = next;
      }
    }

    toolbar.paint(black_box(&mut pixels), frame)?;
  }

  Ok(start.elapsed())
}

fn register(criterion: &mut Criterion) -> Result<(), BenchFault> {
  let Ok(shipped) = palette::read(include_str!(
    "../../../files/usr/local/lib/console/palette.sh"
  ));
  let colors = Wearing::out_of(&shipped)?;
  let [first, ..] = TITLES;
  let single = [first];
  let mut group = criterion.benchmark_group("browser_toolbar");

  for scenario in [
    Scenario {
      name: "paint_one_tab_1x",
      titles: &single,
      device: POINTS,
      work: Work::Paint,
    },
    Scenario {
      name: "paint_six_tabs_1x",
      titles: &TITLES,
      device: POINTS,
      work: Work::Paint,
    },
    Scenario {
      name: "paint_six_tabs_1_5x",
      titles: &TITLES,
      device: SCALED,
      work: Work::Paint,
    },
    Scenario {
      name: "edit_address_and_paint_1_5x",
      titles: &TITLES,
      device: SCALED,
      work: Work::Edit,
    },
  ] {
    let first_fault = OnceCell::new();
    group.bench_function(scenario.name, |bencher| {
      bencher.iter_custom(|iterations| {
        let failed_duration = Duration::from_millis(iterations);

        match first_fault.get() {
          Some(_recorded) => return failed_duration,
          None => {}
        }

        match measure(&scenario, colors, iterations) {
          Ok(duration) => duration,
          Err(fault) => {
            match first_fault.set(fault) {
              Ok(()) => {}
              Err(additional_fault) => {
                eprintln!("toolbar benchmark also failed: {additional_fault}")
              }
            }

            failed_duration
          }
        }
      });
    });

    match first_fault.into_inner() {
      Some(fault) => {
        group.finish();
        return Err(fault);
      }
      None => {}
    }
  }

  group.finish();
  Ok(())
}

fn main() -> ExitCode {
  let mut criterion = Criterion::default().configure_from_args();

  match register(&mut criterion) {
    Ok(()) => {
      criterion.final_summary();
      ExitCode::SUCCESS
    }
    Err(fault) => {
      eprintln!("toolbar benchmark failed: {fault}");
      ExitCode::FAILURE
    }
  }
}
