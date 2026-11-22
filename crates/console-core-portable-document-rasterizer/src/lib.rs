/*!
A crate for rendering PDF files.

This crate allows you to render pages of a PDF file into bitmaps. It is supposed to be relatively
lightweight, since we do not have any dependencies on the GPU. All the rendering happens on the CPU.

The ultimate goal of this crate is to be a *feature-complete* and *performant* PDF rasterizer.
With that said, we are currently still very far away from reaching that goal: So far, no effort
has been put into performance optimizations, as we are still working on implementing missing features.
However, this crate is currently the most comprehensive and feature-complete
implementation of a PDF rasterizer in pure Rust. This claim is supported by the fact that we currently
include over 1000 PDF files in our regression test suite. The majority of those have been scraped
from the `pdf.js` and `PDFBOX` test suites and therefore represent a very large and diverse sample
of PDF files.

As mentioned, there are still some serious limitations, including lack of support for
encrypted/password-protected PDF files, blending and isolation, knockout groups as well as a range
of smaller features such as color key masking. But you should be able to render the vast majority
of PDF files without too many issues.

## Safety
This crate forbids unsafe code via a crate-level attribute.

## Examples
For usage examples, see the [example](https://github.com/LaurenzV/console_core_portable_document_rasterizer/tree/master/console_core_portable_document_rasterizer/examples) in
the GitHub repository.

## Cargo features
This crate has one optional feature:
- `embed-fonts`: See the description of [`console_core_portable_document_rasterizer-interpret`](https://docs.rs/console_core_portable_document_rasterizer-interpret/latest/console_core_portable_document_interpreter/#cargo-features) for more information.
*/

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use crate::renderer::Renderer;
use console_core_never::Never;
use console_core_number_conversion::{fitted, toward_zero_u16};
use console_core_portable_document_interpreter::Device;
use console_core_portable_document_interpreter::FillRule;
use console_core_portable_document_interpreter::InterpreterCache;
use console_core_portable_document_interpreter::InterpreterSettings;
use console_core_portable_document_interpreter::console_core_portable_document_syntax::Pdf;
use console_core_portable_document_interpreter::console_core_portable_document_syntax::page::{
  Page, PageCoordinates,
};
use console_core_portable_document_interpreter::util::{RectExt, TransformExt};
use console_core_portable_document_interpreter::{BlendMode, Context};
use console_core_portable_document_interpreter::{ClipPath, interpret_page};
use console_core_size::Size;
use kurbo::{Affine, Rect, Shape};
use std::collections::HashMap;
use std::ops::RangeInclusive;

pub use console_core_portable_document_interpreter;
pub use console_core_portable_document_interpreter::console_core_portable_document_syntax;
pub use vello_cpu;

use vello_cpu::color::AlphaColor;
use vello_cpu::color::Srgb;
use vello_cpu::color::palette::css::TRANSPARENT;
use vello_cpu::color::palette::css::WHITE;
use vello_cpu::{Level, Pixmap, RenderMode};

mod renderer;

#[doc = "Limit a tiling pattern to the requested number of device pixels."]
pub use renderer::{PatternStep, TileCell, tiling_cell_scale};

const MAX_VIEWPORT_PIXELS: u64 = 1 << 28;

#[doc = "Caches PDF interpretation and glyph outlines between renders of one document."]
#[derive(Clone, Default)]
pub struct RenderCache<'a> {
  interpreter_cache: InterpreterCache<'a>,
  outlines: HashMap<u128, kurbo::BezPath>,
}

impl<'a> RenderCache<'a> {
  #[doc = "Start with empty caches for a PDF."]
  pub fn new() -> Result<Self, Never> {
    let Ok(interpreter_cache) = InterpreterCache::new();

    Ok(Self {
      interpreter_cache,
      outlines: HashMap::new(),
    })
  }
}

#[doc = "Device pixels per PDF point."]
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped float represents a ratio of device pixels to PDF points"
  )
)]
pub struct ScaleFactor(#[doc = "The scale ratio."] pub f32);

#[doc = "A coordinate offset in device pixels."]
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the wrapped float is an offset measured in device pixels"
  )
)]
pub struct ViewOffset(#[doc = "The offset."] pub f32);

#[doc = "Settings to apply during rendering."]
#[derive(Clone, Copy)]
pub struct RenderSettings {
  #[doc = "Horizontal scale from page points to device pixels."]
  pub x_scale: ScaleFactor,
  #[doc = "Vertical scale from page points to device pixels."]
  pub y_scale: ScaleFactor,
  #[doc = "Viewport width in pixels. Derive it from the page size when absent."]
  pub width: Option<u16>,
  #[doc = "Viewport height in pixels. Derive it from the page size when absent."]
  pub height: Option<u16>,
  #[doc = "Color initially filling the viewport."]
  pub bg_color: AlphaColor<Srgb>,
  #[doc = "Horizontal position of the viewport on the scaled page."]
  pub x_offset: ViewOffset,
  #[doc = "Vertical position of the viewport on the scaled page."]
  pub y_offset: ViewOffset,
}

impl Default for RenderSettings {
  fn default() -> Self {
    Self {
      x_scale: ScaleFactor(1.0),
      y_scale: ScaleFactor(1.0),
      width: None,
      height: None,
      bg_color: TRANSPARENT,
      x_offset: ViewOffset(0.0),
      y_offset: ViewOffset(0.0),
    }
  }
}

#[doc = "An unsupported rasterization viewport."]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderError {
  #[doc = "One of the viewport dimensions is zero."]
  Empty,
  #[doc = "The viewport area exceeds the rasterizer's allocation limit."]
  TooLarge,
}

impl std::fmt::Display for RenderError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Empty => to.write_str("the page has an empty rendering viewport"),
      Self::TooLarge => to.write_str("the page rendering viewport exceeds the pixel limit"),
    }
  }
}

impl std::error::Error for RenderError {}

#[doc = "Render a single page to a bounded pixmap, reusing the document cache."]
pub fn render<'a>(
  page: &'a Page<'a>,
  cache: &mut RenderCache<'a>,
  interpreter_settings: &InterpreterSettings,
  settings: &RenderSettings,
) -> Result<Pixmap, RenderError> {
  let Ok((width, height)) = page.render_dimensions();
  let viewport = viewport(Size { width, height }, settings)?;
  let x_scale = f64::from(settings.x_scale.0);
  let y_scale = f64::from(settings.y_scale.0);
  let Ok(page_transform) = page.initial_transform(PageCoordinates::TopLeft);
  let initial_transform = Affine::translate((
    f64::from(-settings.x_offset.0),
    f64::from(-settings.y_offset.0),
  )) * Affine::scale_non_uniform(x_scale, y_scale)
    * page_transform.to_kurbo();
  let canvas = Rect::new(
    0.0,
    0.0,
    f64::from(viewport.width),
    f64::from(viewport.height),
  );
  let Ok(xref) = page.xref();
  let Ok(mut state) = Context::new(
    initial_transform,
    canvas,
    &cache.interpreter_cache,
    xref,
    interpreter_settings.clone(),
  );

  let vello_settings = vello_cpu::RenderSettings {
    level: Level::new(),
    num_threads: 0,
    render_mode: RenderMode::OptimizeSpeed,
  };
  let Ok(mut device) = Renderer::new(viewport, vello_settings, &mut cache.outlines);

  device.ctx.set_paint(settings.bg_color);
  device.ctx.fill_rect(&canvas);
  let Ok(intersected_crop_box) = page.intersected_crop_box();
  let mut clip_path = intersected_crop_box.to_kurbo().to_path(0.1);
  clip_path.apply_affine(initial_transform);
  device.push_clip_path(&ClipPath {
    path: clip_path,
    fill: FillRule::NonZero,
  });

  device.push_transparency_group(1.0, None, BlendMode::Normal);
  let Ok(()) = interpret_page(page, &mut state, &mut device);
  device.pop_transparency_group();
  device.pop_clip_path();

  let mut pixmap = Pixmap::new(viewport.width, viewport.height);
  let mut resources = vello_cpu::Resources::default();
  device.ctx.render_to_pixmap(&mut resources, &mut pixmap);

  Ok(pixmap)
}

fn viewport(page: Size<f32>, settings: &RenderSettings) -> Result<Size<u16>, RenderError> {
  let width = match settings.width {
    Some(width) => width,
    None => {
      let Ok(derived) = toward_zero_u16(f64::from(page.width * settings.x_scale.0).floor());

      derived
    }
  };
  let height = match settings.height {
    Some(height) => height,
    None => {
      let Ok(derived) = toward_zero_u16(f64::from(page.height * settings.y_scale.0).floor());

      derived
    }
  };

  match (width, height) {
    (0, _) | (_, 0) => return Err(RenderError::Empty),
    _ => {}
  }

  let area = u64::from(width).saturating_mul(u64::from(height));

  match (area > MAX_VIEWPORT_PIXELS).then_some(()) {
    Some(()) => Err(RenderError::TooLarge),
    None => Ok(Size { width, height }),
  }
}

#[doc(hidden)]
pub fn render_pdf(
  pdf: &Pdf,
  scale: ScaleFactor,
  settings: InterpreterSettings,
  range: Option<RangeInclusive<u32>>,
) -> Result<Vec<Pixmap>, RenderError> {
  let Ok(mut cache) = RenderCache::new();
  let Ok(pages) = pdf.pages();
  let rendering = RenderSettings {
    x_scale: scale,
    y_scale: scale,
    bg_color: WHITE,
    ..Default::default()
  };
  let mut pixmaps = Vec::new();

  for (index, page) in pages.iter().enumerate() {
    let Ok(number) = fitted(index);

    match range
      .as_ref()
      .filter(|selected| !selected.contains(&number))
    {
      Some(_) => continue,
      None => {}
    }

    let pixmap = render(page, &mut cache, &settings, &rendering)?;
    pixmaps.push(pixmap);
  }

  Ok(pixmaps)
}

pub(crate) fn derive_settings(
  settings: &vello_cpu::RenderSettings,
) -> Result<vello_cpu::RenderSettings, Never> {
  Ok(vello_cpu::RenderSettings {
    num_threads: 0,
    ..*settings
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_viewport_refuses_empty_and_oversized_pixmaps() {
    let settings = RenderSettings::default();

    assert_eq!(
      viewport(
        Size {
          width: 200.75,
          height: 100.5
        },
        &settings
      ),
      Ok(Size {
        width: 200,
        height: 100
      }),
    );
    assert_eq!(
      viewport(
        Size {
          width: 0.5,
          height: 100.0
        },
        &settings
      ),
      Err(RenderError::Empty),
    );
    assert_eq!(
      viewport(
        Size {
          width: 65_535.0,
          height: 65_535.0
        },
        &settings
      ),
      Err(RenderError::TooLarge),
    );
  }
}
