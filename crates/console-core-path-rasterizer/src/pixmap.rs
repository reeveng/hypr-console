use alloc::vec::Vec;

use console_core_never::Never;
use console_core_number_conversion::{checked_index, fixed_index, index};

use console_core_paths::IntSize;

use crate::{Color, IntRect};

use crate::color::PremultipliedColorU8;
use crate::geom::{offset_in_rect, visible_rect};

pub const BYTES_PER_PIXEL: ::console_core_api_compatibility::NativeSize = 4;

#[derive(Clone, PartialEq)]
pub struct Pixmap {
  data: Vec<u8>,
  size: IntSize,
}

impl Pixmap {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the existing constructor accepts width then height and validates them before forwarding to with_size"
    )
  )]
  pub fn new(
    width: u32,
    height: u32,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(size) = IntSize::from_wh(width, height);

    match size {
      Some(size_2) => Self::with_size(size_2),
      None => Ok(None),
    }
  }

  pub fn with_size(size: IntSize) -> Result<Option<Self>, Never> {
    let Ok(required) = data_len_for_size(size);
    let count = match required {
      Some(bytes) => bytes,
      None => return Ok(None),
    };
    let Ok(data) = zeroed_pixel_data(count);

    Ok(data.map(|bytes| Self { data: bytes, size }))
  }

  pub fn from_vec(data: Vec<u8>, size: IntSize) -> Result<Option<Self>, Never> {
    let Ok(required) = data_len_for_size(size);
    let Ok(length) = fixed_index(data.len());

    Ok((required == Some(length)).then_some(Self { data, size }))
  }

  pub fn from_rgba8(
    mut bytes: Vec<u8>,
    size: IntSize,
  ) -> Result<Option<Self>, console_core_never::Never> {
    for [red, green, blue, alpha] in bytes.as_chunks_mut::<4>().0 {
      let Ok(r) = crate::color::Alpha(*alpha).premultiply(*red);
      let Ok(g) = crate::color::Alpha(*alpha).premultiply(*green);
      let Ok(b) = crate::color::Alpha(*alpha).premultiply(*blue);
      (*red, *green, *blue) = (r, g, b);
    }

    Self::from_vec(bytes, size)
  }

  #[cfg(feature = "png-format")]
  pub fn decode_png(data: &[u8]) -> std::io::Result<Self> {
    let picture =
      console_core_png_files::decoded_rgba(data, console_core_png_files::Reduction::HighByte)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let dimensions = picture.size;
    let Ok(decoded_size) = IntSize::from_wh(dimensions.width, dimensions.height);
    let size = decoded_size.ok_or(std::io::ErrorKind::InvalidData)?;
    let Ok(image) = Self::from_rgba8(picture.bytes, size);
    image.ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))
  }

  #[cfg(feature = "png-format")]
  pub fn load_png<P: AsRef<std::path::Path>>(path: P) -> Result<Self, std::io::Error> {
    let data = std::fs::read(path)?;
    Self::decode_png(&data)
  }

  #[cfg(feature = "png-format")]
  pub fn encode_png(&self) -> Result<Vec<u8>, std::io::Error> {
    ({
      let Ok(explicit_value) = self.as_ref();
      explicit_value
    })
    .encode_png()
  }

  #[cfg(feature = "png-format")]
  pub fn save_png<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), std::io::Error> {
    ({
      let Ok(explicit_value) = self.as_ref();
      explicit_value
    })
    .save_png(path)
  }

  pub fn as_ref(&self) -> core::result::Result<PixmapRef<'_>, console_core_never::Never> {
    Ok(PixmapRef {
      data: &self.data,
      size: self.size,
    })
  }

  pub fn as_mut(&mut self) -> core::result::Result<PixmapMut<'_>, console_core_never::Never> {
    Ok(PixmapMut {
      data: &mut self.data,
      size: self.size,
    })
  }

  #[inline]
  pub fn width(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(width) = self.size.width();

    Ok(width)
  }

  #[inline]
  pub fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(height) = self.size.height();

    Ok(height)
  }

  pub fn size(&self) -> core::result::Result<IntSize, console_core_never::Never> {
    Ok(self.size)
  }

  pub fn fill(&mut self, color: Color) -> core::result::Result<(), console_core_never::Never> {
    let Ok(premultiply) = color.premultiply();
    let Ok(c) = premultiply.to_color_u8();
    let Ok(mut view) = self.as_mut();
    let Ok(pixels) = view.pixels_mut();

    for p in pixels {
      *p = c;
    }

    Ok(())
  }

  pub fn data(&self) -> core::result::Result<&[u8], console_core_never::Never> {
    Ok(self.data.as_slice())
  }

  pub fn data_mut(&mut self) -> core::result::Result<&mut [u8], console_core_never::Never> {
    Ok(self.data.as_mut_slice())
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the existing pixel accessor takes an x column then a y row"
    )
  )]
  pub fn pixel(
    &self,
    x: u32,
    y: u32,
  ) -> core::result::Result<Option<PremultipliedColorU8>, console_core_never::Never> {
    let Ok(width) = self.width();
    let idx = match match width.checked_mul(y) {
      Some(value) => value,
      None => return Ok(None),
    }
    .checked_add(x)
    {
      Some(value) => value,
      None => return Ok(None),
    };

    let Ok(pixels) = self.pixels();

    Ok(
      pixels
        .get({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(idx);
          numeric_value
        })
        .cloned(),
    )
  }

  pub fn pixels_mut(
    &mut self,
  ) -> core::result::Result<&mut [PremultipliedColorU8], console_core_never::Never> {
    let Ok(data_mut) = self.data_mut();

    Ok(bytemuck::cast_slice_mut(data_mut))
  }

  pub fn pixels(
    &self,
  ) -> core::result::Result<&[PremultipliedColorU8], console_core_never::Never> {
    let Ok(data) = self.data();

    Ok(bytemuck::cast_slice(data))
  }

  pub fn take(self) -> core::result::Result<Vec<u8>, console_core_never::Never> {
    Ok(self.data)
  }

  pub fn take_demultiplied(mut self) -> core::result::Result<Vec<u8>, console_core_never::Never> {
    let Ok(pixels_mut) = self.pixels_mut();

    for pixel in pixels_mut {
      let Ok(c) = pixel.demultiply();
      let Ok(red) = c.red();
      let Ok(green) = c.green();
      let Ok(blue) = c.blue();
      let Ok(alpha) = c.alpha();
      let Ok(premultiplied_color_u8) =
        PremultipliedColorU8::from_components([red, green, blue, alpha]);
      *pixel = premultiplied_color_u8;
    }

    Ok(self.data)
  }

  pub fn clone_rect(
    &self,
    rect: IntRect,
  ) -> core::result::Result<Option<Pixmap>, console_core_never::Never> {
    let Ok(clone_rect) = ({
      let Ok(explicit_value) = self.as_ref();
      explicit_value
    })
    .clone_rect(rect);

    Ok(clone_rect)
  }
}

impl core::fmt::Debug for Pixmap {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("Pixmap")
      .field("data", &"...")
      .field(
        "width",
        &({
          let Ok(explicit_value) = self.size.width();
          explicit_value
        }),
      )
      .field(
        "height",
        &({
          let Ok(explicit_value) = self.size.height();
          explicit_value
        }),
      )
      .finish()
  }
}

#[derive(Clone, Copy, PartialEq)]
pub struct PixmapRef<'a> {
  data: &'a [u8],
  size: IntSize,
}

impl<'a> PixmapRef<'a> {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "existing pixel buffer views take width then height"
    )
  )]
  pub fn from_bytes(data: &'a [u8], width: u32, height: u32) -> Result<Option<Self>, Never> {
    let Ok(dimensions) = IntSize::from_wh(width, height);
    let size = match dimensions {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(required) = data_len_for_size(size);
    let Ok(available) = fixed_index(data.len());

    Ok(match required {
      Some(bytes) => (bytes <= available).then_some(Self { data, size }),
      None => None,
    })
  }

  pub fn to_owned(&self) -> core::result::Result<Pixmap, console_core_never::Never> {
    Ok(Pixmap {
      data: self.data.to_vec(),
      size: self.size,
    })
  }

  #[inline]
  pub fn width(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(width) = self.size.width();

    Ok(width)
  }

  #[inline]
  pub fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(height) = self.size.height();

    Ok(height)
  }

  pub(crate) fn size(&self) -> core::result::Result<IntSize, console_core_never::Never> {
    Ok(self.size)
  }

  pub fn data(&self) -> core::result::Result<&'a [u8], console_core_never::Never> {
    Ok(self.data)
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "the existing pixel accessor takes an x column then a y row"
    )
  )]
  pub fn pixel(
    &self,
    x: u32,
    y: u32,
  ) -> core::result::Result<Option<PremultipliedColorU8>, console_core_never::Never> {
    let Ok(width) = self.width();
    let idx = match match width.checked_mul(y) {
      Some(value) => value,
      None => return Ok(None),
    }
    .checked_add(x)
    {
      Some(value) => value,
      None => return Ok(None),
    };

    let Ok(pixels) = self.pixels();

    Ok(
      pixels
        .get({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index(idx);
          numeric_value
        })
        .cloned(),
    )
  }

  pub fn pixels(
    &self,
  ) -> core::result::Result<&'a [PremultipliedColorU8], console_core_never::Never> {
    let Ok(data) = self.data();

    Ok(bytemuck::cast_slice(data))
  }

  pub fn clone_rect(&self, rect: IntRect) -> Result<Option<Pixmap>, Never> {
    let Ok(visible) = visible_rect(self.size, &rect);
    let crop = match visible {
      Some(bounds) => bounds,
      None => return Ok(None),
    };
    let Ok(size) = crop.size();
    let Ok(pixmap) = Pixmap::with_size(size);
    let mut image = match pixmap {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(original_width) = self.width();
    let Ok(crop_width) = crop.width();
    let Ok(crop_height) = crop.height();
    let stride = u64::from(original_width).saturating_mul(BYTES_IN_PIXEL);
    let bytes_per_row = u64::from(crop_width).saturating_mul(BYTES_IN_PIXEL);
    let Ok(pixel_offset) = offset_in_rect(original_width, &crop);
    let origin = pixel_offset.saturating_mul(BYTES_IN_PIXEL);

    for row in 0..u64::from(crop_height) {
      let source = origin.saturating_add(row.saturating_mul(stride));
      let output = row.saturating_mul(bytes_per_row);
      let Ok(source_start) = index(source);
      let Ok(source_end) = index(source.saturating_add(bytes_per_row));
      let Ok(target_start) = index(output);
      let Ok(target_end) = index(output.saturating_add(bytes_per_row));

      match (
        self.data.get(source_start..source_end),
        image.data.get_mut(target_start..target_end),
      ) {
        (Some(from), Some(to)) => {
          for (pixel, byte) in to.iter_mut().zip(from) {
            *pixel = *byte;
          }
        }
        _ => return Ok(None),
      }
    }

    Ok(Some(image))
  }

  #[cfg(feature = "png-format")]
  pub fn encode_png(&self) -> Result<Vec<u8>, std::io::Error> {
    let Ok(demultiplied_data) = ({
      let Ok(explicit_value) = self.to_owned();
      explicit_value
    })
    .take_demultiplied();

    let Ok(width) = self.width();
    let Ok(height) = self.height();
    console_core_png_files::encoded_rgba(
      console_core_size::Size { width, height },
      &demultiplied_data,
    )
    .map_err(std::io::Error::other)
  }

  #[cfg(feature = "png-format")]
  pub fn save_png<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), std::io::Error> {
    let data = self.encode_png()?;
    console_core_atomic_writes::whole(path.as_ref(), &data).map_err(std::io::Error::other)?;
    Ok(())
  }
}

impl core::fmt::Debug for PixmapRef<'_> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("PixmapRef")
      .field("data", &"...")
      .field(
        "width",
        &({
          let Ok(explicit_value) = self.size.width();
          explicit_value
        }),
      )
      .field(
        "height",
        &({
          let Ok(explicit_value) = self.size.height();
          explicit_value
        }),
      )
      .finish()
  }
}

#[derive(PartialEq)]
pub struct PixmapMut<'a> {
  data: &'a mut [u8],
  size: IntSize,
}

impl<'a> PixmapMut<'a> {
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "existing pixel buffer views take width then height"
    )
  )]
  pub fn from_bytes(data: &'a mut [u8], width: u32, height: u32) -> Result<Option<Self>, Never> {
    let Ok(dimensions) = IntSize::from_wh(width, height);
    let size = match dimensions {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(required) = data_len_for_size(size);
    let Ok(available) = fixed_index(data.len());

    Ok(match required {
      Some(bytes) => (bytes <= available).then_some(Self { data, size }),
      None => None,
    })
  }

  pub fn to_owned(&self) -> core::result::Result<Pixmap, console_core_never::Never> {
    Ok(Pixmap {
      data: self.data.to_vec(),
      size: self.size,
    })
  }

  pub fn as_ref(&self) -> core::result::Result<PixmapRef<'_>, console_core_never::Never> {
    Ok(PixmapRef {
      data: self.data,
      size: self.size,
    })
  }

  #[inline]
  pub fn width(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(width) = self.size.width();

    Ok(width)
  }

  #[inline]
  pub fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(height) = self.size.height();

    Ok(height)
  }

  pub(crate) fn size(&self) -> core::result::Result<IntSize, console_core_never::Never> {
    Ok(self.size)
  }

  pub fn fill(&mut self, color: Color) -> core::result::Result<(), console_core_never::Never> {
    let Ok(premultiply) = color.premultiply();
    let Ok(c) = premultiply.to_color_u8();

    let Ok(pixels_mut) = self.pixels_mut();

    for p in pixels_mut {
      *p = c;
    }

    Ok(())
  }

  pub fn data_mut(&mut self) -> core::result::Result<&mut [u8], console_core_never::Never> {
    Ok(self.data)
  }

  pub fn pixels_mut(
    &mut self,
  ) -> core::result::Result<&mut [PremultipliedColorU8], console_core_never::Never> {
    let Ok(data_mut) = self.data_mut();

    Ok(bytemuck::cast_slice_mut(data_mut))
  }

  pub(crate) fn as_subpixmap(
    &mut self,
  ) -> core::result::Result<SubPixmapMut<'_>, console_core_never::Never> {
    let Ok(size) = self.size();
    let Ok(width) = self.width();
    let real_width = width;

    Ok(SubPixmapMut {
      size,
      real_width,
      data: self.data,
    })
  }

  pub(crate) fn subpixmap(&mut self, rect: IntRect) -> Result<Option<SubPixmapMut<'_>>, Never> {
    let Ok(visible) = visible_rect(self.size, &rect);
    let slice = match visible {
      Some(bounds) => bounds,
      None => return Ok(None),
    };
    let Ok(real_width) = self.width();
    let Ok(offset) = offset_in_rect(real_width, &slice);
    let Ok(byte_start) = index(offset.saturating_mul(BYTES_IN_PIXEL));
    let Ok(size) = slice.size();

    Ok(self.data.get_mut(byte_start..).map(|data| SubPixmapMut {
      size,
      real_width,
      data,
    }))
  }
}

impl core::fmt::Debug for PixmapMut<'_> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("PixmapMut")
      .field("data", &"...")
      .field(
        "width",
        &({
          let Ok(explicit_value) = self.size.width();
          explicit_value
        }),
      )
      .field(
        "height",
        &({
          let Ok(explicit_value) = self.size.height();
          explicit_value
        }),
      )
      .finish()
  }
}

pub struct SubPixmapMut<'a> {
  pub data: &'a mut [u8],
  pub size: IntSize,
  pub real_width: u32,
}

impl SubPixmapMut<'_> {
  pub fn pixels_mut(
    &mut self,
  ) -> core::result::Result<&mut [PremultipliedColorU8], console_core_never::Never> {
    Ok(bytemuck::cast_slice_mut(self.data))
  }
}

const BYTES_IN_PIXEL: u64 = 4;
#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  expect(
    explicit051_no_machine_width,
    reason = "Vec byte capacity is limited by the host allocator's maximum signed address offset"
  )
)]
const MAX_BUFFER_LENGTH: console_core_api_compatibility::NativeSize = isize::MAX.unsigned_abs();

fn data_len_for_size(size: IntSize) -> Result<Option<u64>, Never> {
  let Ok((width, height)) = size.dimensions();
  let row_bytes = u64::from(width).saturating_mul(BYTES_IN_PIXEL);

  Ok(
    (row_bytes <= u64::from(i32::MAX.unsigned_abs()))
      .then_some(row_bytes.saturating_mul(u64::from(height))),
  )
}

pub(crate) fn zeroed_pixel_data(count: u64) -> Result<Option<Vec<u8>>, Never> {
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "Vec reservation and resizing take a checked host-sized byte count"
    )
  )]
  let length = match checked_index(count) {
    Ok(bytes) => bytes,
    Err(_outside_address_space) => return Ok(None),
  };

  match (length <= MAX_BUFFER_LENGTH).then_some(()) {
    Some(()) => {}
    None => return Ok(None),
  }

  let mut data = Vec::new();

  match data.try_reserve_exact(length.min(MAX_BUFFER_LENGTH)) {
    Ok(()) => {}
    Err(error) => {
      console_core_logging::warn!("could not allocate {count} bytes for raster pixels: {error}");

      return Ok(None);
    }
  }

  data.resize(length.min(data.capacity()), 0);

  Ok(Some(data))
}
