use core::hash::{Hash, Hasher};
use core::ops::Range;

use crate::{CacheKeyFlags, Metrics};

pub use fontdb::{Family, Stretch, Style, Weight};

#[derive(Clone, Copy, Debug, PartialOrd, Ord, Eq, Hash, PartialEq)]
pub struct Color(pub u32);

impl Color {
  pub const fn rgb(channels: [u8; 3]) -> Result<Self, console_core_never::Never> {
    let [red, green, blue] = channels;

    Ok(Self(u32::from_be_bytes([255, red, green, blue])))
  }

  pub const fn rgba(channels: [u8; 4]) -> Result<Self, console_core_never::Never> {
    let [red, green, blue, alpha] = channels;

    Ok(Self(u32::from_be_bytes([alpha, red, green, blue])))
  }

  pub const fn as_rgba_tuple(self) -> Result<(u8, u8, u8, u8), console_core_never::Never> {
    let [alpha, red, green, blue] = self.0.to_be_bytes();

    Ok((red, green, blue, alpha))
  }

  pub const fn as_rgba(self) -> Result<[u8; 4], console_core_never::Never> {
    let [alpha, red, green, blue] = self.0.to_be_bytes();

    Ok([red, green, blue, alpha])
  }

  pub const fn r(&self) -> Result<u8, console_core_never::Never> {
    let [_alpha, red, _green, _blue] = self.0.to_be_bytes();

    Ok(red)
  }

  pub const fn g(&self) -> Result<u8, console_core_never::Never> {
    let [_alpha, _red, green, _blue] = self.0.to_be_bytes();

    Ok(green)
  }

  pub const fn b(&self) -> Result<u8, console_core_never::Never> {
    let [_alpha, _red, _green, blue] = self.0.to_be_bytes();

    Ok(blue)
  }

  pub const fn a(&self) -> Result<u8, console_core_never::Never> {
    let [alpha, _red, _green, _blue] = self.0.to_be_bytes();

    Ok(alpha)
  }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum FamilyOwned {
  Name(String),
  Serif,
  SansSerif,
  Cursive,
  Fantasy,
  Monospace,
}

impl FamilyOwned {
  pub fn new(family: Family) -> Result<Self, console_core_never::Never> {
    {
      Ok(match family {
        Family::Name(name) => Self::Name(String::from(name)),
        Family::Serif => Self::Serif,
        Family::SansSerif => Self::SansSerif,
        Family::Cursive => Self::Cursive,
        Family::Fantasy => Self::Fantasy,
        Family::Monospace => Self::Monospace,
      })
    }
  }

  pub fn as_family(&self) -> Result<Family<'_>, console_core_never::Never> {
    {
      Ok(match self {
        Self::Name(name) => Family::Name(name),
        Self::Serif => Family::Serif,
        Self::SansSerif => Family::SansSerif,
        Self::Cursive => Family::Cursive,
        Self::Fantasy => Family::Fantasy,
        Self::Monospace => Family::Monospace,
      })
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CacheMetrics {
  font_size_bits: u32,
  line_height_bits: u32,
}

impl From<Metrics> for CacheMetrics {
  fn from(metrics: Metrics) -> Self {
    Self {
      font_size_bits: metrics.font_size.to_bits(),
      line_height_bits: metrics.line_height.to_bits(),
    }
  }
}

impl From<CacheMetrics> for Metrics {
  fn from(metrics: CacheMetrics) -> Self {
    Self {
      font_size: f32::from_bits(metrics.font_size_bits),
      line_height: f32::from_bits(metrics.line_height_bits),
    }
  }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FeatureTag([u8; 4]);

impl FeatureTag {
  pub const fn new(tag: &[u8; 4]) -> Result<Self, console_core_never::Never> {
    Ok(Self(*tag))
  }

  pub const KERNING: Self = match Self::new(b"kern") {
    Ok(value) => value,
  };

  pub const STANDARD_LIGATURES: Self = match Self::new(b"liga") {
    Ok(value) => value,
  };

  pub const CONTEXTUAL_LIGATURES: Self = match Self::new(b"clig") {
    Ok(value) => value,
  };

  pub const CONTEXTUAL_ALTERNATES: Self = match Self::new(b"calt") {
    Ok(value) => value,
  };

  pub const DISCRETIONARY_LIGATURES: Self = match Self::new(b"dlig") {
    Ok(value) => value,
  };

  pub const SMALL_CAPS: Self = match Self::new(b"smcp") {
    Ok(value) => value,
  };

  pub const ALL_SMALL_CAPS: Self = match Self::new(b"c2sc") {
    Ok(value) => value,
  };

  pub const STYLISTIC_SET_1: Self = match Self::new(b"ss01") {
    Ok(value) => value,
  };

  pub const STYLISTIC_SET_2: Self = match Self::new(b"ss02") {
    Ok(value) => value,
  };

  pub const fn as_bytes(&self) -> Result<&[u8; 4], console_core_never::Never> {
    Ok(&self.0)
  }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Feature {
  pub tag: FeatureTag,
  pub value: u32,
}

#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct FontFeatures {
  pub features: Vec<Feature>,
}

impl FontFeatures {
  pub const fn new() -> Result<Self, console_core_never::Never> {
    {
      Ok(Self {
        features: Vec::new(),
      })
    }
  }

  pub fn set(
    &mut self,
    tag: FeatureTag,
    value: u32,
  ) -> Result<&mut Self, console_core_never::Never> {
    {
      self.features.push(Feature { tag, value });
      Ok(self)
    }
  }

  pub fn enable(&mut self, tag: FeatureTag) -> Result<&mut Self, console_core_never::Never> {
    {
      let Ok(set) = self.set(tag, 1);

      Ok(set)
    }
  }

  pub fn disable(&mut self, tag: FeatureTag) -> Result<&mut Self, console_core_never::Never> {
    {
      let Ok(set) = self.set(tag, 0);

      Ok(set)
    }
  }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "letter spacing stores a fraction of an em"
  )
)]
pub struct LetterSpacing(pub f32);

impl PartialEq for LetterSpacing {
  fn eq(&self, other: &Self) -> bool {
    match ::core::primitive::bool::then_some(self.0.is_nan(), ()) {
      ::core::option::Option::Some(()) => other.0.is_nan(),
      ::core::option::Option::None => {
        <::core::primitive::f32 as ::core::cmp::PartialOrd>::partial_cmp(&self.0, &other.0)
          == ::core::option::Option::Some(::core::cmp::Ordering::Equal)
      }
    }
  }
}

impl Eq for LetterSpacing {}

impl Hash for LetterSpacing {
  fn hash<H: Hasher>(&self, hasher: &mut H) {
    const CANONICAL_NAN_BITS: u32 = 0x7fc0_0000;

    let bits = match ::core::primitive::bool::then_some(self.0.is_nan(), ()) {
      ::core::option::Option::Some(()) => CANONICAL_NAN_BITS,
      ::core::option::Option::None => (self.0 + 0.0).to_bits(),
    };

    bits.hash(hasher);
  }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum UnderlineStyle {
  #[default]
  None,
  Single,
  Double,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  allow(
    explicit048_no_unreal_state,
    reason = "Decoration colors can be configured before enabling each independent underline, strikethrough, or overline; every flag and color combination is meaningful to the builder."
  )
)]
pub struct TextDecoration {
  pub underline: UnderlineStyle,
  pub underline_color_opt: Option<Color>,
  pub strikethrough: crate::DecorationLine,
  pub strikethrough_color_opt: Option<Color>,
  pub overline: crate::DecorationLine,
  pub overline_color_opt: Option<Color>,
}

impl TextDecoration {
  pub const fn new() -> Result<Self, console_core_never::Never> {
    {
      Ok(Self {
        underline: UnderlineStyle::None,
        underline_color_opt: None,
        strikethrough: crate::DecorationLine::Hidden,
        strikethrough_color_opt: None,
        overline: crate::DecorationLine::Hidden,
        overline_color_opt: None,
      })
    }
  }

  pub const fn has_decoration(&self) -> Result<crate::Decoration, console_core_never::Never> {
    {
      Ok(match (self.underline, self.strikethrough, self.overline) {
        (UnderlineStyle::None, crate::DecorationLine::Hidden, crate::DecorationLine::Hidden) => {
          crate::Decoration::Plain
        }
        (_, _, _) => crate::Decoration::Decorated,
      })
    }
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the decoration offsets and stroke widths are fractions of an em"
  )
)]
pub struct DecorationMetrics {
  pub offset: f32,

  pub thickness: f32,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "the decoration ascent is a fraction of an em"
  )
)]
pub struct GlyphDecorationData {
  pub text_decoration: TextDecoration,

  pub underline_metrics: DecorationMetrics,

  pub strikethrough_metrics: DecorationMetrics,

  pub ascent: f32,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Attrs<'a> {
  pub color_opt: Option<Color>,
  pub family: Family<'a>,
  pub stretch: Stretch,
  pub style: Style,
  pub weight: Weight,
  pub metadata: u32,
  pub cache_key_flags: CacheKeyFlags,
  pub metrics_opt: Option<CacheMetrics>,

  pub letter_spacing_opt: Option<LetterSpacing>,
  pub font_features: FontFeatures,
  pub text_decoration: TextDecoration,
}

impl<'a> Attrs<'a> {
  pub const fn new() -> Result<Self, console_core_never::Never> {
    {
      Ok(Self {
        color_opt: None,
        family: Family::SansSerif,
        stretch: Stretch::Normal,
        style: Style::Normal,
        weight: Weight::NORMAL,
        metadata: 0,
        cache_key_flags: CacheKeyFlags::EMPTY,
        metrics_opt: None,
        letter_spacing_opt: None,
        font_features: FontFeatures {
          features: Vec::new(),
        },
        text_decoration: match TextDecoration::new() {
          Ok(value) => value,
        },
      })
    }
  }

  pub const fn color(mut self, color: Color) -> Result<Self, console_core_never::Never> {
    {
      self.color_opt = Some(color);
      Ok(self)
    }
  }

  pub const fn family(mut self, family: Family<'a>) -> Result<Self, console_core_never::Never> {
    {
      self.family = family;
      Ok(self)
    }
  }

  pub const fn stretch(mut self, stretch: Stretch) -> Result<Self, console_core_never::Never> {
    {
      self.stretch = stretch;
      Ok(self)
    }
  }

  pub const fn style(mut self, style: Style) -> Result<Self, console_core_never::Never> {
    {
      self.style = style;
      Ok(self)
    }
  }

  pub const fn weight(mut self, weight: Weight) -> Result<Self, console_core_never::Never> {
    {
      self.weight = weight;
      Ok(self)
    }
  }

  pub const fn metadata(mut self, metadata: u32) -> Result<Self, console_core_never::Never> {
    {
      self.metadata = metadata;
      Ok(self)
    }
  }

  pub const fn cache_key_flags(
    mut self,
    cache_key_flags: CacheKeyFlags,
  ) -> Result<Self, console_core_never::Never> {
    {
      self.cache_key_flags = cache_key_flags;
      Ok(self)
    }
  }

  pub fn metrics(mut self, metrics: Metrics) -> Result<Self, console_core_never::Never> {
    {
      self.metrics_opt = Some(CacheMetrics::from(metrics));
      Ok(self)
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "the public const builder accepts the scalar fraction of an em used by LetterSpacing"
    )
  )]
  pub const fn letter_spacing(
    mut self,
    letter_spacing: f32,
  ) -> Result<Self, console_core_never::Never> {
    {
      self.letter_spacing_opt = Some(LetterSpacing(letter_spacing));
      Ok(self)
    }
  }

  pub fn font_features(
    mut self,
    font_features: FontFeatures,
  ) -> Result<Self, console_core_never::Never> {
    {
      self.font_features = font_features;
      Ok(self)
    }
  }

  pub const fn underline(
    mut self,
    style: UnderlineStyle,
  ) -> Result<Self, console_core_never::Never> {
    {
      self.text_decoration.underline = style;
      Ok(self)
    }
  }

  pub const fn underline_color(
    mut self,
    color: Color,
  ) -> Result<Self, console_core_never::Never> {
    {
      self.text_decoration.underline_color_opt = Some(color);
      Ok(self)
    }
  }

  pub const fn strikethrough(mut self) -> Result<Self, console_core_never::Never> {
    {
      self.text_decoration.strikethrough = crate::DecorationLine::Visible;
      Ok(self)
    }
  }

  pub const fn strikethrough_color(
    mut self,
    color: Color,
  ) -> Result<Self, console_core_never::Never> {
    {
      self.text_decoration.strikethrough_color_opt = Some(color);
      Ok(self)
    }
  }

  pub const fn overline(mut self) -> Result<Self, console_core_never::Never> {
    {
      self.text_decoration.overline = crate::DecorationLine::Visible;
      Ok(self)
    }
  }

  pub const fn overline_color(mut self, color: Color) -> Result<Self, console_core_never::Never> {
    {
      self.text_decoration.overline_color_opt = Some(color);
      Ok(self)
    }
  }

  pub fn compatible(
    &self,
    other: &Self,
  ) -> Result<crate::Compatibility, console_core_never::Never> {
    {
      Ok(
        match ::core::primitive::bool::then_some(
          self.family == other.family
            && self.stretch == other.stretch
            && self.style == other.style
            && self.weight == other.weight,
          (),
        ) {
          ::core::option::Option::Some(()) => crate::Compatibility::Compatible,
          ::core::option::Option::None => crate::Compatibility::Incompatible,
        },
      )
    }
  }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FontMatchAttrs {
  family: FamilyOwned,
  stretch: Stretch,
  style: Style,
  weight: Weight,
}

impl<'a> From<&Attrs<'a>> for FontMatchAttrs {
  fn from(attrs: &Attrs<'a>) -> Self {
    let Ok(family) = FamilyOwned::new(attrs.family);

    Self {
      family,
      stretch: attrs.stretch,
      style: attrs.style,
      weight: attrs.weight,
    }
  }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AttrsOwned {
  pub color_opt: Option<Color>,
  pub family_owned: FamilyOwned,
  pub stretch: Stretch,
  pub style: Style,
  pub weight: Weight,
  pub metadata: u32,
  pub cache_key_flags: CacheKeyFlags,
  pub metrics_opt: Option<CacheMetrics>,

  pub letter_spacing_opt: Option<LetterSpacing>,
  pub font_features: FontFeatures,
  pub text_decoration: TextDecoration,
}

impl AttrsOwned {
  pub fn new(attrs: &Attrs) -> Result<Self, console_core_never::Never> {
    {
      Ok(Self {
        color_opt: attrs.color_opt,
        family_owned: match FamilyOwned::new(attrs.family) {
          Ok(value) => value,
        },
        stretch: attrs.stretch,
        style: attrs.style,
        weight: attrs.weight,
        metadata: attrs.metadata,
        cache_key_flags: attrs.cache_key_flags,
        metrics_opt: attrs.metrics_opt,
        letter_spacing_opt: attrs.letter_spacing_opt,
        font_features: attrs.font_features.clone(),
        text_decoration: attrs.text_decoration,
      })
    }
  }

  pub fn as_attrs(&self) -> Result<Attrs<'_>, console_core_never::Never> {
    {
      Ok(Attrs {
        color_opt: self.color_opt,
        family: match self.family_owned.as_family() {
          Ok(value) => value,
        },
        stretch: self.stretch,
        style: self.style,
        weight: self.weight,
        metadata: self.metadata,
        cache_key_flags: self.cache_key_flags,
        metrics_opt: self.metrics_opt,
        letter_spacing_opt: self.letter_spacing_opt,
        font_features: self.font_features.clone(),
        text_decoration: self.text_decoration,
      })
    }
  }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AttrsList {
  defaults: AttrsOwned,
  pub(crate) spans: Vec<(Range<u32>, AttrsOwned)>,
}

impl AttrsList {
  pub fn new(defaults: &Attrs) -> Result<Self, console_core_never::Never> {
    {
      let Ok(attrs_owned) = AttrsOwned::new(defaults);

      Ok(Self {
        defaults: attrs_owned,
        spans: Vec::new(),
      })
    }
  }

  pub fn defaults(&self) -> Result<Attrs<'_>, console_core_never::Never> {
    {
      let Ok(attrs) = self.defaults.as_attrs();

      Ok(attrs)
    }
  }

  pub fn spans(&self) -> Result<Vec<(&Range<u32>, &AttrsOwned)>, console_core_never::Never> {
    {
      let Ok(spans_iter) = self.spans_iter();

      Ok(spans_iter.collect())
    }
  }

  pub fn spans_iter(
    &self,
  ) -> Result<impl Iterator<Item = (&Range<u32>, &AttrsOwned)> + '_, console_core_never::Never>
  {
    Ok(self.spans.iter().map(|(range, attrs)| (range, attrs)))
  }

  pub fn clear_spans(&mut self) -> Result<(), console_core_never::Never> {
    {
      self.spans.clear();

      Ok(())
    }
  }

  pub fn add_span(
    &mut self,
    range: Range<u32>,
    attrs: &Attrs,
  ) -> Result<(), console_core_never::Never> {
    {
      match ::core::primitive::bool::then_some(range.is_empty(), ()) {
        ::core::option::Option::Some(()) => {
          return Ok(());
        }
        ::core::option::Option::None => {}
      }

      let Ok(attrs_2) = AttrsOwned::new(attrs);
      let mut spans = Vec::with_capacity(self.spans.len().saturating_add(2));

      'text_items_19: for (old_range, old_attrs) in self.spans.drain(..) {
        match ::core::primitive::bool::then_some(
          old_range.end <= range.start || old_range.start >= range.end,
          (),
        ) {
          ::core::option::Option::Some(()) => {
            spans.push((old_range, old_attrs));
            continue 'text_items_19;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(old_range.start < range.start, ()) {
          ::core::option::Option::Some(()) => {
            spans.push((old_range.start..range.start, old_attrs.clone()));
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(old_range.end > range.end, ()) {
          ::core::option::Option::Some(()) => {
            spans.push((range.end..old_range.end, old_attrs));
          }
          ::core::option::Option::None => {}
        }
      }

      let Ok(position) = console_core_number_conversion::fitted::<_, u64>(
        spans.partition_point(|(old_range, _)| old_range.start < range.start),
      );
      let Ok(boundary) = console_core_number_conversion::index(position);
      let mut existing = spans.into_iter();

      for span in existing.by_ref().take(boundary) {
        let Ok(()) = self.merge_span(span);
      }

      let Ok(()) = self.merge_span((range, attrs_2));

      for span in existing {
        let Ok(()) = self.merge_span(span);
      }

      Ok(())
    }
  }

  fn merge_span(
    &mut self,
    (range, attrs): (Range<u32>, AttrsOwned),
  ) -> Result<(), console_core_never::Never> {
    match self.spans.last_mut() {
      Some((previous, previous_attrs)) => {
        match (previous.end == range.start && *previous_attrs == attrs).then_some(()) {
          Some(()) => {
            previous.end = range.end;
            return Ok(());
          }
          None => {}
        }
      }
      None => {}
    }

    self.spans.push((range, attrs));

    Ok(())
  }

  pub fn get_span(&self, index: u32) -> Result<Attrs<'_>, console_core_never::Never> {
    let Ok(position) = console_core_number_conversion::fitted::<_, u32>(
      self.spans.partition_point(|(range, _)| range.end <= index),
    );
    let Ok(at) = console_core_number_conversion::index(position);

    match self
      .spans
      .get(at)
      .filter(|(range, _)| range.contains(&index))
    {
      Some((_, attrs)) => attrs.as_attrs(),
      None => self.defaults.as_attrs(),
    }
  }

  pub fn split_off(&mut self, index: u32) -> Result<Self, console_core_never::Never> {
    {
      let Ok(own_attrs) = self.defaults.as_attrs();
      let Ok(mut new) = Self::new(&own_attrs);
      let Ok(position) = console_core_number_conversion::fitted::<_, u32>(
        self.spans.partition_point(|(range, _)| range.end <= index),
      );
      new.spans = self
        .spans
        .split_off(match console_core_number_conversion::index(position) {
          Ok(position_2) => position_2,
        });

      match new.spans.first_mut() {
        Some((range, attrs)) => match ::core::primitive::bool::then_some(range.start < index, ())
        {
          ::core::option::Option::Some(()) => {
            self.spans.push((range.start..index, attrs.clone()));
            range.start = index;
          }
          ::core::option::Option::None => {}
        },
        None => {}
      }

      for (range, _) in &mut new.spans {
        range.start = range.start.saturating_sub(index);
        range.end = range.end.saturating_sub(index);
      }

      Ok(new)
    }
  }

  pub(crate) fn reset(mut self, default: &Attrs) -> Result<Self, console_core_never::Never> {
    {
      let Ok(attrs_owned) = AttrsOwned::new(default);
      self.defaults = attrs_owned;
      self.spans.clear();
      Ok(self)
    }
  }
}
