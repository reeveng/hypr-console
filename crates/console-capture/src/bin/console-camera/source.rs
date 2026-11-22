use std::fs;
use std::io::{self, BufReader};
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use console_core_external_programs::Program;
use console_core_geometry::Size;
use console_core_shapes::Pixels;
use console_operating_system_linux::program_lifetime::{BoundToParent, alongside};
use rustix::fs::{Mode, OFlags, open};
use rustix::ioctl::{Getter, ioctl, opcode};

const VIDEO_CAPTURE: u32 = 0x0000_0001;
const VIDEO_CAPTURE_MPLANE: u32 = 0x0000_1000;
const STREAMING: u32 = 0x0400_0000;
const DEVICE_CAPS: u32 = 0x8000_0000;

#[repr(C)]
struct QueryCapabilities {
  driver: [u8; 16],
  card: [u8; 32],
  bus_info: [u8; 32],
  version: u32,
  capabilities: u32,
  device_caps: u32,
  reserved: [u32; 3],
}

const QUERY_CAPABILITIES: u32 = opcode::read::<QueryCapabilities>(b'V', 0);

pub(super) struct Source {
  pub path: PathBuf,
  pub label: String,
}

pub(super) fn discover() -> io::Result<Vec<Source>> {
  let mut found = Vec::new();
  let mut unavailable = None;

  for entry in fs::read_dir("/dev")? {
    let entry = entry?;
    let name = entry.file_name();
    let Some(number) = name
      .to_str()
      .and_then(|name| name.strip_prefix("video"))
      .and_then(|suffix| suffix.parse::<u32>().ok())
    else {
      continue;
    };
    if !entry.file_type()?.is_char_device() {
      continue;
    }

    let path = entry.path();
    match query(&path) {
      Ok(Some(label)) => found.push((number, Source { path, label })),
      Ok(None) => {}
      Err(fault) => {
        console_core_logging::warn!("checking camera {}: {fault}", path.display());
        unavailable = Some(fault);
      }
    }
  }

  if found.is_empty() {
    if let Some(fault) = unavailable {
      return Err(fault);
    }
  }
  found.sort_by_key(|(number, _)| *number);
  Ok(found.into_iter().map(|(_, source)| source).collect())
}

fn query(path: &Path) -> io::Result<Option<String>> {
  let fd = open(
    path,
    OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
    Mode::empty(),
  )?;
  // VIDIOC_QUERYCAP writes a v4l2_capability with the layout declared above.
  let caps = unsafe { ioctl(fd, Getter::<QUERY_CAPABILITIES, QueryCapabilities>::new()) }?;
  let flags = device_flags(&caps);
  if flags & STREAMING == 0 || flags & (VIDEO_CAPTURE | VIDEO_CAPTURE_MPLANE) == 0 {
    return Ok(None);
  }

  let label = String::from_utf8_lossy(&caps.card);
  let label = label.split('\0').next().unwrap_or("").trim();
  let node = path
    .file_name()
    .unwrap_or(path.as_os_str())
    .to_string_lossy();
  Ok(Some(if label.is_empty() {
    node.into_owned()
  } else {
    format!("{label} · {node}")
  }))
}

fn device_flags(caps: &QueryCapabilities) -> u32 {
  if caps.capabilities & DEVICE_CAPS == 0 {
    caps.capabilities
  } else {
    caps.device_caps
  }
}

const MAX_FRAME: Size<u32> = Size {
  width: 1280,
  height: 1280,
};
const FRAME_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Default)]
struct Received {
  frame: Option<Pixels>,
  fault: Option<io::Error>,
}

pub(super) struct Stream {
  _capture: BoundToParent,
  received: Arc<Mutex<Received>>,
  last_frame: Instant,
}

impl Stream {
  pub fn open(source: &Source) -> io::Result<Self> {
    let Ok(mut capture) = Program::Ffmpeg.command();
    capture
      .args([
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "video4linux2",
        "-i",
      ])
      .arg(&source.path)
      .stdin(Stdio::null())
      .stdout(Stdio::piped());
    Self::start(&mut capture)
  }

  fn start(capture: &mut std::process::Command) -> io::Result<Self> {
    capture.args([
      "-map", "0:v:0",
      "-filter_threads", "1",
      "-vf", "scale=w='min(iw,1280)':h='min(ih,1280)':force_original_aspect_ratio=decrease:flags=fast_bilinear",
      "-fps_mode", "passthrough", "-threads", "1",
      "-c:v", "ppm", "-pix_fmt", "rgb24", "-f", "image2pipe", "pipe:1",
    ]);
    let mut capture = alongside(capture)?;
    let Ok(pipe) = capture.take_stdout();
    let pipe = pipe.ok_or_else(|| io::Error::other("camera opened no frame stream"))?;
    let received = Arc::new(Mutex::new(Received::default()));
    let reader = Arc::clone(&received);

    // The child guard closes the pipe when this camera is switched or closed.
    std::thread::Builder::new()
      .name("camera-frames".to_owned())
      .spawn(move || read_frames(BufReader::new(pipe), &reader))?;

    Ok(Self {
      _capture: capture,
      received,
      last_frame: Instant::now(),
    })
  }

  pub fn frame(&mut self) -> io::Result<Option<Pixels>> {
    let mut received = self
      .received
      .lock()
      .map_err(|_| io::Error::other("camera reader failed"))?;
    if let Some(frame) = received.frame.take() {
      self.last_frame = Instant::now();
      return Ok(Some(frame));
    }
    if let Some(fault) = received.fault.take() {
      return Err(fault);
    }
    if self.last_frame.elapsed() >= FRAME_TIMEOUT {
      return Err(io::Error::new(
        io::ErrorKind::TimedOut,
        "camera did not send frames",
      ));
    }
    Ok(None)
  }
}

fn read_frames(mut pipe: impl io::BufRead, received: &Mutex<Received>) {
  loop {
    let next = console_pictures::read_portable_pixmap(&mut pipe, MAX_FRAME);
    let Ok(mut received) = received.lock() else {
      return;
    };
    match next {
      Ok(Some(frame)) => received.frame = Some(frame),
      Ok(None) => {
        received.fault = Some(io::Error::other("camera stream ended"));
        return;
      }
      Err(fault) => {
        received.fault = Some(fault);
        return;
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn capture_nodes_use_their_own_capabilities() {
    let caps = QueryCapabilities {
      driver: [0; 16],
      card: [0; 32],
      bus_info: [0; 32],
      version: 0,
      capabilities: VIDEO_CAPTURE | STREAMING | DEVICE_CAPS,
      device_caps: 0,
      reserved: [0; 3],
    };
    assert_eq!(device_flags(&caps) & VIDEO_CAPTURE, 0);
    assert_eq!(
      device_flags(&QueryCapabilities {
        capabilities: VIDEO_CAPTURE,
        ..caps
      }),
      VIDEO_CAPTURE
    );
  }

  #[test]
  fn the_reader_keeps_the_latest_complete_frame() {
    let size = Size {
      width: 2,
      height: 1,
    };
    let Ok(first) = console_pictures::portable_pixmap(size, &[5, 6, 7, 8, 9, 10]);
    let Ok(second) = console_pictures::portable_pixmap(size, &[11, 12, 13, 14, 15, 16]);
    let received = Mutex::new(Received::default());
    let input = [first, second].concat();
    read_frames(input.as_slice(), &received);
    let received = received.into_inner().expect("reader finished");
    let frame = received.frame.expect("complete frame");
    assert_eq!(frame.bytes, [11, 12, 13, 255, 14, 15, 16, 255]);
    assert!(received.fault.is_some());
  }

  #[test]
  #[ignore = "requires the desktop FFmpeg with the lavfi test source"]
  fn a_monochrome_stream_can_be_previewed_and_encoded() -> io::Result<()> {
    let Ok(mut capture) = Program::Ffmpeg.command();
    capture
      .args([
        "-nostdin",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=16x8:rate=10,format=gray",
        "-frames:v",
        "2",
      ])
      .stdout(Stdio::piped());
    let mut stream = Stream::start(&mut capture)?;
    let frame = loop {
      if let Some(frame) = stream.frame()? {
        break frame;
      }
      std::thread::sleep(Duration::from_millis(10));
    };
    let size = Size {
      width: 16,
      height: 8,
    };
    let png =
      console_core_png_files::encoded_rgba(size, &frame.bytes).map_err(io::Error::other)?;
    let decoded = console_core_png_files::decoded(&png).map_err(io::Error::other)?;
    assert_eq!(decoded.size, size);
    assert_eq!(decoded.bytes, frame.bytes);
    assert!(
      frame
        .bytes
        .chunks_exact(4)
        .all(|pixel| pixel[0] == pixel[1] && pixel[1] == pixel[2] && pixel[3] == 255)
    );
    Ok(())
  }
}
