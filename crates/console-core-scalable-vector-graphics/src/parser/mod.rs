use std::cell::Cell;

use console_core_never::Never;

mod clippath;
mod converter;
mod filter;
mod image;
mod marker;
mod mask;
mod options;
mod paint_server;
mod shapes;
mod style;
mod svgtree;
mod switch;
mod units;
mod use_node;

#[cfg(feature = "text")]
mod text;
#[cfg(feature = "text")]
pub(crate) use converter::Cache;
pub use image::{ImageHrefDataResolverFn, ImageHrefResolver, ImageHrefStringResolverFn};
pub use options::Options;
#[cfg(feature = "writer")]
pub(crate) use svgtree::{AId, EId};

#[derive(Debug)]
pub enum Error {
  NotAnUtf8Str,

  SvgzFeatureNotEnabled,

  MalformedGZip,

  SvgzLimitReached,

  ElementsLimitReached,

  InvalidSize,

  ParsingFailed(console_core_extensible_markup::Error),
}

impl From<console_core_extensible_markup::Error> for Error {
  fn from(e: console_core_extensible_markup::Error) -> Self {
    Error::ParsingFailed(e)
  }
}

impl std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match *self {
      Error::NotAnUtf8Str => {
        write!(f, "provided data has not an UTF-8 encoding")
      }
      Self::SvgzFeatureNotEnabled => {
        write!(f, "enable svgz cargo feature to decode SVGZ data")
      }
      Error::MalformedGZip => {
        write!(f, "provided data has a malformed GZip content")
      }
      Error::SvgzLimitReached => {
        write!(f, "decompressed SVG exceeds its byte limit")
      }
      Error::ElementsLimitReached => {
        write!(f, "the maximum number of SVG elements has been reached")
      }
      Error::InvalidSize => {
        write!(f, "SVG has an invalid size")
      }
      Error::ParsingFailed(ref e) => {
        write!(f, "SVG data parsing failed cause {}", e)
      }
    }
  }
}

impl std::error::Error for Error {}

pub(crate) trait OptionLog {
  fn log_none<F: FnOnce()>(self, f: F) -> Self;
}

impl<T> OptionLog for Option<T> {
  #[inline]
  fn log_none<F: FnOnce()>(self, f: F) -> Self {
    self.or_else(|| {
      f();
      None
    })
  }
}

const MAX_NESTED_DOCUMENTS: u16 = 64;
const MAX_CONVERSION_DEPTH: u16 = 128;

std::thread_local! {
  static DOCUMENT_DEPTH: Cell<u16> = const { Cell::new(0) };
  static CONVERSION_DEPTH: Cell<u16> = const { Cell::new(0) };
}

struct DocumentDepth;

pub(crate) struct ConversionDepth;

pub(crate) struct ConversionLimit;

impl ConversionDepth {
  pub(crate) fn enter() -> Result<Self, ConversionLimit> {
    let depth = CONVERSION_DEPTH.get();

    match (depth < MAX_CONVERSION_DEPTH).then_some(()) {
      Some(()) => {
        CONVERSION_DEPTH.set(depth.saturating_add(1));
        Ok(Self)
      }
      None => Err(ConversionLimit),
    }
  }
}

impl Drop for ConversionDepth {
  fn drop(&mut self) {
    CONVERSION_DEPTH.set(CONVERSION_DEPTH.get().saturating_sub(1));
  }
}

impl DocumentDepth {
  fn enter() -> Result<Self, Error> {
    DOCUMENT_DEPTH
      .try_with(|depth| {
        let current = depth.get();

        match (current < MAX_NESTED_DOCUMENTS).then_some(()) {
          Some(()) => {
            depth.set(current.saturating_add(1));
            Ok(Self)
          }
          None => Err(Error::ElementsLimitReached),
        }
      })
      .map_err(|_closed_thread_context| Error::ElementsLimitReached)?
  }
}

impl Drop for DocumentDepth {
  fn drop(&mut self) {
    match DOCUMENT_DEPTH.try_with(|depth| depth.set(depth.get().saturating_sub(1))) {
      Ok(()) => {}
      Err(closed_thread_context) => {
        console_core_logging::debug!("SVG document depth is unavailable: {closed_thread_context}");
      }
    }
  }
}

impl crate::Tree {
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Document conversion enters the bounded per-thread depth before resolving embedded SVGs"
    )
  )]
  pub fn from_data(data: &[u8], opt: &Options) -> Result<Self, Error> {
    match data {
      [0x1f, 0x8b, ..] => {
        #[cfg(feature = "svgz")]
        {
          let uncompressed = decompress_svgz(data)?;
          let text = std::str::from_utf8(&uncompressed).map_err(|_| Error::NotAnUtf8Str)?;
          Self::from_str(text, opt)
        }

        #[cfg(not(feature = "svgz"))]
        Err(Error::SvgzFeatureNotEnabled)
      }
      _ => {
        let text = std::str::from_utf8(data).map_err(|_| Error::NotAnUtf8Str)?;
        Self::from_str(text, opt)
      }
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Nested SVG data enters the same per-thread document depth before converting another tree"
    )
  )]
  #[cfg_attr(
    all(feature = "text", dylint_lib = "explicit081_no_shared_ownership"),
    expect(
      explicit081_no_shared_ownership,
      reason = "Nested images reuse the parent’s immutable font catalogue"
    )
  )]
  pub fn from_data_nested(data: &[u8], opt: &Options) -> Result<Self, Error> {
    let nested_opt = Options {
      resources_dir: None,
      dpi: opt.dpi,
      font_size: opt.font_size,
      languages: opt.languages.clone(),
      shape_rendering: opt.shape_rendering,
      text_rendering: opt.text_rendering,
      image_rendering: opt.image_rendering,
      default_size: opt.default_size,
      image_href_resolver: ImageHrefResolver {
        resolve_data: Box::new(|a, b, c| (opt.image_href_resolver.resolve_data)(a, b, c)),

        resolve_string: Box::new(|_, _| None),
      },

      #[cfg(feature = "text")]
      console_core_font_database: opt.console_core_font_database.clone(),

      #[cfg(feature = "text")]
      font_resolver: crate::FontResolver {
        select_font: Box::new(|font, db| (opt.font_resolver.select_font)(font, db)),
        select_fallback: Box::new(|c, used_fonts, db| {
          (opt.font_resolver.select_fallback)(c, used_fonts, db)
        }),
      },
      ..Options::default()
    };

    Self::from_data(data, &nested_opt)
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Document conversion enters the bounded per-thread depth before resolving embedded SVGs"
    )
  )]
  pub fn from_str(text: &str, opt: &Options) -> Result<Self, Error> {
    let xml_opt = console_core_extensible_markup::ParsingOptions {
      dtd: console_core_extensible_markup::Dtd::Allow,
      ..Default::default()
    };

    let doc = console_core_extensible_markup::Document::parse_with_options(text, xml_opt)
      .map_err(Error::ParsingFailed)?;

    Self::from_xmltree(&doc, opt)
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "The document guard caps same-thread nested SVG conversion before invoking any image resolver"
    )
  )]
  pub fn from_xmltree(
    doc: &console_core_extensible_markup::Document,
    opt: &Options,
  ) -> Result<Self, Error> {
    let _depth = DocumentDepth::enter()?;
    let doc_2 = svgtree::Document::parse_tree(doc, opt.style_sheet.as_deref())?;
    self::converter::convert_doc(&doc_2, opt)
  }
}

#[cfg(feature = "svgz")]
pub fn decompress_svgz(data: &[u8]) -> Result<Vec<u8>, Error> {
  const SVG_BYTES: u32 = 64 * 1024 * 1024;

  let decoder = flate2::read::GzDecoder::new(data);

  console_core_bounded_reads::bytes(decoder, SVG_BYTES).map_err(|fault| match fault {
    console_core_bounded_reads::ReadError::TooLong { .. } => Error::SvgzLimitReached,
    console_core_bounded_reads::ReadError::Io(_)
    | console_core_bounded_reads::ReadError::NotUtf8(_) => Error::MalformedGZip,
  })
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The limits and value share the same dimensionless filter or gradient quantity"
  )
)]
pub(crate) struct UnitClamp {
  pub lower: f32,
  pub value: f32,
  pub upper: f32,
}

pub(crate) fn f32_bound(
  UnitClamp {
    lower,
    value,
    upper,
  }: UnitClamp,
) -> Result<f32, Never> {
  Ok(match (value > upper, value >= lower) {
    (true, _) => upper,
    (false, true) => value,
    (false, false) => lower,
  })
}
