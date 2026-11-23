//! Every rule named in the lint workspace must load before cargo can check a crate.
//!
//! Dylint's `--all` succeeds when discovery finds nothing. Requiring each library by name makes
//! missing libraries a failed run instead, including when only part of the suite was built.

use cargo_metadata::{Metadata, MetadataCommand};
use console_core_never::Never;

const MAXIMUM_METADATA_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug)]
pub enum LibrariesFault {
  TooLarge,
  ParseMetadata(cargo_metadata::Error),
  NoRules,
}

pub fn read_metadata(output: &str) -> Result<Metadata, LibrariesFault> {
  let bytes = u64::try_from(output.len()).map_err(|_outside_u64| LibrariesFault::TooLarge)?;

  match ::core::primitive::bool::then_some(bytes <= MAXIMUM_METADATA_BYTES, ()) {
    ::core::option::Option::Some(()) => {
      MetadataCommand::parse(output).map_err(LibrariesFault::ParseMetadata)
    }
    ::core::option::Option::None => Err(LibrariesFault::TooLarge),
  }
}

pub fn libraries(metadata: &str) -> Result<Vec<String>, LibrariesFault> {
  let read = read_metadata(metadata)?;
  let mut names: Vec<String> = read
    .packages
    .into_iter()
    .map(|package| package.name.to_string())
    .filter(|name| name.starts_with("explicit"))
    .collect();

  names.sort();
  names.dedup();

  match ::core::primitive::bool::then_some(names.is_empty(), ()) {
    ::core::option::Option::Some(()) => Err(LibrariesFault::NoRules),
    ::core::option::Option::None => Ok(names),
  }
}

pub fn selection(names: &[String]) -> Result<Vec<String>, Never> {
  Ok(
    names
      .iter()
      .flat_map(|name| ["--lib".to_string(), name.clone()])
      .collect(),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  fn metadata(packages: &[&str]) -> Result<String, Never> {
    let entries = packages.iter().map(|name| format!(
            r#"{{"name":"{name}","version":"0.1.0","id":"{name}","dependencies":[],"targets":[],"features":{{}},"manifest_path":"/{name}/Cargo.toml"}}"#
        )).collect::<Vec<_>>().join(",");

    Ok(format!(
      r#"{{"version":1,"packages":[{entries}],"workspace_members":[],"workspace_root":"/","target_directory":"/target"}}"#
    ))
  }

  #[test]
  fn no_libraries_cannot_be_a_green_run() {
    let Ok(empty) = metadata(&[]);
    let Ok(collectors) = metadata(&["architecture_facts"]);

    assert!(matches!(libraries(&empty), Err(LibrariesFault::NoRules)));
    assert!(matches!(
      libraries(&collectors),
      Err(LibrariesFault::NoRules)
    ));
    assert!(matches!(
      libraries("{}"),
      Err(LibrariesFault::ParseMetadata(_))
    ));
    assert!(matches!(
      libraries("not metadata"),
      Err(LibrariesFault::ParseMetadata(_))
    ));
  }

  #[test]
  fn every_rule_is_required_by_name_and_collectors_are_left_out() -> Result<(), LibrariesFault> {
    let Ok(held) = metadata(&[
      "explicit104_no_secret_in_source",
      "architecture_facts",
      "explicit001_fallible_result",
    ]);
    let names = libraries(&held)?;
    let Ok(arguments) = selection(&names);

    assert_eq!(
      arguments,
      [
        "--lib",
        "explicit001_fallible_result",
        "--lib",
        "explicit104_no_secret_in_source"
      ]
    );

    Ok(())
  }
}
