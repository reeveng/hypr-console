//! Run a typed blur-and-threshold effect as a map pipeline or a fused pass.

use std::io::{self, Write};
use std::marker::PhantomData;
use std::path::Path;

use console_core_never::Never;
use console_core_state_machine::{EffectDependencies, Machine, Queue, Service};
use console_operating_system_linux::compute_map;
use console_operating_system_linux::program_runtime::compute::{
  self, Map, MapProgram, MapStage, Target, Vulkan,
};
use console_operating_system_linux::program_runtime::{Clock, Interpreter, execute};
use console_program_contract::{Effect, Event, Exit};

#[derive(Clone, Copy)]
struct ImageSize {
  width: u32,
  height: u32,
}

#[derive(Clone, Copy)]
struct Mask<'pixels> {
  size: ImageSize,
  pixels: &'pixels [u32],
  cutoff: u32,
}

impl EffectDependencies for Mask<'_> {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(&[Service::Compute])
  }
}

type Effects<'pixels> = Queue<Effect<Mask<'pixels>>>;
type Request = Event<Vec<u32>>;

struct Filter<'pixels>(PhantomData<&'pixels [u32]>);

impl<'pixels> Machine for Filter<'pixels> {
  type Input = Mask<'pixels>;
  type State = Vec<u32>;
  type Request = Request;
  type Effect = Effect<Mask<'pixels>>;

  fn initialize(
    input: &Self::Input,
    _previous: Option<Self::State>,
    effects: &mut Effects<'pixels>,
  ) -> Result<Self::State, Never> {
    let Ok(()) = effects.offer(Effect::Custom(*input));
    Ok(Vec::new())
  }

  fn handle(
    state: Self::State,
    request: Request,
    effects: &mut Effects<'pixels>,
  ) -> Result<Self::State, Never> {
    match request {
      Event::Custom(mask) => {
        let Ok(()) = effects.offer(Effect::Stop(Exit::Success));
        Ok(mask)
      }
      Event::Opened
      | Event::Modified(_)
      | Event::Tick(_, _)
      | Event::Replied(_)
      | Event::Written(_)
      | Event::Chosen(_)
      | Event::Stopping => Ok(state),
    }
  }
}

struct Driver<'a, 'pixels> {
  target: Target<'a>,
  shaders: Shaders<'a>,
  pixels: PhantomData<&'pixels [u32]>,
}

#[derive(Clone, Copy)]
enum Shaders<'a> {
  Separable([Option<&'a Path>; 2]),
  Fused(&'a Path),
}

impl<'pixels> Interpreter for Driver<'_, 'pixels> {
  type Effect = Mask<'pixels>;
  type Event = Vec<u32>;
  type Error = io::Error;

  fn interpret(&mut self, effect: &Self::Effect, _clock: &Clock) -> io::Result<Vec<Request>> {
    let output = match (&self.shaders, self.target) {
      (Shaders::Separable(shaders), target) => effect.separable(target, *shaders),
      (Shaders::Fused(shader), target @ Target::RequireVulkan(_)) => {
        effect.fused(target, Some(shader))
      }
      (Shaders::Fused(shader), Target::PreferVulkan(worker)) => compute::run(
        Some(worker),
        |device| effect.fused(Target::RequireVulkan(device), Some(shader)),
        || effect.separable(Target::Cpu, [None; 2]),
      ),
      (Shaders::Fused(_), target) => effect.separable(target, [None; 2]),
    }?;
    Ok(vec![Event::Custom(output)])
  }
}

impl Mask<'_> {
  fn separable(
    &self,
    target: Target<'_>,
    [blur_shader, threshold_shader]: [Option<&Path>; 2],
  ) -> io::Result<Vec<u32>> {
    let blur = MapProgram {
      kernel: &BLUR,
      spirv: blur_shader,
    };
    compute::map_pipeline(
      target,
      Map {
        program: blur,
        input: self.pixels,
        dimensions: [self.size.width, self.size.height, 1],
        parameters: [0; 2],
      },
      &[
        MapStage {
          program: blur,
          parameters: [1, 0],
        },
        MapStage {
          program: MapProgram {
            kernel: &THRESHOLD,
            spirv: threshold_shader,
          },
          parameters: [self.cutoff, 0],
        },
      ],
    )
  }

  fn fused(&self, target: Target<'_>, shader: Option<&Path>) -> io::Result<Vec<u32>> {
    compute::map(
      target,
      Map {
        program: MapProgram {
          kernel: &MASK,
          spirv: shader,
        },
        input: self.pixels,
        dimensions: [self.size.width, self.size.height, 1],
        parameters: [self.cutoff, 0],
      },
    )
  }
}

fn fixture(ImageSize { width, height }: ImageSize) -> io::Result<Vec<u32>> {
  let length = width
    .checked_mul(height)
    .ok_or_else(|| io::Error::other("image size overflow"))?;
  let pixels = (0..length).scan(0xa5de_4321_u32, |random, _| {
    *random ^= random.wrapping_shl(13);
    *random ^= random.wrapping_shr(17);
    *random ^= random.wrapping_shl(5);
    Some(*random & 255)
  });

  Ok(pixels.collect())
}

compute_map! {
  static MASK = [8, 8, 1] |item| {
    let Ok(x): Result<u32, Never> = item.id(0);
    let Ok(y): Result<u32, Never> = item.id(1);
    let Ok(width): Result<u32, Never> = item.extent(0);
    let Ok(height): Result<u32, Never> = item.extent(1);


    let center: u32 = u32::wrapping_add(u32::wrapping_mul(y, width), x);
    let mut left: u32 = center;
    let mut right: u32 = center;
    let mut above: u32 = 0;
    let mut below: u32 = 0;

    match (x > 0).then_some(()) {
      Some(()) => { left = u32::wrapping_sub(left, 1); }
      None => {}
    }

    match (u32::wrapping_add(x, 1) < width).then_some(()) {
      Some(()) => { right = u32::wrapping_add(right, 1); }
      None => {}
    }

    match (y > 0).then_some(()) {
      Some(()) => { above = width; }
      None => {}
    }

    match (u32::wrapping_add(y, 1) < height).then_some(()) {
      Some(()) => { below = width; }
      None => {}
    }


    let Ok(top_left): Result<u32, Never> = item.word(u32::wrapping_sub(left, above));
    let Ok(top_center): Result<u32, Never> = item.word(u32::wrapping_sub(center, above));
    let Ok(top_right): Result<u32, Never> = item.word(u32::wrapping_sub(right, above));
    let top_edges: u32 = u32::wrapping_add(top_left, top_right);
    let top_sum: u32 = u32::wrapping_add(top_edges, u32::wrapping_mul(2, top_center));
    let top: u32 = u32::wrapping_shr(u32::wrapping_add(top_sum, 2), 2);
    let Ok(middle_left): Result<u32, Never> = item.word(left);
    let Ok(middle_right): Result<u32, Never> = item.word(right);
    let middle_edges: u32 = u32::wrapping_add(middle_left, middle_right);
    let Ok(middle_center): Result<u32, Never> = item.word(center);
    let middle_sum: u32 = u32::wrapping_add(middle_edges, u32::wrapping_mul(2, middle_center));
    let middle: u32 = u32::wrapping_shr(u32::wrapping_add(middle_sum, 2), 2);
    let Ok(bottom_left): Result<u32, Never> = item.word(u32::wrapping_add(left, below));
    let Ok(bottom_center): Result<u32, Never> = item.word(u32::wrapping_add(center, below));
    let Ok(bottom_right): Result<u32, Never> = item.word(u32::wrapping_add(right, below));
    let bottom_edges: u32 = u32::wrapping_add(bottom_left, bottom_right);
    let bottom_sum: u32 = u32::wrapping_add(bottom_edges, u32::wrapping_mul(2, bottom_center));
    let bottom: u32 = u32::wrapping_shr(u32::wrapping_add(bottom_sum, 2), 2);
    let middle_twice: u32 = u32::wrapping_mul(2, middle);
    let combined: u32 = u32::wrapping_add(u32::wrapping_add(top, middle_twice), bottom);
    let blurred: u32 = u32::wrapping_shr(u32::wrapping_add(combined, 2), 2);
    let mut value: u32 = 0;
    let Ok(threshold): Result<u32, Never> = item.param(0);

    match (blurred >= threshold).then_some(()) {
      Some(()) => { value = 255; }
      None => {}
    }

    value
  }
}

compute_map! {
  static BLUR = [8, 8, 1] |item| {
    let Ok(width): Result<u32, Never> = item.extent(0);
    let Ok(index): Result<u32, Never> = item.index();
    let mut stride: u32 = 1;
    let Ok(mut coordinate): Result<u32, Never> = item.id(0);
    let mut extent: u32 = width;
    let Ok(axis): Result<u32, Never> = item.param(0);

    match (axis != 0).then_some(()) {
      Some(()) => {
        let Ok(row): Result<u32, Never> = item.id(1);
        let Ok(height): Result<u32, Never> = item.extent(1);
        stride = width;
        coordinate = row;
        extent = height;
      }
      None => {}
    }

    let mut before: u32 = index;
    let mut after: u32 = index;

    match (coordinate > 0).then_some(()) {
      Some(()) => { before = u32::wrapping_sub(before, stride); }
      None => {}
    }

    match (u32::wrapping_add(coordinate, 1) < extent).then_some(()) {
      Some(()) => { after = u32::wrapping_add(after, stride); }
      None => {}
    }

    let Ok(center): Result<u32, Never> = item.word(index);
    let middle: u32 = u32::wrapping_mul(2, center);
    let Ok(previous): Result<u32, Never> = item.word(before);
    let Ok(next): Result<u32, Never> = item.word(after);
    let adjacent: u32 = u32::wrapping_add(previous, next);
    let value: u32 = u32::wrapping_add(adjacent, middle);
    u32::wrapping_shr(u32::wrapping_add(value, 2), 2)
  }
}

compute_map! {
  static THRESHOLD = [8, 8, 1] |item| {
    let Ok(at): Result<u32, Never> = item.index();
    let Ok(word): Result<u32, Never> = item.word(at);
    let Ok(threshold): Result<u32, Never> = item.param(0);
    let mut value: u32 = 0;

    match (word >= threshold).then_some(()) {
      Some(()) => { value = 255; }
      None => {}
    }

    value
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let arguments: Vec<_> = std::env::args_os().skip(1).collect();
  let worker;
  let command = arguments
    .split_first()
    .map(|(flag, rest)| (flag.to_str(), rest));
  let (target, shaders) = match command {
    None => (Target::Cpu, Shaders::Separable([None; 2])),
    Some((Some("--emit-shader"), [shader])) => {
      let generated = match shader.to_str() {
        Some("mask") => MASK.glsl(),
        Some("blur") => BLUR.glsl(),
        Some("threshold") => THRESHOLD.glsl(),
        Some(_) | None => {
          return Err(Box::from(io::Error::other(
            "expected mask, blur, or threshold",
          )));
        }
      };
      let glsl = generated?;
      io::stdout().lock().write_all(glsl.as_bytes())?;
      return Ok(());
    }
    Some((Some(flag @ ("--prefer" | "--require")), [path, paths @ ..])) => {
      let shaders = match paths {
        [mask] => Shaders::Fused(Path::new(mask)),
        [blur, threshold] => {
          Shaders::Separable([Some(Path::new(blur)), Some(Path::new(threshold))])
        }
        _ => {
          return Err(Box::from(io::Error::other(
            "expected MASK.spv or BLUR.spv THRESHOLD.spv",
          )));
        }
      };
      let Ok(device) = Vulkan::new(<::std::path::PathBuf as ::core::convert::From<
        &::std::ffi::OsString,
      >>::from(path));
      worker = device;
      let target = match flag {
        "--require" => Target::RequireVulkan(&worker),
        _ => Target::PreferVulkan(&worker),
      };
      (target, shaders)
    }
    Some(_) => {
      return Err(Box::from(io::Error::other(
        "usage: gpu-image-mask [--emit-shader KERNEL | --prefer|--require WORKER MASK.spv]",
      )));
    }
  };

  for (width, height, cutoff) in [
    (1, 1, 255),
    (1, 17, 128),
    (19, 1, 128),
    (7, 9, 0),
    (65, 33, 256),
    (1920, 1200, 128),
    (321, 207, 128),
  ] {
    let size = ImageSize { width, height };
    let pixels = fixture(size)?;
    let input = Mask {
      size,
      pixels: &pixels,
      cutoff,
    };
    let cpu = execute::<Filter, _>(
      "gpu-image-mask",
      &input,
      &mut Driver {
        target: Target::Cpu,
        shaders: Shaders::Separable([None; 2]),
        pixels: PhantomData,
      },
    )?;

    match (size, target, shaders) {
      (
        ImageSize {
          width: 65,
          height: 33,
        },
        Target::RequireVulkan(device),
        Shaders::Fused(_),
      ) => {
        let bad_shader = Path::new("/dev/null");

        match input.fused(target, Some(bad_shader)) {
          Err(fault) => {
            console_core_logging::debug!("Vulkan rejected the invalid shader: {fault}")
          }
          Ok(_) => return Err(Box::from("requiring Vulkan must report a bad shader")),
        }

        let recovered = execute::<Filter, _>(
          "gpu-image-mask",
          &input,
          &mut Driver {
            target: Target::PreferVulkan(device),
            shaders: Shaders::Fused(bad_shader),
            pixels: PhantomData,
          },
        )?;
        assert!(
          cpu == recovered,
          "a failed GPU attempt must publish the CPU result"
        );
      }
      _ => {}
    }

    let result = execute::<Filter, _>(
      "gpu-image-mask",
      &input,
      &mut Driver {
        target,
        shaders,
        pixels: PhantomData,
      },
    )?;
    assert!(cpu == result, "effect result for {width}×{height}");
    assert_eq!(result.exit, Exit::Success);
    assert_eq!(result.state.len(), pixels.len());
    writeln!(
      io::stdout().lock(),
      "{width}×{height} mask matched through the program runtime"
    )?;
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_map_pipeline_matches_the_fused_image_at_edges_and_interior() -> io::Result<()> {
    for (width, height) in [(1, 31), (29, 1), (65, 33)] {
      let size = ImageSize { width, height };
      let pixels = fixture(size)?;
      let input = Mask {
        size,
        pixels: &pixels,
        cutoff: 128,
      };
      let pipeline = input.separable(Target::Cpu, [None; 2])?;
      let fused = input.fused(Target::SerialCpu, None)?;
      assert_eq!(pipeline, fused, "{width}×{height}");
    }

    Ok(())
  }
}
