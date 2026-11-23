//! The fonts this machine has installed: where fontconfig says they are, or the usual places.
//!
//! The configuration is the file `FONTCONFIG_FILE` names when it is set.
//! Otherwise it is the person's own `fontconfig/fonts.conf` under her
//! configuration base, then the machine's `local.conf` when hers could not be
//! read, then the machine's `fonts.conf`, merged into one; a file that cannot be
//! merged is passed over and said in the log. Its aliases name what serif,
//! sans-serif, monospace, cursive and fantasy stand for -- the first family
//! preferred, else accepted, else defaulted to -- and its directories are walked
//! in order, with one record of what was visited across all of them.
//!
//! When the configuration names no directory, the usual ones are walked
//! instead: the machine's shared fonts, the local ones, and the two a person
//! keeps under her home. That is also the whole of it when this is built
//! without fontconfig, which then says nothing about falling back.
//!
//! `FONTCONFIG_FILE` is read here and nowhere else. The font configuration
//! crate reads the files it is handed and resolves them against the bases it is
//! handed, and `console-core-places` is where those bases come from, so the one
//! name left is the one that says which file decides where the fonts are, and
//! that decision is this crate's.

use std::path::Path;

use console_core_never::Never;

use crate::Database;
use crate::files::Seen;

#[cfg(feature = "fontconfig")]
use console_core_font_configuration::{Alias, Bases, DirData, FontConfig};

#[cfg(feature = "fontconfig")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Merged {
  Loaded,
  Unavailable,
}

#[cfg(feature = "fontconfig")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Configured {
  Directories,
  Absent,
}

impl Database {
  #[cfg(feature = "fontconfig")]
  pub fn load_system_fonts(&mut self) -> Result<(), Never> {
    let Ok(configured) = self.load_fontconfig();

    match configured {
      Configured::Directories => {}
      Configured::Absent => {
        console_core_logging::warn!("Fallback to loading from known font dir paths.");

        let Ok(()) = self.load_known_dirs();
      }
    }

    Ok(())
  }

  #[cfg(not(feature = "fontconfig"))]
  pub fn load_system_fonts(&mut self) -> Result<(), Never> {
    self.load_known_dirs()
  }

  fn load_known_dirs(&mut self) -> Result<(), Never> {
    let mut seen = Seen::new();
    let Ok(()) = self.load_fonts_dir_impl(Path::new("/usr/share/fonts/"), &mut seen);
    let Ok(()) = self.load_fonts_dir_impl(Path::new("/usr/local/share/fonts/"), &mut seen);
    let Ok(home) = console_core_places::home();

    match home {
      Some(home_2) => {
        let Ok(()) = self.load_fonts_dir_impl(&home_2.join(".fonts"), &mut seen);
        let Ok(()) = self.load_fonts_dir_impl(&home_2.join(".local/share/fonts"), &mut seen);
      }
      None => {}
    }

    Ok(())
  }

  #[cfg(feature = "fontconfig")]
  fn load_fontconfig(&mut self) -> Result<Configured, Never> {
    let Ok(bases) = bases();
    let mut fontconfig = FontConfig::default();
    let Ok(named) = configuration_file();

    match named {
      Some(config_file) => {
        let Ok(_merged) = merge(&mut fontconfig, &config_file, &bases);
      }
      None => {
        let Ok(own) = merge_own(&mut fontconfig, &bases);

        match own {
          Merged::Unavailable => {
            let Ok(_merged) = merge(&mut fontconfig, Path::new("/etc/fonts/local.conf"), &bases);
          }
          Merged::Loaded => {}
        }

        let Ok(_merged) = merge(&mut fontconfig, Path::new("/etc/fonts/fonts.conf"), &bases);
      }
    }

    let Ok(()) = self.apply_aliases(fontconfig.aliases);

    match fontconfig.dirs.as_slice() {
      [] => Ok(Configured::Absent),
      [_, ..] => {
        let Ok(()) = self.load_configured_dirs(fontconfig.dirs, bases.home.as_deref());

        Ok(Configured::Directories)
      }
    }
  }

  #[cfg(feature = "fontconfig")]
  fn apply_aliases(&mut self, aliases: Vec<Alias>) -> Result<(), Never> {
    for Alias {
      alias,
      default,
      prefer,
      accept,
    } in aliases
    {
      let name = prefer
        .first()
        .or_else(|| accept.first())
        .or_else(|| default.first());

      match name {
        Some(name_2) => match alias.to_lowercase().as_str() {
          "serif" => self.set_serif_family(name_2)?,
          "sans-serif" | "sans serif" => self.set_sans_serif_family(name_2)?,
          "monospace" => self.set_monospace_family(name_2)?,
          "cursive" => self.set_cursive_family(name_2)?,
          "fantasy" => self.set_fantasy_family(name_2)?,
          _ => {}
        },
        None => {}
      }
    }

    Ok(())
  }

  #[cfg(feature = "fontconfig")]
  fn load_configured_dirs(
    &mut self,
    dirs: Vec<DirData>,
    home: Option<&Path>,
  ) -> Result<(), Never> {
    let mut seen = Seen::new();

    for dir in dirs {
      #[cfg_attr(
        dylint_lib = "explicit086_no_climbing_join",
        expect(
          explicit086_no_climbing_join,
          reason = "stripping the tilde leaves a relative fontconfig path whose parent components keep their filesystem meaning"
        )
      )]
      let path = match dir.path.strip_prefix("~") {
        Ok(under_home) => match home {
          Some(home_2) => home_2.join(under_home),
          None => continue,
        },
        Err(_not_under_home) => dir.path,
      };
      let Ok(()) = self.load_fonts_dir_impl(&path, &mut seen);
    }

    Ok(())
  }
}

#[cfg(feature = "fontconfig")]
fn bases() -> Result<Bases, Never> {
  let Ok(home) = console_core_places::home();
  let Ok(configuration) = console_core_places::Base::Configuration.user();
  let Ok(share) = console_core_places::Base::Share.user();
  let Ok(cache) = console_core_places::Base::Cache.user();

  Ok(Bases {
    home,
    configuration,
    share,
    cache,
  })
}

#[cfg(feature = "fontconfig")]
#[cfg_attr(
  dylint_lib = "explicit026_env_read_once",
  expect(
    explicit026_env_read_once,
    reason = "FONTCONFIG_FILE names the fontconfig file that decides where the fonts are, and finding the fonts is this crate's"
  )
)]
fn configuration_file() -> Result<Option<std::path::PathBuf>, Never> {
  Ok(match std::env::var("FONTCONFIG_FILE") {
    Ok(named) => Some(std::path::PathBuf::from(named)),
    Err(std::env::VarError::NotPresent) => None,
    Err(fault @ std::env::VarError::NotUnicode(_)) => {
      console_core_logging::warn!("Skipping FONTCONFIG_FILE: {}", fault);

      None
    }
  })
}

#[cfg(feature = "fontconfig")]
fn merge_own(fontconfig: &mut FontConfig, bases: &Bases) -> Result<Merged, Never> {
  match &bases.configuration {
    Some(configuration) => merge(
      fontconfig,
      &configuration.join("fontconfig/fonts.conf"),
      bases,
    ),
    None => Ok(Merged::Unavailable),
  }
}

#[cfg(feature = "fontconfig")]
fn merge(fontconfig: &mut FontConfig, path: &Path, bases: &Bases) -> Result<Merged, Never> {
  Ok(match fontconfig.merge_config(path, bases) {
    Ok(()) => Merged::Loaded,
    Err(fault) => {
      console_core_logging::debug!(
        "Skipping font configuration '{}': {}",
        path.display(),
        fault
      );

      Merged::Unavailable
    }
  })
}
