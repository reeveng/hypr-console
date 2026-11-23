//! Where a directory or file a configuration names really is, resolved against the bases it was handed.
//!
//! A path that begins with `~` is under the home. Then an absolute path is
//! itself, and a relative one is resolved by its prefix: `xdg` puts it under
//! the base directory its element belongs to, `relative` beside the file that
//! named it, `cwd` under the working directory, and `default` is whichever of
//! those its element uses when nothing is said -- the machine's `/etc/fonts` for
//! an include, the working directory for the rest. These are fontconfig's own
//! rules, and fontconfig-parser's.
//!
//! The home and the bases are handed in rather than read here, so this crate
//! reads no variable. A base the caller does not know is the usual directory
//! under the home, and a home the caller does not know is the root directory,
//! which is what fontconfig-parser answered with no `HOME` set: a path under
//! it names a directory that is not there rather than refusing the file.

use std::path::{Path, PathBuf};

use console_core_never::Never;

const WITHOUT_A_HOME: &str = "/";
const MACHINE_CONFIGURATION: &str = "/etc/fonts";
const WORKING_DIRECTORY: &str = ".";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dir {
  pub prefix: DirPrefix,
  pub salt: String,
  pub path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CacheDir {
  pub prefix: DirPrefix,
  pub path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Include {
  pub prefix: DirPrefix,
  pub ignore_missing: IncludeFailure,
  pub path: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(from = "bool", into = "bool"))]
pub enum IncludeFailure {
  Ignore,
  #[default]
  Report,
}

#[cfg(feature = "serde")]
impl From<bool> for IncludeFailure {
  fn from(ignore: bool) -> Self {
    match bool::then_some(ignore, ()) {
      Some(()) => Self::Ignore,
      None => Self::Report,
    }
  }
}

#[cfg(feature = "serde")]
impl From<IncludeFailure> for bool {
  fn from(policy: IncludeFailure) -> Self {
    match policy {
      IncludeFailure::Ignore => true,
      IncludeFailure::Report => false,
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RemapDir {
  pub prefix: DirPrefix,
  pub as_path: String,
  pub salt: String,
  pub path: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DirPrefix {
  #[default]
  Default,
  Cwd,
  Xdg,
  Relative,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefixBehavior {
  Configuration,
  Cwd,
  Xdg,
  Relative,
}

parse_enum! {
    DirPrefix,
    (Default, "default"),
    (Cwd, "cwd"),
    (Xdg, "xdg"),
    (Relative, "relative"),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bases {
  pub home: Option<PathBuf>,
  pub configuration: Option<PathBuf>,
  pub share: Option<PathBuf>,
  pub cache: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Base {
  Configuration,
  Share,
  Cache,
}

impl Bases {
  fn home(&self) -> Result<&Path, Never> {
    Ok(match &self.home {
      Some(home) => home,
      None => Path::new(WITHOUT_A_HOME),
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit086_no_climbing_join",
    expect(
      explicit086_no_climbing_join,
      reason = "the fallback joins one of the three literal XDG defaults under the home directory"
    )
  )]
  fn base(&self, base: Base) -> Result<PathBuf, Never> {
    let (said, usual) = match base {
      Base::Configuration => (&self.configuration, ".config"),
      Base::Share => (&self.share, ".local/share"),
      Base::Cache => (&self.cache, ".cache"),
    };
    let Ok(home) = self.home();

    Ok(match said {
      Some(said_2) => said_2.clone(),
      None => home.join(usual),
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit086_no_climbing_join",
    expect(
      explicit086_no_climbing_join,
      reason = "after stripping the leading tilde component, the remaining path is relative and parent components retain their filesystem meaning"
    )
  )]
  fn expand_tilde(&self, path: &str) -> Result<PathBuf, Never> {
    let named = Path::new(path);
    let Ok(home) = self.home();

    Ok(match named.strip_prefix("~") {
      Ok(under_home) => home.join(under_home),
      Err(_not_under_home) => named.to_path_buf(),
    })
  }
}

struct Named<'a> {
  path: &'a str,
  prefix: DirPrefix,
  unsaid: PrefixBehavior,
  base: Base,
}

enum PathAnchor {
  Absolute,
  Relative,
}

impl From<bool> for PathAnchor {
  fn from(absolute: bool) -> Self {
    match bool::then_some(absolute, ()) {
      Some(()) => Self::Absolute,
      None => Self::Relative,
    }
  }
}

impl Named<'_> {
  #[cfg_attr(
    dylint_lib = "explicit086_no_climbing_join",
    expect(
      explicit086_no_climbing_join,
      reason = "only paths that passed the absolute-path check are joined, and fontconfig paths may intentionally name files above the prefix"
    )
  )]
  fn resolve(&self, config_file_path: &Path, bases: &Bases) -> Result<PathBuf, Never> {
    let Ok(expanded) = bases.expand_tilde(self.path);

    match PathAnchor::from(expanded.is_absolute()) {
      PathAnchor::Absolute => return Ok(expanded),
      PathAnchor::Relative => {}
    }

    let behavior = match self.prefix {
      DirPrefix::Default => self.unsaid,
      DirPrefix::Cwd => PrefixBehavior::Cwd,
      DirPrefix::Xdg => PrefixBehavior::Xdg,
      DirPrefix::Relative => PrefixBehavior::Relative,
    };

    Ok(match behavior {
      PrefixBehavior::Configuration => Path::new(MACHINE_CONFIGURATION).join(expanded),
      PrefixBehavior::Cwd => Path::new(WORKING_DIRECTORY).join(expanded),
      PrefixBehavior::Relative => match config_file_path.parent() {
        Some(beside) => beside.join(expanded),
        None => Path::new(WORKING_DIRECTORY).join(expanded),
      },
      PrefixBehavior::Xdg => {
        let Ok(base) = bases.base(self.base);

        base.join(expanded)
      }
    })
  }
}

impl Dir {
  pub fn calculate_path<P: AsRef<Path> + ?Sized>(
    &self,
    config_file_path: &P,
    bases: &Bases,
  ) -> Result<PathBuf, Never> {
    let named = Named {
      path: &self.path,
      prefix: self.prefix,
      unsaid: PrefixBehavior::Cwd,
      base: Base::Share,
    };

    named.resolve(config_file_path.as_ref(), bases)
  }
}

impl CacheDir {
  pub fn calculate_path<P: AsRef<Path> + ?Sized>(
    &self,
    config_file_path: &P,
    bases: &Bases,
  ) -> Result<PathBuf, Never> {
    let named = Named {
      path: &self.path,
      prefix: self.prefix,
      unsaid: PrefixBehavior::Cwd,
      base: Base::Cache,
    };

    named.resolve(config_file_path.as_ref(), bases)
  }
}

impl Include {
  pub fn calculate_path<P: AsRef<Path> + ?Sized>(
    &self,
    config_file_path: &P,
    bases: &Bases,
  ) -> Result<PathBuf, Never> {
    let named = Named {
      path: &self.path,
      prefix: self.prefix,
      unsaid: PrefixBehavior::Configuration,
      base: Base::Configuration,
    };

    named.resolve(config_file_path.as_ref(), bases)
  }
}

impl RemapDir {
  pub fn calculate_path<P: AsRef<Path> + ?Sized>(
    &self,
    config_file_path: &P,
    bases: &Bases,
  ) -> Result<PathBuf, Never> {
    let named = Named {
      path: &self.path,
      prefix: self.prefix,
      unsaid: PrefixBehavior::Cwd,
      base: Base::Configuration,
    };

    named.resolve(config_file_path.as_ref(), bases)
  }
}
