use crate::CoverageMode;
#[cfg(all(not(feature = "std"), feature = "no-std-float"))]
use console_core_paths::NoStdFloat;

use alloc::vec;
use alloc::vec::Vec;

use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index};
use console_core_paths::{IntRect, IntSize, Path, Transform};

use crate::geom::{IntSizeExt, offset_in_rect, visible_rect};
use crate::painter::{DrawTiler, filled_path, tile_transforms};
use crate::pipeline::RasterPipelineBlitter;
use crate::pixmap::{SubPixmapMut, zeroed_pixel_data};
use crate::scan;
use crate::{FillRule, PixmapRef};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MaskType {
  Alpha,

  Luminance,
}

#[derive(Clone, PartialEq)]
pub struct Mask {
  data: Vec<u8>,
  size: IntSize,
}

impl Mask {
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
      Some(dimensions) => {
        let Ok(area) = mask_area(dimensions);
        let Ok(data) = zeroed_pixel_data(area);

        Ok(data.map(|bytes| Self {
          data: bytes,
          size: dimensions,
        }))
      }
      None => Ok(None),
    }
  }

  pub fn with_size(size: IntSize) -> Result<Self, Never> {
    let Ok(area) = mask_area(size);
    let Ok(length) = index(area);
    #[cfg_attr(
      dylint_lib = "explicit088_no_arriving_length",
      expect(
        explicit088_no_arriving_length,
        reason = "the infallible constructor allocates one coverage byte per pixel in the caller's explicit image size"
      )
    )]
    let data = vec![0; length];

    Ok(Self { data, size })
  }

  pub fn from_pixmap(
    pixmap: PixmapRef,
    mask_type: MaskType,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(size) = pixmap.size();
    let Ok(mut mask) = Self::with_size(size);

    match mask_type {
      MaskType::Alpha => {
        for (p, a) in ({
          let Ok(explicit_value) = pixmap.pixels();
          explicit_value
        })
        .iter()
        .zip(mask.data.as_mut_slice())
        {
          *a = {
            let Ok(explicit_value) = p.alpha();
            explicit_value
          };
        }
      }
      MaskType::Luminance => {
        for (p, ma) in ({
          let Ok(explicit_value) = pixmap.pixels();
          explicit_value
        })
        .iter()
        .zip(mask.data.as_mut_slice())
        {
          let mut r = f32::from({
            let Ok(explicit_value) = p.red();
            explicit_value
          }) / 255.0;
          let mut g = f32::from({
            let Ok(explicit_value) = p.green();
            explicit_value
          }) / 255.0;
          let mut b = f32::from({
            let Ok(explicit_value) = p.blue();
            explicit_value
          }) / 255.0;
          let a = f32::from({
            let Ok(explicit_value) = p.alpha();
            explicit_value
          }) / 255.0;

          match ::core::primitive::bool::then_some(
            ({
              let Ok(explicit_value) = p.alpha();
              explicit_value
            }) != 0,
            (),
          ) {
            ::core::option::Option::Some(()) => {
              r /= a;
              g /= a;
              b /= a;
            }
            ::core::option::Option::None => {}
          }

          let luma = r * 0.2126 + g * 0.7152 + b * 0.0722;
          *ma = {
            let Ok(numeric_value) = console_core_number_conversion::toward_zero_u8(f64::from(
              ((luma * a) * 255.0).clamp(0.0, 255.0).ceil(),
            ));
            numeric_value
          };
        }
      }
    }

    Ok(mask)
  }

  pub fn from_vec(data: Vec<u8>, size: IntSize) -> Result<Option<Self>, Never> {
    let Ok(expected) = mask_area(size);
    let Ok(length) = fixed_index(data.len());

    Ok((length == expected).then_some(Self { data, size }))
  }

  #[inline]
  pub fn width(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok({
      let Ok(explicit_value) = self.size.width();
      explicit_value
    })
  }

  #[inline]
  pub fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok({
      let Ok(explicit_value) = self.size.height();
      explicit_value
    })
  }

  pub(crate) fn size(&self) -> core::result::Result<IntSize, console_core_never::Never> {
    Ok(self.size)
  }

  pub fn data(&self) -> core::result::Result<&[u8], console_core_never::Never> {
    Ok(self.data.as_slice())
  }

  pub fn data_mut(&mut self) -> core::result::Result<&mut [u8], console_core_never::Never> {
    Ok(self.data.as_mut_slice())
  }

  pub fn take(self) -> core::result::Result<Vec<u8>, console_core_never::Never> {
    Ok(self.data)
  }

  pub(crate) fn as_submask(
    &self,
  ) -> core::result::Result<SubMaskRef<'_>, console_core_never::Never> {
    Ok(SubMaskRef {
      size: self.size,
      real_width: ({
        let Ok(explicit_value) = self.size.width();
        explicit_value
      }),
      data: &self.data,
    })
  }

  pub(crate) fn submask(&self, rect: IntRect) -> Result<Option<SubMaskRef<'_>>, Never> {
    let Ok(visible) = visible_rect(self.size, &rect);
    let view = match visible {
      Some(bounds) => bounds,
      None => return Ok(None),
    };
    let Ok(real_width) = self.width();
    let Ok(offset) = offset_in_rect(real_width, &view);
    let Ok(start) = index(offset);
    let Ok(size) = view.size();

    Ok(self.data.get(start..).map(|data| SubMaskRef {
      data,
      size,
      real_width,
    }))
  }

  pub(crate) fn as_subpixmap(
    &mut self,
  ) -> core::result::Result<SubPixmapMut<'_>, console_core_never::Never> {
    Ok(SubPixmapMut {
      size: self.size,
      real_width: {
        let Ok(width) = self.width();
        width
      },
      data: &mut self.data,
    })
  }

  pub(crate) fn subpixmap(&mut self, rect: IntRect) -> Result<Option<SubPixmapMut<'_>>, Never> {
    let Ok(visible) = visible_rect(self.size, &rect);
    let view = match visible {
      Some(bounds) => bounds,
      None => return Ok(None),
    };
    let Ok(real_width) = self.width();
    let Ok(offset) = offset_in_rect(real_width, &view);
    let Ok(start) = index(offset);
    let Ok(size) = view.size();

    Ok(self.data.get_mut(start..).map(|data| SubPixmapMut {
      data,
      size,
      real_width,
    }))
  }

  #[cfg(feature = "png-format")]
  pub fn decode_png(data: &[u8]) -> Result<Self, std::io::Error> {
    let (dimensions, bytes) = console_core_png_files::decoded_greyscale(data)
      .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let Ok(decoded_size) = IntSize::from_wh(dimensions.width, dimensions.height);
    let size = decoded_size.ok_or(std::io::ErrorKind::InvalidData)?;
    let Ok(image) = Self::from_vec(bytes, size);
    image.ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))
  }

  #[cfg(feature = "png-format")]
  pub fn load_png<P: AsRef<std::path::Path>>(path: P) -> Result<Self, std::io::Error> {
    let data = std::fs::read(path)?;
    Self::decode_png(&data)
  }

  #[cfg(feature = "png-format")]
  pub fn encode_png(&self) -> Result<Vec<u8>, std::io::Error> {
    let Ok(width) = self.width();
    let Ok(height) = self.height();
    console_core_png_files::encoded_greyscale(
      console_core_size::Size { width, height },
      &self.data,
    )
    .map_err(std::io::Error::other)
  }

  #[cfg(feature = "png-format")]
  pub fn save_png<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), std::io::Error> {
    let data = self.encode_png()?;
    console_core_atomic_writes::whole(path.as_ref(), &data).map_err(std::io::Error::other)?;
    Ok(())
  }

  pub fn fill_path(
    &mut self,
    path: &Path,
    fill_rule: FillRule,
    coverage: CoverageMode,
    transform: Transform,
  ) -> Result<(), Never> {
    let Ok(answer) = filled_path(path, transform);
    let region = match answer {
      Some(value) => value,
      None => return Ok(()),
    };
    let Ok(tiler) = DrawTiler::new(self.size);

    match tiler {
      Some(tiles) => self.fill_tiles(region.into_owned(), fill_rule, coverage, tiles),
      None => {
        let Ok(mut surface) = self.as_subpixmap();

        fill_mask(&region, fill_rule, coverage, &mut surface)
      }
    }
  }

  fn fill_tiles(
    &mut self,
    mut path: Path,
    fill_rule: FillRule,
    coverage: CoverageMode,
    tiles: DrawTiler,
  ) -> Result<(), Never> {
    for tile in tiles {
      let Ok([to_tile, from_tile]) = tile_transforms(&tile);
      let Ok(translated) = path.transform(to_tile);
      let geometry = match translated {
        Some(value) => value,
        None => {
          console_core_logging::warn!("path transformation failed");

          return Ok(());
        }
      };
      let Ok(bounds) = tile.to_int_rect();
      let Ok(surface) = self.subpixmap(bounds);

      match surface {
        Some(mut target) => {
          let Ok(()) = fill_mask(&geometry, fill_rule, coverage, &mut target);
        }
        None => {}
      }

      let Ok(restored) = geometry.transform(from_tile);
      path = match restored {
        Some(value) => value,
        None => return Ok(()),
      };
    }

    Ok(())
  }

  pub fn intersect_path(
    &mut self,
    path: &Path,
    fill_rule: FillRule,
    coverage: CoverageMode,
    transform: Transform,
  ) -> core::result::Result<(), console_core_never::Never> {
    let Ok(area) = mask_area(self.size);
    let Ok(allocated) = zeroed_pixel_data(area);
    let mut submask = match allocated {
      Some(data) => Self {
        data,
        size: self.size,
      },
      None => return Ok(()),
    };
    let Ok(()) = submask.fill_path(path, fill_rule, coverage, transform);

    for (a, b) in self.data.iter_mut().zip(submask.data.iter()) {
      *a = {
        let Ok(explicit_value) = crate::color::Alpha(*b).premultiply(*a);
        explicit_value
      };
    }

    Ok(())
  }

  pub fn invert(&mut self) -> core::result::Result<(), console_core_never::Never> {
    self
      .data
      .iter_mut()
      .for_each(|a| *a = u8::wrapping_sub(255, *a));

    Ok(())
  }

  pub fn clear(&mut self) -> core::result::Result<(), console_core_never::Never> {
    self.data.fill(0);

    Ok(())
  }
}

fn mask_area(size: IntSize) -> Result<u64, Never> {
  let Ok((width, height)) = size.dimensions();

  Ok(u64::from(width).saturating_mul(u64::from(height)))
}

fn fill_mask(
  path: &Path,
  rule: FillRule,
  coverage: CoverageMode,
  surface: &mut SubPixmapMut<'_>,
) -> Result<(), Never> {
  let Ok(bounds) = surface.size.at_origin();
  let clip = match bounds {
    Some(value) => value,
    None => return Ok(()),
  };
  let Ok(backend) = RasterPipelineBlitter::new_mask(surface);

  match backend {
    Some(mut blitter) => scan::fill_path(path, rule, coverage, &clip, &mut blitter),
    None => Ok(()),
  }
}

impl core::fmt::Debug for Mask {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("Mask")
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

#[derive(Clone, Copy)]
pub struct SubMaskRef<'a> {
  pub data: &'a [u8],
  pub size: IntSize,
  pub real_width: u32,
}

impl<'a> SubMaskRef<'a> {
  pub(crate) fn mask_ctx(
    &self,
  ) -> core::result::Result<crate::pipeline::MaskCtx<'a>, console_core_never::Never> {
    Ok(crate::pipeline::MaskCtx {
      data: self.data,
      real_width: self.real_width,
    })
  }
}
