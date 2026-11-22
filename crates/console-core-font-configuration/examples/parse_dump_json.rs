//! From the workspace root, run
//! `cargo run -p console-core-font-configuration --features serde --example parse_dump_json -- --fixtures`
//! to regenerate the JSON beside the included fontconfig files.

#[cfg(feature = "serde")]
mod dump {
  use console_core_font_configuration::{ConfigPart, parse_config_parts};
  use std::ffi::OsStr;
  use std::fs;
  use std::io;
  use std::path::{Path, PathBuf};

  type Failure = Box<dyn std::error::Error>;

  fn parse(path: &Path) -> Result<Vec<ConfigPart>, Failure> {
    let text = fs::read_to_string(path)?;
    Ok(parse_config_parts(&text)?)
  }

  fn regenerate(directory: &Path) -> Result<(), Failure> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory)? {
      let path = entry?.path();
      if path.extension() == Some(OsStr::new("conf")) {
        paths.push(path);
      }
    }
    paths.sort();

    for path in paths {
      let write = || -> Result<(), Failure> {
        let parts = parse(&path)?;
        let mut output = serde_json::to_vec_pretty(&parts)?;
        output.push(b'\n');
        let mut destination = path.as_os_str().to_os_string();
        destination.push(".json");
        console_core_atomic_writes::whole(Path::new(&destination), &output)?;
        Ok(())
      };
      write().map_err(|fault| format!("{}: {fault}", path.display()))?;
      println!("Processed {}", path.display());
    }

    Ok(())
  }

  pub fn run() -> Result<(), Failure> {
    let mut args = std::env::args_os().skip(1);
    let usage = "usage: parse_dump_json <conf file path> | --fixtures [directory]";
    match args.next() {
      Some(mode) if mode == "--fixtures" => {
        let path = args.next().map_or_else(
          || Path::new(env!("CARGO_MANIFEST_DIR")).join("test-conf/conf.d"),
          PathBuf::from,
        );
        if args.next().is_some() {
          return Err(usage.into());
        }
        regenerate(&path)?;
      }
      Some(path) if args.next().is_none() => {
        let parts = parse(Path::new(&path))
          .map_err(|fault| format!("{}: {fault}", Path::new(&path).display()))?;
        serde_json::to_writer(io::stdout().lock(), &parts)?;
      }
      Some(_) => return Err(usage.into()),
      None => eprintln!("{usage}"),
    }

    Ok(())
  }
}

#[cfg(feature = "serde")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
  dump::run()
}

#[cfg(not(feature = "serde"))]
fn main() {}
