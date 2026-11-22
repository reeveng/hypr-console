use console_core_never::Never;

use crate::LengthU32;
use crate::alpha_runs::AlphaRun;
use crate::color::AlphaU8;
use crate::geom::{PixelPosition, ScreenIntRect};

pub struct Mask {
  pub image: [u8; 2],
  pub bounds: ScreenIntRect,
  pub row_bytes: u32,
}

pub trait SpanBlitter {
  fn blit_h(&mut self, at: PixelPosition, width: LengthU32) -> Result<(), Never>;
}

pub trait CoverageBlitter {
  fn blit_anti_h(
    &mut self,
    at: PixelPosition,
    antialias: &mut [AlphaU8],
    runs: &mut [AlphaRun],
  ) -> Result<(), Never>;

  fn blit_v(&mut self, at: PixelPosition, height: LengthU32, alpha: AlphaU8)
  -> Result<(), Never>;

  fn blit_anti_h2(&mut self, at: PixelPosition, alpha: [AlphaU8; 2]) -> Result<(), Never>;

  fn blit_anti_v2(&mut self, at: PixelPosition, alpha: [AlphaU8; 2]) -> Result<(), Never>;
}

pub trait Blitter: SpanBlitter + CoverageBlitter {
  fn blit_rect(&mut self, rect: &ScreenIntRect) -> Result<(), Never>;
}
