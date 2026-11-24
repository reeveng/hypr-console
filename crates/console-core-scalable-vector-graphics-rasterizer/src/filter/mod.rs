//! Copyright 2018 the Resvg Authors
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use console_core_never::Never;
use console_core_number_conversion::{
  index, nearest_f32_from_i64, nearest_f32_from_u64, toward_zero_i32, toward_zero_u8,
};
use console_core_path_rasterizer::{FloatRectangle, Paint, Pixmap, Transform};
use console_core_paths::{NonZeroRect, Point};
use console_core_pixel_buffers::{Rgba8, rgba_pixels, rgba_pixels_mut};
use console_core_scalable_vector_graphics::console_core_paths::{StrictNormalized, StrictNum};
use console_core_scalable_vector_graphics::filter::{
  ColorInterpolation, Filter, Input, Kind, Primitive,
};
use console_core_size::Size;
use std::rc::Rc;

use console_core_path_rasterizer::IntRect;
use console_core_scalable_vector_graphics::{ApproxEqUlps, ApproxZeroUlps, ZeroProximity};

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "Gaussian deviations are measured in output pixels along each axis"
  )
)]
struct BlurDeviation {
  horizontal: f64,
  vertical: f64,
}

type PixelPosition = console_core_point::Point<u32>;

#[derive(Clone, Copy)]
enum ImageAxis {
  Rows,
  Columns,
}

struct ImageLines {
  length: u32,
  count: u32,
  pixel_step: u32,
  line_step: u32,
}

impl ImageAxis {
  fn lines(self, size: Size<u32>) -> Result<ImageLines, console_core_never::Never> {
    Ok(match self {
      Self::Rows => ImageLines {
        length: size.width,
        count: size.height,
        pixel_step: 1,
        line_step: size.width,
      },
      Self::Columns => ImageLines {
        length: size.height,
        count: size.width,
        pixel_step: size.width,
        line_step: 1,
      },
    })
  }
}

mod box_blur;
mod color_matrix;
mod component_transfer;
mod composite;
pub(super) mod convolve_matrix;

mod displacement_map;
mod iir_blur;
mod lighting;
mod morphology;
mod turbulence;

pub(super) type Convolve<'a> = dyn Fn(
    &console_core_scalable_vector_graphics::filter::ConvolveMatrix,
    ImageRefMut<'_>,
  ) -> Result<(), Error>
  + 'a;

#[doc = "A borrowed RGBA image. The filter determines whether it expects premultiplied channels."]
#[derive(Clone, Copy)]
pub struct ImageRef<'a> {
  data: &'a [Rgba8],
  width: u32,
  height: u32,
}

impl<'a> ImageRef<'a> {
  fn new(size: Size<u32>, data: &'a [Rgba8]) -> Result<Self, Never> {
    Ok(Self {
      data,
      width: size.width,
      height: size.height,
    })
  }

  fn dimensions(&self) -> Result<Size<u32>, Error> {
    checked_dimensions(
      Size {
        width: self.width,
        height: self.height,
      },
      self.data,
    )
  }

  #[inline]
  fn alpha_at(&self, position: PixelPosition) -> Result<i16, console_core_never::Never> {
    let Ok(sample) = self.sample(position);
    Ok(match sample {
      Some([_, _, _, alpha]) => i16::from(alpha),
      None => 0,
    })
  }

  fn sample(&self, position: PixelPosition) -> Result<Option<Rgba8>, console_core_never::Never> {
    match (position.x < self.width && position.y < self.height).then_some(()) {
      Some(()) => {
        let Ok(offset) = pixel_offset(position, self.width);
        let Ok(location) = index(offset);
        Ok(self.data.get(location).copied())
      }
      None => Ok(None),
    }
  }
}

#[doc = "A mutable RGBA image buffer."]
pub struct ImageRefMut<'a> {
  data: &'a mut [Rgba8],
  width: u32,
  height: u32,
}

impl<'a> ImageRefMut<'a> {
  fn dimensions(&self) -> Result<Size<u32>, Error> {
    checked_dimensions(
      Size {
        width: self.width,
        height: self.height,
      },
      self.data,
    )
  }

  fn new(size: Size<u32>, data: &'a mut [Rgba8]) -> Result<Self, Never> {
    Ok(Self {
      data,
      width: size.width,
      height: size.height,
    })
  }

  fn borrowed(&self) -> Result<ImageRef<'_>, console_core_never::Never> {
    ImageRef::new(
      Size {
        width: self.width,
        height: self.height,
      },
      self.data,
    )
  }
}

fn checked_dimensions(size: Size<u32>, pixels: &[Rgba8]) -> Result<Size<u32>, Error> {
  let Ok(length) = console_core_number_conversion::fixed_index(pixels.len());
  let Ok(area) = size.area();

  match (area == length).then_some(()) {
    Some(()) => Ok(size),
    None => Err(Error::InvalidRegion),
  }
}

fn pixel_offset(position: PixelPosition, row_width: u32) -> Result<u64, Never> {
  Ok(
    u64::from(position.y)
      .saturating_mul(u64::from(row_width))
      .saturating_add(u64::from(position.x)),
  )
}

#[derive(Debug)]
pub(crate) enum Error {
  InvalidRegion,
  NoResults,
}

trait PixmapExt: Sized {
  fn try_create(width: u32, height: u32) -> Result<Pixmap, Error>;

  fn copy_region(&self, region: IntRect) -> Result<Pixmap, Error>;
}

impl PixmapExt for Pixmap {
  fn try_create(width: u32, height: u32) -> Result<Pixmap, Error> {
    let Ok(allocated) = Pixmap::new(width, height);
    allocated.ok_or(Error::InvalidRegion)
  }

  fn copy_region(&self, region: IntRect) -> Result<Pixmap, Error> {
    let Ok(copied) = self.clone_rect(region);
    copied.ok_or(Error::InvalidRegion)
  }
}

fn convert_color_space(
  pixmap: &mut Pixmap,
  destination: ColorInterpolation,
) -> Result<(), Never> {
  let Ok(data) = pixmap.data_mut();
  let Ok(pixels) = rgba_pixels_mut(data);
  let Ok(()) = demultiply_alpha(pixels);

  match destination {
    ColorInterpolation::SRGB => {
      let Ok(()) = from_linear_rgb(pixels);
    }
    ColorInterpolation::LinearRGB => {
      let Ok(()) = into_linear_rgb(pixels);
    }
  }

  multiply_alpha(pixels)
}

fn multiply_alpha(data: &mut [Rgba8]) -> Result<(), Never> {
  for [red, green, blue, alpha] in data {
    let opacity = f32::from(*alpha) / 255.0;

    for channel in [red, green, blue] {
      let Ok(value) = toward_zero_u8(f64::from(f32::from(*channel) * opacity + 0.5));
      *channel = value;
    }
  }

  Ok(())
}

fn demultiply_alpha(data: &mut [Rgba8]) -> Result<(), Never> {
  for [red, green, blue, alpha] in data {
    let opacity = f32::from(*alpha) / 255.0;

    for channel in [red, green, blue] {
      let Ok(value) = toward_zero_u8(f64::from(f32::from(*channel) / opacity + 0.5));
      *channel = value;
    }
  }

  Ok(())
}

const SRGB_TO_LINEAR_RGB_TABLE: &[u8; 256] = &[
  0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3,
  4, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6, 7, 7, 7, 8, 8, 8, 8, 9, 9, 9, 10, 10, 10, 11, 11, 12,
  12, 12, 13, 13, 13, 14, 14, 15, 15, 16, 16, 17, 17, 17, 18, 18, 19, 19, 20, 20, 21, 22, 22, 23,
  23, 24, 24, 25, 25, 26, 27, 27, 28, 29, 29, 30, 30, 31, 32, 32, 33, 34, 35, 35, 36, 37, 37, 38,
  39, 40, 41, 41, 42, 43, 44, 45, 45, 46, 47, 48, 49, 50, 51, 51, 52, 53, 54, 55, 56, 57, 58, 59,
  60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 76, 77, 78, 79, 80, 81, 82, 84, 85,
  86, 87, 88, 90, 91, 92, 93, 95, 96, 97, 99, 100, 101, 103, 104, 105, 107, 108, 109, 111, 112,
  114, 115, 116, 118, 119, 121, 122, 124, 125, 127, 128, 130, 131, 133, 134, 136, 138, 139, 141,
  142, 144, 146, 147, 149, 151, 152, 154, 156, 157, 159, 161, 163, 164, 166, 168, 170, 171, 173,
  175, 177, 179, 181, 183, 184, 186, 188, 190, 192, 194, 196, 198, 200, 202, 204, 206, 208, 210,
  212, 214, 216, 218, 220, 222, 224, 226, 229, 231, 233, 235, 237, 239, 242, 244, 246, 248, 250,
  253, 255,
];

#[doc = "Precomputed LinearRGB to sRGB table."]
#[doc = ""]
#[doc = "Since we are storing the result in `u8`, there is no need to compute those"]
#[doc = "values each time. Mainly because it's very expensive."]
#[doc = ""]
#[doc = "```text"]
#[doc = "if (C_lin <= 0.0031308)"]
#[doc = "    C_srgb = C_lin * 12.92;"]
#[doc = "else"]
#[doc = "    C_srgb = 1.055 * pow(C_lin, 1.0 / 2.4) - 0.055;"]
#[doc = "```"]
#[doc = ""]
#[doc = "Thanks to librsvg for the idea."]
#[rustfmt::skip]
const LINEAR_RGB_TO_SRGB_TABLE: &[u8; 256] = &[
    0,  13,  22,  28,  34,  38,  42,  46,  50,  53,  56,  59,  61,  64,  66,  69,
    71,  73,  75,  77,  79,  81,  83,  85,  86,  88,  90,  92,  93,  95,  96,  98,
    99, 101, 102, 104, 105, 106, 108, 109, 110, 112, 113, 114, 115, 117, 118, 119,
    120, 121, 122, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136,
    137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 148, 149, 150, 151,
    152, 153, 154, 155, 155, 156, 157, 158, 159, 159, 160, 161, 162, 163, 163, 164,
    165, 166, 167, 167, 168, 169, 170, 170, 171, 172, 173, 173, 174, 175, 175, 176,
    177, 178, 178, 179, 180, 180, 181, 182, 182, 183, 184, 185, 185, 186, 187, 187,
    188, 189, 189, 190, 190, 191, 192, 192, 193, 194, 194, 195, 196, 196, 197, 197,
    198, 199, 199, 200, 200, 201, 202, 202, 203, 203, 204, 205, 205, 206, 206, 207,
    208, 208, 209, 209, 210, 210, 211, 212, 212, 213, 213, 214, 214, 215, 215, 216,
    216, 217, 218, 218, 219, 219, 220, 220, 221, 221, 222, 222, 223, 223, 224, 224,
    225, 226, 226, 227, 227, 228, 228, 229, 229, 230, 230, 231, 231, 232, 232, 233,
    233, 234, 234, 235, 235, 236, 236, 237, 237, 238, 238, 238, 239, 239, 240, 240,
    241, 241, 242, 242, 243, 243, 244, 244, 245, 245, 246, 246, 246, 247, 247, 248,
    248, 249, 249, 250, 250, 251, 251, 251, 252, 252, 253, 253, 254, 254, 255, 255,
];

#[doc = "Converts input pixel from sRGB into LinearRGB."]
#[doc = ""]
#[doc = "Provided pixels should have an **unpremultiplied alpha**."]
#[doc = ""]
#[doc = "RGB channels order of the input image doesn't matter, but alpha channel must be the last one."]
fn into_linear_rgb(data: &mut [Rgba8]) -> Result<(), Never> {
  convert_channels(data, SRGB_TO_LINEAR_RGB_TABLE)
}

fn from_linear_rgb(data: &mut [Rgba8]) -> Result<(), Never> {
  convert_channels(data, LINEAR_RGB_TO_SRGB_TABLE)
}

fn convert_channels(data: &mut [Rgba8], table: &[u8; 256]) -> Result<(), Never> {
  for [red, green, blue, _alpha] in data {
    for channel in [red, green, blue] {
      let Ok(location) = index(u64::from(*channel));

      match table.get(location) {
        Some(corrected) => *channel = *corrected,
        None => {}
      }
    }
  }

  Ok(())
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "A filter channel is a ratio relative to a fully lit sample"
  )
)]
struct ChannelValue(f32);

impl ChannelValue {
  fn from_byte(byte: u8) -> Result<Self, console_core_never::Never> {
    Ok(Self(f32::from(byte) / 255.0))
  }

  fn clamped(self, Self(upper): Self) -> Result<Self, console_core_never::Never> {
    let value = self.0;
    Ok(Self(match (value > upper, value >= 0.0) {
      (true, _) => upper,
      (false, true) => value,
      (false, false) => 0.0,
    }))
  }

  fn quantized(self) -> Result<u8, console_core_never::Never> {
    let Ok(Self(bounded)) = self.clamped(Self(1.0));
    console_core_number_conversion::toward_zero_u8(f64::from(bounded * 255.0))
  }
}

#[derive(Clone)]
struct Image {
  #[doc = "Immutable pixels that can be named by several completed filter primitives."]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Several later filter primitives can read the same immutable pixmap. Mutation requires taking or copying the pixmap first"
    )
  )]
  image: Rc<Pixmap>,
  #[doc = "The nontransparent region in global coordinates, used by the tile primitive."]
  region: IntRect,
  color_space: ColorInterpolation,
}

impl Image {
  fn from_image(image: Pixmap, color_space: ColorInterpolation) -> Result<Self, Error> {
    let Ok(width) = image.width();
    let Ok(height) = image.height();
    let Ok(bounds) = IntRect::from_xywh(0, 0, width, height);
    let region = bounds.ok_or(Error::InvalidRegion)?;
    let Ok(image_2) = Self::with_region(image, region, color_space);

    Ok(image_2)
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The reference-counted pixmap is only read by stages that reference this completed filter result"
    )
  )]
  fn with_region(
    image: Pixmap,
    region: IntRect,
    color_space: ColorInterpolation,
  ) -> Result<Self, Never> {
    Ok(Self {
      image: Rc::new(image),
      region,
      color_space,
    })
  }

  fn into_color_space(self, color_space: ColorInterpolation) -> Result<Self, Error> {
    match (color_space == self.color_space).then_some(()) {
      Some(()) => return Ok(self),
      None => {}
    }

    let region = self.region;
    let mut image = self.take()?;

    let Ok(()) = convert_color_space(&mut image, color_space);

    let Ok(converted) = Self::with_region(image, region, color_space);

    Ok(converted)
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Attempt to take the only reference to this immutable result before allocating a private editable copy"
    )
  )]
  fn take(self) -> Result<Pixmap, Error> {
    match Rc::try_unwrap(self.image) {
      Ok(pixmap) => Ok(pixmap),
      Err(shared) => copy_pixmap(&shared),
    }
  }

  fn as_pixmap(&self) -> Result<&Pixmap, Never> {
    Ok(&self.image)
  }
}

fn copy_pixmap(image: &Pixmap) -> Result<Pixmap, Error> {
  let Ok(width) = image.width();
  let Ok(height) = image.height();
  let mut duplicate = Pixmap::try_create(width, height)?;
  let Ok(input) = image.data();
  let Ok(output) = duplicate.data_mut();

  for (target, sample) in output.iter_mut().zip(input) {
    *target = *sample;
  }

  Ok(duplicate)
}

enum ClipOrigin {
  Filter,
  Primitive,
}

struct FilterResult {
  name: String,
  image: Image,
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Filter image children are rendered with the inherited, bounded context"
  )
)]
pub(super) fn apply(
  filter: &Filter,
  context: &crate::render::Context<'_>,
  transform: Transform,
  source: &mut Pixmap,
) -> Result<(), Never> {
  let result = match apply_inner(filter, context, transform, source) {
    Ok(image) => apply_to_canvas(image, source),
    Err(failure) => Err(failure),
  };

  match result {
    Ok(()) => {}
    Err(problem) => {
      let Ok(()) = source.fill(console_core_path_rasterizer::Color::TRANSPARENT);

      match problem {
        Error::InvalidRegion => console_core_logging::warn!("Filter has an invalid region."),
        Error::NoResults => {}
      }
    }
  }

  Ok(())
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Every stage receives the current context for bounded nested SVG paint operations"
  )
)]
fn apply_inner(
  filter: &Filter,
  context: &crate::render::Context<'_>,
  transform: Transform,
  source: &Pixmap,
) -> Result<Image, Error> {
  let Ok(bounds) = filter.rect();
  let region = transformed_region(bounds, transform)?;
  let Ok(width) = source.width();
  let Ok(height) = source.height();
  let Ok(bounds_2) = IntRect::from_xywh(0, 0, width, height);
  let source_region = bounds_2.ok_or(Error::InvalidRegion)?;
  let Ok(clipped) = crate::geom::fit_to_rect(region, source_region);
  let region_2 = clipped.ok_or(Error::InvalidRegion)?;
  let Ok(primitives) = filter.primitives();
  let mut results = Vec::new();

  for primitive in primitives {
    let stage = FilterStage {
      context,
      transform,
      region: region_2,
      source,
      results: &results,
    };
    let image = stage.run(primitive)?;
    let Ok(reference) = primitive.result();
    results.push(FilterResult {
      name: reference.to_owned(),
      image,
    });
  }

  match results.pop() {
    Some(result) => Ok(result.image),
    None => Err(Error::NoResults),
  }
}

struct FilterStage<'a> {
  context: &'a crate::render::Context<'a>,
  transform: Transform,
  region: IntRect,
  source: &'a Pixmap,
  results: &'a [FilterResult],
}

impl FilterStage<'_> {
  fn input(&self, input: &Input) -> Result<Image, Error> {
    get_input(input, self.region, self.source, self.results)
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "The image primitive retains the filter stage’s inherited depth budget"
    )
  )]
  fn run(&self, primitive: &Primitive) -> Result<Image, Error> {
    let Ok(rectangle) = primitive.rect();
    let mut subregion = transformed_region(rectangle, self.transform)?;
    let mut clip_origin = ClipOrigin::Primitive;
    let Ok(cs) = primitive.color_interpolation();
    let Ok(kind) = primitive.kind();

    let image = match kind {
      Kind::Blend(fe) => {
        let Ok(fe_input1) = fe.input1();
        let input1 = self.input(fe_input1)?;
        let Ok(fe_input2) = fe.input2();
        let input2 = self.input(fe_input2)?;
        apply_blend(fe, cs, self.region, input1, input2)
      }
      Kind::DropShadow(fe) => {
        let Ok(fe_input) = fe.input();
        let input = self.input(fe_input)?;
        apply_drop_shadow(fe, cs, self.transform, input)
      }
      Kind::Flood(fe) => apply_flood(fe, self.region),
      Kind::GaussianBlur(fe) => {
        let Ok(fe_input) = fe.input();
        let input = self.input(fe_input)?;
        apply_blur(fe, cs, self.transform, input)
      }
      Kind::Offset(fe) => {
        clip_origin = ClipOrigin::Filter;
        let Ok(reference) = fe.input();

        let Ok(local) = offset_region(reference, subregion, self.results);
        subregion = local;
        let input = self.input(reference)?;
        apply_offset(fe, self.transform, input)
      }
      Kind::Composite(fe) => {
        let Ok(fe_input1) = fe.input1();
        let input1 = self.input(fe_input1)?;
        let Ok(fe_input2) = fe.input2();
        let input2 = self.input(fe_input2)?;
        apply_composite(fe, cs, self.region, input1, input2)
      }
      Kind::Merge(fe) => apply_merge(fe, cs, self.region, self.source, self.results),
      Kind::Tile(fe) => {
        let Ok(fe_input) = fe.input();
        let input = self.input(fe_input)?;
        apply_tile(input, self.region)
      }
      Kind::Image(fe) => apply_image(fe, self.region, subregion, self.transform, self.context),
      Kind::ComponentTransfer(fe) => {
        let Ok(fe_input) = fe.input();
        let input = self.input(fe_input)?;
        apply_component_transfer(fe, cs, input)
      }
      Kind::ColorMatrix(fe) => {
        let Ok(fe_input) = fe.input();
        let input = self.input(fe_input)?;
        apply_color_matrix(fe, cs, input)
      }
      Kind::ConvolveMatrix(fe) => {
        let Ok(fe_input) = fe.input();
        let input = self.input(fe_input)?;
        apply_convolve_matrix(fe, cs, input, self.context.convolve)
      }
      Kind::Morphology(fe) => {
        let Ok(fe_input) = fe.input();
        let input = self.input(fe_input)?;
        apply_morphology(fe, cs, self.transform, input)
      }
      Kind::DisplacementMap(fe) => {
        let Ok(fe_input1) = fe.input1();
        let input1 = self.input(fe_input1)?;
        let Ok(fe_input2) = fe.input2();
        let input2 = self.input(fe_input2)?;
        apply_displacement_map(fe, self.region, cs, self.transform, input1, input2)
      }
      Kind::Turbulence(fe) => apply_turbulence(fe, self.region, cs, self.transform),
      Kind::DiffuseLighting(fe) => {
        let Ok(fe_input) = fe.input();
        let input = self.input(fe_input)?;
        apply_diffuse_lighting(fe, self.region, cs, self.transform, input)
      }
      Kind::SpecularLighting(fe) => {
        let Ok(fe_input) = fe.input();
        let input = self.input(fe_input)?;
        apply_specular_lighting(fe, self.region, cs, self.transform, input)
      }
    }?;

    match (self.region == subregion).then_some(()) {
      Some(()) => Ok(image),
      None => clip_image(image, self.region, subregion, clip_origin),
    }
  }
}

fn transformed_region(rect: NonZeroRect, transform: Transform) -> Result<IntRect, Error> {
  let Ok(mapped) = rect.transform(transform);
  let mapped_2 = mapped.ok_or(Error::InvalidRegion)?;
  let Ok(pixel_bounds) = mapped_2.to_int_rect();

  pixel_bounds.ok_or(Error::InvalidRegion)
}

fn clip_image(
  image: Image,
  region: IntRect,
  subregion: IntRect,
  origin: ClipOrigin,
) -> Result<Image, Error> {
  let bounds = match origin {
    ClipOrigin::Filter => {
      let Ok(local) = region.translate_to(0, 0);
      local.ok_or(Error::InvalidRegion)?
    }
    ClipOrigin::Primitive => local_region(subregion, region)?,
  };
  let color_space = image.color_space;
  let mut pixmap = image.take()?;
  let Ok(()) = clear_outside(&mut pixmap, bounds);

  let Ok(clipped) = Image::with_region(pixmap, subregion, color_space);

  Ok(clipped)
}

fn clear_outside(canvas: &mut Pixmap, kept: IntRect) -> Result<(), Never> {
  let Ok(pixel_width) = canvas.width();
  let Ok(pixel_height) = canvas.height();
  let Ok(width) = nearest_f32_from_u64(u64::from(pixel_width));
  let Ok(height) = nearest_f32_from_u64(u64::from(pixel_height));
  let Ok(left) = kept.x();
  let Ok(top) = kept.y();
  let Ok(right) = kept.right();
  let Ok(bottom) = kept.bottom();
  let Ok(left_2) = nearest_f32_from_i64(i64::from(left));
  let Ok(top_2) = nearest_f32_from_i64(i64::from(top));
  let Ok(right_2) = nearest_f32_from_i64(i64::from(right));
  let Ok(bottom_2) = nearest_f32_from_i64(i64::from(bottom));
  let mut paint = Paint::default();
  let Ok(()) = paint.set_color(console_core_path_rasterizer::Color::BLACK);
  paint.blend_mode = console_core_path_rasterizer::BlendMode::Clear;
  let Ok(identity) = Transform::identity();

  for [x, y, w, h] in [
    [0.0, 0.0, width, top_2],
    [0.0, 0.0, left_2, height],
    [right_2, 0.0, width, height],
    [0.0, bottom_2, width, height],
  ] {
    let Ok(affected) = FloatRectangle::from_xywh(x, y, w, h);

    match affected {
      Some(rect) => {
        let Ok(()) = canvas.fill_rect(rect, &paint, identity, None);
      }
      None => {}
    }
  }

  Ok(())
}

#[must_use]
fn offset_region(
  input: &Input,
  fallback: IntRect,
  results: &[FilterResult],
) -> Result<IntRect, Never> {
  let name = match input {
    Input::Reference(name) => name,
    Input::SourceGraphic | Input::SourceAlpha => return Ok(fallback),
  };
  let Ok(reference) = reference_result(results, name);

  Ok(match reference {
    Some(result) => result.image.region,
    None => fallback,
  })
}

fn reference_result<'a>(
  results: &'a [FilterResult],
  name: &str,
) -> Result<Option<&'a FilterResult>, Never> {
  Ok(results.iter().rev().find(|result| result.name == name))
}

fn get_input(
  input: &Input,
  region: IntRect,
  source: &Pixmap,
  results: &[FilterResult],
) -> Result<Image, Error> {
  let image = match input {
    Input::SourceGraphic => copy_pixmap(source)?,
    Input::SourceAlpha => {
      let mut alpha = copy_pixmap(source)?;
      let Ok(data) = alpha.data_mut();
      let Ok(pixels) = rgba_pixels_mut(data);

      for [red, green, blue, _alpha] in pixels {
        *red = 0;
        *green = 0;
        *blue = 0;
      }

      alpha
    }
    Input::Reference(name) => {
      let Ok(referenced) = reference_result(results, name);

      match referenced {
        Some(result) => return Ok(result.image.clone()),
        None => {
          console_core_logging::warn!("Unknown filter primitive reference '{}'.", name);
          copy_pixmap(source)?
        }
      }
    }
  };

  let Ok(source_image) = Image::with_region(image, region, ColorInterpolation::SRGB);

  Ok(source_image)
}

trait PixmapToImageRef<'a> {
  fn as_image_ref(&'a self) -> ImageRef<'a>;

  fn as_image_ref_mut(&'a mut self) -> ImageRefMut<'a>;
}

impl<'a> PixmapToImageRef<'a> for console_core_path_rasterizer::Pixmap {
  fn as_image_ref(&'a self) -> ImageRef<'a> {
    let Ok(width) = self.width();
    let Ok(height) = self.height();
    let Ok(data) = self.data();
    let Ok(pixels) = rgba_pixels(data);

    let Ok(view) = ImageRef::new(Size { width, height }, pixels);
    view
  }

  fn as_image_ref_mut(&'a mut self) -> ImageRefMut<'a> {
    let Ok(width) = self.width();
    let Ok(height) = self.height();
    let Ok(data_mut) = self.data_mut();
    let Ok(pixels) = rgba_pixels_mut(data_mut);

    let Ok(view) = ImageRefMut::new(Size { width, height }, pixels);
    view
  }
}

fn apply_drop_shadow(
  shadow: &console_core_scalable_vector_graphics::filter::DropShadow,
  color_space: ColorInterpolation,
  transform: Transform,
  input: Image,
) -> Result<Image, Error> {
  let Ok(x) = shadow.dx();
  let Ok(y) = shadow.dy();
  let Ok(offset) = scale_coordinates(Point { x, y }, transform);
  let original = input.into_color_space(color_space)?;
  let original_2 = original.take()?;
  let mut shadow_pixels = copy_pixmap(&original_2)?;
  let Ok(horizontal) = shadow.std_dev_x();
  let Ok(vertical) = shadow.std_dev_y();
  let Ok(blur) = resolve_std_dev(
    Point {
      x: horizontal.get(),
      y: vertical.get(),
    },
    transform,
  );

  match blur {
    Some(blur_2) => blur_image(blur_2, &mut shadow_pixels)?,
    None => {}
  }

  let Ok(()) = tint_shadow(shadow, &mut shadow_pixels);
  let Ok(()) = convert_color_space(&mut shadow_pixels, color_space);
  let Ok(width) = original_2.width();
  let Ok(height) = original_2.height();
  let mut output = Pixmap::try_create(width, height)?;
  let Ok(()) = draw_shifted(&mut output, &shadow_pixels, offset);
  let Ok(()) = draw_shifted(&mut output, &original_2, Point::default());

  Image::from_image(output, color_space)
}

fn tint_shadow(
  shadow: &console_core_scalable_vector_graphics::filter::DropShadow,
  pixmap: &mut Pixmap,
) -> Result<(), Never> {
  use console_core_path_rasterizer::Color;

  let Ok(rgb) = shadow.color();
  let Ok(opacity) = shadow.opacity();
  let Ok(base_color) = Color::from_rgba8(rgb.red, rgb.green, rgb.blue, opacity.to_u8());
  let Ok(pixels) = pixmap.pixels_mut();

  for pixel in pixels {
    let mut tint = base_color;
    let Ok(alpha) = pixel.alpha();
    let Ok(()) = tint.apply_opacity(f32::from(alpha) / 255.0);
    let Ok(premultiplied) = tint.premultiply();
    let Ok(encoded) = premultiplied.to_color_u8();
    *pixel = encoded;
  }

  Ok(())
}

fn draw_shifted(target: &mut Pixmap, source: &Pixmap, offset: Point) -> Result<(), Never> {
  let Ok(dx) = toward_zero_i32(f64::from(offset.x));
  let Ok(dy) = toward_zero_i32(f64::from(offset.y));
  let Ok(view) = source.as_ref();
  let Ok(identity) = Transform::identity();

  target.draw_pixmap(
    dx,
    dy,
    view,
    &console_core_path_rasterizer::PixmapPaint::default(),
    identity,
    None,
  )
}

fn apply_blur(
  filter: &console_core_scalable_vector_graphics::filter::GaussianBlur,
  color_space: ColorInterpolation,
  transform: Transform,
  input: Image,
) -> Result<Image, Error> {
  let Ok(horizontal) = filter.std_dev_x();
  let Ok(vertical) = filter.std_dev_y();
  let Ok(blur) = resolve_std_dev(
    Point {
      x: horizontal.get(),
      y: vertical.get(),
    },
    transform,
  );
  let blur_3 = match blur {
    Some(blur_2) => blur_2,
    None => return Ok(input),
  };

  let image = input.into_color_space(color_space)?;
  let mut pixmap = image.take()?;
  blur_image(blur_3, &mut pixmap)?;

  Image::from_image(pixmap, color_space)
}

fn apply_offset(
  filter: &console_core_scalable_vector_graphics::filter::Offset,
  transform: Transform,
  input: Image,
) -> Result<Image, Error> {
  let Ok(x) = filter.dx();
  let Ok(y) = filter.dy();
  let Ok(offset) = scale_coordinates(Point { x, y }, transform);

  match (offset.x.zero_proximity(4), offset.y.zero_proximity(4)) {
    (ZeroProximity::NearZero, ZeroProximity::NearZero) => return Ok(input),
    _ => {}
  }

  let Ok(original) = input.as_pixmap();
  let Ok(width) = original.width();
  let Ok(height) = original.height();
  let mut shifted = Pixmap::try_create(width, height)?;
  let Ok(()) = draw_shifted(&mut shifted, original, offset);

  Image::from_image(shifted, input.color_space)
}

fn apply_blend(
  fe: &console_core_scalable_vector_graphics::filter::Blend,
  cs: console_core_scalable_vector_graphics::filter::ColorInterpolation,
  region: IntRect,
  input1: Image,
  input2: Image,
) -> Result<Image, Error> {
  let input1_2 = input1.into_color_space(cs)?;
  let input2_2 = input2.into_color_space(cs)?;

  let Ok(width) = region.width();
  let Ok(height) = region.height();
  let Ok(svg_blend) = fe.mode();
  let Ok(blend) = crate::render::convert_blend_mode(svg_blend);
  let mut pixmap = console_core_path_rasterizer::Pixmap::try_create(width, height)?;

  let ::core::result::Result::Ok(()) = pixmap.draw_pixmap(
    0,
    0,
    {
      let Ok(source_image) = input2_2.as_pixmap();
      let Ok(explicit_value) = source_image.as_ref();
      explicit_value
    },
    &console_core_path_rasterizer::PixmapPaint::default(),
    {
      let Ok(explicit_value) = console_core_path_rasterizer::Transform::identity();
      explicit_value
    },
    None,
  );

  let ::core::result::Result::Ok(()) = pixmap.draw_pixmap(
    0,
    0,
    {
      let Ok(source_image) = input1_2.as_pixmap();
      let Ok(explicit_value) = source_image.as_ref();
      explicit_value
    },
    &console_core_path_rasterizer::PixmapPaint {
      blend_mode: blend,
      ..console_core_path_rasterizer::PixmapPaint::default()
    },
    {
      let Ok(explicit_value) = console_core_path_rasterizer::Transform::identity();
      explicit_value
    },
    None,
  );

  Image::from_image(pixmap, cs)
}

fn apply_composite(
  fe: &console_core_scalable_vector_graphics::filter::Composite,
  cs: console_core_scalable_vector_graphics::filter::ColorInterpolation,
  region: IntRect,
  input1: Image,
  input2: Image,
) -> Result<Image, Error> {
  use console_core_scalable_vector_graphics::filter::CompositeOperator as Operator;

  let input1_2 = input1.into_color_space(cs)?;
  let input2_2 = input2.into_color_space(cs)?;

  let Ok(width) = region.width();
  let Ok(height) = region.height();
  let mut pixmap = console_core_path_rasterizer::Pixmap::try_create(width, height)?;

  let Ok(operator) = fe.operator();

  let blend_mode = match operator {
    Operator::Arithmetic { k1, k2, k3, k4 } => {
      let pixmap1 = input1_2.take()?;
      let pixmap2 = input2_2.take()?;

      composite::arithmetic(
        composite::Equation {
          product: k1,
          first: k2,
          second: k3,
          constant: k4,
        },
        pixmap1.as_image_ref(),
        pixmap2.as_image_ref(),
        pixmap.as_image_ref_mut(),
      )?;

      return Image::from_image(pixmap, cs);
    }
    Operator::Over => console_core_path_rasterizer::BlendMode::SourceOver,
    Operator::In => console_core_path_rasterizer::BlendMode::SourceIn,
    Operator::Out => console_core_path_rasterizer::BlendMode::SourceOut,
    Operator::Atop => console_core_path_rasterizer::BlendMode::SourceAtop,
    Operator::Xor => console_core_path_rasterizer::BlendMode::Xor,
  };

  let ::core::result::Result::Ok(()) = pixmap.draw_pixmap(
    0,
    0,
    {
      let Ok(source_image) = input2_2.as_pixmap();
      let Ok(explicit_value) = source_image.as_ref();
      explicit_value
    },
    &console_core_path_rasterizer::PixmapPaint::default(),
    {
      let Ok(explicit_value) = console_core_path_rasterizer::Transform::identity();
      explicit_value
    },
    None,
  );

  let ::core::result::Result::Ok(()) = pixmap.draw_pixmap(
    0,
    0,
    {
      let Ok(source_image) = input1_2.as_pixmap();
      let Ok(explicit_value) = source_image.as_ref();
      explicit_value
    },
    &console_core_path_rasterizer::PixmapPaint {
      blend_mode,
      ..console_core_path_rasterizer::PixmapPaint::default()
    },
    {
      let Ok(explicit_value) = console_core_path_rasterizer::Transform::identity();
      explicit_value
    },
    None,
  );

  Image::from_image(pixmap, cs)
}

fn apply_merge(
  fe: &console_core_scalable_vector_graphics::filter::Merge,
  cs: console_core_scalable_vector_graphics::filter::ColorInterpolation,
  region: IntRect,
  source: &console_core_path_rasterizer::Pixmap,
  results: &[FilterResult],
) -> Result<Image, Error> {
  let Ok(width) = region.width();
  let Ok(height) = region.height();
  let mut pixmap = console_core_path_rasterizer::Pixmap::try_create(width, height)?;

  let Ok(inputs) = fe.inputs();

  for input in inputs {
    let input_2 = get_input(input, region, source, results)?;
    let input_3 = input_2.into_color_space(cs)?;
    let ::core::result::Result::Ok(()) = pixmap.draw_pixmap(
      0,
      0,
      {
        let Ok(source_image) = input_3.as_pixmap();
        let Ok(explicit_value) = source_image.as_ref();
        explicit_value
      },
      &console_core_path_rasterizer::PixmapPaint::default(),
      {
        let Ok(explicit_value) = console_core_path_rasterizer::Transform::identity();
        explicit_value
      },
      None,
    );
  }

  Image::from_image(pixmap, cs)
}

fn apply_flood(
  fe: &console_core_scalable_vector_graphics::filter::Flood,
  region: IntRect,
) -> Result<Image, Error> {
  let Ok(c) = fe.color();

  let Ok(width) = region.width();
  let Ok(height) = region.height();
  let mut pixmap = console_core_path_rasterizer::Pixmap::try_create(width, height)?;
  let Ok(opacity) = fe.opacity();
  let Ok(color) =
    console_core_path_rasterizer::Color::from_rgba8(c.red, c.green, c.blue, opacity.to_u8());
  let Ok(()) = pixmap.fill(color);

  Image::from_image(
    pixmap,
    console_core_scalable_vector_graphics::filter::ColorInterpolation::SRGB,
  )
}

fn apply_tile(input: Image, region: IntRect) -> Result<Image, Error> {
  use console_core_path_rasterizer::{FilterQuality, Pattern, SpreadMode};

  let subregion = local_region(input.region, region)?;
  let Ok(original) = input.as_pixmap();
  let tile = original.copy_region(subregion)?;
  let Ok(view) = tile.as_ref();
  let Ok(offset) = origin_in_path_units(subregion);
  let Ok(placement) = Transform::from_translation(offset);
  let Ok(shader) = Pattern::new(
    view,
    SpreadMode::Repeat,
    FilterQuality::Bicubic,
    1.0,
    placement,
  );
  let paint = Paint {
    shader,
    ..Paint::default()
  };
  let Ok(width) = region.width();
  let Ok(height) = region.height();
  let mut canvas = Pixmap::try_create(width, height)?;
  let Ok(local) = region.translate_to(0, 0);
  let local_2 = local.ok_or(Error::InvalidRegion)?;
  let Ok(rect) = local_2.to_rect();
  let Ok(identity) = Transform::identity();
  let Ok(()) = canvas.fill_rect(rect, &paint, identity, None);

  Image::from_image(canvas, ColorInterpolation::SRGB)
}

fn local_region(rect: IntRect, origin: IntRect) -> Result<IntRect, Error> {
  let Ok(x) = origin.x();
  let Ok(y) = origin.y();
  let horizontal = x.checked_neg().ok_or(Error::InvalidRegion)?;
  let vertical = y.checked_neg().ok_or(Error::InvalidRegion)?;
  let Ok(relative) = rect.translate(horizontal, vertical);

  relative.ok_or(Error::InvalidRegion)
}

fn origin_in_path_units(rect: IntRect) -> Result<Point, Never> {
  let Ok(left) = rect.x();
  let Ok(top) = rect.y();
  let Ok(x) = nearest_f32_from_i64(i64::from(left));
  let Ok(y) = nearest_f32_from_i64(i64::from(top));

  Ok(Point { x, y })
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Filter image children inherit this render context and each new child consumes one unit"
  )
)]
fn apply_image(
  image: &console_core_scalable_vector_graphics::filter::Image,
  region: IntRect,
  subregion: IntRect,
  transform: Transform,
  context: &crate::render::Context<'_>,
) -> Result<Image, Error> {
  let Ok(width) = region.width();
  let Ok(height) = region.height();
  let mut pixmap = Pixmap::try_create(width, height)?;
  let Ok((sx, sy)) = transform.get_scale();
  let Ok(left) = subregion.x();
  let Ok(top) = subregion.y();
  let Ok(x) = nearest_f32_from_i64(i64::from(left));
  let Ok(y) = nearest_f32_from_i64(i64::from(top));
  let Ok(placement) = Transform::from_row(sx, 0.0, 0.0, sy, x, y);

  let Ok(bounds) = IntRect::from_xywh(0, 0, width, height);
  let bounds_2 = bounds.ok_or(Error::InvalidRegion)?;
  let Ok(context_2) = context.with_bounds(bounds_2);
  let Ok(root) = image.root();
  let Ok(mut view) = pixmap.as_mut();
  let Ok(()) = crate::render::render_nodes(root, &context_2, placement, &mut view);

  Image::from_image(pixmap, ColorInterpolation::SRGB)
}

fn apply_component_transfer(
  fe: &console_core_scalable_vector_graphics::filter::ComponentTransfer,
  cs: console_core_scalable_vector_graphics::filter::ColorInterpolation,
  input: Image,
) -> Result<Image, Error> {
  let converted_input = input.into_color_space(cs)?;
  let mut pixmap = converted_input.take()?;

  let Ok(()) = demultiply_alpha(pixmap.as_image_ref_mut().data);
  let Ok(()) = component_transfer::apply(fe, pixmap.as_image_ref_mut());
  let Ok(()) = multiply_alpha(pixmap.as_image_ref_mut().data);

  Image::from_image(pixmap, cs)
}

fn apply_color_matrix(
  fe: &console_core_scalable_vector_graphics::filter::ColorMatrix,
  cs: console_core_scalable_vector_graphics::filter::ColorInterpolation,
  input: Image,
) -> Result<Image, Error> {
  let converted_input = input.into_color_space(cs)?;
  let mut pixmap = converted_input.take()?;

  let Ok(()) = demultiply_alpha(pixmap.as_image_ref_mut().data);
  let Ok(kind) = fe.kind();
  color_matrix::apply(kind, pixmap.as_image_ref_mut())?;
  let Ok(()) = multiply_alpha(pixmap.as_image_ref_mut().data);

  Image::from_image(pixmap, cs)
}

fn apply_convolve_matrix(
  fe: &console_core_scalable_vector_graphics::filter::ConvolveMatrix,
  cs: console_core_scalable_vector_graphics::filter::ColorInterpolation,
  input: Image,
  convolve: &Convolve<'_>,
) -> Result<Image, Error> {
  let converted_input = input.into_color_space(cs)?;
  let mut pixmap = converted_input.take()?;

  let Ok(alpha_mode) = fe.preserve_alpha();

  match ::core::primitive::bool::then_some(
    alpha_mode == console_core_scalable_vector_graphics::PreserveAlphaAnswer::Yes,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      let Ok(()) = demultiply_alpha(pixmap.as_image_ref_mut().data);
    }
    ::core::option::Option::None => {}
  }

  convolve(fe, pixmap.as_image_ref_mut())?;

  Image::from_image(pixmap, cs)
}

fn apply_morphology(
  fe: &console_core_scalable_vector_graphics::filter::Morphology,
  cs: console_core_scalable_vector_graphics::filter::ColorInterpolation,
  ts: console_core_scalable_vector_graphics::Transform,
  input: Image,
) -> Result<Image, Error> {
  let converted_input = input.into_color_space(cs)?;
  let mut pixmap = converted_input.take()?;

  let Ok(radius_x) = fe.radius_x();
  let Ok(radius_y) = fe.radius_y();
  let Ok(Point { x: rx, y: ry }) = scale_coordinates(
    Point {
      x: radius_x.get(),
      y: radius_y.get(),
    },
    ts,
  );

  match ::core::primitive::bool::then_some(!(rx > 0.0 && ry > 0.0), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(()) = pixmap.fill(console_core_path_rasterizer::Color::TRANSPARENT);
      return Image::from_image(pixmap, cs);
    }
    ::core::option::Option::None => {}
  }

  let Ok(operator) = fe.operator();
  morphology::apply(
    operator,
    morphology::Radii { x: rx, y: ry },
    pixmap.as_image_ref_mut(),
  )?;

  Image::from_image(pixmap, cs)
}

fn apply_displacement_map(
  fe: &console_core_scalable_vector_graphics::filter::DisplacementMap,
  region: IntRect,
  cs: console_core_scalable_vector_graphics::filter::ColorInterpolation,
  ts: console_core_scalable_vector_graphics::Transform,
  input1: Image,
  input2: Image,
) -> Result<Image, Error> {
  let converted_input1 = input1.into_color_space(cs)?;
  let pixmap1 = converted_input1.take()?;
  let converted_input2 = input2.into_color_space(cs)?;
  let pixmap2 = converted_input2.take()?;

  let Ok(width) = region.width();
  let Ok(height) = region.height();
  let mut pixmap = console_core_path_rasterizer::Pixmap::try_create(width, height)?;

  let Ok(scale) = fe.scale();
  let Ok(fe_scale) = fe.scale();
  let Ok(Point { x: sx, y: sy }) = scale_coordinates(
    Point {
      x: scale,
      y: fe_scale,
    },
    ts,
  );

  displacement_map::apply(
    fe,
    displacement_map::CanvasScale { x: sx, y: sy },
    pixmap1.as_image_ref(),
    pixmap2.as_image_ref(),
    pixmap.as_image_ref_mut(),
  )?;

  Image::from_image(pixmap, cs)
}

fn apply_turbulence(
  fe: &console_core_scalable_vector_graphics::filter::Turbulence,
  region: IntRect,
  cs: console_core_scalable_vector_graphics::filter::ColorInterpolation,
  ts: console_core_scalable_vector_graphics::Transform,
) -> Result<Image, Error> {
  let Ok(width) = region.width();
  let Ok(height) = region.height();
  let mut pixmap = console_core_path_rasterizer::Pixmap::try_create(width, height)?;

  let Ok((sx, sy)) = ts.get_scale();

  match ::core::primitive::bool::then_some(
    {
      matches!(
        sx.zero_proximity(4),
        console_core_scalable_vector_graphics::ZeroProximity::NearZero
      ) || matches!(
        sy.zero_proximity(4),
        console_core_scalable_vector_graphics::ZeroProximity::NearZero
      )
    },
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Image::from_image(pixmap, cs);
    }
    ::core::option::Option::None => {}
  }

  let Ok(x) = region.x();
  let Ok(y) = region.y();
  let Ok(frequency_x) = fe.base_frequency_x();
  let Ok(frequency_y) = fe.base_frequency_y();
  let Ok(num_octaves) = fe.num_octaves();
  let Ok(seed) = fe.seed();
  let Ok(stitch_tiles) = fe.stitch_tiles();
  let Ok(kind) = fe.kind();

  let settings = turbulence::Settings {
    offset: (
      f64::from(x) - f64::from(ts.tx),
      f64::from(y) - f64::from(ts.ty),
    ),
    scale: (f64::from(sx), f64::from(sy)),
    base_frequency: (f64::from(frequency_x.get()), f64::from(frequency_y.get())),
    num_octaves,
    seed,
    stitch_tiles,
    kind,
  };
  turbulence::apply(settings, pixmap.as_image_ref_mut())?;

  let Ok(()) = multiply_alpha(pixmap.as_image_ref_mut().data);

  Image::from_image(pixmap, cs)
}

fn apply_diffuse_lighting(
  fe: &console_core_scalable_vector_graphics::filter::DiffuseLighting,
  region: IntRect,
  cs: console_core_scalable_vector_graphics::filter::ColorInterpolation,
  ts: console_core_scalable_vector_graphics::Transform,
  input: Image,
) -> Result<Image, Error> {
  let Ok(width) = region.width();
  let Ok(height) = region.height();
  let mut pixmap = console_core_path_rasterizer::Pixmap::try_create(width, height)?;

  let Ok(fe_light_source) = fe.light_source();
  let Ok(light_source) = transform_light_source(fe_light_source, region, ts);

  lighting::diffuse_lighting(
    fe,
    light_source,
    {
      let Ok(image) = input.as_pixmap();
      image.as_image_ref()
    },
    pixmap.as_image_ref_mut(),
  )?;

  Image::from_image(pixmap, cs)
}

fn apply_specular_lighting(
  fe: &console_core_scalable_vector_graphics::filter::SpecularLighting,
  region: IntRect,
  cs: console_core_scalable_vector_graphics::filter::ColorInterpolation,
  ts: console_core_scalable_vector_graphics::Transform,
  input: Image,
) -> Result<Image, Error> {
  let Ok(width) = region.width();
  let Ok(height) = region.height();
  let mut pixmap = console_core_path_rasterizer::Pixmap::try_create(width, height)?;

  let Ok(fe_light_source) = fe.light_source();
  let Ok(light_source) = transform_light_source(fe_light_source, region, ts);

  lighting::specular_lighting(
    fe,
    light_source,
    {
      let Ok(image) = input.as_pixmap();
      image.as_image_ref()
    },
    pixmap.as_image_ref_mut(),
  )?;

  Image::from_image(pixmap, cs)
}

fn transform_light_source(
  mut source: console_core_scalable_vector_graphics::filter::LightSource,
  region: IntRect,
  transform: Transform,
) -> Result<console_core_scalable_vector_graphics::filter::LightSource, Never> {
  use console_core_scalable_vector_graphics::filter::LightSource;
  use std::f32::consts::SQRT_2;

  let Ok(offset) = origin_in_path_units(region);
  let world_scale = (transform.sx * transform.sx + transform.sy * transform.sy).sqrt();

  match &mut source {
    LightSource::DistantLight(_) => {}
    LightSource::PointLight(light) => {
      let mut point = Point {
        x: light.x,
        y: light.y,
      };
      let Ok(()) = transform.map_point(&mut point);
      light.x = point.x - offset.x;
      light.y = point.y - offset.y;
      light.z = light.z * world_scale / SQRT_2;
    }
    LightSource::SpotLight(light) => {
      let mut endpoints = [
        Point {
          x: light.x,
          y: light.y,
        },
        Point {
          x: light.points_at_x,
          y: light.points_at_y,
        },
      ];
      let Ok(()) = transform.map_points(&mut endpoints);
      let [location, aim] = endpoints;
      light.x = location.x - offset.x;
      light.y = location.y - offset.y;
      light.z *= world_scale / SQRT_2;
      light.points_at_x = aim.x - offset.x;
      light.points_at_y = aim.y - offset.y;
      light.points_at_z *= world_scale / SQRT_2;
    }
  }

  Ok(source)
}

fn apply_to_canvas(
  input: Image,
  pixmap: &mut console_core_path_rasterizer::Pixmap,
) -> Result<(), Error> {
  let input_2 = input
    .into_color_space(console_core_scalable_vector_graphics::filter::ColorInterpolation::SRGB)?;

  let Ok(()) = pixmap.fill(console_core_path_rasterizer::Color::TRANSPARENT);
  let ::core::result::Result::Ok(()) = pixmap.draw_pixmap(
    0,
    0,
    {
      let Ok(source_image) = input_2.as_pixmap();
      let Ok(explicit_value) = source_image.as_ref();
      explicit_value
    },
    &console_core_path_rasterizer::PixmapPaint::default(),
    {
      let Ok(explicit_value) = console_core_path_rasterizer::Transform::identity();
      explicit_value
    },
    None,
  );

  Ok(())
}

enum BlurAlgorithm {
  Box,
  Iir,
}

struct Blur {
  algorithm: BlurAlgorithm,
  deviation: BlurDeviation,
}

fn blur_image(blur: Blur, image: &mut Pixmap) -> Result<(), Error> {
  match blur.algorithm {
    BlurAlgorithm::Box => box_blur::apply(blur.deviation, image.as_image_ref_mut()),
    BlurAlgorithm::Iir => iir_blur::apply(blur.deviation, image.as_image_ref_mut()),
  }
}

#[doc = "Chooses a box filter for large deviations and a recursive filter for small deviations."]
fn resolve_std_dev(deviation: Point, transform: Transform) -> Result<Option<Blur>, Never> {
  let Ok(scaled) = scale_coordinates(deviation, transform);

  match (scaled.x.approx_eq_ulps(&0.0, 4) && scaled.y.approx_eq_ulps(&0.0, 4)).then_some(()) {
    Some(()) => return Ok(None),
    None => {}
  }

  let horizontal = match (scaled.x < 0.05).then_some(()) {
    Some(()) => 0.0,
    None => scaled.x,
  };
  let vertical = match (scaled.y < 0.05).then_some(()) {
    Some(()) => 0.0,
    None => scaled.y,
  };

  const BOX_BLUR_THRESHOLD_IN_PIXELS: f32 = 2.0;
  let algorithm = match (horizontal >= BOX_BLUR_THRESHOLD_IN_PIXELS
    || vertical >= BOX_BLUR_THRESHOLD_IN_PIXELS)
    .then_some(())
  {
    Some(()) => BlurAlgorithm::Box,
    None => BlurAlgorithm::Iir,
  };

  Ok(Some(Blur {
    algorithm,
    deviation: BlurDeviation {
      horizontal: f64::from(horizontal),
      vertical: f64::from(vertical),
    },
  }))
}

fn scale_coordinates(point: Point, transform: Transform) -> Result<Point, Never> {
  let Ok((sx, sy)) = transform.get_scale();

  Ok(Point {
    x: point.x * sx,
    y: point.y * sy,
  })
}
