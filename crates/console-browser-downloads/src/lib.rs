//! A completed download has one directory keyed by its ID, with the display
//! filename inside it. Staging on the same filesystem allows one atomic rename.
//! Trash keeps the file and its metadata together for restore. If a download
//! directory contains extra user files, trash moves only the selected file.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File, Metadata};
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use console_core_never::Never;
use console_core_places::Folder;
use rustix::fs::{CWD, Mode, OFlags, RenameFlags, open, renameat_with};
use serde_json::value::RawValue;

const PENDING: &str = ".partial";
const DETAILS: &str = ".download.json";
const DETAILS_LIMIT: u32 = 4096;

pub fn folder() -> io::Result<PathBuf> {
  let Ok(downloads) = Folder::Downloads.user();
  let directory = downloads
    .filter(|path| path.is_absolute())
    .ok_or_else(|| io::Error::other("the Downloads folder is unavailable"))?;
  let root = directory.join("Console Browser");
  fs::create_dir_all(&root)?;
  Ok(root)
}

pub struct Staging(PathBuf);

impl Staging {
  pub fn temporary(label: &str) -> io::Result<Self> {
    console_core_temporary_directories::private(label)
      .map(Self)
      .map_err(io::Error::other)
  }

  pub fn new(root: &Path) -> io::Result<Self> {
    let pending = root.join(PENDING);

    match fs::DirBuilder::new().mode(0o700).create(&pending) {
      Ok(()) => {}
      Err(fault) => match (fault.kind() == io::ErrorKind::AlreadyExists).then_some(()) {
        Some(()) => {
          let metadata = pending.symlink_metadata()?;

          match metadata.is_dir().then_some(()) {
            Some(()) => {}
            None => return Err(fault),
          }
        }
        None => return Err(fault),
      },
    }

    Self::beneath(&pending, "browser-downloads")
  }

  fn beneath(parent: &Path, label: &str) -> io::Result<Self> {
    console_core_temporary_directories::private_under(parent, label)
      .map(Self)
      .map_err(io::Error::other)
  }
}

impl AsRef<Path> for Staging {
  fn as_ref(&self) -> &Path {
    &self.0
  }
}

impl Drop for Staging {
  fn drop(&mut self) {
    match fs::remove_dir_all(&self.0) {
      Ok(()) => {}
      Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
        Some(()) => {}
        None => console_core_logging::warn!(
          "could not clean browser downloads at {}: {fault}",
          self.0.display()
        ),
      },
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(String);

#[derive(Debug)]
pub struct InvalidId;

impl std::fmt::Display for InvalidId {
  fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    out.write_str("invalid download ID")
  }
}

impl std::error::Error for InvalidId {}

impl Id {
  pub fn parse(guid: &str) -> Result<Self, InvalidId> {
    (guid.len() == 36
      && guid.bytes().enumerate().all(|(index, byte)| match index {
        8 | 13 | 18 | 23 => byte == b'-',
        _ => byte.is_ascii_hexdigit(),
      }))
    .then(|| Self(guid.to_owned()))
    .ok_or(InvalidId)
  }

  pub fn as_str(&self) -> Result<&str, Never> {
    Ok(&self.0)
  }
}

impl AsRef<Path> for Id {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

pub struct Name(String);

impl Name {
  pub fn suggested(suggested: &str) -> Result<Self, Never> {
    let basename = match suggested.rsplit(['/', '\\']).next() {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => "",
    }
    .trim();
    let mut cleaned: String = basename
      .trim_start_matches('.')
      .chars()
      .map(|letter| match letter {
        '\u{061c}'
        | '\u{200e}'
        | '\u{200f}'
        | '\u{202a}'..='\u{202e}'
        | '\u{2066}'..='\u{2069}' => '_',
        other => match other.is_control().then_some(()) {
          Some(()) => '_',
          None => other,
        },
      })
      .collect();
    cleaned.truncate(cleaned.floor_char_boundary(240));

    Ok(Self(match cleaned.trim() {
      "" => "download".to_owned(),
      _ => cleaned,
    }))
  }

  pub fn as_str(&self) -> Result<&str, Never> {
    Ok(&self.0)
  }
}

impl AsRef<Path> for Name {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

pub fn source_site(url: &str) -> Result<Option<&str>, Never> {
  let address = match url.strip_prefix("blob:") {
    Some(origin) => origin,
    None => url,
  };
  let without_scheme = address
    .strip_prefix("https://")
    .or_else(|| address.strip_prefix("http://"));
  let host_and_port = without_scheme
    .and_then(|part| part.split(['/', '?', '#']).next())
    .and_then(|authority| authority.rsplit('@').next());
  let host = host_and_port.and_then(|part| match part.strip_prefix('[') {
    Some(ipv6) => ipv6.split_once(']').map(|(host, _port)| host),
    None => part.split(':').next(),
  });

  match host {
    Some(domain) => site_name(domain),
    None => Ok(None),
  }
}

fn site_name(host: &str) -> Result<Option<&str>, Never> {
  let valid = !host.is_empty()
    && host.len() <= 253
    && host
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':'));

  Ok(valid.then_some(host))
}

#[derive(Default)]
struct Details {
  name: Option<String>,
  site: Option<String>,
  saved_at: Option<i64>,
}

impl Details {
  fn read(directory: &Path) -> io::Result<Self> {
    let fd = match open(
      directory.join(DETAILS),
      OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
      Mode::empty(),
    ) {
      Ok(fd) => fd,
      Err(rustix::io::Errno::NOENT) => return Ok(Self::default()),
      Err(fault) => return Err(io::Error::from(fault)),
    };
    let file = File::from(fd);

    match {
      let metadata = file.metadata()?;

      ::core::primitive::bool::then_some(!metadata.is_file(), ())
    } {
      ::core::option::Option::Some(()) => {
        return Err(io::Error::other("download metadata is not a regular file"));
      }
      ::core::option::Option::None => {}
    }

    let bytes = console_core_bounded_reads::bytes(file, DETAILS_LIMIT)?;
    Self::decode(&bytes)
  }

  fn decode(bytes: &[u8]) -> io::Result<Self> {
    let fields: BTreeMap<String, &RawValue> = serde_json::from_slice(bytes)?;
    let mut details = Self::default();

    for (field, value) in fields {
      match field.as_str() {
        "name" => {
          let name = serde_json::from_str(value.get())?;
          details.name = name;
        }
        "site" => {
          let site: Option<String> = serde_json::from_str(value.get())?;

          details.site = site.filter(|hostname| {
            let Ok(valid) = site_name(hostname);
            valid.is_some()
          });
        }
        "saved_at" => {
          let saved_at = serde_json::from_str(value.get())?;
          details.saved_at = saved_at;
        }
        _ => {
          return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown download metadata field {field:?}"),
          ));
        }
      }
    }

    Ok(details)
  }
}

pub struct Stored {
  pub filename: OsString,
  pub saved_at: Option<i64>,
  pub site: Option<String>,
  path: PathBuf,
}

pub fn stored(root: &Path, id: &Id) -> io::Result<Option<Stored>> {
  let directory = root.join(id);
  let directory_metadata = directory.symlink_metadata()?;

  match directory_metadata.is_dir().then_some(()) {
    Some(()) => {}
    None => return Ok(None),
  }

  let details = match Details::read(&directory) {
    Ok(details) => details,
    Err(fault) => {
      let Ok(id_text) = id.as_str();
      console_core_logging::debug!("reading download metadata for {id_text}: {fault}");

      Details::default()
    }
  };

  let found = stored_file(&directory, details.name.as_deref())?;
  let DownloadFile {
    filename,
    path,
    metadata,
  } = match found {
    Some(file) => file,
    None => return Ok(None),
  };
  let saved_at = details.saved_at.or_else(|| match metadata.modified() {
    Ok(modified) => {
      let Ok(seconds) = unix_seconds(modified);
      seconds
    }
    Err(fault) => {
      let Ok(id_text) = id.as_str();
      console_core_logging::debug!("reading modification time for {id_text}: {fault}");

      None
    }
  });

  Ok(Some(Stored {
    filename,
    saved_at,
    site: details.site,
    path,
  }))
}

struct DownloadFile {
  filename: OsString,
  metadata: Metadata,
  path: PathBuf,
}

struct MetadataName<'a>(&'a str);

impl AsRef<Path> for MetadataName<'_> {
  fn as_ref(&self) -> &Path {
    Path::new(self.0)
  }
}

fn stored_file(directory: &Path, known_name: Option<&str>) -> io::Result<Option<DownloadFile>> {
  let validated = known_name
    .filter(|name| !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\0']))
    .map(MetadataName);

  match validated {
    Some(name) => {
      let path = directory.join(&name);

      match path.symlink_metadata() {
        Ok(metadata) => match metadata.is_file().then_some(()) {
          Some(()) => {
            return Ok(Some(DownloadFile {
              filename: OsString::from(name.0),
              metadata,
              path,
            }));
          }
          None => {}
        },
        Err(fault) => match (fault.kind() == io::ErrorKind::NotFound).then_some(()) {
          Some(()) => {}
          None => return Err(fault),
        },
      }
    }
    None => {}
  }

  let mut only = None;

  let dir = fs::read_dir(directory)?;

  for result in dir {
    let entry = result?;

    match entry.file_name().as_encoded_bytes().first() {
      Some(b'.') => continue,
      Some(_) | None => {}
    }

    let file_type = entry.file_type()?;

    match file_type.is_file().then_some(()) {
      Some(()) => {}
      None => continue,
    }

    match only {
      ::core::option::Option::Some(_) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    only = Some(entry);
  }

  match only {
    Some(entry) => {
      let metadata = entry.metadata()?;

      Ok(Some(DownloadFile {
        filename: entry.file_name(),
        metadata,
        path: entry.path(),
      }))
    }
    None => Ok(None),
  }
}

pub fn locate(root: &Path, id: &Id) -> io::Result<PathBuf> {
  let found = stored(root, id)?;
  let item = found.ok_or_else(|| {
    io::Error::new(
      io::ErrorKind::NotFound,
      "Download has moved or been removed",
    )
  })?;
  Ok(item.path)
}

pub fn trash_target(root: &Path, id: &Id) -> io::Result<PathBuf> {
  let file = locate(root, id)?;
  let directory = root.join(id);

  let mut found = false;

  let dir = fs::read_dir(&directory)?;

  for entry in dir {
    let entry_2 = entry?;
    let name = entry_2.file_name();

    match { ::core::primitive::bool::then_some(Some(name.as_os_str()) == file.file_name(), ()) } {
      ::core::option::Option::Some(()) => {
        match {
          let file_type = entry_2.file_type()?;

          ::core::primitive::bool::then_some(!file_type.is_file(), ())
        } {
          ::core::option::Option::Some(()) => {
            break;
          }
          ::core::option::Option::None => {}
        }

        found = true;
        continue;
      }
      ::core::option::Option::None => {}
    }

    match (name == DETAILS).then_some(()) {
      Some(()) => {
        let file_type = entry_2.file_type()?;

        match file_type.is_file().then_some(()) {
          Some(()) => {}
          None => return Ok(file),
        }
      }
      None => return Ok(file),
    }
  }

  match ::core::primitive::bool::then_some(found, ()) {
    ::core::option::Option::Some(()) => Ok(directory),
    ::core::option::Option::None => Err(io::Error::new(
      io::ErrorKind::NotFound,
      "Download has moved or been removed",
    )),
  }
}

fn unix_seconds(time: SystemTime) -> Result<Option<i64>, Never> {
  Ok(match time.duration_since(UNIX_EPOCH) {
    Ok(after) => match i64::try_from(after.as_secs()) {
      ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_),
      ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None,
    },
    Err(before) => {
      let span = before.duration();

      match i64::try_from(span.as_secs()) {
        Ok(seconds) => seconds
          .checked_neg()
          .and_then(|whole| whole.checked_sub(i64::from(span.subsec_nanos() != 0))),
        Err(_outside_integer_range) => None,
      }
    }
  })
}

fn write_details(
  directory: &Path,
  name: &Name,
  site: Option<&str>,
  completed_at: SystemTime,
) -> io::Result<()> {
  let Ok(filename) = name.as_str();
  let Ok(saved_at) = unix_seconds(completed_at);
  let data = serde_json::to_vec(&serde_json::json!({
    "name": filename,
    "site": site,
    "saved_at": saved_at,
  }))?;
  let path = directory.join(DETAILS);

  console_core_atomic_writes::whole_without_overwriting(&path, &data).map_err(io::Error::other)
}

pub fn publish(
  staging: &Path,
  id: &Id,
  name: &Name,
  site: Option<&str>,
  completed_at: SystemTime,
) -> io::Result<()> {
  let root = staging
    .parent()
    .and_then(Path::parent)
    .ok_or_else(|| io::Error::other("the Downloads folder is unavailable"))?;
  let source = staging.join(id);
  let fd = open(
    &source,
    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
    Mode::empty(),
  )?;
  let file = File::from(fd);

  let metadata = file.metadata()?;

  match ::core::primitive::bool::then_some(metadata.is_file(), ()) {
    ::core::option::Option::Some(()) => file.sync_all()?,
    ::core::option::Option::None => {
      return Err(io::Error::other("Chromium download data is not a file"));
    }
  }

  let completed = Staging::beneath(staging, "completed")?;
  fs::rename(&source, completed.0.join(name))?;

  match write_details(&completed.0, name, site, completed_at) {
    Ok(()) => {}
    Err(fault) => {
      let Ok(id_text) = id.as_str();
      console_core_logging::warn!("could not store download metadata for {id_text}: {fault}");

      let directory = File::open(&completed.0)?;

      directory.sync_all()?;
    }
  }

  renameat_with(
    CWD,
    &completed.0,
    CWD,
    root.join(id),
    RenameFlags::NOREPLACE,
  )?;
  let directory = File::open(root)?;

  directory.sync_all()
}

#[cfg(test)]
mod tests {
  use super::*;

  fn fixture(path: &Path, bytes: &[u8]) -> io::Result<()> {
    console_core_atomic_writes::whole_without_overwriting(path, bytes).map_err(io::Error::other)
  }

  #[test]
  fn trash_keeps_the_download_bundle_together_and_leaves_unrelated_files_alone() -> io::Result<()>
  {
    let root = Staging::temporary("download-trash-target")?;
    let id = Id::parse("456789ab-cdef-4567-abcd-0123456789ab").map_err(io::Error::other)?;
    let directory = root.0.join(&id);
    fs::create_dir(&directory)?;
    let Ok(name) = Name::suggested("report.txt");
    let file = directory.join(&name);
    fixture(&file, b"report")?;
    write_details(&directory, &name, Some("example.org"), UNIX_EPOCH)?;
    let bundled = trash_target(&root.0, &id)?;
    assert_eq!(bundled, directory);

    fixture(&directory.join("personal notes.txt"), b"keep")?;
    let individual = trash_target(&root.0, &id)?;
    assert_eq!(individual, file);
    Ok(())
  }

  #[test]
  fn the_source_site_keeps_the_host_without_credentials_or_url_parameters() {
    for (url, host) in [
      (
        "https://user:secret@example.org:443/download?q=token#part",
        Some("example.org"),
      ),
      (
        "blob:https://downloads.example.org/id",
        Some("downloads.example.org"),
      ),
      ("http://[::1]:4500/download", Some("::1")),
      ("file:///home/me/file.txt", None),
      ("data:text/plain,hello", None),
    ] {
      assert_eq!(source_site(url), Ok(host));
    }
  }

  #[test]
  fn site_names_cannot_choose_a_path_or_hide_the_download() {
    for (suggested, expected) in [
      ("../report.txt", "report.txt"),
      ("folder\\report.txt", "report.txt"),
      ("..", "download"),
      (".hidden", "hidden"),
      ("bad\u{202e}\n.txt", "bad__.txt"),
    ] {
      let Ok(name) = Name::suggested(suggested);
      assert_eq!(name.as_str(), Ok(expected));
    }

    let Ok(name) = Name::suggested(&"é".repeat(150));
    let Ok(name_text) = name.as_str();
    assert_eq!(name_text.len(), 240);
    assert!(matches!(Id::parse("../outside"), Err(InvalidId)));
  }

  #[test]
  fn an_invalid_metadata_name_uses_the_single_file_in_the_bundle() -> io::Result<()> {
    let root = Staging::temporary("download-metadata-file")?;
    let id = Id::parse("456789ab-cdef-4567-abcd-0123456789ab").map_err(io::Error::other)?;
    let directory = root.as_ref().join(&id);
    fs::create_dir(&directory)?;
    fixture(&root.as_ref().join("outside.txt"), b"outside")?;
    let file = directory.join("report.txt");
    fixture(&file, b"report")?;
    fixture(&directory.join(DETAILS), br#"{"name":"../outside.txt"}"#)?;

    let located = locate(root.as_ref(), &id)?;
    assert_eq!(located, file);

    Ok(())
  }

  #[test]
  fn metadata_keeps_the_existing_fields_and_rejects_other_shapes() -> io::Result<()> {
    let details = Details::decode(br#"{"name":"notes.txt","site":null,"saved_at":12}"#)?;
    assert_eq!(details.name.as_deref(), Some("notes.txt"));
    assert_eq!(details.site, None);
    assert_eq!(details.saved_at, Some(12));

    for invalid in [
      br#"{"name":10}"#.as_slice(),
      br#"{"saved_at":"12"}"#.as_slice(),
      br#"{"extra":true}"#.as_slice(),
    ] {
      let decoded = Details::decode(invalid);
      assert!(matches!(decoded, Err(fault) if fault.kind() == io::ErrorKind::InvalidData));
    }

    Ok(())
  }

  #[test]
  fn publishing_a_known_id_twice_preserves_the_first_file() -> io::Result<()> {
    let root = Staging::temporary("browser-downloads-test")?;
    let staging = Staging::new(&root.0)?;
    let id = Id::parse("456789ab-cdef-4567-abcd-0123456789ab").map_err(io::Error::other)?;
    let Ok(name) = Name::suggested("report.txt");
    let source = staging.0.join(&id);
    assert!(matches!(
      publish(&staging.0, &id, &name, None, UNIX_EPOCH),
      Err(fault) if fault.kind() == io::ErrorKind::NotFound
    ));
    let published = root.0.join(&id);
    assert!(matches!(
      published.symlink_metadata(),
      Err(fault) if fault.kind() == io::ErrorKind::NotFound
    ));

    fixture(&source, b"first")?;
    publish(&staging.0, &id, &name, Some("example.org"), UNIX_EPOCH)?;
    fixture(&source, b"second")?;
    assert!(matches!(
      publish(&staging.0, &id, &name, None, UNIX_EPOCH),
      Err(fault) if fault.kind() == io::ErrorKind::AlreadyExists
    ));
    let read = fs::read(published.join(&name))?;
    assert_eq!(read, b"first");
    let metadata = Details::read(&published)?;
    assert_eq!(metadata.site.as_deref(), Some("example.org"));
    assert_eq!(metadata.name.as_deref(), Some("report.txt"));
    assert_eq!(metadata.saved_at, Some(0));
    Ok(())
  }
}
