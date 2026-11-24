//! Builds the native renderer and embeds Calculator when given a system directory.

mod renderer;

use std::env;
use std::fs;
use std::io::{self, BufReader, ErrorKind, Read};
use std::path::{Path, PathBuf};

const PAGE_BYTES: u64 = 4096;

fn calculator_payload(system: &Path) -> io::Result<Vec<u8>> {
  let path = system.join("calculator");
  let recorded = match fs::metadata(&path) {
    Ok(metadata) => metadata,
    Err(error) => {
      let optional = error.kind() == ErrorKind::NotFound
        && fs::metadata(system).is_ok_and(|metadata| metadata.is_dir());

      return match optional.then_some(()) {
        Some(()) => Ok(Vec::new()),
        None => Err(error),
      };
    }
  };
  let regular_page = recorded.is_file() && (1..=PAGE_BYTES).contains(&recorded.len());

  match bool::then_some(regular_page, ()) {
    Some(()) => {}
    None => {
      return Err(io::Error::new(
        ErrorKind::InvalidData,
        "Calculator must be a regular file that fits in one user code page",
      ));
    }
  }

  let file = fs::File::open(path)?;
  let opened = file.metadata()?;

  match bool::then_some(opened.is_file(), ()) {
    Some(()) => {}
    None => {
      return Err(io::Error::new(
        ErrorKind::InvalidData,
        "Calculator must be a regular file",
      ));
    }
  }

  let mut bytes = Vec::new();
  let mut input = BufReader::new(file.take(PAGE_BYTES.saturating_add(1)));

  for byte in input.by_ref().bytes() {
    let read = byte?;
    bytes.push(read);
  }

  match (bytes.as_slice(), input.get_ref().limit()) {
    ([_, ..], 1..) => Ok(bytes),
    ([], _) | (_, 0) => Err(io::Error::new(
      ErrorKind::InvalidData,
      "Calculator must fit in one user code page",
    )),
  }
}

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  expect(
    explicit026_env_read_once,
    reason = "the kernel build owns CONSOLE_SYSTEM and Cargo supplies OUT_DIR, CARGO_CFG_TARGET_OS, and RUSTC"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit040_no_torn_write",
  expect(
    explicit040_no_torn_write,
    reason = "Cargo reads the generated private OUT_DIR artifact only after this build script succeeds"
  )
)]
fn main() -> io::Result<()> {
  println!("cargo:rerun-if-env-changed=CONSOLE_SYSTEM");
  println!("cargo:rustc-check-cfg=cfg(console_generation)");

  let output_directory =
    env::var_os("OUT_DIR").ok_or_else(|| io::Error::other("missing OUT_DIR"))?;
  let output = PathBuf::from(output_directory);

  match env::var("CARGO_CFG_TARGET_OS").as_deref() {
    Ok("uefi") => {
      let rustc = env::var_os("RUSTC").ok_or_else(|| io::Error::other("missing RUSTC"))?;
      let image = renderer::build(&output, &rustc)?;
      let program = renderer::program(&image)?;
      fs::write(output.join("native-renderer.bin"), program)?;
    }
    Ok(_hosted) => {}
    Err(error) => return Err(io::Error::other(error.to_string())),
  }

  let system = {
    let optional_system = env::var_os("CONSOLE_SYSTEM").map(PathBuf::from);

    match optional_system {
      Some(matched_system) => matched_system,
      ::core::option::Option::None => return Ok(()),
    }
  };

  println!("cargo:rerun-if-changed={}", system.display());

  let payload = calculator_payload(&system)?;
  fs::write(output.join("calculator.bin"), payload)?;
  println!("cargo:rustc-cfg=console_generation");
  Ok(())
}
