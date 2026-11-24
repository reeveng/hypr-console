//! Compare the viewer's two frame preparation and Wayland presentation paths.

use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use console_concurrency::Cores;
use console_core_geometry::Point;
use console_core_iteration::{Step, iterate};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index};
use console_core_shapes::Picture as ShapePicture;
use console_draw_painting::{Frame, over_picture};
use console_draw_surface::standing::DmaBufSupport;
use console_draw_surface::{
  Anchor, DmaBufView, Keyboard, Margin, Part, Room, Surface, Under, Visible, Wanted,
};
use console_films::{Picture, Seconds};
use console_operating_system_linux::program_runtime::compute::Vulkan;

type Failure = Box<dyn std::error::Error>;

#[derive(Clone, Copy)]
enum Backend<'a> {
  Cpu,
  Vulkan(&'a Vulkan, &'a Path),
}

impl Backend<'_> {
  fn submit(
    self,
    surface: &mut Surface,
    part: Part,
    picture: &Picture,
    cores: Cores,
  ) -> Result<(), Failure> {
    let visible = match self {
      Self::Cpu => {
        let Ok(bitmap) = picture.color_pixels_at(part.size, cores);
        let Ok(pixels) = bitmap.into_strided();
        let mut painted = Ok(());
        let visible =
          surface.draw_layer_with(part, &mut painted, |result, bytes, device, _| {
            *result = over_picture(
              bytes,
              Frame {
                points: part.size,
                device,
              },
              ShapePicture {
                at: Point { x: 0, y: 0 },
                size: part.size,
                pixels: &pixels,
              },
              &[],
            );
            Ok(())
          })?;
        painted?;
        visible
      }
      Self::Vulkan(worker, shader) => {
        let frame = picture.export_pixels_at(part.size, cores, worker, shader)?;
        let visible = surface.present_dma_buf(
          part,
          DmaBufView {
            fd: frame.fd.as_fd(),
            size: frame.size,
            stride: frame.stride,
          },
        )?;
        visible
      }
    };

    match visible {
      Visible::Yes => Ok(()),
      Visible::NotYet => Err(Failure::from(
        "the compositor did not accept the video frame",
      )),
    }
  }
}

fn deadline(now: Instant, delay: Duration) -> Result<Instant, Failure> {
  let time = now
    .checked_add(delay)
    .ok_or("presentation deadline exceeds the clock range")?;
  Ok(time)
}

fn wait_until(surface: &mut Surface, until: Instant) -> Result<(), Failure> {
  let waited = iterate(surface, |surface_2| {
    match until.checked_duration_since(Instant::now()) {
      Some(left) => match surface_2.wait(&[], Some(left)) {
        Ok(()) => Ok(Step::Again(surface_2)),
        Err(fault) => Ok(Step::Halt(Err(Failure::from(fault)))),
      },
      None => Ok(Step::Halt(Ok(()))),
    }
  })?;
  waited
}

struct Presentation<'a> {
  part: Part,
  backend: Backend<'a>,
  cores: Cores,
  frame: Duration,
  start: Instant,
  first: Seconds,
}

#[derive(Default)]
struct Measurement {
  ready: Vec<Duration>,
  skipped: u64,
}

impl Presentation<'_> {
  fn sample(
    &self,
    surface: &mut Surface,
    picture: &Picture,
    measurement: &mut Measurement,
  ) -> Result<(), Failure> {
    let Ok(at) = picture.at();
    let delay = Duration::try_from_secs_f64((at - self.first.0).max(0.0))?;
    let due = deadline(self.start, delay)?;
    let before = match due.checked_sub(self.frame / 2) {
      Some(before) => before,
      None => self.start,
    };
    wait_until(surface, before)?;
    let grace = deadline(due, self.frame.saturating_mul(2))?;

    match Instant::now().cmp(&grace) {
      std::cmp::Ordering::Greater => {
        measurement.skipped = measurement.skipped.saturating_add(1);
      }
      std::cmp::Ordering::Less | std::cmp::Ordering::Equal => {
        let preparing = Instant::now();
        self
          .backend
          .submit(surface, self.part, picture, self.cores)?;
        measurement.ready.push(preparing.elapsed());
      }
    }

    Ok(())
  }
}

fn run(
  surface: &mut Surface,
  clip: &Path,
  part: Part,
  backend: Backend<'_>,
) -> Result<(), Failure> {
  let opened = console_films::open(clip)?;
  let film = opened.ok_or("unsupported clip")?;
  let Ok(timing) = film.facts();
  let facts = timing.ok_or("clip has no timing")?;
  let frame = Duration::try_from_secs_f64(1.0 / facts.rate.0)?;
  let video = film.pictures(Seconds(0.0))?;
  let mut pictures = video.ok_or("clip has no video")?;
  let Ok(cores) = Cores::counted();

  for _ in 0..24_u8 {
    let next = pictures.next_picture()?;
    let picture = next.ok_or("clip is too short to warm up")?;
    backend.submit(surface, part, &picture, cores)?;
    let until = deadline(Instant::now(), frame)?;
    wait_until(surface, until)?;
  }

  let settle = deadline(Instant::now(), frame.saturating_mul(4))?;
  wait_until(surface, settle)?;
  let Ok(before) = surface.presentations();
  let next = pictures.next_picture()?;
  let first_picture = next.ok_or("empty measurement")?;
  let Ok(first) = first_picture.at();
  let presentation = Presentation {
    part,
    backend,
    cores,
    frame,
    start: Instant::now(),
    first: Seconds(first),
  };
  let mut measurement = Measurement::default();
  presentation.sample(surface, &first_picture, &mut measurement)?;

  let measured = iterate(
    (&mut *surface, pictures, measurement),
    |(surface_2, mut pictures_2, mut measurement_2)| -> Result<_, Never> {
      let picture = match pictures_2.next_picture() {
        Ok(Some(picture)) => picture,
        Ok(None) => return Ok(Step::Halt(Ok(measurement_2))),
        Err(fault) => return Ok(Step::Halt(Err(Failure::from(fault)))),
      };

      Ok(
        match presentation.sample(surface_2, &picture, &mut measurement_2) {
          Ok(()) => Step::Again((surface_2, pictures_2, measurement_2)),
          Err(fault) => Step::Halt(Err(fault)),
        },
      )
    },
  )?;
  let Measurement { mut ready, skipped } = measured?;

  let settle_2 = deadline(Instant::now(), Duration::from_millis(250))?;
  wait_until(surface, settle_2)?;
  let Ok(after) = surface.presentations();
  let committed = after.committed.saturating_sub(before.committed);
  let presented = after.presented.saturating_sub(before.presented);
  let discarded = after.discarded.saturating_sub(before.discarded);
  let unconfirmed = committed.saturating_sub(presented.saturating_add(discarded));
  let (median, p95) = percentiles(&mut ready)?;
  let name = match backend {
    Backend::Cpu => "CPU / shared memory",
    Backend::Vulkan(..) => "Vulkan / DMA-BUF",
  };
  println!(
    "{name:<21} ready p50 {:6.2} ms  p95 {:6.2} ms  scheduled {:3}  presented {:3}  late {:3}  discarded {:3}  unconfirmed {:3}",
    median.as_secs_f64() * 1000.0,
    p95.as_secs_f64() * 1000.0,
    committed.saturating_add(skipped),
    presented,
    skipped,
    discarded,
    unconfirmed
  );

  match (committed, unconfirmed, presented) {
    (0, _, _) | (_, _, 0) => Err(Failure::from(
      "the compositor did not confirm the measurement",
    )),
    (_, 0, _) => Ok(()),
    (_, _, _) => Err(Failure::from(
      "the compositor left presentation feedback pending",
    )),
  }
}

fn percentiles(ready: &mut [Duration]) -> Result<(Duration, Duration), Failure> {
  ready.sort_unstable();
  let Ok(count) = fitted::<_, u64>(ready.len());
  let Ok(mid) = index(count.saturating_div(2));
  let rank = count.saturating_sub(count.saturating_div(20));
  let Ok(percentile) = index(rank.saturating_sub(1));
  let median = ready.get(mid).ok_or("every frame was skipped")?;
  let p95 = ready
    .get(percentile)
    .ok_or("the 95th percentile sample is missing")?;
  Ok((*median, *p95))
}

fn main() -> Result<(), Failure> {
  const USAGE: &str = "usage: video-presentation CLIP WORKER COLOUR.spv";
  let args: Vec<_> = std::env::args_os().skip(1).collect();
  let (clip, worker, shader) = match args.as_slice() {
    [switch] => match (switch == "--emit-shader").then_some(()) {
      Some(()) => {
        let glsl = console_films::colour_glsl()?;
        print!("{glsl}");
        return Ok(());
      }
      None => return Err(Failure::from(USAGE)),
    },
    [clip, worker, shader] => (clip, worker, shader),
    _ => return Err(Failure::from(USAGE)),
  };
  let clip_path = Path::new(clip);
  let opened = console_films::open(clip_path)?;
  let film = opened.ok_or("unsupported clip")?;
  let Ok(timing) = film.facts();
  let facts = timing.ok_or("clip has no video facts")?;
  let mut surface = Surface::connect()?;
  surface.show(&Wanted {
    namespace: String::from("org.console.VideoBenchmark"),
    anchor: Anchor::Whole,
    size: facts.size,
    margin: Margin::default(),
    keyboard: Keyboard::Declines,
    room: Room::Over,
    under: Under::None,
  })?;
  surface.draw(|pixels, _, _| {
    pixels.fill(0);
    Ok(())
  })?;
  let Ok(logical) = surface.logical();
  let room = logical.ok_or("benchmark window did not configure")?;
  let size = facts.size;
  let (margin_width, margin_height) = match (
    room.width.checked_sub(size.width),
    room.height.checked_sub(size.height),
  ) {
    (Some(width), Some(height)) => (width, height),
    (None, _) | (_, None) => {
      return Err(Failure::from(format!(
        "benchmark needs a {}×{} viewport, but the output is {}×{}",
        size.width, size.height, room.width, room.height
      )));
    }
  };
  let x = i32::try_from(margin_width.saturating_div(2))?;
  let y = i32::try_from(margin_height.saturating_div(2))?;
  let part = Part {
    at: Point { x, y },
    size,
  };
  let until = deadline(Instant::now(), Duration::from_millis(250))?;
  wait_until(&mut surface, until)?;
  let Ok(support) = surface.dma_buf_support();

  match support {
    DmaBufSupport::Available => {}
    DmaBufSupport::Unavailable => {
      return Err(Failure::from(
        "the compositor does not advertise linear RGBA DMA-BUF",
      ));
    }
  }

  println!("{}×{} at {:.2} fps", size.width, size.height, facts.rate.0);
  let Ok(device) = Vulkan::new(PathBuf::from(worker));
  let gpu = Backend::Vulkan(&device, Path::new(shader));

  for backend in [Backend::Cpu, gpu, gpu, Backend::Cpu] {
    run(&mut surface, clip_path, part, backend)?;
  }

  Ok(())
}
