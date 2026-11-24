//! Fingerprint demuxed audio packets and the information needed to decode them.

use std::fmt::Write;
use std::io::{self, ErrorKind};
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::Stdio;

use console_core_checksums::sha256::Sha256;
use console_core_external_programs::Program;
use console_core_never::Never;
use console_operating_system_linux::program_lifetime::{TimedReadPipe, alongside};

use super::super::{Request, read_pipe};

const FIELDS: &str = concat!(
  "program_version=version:library_version=name,major,minor,micro:",
  "stream=codec_name,profile,codec_tag,sample_fmt,sample_rate,channels,channel_layout,",
  "bits_per_sample,bits_per_raw_sample,extradata_hash,time_base:",
  "stream_tags=:stream_disposition=:stream_side_data=side_data_type:",
  "packet=pts,dts,duration,size,data_hash:packet_tags=:",
  "packet_side_data=side_data_type,skip_samples,discard_padding",
);

#[derive(Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct AudioKey(String);

impl AudioKey {
  pub(super) fn for_path(path: &Path) -> Result<Self, Never> {
    let Ok(digest) = console_core_checksums::sha256::of(path.as_os_str().as_bytes());
    let Ok(hash) = hex(digest);

    Ok(Self(hash))
  }

  pub(super) fn parse(text: &str) -> Result<Option<Self>, Never> {
    Ok(
      (text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| Self(text.to_ascii_lowercase())),
    )
  }

  pub(super) fn text(&self) -> Result<&str, Never> {
    Ok(&self.0)
  }
}

impl AsRef<Path> for AudioKey {
  fn as_ref(&self) -> &Path {
    Path::new(&self.0)
  }
}

pub(super) fn of(song: &Path, wanted: impl Fn() -> Request) -> io::Result<Option<AudioKey>> {
  match wanted() {
    Request::Continue => {}
    Request::Cancel => return Err(io::Error::from(ErrorKind::Interrupted)),
  }

  let Ok(mut asking) = Program::Ffprobe.command();
  asking
    .env("LC_ALL", "C")
    .args([
      "-v",
      "error",
      "-protocol_whitelist",
      "file",
      "-select_streams",
      "a:0",
      "-show_data_hash",
      "SHA256",
      "-show_entries",
      FIELDS,
      "-of",
      "compact",
      "-i",
    ])
    .arg(song)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
  let mut probe = alongside(&mut asking)?;
  let Ok(stdout) = probe.take_stdout();
  let stream = stdout.ok_or_else(|| io::Error::other("missing encoded audio report pipe"))?;
  let reader = TimedReadPipe::new(OwnedFd::from(stream))?;
  let report = read_pipe(&reader, wanted, Report::default(), Report::take)?;
  let status = probe.wait()?;

  match status.success().then_some(()) {
    Some(()) => {
      let Ok(key) = report.finish();
      Ok(key)
    }
    None => Err(io::Error::other("encoded audio inspection did not finish")),
  }
}

#[derive(Default)]
struct Report {
  digest: Sha256,
  line: Vec<u8>,
  packets: u64,
}

impl Report {
  fn take(&mut self, bytes: &[u8]) -> io::Result<()> {
    for piece in bytes.split_inclusive(|byte| *byte == b'\n') {
      match (self.line.len().saturating_add(piece.len()) <= 8192).then_some(()) {
        Some(()) => self.line.extend_from_slice(piece),
        None => return Err(io::Error::from(ErrorKind::InvalidData)),
      }

      match piece.last() {
        Some(b'\n') => {
          self.complete()?;
          self.line.clear();
        }
        Some(_) | None => {}
      }
    }

    Ok(())
  }

  fn complete(&mut self) -> io::Result<()> {
    let line = std::str::from_utf8(&self.line).map_err(io::Error::other)?;
    let mut fields = line.trim_end().split('|');
    let section = match fields.next() {
      Some(section) => section,
      None => return Err(io::Error::from(ErrorKind::InvalidData)),
    };
    let Ok(()) = self.digest.update(section.as_bytes());
    let mut has_packet = false;

    for field in fields {
      match field.split_once('=') {
        Some((name, value)) => match name.rsplit(':').next() {
          Some("side_data_type") => match value {
            "Replay Gain" | "Strings Metadata" => {}
            "Skip Samples" => {
              let Ok(()) = self.digest.update(b"|skip-samples");
            }
            _ => return Err(io::Error::from(ErrorKind::Unsupported)),
          },
          Some("data_hash") => {
            has_packet = value.starts_with("SHA256:");
            let Ok(()) = self.digest.update(b"|");
            let Ok(()) = self.digest.update(field.as_bytes());
          }
          Some(_) | None => {
            let Ok(()) = self.digest.update(b"|");
            let Ok(()) = self.digest.update(field.as_bytes());
          }
        },
        None => return Err(io::Error::from(ErrorKind::InvalidData)),
      }
    }

    match (section, has_packet) {
      ("packet", true) => self.packets = self.packets.saturating_add(1),
      ("packet", false) => return Err(io::Error::from(ErrorKind::InvalidData)),
      (_, _) => {}
    }

    let Ok(()) = self.digest.update(b"\n");
    Ok(())
  }

  #[must_use]
  fn finish(self) -> Result<Option<AudioKey>, Never> {
    match (self.line.is_empty() && self.packets > 0).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    }

    let Ok(hash) = self.digest.finalize();
    let Ok(key) = hex(hash);

    Ok(Some(AudioKey(key)))
  }
}

fn hex(hash: [u8; 32]) -> Result<String, Never> {
  let mut encoded = String::new();

  for byte in hash {
    let _written = write!(encoded, "{byte:02x}");
  }

  Ok(encoded)
}
