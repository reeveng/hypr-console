use console_core_font_configuration::{Alias, Bases, DirData, Error, FontConfig};
use console_core_never::Never;
use std::path::PathBuf;

fn read_from(path: &str) -> Result<DirData, Never> {
  Ok(DirData {
    path: PathBuf::from(path),
    salt: String::new(),
  })
}

#[test]
fn a_reset_takes_back_the_directories_before_it_and_keeps_the_ones_after() -> Result<(), Error> {
  let mut merged = FontConfig::default();
  merged.merge_config("./test-conf/fonts.conf", &Bases::default())?;

  let Ok(before) = read_from("/before/reset");
  let Ok(after) = read_from("/after/reset");
  let Ok(after_after) = read_from("/after/after/reset");
  let Ok(cache) = read_from("/nix/store/i1yhgnfvaihqzs079lcx4zjnrdzcvaak-noto-fonts-2020-01-23");

  assert!(!merged.dirs.contains(&before));
  assert!(merged.dirs.contains(&after));
  assert!(merged.dirs.contains(&after_after));
  assert!(merged.dirs.contains(&cache));

  Ok(())
}

#[test]
fn an_included_alias_is_merged_with_every_family_it_prefers() -> Result<(), Error> {
  let mut merged = FontConfig::default();
  merged.merge_config("./test-conf/fonts.conf", &Bases::default())?;

  assert!(merged.aliases.contains(&Alias {
    alias: String::from("serif"),
    prefer: Vec::from([
      String::from("FreeSerif"),
      String::from("Code2000"),
      String::from("Code2001")
    ]),
    accept: Vec::new(),
    default: Vec::new(),
  }));

  Ok(())
}
