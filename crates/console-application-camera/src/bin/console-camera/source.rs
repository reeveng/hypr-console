use std::convert::Infallible;
use std::fs;
use std::io::{self, BufReader};
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use console_actor::{Actor, Scope, boot};
use console_core_external_programs::Program;
use console_core_geometry::Size;
use console_core_iteration::{Step, iterate};
use console_core_state_machine::{EffectDependencies, Machine, Never, Queue, Service};
use console_operating_system_linux::program_lifetime::{BoundToParent, alongside};
use console_panel::ui::shapes::Pixels;
use console_program_lifetime::threads::let_go;
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

  let devices = fs::read_dir("/dev")?;

  for entry in devices {
    let entry_2 = entry?;
    let name = entry_2.file_name();

    let node = match name.to_str() {
      Some(node) => node,
      None => continue,
    };
    let suffix = match node.strip_prefix("video") {
      Some(suffix) => suffix,
      None => continue,
    };
    let number = match suffix.parse::<u32>() {
      Ok(number) => number,
      Err(_invalid_video_number) => continue,
    };

    let file_type = entry_2.file_type()?;

    match (!file_type.is_char_device()).then_some(()) {
      ::core::option::Option::Some(()) => {
        continue;
      }
      ::core::option::Option::None => {}
    }

    let path = entry_2.path();

    match query(&path, node) {
      Ok(Some(label)) => found.push((number, Source { path, label })),
      Ok(None) => {}
      Err(fault) => {
        console_core_logging::warn!("checking camera {}: {fault}", path.display());
        unavailable = Some(fault);
      }
    }
  }

  match { ::core::primitive::bool::then_some(found.is_empty(), ()) } {
    ::core::option::Option::Some(()) => match unavailable {
      Some(fault) => {
        return Err(fault);
      }
      ::core::option::Option::None => {}
    },
    ::core::option::Option::None => {}
  }

  found.sort_by_key(|(number, _)| *number);
  Ok(found.into_iter().map(|(_, source)| source).collect())
}

#[cfg_attr(
  dylint_lib = "explicit058_contain_unsafe",
  expect(
    explicit058_contain_unsafe,
    reason = "the camera boundary queries a V4L2 device using the kernel's repr(C) capability layout"
  )
)]
fn query(path: &Path, node: &str) -> io::Result<Option<String>> {
  let fd = open(
    path,
    OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
    Mode::empty(),
  )?;
  // SAFETY: VIDIOC_QUERYCAP initializes the full v4l2_capability represented by QueryCapabilities.
  let caps = unsafe { ioctl(fd, Getter::<QUERY_CAPABILITIES, QueryCapabilities>::new()) }?;
  let Ok(flags) = device_flags(&caps);

  match {
    ::core::primitive::bool::then_some(
      flags & STREAMING == 0 || flags & (VIDEO_CAPTURE | VIDEO_CAPTURE_MPLANE) == 0,
      (),
    )
  } {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let card = match caps.card.split(|byte| *byte == 0).next() {
    Some(card) => card,
    None => return Ok(Some(node.to_owned())),
  };
  let label = match std::str::from_utf8(card) {
    Ok(card_text) => match card_text.trim() {
      "" => node.to_owned(),
      label => format!("{label} · {node}"),
    },
    Err(_card_is_not_text) => node.to_owned(),
  };
  Ok(Some(label))
}

fn device_flags(caps: &QueryCapabilities) -> Result<u32, Infallible> {
  Ok(
    match { ::core::primitive::bool::then_some(caps.capabilities & DEVICE_CAPS == 0, ()) } {
      ::core::option::Option::Some(()) => caps.capabilities,
      ::core::option::Option::None => caps.device_caps,
    },
  )
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

enum CameraRequest {
  Offer(Pixels),
  End(io::Error),
  Take,
}

#[derive(Debug)]
enum Receipt {
  Frame(Pixels),
  Fault(io::Error),
}

impl EffectDependencies for Receipt {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(&[])
  }
}

struct Inbox;

impl Machine for Inbox {
  type Input = ();
  type State = Received;
  type Request = CameraRequest;
  type Effect = Receipt;

  fn initialize(_: &(), _: Option<Received>, _: &mut Queue<Receipt>) -> Result<Received, Never> {
    Ok(Received::default())
  }

  fn handle(
    mut state: Received,
    request: CameraRequest,
    effects: &mut Queue<Receipt>,
  ) -> Result<Received, Never> {
    let effect = match request {
      CameraRequest::Offer(frame) => {
        state.frame = Some(frame);
        None
      }
      CameraRequest::End(fault) => {
        state.fault = Some(fault);
        None
      }
      CameraRequest::Take => match state.frame.take() {
        Some(frame) => Some(Receipt::Frame(frame)),
        None => state.fault.take().map(Receipt::Fault),
      },
    };

    let Ok(()) = effects.offer_all(effect);
    Ok(state)
  }
}

pub(super) struct Stream {
  _capture: BoundToParent,
  received: Actor<Inbox>,
  scope: Option<Scope>,
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
      .stdin(Stdio::null());
    Self::start(&mut capture)
  }

  fn start(capture: &mut std::process::Command) -> io::Result<Self> {
    let (process, pipe) = capture_frames(capture)?;
    let inbox = boot::<Inbox>(()).map_err(io::Error::other)?;
    let sender = inbox.actor.clone();
    let started = std::thread::Builder::new()
      .name(String::from("camera-frames"))
      .spawn(move || {
        let Ok(()) = read_frames(BufReader::new(pipe), &sender);
      });
    let worker = match started {
      Ok(worker) => worker,
      Err(fault) => {
        let Ok(()) = inbox.scope.close();
        return Err(fault);
      }
    };
    let Ok(()) = let_go(worker);

    Ok(Self {
      _capture: process,
      received: inbox.actor,
      scope: Some(inbox.scope),
      last_frame: Instant::now(),
    })
  }

  pub fn frame(&mut self, now: Instant) -> io::Result<Option<Pixels>> {
    let effects = self
      .received
      .send(CameraRequest::Take)
      .map_err(io::Error::other)?;

    match effects.into_iter().next() {
      Some(Receipt::Frame(frame)) => {
        self.last_frame = now;
        return Ok(Some(frame));
      }
      Some(Receipt::Fault(fault)) => return Err(fault),
      None => {}
    }

    match {
      ::core::primitive::bool::then_some(
        now.saturating_duration_since(self.last_frame) >= FRAME_TIMEOUT,
        (),
      )
    } {
      ::core::option::Option::Some(()) => {
        return Err(io::Error::new(
          io::ErrorKind::TimedOut,
          "camera did not send frames",
        ));
      }
      ::core::option::Option::None => {}
    }

    Ok(None)
  }
}

fn capture_frames(
  capture: &mut std::process::Command,
) -> io::Result<(BoundToParent, UnixStream)> {
  capture.args([
    "-map", "0:v:0",
    "-filter_threads", "1",
    "-vf", "scale=w='min(iw,1280)':h='min(ih,1280)':force_original_aspect_ratio=decrease:flags=fast_bilinear",
    "-fps_mode", "passthrough", "-threads", "1",
    "-c:v", "ppm", "-pix_fmt", "rgb24", "-f", "image2pipe", "pipe:1",
  ]);
  let (output, child_output) = UnixStream::pair()?;
  output.set_read_timeout(Some(FRAME_TIMEOUT))?;
  capture.stdout(Stdio::from(OwnedFd::from(child_output)));
  let started = alongside(capture);
  capture.stdout(Stdio::null());
  let process = started?;
  Ok((process, output))
}

impl Drop for Stream {
  fn drop(&mut self) {
    match self.scope.take() {
      Some(scope) => {
        let Ok(()) = scope.close();
      }
      None => {}
    }
  }
}

fn read_frames(pipe: impl io::BufRead, inbox: &Actor<Inbox>) -> Result<(), Never> {
  let stopped = iterate(pipe, |mut input| {
    let decoded = console_pictures::read_portable_pixmap(&mut input, MAX_FRAME);
    let request = match decoded {
      Ok(Some(frame)) => CameraRequest::Offer(frame),
      Ok(None) => CameraRequest::End(io::Error::other("camera stream ended")),
      Err(fault) => CameraRequest::End(fault),
    };
    let continues = matches!(request, CameraRequest::Offer(_));
    let submitted = inbox.send(request);

    Ok(match (submitted, continues) {
      (Ok(_receipt), true) => Step::Again(input),
      (Ok(_), false) | (Err(console_actor::SendError::Closed), _) => Step::Halt(()),
      (Err(fault @ console_actor::SendError::MachineDefect(_)), _) => {
        console_core_logging::warn!("camera frame inbox: {fault}");
        Step::Halt(())
      }
    })
  });

  match stopped {
    Ok(()) => {}
    Err(fault) => console_core_logging::warn!("camera frame reader: {fault}"),
  }

  Ok(())
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
    let Ok(flags) = device_flags(&caps);
    assert_eq!(flags & VIDEO_CAPTURE, 0);
    let Ok(legacy_flags) = device_flags(&QueryCapabilities {
      capabilities: VIDEO_CAPTURE,
      ..caps
    });
    assert_eq!(legacy_flags, VIDEO_CAPTURE);
  }

  #[test]
  fn the_reader_keeps_the_latest_complete_frame() -> io::Result<()> {
    let size = Size {
      width: 2,
      height: 1,
    };
    let Ok(first) = console_pictures::portable_pixmap(size, &[5, 6, 7, 8, 9, 10]);
    let Ok(second) = console_pictures::portable_pixmap(size, &[11, 12, 13, 14, 15, 16]);
    let inbox = boot::<Inbox>(()).map_err(io::Error::other)?;
    let input = [first, second].concat();
    let Ok(()) = read_frames(input.as_slice(), &inbox.actor);
    let newest = inbox.actor.send(CameraRequest::Take);
    let terminal = inbox.actor.send(CameraRequest::Take);
    let Ok(()) = inbox.scope.close();
    let last_frame = newest.map_err(io::Error::other)?;
    let ended = terminal.map_err(io::Error::other)?;

    match last_frame.as_slice() {
      [Receipt::Frame(pixels)] => assert_eq!(pixels.bytes, [11, 12, 13, 255, 14, 15, 16, 255]),
      _ => return Err(io::Error::other("latest camera frame was lost")),
    }

    match ended.as_slice() {
      [Receipt::Fault(fault)] => assert_eq!(fault.to_string(), "camera stream ended"),
      _ => return Err(io::Error::other("camera fault was lost")),
    }

    Ok(())
  }

  #[test]
  #[ignore = "requires the desktop FFmpeg with the lavfi test source"]
  fn a_monochrome_stream_can_be_previewed_and_encoded() -> io::Result<()> {
    let Ok(mut capture) = Program::Ffmpeg.command();
    capture.args([
      "-nostdin",
      "-loglevel",
      "error",
      "-f",
      "lavfi",
      "-i",
      "testsrc=size=16x8:rate=10,format=gray",
      "-frames:v",
      "2",
    ]);
    let (_capture, pipe) = capture_frames(&mut capture)?;
    let mut source = BufReader::new(pipe);
    let decoded_frame = console_pictures::read_portable_pixmap(&mut source, MAX_FRAME)?;
    let frame = decoded_frame.ok_or_else(|| io::Error::other("no camera frame was decoded"))?;
    let size = Size {
      width: 16,
      height: 8,
    };
    let png =
      console_core_png_files::encoded_rgba(size, &frame.bytes).map_err(io::Error::other)?;
    let decoded = console_core_png_files::decoded(&png).map_err(io::Error::other)?;
    assert_eq!(decoded.size, size);
    assert_eq!(decoded.bytes, frame.bytes);
    let (pixels, incomplete) = frame.bytes.as_chunks::<4>();
    assert!(incomplete.is_empty());
    assert!(
      pixels
        .iter()
        .all(|[red, green, blue, alpha]| { red == green && green == blue && *alpha == 255 })
    );
    Ok(())
  }
}
