//! Linux channels carry the same frame metadata as the Console kernel, with
//! one sealed memory descriptor alongside a frame and none alongside control.

#![forbid(unsafe_code)]

use std::cmp::Ordering;
use std::fs::File;
use std::io::{self, IoSlice, IoSliceMut, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use console_core_web_engine::FrameLayout;
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, fcntl_get_seals, fstat, memfd_create};
use rustix::net::{
  RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
  SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
};

type PacketBytes = [u8; 65_536];

pub const MAX_SIDE: u32 = 2048;
const SEALED: SealFlags = SealFlags::WRITE
  .union(SealFlags::GROW)
  .union(SealFlags::SHRINK);
pub const RELEASED: &str = "frame-released";

pub struct Packet {
  pub line: String,
  pub frame: Option<OwnedFd>,
}

enum Attachment {
  Ready,
  Failed,
}

impl From<bool> for Attachment {
  fn from(attached: bool) -> Self {
    match bool::then_some(attached, ()) {
      Some(()) => Self::Ready,
      None => Self::Failed,
    }
  }
}

enum PacketExtent {
  Complete,
  Truncated,
}

impl From<bool> for PacketExtent {
  fn from(truncated: bool) -> Self {
    match bool::then_some(truncated, ()) {
      Some(()) => Self::Truncated,
      None => Self::Complete,
    }
  }
}

pub fn send(socket: BorrowedFd<'_>, line: &str, frame: Option<&OwnedFd>) -> io::Result<()> {
  let payload = line.as_bytes();

  match (payload, payload.len().cmp(&size_of::<PacketBytes>())) {
    ([_, ..], Ordering::Less | Ordering::Equal) => {}
    ([], _) | (_, Ordering::Greater) => return Err(invalid("invalid engine message length")),
  }

  let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
  let mut control = SendAncillaryBuffer::new(&mut space);
  let rights = frame.map(|frame_2| [frame_2.as_fd()]);

  let attachment = match &rights {
    None => Attachment::Ready,
    Some(rights_2) => Attachment::from(control.push(SendAncillaryMessage::ScmRights(rights_2))),
  };

  match attachment {
    Attachment::Ready => {}
    Attachment::Failed => return Err(invalid("could not attach the frame descriptor")),
  }

  sendmsg(
    socket,
    &[IoSlice::new(payload)],
    &mut control,
    SendFlags::NOSIGNAL,
  )?;

  Ok(())
}

pub fn receive(socket: BorrowedFd<'_>) -> io::Result<Packet> {
  let mut bytes: PacketBytes = [0; _];
  let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
  let mut control = RecvAncillaryBuffer::new(&mut space);
  let read = recvmsg(
    socket,
    &mut [IoSliceMut::new(&mut bytes)],
    &mut control,
    RecvFlags::CMSG_CLOEXEC,
  )?;

  match (
    read.bytes,
    PacketExtent::from(
      read
        .flags
        .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC),
    ),
  ) {
    (0, PacketExtent::Complete | PacketExtent::Truncated) => {
      return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
    }
    (1.., PacketExtent::Truncated) => return Err(invalid("truncated engine message")),
    (1.., PacketExtent::Complete) => {}
  }

  let mut frame = None;

  for message in control.drain() {
    #[cfg_attr(
      dylint_lib = "explicit016_no_wildcard_arm",
      expect(
        explicit016_no_wildcard_arm,
        reason = "rustix exposes a non-exhaustive enum and the frame channel rejects every ancillary message except SCM_RIGHTS"
      )
    )]
    let rights = match message {
      RecvAncillaryMessage::ScmRights(rights) => rights,
      _unsupported => return Err(invalid("unexpected ancillary engine data")),
    };

    for descriptor in rights {
      match frame.replace(descriptor) {
        None => {}
        Some(_previous) => return Err(invalid("engine message carried more than one frame")),
      }
    }
  }

  let payload = bytes
    .get(..read.bytes)
    .ok_or_else(|| invalid("invalid engine message length"))?;
  let line = std::str::from_utf8(payload).map_err(io::Error::other)?;

  Ok(Packet {
    line: line.to_owned(),
    frame,
  })
}

pub fn sealed(pixels: &[u8]) -> io::Result<OwnedFd> {
  let fd = memfd_create(
    "console-browser-frame",
    MemfdFlags::ALLOW_SEALING | MemfdFlags::CLOEXEC,
  )?;
  let mut file = File::from(fd);
  file.write_all(pixels)?;
  fcntl_add_seals(&file, SEALED | SealFlags::SEAL)?;

  Ok(<_ as ::core::convert::Into<::rustix::fd::OwnedFd>>::into(
    file,
  ))
}

enum FrameValidity {
  Valid,
  Invalid,
}

impl From<bool> for FrameValidity {
  fn from(valid: bool) -> Self {
    match bool::then_some(valid, ()) {
      Some(()) => Self::Valid,
      None => Self::Invalid,
    }
  }
}

pub fn validate(frame: &OwnedFd, layout: FrameLayout) -> io::Result<()> {
  let seals = fcntl_get_seals(frame)?;
  let validity = match FrameValidity::from(seals.contains(SEALED)) {
    FrameValidity::Invalid => FrameValidity::Invalid,
    FrameValidity::Valid => {
      let stat = fstat(frame)?;

      FrameValidity::from(u64::try_from(stat.st_size) == Ok(layout.bytes))
    }
  };

  match validity {
    FrameValidity::Valid => Ok(()),
    FrameValidity::Invalid => Err(invalid(
      "frame memory must be immutable and match its dimensions",
    )),
  }
}

#[must_use]
#[cfg_attr(
  dylint_lib = "explicit002_infallible_result",
  expect(
    explicit002_infallible_result,
    reason = "the renderer projects use this constructor to supply an io::Error to map_err and ok_or_else"
  )
)]
pub fn invalid(message: &'static str) -> io::Error {
  io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_core_size::Size;

  #[test]
  fn the_window_refuses_writable_or_wrong_sized_frames() -> io::Result<()> {
    let layout = FrameLayout::rgba(Size {
      width: 1,
      height: 1,
    })
    .map_err(|_invalid_layout| invalid("one pixel fits"))?;
    let immutable = sealed(&[1, 2, 3, 255])?;

    validate(&immutable, layout)?;

    let wrong_size = sealed(&[0; 8])?;

    assert_eq!(
      validate(&wrong_size, layout).map_err(|fault| fault.kind()),
      Err(io::ErrorKind::InvalidData)
    );
    let writable = memfd_create(
      "unsealed-frame",
      MemfdFlags::ALLOW_SEALING | MemfdFlags::CLOEXEC,
    )?;

    rustix::fs::ftruncate(&writable, layout.bytes)?;
    assert_eq!(
      validate(&writable, layout).map_err(|fault| fault.kind()),
      Err(io::ErrorKind::InvalidData)
    );

    Ok(())
  }
}
