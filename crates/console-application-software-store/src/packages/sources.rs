//! Read packages for the Store's profile and supply its terminal for AUR installs.

use std::convert::Infallible;
use std::io;

use console_core_external_programs::{Installed, Program};
use console_package_manager::catalogue::{Catalogue, Manager, Package};
use console_package_manager::system;
use console_package_manager::user::Profile;

use super::{Local, has};

pub struct Inventory {
  pub nix: Installed,
  pub profile: io::Result<Vec<Local>>,
  pub catalogue: Catalogue,
  pub pacman_changes: Installed,
  pub terminal: Installed,
}

pub fn inventory(profile: &Profile) -> Result<Inventory, Infallible> {
  let Ok(catalogue) = Catalogue::read();
  let Ok(nix) = Manager::Nix.availability();
  let Ok(pacman_changes) = system::applications::can_change();
  let Ok(terminal) = has(Program::Alacritty);
  let listed = match nix {
    Installed::Yes => super::list(profile),
    Installed::No => Ok(Vec::new()),
  };

  Ok(Inventory {
    nix,
    profile: listed,
    catalogue,
    pacman_changes,
    terminal,
  })
}

pub fn install(package: &Package, profile: &Profile) -> io::Result<()> {
  package.source.install(profile, || {
    let mut terminal = Program::Alacritty.provide()?;
    terminal.arg("-e");

    Ok(terminal)
  })
}
