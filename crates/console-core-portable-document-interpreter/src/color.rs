//! PDF colors and color spaces.

use crate::cache::{Cache, CacheKey, Cached};
use crate::interpret::state::Opacity;
use console_core_never::Never;
use console_core_portable_document_functions::{Function, PdfSample};
use console_core_portable_document_syntax::object;
use console_core_portable_document_syntax::object::Array;
use console_core_portable_document_syntax::object::Dict;
use console_core_portable_document_syntax::object::Name;
use console_core_portable_document_syntax::object::Object;
use console_core_portable_document_syntax::object::Stream;
use console_core_portable_document_syntax::object::array::FlexArrayIter;
use console_core_portable_document_syntax::object::dict::keys::*;
use moxcms::{
  ColorProfile, DataColorSpace, Layout, Transform8BitExecutor, TransformF32Executor,
  TransformOptions, Xyzd,
};
use std::borrow::Cow;
use std::fmt::{Debug, Formatter};
use std::num::NonZeroU8;
use std::ops::Deref;
use std::sync::{Arc, LazyLock, OnceLock};

pub type ColorComponents = Vec<f32>;

#[derive(Debug, Copy, Clone)]
pub struct AlphaColor {
  components: [f32; 4],
}

impl AlphaColor {
  pub const BLACK: Self = ({
    let Ok(explicit_value) = Self::new([0., 0., 0., 1.]);
    explicit_value
  });

  pub const TRANSPARENT: Self = ({
    let Ok(explicit_value) = Self::new([0., 0., 0., 0.]);
    explicit_value
  });

  pub const WHITE: Self = ({
    let Ok(explicit_value) = Self::new([1., 1., 1., 1.]);
    explicit_value
  });

  pub const fn new(
    components: [f32; 4],
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { components })
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public RGB constructor follows the documented red-green-blue channel order"
    )
  )]
  pub const fn from_rgb8(
    r: u8,
    g: u8,
    b: u8,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let components = [
      ({
        let Ok(explicit_value) = u8_to_f32(r);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = u8_to_f32(g);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = u8_to_f32(b);
        explicit_value
      }),
      1.,
    ];
    let Ok(alpha_color) = Self::new(components);

    Ok(alpha_color)
  }

  pub fn premultiplied(&self) -> core::result::Result<[f32; 4], console_core_never::Never> {
    Ok({
      [
        (match self.components {
          [element, ..] => element,
        }) * (match self.components {
          [_, _, _, element, ..] => element,
        }),
        (match self.components {
          [_, element, ..] => element,
        }) * (match self.components {
          [_, _, _, element, ..] => element,
        }),
        (match self.components {
          [_, _, element, ..] => element,
        }) * (match self.components {
          [_, _, _, element, ..] => element,
        }),
        match self.components {
          [_, _, _, _explicit_element, ..] => _explicit_element,
        },
      ]
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public RGBA constructor follows the documented red-green-blue-alpha channel order"
    )
  )]
  pub const fn from_rgba8(
    r: u8,
    g: u8,
    b: u8,
    a: u8,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let components = [
      ({
        let Ok(explicit_value) = u8_to_f32(r);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = u8_to_f32(g);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = u8_to_f32(b);
        explicit_value
      }),
      ({
        let Ok(explicit_value) = u8_to_f32(a);
        explicit_value
      }),
    ];
    let Ok(alpha_color) = Self::new(components);

    Ok(alpha_color)
  }

  pub fn to_rgba8(&self) -> core::result::Result<[u8; 4], console_core_never::Never> {
    Ok({
      [
        ({
          let Ok(numeric_value) = console_core_number_conversion::toward_zero_u8(f64::from(
            (match self.components {
              [element, ..] => element,
            }) * 255.0
              + 0.5,
          ));
          numeric_value
        }),
        ({
          let Ok(numeric_value) = console_core_number_conversion::toward_zero_u8(f64::from(
            (match self.components {
              [_, element, ..] => element,
            }) * 255.0
              + 0.5,
          ));
          numeric_value
        }),
        ({
          let Ok(numeric_value) = console_core_number_conversion::toward_zero_u8(f64::from(
            (match self.components {
              [_, _, element, ..] => element,
            }) * 255.0
              + 0.5,
          ));
          numeric_value
        }),
        ({
          let Ok(numeric_value) = console_core_number_conversion::toward_zero_u8(f64::from(
            (match self.components {
              [_, _, _, element, ..] => element,
            }) * 255.0
              + 0.5,
          ));
          numeric_value
        }),
      ]
    })
  }

  pub fn components(&self) -> core::result::Result<[f32; 4], console_core_never::Never> {
    Ok(self.components)
  }
}

#[cfg_attr(
  dylint_lib = "explicit011_no_as_cast",
  expect(
    explicit011_no_as_cast,
    reason = "Every u8 is exactly representable in f32 and this conversion must run in const constructors"
  )
)]
const fn u8_to_f32(x: u8) -> core::result::Result<f32, console_core_never::Never> {
  Ok(x as f32 * (1.0 / 255.0))
}

#[derive(Debug, Clone)]
pub(crate) enum ColorSpaceType {
  DeviceCmyk,
  DeviceGray,
  DeviceRgb,
  Pattern(ColorSpace),
  Indexed(Indexed),
  ICCBased(ICCProfile),
  CalGray(CalGray),
  CalRgb(CalRgb),
  Lab(Lab),
  Separation(Separation),
  DeviceN(DeviceN),
}

enum ColorRead<'a> {
  Ready(Option<ColorSpaceType>),
  Nested(Object<'a>, ColorFrame<'a>),
}

enum ColorFrame<'a> {
  Pattern,
  Indexed(FlexArrayIter<'a>),
  Separation {
    tail: FlexArrayIter<'a>,
    coverage: InkCoverage,
  },
  DeviceN {
    tail: FlexArrayIter<'a>,
    components: NonZeroU8,
    coverage: InkCoverage,
  },
  Profile {
    key: u128,
    count: u32,
  },
}

impl ColorFrame<'_> {
  fn complete(
    self,
    base: Option<ColorSpaceType>,
    cache: &Cache,
  ) -> Result<Option<ColorSpaceType>, console_core_never::Never> {
    match self {
      Self::Pattern => {
        let Ok(space) = match base {
          Some(base_2) => ColorSpace::from_kind(base_2),
          None => ColorSpace::device_rgb(),
        };

        Ok(Some(ColorSpaceType::Pattern(space)))
      }
      Self::Indexed(iter) => {
        let Ok(indexed) = Indexed::finish(iter, base);

        Ok(indexed.map(ColorSpaceType::Indexed))
      }
      Self::Separation { tail, coverage } => {
        let Ok(prepared) = finish_tint(tail, base);
        Ok(prepared.map(|(alternate_space, tint_transform)| {
          ColorSpaceType::Separation(Separation {
            alternate_space,
            tint_transform,
            coverage,
          })
        }))
      }
      Self::DeviceN {
        tail,
        components,
        coverage,
      } => {
        let Ok(prepared) = finish_tint(tail, base);
        Ok(prepared.map(|(alternate_space, tint_transform)| {
          ColorSpaceType::DeviceN(DeviceN {
            alternate_space,
            tint_transform,
            coverage,
            num_components: components,
          })
        }))
      }
      Self::Profile { key, count } => cache.get_or_insert_with(key, || {
        let Ok(default) = device_space(count);
        base.or(default)
      }),
    }
  }
}

fn finish_tint(
  mut tail: FlexArrayIter<'_>,
  base: Option<ColorSpaceType>,
) -> Result<Option<(ColorSpace, Function)>, console_core_never::Never> {
  let Ok(alternate) = match base {
    Some(base_2) => ColorSpace::from_kind(base_2),
    None => return Ok(None),
  };
  let Ok(function) = tail.next::<Object<'_>>();

  Ok(match function {
    Some(function_2) => {
      let Ok(tint_transform) = Function::new(&function_2);
      tint_transform.map(|transform| (alternate, transform))
    }
    None => None,
  })
}

fn finish_color(
  mut base: Option<ColorSpaceType>,
  held: Vec<ColorFrame<'_>>,
  cache: &Cache,
) -> Result<Option<ColorSpaceType>, console_core_never::Never> {
  for frame in held.into_iter().rev() {
    let Ok(completed) = frame.complete(base, cache);
    base = completed;
  }

  Ok(base)
}

fn read_color<'a>(
  object: Object<'a>,
  cache: &Cache,
) -> Result<ColorRead<'a>, console_core_never::Never> {
  let array = match object {
    Object::Name(name) => {
      let Ok(named) = ColorSpaceType::new_from_name(&name);
      return Ok(ColorRead::Ready(named));
    }
    Object::Array(array) => array,
    Object::Null(_)
    | Object::Boolean(_)
    | Object::Number(_)
    | Object::String(_)
    | Object::Dict(_)
    | Object::Stream(_) => return Ok(ColorRead::Ready(None)),
  };
  let Ok(mut entries) = array.flex_iter();
  let Ok(first) = entries.next::<Name<'_>>();
  let name = match first {
    Some(name) => name,
    None => return Ok(ColorRead::Ready(None)),
  };

  match name.deref() {
    ICC_BASED => {
      let Ok(profile) = entries.next::<Stream<'_>>();

      match profile {
        Some(profile_2) => color_profile(profile_2, cache),
        None => Ok(ColorRead::Ready(None)),
      }
    }
    CALGRAY => read_calibrated(entries, |dict| {
      let Ok(space) = CalGray::new(dict);
      Ok(space.map(ColorSpaceType::CalGray))
    }),
    CALRGB => read_calibrated(entries, |dict| {
      let Ok(space) = CalRgb::new(dict);
      Ok(space.map(ColorSpaceType::CalRgb))
    }),
    LAB => read_calibrated(entries, |dict| {
      let Ok(space) = Lab::new(dict);
      Ok(space.map(ColorSpaceType::Lab))
    }),
    INDEXED | I => {
      let Ok(base) = entries.next::<Object<'_>>();
      child_color(base, ColorFrame::Indexed(entries))
    }
    SEPARATION => {
      let Ok(colorant) = entries.next::<Name<'_>>();
      let name_3 = match colorant {
        Some(name_2) => name_2,
        None => return Ok(ColorRead::Ready(None)),
      };
      let coverage = match name_3.as_ref() {
        b"None" => InkCoverage::NoInk,
        _named_colorant => InkCoverage::Paints,
      };
      let Ok(base) = entries.next::<Object<'_>>();
      child_color(
        base,
        ColorFrame::Separation {
          tail: entries,
          coverage,
        },
      )
    }
    DEVICE_N => read_device_n(entries),
    PATTERN => {
      let Ok(_uncolored_pattern_name) = entries.next::<Name<'_>>();
      let Ok(base) = entries.next::<Object<'_>>();

      match base {
        Some(base_2) => Ok(ColorRead::Nested(base_2, ColorFrame::Pattern)),
        None => {
          let Ok(rgb) = ColorSpace::device_rgb();
          Ok(ColorRead::Ready(Some(ColorSpaceType::Pattern(rgb))))
        }
      }
    }
    DEVICE_RGB | RGB => Ok(ColorRead::Ready(Some(ColorSpaceType::DeviceRgb))),
    DEVICE_GRAY | G => Ok(ColorRead::Ready(Some(ColorSpaceType::DeviceGray))),
    DEVICE_CMYK | CMYK | CALCMYK => Ok(ColorRead::Ready(Some(ColorSpaceType::DeviceCmyk))),
    _unsupported_name => {
      let Ok(spelled) = name.as_str();
      warn!("unsupported color space: {}", spelled);

      Ok(ColorRead::Ready(None))
    }
  }
}

fn child_color<'a>(
  base: Option<Object<'a>>,
  frame: ColorFrame<'a>,
) -> Result<ColorRead<'a>, console_core_never::Never> {
  Ok(match base {
    Some(base_2) => ColorRead::Nested(base_2, frame),
    None => ColorRead::Ready(None),
  })
}

fn read_calibrated<'a>(
  mut entries: FlexArrayIter<'a>,
  read: impl FnOnce(&Dict<'a>) -> Result<Option<ColorSpaceType>, console_core_never::Never>,
) -> Result<ColorRead<'a>, console_core_never::Never> {
  let Ok(calibration) = entries.next::<Dict<'_>>();

  match calibration {
    Some(calibration_2) => read(&calibration_2).map(ColorRead::Ready),
    None => Ok(ColorRead::Ready(None)),
  }
}

fn read_device_n(
  mut entries: FlexArrayIter<'_>,
) -> Result<ColorRead<'_>, console_core_never::Never> {
  let Ok(named) = entries.next::<Array<'_>>();
  let names = match named {
    Some(names) => names,
    None => return Ok(ColorRead::Ready(None)),
  };
  let Ok(iter) = names.iter::<Name<'_>>();
  let mut count = 0_u16;
  let mut coverage = InkCoverage::NoInk;

  for name in iter.take(256) {
    count = count.saturating_add(1);

    match name.as_ref() {
      b"None" => {}
      _named_colorant => coverage = InkCoverage::Paints,
    }
  }

  let name_count = match u8::try_from(count) {
    Ok(count_2) => count_2,
    Err(_too_many_inks) => return Ok(ColorRead::Ready(None)),
  };
  let components = match NonZeroU8::new(name_count) {
    Some(count_3) => count_3,
    None => return Ok(ColorRead::Ready(None)),
  };
  let Ok(base) = entries.next::<Object<'_>>();

  child_color(
    base,
    ColorFrame::DeviceN {
      tail: entries,
      components,
      coverage,
    },
  )
}

fn color_profile<'a>(
  stream: Stream<'a>,
  cache: &Cache,
) -> Result<ColorRead<'a>, console_core_never::Never> {
  let Ok(dict) = stream.dict();
  let Ok(mentioned) = dict.get::<u32>(N);
  let count = match mentioned {
    Some(count) => count,
    None => return Ok(ColorRead::Ready(None)),
  };
  let key = stream.cache_key();
  let Ok(cached) = cache.cached(key);

  match cached {
    Cached::Present(value) => return Ok(ColorRead::Ready(value)),
    Cached::Absent => {}
  }

  let bytes = match stream.decoded() {
    Ok(bytes) => bytes,
    Err(_invalid_icc_stream_encoding) => {
      return cache.get_or_insert_with(key, || None).map(ColorRead::Ready);
    }
  };
  let Ok(profile) = ICCProfile::new(&bytes, count);

  match profile {
    Some(profile_2) => {
      let Ok(srgb) = profile_2.is_srgb();
      let kind = match srgb {
        crate::SrgbProfile::Srgb => ColorSpaceType::DeviceRgb,
        crate::SrgbProfile::NotSrgb => ColorSpaceType::ICCBased(profile_2),
      };
      cache
        .get_or_insert_with(key, || Some(kind))
        .map(ColorRead::Ready)
    }
    None => {
      let Ok(alternate) = dict.get::<Object<'_>>(ALTERNATE);

      match alternate {
        Some(alternate_2) => Ok(ColorRead::Nested(
          alternate_2,
          ColorFrame::Profile { key, count },
        )),
        None => cache
          .get_or_insert_with(key, || {
            let Ok(default) = device_space(count);
            default
          })
          .map(ColorRead::Ready),
      }
    }
  }
}

fn device_space(count: u32) -> Result<Option<ColorSpaceType>, console_core_never::Never> {
  Ok(match count {
    1 => Some(ColorSpaceType::DeviceGray),
    3 => Some(ColorSpaceType::DeviceRgb),
    4 => Some(ColorSpaceType::DeviceCmyk),
    _unsupported_channels => None,
  })
}

impl ColorSpaceType {
  fn new_from_name(
    name: &Name<'_>,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(match name.deref() {
      DEVICE_RGB | RGB => Some(Self::DeviceRgb),
      DEVICE_GRAY | G => Some(Self::DeviceGray),
      DEVICE_CMYK | CMYK => Some(Self::DeviceCmyk),
      CALCMYK => Some(Self::DeviceCmyk),
      PATTERN => {
        let Ok(device_rgb) = ColorSpace::device_rgb();

        Some(Self::Pattern(device_rgb))
      }
      _ => None,
    })
  }
}

#[derive(Debug, Clone)]
pub struct ColorSpace(ColorStorage);

#[derive(Debug, Clone)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "complex immutable color descriptions can be retained by graphics snapshots and deferred images"
  )
)]
enum ColorStorage {
  Device(Channels),
  Parsed(Arc<ColorSpaceType>),
}

impl AsRef<ColorSpaceType> for ColorStorage {
  fn as_ref(&self) -> &ColorSpaceType {
    match self {
      Self::Device(Channels::Gray) => &ColorSpaceType::DeviceGray,
      Self::Device(Channels::Rgb) => &ColorSpaceType::DeviceRgb,
      Self::Device(Channels::Cmyk) => &ColorSpaceType::DeviceCmyk,
      Self::Parsed(value) => value,
    }
  }
}

impl ColorSpace {
  pub(crate) fn new(
    mut object: Object<'_>,
    cache: &Cache,
  ) -> Result<Option<Self>, console_core_never::Never> {
    let mut held = Vec::new();
    let mut base = None;

    for _ in 0..=MAX_COLOR_CHAIN {
      let Ok(step) = read_color(object, cache);

      match step {
        ColorRead::Ready(value) => {
          base = value;
          break;
        }
        ColorRead::Nested(child, frame) => {
          held.push(frame);
          object = child;
        }
      }
    }

    let Ok(completed) = finish_color(base, held, cache);
    Ok(completed.map(|kind| {
      let Ok(space) = Self::from_kind(kind);
      space
    }))
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "one allocation preserves the parsed color descriptor for graphics snapshots and deferred images"
    )
  )]
  fn from_kind(kind: ColorSpaceType) -> Result<Self, console_core_never::Never> {
    Ok(Self(match kind {
      ColorSpaceType::DeviceGray => ColorStorage::Device(Channels::Gray),
      ColorSpaceType::DeviceRgb => ColorStorage::Device(Channels::Rgb),
      ColorSpaceType::DeviceCmyk => ColorStorage::Device(Channels::Cmyk),
      parsed @ (ColorSpaceType::Pattern(_)
      | ColorSpaceType::Indexed(_)
      | ColorSpaceType::ICCBased(_)
      | ColorSpaceType::CalGray(_)
      | ColorSpaceType::CalRgb(_)
      | ColorSpaceType::Lab(_)
      | ColorSpaceType::Separation(_)
      | ColorSpaceType::DeviceN(_)) => ColorStorage::Parsed(Arc::new(parsed)),
    }))
  }

  pub(crate) fn new_from_name(
    name: &Name<'_>,
  ) -> Result<Option<Self>, console_core_never::Never> {
    let Ok(named) = ColorSpaceType::new_from_name(name);

    Ok(named.map(|kind| {
      let Ok(space) = Self::from_kind(kind);
      space
    }))
  }

  pub(crate) fn device_gray() -> Result<Self, console_core_never::Never> {
    Self::from_kind(ColorSpaceType::DeviceGray)
  }

  pub(crate) fn device_rgb() -> Result<Self, console_core_never::Never> {
    Self::from_kind(ColorSpaceType::DeviceRgb)
  }

  pub(crate) fn device_cmyk() -> Result<Self, console_core_never::Never> {
    Self::from_kind(ColorSpaceType::DeviceCmyk)
  }

  pub(crate) fn pattern() -> Result<Self, console_core_never::Never> {
    let Ok(base) = Self::device_gray();

    Self::from_kind(ColorSpaceType::Pattern(base))
  }

  pub(crate) fn pattern_cs(
    &self,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(match self.0.as_ref() {
      ColorSpaceType::Pattern(cs) => Some(cs.clone()),
      ColorSpaceType::DeviceCmyk
      | ColorSpaceType::DeviceGray
      | ColorSpaceType::DeviceRgb
      | ColorSpaceType::Indexed(_)
      | ColorSpaceType::ICCBased(_)
      | ColorSpaceType::CalGray(_)
      | ColorSpaceType::CalRgb(_)
      | ColorSpaceType::Lab(_)
      | ColorSpaceType::Separation(_)
      | ColorSpaceType::DeviceN(_) => None,
    })
  }

  pub(crate) fn is_pattern(
    &self,
  ) -> core::result::Result<crate::PatternSpace, console_core_never::Never> {
    Ok(match self.0.as_ref() {
      ColorSpaceType::Pattern(_) => crate::PatternSpace::Pattern,
      ColorSpaceType::DeviceCmyk
      | ColorSpaceType::DeviceGray
      | ColorSpaceType::DeviceRgb
      | ColorSpaceType::Indexed(_)
      | ColorSpaceType::ICCBased(_)
      | ColorSpaceType::CalGray(_)
      | ColorSpaceType::CalRgb(_)
      | ColorSpaceType::Lab(_)
      | ColorSpaceType::Separation(_)
      | ColorSpaceType::DeviceN(_) => crate::PatternSpace::NotPattern,
    })
  }

  pub(crate) fn is_indexed(
    &self,
  ) -> core::result::Result<crate::IndexedSpace, console_core_never::Never> {
    Ok(match self.0.as_ref() {
      ColorSpaceType::Indexed(_) => crate::IndexedSpace::Indexed,
      ColorSpaceType::DeviceCmyk
      | ColorSpaceType::DeviceGray
      | ColorSpaceType::DeviceRgb
      | ColorSpaceType::Pattern(_)
      | ColorSpaceType::ICCBased(_)
      | ColorSpaceType::CalGray(_)
      | ColorSpaceType::CalRgb(_)
      | ColorSpaceType::Lab(_)
      | ColorSpaceType::Separation(_)
      | ColorSpaceType::DeviceN(_) => crate::IndexedSpace::NotIndexed,
    })
  }

  fn without_pattern(&self) -> Result<&ColorSpaceType, console_core_never::Never> {
    let mut current = self;

    for _ in 0..=MAX_COLOR_CHAIN {
      match current.0.as_ref() {
        ColorSpaceType::Pattern(base) => current = base,
        terminal @ (ColorSpaceType::DeviceCmyk
        | ColorSpaceType::DeviceGray
        | ColorSpaceType::DeviceRgb
        | ColorSpaceType::Indexed(_)
        | ColorSpaceType::ICCBased(_)
        | ColorSpaceType::CalGray(_)
        | ColorSpaceType::CalRgb(_)
        | ColorSpaceType::Lab(_)
        | ColorSpaceType::Separation(_)
        | ColorSpaceType::DeviceN(_)) => return Ok(terminal),
      }
    }

    Ok(&ColorSpaceType::DeviceRgb)
  }

  pub(crate) fn default_decode_arr(
    &self,
    bit_depth: u8,
  ) -> core::result::Result<Vec<(f32, f32)>, console_core_never::Never> {
    Ok(match self.0.as_ref() {
      ColorSpaceType::DeviceCmyk => vec![(0.0, 1.0), (0.0, 1.0), (0.0, 1.0), (0.0, 1.0)],
      ColorSpaceType::DeviceGray => vec![(0.0, 1.0)],
      ColorSpaceType::DeviceRgb => vec![(0.0, 1.0), (0.0, 1.0), (0.0, 1.0)],
      ColorSpaceType::ICCBased(profile) => match profile.0.channels {
        Channels::Gray => vec![(0.0, 1.0)],
        Channels::Rgb => vec![(0.0, 1.0); 3],
        Channels::Cmyk => vec![(0.0, 1.0); 4],
      },
      ColorSpaceType::CalGray(_) => vec![(0.0, 1.0)],
      ColorSpaceType::CalRgb(_) => vec![(0.0, 1.0), (0.0, 1.0), (0.0, 1.0)],
      ColorSpaceType::Lab(l) => vec![
        (0.0, 100.0),
        (
          match l.range {
            [_explicit_element, ..] => _explicit_element,
          },
          match l.range {
            [_, _explicit_element, ..] => _explicit_element,
          },
        ),
        (
          match l.range {
            [_, _, _explicit_element, ..] => _explicit_element,
          },
          match l.range {
            [_, _, _, _explicit_element, ..] => _explicit_element,
          },
        ),
      ],
      ColorSpaceType::Indexed(_) => vec![(0.0, 2.0_f32.powf(f32::from(bit_depth)) - 1.0)],
      ColorSpaceType::Separation(_) => vec![(0.0, 1.0)],
      ColorSpaceType::DeviceN(device) => (0..device.num_components.get())
        .map(|_| (0.0, 1.0))
        .collect(),

      ColorSpaceType::Pattern(_) => vec![(0.0, 1.0)],
    })
  }

  pub(crate) fn inverted_default_decode_arr(
    &self,
    bit_depth: u8,
  ) -> core::result::Result<Vec<(f32, f32)>, console_core_never::Never> {
    let Ok(default_decode_arr) = self.default_decode_arr(bit_depth);

    Ok(
      default_decode_arr
        .iter()
        .map(|(min, max)| (*max, *min))
        .collect(),
    )
  }

  pub(crate) fn initial_color(
    &self,
  ) -> core::result::Result<ColorComponents, console_core_never::Never> {
    let Ok(base) = self.without_pattern();

    Ok(match base {
      ColorSpaceType::DeviceCmyk => vec![0.0, 0.0, 0.0, 1.0],
      ColorSpaceType::DeviceGray => vec![0.0],
      ColorSpaceType::DeviceRgb => vec![0.0, 0.0, 0.0],
      ColorSpaceType::ICCBased(profile) => match profile.0.channels {
        Channels::Gray => vec![0.0],
        Channels::Rgb => vec![0.0, 0.0, 0.0],
        Channels::Cmyk => vec![0.0, 0.0, 0.0, 1.0],
      },
      ColorSpaceType::CalGray(_) => vec![0.0],
      ColorSpaceType::CalRgb(_) => vec![0.0, 0.0, 0.0],
      ColorSpaceType::Lab(_) => vec![0.0, 0.0, 0.0],
      ColorSpaceType::Indexed(_) => vec![0.0],
      ColorSpaceType::Separation(_) => vec![1.0],
      ColorSpaceType::Pattern(_) => vec![0.0, 0.0, 0.0],
      ColorSpaceType::DeviceN(device) => (0..device.num_components.get()).map(|_| 1.0).collect(),
    })
  }

  pub(crate) fn is_device_gray(
    &self,
  ) -> core::result::Result<crate::GraySpace, console_core_never::Never> {
    Ok(match self.0.as_ref() {
      ColorSpaceType::DeviceGray => crate::GraySpace::DeviceGray,
      ColorSpaceType::DeviceCmyk
      | ColorSpaceType::DeviceRgb
      | ColorSpaceType::Pattern(_)
      | ColorSpaceType::Indexed(_)
      | ColorSpaceType::ICCBased(_)
      | ColorSpaceType::CalGray(_)
      | ColorSpaceType::CalRgb(_)
      | ColorSpaceType::Lab(_)
      | ColorSpaceType::Separation(_)
      | ColorSpaceType::DeviceN(_) => crate::GraySpace::NotDeviceGray,
    })
  }

  pub(crate) fn num_components(&self) -> core::result::Result<u8, console_core_never::Never> {
    let Ok(base) = self.without_pattern();

    Ok(match base {
      ColorSpaceType::DeviceCmyk => 4,
      ColorSpaceType::DeviceGray => 1,
      ColorSpaceType::DeviceRgb => 3,
      ColorSpaceType::ICCBased(icc) => {
        let Ok(count) = icc.0.channels.count();
        count
      }
      ColorSpaceType::CalGray(_) => 1,
      ColorSpaceType::CalRgb(_) => 3,
      ColorSpaceType::Lab(_) => 3,
      ColorSpaceType::Indexed(_) => 1,
      ColorSpaceType::Separation(_) => 1,
      ColorSpaceType::Pattern(_) => 3,
      ColorSpaceType::DeviceN(device) => device.num_components.get(),
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The public color-space conversion accepts the normalized float opacity of existing callers"
    )
  )]
  pub fn to_rgba(
    &self,
    c: &[f32],
    opacity: f32,
    scaling: InputScaling,
  ) -> core::result::Result<AlphaColor, console_core_never::Never> {
    let Ok(alpha_color) = self.to_alpha_color(c, Opacity(opacity), scaling);

    Ok(match alpha_color {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => AlphaColor::BLACK,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputScaling {
  ApplyColorSpace,
  AlreadyScaled,
}

pub(crate) enum SampleConversion {
  ByteOrFloat,
  FloatOnly,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum InkCoverage {
  Paints,
  NoInk,
}

const MAX_COLOR_CHAIN: u32 = crate::context::MAX_NESTED_INTERPRETATION_DEPTH;

impl ToRgb for ColorSpace {
  fn convert_f32(&self, input: &[f32], output: &mut [u8], scaling: InputScaling) -> Option<()> {
    let mut color_space = self;
    let mut samples = Cow::Borrowed(input);
    let mut scaling_2 = scaling;

    for _ in 0..=MAX_COLOR_CHAIN {
      match color_space.0.as_ref() {
        ColorSpaceType::Pattern(base) => {
          color_space = base;
        }
        ColorSpaceType::Indexed(indexed) => {
          let expanded = match indexed.expand(&samples) {
            Ok(values) => values,
            Err(_missing_palette_entry) => return None,
          };
          samples = Cow::Owned(expanded);
          color_space = &indexed.base;
          scaling_2 = InputScaling::AlreadyScaled;
        }
        ColorSpaceType::Separation(separation) => {
          let Ok(expanded) = separation.expand(&samples);
          samples = Cow::Owned(expanded);
          color_space = &separation.alternate_space;
          scaling_2 = InputScaling::ApplyColorSpace;
        }
        ColorSpaceType::DeviceN(device) => {
          let Ok(expanded) = device.expand(&samples);
          samples = Cow::Owned(expanded);
          color_space = &device.alternate_space;
          scaling_2 = InputScaling::ApplyColorSpace;
        }
        leaf @ (ColorSpaceType::DeviceCmyk
        | ColorSpaceType::DeviceGray
        | ColorSpaceType::DeviceRgb
        | ColorSpaceType::ICCBased(_)
        | ColorSpaceType::CalGray(_)
        | ColorSpaceType::CalRgb(_)
        | ColorSpaceType::Lab(_)) => {
          return match leaf_float(leaf, &samples, output, scaling_2) {
            Ok(()) => Some(()),
            Err(_unconvertible_color) => None,
          };
        }
      }
    }

    None
  }

  fn sample_conversion(&self) -> Result<SampleConversion, Never> {
    let Ok(base) = self.without_pattern();

    Ok(match base {
      ColorSpaceType::DeviceCmyk | ColorSpaceType::DeviceGray | ColorSpaceType::DeviceRgb => {
        SampleConversion::ByteOrFloat
      }
      ColorSpaceType::ICCBased(profile) => return profile.sample_conversion(),
      ColorSpaceType::Pattern(_)
      | ColorSpaceType::Indexed(_)
      | ColorSpaceType::CalGray(_)
      | ColorSpaceType::CalRgb(_)
      | ColorSpaceType::Lab(_)
      | ColorSpaceType::Separation(_)
      | ColorSpaceType::DeviceN(_) => SampleConversion::FloatOnly,
    })
  }

  fn convert_u8(
    &self,
    input: &[u8],
    output: &mut [u8],
  ) -> Result<Option<()>, console_core_never::Never> {
    let Ok(base) = self.without_pattern();

    match base {
      ColorSpaceType::DeviceCmyk => convert_cmyk(input, output),
      ColorSpaceType::DeviceGray => {
        let Ok(()) = copy_gray(input.iter().copied(), output);
        Ok(Some(()))
      }
      ColorSpaceType::DeviceRgb => {
        for (input_2, output_2) in input.iter().zip(output.iter_mut()) {
          *output_2 = *input_2;
        }

        Ok(Some(()))
      }
      ColorSpaceType::ICCBased(profile) => profile.convert_u8(input, output),
      ColorSpaceType::Pattern(_)
      | ColorSpaceType::Indexed(_)
      | ColorSpaceType::CalGray(_)
      | ColorSpaceType::CalRgb(_)
      | ColorSpaceType::Lab(_)
      | ColorSpaceType::Separation(_)
      | ColorSpaceType::DeviceN(_) => Ok(None),
    }
  }

  fn ink_coverage(&self) -> Result<InkCoverage, Never> {
    Ok(match self.0.as_ref() {
      ColorSpaceType::Separation(separation) => separation.coverage,
      ColorSpaceType::DeviceN(device) => device.coverage,
      ColorSpaceType::DeviceCmyk
      | ColorSpaceType::DeviceGray
      | ColorSpaceType::DeviceRgb
      | ColorSpaceType::Pattern(_)
      | ColorSpaceType::Indexed(_)
      | ColorSpaceType::ICCBased(_)
      | ColorSpaceType::CalGray(_)
      | ColorSpaceType::CalRgb(_)
      | ColorSpaceType::Lab(_) => InkCoverage::Paints,
    })
  }
}

enum ColorConversionError {
  MissingPaletteEntry,
  TransformUnavailable,
  InvalidLeafSpace,
}

fn leaf_float(
  color_space: &ColorSpaceType,
  input: &[f32],
  output: &mut [u8],
  scaling: InputScaling,
) -> Result<(), ColorConversionError> {
  match color_space {
    ColorSpaceType::DeviceCmyk => {
      let converted = match input {
        [cyan, magenta, yellow, black] => {
          let bytes = [*cyan, *magenta, *yellow, *black].map(|value| {
            let Ok(byte) = f32_to_u8(PdfSample(value));
            byte
          });
          let Ok(result) = convert_cmyk(&bytes, output);

          return result.ok_or(ColorConversionError::TransformUnavailable);
        }
        other => other
          .iter()
          .copied()
          .map(|value| {
            let Ok(byte) = f32_to_u8(PdfSample(value));
            byte
          })
          .collect::<Vec<_>>(),
      };
      let Ok(result) = convert_cmyk(&converted, output);

      result.ok_or(ColorConversionError::TransformUnavailable)
    }
    ColorSpaceType::DeviceGray => {
      let values = input.iter().copied().map(|gray| {
        let Ok(byte) = f32_to_u8(PdfSample(gray));
        byte
      });
      let Ok(()) = copy_gray(values, output);

      Ok(())
    }
    ColorSpaceType::DeviceRgb => {
      for (component, output_2) in input.iter().copied().zip(output) {
        let Ok(byte) = f32_to_u8(PdfSample(component));
        *output_2 = byte;
      }

      Ok(())
    }
    ColorSpaceType::ICCBased(profile) => profile
      .convert_f32(input, output, scaling)
      .ok_or(ColorConversionError::TransformUnavailable),
    ColorSpaceType::CalGray(space) => space
      .convert_f32(input, output, scaling)
      .ok_or(ColorConversionError::TransformUnavailable),
    ColorSpaceType::CalRgb(space) => space
      .convert_f32(input, output, scaling)
      .ok_or(ColorConversionError::TransformUnavailable),
    ColorSpaceType::Lab(space) => space
      .convert_f32(input, output, scaling)
      .ok_or(ColorConversionError::TransformUnavailable),
    ColorSpaceType::Pattern(_)
    | ColorSpaceType::Indexed(_)
    | ColorSpaceType::Separation(_)
    | ColorSpaceType::DeviceN(_) => Err(ColorConversionError::InvalidLeafSpace),
  }
}

fn copy_gray(
  values: impl Iterator<Item = u8>,
  output: &mut [u8],
) -> Result<(), console_core_never::Never> {
  for (gray, rgb) in values.zip(output.chunks_exact_mut(3)) {
    match rgb {
      [r, g, b] => {
        *r = gray;
        *g = gray;
        *b = gray;
      }
      _wrong_channel_count => {}
    }
  }

  Ok(())
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The calibrated gray gamma is the exponent of the PDF gray transfer curve"
  )
)]
struct GammaExponent(f32);

#[derive(Debug, Clone)]
pub(crate) struct CalGray {
  white_point: [f32; 3],
  gamma: GammaExponent,
}

impl CalGray {
  fn new(dict: &Dict<'_>) -> Result<Option<Self>, Never> {
    let Ok(stated_white_point) = dict.get::<[f32; 3]>(WHITE_POINT);
    let white_point = match stated_white_point {
      Some(value) => value,
      None => [1.0, 1.0, 1.0],
    };
    let Ok(stated_gamma) = dict.get::<f32>(GAMMA);
    let gamma = match stated_gamma {
      Some(value) => GammaExponent(value),
      None => GammaExponent(1.0),
    };

    Ok(Some(Self { white_point, gamma }))
  }
}

impl ToRgb for CalGray {
  fn convert_f32(&self, input: &[f32], output: &mut [u8], _: InputScaling) -> Option<()> {
    let (pixels, _partial_pixel) = output.as_chunks_mut::<3>();
    let [_x, y, _z] = self.white_point;

    for (input_gray, pixel) in input.iter().copied().zip(pixels) {
      let lightness = y * input_gray.powf(self.gamma.0);
      let Ok(gray) = console_core_number_conversion::toward_zero_u8(f64::from(
        0.0_f32.max(295.8 * lightness.powf(0.333_333_34) - 40.8) + 0.5,
      ));
      *pixel = [gray; 3];
    }

    Some(())
  }
}

#[derive(Debug, Clone)]
pub(crate) struct CalRgb {
  white_point: [f32; 3],
  black_point: [f32; 3],
  matrix: [f32; 9],
  gamma: [f32; 3],
}

impl CalRgb {
  fn new(dict: &Dict<'_>) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(white_point_entry) = dict.get::<[f32; 3]>(WHITE_POINT);
    let white_point = match white_point_entry {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => [1.0, 1.0, 1.0],
    };
    let Ok(black_point_entry) = dict.get::<[f32; 3]>(BLACK_POINT);
    let black_point = match black_point_entry {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => [0.0, 0.0, 0.0],
    };
    let Ok(matrix_entry) = dict.get::<[f32; 9]>(MATRIX);
    let matrix = match matrix_entry {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
    };
    let Ok(gamma_entry) = dict.get::<[f32; 3]>(GAMMA);
    let gamma = match gamma_entry {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => [1.0, 1.0, 1.0],
    };

    Ok(Some(Self {
      white_point,
      black_point,
      matrix,
      gamma,
    }))
  }

  const BRADFORD_SCALE_MATRIX: [f32; 9] = [
    0.8951, 0.2664, -0.1614, -0.7502, 1.7135, 0.0367, 0.0389, -0.0685, 1.0296,
  ];

  const BRADFORD_SCALE_INVERSE_MATRIX: [f32; 9] = [
    0.9869929, -0.1470543, 0.1599627, 0.4323053, 0.5183603, 0.0492912, -0.0085287, 0.0400428,
    0.9684867,
  ];

  const SRGB_D65_XYZ_TO_RGB_MATRIX: [f32; 9] = [
    3.2404542, -1.5371385, -0.4985314, -0.969_266, 1.8760108, 0.0415560, 0.0556434, -0.2040259,
    1.0572252,
  ];

  const FLAT_WHITEPOINT: [f32; 3] = [1.0, 1.0, 1.0];
  const D65_WHITEPOINT: [f32; 3] = [0.95047, 1.0, 1.08883];

  fn decode_l_constant() -> core::result::Result<f32, console_core_never::Never> {
    Ok(((8.0_f32 + 16.0) / 116.0).powi(3) / 8.0)
  }

  fn srgb_transfer_function(
    PdfSample(color): PdfSample,
  ) -> core::result::Result<f32, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(color <= 0.0031308, ()) {
        ::core::option::Option::Some(()) => (12.92 * color).clamp(0.0, 1.0),
        ::core::option::Option::None => {
          match ::core::primitive::bool::then_some(color >= 0.99554525, ()) {
            ::core::option::Option::Some(()) => 1.0,
            ::core::option::Option::None => {
              ((1.0 + 0.055) * color.powf(1.0 / 2.4) - 0.055).clamp(0.0, 1.0)
            }
          }
        }
      },
    )
  }

  fn matrix_product(
    &[m00, m01, m02, m10, m11, m12, m20, m21, m22]: &[f32; 9],
    &[x, y, z]: &[f32; 3],
  ) -> Result<[f32; 3], Never> {
    Ok([
      m00 * x + m01 * y + m02 * z,
      m10 * x + m11 * y + m12 * z,
      m20 * x + m21 * y + m22 * z,
    ])
  }

  fn to_flat(&[p0, p1, p2]: &[f32; 3], &[l, m, s]: &[f32; 3]) -> Result<[f32; 3], Never> {
    Ok([l / p0, m / p1, s / p2])
  }

  fn to_d65(&[p0, p1, p2]: &[f32; 3], &[l, m, s]: &[f32; 3]) -> Result<[f32; 3], Never> {
    let [d0, d1, d2] = Self::D65_WHITEPOINT;
    Ok([l * d0 / p0, m * d1 / p1, s * d2 / p2])
  }

  fn decode_l(PdfSample(lightness): PdfSample) -> Result<f32, Never> {
    let negative = (lightness < 0.0).then_some(());
    let magnitude = match negative {
      Some(()) => -lightness,
      None => lightness,
    };
    let linear = match (magnitude > 8.0).then_some(()) {
      Some(()) => ((magnitude + 16.0) / 116.0).powi(3),
      None => {
        let Ok(coefficient) = Self::decode_l_constant();
        magnitude * coefficient
      }
    };

    Ok(match negative {
      Some(()) => -linear,
      None => linear,
    })
  }

  fn compensate_black_point(
    source_bp: &[f32; 3],
    xyz_flat: &[f32; 3],
  ) -> core::result::Result<[f32; 3], console_core_never::Never> {
    match ::core::primitive::bool::then_some(source_bp == &[0.0, 0.0, 0.0], ()) {
      ::core::option::Option::Some(()) => {
        return Ok(*xyz_flat);
      }
      ::core::option::Option::None => {}
    }

    let Ok(zero_decode_l) = Self::decode_l(PdfSample(0.0));

    let mut out = [0.0; 3];

    for ((source_bp_element, out_element), xyz_flat_element) in <[_]>::iter(source_bp)
      .zip(<[_]>::iter_mut(&mut out))
      .zip(<[_]>::iter(xyz_flat))
    {
      let Ok(src) = Self::decode_l(PdfSample(*source_bp_element));
      let scale = (1.0 - zero_decode_l) / (1.0 - src);
      let offset = 1.0 - scale;
      *out_element = *xyz_flat_element * scale + offset;
    }

    Ok(out)
  }

  fn normalize_white_point_to_flat(
    &self,
    source_white_point: &[f32; 3],
    xyz: &[f32; 3],
  ) -> core::result::Result<[f32; 3], console_core_never::Never> {
    match ::core::primitive::bool::then_some(
      ::core::primitive::f32::to_bits(match source_white_point {
        &[element, ..] => element,
      }) == ::core::primitive::f32::to_bits(1.0)
        && ::core::primitive::f32::to_bits(match source_white_point {
          &[_, _, element, ..] => element,
        }) == ::core::primitive::f32::to_bits(1.0),
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(*xyz);
      }
      ::core::option::Option::None => {}
    }

    let Ok(lms) = Self::matrix_product(&Self::BRADFORD_SCALE_MATRIX, xyz);
    let Ok(lms_flat) = Self::to_flat(source_white_point, &lms);
    let Ok(matrix_product) =
      Self::matrix_product(&Self::BRADFORD_SCALE_INVERSE_MATRIX, &lms_flat);

    Ok(matrix_product)
  }

  fn normalize_white_point_to_d65(
    &self,
    source_white_point: &[f32; 3],
    xyz: &[f32; 3],
  ) -> core::result::Result<[f32; 3], console_core_never::Never> {
    let Ok(lms) = Self::matrix_product(&Self::BRADFORD_SCALE_MATRIX, xyz);
    let Ok(lms_d65) = Self::to_d65(source_white_point, &lms);
    let Ok(matrix_product) = Self::matrix_product(&Self::BRADFORD_SCALE_INVERSE_MATRIX, &lms_d65);

    Ok(matrix_product)
  }
}

impl ToRgb for CalRgb {
  fn convert_f32(&self, input: &[f32], output: &mut [u8], _: InputScaling) -> Option<()> {
    let (samples, _partial_sample) = input.as_chunks::<3>();
    let (pixels, _partial_pixel) = output.as_chunks_mut::<3>();
    let [gr, gg, gb] = self.gamma;
    let [m00, m10, m20, m01, m11, m21, m02, m12, m22] = self.matrix;
    let matrix = [m00, m01, m02, m10, m11, m12, m20, m21, m22];

    for ([r, g, b], pixel) in samples.iter().zip(pixels) {
      let corrected = [(*r, gr), (*g, gg), (*b, gb)].map(|(sample, gamma)| {
        let clamped = sample.clamp(0.0, 1.0);

        match (clamped.to_bits() == 1.0_f32.to_bits()).then_some(()) {
          Some(()) => 1.0,
          None => clamped.powf(gamma),
        }
      });
      let Ok(xyz) = Self::matrix_product(&matrix, &corrected);
      let Ok(xyz_flat) = self.normalize_white_point_to_flat(&self.white_point, &xyz);
      let Ok(xyz_black) = Self::compensate_black_point(&self.black_point, &xyz_flat);
      let Ok(xyz_d65) = self.normalize_white_point_to_d65(&Self::FLAT_WHITEPOINT, &xyz_black);
      let Ok(rgb) = Self::matrix_product(&Self::SRGB_D65_XYZ_TO_RGB_MATRIX, &xyz_d65);
      *pixel = rgb.map(|linear| {
        let Ok(transferred) = Self::srgb_transfer_function(PdfSample(linear));
        let Ok(encoded) = f32_to_u8(PdfSample(transferred));
        encoded
      });
    }

    Some(())
  }
}

#[derive(Debug, Clone)]
pub(crate) struct Lab {
  range: [f32; 4],
  profile: ICCProfile,
}

impl Lab {
  fn new(dict: &Dict<'_>) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(white_point_entry) = dict.get::<[f32; 3]>(WHITE_POINT);
    let white_point = match white_point_entry {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => [1.0, 1.0, 1.0],
    };

    let Ok(black_point_entry) = dict.get::<[f32; 3]>(BLACK_POINT);
    let _black_point = match black_point_entry {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => [0.0, 0.0, 0.0],
    };
    let Ok(range_entry) = dict.get::<[f32; 4]>(RANGE);
    let range = match range_entry {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => [-100.0, 100.0, -100.0, 100.0],
    };

    let mut profile =
      match match ColorProfile::new_from_slice(include_bytes!("../assets/LAB.icc")) {
        Ok(parsed_value) => Some(parsed_value),
        Err(_invalid_color_profile) => None,
      } {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      };
    profile.white_point = Xyzd::new(
      f64::from(match white_point {
        [element, ..] => element,
      }),
      f64::from(match white_point {
        [_, element, ..] => element,
      }),
      f64::from(match white_point {
        [_, _, element, ..] => element,
      }),
    );

    let Ok(icc_profile) = ICCProfile::new_from_src_profile(
      profile,
      crate::SrgbProfile::NotSrgb,
      crate::LabProfile::NotLab,
      3,
    );
    let profile_2 = match icc_profile {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    Ok(Some(Self {
      range,
      profile: profile_2,
    }))
  }
}

impl ToRgb for Lab {
  fn convert_f32(&self, input: &[f32], output: &mut [u8], scaling: InputScaling) -> Option<()> {
    match scaling {
      InputScaling::ApplyColorSpace => {
        let (pixels, _partial_pixel) = input.as_chunks::<3>();
        let input_2 = pixels
          .iter()
          .flat_map(|[lightness, green_red, blue_yellow]| {
            let l = lightness / 100.0;
            let a = (green_red + 128.0) / 255.0;
            let b = (blue_yellow + 128.0) / 255.0;

            [l, a, b]
          })
          .collect::<Vec<_>>();

        self.profile.convert_f32(&input_2, output, scaling)
      }
      InputScaling::AlreadyScaled => self.profile.convert_f32(input, output, scaling),
    }
  }
}

#[derive(Debug, Clone)]
pub(crate) struct Indexed {
  values: Vec<Vec<f32>>,
  hival: u8,
  base: Box<ColorSpace>,
}

impl Indexed {
  fn finish(
    mut iter: FlexArrayIter<'_>,
    base: Option<ColorSpaceType>,
  ) -> Result<Option<Self>, console_core_never::Never> {
    let Ok(base_color_space) = match base {
      Some(base_2) => ColorSpace::from_kind(base_2),
      None => return Ok(None),
    };
    let Ok(highest_index_entry) = iter.next::<u32>();
    let Ok(hival) = console_core_number_conversion::wrapped::<_, u8>(
      (match highest_index_entry {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      })
      .min({
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(u8::MAX);
        numeric_value
      }),
    );

    let values = {
      let Ok(stream) = iter.next::<Stream<'_>>();
      let stream_bytes = stream.and_then(|stream_2| match stream_2.decoded() {
        Ok(bytes) => Some(bytes),
        Err(_invalid_palette_stream) => None,
      });
      let data = match stream_bytes {
        Some(bytes) => bytes,
        None => {
          let Ok(literal) = iter.next::<object::String<'_>>();

          match literal {
            Some(bytes) => Cow::Owned(bytes.to_vec()),
            None => return Ok(None),
          }
        }
      };

      let Ok(num_components) = base_color_space.num_components();

      let mut byte_iter = data.iter().copied();

      let mut vals = vec![];

      for _ in 0..=hival {
        let mut temp = vec![];

        for _ in 0..num_components {
          temp.push(
            f32::from(match byte_iter.next() {
              Some(explicit_value) => explicit_value,
              None => return Ok(None),
            }) / 255.0,
          );
        }

        vals.push(temp);
      }

      vals
    };

    Ok(Some(Self {
      values,
      hival,
      base: Box::new(base_color_space),
    }))
  }
}

impl Indexed {
  fn expand(&self, input: &[f32]) -> Result<Vec<f32>, ColorConversionError> {
    let mut values = Vec::new();

    for sample in input {
      let Ok(slot) = console_core_number_conversion::toward_zero_usize(f64::from(
        sample.clamp(0.0, f32::from(self.hival)) + 0.5,
      ));
      let palette = self
        .values
        .get(slot)
        .ok_or(ColorConversionError::MissingPaletteEntry)?;
      values.extend_from_slice(palette);
    }

    Ok(values)
  }
}

#[derive(Debug, Clone)]
pub(crate) struct Separation {
  alternate_space: ColorSpace,
  tint_transform: Function,
  coverage: InkCoverage,
}

impl Separation {
  fn expand(&self, input: &[f32]) -> Result<Vec<f32>, console_core_never::Never> {
    let mut values = Vec::new();

    for sample in input {
      let Ok(evaluated) = self.tint_transform.eval(vec![*sample]);
      let Ok(expanded) = alternate_values(evaluated, &self.alternate_space);
      values.extend(expanded);
    }

    Ok(values)
  }
}

fn alternate_values(
  evaluated: Option<Vec<f32>>,
  alternate: &ColorSpace,
) -> Result<Vec<f32>, console_core_never::Never> {
  match evaluated {
    Some(output) => Ok(output),
    None => alternate.initial_color(),
  }
}

#[derive(Debug, Clone)]
pub(crate) struct DeviceN {
  alternate_space: ColorSpace,
  num_components: NonZeroU8,
  tint_transform: Function,
  coverage: InkCoverage,
}

impl DeviceN {
  #[cfg_attr(
    dylint_lib = "explicit056_no_hidden_panic_api",
    expect(
      explicit056_no_hidden_panic_api,
      reason = "The native chunk stride is exactly the NonZeroU8 component count"
    )
  )]
  fn expand(&self, input: &[f32]) -> Result<Vec<f32>, Never> {
    let Ok(components) = console_core_number_conversion::index(self.num_components.get());
    let mut values = Vec::new();

    for sample in input.chunks_exact(components) {
      let Ok(evaluated) = self.tint_transform.eval(sample.to_vec());
      let Ok(expanded) = alternate_values(evaluated, &self.alternate_space);
      values.extend(expanded);
    }

    Ok(values)
  }
}

#[derive(Debug, Clone, Copy)]
enum Channels {
  Gray,
  Rgb,
  Cmyk,
}

impl Channels {
  fn count(self) -> Result<u8, console_core_never::Never> {
    Ok(match self {
      Self::Gray => 1,
      Self::Rgb => 3,
      Self::Cmyk => 4,
    })
  }

  fn layout(self) -> Result<Layout, console_core_never::Never> {
    Ok(match self {
      Self::Gray => Layout::Gray,
      Self::Rgb => Layout::Rgb,
      Self::Cmyk => Layout::Rgba,
    })
  }
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "moxcms returns thread-safe transform executors under its reference-counted interface"
  )
)]
struct ICCColorRepr {
  src_profile: ColorProfile,
  channels: Channels,
  srgb: crate::SrgbProfile,
  lab: crate::LabProfile,
  transform_u8: Arc<Transform8BitExecutor>,
  transform_f32: OnceLock<Option<Arc<TransformF32Executor>>>,
}

#[derive(Clone)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "decoded color transforms follow the ICC profile cached by the document and its deferred devices"
  )
)]
pub(crate) struct ICCProfile(Arc<ICCColorRepr>);

impl Debug for ICCProfile {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ICCColor {{..}}")
  }
}

impl ICCProfile {
  fn new(
    profile: &[u8],
    number_components: u32,
  ) -> Result<Option<Self>, console_core_never::Never> {
    let src_profile = match ColorProfile::new_from_slice(profile) {
      Ok(profile_2) => profile_2,
      Err(_invalid_color_profile) => return Ok(None),
    };
    let srgb = match profile.get(52..56) {
      Some(b"sRGB") => crate::SrgbProfile::Srgb,
      Some(_) | None => crate::SrgbProfile::NotSrgb,
    };
    let lab = match matches!(src_profile.color_space, DataColorSpace::Lab).then_some(()) {
      Some(()) => crate::LabProfile::Lab,
      None => crate::LabProfile::NotLab,
    };

    Self::new_from_src_profile(src_profile, srgb, lab, number_components)
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "this constructor stores one decoded profile and its native transforms for all uses of the cached color space"
    )
  )]
  fn new_from_src_profile(
    src_profile: ColorProfile,
    srgb: crate::SrgbProfile,
    lab: crate::LabProfile,
    number_components: u32,
  ) -> Result<Option<Self>, console_core_never::Never> {
    let channels = match number_components {
      1 => Channels::Gray,
      3 => Channels::Rgb,
      4 => Channels::Cmyk,
      _unexpected_count => {
        warn!("unsupported number of components {number_components} for ICC profile");

        return Ok(None);
      }
    };
    let Ok(layout) = channels.layout();
    let dest_profile = ColorProfile::new_srgb();
    let transform_u8 = match src_profile.clone().create_transform_8bit(
      layout,
      &dest_profile,
      Layout::Rgb,
      TransformOptions::default(),
    ) {
      Ok(transform) => transform,
      Err(_unavailable_color_transform) => return Ok(None),
    };

    Ok(Some(Self(Arc::new(ICCColorRepr {
      src_profile,
      channels,
      srgb,
      lab,
      transform_u8,
      transform_f32: OnceLock::new(),
    }))))
  }

  fn is_srgb(&self) -> Result<crate::SrgbProfile, console_core_never::Never> {
    Ok(self.0.srgb)
  }

  fn is_lab(&self) -> Result<crate::LabProfile, console_core_never::Never> {
    Ok(self.0.lab)
  }

  fn transform_u8(&self) -> Result<&Transform8BitExecutor, console_core_never::Never> {
    Ok(self.0.transform_u8.as_ref())
  }

  fn transform_f32(&self) -> Result<Option<&TransformF32Executor>, console_core_never::Never> {
    let transform = self.0.transform_f32.get_or_init(|| {
      let dest_profile = ColorProfile::new_srgb();
      let Ok(layout) = self.0.channels.layout();

      match self.0.src_profile.clone().create_transform_f32(
        layout,
        &dest_profile,
        Layout::Rgb,
        TransformOptions::default(),
      ) {
        Ok(transform) => Some(transform),
        Err(_unavailable_color_transform) => None,
      }
    });

    Ok(transform.as_deref())
  }
}

impl ToRgb for ICCProfile {
  fn convert_f32(&self, input: &[f32], output: &mut [u8], _: InputScaling) -> Option<()> {
    let Ok(is_lab) = self.is_lab();
    let values = match is_lab {
      crate::LabProfile::Lab => Cow::Owned(
        input
          .chunks_exact(3)
          .flat_map(|pixel| match pixel {
            [lightness, across, down] => [
              lightness * (1.0 / 100.0),
              (across + 128.0) * (1.0 / 255.0),
              (down + 128.0) * (1.0 / 255.0),
            ],
            _incomplete_pixel => [0.0, 0.0, 0.0],
          })
          .collect(),
      ),
      crate::LabProfile::NotLab => Cow::Borrowed(input),
    };
    let Ok(transform) = self.transform_f32();
    let selected = transform?;
    let mut temp = vec![0.0_f32; output.len()];

    match selected.transform(&values, &mut temp) {
      Ok(()) => {}
      Err(_invalid_color_transform_input) => return None,
    }

    for (input_2, output_2) in temp.iter().zip(output.iter_mut()) {
      *output_2 = {
        let Ok(numeric_value) =
          console_core_number_conversion::toward_zero_u8(f64::from(input_2 * 255.0 + 0.5));
        numeric_value
      };
    }

    Some(())
  }

  fn sample_conversion(&self) -> Result<SampleConversion, Never> {
    Ok(SampleConversion::ByteOrFloat)
  }

  fn convert_u8(
    &self,
    input: &[u8],
    output: &mut [u8],
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    let Ok(profile) = self.is_srgb();

    match profile {
      crate::SrgbProfile::Srgb => match (output.len() == input.len()).then_some(()) {
        Some(()) => {
          for (output_2, input_2) in output.iter_mut().zip(input) {
            *output_2 = *input_2;
          }
        }
        None => return Ok(None),
      },
      crate::SrgbProfile::NotSrgb => {
        let Ok(transform) = self.transform_u8();

        match transform.transform(input, output) {
          Ok(()) => {}
          Err(_unconvertible) => return Ok(None),
        }
      }
    }

    Ok(Some(()))
  }
}

#[inline(always)]
fn f32_to_u8(PdfSample(val): PdfSample) -> core::result::Result<u8, console_core_never::Never> {
  let Ok(numeric_value) =
    console_core_number_conversion::toward_zero_u8(f64::from(val * 255.0 + 0.5));

  Ok(numeric_value)
}

#[derive(Debug, Clone)]

pub struct Color {
  color_space: ColorSpace,
  components: ColorComponents,
  opacity: Opacity,
}

impl Color {
  pub(crate) fn new(
    color_space: ColorSpace,
    components: ColorComponents,
    opacity: Opacity,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      color_space,
      components,
      opacity,
    })
  }

  pub fn to_rgba(&self) -> core::result::Result<AlphaColor, console_core_never::Never> {
    let Ok(rgba) = self.color_space.to_rgba(
      &self.components,
      self.opacity.0,
      InputScaling::ApplyColorSpace,
    );

    Ok(rgba)
  }

  pub fn from_rgba(rgba: AlphaColor) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(c) = rgba.components();
    let Ok(color_space) = ColorSpace::device_rgb();

    Ok(Self {
      color_space,
      components: vec![
        match c {
          [_explicit_element, ..] => _explicit_element,
        },
        match c {
          [_, _explicit_element, ..] => _explicit_element,
        },
        match c {
          [_, _, _explicit_element, ..] => _explicit_element,
        },
      ],
      opacity: Opacity(match c {
        [_, _, _, alpha] => alpha,
      }),
    })
  }
}

#[cfg_attr(
  dylint_lib = "explicit044_no_ambient_value",
  expect(
    explicit044_no_ambient_value,
    reason = "the immutable transform is computed once from the embedded ICC profile; document processing only borrows it"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit068_no_global_mutable_state",
  expect(
    explicit068_no_global_mutable_state,
    reason = "the immutable transform is computed once from the embedded ICC profile; document processing only borrows it"
  )
)]
static CMYK_TRANSFORM: LazyLock<Option<ICCProfile>> = LazyLock::new(|| {
  let Ok(profile) = ICCProfile::new(include_bytes!("../assets/CGATS001Compat-v2-micro.icc"), 4);
  profile
});

fn convert_cmyk(
  input: &[u8],
  output: &mut [u8],
) -> Result<Option<()>, console_core_never::Never> {
  match CMYK_TRANSFORM.as_ref() {
    Some(profile) => profile.convert_u8(input, output),
    None => Ok(None),
  }
}

pub(crate) trait ToRgb {
  fn convert_sample(
    &self,
    input: &[f32],
    output: &mut [u8],
    scaling: InputScaling,
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    let Ok(conversion) = self.sample_conversion();

    match conversion {
      SampleConversion::ByteOrFloat => {
        let converted = input
          .iter()
          .copied()
          .map(|explicit_argument_0| {
            let Ok(explicit_value) = f32_to_u8(PdfSample(explicit_argument_0));
            explicit_value
          })
          .collect::<Vec<u8>>();

        let Ok(convert_u8) = self.convert_u8(&converted, output);

        match convert_u8 {
          ::core::option::Option::Some(_) => {
            return Ok(Some(()));
          }
          ::core::option::Option::None => {}
        }
      }
      SampleConversion::FloatOnly => {}
    }

    Ok(self.convert_f32(input, output, scaling))
  }

  fn convert_f32(&self, input: &[f32], output: &mut [u8], scaling: InputScaling) -> Option<()>;

  #[must_use]
  fn sample_conversion(&self) -> Result<SampleConversion, Never> {
    Ok(SampleConversion::FloatOnly)
  }

  fn convert_u8(
    &self,
    _: &[u8],
    _: &mut [u8],
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    Ok(None)
  }

  #[must_use]
  fn ink_coverage(&self) -> Result<InkCoverage, Never> {
    Ok(InkCoverage::Paints)
  }

  fn to_alpha_color(
    &self,
    input: &[f32],
    Opacity(mut opacity): Opacity,
    scaling: InputScaling,
  ) -> core::result::Result<Option<AlphaColor>, console_core_never::Never> {
    let mut output = [0; 3];
    let Ok(convert_sample) = self.convert_sample(input, &mut output, scaling);
    (match convert_sample {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    });

    let Ok(coverage) = self.ink_coverage();

    match coverage {
      InkCoverage::NoInk => opacity = 0.0,
      InkCoverage::Paints => {}
    }

    let Ok(alpha_color) = AlphaColor::from_rgba8(
      match output {
        [element, ..] => element,
      },
      match output {
        [_, element, ..] => element,
      },
      match output {
        [_, _, element, ..] => element,
      },
      {
        let Ok(numeric_value) =
          console_core_number_conversion::toward_zero_u8(f64::from(opacity * 255.0 + 0.5));
        numeric_value
      },
    );

    Ok(Some(alpha_color))
  }
}
