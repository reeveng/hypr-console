//! The format-specific decoders behind a common byte-to-pixel interface.

use console_core_pixel_buffers::PackedPixels;
use console_core_size::Size;

pub use console_core_picture_sizes::{PictureFormat as Format, format};

#[derive(Debug)]
pub enum DecodeError {
  Jpeg(console_core_jpeg_files::JpegError),
  Png(console_core_png_files::PngError),
  Gif(console_core_gif_files::GifError),
  Webp(console_core_webp_files::WebpError),
}

impl std::fmt::Display for DecodeError {
  fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Jpeg(error) => write!(out, "JPEG: {error}"),
      Self::Png(error) => write!(out, "PNG: {error}"),
      Self::Gif(error) => write!(out, "GIF: {error}"),
      Self::Webp(error) => write!(out, "WebP: {error}"),
    }
  }
}

impl std::error::Error for DecodeError {}

/// The rendering policy distinguishes embedded images from standalone pictures.
/// Embedded JPEGs use their stored orientation and GIFs use the first frame's
/// rectangle. Standalone JPEGs honor orientation and can decode at reduced size.
pub enum DecodeMode<'a> {
  Embedded,
  Standalone {
    covering: Size<u32>,
    jpeg_spread: Option<console_core_jpeg_files::Spread<'a>>,
    webp_spread: Option<console_core_webp_files::Spread<'a>>,
  },
}

pub fn measured(format: Format, bytes: &[u8]) -> Result<Size<u32>, DecodeError> {
  match format {
    Format::Jpeg => console_core_jpeg_files::measured(bytes).map_err(DecodeError::Jpeg),
    Format::Png => console_core_png_files::measured(bytes).map_err(DecodeError::Png),
    Format::Gif => console_core_gif_files::measured(bytes).map_err(DecodeError::Gif),
    Format::Webp => console_core_webp_files::measured(bytes).map_err(DecodeError::Webp),
  }
}

pub fn decoded(
  format: Format,
  bytes: &[u8],
  rendering: DecodeMode<'_>,
) -> Result<PackedPixels, DecodeError> {
  match format {
    Format::Jpeg => match rendering {
      DecodeMode::Embedded => console_core_jpeg_files::decoded_as_stored(bytes),
      DecodeMode::Standalone {
        covering,
        jpeg_spread,
        ..
      } => match jpeg_spread {
        Some(spread) => console_core_jpeg_files::decoded_spread(bytes, covering, spread),
        None => console_core_jpeg_files::decoded(bytes, covering),
      },
    }
    .map_err(DecodeError::Jpeg),
    Format::Png => {
      use console_core_png_files::Reduction;
      let reduction = match rendering {
        DecodeMode::Embedded => Reduction::HighByte,
        DecodeMode::Standalone { .. } => Reduction::Nearest,
      };
      console_core_png_files::decoded_rgba(bytes, reduction).map_err(DecodeError::Png)
    }
    Format::Gif => match rendering {
      DecodeMode::Embedded => console_core_gif_files::decoded_frame(bytes),
      DecodeMode::Standalone { .. } => console_core_gif_files::decoded(bytes),
    }
    .map_err(DecodeError::Gif),
    Format::Webp => match rendering {
      DecodeMode::Standalone {
        webp_spread: Some(spread),
        ..
      } => console_core_webp_files::decoded_spread(bytes, spread),
      DecodeMode::Embedded
      | DecodeMode::Standalone {
        webp_spread: None, ..
      } => console_core_webp_files::decoded(bytes),
    }
    .map_err(DecodeError::Webp),
  }
}
