//! Import linear RGBA frames into the existing video subsurface.
//! Version 3 advertises supported format/modifier pairs as events.
//! Presented frames are immutable and complete. After import the compositor owns its own file descriptor.

use std::os::fd::BorrowedFd;

use console_core_geometry::Size;
use console_core_never::Never;
use console_response_times::frames::PresentationCounts;
use wayland_client::globals::GlobalList;
use wayland_client::protocol::wl_buffer;
use wayland_client::{Connection, Dispatch, QueueHandle, event_created_child};
use wayland_protocols::wp::linux_dmabuf::zv1::client::{
  zwp_linux_buffer_params_v1, zwp_linux_dmabuf_v1,
};

use super::{
  Bound, Buffer, Fit, Part, RegionLayer, Surface, SurfaceError, Timing, Visible, record_frame,
  region, start_painting,
};

const ABGR8888: u32 = u32::from_le_bytes(*b"AB24");

pub struct DmaBufView<'a> {
  pub fd: BorrowedFd<'a>,
  pub size: Size<u32>,
  pub stride: u32,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DmaBufSupport {
  Available,
  Unavailable,
}

pub(super) struct Import {
  proxy: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
  state: ImportState,
}

enum ImportState {
  WaitingForFormats,
  Ready,
  WaitingForBuffer,
  Imported(wl_buffer::WlBuffer),
  Rejected,
}

struct PreparedFrame<'a> {
  fd: BorrowedFd<'a>,
  width: i32,
  height: i32,
  stride: u32,
}

impl<'a> PreparedFrame<'a> {
  fn prepare(frame: DmaBufView<'a>) -> Result<Option<Self>, Never> {
    let (width, height, row) = match (
      i32::try_from(frame.size.width),
      i32::try_from(frame.size.height),
      frame.size.width.checked_mul(4),
    ) {
      (Ok(width @ 1..), Ok(height @ 1..), Some(row)) => (width, height, row),
      _ => return Ok(None),
    };
    Ok((frame.stride >= row).then_some(Self {
      fd: frame.fd,
      width,
      height,
      stride: frame.stride,
    }))
  }
}

impl Import {
  pub(super) fn bind(
    globals: &GlobalList,
    hand: &QueueHandle<Bound>,
  ) -> Result<Option<Self>, Never> {
    let proxy = match globals.bind(hand, 3..=3, ()) {
      Ok(proxy) => proxy,
      Err(_dma_buf_protocol_unavailable) => return Ok(None),
    };
    Ok(Some(Self {
      proxy,
      state: ImportState::WaitingForFormats,
    }))
  }

  fn take_buffer(&mut self) -> Result<Option<wl_buffer::WlBuffer>, Never> {
    Ok(
      match std::mem::replace(&mut self.state, ImportState::Rejected) {
        ImportState::Imported(buffer) => {
          self.state = ImportState::Ready;
          Some(buffer)
        }
        ImportState::WaitingForFormats
        | ImportState::Ready
        | ImportState::WaitingForBuffer
        | ImportState::Rejected => None,
      },
    )
  }

  fn accept_linear_format(&mut self) -> Result<(), Never> {
    match self.state {
      ImportState::WaitingForFormats => self.state = ImportState::Ready,
      ImportState::Ready
      | ImportState::WaitingForBuffer
      | ImportState::Imported(_)
      | ImportState::Rejected => {}
    }

    Ok(())
  }

  fn request(
    &mut self,
    hand: &QueueHandle<Bound>,
    frame: &PreparedFrame<'_>,
  ) -> Result<Option<zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1>, Never> {
    Ok(match self.state {
      ImportState::Ready => {
        let parameters = self.proxy.create_params(hand, ());
        parameters.add(frame.fd, 0, 0, frame.stride, 0, 0);
        parameters.create(
          frame.width,
          frame.height,
          ABGR8888,
          zwp_linux_buffer_params_v1::Flags::empty(),
        );
        self.state = ImportState::WaitingForBuffer;
        Some(parameters)
      }
      ImportState::WaitingForFormats
      | ImportState::WaitingForBuffer
      | ImportState::Imported(_)
      | ImportState::Rejected => None,
    })
  }

  fn observe(&mut self, event: zwp_linux_buffer_params_v1::Event) -> Result<(), Never> {
    match (event, &self.state) {
      (zwp_linux_buffer_params_v1::Event::Created { buffer }, ImportState::WaitingForBuffer) => {
        self.state = ImportState::Imported(buffer);
      }
      (zwp_linux_buffer_params_v1::Event::Created { buffer }, _) => buffer.destroy(),
      (zwp_linux_buffer_params_v1::Event::Failed, ImportState::WaitingForBuffer) => {
        self.state = ImportState::Rejected;
      }
      _ => {}
    }

    Ok(())
  }
}

impl Drop for Import {
  fn drop(&mut self) {
    match &self.state {
      ImportState::Imported(buffer) => buffer.destroy(),
      ImportState::WaitingForFormats
      | ImportState::Ready
      | ImportState::WaitingForBuffer
      | ImportState::Rejected => {}
    }

    self.proxy.destroy();
  }
}

impl Surface {
  #[must_use]
  pub fn dma_buf_support(&self) -> Result<DmaBufSupport, Never> {
    Ok(
      match (
        self.bound.subcompositor.as_ref(),
        self.bound.dmabuf.as_ref(),
      ) {
        (
          Some(_),
          Some(Import {
            state: ImportState::Ready,
            ..
          }),
        ) => DmaBufSupport::Available,
        _ => DmaBufSupport::Unavailable,
      },
    )
  }

  #[must_use]
  pub fn presentations(&self) -> Result<PresentationCounts, Never> {
    match self.bound.timing.as_ref() {
      Some(frames) => frames.presentations(),
      None => Ok(PresentationCounts::default()),
    }
  }

  pub fn present_dma_buf(
    &mut self,
    part: Part,
    frame: DmaBufView<'_>,
  ) -> Result<Visible, SurfaceError> {
    let (manager, parent, logical) = match (
      self.bound.subcompositor.clone(),
      self.bound.up.as_ref().map(|up| up.surface.clone()),
      self.bound.logical,
    ) {
      (Some(manager), Some(parent), Some(logical)) => (manager, parent, logical),
      _ => return Ok(Visible::NotYet),
    };
    let Ok(fit) = region::fit(logical, part);

    match fit {
      Fit::Inside => {}
      Fit::Outside => return Ok(Visible::NotYet),
    }

    let Ok(candidate) = PreparedFrame::prepare(frame);
    let prepared = match candidate {
      Some(verified) => verified,
      None => return Ok(Visible::NotYet),
    };
    let hand = self.queue.handle();
    let Ok(painting) = start_painting(self.bound.timing.as_ref());
    let offered = self.import_frame(&hand, &prepared)?;
    let buffer = match offered {
      Some(buffer) => buffer,
      None => return Ok(Visible::NotYet),
    };

    let layer = self.bound.region.get_or_insert_with(|| {
      let Ok(layer) = RegionLayer::create(
        &self.bound.compositor,
        &manager,
        &self.bound.viewporter,
        &parent,
        &hand,
      );
      layer
    });
    let Ok(position) = layer.position(part.at);
    let Ok(across) = console_core_number_conversion::fitted(part.size.width);
    let Ok(down) = console_core_number_conversion::fitted(part.size.height);
    layer.surface.attach(Some(&buffer), 0, 0);
    layer.viewport.set_destination(across, down);
    layer
      .surface
      .damage_buffer(0, 0, prepared.width, prepared.height);
    let child = layer.surface.clone();
    let Ok(()) = record_frame(Timing {
      bound: &mut self.bound,
      painting,
      held: Buffer::Released,
      hand: &hand,
      region: Some(&child),
    });
    child.commit();
    let Ok(()) = position.commit(&parent);

    let _sent = self.flush()?;
    Ok(Visible::Yes)
  }

  fn import_frame(
    &mut self,
    hand: &QueueHandle<Bound>,
    frame: &PreparedFrame<'_>,
  ) -> Result<Option<wl_buffer::WlBuffer>, SurfaceError> {
    let request = self.bound.dmabuf.as_mut().and_then(|import| {
      let Ok(request) = import.request(hand, frame);
      request
    });
    let parameters = match request {
      Some(parameters) => parameters,
      None => return Ok(None),
    };
    let result = self.queue.roundtrip(&mut self.bound).map(|_| ());
    parameters.destroy();
    result.map_err(SurfaceError::Went)?;

    Ok(self.bound.dmabuf.as_mut().and_then(|import| {
      let Ok(buffer) = import.take_buffer();
      buffer
    }))
  }
}

impl Dispatch<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1, ()> for Bound {
  fn event(
    state: &mut Self,
    _: &zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
    event: zwp_linux_dmabuf_v1::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match (event, state.dmabuf.as_mut()) {
      (
        zwp_linux_dmabuf_v1::Event::Modifier {
          format: ABGR8888,
          modifier_hi: 0,
          modifier_lo: 0,
        },
        Some(import),
      ) => {
        let Ok(()) = import.accept_linear_format();
      }
      _ => {}
    }
  }
}

struct Imported;

impl Dispatch<zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1, ()> for Bound {
  fn event(
    state: &mut Self,
    _: &zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
    event: zwp_linux_buffer_params_v1::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match (state.dmabuf.as_mut(), event) {
      (Some(import), reply) => {
        let Ok(()) = import.observe(reply);
      }
      (None, zwp_linux_buffer_params_v1::Event::Created { buffer }) => buffer.destroy(),
      (None, _) => {}
    }
  }

  event_created_child!(Bound, zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1, [
    zwp_linux_buffer_params_v1::EVT_CREATED_OPCODE => (wl_buffer::WlBuffer, Imported)
  ]);
}

impl Dispatch<wl_buffer::WlBuffer, Imported> for Bound {
  fn event(
    _: &mut Self,
    buffer: &wl_buffer::WlBuffer,
    event: wl_buffer::Event,
    _: &Imported,
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    match matches!(event, wl_buffer::Event::Release).then_some(()) {
      Some(()) => buffer.destroy(),
      None => {}
    }
  }
}
