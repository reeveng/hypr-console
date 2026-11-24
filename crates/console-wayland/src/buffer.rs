//! Shared-memory buffers in the compositor's four-byte pixel formats.

use std::io;

use console_core_geometry::Size;
use console_core_never::Never;

use crate::client::protocol::{wl_buffer, wl_shm, wl_shm_pool};
use crate::client::{Dispatch, QueueHandle};
use crate::memory::Shared;

const PIXEL_BYTES: u32 = 4;

struct Objects {
  buffer: wl_buffer::WlBuffer,
  pool: wl_shm_pool::WlShmPool,
}

impl Drop for Objects {
  fn drop(&mut self) {
    self.buffer.destroy();
    self.pool.destroy();
  }
}

pub struct ShmBuffer {
  objects: Objects,
  memory: Shared,
  pixels: Option<Vec<u8>>,
  stride: u32,
}

impl ShmBuffer {
  pub fn new<State>(
    shm: &wl_shm::WlShm,
    hand: &QueueHandle<State>,
    size: Size<u32>,
    format: wl_shm::Format,
  ) -> io::Result<Self>
  where
    State: Dispatch<wl_shm_pool::WlShmPool, ()> + Dispatch<wl_buffer::WlBuffer, ()> + 'static,
  {
    let too_large = || {
      io::Error::new(
        io::ErrorKind::InvalidInput,
        "frame exceeds Wayland shared-memory limits",
      )
    };
    let stride = size.width.checked_mul(PIXEL_BYTES).ok_or_else(too_large)?;
    let bytes = u64::from(stride)
      .saturating_mul(u64::from(size.height))
      .max(u64::from(PIXEL_BYTES));
    let length = i32::try_from(bytes).map_err(|_| too_large())?;
    let width = i32::try_from(size.width).map_err(|_| too_large())?;
    let height = i32::try_from(size.height).map_err(|_| too_large())?;
    let row = i32::try_from(stride).map_err(|_| too_large())?;
    let memory = Shared::of(bytes)?;
    let Ok(descriptor) = memory.descriptor();
    let pool = shm.create_pool(descriptor, length, hand, ());
    let buffer = pool.create_buffer(0, width, height, row, format, hand, ());

    Ok(Self {
      objects: Objects { buffer, pool },
      memory,
      pixels: None,
      stride,
    })
  }

  #[must_use]
  pub fn buffer(&self) -> Result<&wl_buffer::WlBuffer, Never> {
    Ok(&self.objects.buffer)
  }

  pub fn pixels(&mut self) -> io::Result<&mut [u8]> {
    let pixels = match &mut self.pixels {
      Some(pixels) => pixels,
      empty @ None => {
        let allocated = self.memory.allocate_pixels()?;
        empty.insert(allocated)
      }
    };
    Ok(pixels)
  }

  pub fn painted_buffer(&self) -> io::Result<&wl_buffer::WlBuffer> {
    match self.pixels.as_deref() {
      Some(pixels) => self.memory.write_pixels(pixels)?,
      None => {}
    }

    let Ok(buffer) = self.buffer();
    Ok(buffer)
  }

  pub fn write(&self, pixels: &[u8]) -> io::Result<()> {
    self.memory.write_pixels(pixels)
  }

  #[must_use]
  pub fn into_memory(self) -> Result<(Shared, u32), Never> {
    Ok((self.memory, self.stride))
  }
}
