use std::{env, ffi::OsString, io, path::PathBuf, process::Command};

#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  expect(
    explicit026_env_read_once,
    reason = "the native plugin build owns its C compiler selection and Cargo assigns its output directory"
  )
)]
fn main() -> io::Result<()> {
  println!("cargo:rerun-if-changed=meter/gain.c");
  println!("cargo:rerun-if-env-changed=CC");
  let out = env::var_os("OUT_DIR").ok_or(io::ErrorKind::NotFound)?;
  let compiler = match env::var_os("CC") {
    Some(configured) => configured,
    None => OsString::from("cc"),
  };
  let path = PathBuf::from(out).join("console-playback-gain.so");
  let status = Command::new(compiler)
    .args([
      "-std=c11",
      "-O2",
      "-Wall",
      "-Wextra",
      "-Werror",
      "-fPIC",
      "-fvisibility=hidden",
      "-shared",
      "-o",
    ])
    .arg(path)
    .arg("meter/gain.c")
    .status()?;

  match status.success().then_some(()) {
    Some(()) => Ok(()),
    None => Err(io::Error::other("could not build the playback gain meter")),
  }
}
