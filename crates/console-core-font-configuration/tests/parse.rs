#![cfg(feature = "serde")]

use console_core_font_configuration::{
  Bool, ConfigPart, Include, IncludeFailure, Value, parse_config_parts,
};
use std::path::{Path, PathBuf};

#[derive(Debug)]
enum Fault {
  Unread(std::io::Error),
  Unparsed(console_core_font_configuration::Error),
  Unwritten(serde_json::Error),
  Unnamed(PathBuf),
}

impl std::fmt::Display for Fault {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Unread(fault) => write!(to, "a configuration could not be read: {fault}"),
      Self::Unparsed(fault) => write!(to, "a configuration could not be parsed: {fault}"),
      Self::Unwritten(fault) => write!(to, "the parts could not be compared as JSON: {fault}"),
      Self::Unnamed(path) => write!(to, "{} names no file", path.display()),
    }
  }
}

impl From<std::io::Error> for Fault {
  fn from(fault: std::io::Error) -> Self {
    Self::Unread(fault)
  }
}

impl From<console_core_font_configuration::Error> for Fault {
  fn from(fault: console_core_font_configuration::Error) -> Self {
    Self::Unparsed(fault)
  }
}

impl From<serde_json::Error> for Fault {
  fn from(fault: serde_json::Error) -> Self {
    Self::Unwritten(fault)
  }
}

fn read_as_written(path: &Path) -> Result<(), Fault> {
  let name = match path.file_name() {
    Some(name) => name,
    None => return Err(Fault::Unnamed(path.to_path_buf())),
  };
  let mut written = name.to_os_string();
  written.push(".json");

  let text = std::fs::read_to_string(path)?;
  let parts = parse_config_parts(text.as_str())?;
  let expected_text = std::fs::read_to_string(path.with_file_name(written))?;
  let expected: Vec<ConfigPart> = serde_json::from_str(&expected_text)?;

  assert_eq!(expected, parts, "{}", path.display());

  Ok(())
}

#[test]
fn the_whole_configuration_reads_as_it_was_written() -> Result<(), Fault> {
  read_as_written(Path::new("./test-conf/fonts.conf"))
}

#[test]
fn every_included_file_reads_as_it_was_written() -> Result<(), Fault> {
  let mut confs = Vec::new();

  let entries = std::fs::read_dir("./test-conf/conf.d")?;

  for entry in entries {
    let entry_2 = entry?;
    let path = entry_2.path();

    match path.extension().and_then(|end| end.to_str()) {
      Some("conf") => confs.push(path),
      Some(_other_extension) => {}
      None => {}
    }
  }

  confs.sort();

  assert!(!confs.is_empty());

  for conf in &confs {
    read_as_written(conf)?;
  }

  Ok(())
}

#[test]
fn boolean_values_and_include_policy_serialize_as_booleans() -> Result<(), Fault> {
  for (value, encoded) in [
    (Bool::True, serde_json::json!(true)),
    (Bool::False, serde_json::json!(false)),
  ] {
    let actual = serde_json::to_value(Value::Bool(value))?;

    assert_eq!(actual, serde_json::json!({ "Bool": encoded }));
  }

  for (policy, encoded) in [
    (IncludeFailure::Ignore, serde_json::json!(true)),
    (IncludeFailure::Report, serde_json::json!(false)),
  ] {
    let actual = serde_json::to_value(Include {
      ignore_missing: policy,
      ..Include::default()
    })?;

    assert_eq!(
      actual,
      serde_json::json!({ "prefix": "Default", "ignore_missing": encoded, "path": "" })
    );
  }

  Ok(())
}
