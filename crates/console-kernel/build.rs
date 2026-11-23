use std::env;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::PathBuf;

fn main() -> io::Result<()> {
  println!("cargo:rerun-if-env-changed=CONSOLE_SYSTEM");
  println!("cargo:rustc-check-cfg=cfg(console_generation)");

  let (Some(directory), Some(output)) = (env::var_os("CONSOLE_SYSTEM"), env::var_os("OUT_DIR"))
  else {
    return Ok(());
  };
  let system = PathBuf::from(directory);
  let calculator = system.join("calculator");
  println!("cargo:rerun-if-changed={}", system.display());

  let payload = match fs::read(calculator) {
    Ok(bytes) if (1..=4096).contains(&bytes.len()) => bytes,
    Ok(_) => {
      return Err(io::Error::new(
        ErrorKind::InvalidData,
        "Calculator must fit in one user code page",
      ));
    }
    Err(error) if error.kind() == ErrorKind::NotFound && system.is_dir() => Vec::new(),
    Err(error) => return Err(error),
  };
  fs::write(PathBuf::from(output).join("calculator.bin"), payload)?;
  println!("cargo:rustc-cfg=console_generation");
  Ok(())
}
