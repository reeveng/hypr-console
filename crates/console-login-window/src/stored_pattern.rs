//! The pattern, kept apart from the account's password.
//!
//! The dots used to spell the account's password, and PAM checked them. That
//! made the pattern something a person had to know before they had ever drawn
//! one, and it left a machine whose password had been forgotten one fallen
//! desktop away from a greeter nobody could get past. So the pattern is its own
//! secret, chosen on the device, and a person who never chose one is never
//! asked for one.
//!
//! It is kept as a crypt(3) hash in yescrypt, the function and the format
//! `/etc/shadow` keeps, from the libxcrypt PAM already loads -- nothing about
//! it is invented here. The file is in the person's own configuration, because
//! choosing it is theirs; it is read by root, so a link there is refused rather
//! than followed.

use std::ffi::{CStr, CString, c_char, c_int, c_ulong, c_void};
use std::fs::Permissions;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use console_core_atomic_writes::{
  Ungone, Unwritten, gone, read_text, whole_with_folders_and_permissions,
};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_places::Base;

use crate::foreign;

pub const FILE_NAME: &str = "login-pattern";

pub const NOT_THE_PATTERN: &str = "that is not the pattern";

const YESCRYPT: &CStr = c"$y$";

const THEIRS_ALONE: u32 = 0o600;

#[link(name = "crypt")]
unsafe extern "C" {
  fn crypt_rn(
    phrase: *const c_char,
    setting: *const c_char,
    data: *mut c_void,
    size: c_int,
  ) -> *mut c_char;

  fn crypt_gensalt_rn(
    prefix: *const c_char,
    count: c_ulong,
    random: *const c_char,
    bytes: c_int,
    output: *mut c_char,
    size: c_int,
  ) -> *mut c_char;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredPattern {
  Absent,
  Hash(Hash),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hash(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matched {
  Yes,
  No,
}

#[derive(Debug)]
pub enum PatternStoreError {
  Reading(PathBuf, io::Error),
  NotAFile(PathBuf),
  Hashing,
  Writing(Unwritten),
  Closing(PathBuf, io::Error),
  Removing(PathBuf, io::Error),
}

impl std::fmt::Display for PatternStoreError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      PatternStoreError::Reading(path, why) => {
        write!(to, "cannot read {}: {why}", path.display())
      }
      PatternStoreError::NotAFile(path) => write!(to, "{} is not a plain file", path.display()),
      PatternStoreError::Hashing => write!(to, "libcrypt would not hash the pattern"),
      PatternStoreError::Writing(why) => write!(to, "{why}"),
      PatternStoreError::Closing(path, why) => {
        write!(to, "cannot keep {} to its owner: {why}", path.display())
      }
      PatternStoreError::Removing(path, why) => {
        write!(to, "cannot remove {}: {why}", path.display())
      }
    }
  }
}

impl std::error::Error for PatternStoreError {}

pub fn at(home: &Path) -> Result<PathBuf, Never> {
  let Ok(ours) = Base::Configuration.application_under(home);

  Ok(ours.join(FILE_NAME))
}

pub fn stored(home: &Path) -> Result<StoredPattern, PatternStoreError> {
  let Ok(path) = at(home);

  use rustix::io::Errno;

  let text = match read_text(&path) {
    Ok(text) => text,
    Err(why) => match Errno::from_io_error(&why) {
      Some(Errno::NOENT) => return Ok(StoredPattern::Absent),
      Some(Errno::LOOP | Errno::ISDIR | Errno::INVAL | Errno::NOTDIR) => {
        return Err(PatternStoreError::NotAFile(path));
      }
      Some(_) | None => return Err(PatternStoreError::Reading(path, why)),
    },
  };
  let hash = text.trim();

  Ok(
    match ::core::primitive::bool::then_some(hash.is_empty(), ()) {
      ::core::option::Option::Some(()) => StoredPattern::Absent,
      ::core::option::Option::None => StoredPattern::Hash(Hash(hash.to_string())),
    },
  )
}

pub fn store(home: &Path, hash: &Hash) -> Result<(), PatternStoreError> {
  let Ok(path) = at(home);

  let line = format!("{}\n", hash.0);

  whole_with_folders_and_permissions(&path, line.as_bytes(), Permissions::from_mode(THEIRS_ALONE))
    .map_err(PatternStoreError::Writing)
}

pub fn remove(home: &Path) -> Result<(), PatternStoreError> {
  let Ok(path) = at(home);

  gone(&path).map_err(|Ungone(path_2, why)| PatternStoreError::Removing(path_2, why))
}

pub fn matches(letters: &str, hash: &Hash) -> Result<Matched, Never> {
  let setting = match CString::new(hash.0.as_str()) {
    Ok(setting) => setting,
    Err(_holds_a_nul) => return Ok(Matched::No),
  };
  let again = match crypted(letters, &setting) {
    Ok(again) => again,
    Err(_hash_rejected) => return Ok(Matched::No),
  };
  let (ours, theirs) = (again.as_bytes(), hash.0.as_bytes());
  let apart = ours
    .iter()
    .zip(theirs)
    .fold(0_u8, |apart, (one, other)| apart | (one ^ other));

  Ok(match (ours.len() == theirs.len(), apart) {
    (true, 0) => Matched::Yes,
    (true | false, _) => Matched::No,
  })
}

pub fn hashed(letters: &str) -> Result<Hash, PatternStoreError> {
  let mut setting = [0_u8; 192];
  let Ok(size) = fitted::<_, c_int>(setting.len());

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "libxcrypt generates a yescrypt salt using system entropy through its C ABI"
    )
  )]
  // SAFETY: the output is this frame's buffer and the size handed in is its
  // own; a null `random` asks libcrypt to gather the salt itself.
  let made = unsafe {
    crypt_gensalt_rn(
      YESCRYPT.as_ptr(),
      0,
      std::ptr::null(),
      0,
      setting.as_mut_ptr().cast::<c_char>(),
      size,
    )
  };

  let salt = foreign::text_at(&setting, made).map_err(|_invalid| PatternStoreError::Hashing)?;
  crypted(letters, salt).map(Hash)
}

fn crypted(letters: &str, setting: &CStr) -> Result<String, PatternStoreError> {
  let phrase = CString::new(letters).map_err(|_holds_a_nul| PatternStoreError::Hashing)?;
  let mut data = vec![0_u8; 32_768];
  let Ok(size) = fitted::<_, c_int>(data.len());

  #[cfg_attr(
    dylint_lib = "explicit058_contain_unsafe",
    expect(
      explicit058_contain_unsafe,
      reason = "the stored crypt(3) format is implemented by libxcrypt, which takes borrowed C strings and caller-owned storage"
    )
  )]
  // SAFETY: both strings are NUL-ended and live for the call, and the data
  // area is this frame's, as large as `struct crypt_data`, with its size.
  let made = unsafe {
    crypt_rn(
      phrase.as_ptr(),
      setting.as_ptr(),
      data.as_mut_ptr().cast::<c_void>(),
      size,
    )
  };

  let encoded = foreign::text_at(&data, made).map_err(|_invalid| PatternStoreError::Hashing)?;
  let hash = encoded
    .to_str()
    .map_err(|_invalid_text| PatternStoreError::Hashing)?;

  Ok(hash.to_owned())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::error::Error;

  #[test]
  fn the_pattern_that_was_kept_is_the_one_that_opens() -> Result<(), Box<dyn Error>> {
    let kept = hashed("gbn")?;

    assert!(kept.0.starts_with("$y$"));
    assert_eq!(matches("gbn", &kept), Ok(Matched::Yes));

    Ok(())
  }

  #[test]
  fn another_pattern_does_not_open() -> Result<(), Box<dyn Error>> {
    let kept = hashed("gbn")?;

    assert_eq!(matches("gb", &kept), Ok(Matched::No));
    assert_eq!(matches("gbnm", &kept), Ok(Matched::No));

    Ok(())
  }

  #[test]
  fn a_hash_libcrypt_cannot_read_opens_nothing() {
    assert_eq!(
      matches("gbn", &Hash("not a hash".to_string())),
      Ok(Matched::No)
    );
  }

  #[test]
  fn two_keepings_of_one_pattern_are_salted_apart() -> Result<(), Box<dyn Error>> {
    let first = hashed("gbn")?;
    let second = hashed("gbn")?;

    assert_ne!(first, second);

    Ok(())
  }

  #[test]
  fn a_home_with_no_pattern_in_it_asks_for_none() -> Result<(), Box<dyn Error>> {
    let home = console_core_temporary_directories::fresh("login-stored-nobody")?;

    assert!(matches!(stored(&home), Ok(StoredPattern::Absent)));

    Ok(())
  }

  #[test]
  fn a_kept_pattern_is_read_back_and_forgotten() -> Result<(), Box<dyn Error>> {
    let home = console_core_temporary_directories::fresh("login-stored-round")?;
    let kept_one = hashed("gbn")?;

    store(&home, &kept_one)?;

    assert!(matches!(stored(&home), Ok(StoredPattern::Hash(read)) if read == kept_one));

    remove(&home)?;

    assert!(matches!(stored(&home), Ok(StoredPattern::Absent)));

    Ok(())
  }

  #[test]
  fn a_link_where_the_pattern_goes_is_not_followed() -> Result<(), Box<dyn Error>> {
    let home = console_core_temporary_directories::fresh("login-stored-linked")?;
    let Ok(path) = at(&home);
    let folder = path.parent().ok_or("the pattern's path has no folder")?;
    let parent = rustix::fs::open(
      &home,
      rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
      rustix::fs::Mode::empty(),
    )?;
    let relative = folder.strip_prefix(&home)?;
    let mut current = parent;

    for component in relative.iter() {
      rustix::fs::mkdirat(
        &current,
        component,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR | rustix::fs::Mode::XUSR,
      )?;
      let opened = rustix::fs::openat(
        &current,
        component,
        rustix::fs::OFlags::RDONLY
          | rustix::fs::OFlags::DIRECTORY
          | rustix::fs::OFlags::NOFOLLOW
          | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
      )?;
      current = opened;
    }

    rustix::fs::symlinkat("/etc/hostname", &current, FILE_NAME)?;

    assert!(matches!(stored(&home), Err(PatternStoreError::NotAFile(_))));

    Ok(())
  }
}
