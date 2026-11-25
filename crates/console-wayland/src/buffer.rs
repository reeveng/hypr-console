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

pub struct PreparedFrame {
  size: Size<u32>,
  memory: Shared,
}

impl PreparedFrame {
  pub fn size(&self) -> Result<Size<u32>, Never> {
    Ok(self.size)
  }

  pub fn paint(
    size: Size<u32>,
    paint: impl FnOnce(&mut [u8]) -> Result<(), Never>,
  ) -> io::Result<Self> {
    let dimensions = Dimensions::new(size)?;
    let memory = Shared::of(dimensions.bytes)?;
    let mut pixels = memory.allocate_pixels()?;
    let Ok(()) = paint(&mut pixels);
    memory.write_pixels(&pixels)?;

    Ok(Self { size, memory })
  }
}

struct Dimensions {
  size: Size<u32>,
  stride: u32,
  bytes: u64,
}

impl Dimensions {
  fn new(size: Size<u32>) -> io::Result<Self> {
    let stride = size
      .width
      .checked_mul(PIXEL_BYTES)
      .ok_or(io::ErrorKind::InvalidInput)?;
    Ok(Self {
      size,
      stride,
      bytes: u64::from(stride)
        .saturating_mul(u64::from(size.height))
        .max(u64::from(PIXEL_BYTES)),
    })
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
    let dimensions = Dimensions::new(size)?;
    let _length = i32::try_from(dimensions.bytes).map_err(|_| io::ErrorKind::InvalidInput)?;
    let memory = Shared::of(dimensions.bytes)?;

    Self::from_memory(shm, hand, dimensions, format, memory)
  }

  pub(crate) fn prepared<State>(
    shm: &wl_shm::WlShm,
    hand: &QueueHandle<State>,
    prepared: &PreparedFrame,
  ) -> io::Result<Self>
  where
    State: Dispatch<wl_shm_pool::WlShmPool, ()> + Dispatch<wl_buffer::WlBuffer, ()> + 'static,
  {
    let dimensions = Dimensions::new(prepared.size)?;
    let memory = prepared.memory.duplicate()?;

    Self::from_memory(shm, hand, dimensions, wl_shm::Format::Argb8888, memory)
  }

  pub fn import_prepared<State>(
    shm: &wl_shm::WlShm,
    hand: &QueueHandle<State>,
    prepared: PreparedFrame,
  ) -> io::Result<Self>
  where
    State: Dispatch<wl_shm_pool::WlShmPool, ()> + Dispatch<wl_buffer::WlBuffer, ()> + 'static,
  {
    let dimensions = Dimensions::new(prepared.size)?;

    Self::from_memory(
      shm,
      hand,
      dimensions,
      wl_shm::Format::Argb8888,
      prepared.memory,
    )
  }

  fn from_memory<State>(
    shm: &wl_shm::WlShm,
    hand: &QueueHandle<State>,
    dimensions: Dimensions,
    format: wl_shm::Format,
    memory: Shared,
  ) -> io::Result<Self>
  where
    State: Dispatch<wl_shm_pool::WlShmPool, ()> + Dispatch<wl_buffer::WlBuffer, ()> + 'static,
  {
    let Dimensions {
      size,
      stride,
      bytes,
    } = dimensions;
    let length = i32::try_from(bytes).map_err(|_| io::ErrorKind::InvalidInput)?;
    let width = i32::try_from(size.width).map_err(|_| io::ErrorKind::InvalidInput)?;
    let height = i32::try_from(size.height).map_err(|_| io::ErrorKind::InvalidInput)?;
    let row = i32::try_from(stride).map_err(|_| io::ErrorKind::InvalidInput)?;
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

  pub(crate) fn keep_painted(&mut self, size: Size<u32>) -> io::Result<PreparedFrame> {
    let memory = self.memory.duplicate()?;
    self.pixels = None;

    Ok(PreparedFrame { size, memory })
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
