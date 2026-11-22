//! The WPE adapter speaks the same frame and input protocol as the other renderers.

#![deny(unsafe_op_in_unsafe_fn)]

use std::cell::RefCell;
use std::ffi::CString;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::process::ExitCode;

use console_browser_renderer::{self as channel, MAX_SIDE, invalid};
use console_core_geometry::Size;
use console_core_number_conversion::{fitted, index};
use console_core_pixel_buffers::RowLayout;
use console_core_web_engine::{Event, FrameLayout, Request, decoded_request, encoded_event};
mod wpe;

#[derive(Clone, Copy, Default)]
enum FrameQueue {
  #[default]
  Idle,
  Ready(Size<u32>),
  AwaitingRelease,
  AwaitingReleaseWithFrame(Size<u32>),
}

#[derive(Default)]
struct Frames {
  queue: FrameQueue,
  pixels: Vec<u8>,
}

struct Session<'a> {
  socket: BorrowedFd<'a>,
  frames: RefCell<Frames>,
}

impl Session<'_> {
  fn send(&self, event: Event) -> io::Result<()> {
    let Ok(line) = encoded_event(&event);
    channel::send(self.socket, &line, None)
  }

  fn frame(&self, size: Size<u32>, stride: u64, argb: &[u8]) -> io::Result<()> {
    let mut frames = self
      .frames
      .try_borrow_mut()
      .map_err(|_| io::Error::other("WebKit frame callback reentered"))?;
    packed_rgba(size, stride, argb, &mut frames.pixels)?;
    frames.queue = match frames.queue {
      FrameQueue::Idle | FrameQueue::Ready(_) => FrameQueue::Ready(size),
      FrameQueue::AwaitingRelease | FrameQueue::AwaitingReleaseWithFrame(_) => {
        FrameQueue::AwaitingReleaseWithFrame(size)
      }
    };
    self.publish(&mut frames)
  }

  fn publish(&self, frames: &mut Frames) -> io::Result<()> {
    let size = match frames.queue {
      FrameQueue::Ready(size) => size,
      FrameQueue::Idle
      | FrameQueue::AwaitingRelease
      | FrameQueue::AwaitingReleaseWithFrame(_) => {
        return Ok(());
      }
    };
    let memory = channel::sealed(&frames.pixels)?;
    let Ok(line) = encoded_event(&Event::Frame(size));
    channel::send(self.socket, &line, Some(&memory))?;
    frames.queue = FrameQueue::AwaitingRelease;
    Ok(())
  }

  fn release(&self) -> io::Result<()> {
    let mut frames = self
      .frames
      .try_borrow_mut()
      .map_err(|_| io::Error::other("WebKit frame release reentered"))?;
    frames.queue = match frames.queue {
      FrameQueue::AwaitingRelease => FrameQueue::Idle,
      FrameQueue::AwaitingReleaseWithFrame(size) => FrameQueue::Ready(size),
      FrameQueue::Idle => FrameQueue::Idle,
      FrameQueue::Ready(size) => FrameQueue::Ready(size),
    };
    self.publish(&mut frames)
  }
}

fn packed_rgba(
  size: Size<u32>,
  stride: u64,
  argb: &[u8],
  pixels: &mut Vec<u8>,
) -> io::Result<()> {
  const _: () = assert!(MAX_SIDE.saturating_mul(MAX_SIDE).saturating_mul(4) <= 16_777_216);

  match bool::then_some(size.width <= MAX_SIDE && size.height <= MAX_SIDE, ()) {
    Some(()) => {}
    None => return Err(invalid("WebKit frame exceeds the viewport limit")),
  }

  let layout = FrameLayout::rgba(size).map_err(|_| invalid("empty WebKit frame"))?;
  let row_extent = Size {
    width: layout.stride,
    height: u64::from(size.height),
  };
  let step = i64::try_from(stride).map_err(io::Error::other)?;
  let strided = RowLayout::new(row_extent, 0, step)
    .map_err(|_| invalid("WebKit frame has invalid row storage"))?;
  let rows = strided
    .rows(argb)
    .map_err(|_| invalid("WebKit frame is missing rows"))?;
  let Ok(capacity) = index(layout.bytes);
  pixels
    .try_reserve_exact(
      capacity
        .saturating_sub(pixels.len())
        .min(const { 16_777_216 }),
    )
    .map_err(io::Error::other)?;
  pixels.clear();

  for row in rows {
    let (visible_pixels, _) = row.as_chunks::<4>();

    for pixel in visible_pixels {
      #[cfg(target_endian = "big")]
      let [a, r, g, b] = *pixel;
      #[cfg(target_endian = "little")]
      let [b, g, r, a] = *pixel;
      let unpremultiply = |channel| match a {
        0 => 0,
        255 => channel,
        _ => {
          let full = u16::from(channel).saturating_mul(255);
          let ratio = match full.checked_div(u16::from(a)) {
            Some(color) => color,
            None => return 0,
          };
          let Ok(saturated) = fitted::<_, u8>(ratio);
          saturated
        }
      };
      pixels.extend_from_slice(&[unpremultiply(r), unpremultiply(g), unpremultiply(b), a]);
    }
  }

  Ok(())
}

fn message(session: &Session<'_>) -> io::Result<Option<Request>> {
  let packet = channel::receive(session.socket)?;

  match packet.frame {
    Some(_) => return Err(invalid("a WebKit request carried frame memory")),
    None => {}
  }

  match packet.line.as_str() {
    channel::RELEASED => {
      session.release()?;
      Ok(None)
    }
    line => {
      let Ok(request) = decoded_request(line);
      Ok(request)
    }
  }
}

fn run(socket: BorrowedFd<'_>) -> io::Result<()> {
  let session = Session {
    socket,
    frames: RefCell::default(),
  };
  let resize = message(&session)?;
  let size = match resize {
    Some(Request::Resize(size)) => size,
    Some(_) | None => return Err(invalid("set the WebKit viewport first")),
  };
  let open = message(&session)?;
  let address = match open {
    Some(Request::Open(address)) => address,
    Some(_) | None => return Err(invalid("send a WebKit address next")),
  };
  let address_c_string = CString::new(address).map_err(io::Error::other)?;
  wpe::run(&session, size, &address_c_string)
}

fn main() -> ExitCode {
  match run(std::io::stdin().as_fd()) {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      console_core_logging::error!("WebKit renderer: {error}");
      ExitCode::FAILURE
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn native_argb_becomes_packed_straight_rgba() -> io::Result<()> {
    let mut bytes = Vec::new();
    #[cfg(target_endian = "little")]
    let (opaque, translucent) = ([56, 34, 12, 255], [16, 32, 64, 128]);
    #[cfg(target_endian = "big")]
    let (opaque, translucent) = ([255, 12, 34, 56], [128, 64, 32, 16]);
    bytes.extend_from_slice(&opaque);
    bytes.extend_from_slice(&[77; 4]);
    bytes.extend_from_slice(&translucent);
    let mut packed = Vec::new();
    packed_rgba(
      Size {
        width: 1,
        height: 2,
      },
      8,
      &bytes,
      &mut packed,
    )?;
    assert_eq!(packed, [12, 34, 56, 255, 127, 63, 31, 128]);
    Ok(())
  }

  #[test]
  fn invalid_strided_frame_preserves_the_queued_pixels() {
    let size = Size { width: 1, height: 2 };
    let mut packed = vec![10, 20, 30, 255];
    let short = [0; 11];

    for stride in [0, 8, u64::MAX] {
      let packed_result = packed_rgba(size, stride, &short, &mut packed);
      assert!(matches!(packed_result, Err(problem)
        if matches!(problem.kind(), io::ErrorKind::InvalidData | io::ErrorKind::Other)));
      assert_eq!(packed, [10, 20, 30, 255]);
    }
  }
}
