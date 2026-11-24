//! Compare replies from the checksum effect against the kernel CPU reference.

use std::ffi::OsString;
use std::io::{self, Write};
use std::marker::PhantomData;
use std::path::Path;

use console_core_never::Never;
use console_core_state_machine::{Machine, Queue};
use console_kernel::compute::hosted::{CHECKSUM_KERNEL, Checksummed, Checksums, Output};
use console_kernel::compute::{BLOCK_BYTES, Block, MAX_BLOCKS, checksum_sum, checksums};
use console_operating_system_linux::program_runtime::compute::{
  MapKernel, Router, RoutingPolicy, Target, Vulkan, WRAPPING_SUM,
};
use console_operating_system_linux::program_runtime::{Clock, Interpreter, execute};
use console_program_contract::{Effect, Event, Exit};

struct ChecksumProgram<'blocks>(PhantomData<&'blocks [Block]>);

impl<'blocks> Machine for ChecksumProgram<'blocks> {
  type Input = Checksums<'blocks>;
  type State = Option<Checksummed>;
  type Request = Event<Checksummed>;
  type Effect = Effect<Checksums<'blocks>>;

  fn initialize(
    input: &Checksums<'blocks>,
    _previous: Option<Self::State>,
    effects: &mut Queue<Self::Effect>,
  ) -> Result<Self::State, Never> {
    let Ok(()) = effects.offer(Effect::Custom(*input));
    Ok(None)
  }

  fn handle(
    state: Self::State,
    request: Self::Request,
    effects: &mut Queue<Self::Effect>,
  ) -> Result<Self::State, Never> {
    match request {
      Event::Custom(reply) => {
        let Ok(()) = effects.offer(Effect::Stop(Exit::Success));
        Ok(Some(reply))
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

struct Driver<'a> {
  placement: Placement<'a>,
  shader: Option<&'a Path>,
}

enum Placement<'a> {
  Fixed(Target<'a>),
  Automatic(Router<'a>),
}

struct InterpreterScope<'call, 'device, 'blocks> {
  driver: &'call mut Driver<'device>,
  blocks: PhantomData<&'blocks [Block]>,
}

impl<'device> Driver<'device> {
  fn scoped<'blocks>(&mut self) -> Result<InterpreterScope<'_, 'device, 'blocks>, Never> {
    Ok(InterpreterScope {
      driver: self,
      blocks: PhantomData,
    })
  }
}

impl<'blocks> Interpreter for InterpreterScope<'_, '_, 'blocks> {
  type Effect = Checksums<'blocks>;
  type Event = Checksummed;
  type Error = io::Error;

  fn interpret(
    &mut self,
    effect: &Checksums<'blocks>,
    _clock: &Clock,
  ) -> io::Result<Vec<Event<Checksummed>>> {
    let shader = self.driver.shader;
    let checksum = match &mut self.driver.placement {
      Placement::Fixed(target) => effect.calculate(*target, shader)?,
      Placement::Automatic(router) => {
        let Ok(workload) = effect.workload();
        let run = router.run(workload, |target| effect.calculate(target, shader))?;

        run.value
      }
    };
    Ok(vec![Event::Custom(checksum)])
  }
}

fn input(count: u32) -> io::Result<Vec<Block>> {
  let bounded = count.min(MAX_BLOCKS);

  match bool::then_some(count == bounded, ()) {
    Some(()) => {}
    None => return Err(io::Error::other("too many checksum blocks")),
  }

  let mut seed = 0x7a11_5039_u32;
  let blocks = (0..bounded)
    .map(move |position| match position.wrapping_rem(5) {
      0 => [0; BLOCK_BYTES],
      1 => [255; BLOCK_BYTES],
      _ => {
        let mut bytes = [0; BLOCK_BYTES];

        for byte in &mut bytes {
          seed ^= seed.wrapping_shl(13);
          seed ^= seed.wrapping_shr(17);
          seed ^= seed.wrapping_shl(5);
          let [low, ..] = seed.to_le_bytes();
          *byte = low;
        }

        bytes
      }
    })
    .collect();

  Ok(blocks)
}

fn compare(driver: &mut Driver<'_>, count: u32) -> io::Result<()> {
  let blocks = input(count)?;
  let reference_error = |fault| io::Error::other(format!("{fault:?}"));
  let mut words = vec![0; blocks.len()];
  checksums(&blocks, &mut words).map_err(reference_error)?;
  let serial = words.iter().copied().fold(0, u32::wrapping_add);
  let block_results = Checksummed::Blocks(words.clone());
  let tree = checksum_sum(&blocks, &mut words).map_err(reference_error)?;

  match bool::then_some(serial == tree, ()) {
    Some(()) => {}
    None => {
      return Err(io::Error::other(format!(
        "checksum reduction mismatch for {count} blocks: serial {serial:08x}, tree {tree:08x}"
      )));
    }
  }

  for (output, expected) in [
    (Output::Blocks, block_results),
    (Output::Sum, Checksummed::Sum(tree)),
  ] {
    let request = Checksums::new(&blocks, output).map_err(reference_error)?;
    let mut cpu_driver = Driver {
      placement: Placement::Fixed(Target::Cpu),
      shader: None,
    };
    let Ok(mut cpu_scope) = cpu_driver.scoped();
    let cpu = execute::<ChecksumProgram<'_>, _>("gpu-checksums", &request, &mut cpu_scope)
      .map_err(io::Error::other)?;

    match bool::then_some(cpu.state.as_ref() == Some(&expected), ()) {
      Some(()) => {}
      None => {
        return Err(io::Error::other(format!(
          "CPU reference mismatch for {count} blocks and {output:?}"
        )));
      }
    }

    match driver.placement {
      Placement::Fixed(Target::Cpu) => {}
      Placement::Fixed(_) | Placement::Automatic(_) => {
        let Ok(mut scope) = driver.scoped();
        let result = execute::<ChecksumProgram<'_>, _>("gpu-checksums", &request, &mut scope)
          .map_err(io::Error::other)?;

        match bool::then_some(cpu == result, ()) {
          Some(()) => {}
          None => {
            return Err(io::Error::other(format!(
              "effect reply mismatch for {count} blocks and {output:?}"
            )));
          }
        }
      }
    }
  }

  writeln!(
    io::stdout().lock(),
    "Checksum effect matched the reference for {count} blocks and reduction {tree:08x}"
  )
}

enum Route {
  Require,
  Prefer,
  Automatic,
}

enum Command<'a> {
  Shader(&'static MapKernel),
  Cpu,
  Vulkan {
    worker: &'a Path,
    shader: &'a Path,
    route: Route,
  },
}

const USAGE: &str = "usage: gpu-checksums [--emit-shader [checksums|wrapping-sum] | [--prefer|--auto] WORKER CHECKSUMS.spv]";

fn command(arguments: &[OsString]) -> io::Result<Command<'_>> {
  let mode = arguments.first().and_then(|flag| flag.to_str());

  match (mode, arguments) {
    (_, []) => Ok(Command::Cpu),
    (Some("--emit-shader"), [_]) => Ok(Command::Shader(&CHECKSUM_KERNEL)),
    (Some("--emit-shader"), [_, name]) => {
      let kernel = match name.to_str() {
        Some("checksums") => &CHECKSUM_KERNEL,
        Some("wrapping-sum") => &WRAPPING_SUM,
        Some(_) | None => return Err(io::Error::other("unknown checksum shader")),
      };
      Ok(Command::Shader(kernel))
    }
    (Some("--prefer"), [_, worker, shader]) => Ok(Command::Vulkan {
      worker: Path::new(worker),
      shader: Path::new(shader),
      route: Route::Prefer,
    }),
    (Some("--auto"), [_, worker, shader]) => Ok(Command::Vulkan {
      worker: Path::new(worker),
      shader: Path::new(shader),
      route: Route::Automatic,
    }),
    (Some("--emit-shader" | "--prefer" | "--auto"), _) => Err(io::Error::other(USAGE)),
    (_, [worker, shader]) => Ok(Command::Vulkan {
      worker: Path::new(worker),
      shader: Path::new(shader),
      route: Route::Require,
    }),
    (_, [_] | [_, _, ..]) => Err(io::Error::other(USAGE)),
  }
}

fn main() -> io::Result<()> {
  let arguments: Vec<_> = std::env::args_os().skip(1).collect();
  let requested = command(&arguments)?;

  match requested {
    Command::Shader(kernel) => {
      let glsl = kernel.glsl()?;
      io::stdout().lock().write_all(glsl.as_bytes())
    }
    Command::Cpu => run(Driver {
      placement: Placement::Fixed(Target::Cpu),
      shader: None,
    }),
    Command::Vulkan {
      worker,
      shader,
      route,
    } => {
      let Ok(device) = Vulkan::new(worker.to_path_buf());
      let placement = match route {
        Route::Require => Placement::Fixed(Target::RequireVulkan(&device)),
        Route::Prefer => Placement::Fixed(Target::PreferVulkan(&device)),
        Route::Automatic => {
          let Ok(router) = Router::new(Some(&device), RoutingPolicy::default());
          Placement::Automatic(router)
        }
      };
      run(Driver {
        placement,
        shader: Some(shader),
      })
    }
  }
}

fn run(mut driver: Driver<'_>) -> io::Result<()> {
  for count in [0, 1, 2, 3, 63, 64, 65, 127, 128, 129, 4097, MAX_BLOCKS] {
    compare(&mut driver, count)?;
  }

  Ok(())
}
