//! Whole-calculation CPU and Vulkan timings. Run with `just compute-benchmark`.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;

use console_core_never::Never;
use console_kernel::compute::hosted::{CHECKSUM_KERNEL, Checksums, Output};
use console_kernel::compute::{Block, MAX_BLOCKS};
use console_operating_system_linux::compute_map;
use console_operating_system_linux::program_runtime::compute::{
  self, Backend, Map, MapProgram, Router, RoutingPolicy, Target, Vulkan, WRAPPING_SUM, Workload,
};

const REPETITIONS: u32 = 8;

fn measure<T: Eq>(
  router: &mut Router<'_>,
  work: Workload,
  mut calculate: impl FnMut(Target<'_>) -> io::Result<T>,
) -> io::Result<()> {
  for _ in 0..REPETITIONS {
    router.verify(work, &mut calculate, PartialEq::eq)?;
  }

  let Ok(recorded) = router.history(work);
  let history =
    recorded.ok_or_else(|| io::Error::other("verified runs have no timing history"))?;
  let Ok(serial_average) = history.average(Backend::SerialCpu);
  let serial = serial_average.ok_or_else(|| io::Error::other("serial CPU sample is missing"))?;
  let Ok(cpu_average) = history.average(Backend::Cpu);
  let cpu = cpu_average.ok_or_else(|| io::Error::other("CPU sample is missing"))?;
  let Ok(gpu_average) = history.average(Backend::Vulkan);
  let gpu = gpu_average.ok_or_else(|| io::Error::other("Vulkan sample is missing"))?;
  let route = router.run(work, &mut calculate)?;
  println!(
    "{:<16} {:>8} {:>11.3} {:>11.3} {:>10.3} {:>9?}",
    work.effect,
    work.size,
    serial.as_secs_f64() * 1_000.0,
    cpu.as_secs_f64() * 1_000.0,
    gpu.as_secs_f64() * 1_000.0,
    route.backend,
  );
  Ok(())
}

compute_map! {
  static MANDELBROT = [8, 8, 1] |item| {
    let Ok(column): Result<u32, Never> = item.id(0);
    let Ok(row): Result<u32, Never> = item.id(1);
    let Ok(side): Result<u32, Never> = item.extent(0);
    let Ok(column_coordinate): Result<u32, Never> = item.word(column);
    let Ok(row_coordinate): Result<u32, Never> = item.word(u32::wrapping_add(side, row));
    let x: f32 = f32::from_bits(column_coordinate);
    let y: f32 = f32::from_bits(row_coordinate);
    let mut real: f32 = 0.0f32;
    let mut imaginary: f32 = 0.0f32;
    let mut steps: u32 = 0;
    let Ok(step_limit): Result<u32, Never> = item.param(0);

    for _turn in 0u32..step_limit {
      let real_squared: f32 = real * real;
      let imaginary_squared: f32 = imaginary * imaginary;
      let norm: f32 = real_squared + imaginary_squared;

      match (norm >= 4.0f32).then_some(()) {
        Some(()) => { break; }
        None => {}
      }

      let next_real: f32 = real_squared - imaginary_squared + x;
      imaginary = 2.0f32 * real * imaginary + y;
      real = next_real;
      steps = u32::wrapping_add(steps, 1);
    }

    steps
  }
}

#[derive(Clone, Copy)]
struct Fractal {
  side: u32,
  steps: u32,
}

fn fractal(
  target: Target<'_>,
  Fractal { side, steps }: Fractal,
  input: &[u32],
  shader: &Path,
) -> io::Result<Vec<u32>> {
  compute::map(
    target,
    Map {
      program: MapProgram {
        kernel: &MANDELBROT,
        spirv: Some(shader),
      },
      input,
      dimensions: [side, side, 1],
      parameters: [steps, 0],
    },
  )
}

enum Profile {
  Standard,
  Large,
}

struct Programs<'a> {
  worker: &'a Path,
  checksum: &'a Path,
  image: &'a Path,
}

const USAGE: &str =
  "usage: compute-benchmark WORKER CHECKSUM.spv MANDELBROT.spv [standard|large]";

fn parse_profile(arguments: &[OsString]) -> io::Result<Profile> {
  match arguments {
    [] => Ok(Profile::Standard),
    [name] => match name.to_str() {
      Some("standard") => Ok(Profile::Standard),
      Some("large") => Ok(Profile::Large),
      Some(_) | None => Err(io::Error::other(
        "expected the standard or large benchmark profile",
      )),
    },
    [_, _, ..] => Err(io::Error::other(
      "expected the standard or large benchmark profile",
    )),
  }
}

fn main() -> io::Result<()> {
  let arguments: Vec<_> = std::env::args_os().skip(1).collect();
  let mode = arguments.first().and_then(|name| name.to_str());

  match (mode, arguments.as_slice()) {
    (Some("--emit-shader"), [_, name]) => {
      let kernel = match name.to_str() {
        Some("checksum-bench") => &CHECKSUM_KERNEL,
        Some("wrapping-sum") => &WRAPPING_SUM,
        Some("mandelbrot") => &MANDELBROT,
        Some(_) | None => return Err(io::Error::other("unknown benchmark kernel")),
      };
      let glsl = kernel.glsl()?;
      io::stdout().lock().write_all(glsl.as_bytes())
    }
    (Some("--emit-shader"), _) => Err(io::Error::other(USAGE)),
    (_, [worker, checksum, image, profile_arguments @ ..]) => {
      let profile = parse_profile(profile_arguments)?;
      run(
        Programs {
          worker: Path::new(worker),
          checksum: Path::new(checksum),
          image: Path::new(image),
        },
        profile,
      )
    }
    (_, []) | (_, [_]) | (_, [_, _]) => Err(io::Error::other(USAGE)),
  }
}

fn run(programs: Programs<'_>, profile: Profile) -> io::Result<()> {
  let Ok(worker) = Vulkan::new(programs.worker.to_path_buf());
  let Ok(mut router) = Router::new(Some(&worker), RoutingPolicy::default());
  println!(
    "Average per call over {REPETITIONS} verified runs per target, including setup and transfers"
  );
  println!(
    "{:<16} {:>8} {:>11} {:>11} {:>10} {:>9}",
    "Workload", "Size", "Serial (ms)", "CPU auto (ms)", "GPU (ms)", "Route"
  );

  for count in [64, 4_096, MAX_BLOCKS] {
    let blocks: Vec<Block> = (0..count)
      .map(|index| {
        let [first, ..] = index.to_le_bytes();
        [first; 256]
      })
      .collect();
    let effect = Checksums::new(&blocks, Output::Sum)
      .map_err(|fault| io::Error::other(format!("{fault:?}")))?;
    let Ok(workload) = effect.workload();
    measure(&mut router, workload, |target| {
      effect.calculate(target, Some(programs.checksum))
    })?;
  }

  let larger: &[(u32, u32)] = match profile {
    Profile::Standard => &[],
    Profile::Large => &[(1024, 512), (2048, 512), (1024, 4096)],
  };

  for (side, steps) in [(64, 512), (256, 512), (512, 512)]
    .into_iter()
    .chain(larger.iter().copied())
  {
    let work = Workload {
      effect: match steps {
        512 => "Mandelbrot",
        _ => "Mandelbrot 4096",
      },
      size: u64::from(side).wrapping_mul(u64::from(side)),
    };
    let Ok(input) = fractal_input(side);
    let parameters = Fractal { side, steps };
    measure(&mut router, work, |target| {
      fractal(target, parameters, &input, programs.image)
    })?;
  }

  Ok(())
}

fn fractal_input(side: u32) -> Result<Vec<u32>, console_core_never::Never> {
  use console_core_number_conversion::nearest_f32_from_u64;

  let Ok(width) = nearest_f32_from_u64(u64::from(side));
  let columns = (0..side).map(|index| {
    let Ok(column) = nearest_f32_from_u64(u64::from(index));
    -2.25 + 3.0 * (column / width)
  });
  let rows = (0..side).map(|index| {
    let Ok(row) = nearest_f32_from_u64(u64::from(index));
    -1.5 + 3.0 * (row / width)
  });

  Ok(columns.chain(rows).map(f32::to_bits).collect())
}
