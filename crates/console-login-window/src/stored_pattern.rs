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
use std::fs::{self, Permissions};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use console_core_atomic_writes::{Unwritten, whole};
use console_core_never::Never;
use console_core_number_conversion::index;
use console_core_places::Base;

pub const FILE_NAME: &str = "login-pattern";

const YESCRYPT: &CStr = c"$y$";

const DATA: c_int = 32768;

const SETTING: c_int = 128;

const THEIRS_ALONE: u32 = 0o600;

#[link(name = "crypt")]
unsafe extern "C" {
    fn crypt_rn(phrase: *const c_char, setting: *const c_char, data: *mut c_void, size: c_int) -> *mut c_char;

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
            PatternStoreError::Reading(path, why) => write!(to, "cannot read {}: {why}", path.display()),
            PatternStoreError::NotAFile(path) => write!(to, "{} is not a plain file", path.display()),
            PatternStoreError::Hashing => write!(to, "libcrypt would not hash the pattern"),
            PatternStoreError::Writing(why) => write!(to, "{why}"),
            PatternStoreError::Closing(path, why) => write!(to, "cannot keep {} to its owner: {why}", path.display()),
            PatternStoreError::Removing(path, why) => write!(to, "cannot remove {}: {why}", path.display()),
        }
    }
}

impl std::error::Error for PatternStoreError {}

pub fn at(home: &Path) -> Result<PathBuf, Never> {
    let Ok(ours) = Base::Configuration.ours_under(home);

    Ok(ours.join(FILE_NAME))
}

pub fn stored(home: &Path) -> Result<StoredPattern, PatternStoreError> {
    let Ok(path) = at(home);

    match fs::symlink_metadata(&path) {
        Ok(found) => match found.is_file() {
            true => {}
            false => return Err(PatternStoreError::NotAFile(path)),
        },
        Err(why) => match why.kind() == io::ErrorKind::NotFound {
            true => return Ok(StoredPattern::Absent),
            false => return Err(PatternStoreError::Reading(path, why)),
        },
    }

    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(why) => return Err(PatternStoreError::Reading(path, why)),
    };
    let hash = text.trim();

    Ok(match hash.is_empty() {
        true => StoredPattern::Absent,
        false => StoredPattern::Hash(Hash(hash.to_string())),
    })
}

pub fn store(home: &Path, hash: &Hash) -> Result<(), PatternStoreError> {
    let Ok(path) = at(home);

    match path.parent() {
        Some(holding) => {
            let made = fs::create_dir_all(holding);

            made.map_err(|why| PatternStoreError::Reading(holding.to_path_buf(), why))?;
        }
        None => {}
    }

    let line = format!("{}\n", hash.0);
    let written = whole(&path, line.as_bytes());

    written.map_err(PatternStoreError::Writing)?;

    let closed = fs::set_permissions(&path, Permissions::from_mode(THEIRS_ALONE));

    closed.map_err(|why| PatternStoreError::Closing(path, why))
}

pub fn remove(home: &Path) -> Result<(), PatternStoreError> {
    let Ok(path) = at(home);

    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(why) => match why.kind() == io::ErrorKind::NotFound {
            true => Ok(()),
            false => Err(PatternStoreError::Removing(path, why)),
        },
    }
}

pub fn matches(letters: &str, hash: &Hash) -> Result<Matched, Never> {
    let setting = match CString::new(hash.0.as_str()) {
        Ok(setting) => setting,
        Err(_) => return Ok(Matched::No),
    };
    let Ok(again) = crypted(letters, &setting);

    Ok(match again {
        Some(again) => {
            let (ours, theirs) = (again.as_bytes(), hash.0.as_bytes());
            let apart = ours.iter().zip(theirs).fold(0_u8, |apart, (one, other)| apart | (one ^ other));

            match (ours.len() == theirs.len(), apart) {
                (true, 0) => Matched::Yes,
                (true | false, _) => Matched::No,
            }
        }
        None => Matched::No,
    })
}

pub fn hashed(letters: &str) -> Result<Hash, PatternStoreError> {
    let Ok(room) = index(SETTING);
    let mut setting = vec![0_u8; room];

    // SAFETY: the output is this frame's buffer and the size handed in is its
    // own; a null `random` asks libcrypt to gather the salt itself.
    let made = unsafe {
        crypt_gensalt_rn(YESCRYPT.as_ptr(), 0, std::ptr::null(), 0, setting.as_mut_ptr().cast::<c_char>(), SETTING)
    };

    match made.is_null() {
        true => return Err(PatternStoreError::Hashing),
        false => {}
    }

    // SAFETY: a NUL-ended string inside `setting`, which is still held.
    let setting = unsafe { CStr::from_ptr(made) };
    let Ok(hash) = crypted(letters, setting);

    hash.map(Hash).ok_or(PatternStoreError::Hashing)
}

fn crypted(letters: &str, setting: &CStr) -> Result<Option<String>, Never> {
    let phrase = match CString::new(letters) {
        Ok(phrase) => phrase,
        Err(_) => return Ok(None),
    };
    let Ok(room) = index(DATA);
    let mut data = vec![0_u8; room];

    // SAFETY: both strings are NUL-ended and live for the call, and the data
    // area is this frame's, as large as `struct crypt_data`, with its size.
    let made = unsafe { crypt_rn(phrase.as_ptr(), setting.as_ptr(), data.as_mut_ptr().cast::<c_void>(), DATA) };

    Ok(match made.is_null() {
        true => None,
        false => {
            // SAFETY: a NUL-ended string inside `data`, which is still held.
            Some(unsafe { CStr::from_ptr(made) }.to_string_lossy().into_owned())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(letters: &str) -> Hash {
        match hashed(letters) {
            Ok(hash) => hash,
            Err(why) => panic!("{why}"),
        }
    }

    #[test]
    fn the_pattern_that_was_kept_is_the_one_that_opens() {
        let kept = hash("gbn");

        assert!(kept.0.starts_with("$y$"));
        assert_eq!(matches("gbn", &kept), Ok(Matched::Yes));
    }

    #[test]
    fn another_pattern_does_not_open() {
        let kept = hash("gbn");

        assert_eq!(matches("gb", &kept), Ok(Matched::No));
        assert_eq!(matches("gbnm", &kept), Ok(Matched::No));
    }

    #[test]
    fn a_hash_libcrypt_cannot_read_opens_nothing() {
        assert_eq!(matches("gbn", &Hash("not a hash".to_string())), Ok(Matched::No));
    }

    #[test]
    fn two_keepings_of_one_pattern_are_salted_apart() {
        assert_ne!(hash("gbn"), hash("gbn"));
    }

    #[test]
    fn a_home_with_no_pattern_in_it_asks_for_none() {
        let home = std::env::temp_dir().join("console-login-stored-nobody");

        assert!(matches!(stored(&home), Ok(StoredPattern::Absent)));
    }

    #[test]
    fn a_kept_pattern_is_read_back_and_forgotten() {
        let home = std::env::temp_dir().join("console-login-stored-round");
        let kept_one = hash("gbn");

        assert!(store(&home, &kept_one).is_ok());
        assert!(matches!(stored(&home), Ok(StoredPattern::Hash(read)) if read == kept_one));
        assert!(remove(&home).is_ok());
        assert!(matches!(stored(&home), Ok(StoredPattern::Absent)));
    }

    #[test]
    fn a_link_where_the_pattern_goes_is_not_followed() {
        let home = std::env::temp_dir().join("console-login-stored-linked");
        let Ok(path) = at(&home);
        let _ = fs::remove_file(&path);
        let _ = fs::create_dir_all(path.parent().unwrap_or(&home));
        let _ = std::os::unix::fs::symlink("/etc/hostname", &path);

        assert!(matches!(stored(&home), Err(PatternStoreError::NotAFile(_))));
    }
}
