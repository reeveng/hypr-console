//! Package management for per-user applications, system deployment, and development tools.

#[cfg(feature = "user")]
pub mod user {
  //! Search and transactions for an application profile using Nix.

  pub use console_package_manager_nix::{
    Activation, Installable, Profile, ProfileEntry, SearchMatch, availability, search,
  };
}

#[cfg(feature = "system")]
pub mod system {
  //! System package operations for the Arch deployment using pacman.

  pub use console_package_manager_pacman::{
    Ownership, PacmanOutput as InstallOutput, Removal, explicit, install_with, installed,
    mark_explicit, owner, remove,
  };
}

#[cfg(feature = "tools")]
pub mod tools {
  //! Versioned developer tools for a project using mise.

  pub use console_package_manager_mise::{Operation, command};
}
