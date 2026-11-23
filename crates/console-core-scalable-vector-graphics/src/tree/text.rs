use std::sync::Arc;

use console_core_finite_numbers::NonZeroPositiveF32;
pub use console_core_scalable_vector_graphics_values::FontFamily;

#[cfg(feature = "text")]
use crate::layout::Span;
use crate::{Fill, Group, NonEmptyString, PaintOrder, FloatRectangle, Stroke, TextRendering, Transform};

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Debug, Hash)]
pub enum FontStretch {
  UltraCondensed,
  ExtraCondensed,
  Condensed,
  SemiCondensed,
  Normal,
  SemiExpanded,
  Expanded,
  ExtraExpanded,
  UltraExpanded,
}

impl Default for FontStretch {
  #[inline]
  fn default() -> Self {
    Self::Normal
  }
}

#[cfg(feature = "text")]
impl From<console_core_font_database::Stretch> for FontStretch {
  fn from(stretch: console_core_font_database::Stretch) -> Self {
    match stretch {
      console_core_font_database::Stretch::UltraCondensed => FontStretch::UltraCondensed,
      console_core_font_database::Stretch::ExtraCondensed => FontStretch::ExtraCondensed,
      console_core_font_database::Stretch::Condensed => FontStretch::Condensed,
      console_core_font_database::Stretch::SemiCondensed => FontStretch::SemiCondensed,
      console_core_font_database::Stretch::Normal => FontStretch::Normal,
      console_core_font_database::Stretch::SemiExpanded => FontStretch::SemiExpanded,
      console_core_font_database::Stretch::Expanded => FontStretch::Expanded,
      console_core_font_database::Stretch::ExtraExpanded => FontStretch::ExtraExpanded,
      console_core_font_database::Stretch::UltraExpanded => FontStretch::UltraExpanded,
    }
  }
}

#[cfg(feature = "text")]
impl From<FontStretch> for console_core_font_database::Stretch {
  fn from(stretch: FontStretch) -> Self {
    match stretch {
      FontStretch::UltraCondensed => console_core_font_database::Stretch::UltraCondensed,
      FontStretch::ExtraCondensed => console_core_font_database::Stretch::ExtraCondensed,
      FontStretch::Condensed => console_core_font_database::Stretch::Condensed,
      FontStretch::SemiCondensed => console_core_font_database::Stretch::SemiCondensed,
      FontStretch::Normal => console_core_font_database::Stretch::Normal,
      FontStretch::SemiExpanded => console_core_font_database::Stretch::SemiExpanded,
      FontStretch::Expanded => console_core_font_database::Stretch::Expanded,
      FontStretch::ExtraExpanded => console_core_font_database::Stretch::ExtraExpanded,
      FontStretch::UltraExpanded => console_core_font_database::Stretch::UltraExpanded,
    }
  }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "This type stores resolved SVG user-space lengths"
  )
)]
pub(crate) struct UserLength(pub(crate) f32);

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "OpenType variation coordinates use the units of the public axis tag"
  )
)]
pub struct FontVariation {
  pub tag: [u8; 4],

  pub value: f32,
}

impl FontVariation {
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The public constructor accepts a coordinate in the tagged OpenType axis"
    )
  )]
  pub fn new(tag: [u8; 4], value: f32) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { tag, value })
  }
}

impl PartialEq for FontVariation {
  fn eq(&self, other: &Self) -> bool {
    self.tag == other.tag && self.value.to_bits() == other.value.to_bits()
  }
}

impl Eq for FontVariation {}

impl std::hash::Hash for FontVariation {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.tag.hash(state);
    self.value.to_bits().hash(state);
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FontStyle {
  Normal,

  Italic,

  Oblique,
}

impl Default for FontStyle {
  #[inline]
  fn default() -> FontStyle {
    Self::Normal
  }
}

#[cfg(feature = "text")]
impl From<console_core_font_database::Style> for FontStyle {
  fn from(style: console_core_font_database::Style) -> Self {
    match style {
      console_core_font_database::Style::Normal => FontStyle::Normal,
      console_core_font_database::Style::Italic => FontStyle::Italic,
      console_core_font_database::Style::Oblique => FontStyle::Oblique,
    }
  }
}

#[cfg(feature = "text")]
impl From<FontStyle> for console_core_font_database::Style {
  fn from(style: FontStyle) -> Self {
    match style {
      FontStyle::Normal => console_core_font_database::Style::Normal,
      FontStyle::Italic => console_core_font_database::Style::Italic,
      FontStyle::Oblique => console_core_font_database::Style::Oblique,
    }
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Font {
  pub(crate) families: Vec<FontFamily>,
  pub(crate) style: FontStyle,
  pub(crate) stretch: FontStretch,
  pub(crate) weight: u16,
  pub(crate) variations: Vec<FontVariation>,
}

impl Font {
  pub fn families(&self) -> core::result::Result<&[FontFamily], console_core_never::Never> {
    Ok(&self.families)
  }

  pub fn style(&self) -> core::result::Result<FontStyle, console_core_never::Never> {
    Ok(self.style)
  }

  pub fn stretch(&self) -> core::result::Result<FontStretch, console_core_never::Never> {
    Ok(self.stretch)
  }

  pub fn weight(&self) -> core::result::Result<u16, console_core_never::Never> {
    Ok(self.weight)
  }

  pub fn variations(&self) -> core::result::Result<&[FontVariation], console_core_never::Never> {
    Ok(&self.variations)
  }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum DominantBaseline {
  #[default]
  Auto,
  UseScript,
  NoChange,
  ResetSize,
  Ideographic,
  Alphabetic,
  Hanging,
  Mathematical,
  Central,
  Middle,
  TextAfterEdge,
  TextBeforeEdge,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum AlignmentBaseline {
  #[default]
  Auto,
  Baseline,
  BeforeEdge,
  TextBeforeEdge,
  Middle,
  Central,
  AfterEdge,
  TextAfterEdge,
  Ideographic,
  Alphabetic,
  Hanging,
  Mathematical,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BaselineShift {
  Baseline,
  Subscript,
  Superscript,
  Number(f32),
}

impl Default for BaselineShift {
  #[inline]
  fn default() -> BaselineShift {
    BaselineShift::Baseline
  }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum LengthAdjust {
  #[default]
  Spacing,
  SpacingAndGlyphs,
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum FontOpticalSizing {
  #[default]
  Auto,

  None,
}

#[derive(Clone, Debug)]
pub struct TextDecorationStyle {
  pub(crate) fill: Option<Fill>,
  pub(crate) stroke: Option<Stroke>,
}

impl TextDecorationStyle {
  pub fn fill(&self) -> core::result::Result<Option<&Fill>, console_core_never::Never> {
    Ok(self.fill.as_ref())
  }

  pub fn stroke(&self) -> core::result::Result<Option<&Stroke>, console_core_never::Never> {
    Ok(self.stroke.as_ref())
  }
}

#[derive(Clone, Debug)]
pub struct TextDecoration {
  pub(crate) underline: Option<TextDecorationStyle>,
  pub(crate) overline: Option<TextDecorationStyle>,
  pub(crate) line_through: Option<TextDecorationStyle>,
}

impl TextDecoration {
  pub fn underline(
    &self,
  ) -> core::result::Result<Option<&TextDecorationStyle>, console_core_never::Never> {
    Ok(self.underline.as_ref())
  }

  pub fn overline(
    &self,
  ) -> core::result::Result<Option<&TextDecorationStyle>, console_core_never::Never> {
    Ok(self.overline.as_ref())
  }

  pub fn line_through(
    &self,
  ) -> core::result::Result<Option<&TextDecorationStyle>, console_core_never::Never> {
    Ok(self.line_through.as_ref())
  }
}

#[derive(Clone, Debug)]
pub struct TextSpan {
  pub(crate) byte_range: std::ops::Range<u64>,
  pub(crate) fill: Option<Fill>,
  pub(crate) stroke: Option<Stroke>,
  pub(crate) paint_order: PaintOrder,
  pub(crate) font: Font,
  pub(crate) font_size: NonZeroPositiveF32,
  pub(crate) small_caps: crate::SmallCapsAnswer,
  pub(crate) apply_kerning: crate::ApplyKerningAnswer,
  pub(crate) font_optical_sizing: FontOpticalSizing,
  pub(crate) decoration: TextDecoration,
  pub(crate) dominant_baseline: DominantBaseline,
  pub(crate) alignment_baseline: AlignmentBaseline,
  pub(crate) baseline_shift: Vec<BaselineShift>,
  pub(crate) visibility: crate::Visibility,
  pub(crate) letter_spacing: UserLength,
  pub(crate) word_spacing: UserLength,
  pub(crate) text_length: Option<UserLength>,
  pub(crate) length_adjust: LengthAdjust,
}

impl TextSpan {
  pub fn start(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    console_core_number_conversion::wrapped_index(self.byte_range.start)
  }

  pub fn end(
    &self,
  ) -> core::result::Result<::console_core_api_compatibility::NativeSize, console_core_never::Never>
  {
    console_core_number_conversion::wrapped_index(self.byte_range.end)
  }

  pub fn fill(&self) -> core::result::Result<Option<&Fill>, console_core_never::Never> {
    Ok(self.fill.as_ref())
  }

  pub fn stroke(&self) -> core::result::Result<Option<&Stroke>, console_core_never::Never> {
    Ok(self.stroke.as_ref())
  }

  pub fn paint_order(&self) -> core::result::Result<PaintOrder, console_core_never::Never> {
    Ok(self.paint_order)
  }

  pub fn font(&self) -> core::result::Result<&Font, console_core_never::Never> {
    Ok(&self.font)
  }

  pub fn font_size(&self) -> core::result::Result<NonZeroPositiveF32, console_core_never::Never> {
    Ok(self.font_size)
  }

  pub fn small_caps(
    &self,
  ) -> core::result::Result<crate::SmallCapsAnswer, console_core_never::Never> {
    Ok(self.small_caps)
  }

  pub fn apply_kerning(
    &self,
  ) -> core::result::Result<crate::ApplyKerningAnswer, console_core_never::Never> {
    Ok(self.apply_kerning)
  }

  pub fn font_optical_sizing(
    &self,
  ) -> core::result::Result<FontOpticalSizing, console_core_never::Never> {
    Ok(self.font_optical_sizing)
  }

  pub fn decoration(&self) -> core::result::Result<&TextDecoration, console_core_never::Never> {
    Ok(&self.decoration)
  }

  pub fn dominant_baseline(
    &self,
  ) -> core::result::Result<DominantBaseline, console_core_never::Never> {
    Ok(self.dominant_baseline)
  }

  pub fn alignment_baseline(
    &self,
  ) -> core::result::Result<AlignmentBaseline, console_core_never::Never> {
    Ok(self.alignment_baseline)
  }

  pub fn baseline_shift(
    &self,
  ) -> core::result::Result<&[BaselineShift], console_core_never::Never> {
    Ok(&self.baseline_shift)
  }

  pub fn is_visible(
    &self,
  ) -> core::result::Result<crate::IsVisibleAnswer, console_core_never::Never> {
    Ok(match self.visibility {
      crate::Visibility::Visible => crate::IsVisibleAnswer::Yes,
      crate::Visibility::Hidden | crate::Visibility::Collapse => crate::IsVisibleAnswer::No,
    })
  }

  pub fn letter_spacing(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.letter_spacing.0)
  }

  pub fn word_spacing(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.word_spacing.0)
  }

  pub fn text_length(&self) -> core::result::Result<Option<f32>, console_core_never::Never> {
    Ok(self.text_length.map(|length| length.0))
  }

  pub fn length_adjust(&self) -> core::result::Result<LengthAdjust, console_core_never::Never> {
    Ok(self.length_adjust)
  }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum TextAnchor {
  #[default]
  Start,
  Middle,
  End,
}

#[derive(Debug)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The parsed text path shares an immutable outline with its serialized definition"
  )
)]
pub struct TextPath {
  pub(crate) id: NonEmptyString,
  pub(crate) start_offset: UserLength,
  pub(crate) path: Arc<console_core_paths::Path>,
}

impl TextPath {
  pub fn id(&self) -> core::result::Result<&str, console_core_never::Never> {
    let Ok(id) = self.id.get();

    Ok(id)
  }

  pub fn start_offset(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.start_offset.0)
  }

  pub fn path(
    &self,
  ) -> core::result::Result<&console_core_paths::Path, console_core_never::Never> {
    Ok(&self.path)
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The public path variant preserves the same immutable text path across chunks"
  )
)]
pub enum TextFlow {
  Linear,

  Path(Arc<TextPath>),
}

#[derive(Clone, Debug)]
pub struct TextChunk {
  pub(crate) x: Option<f32>,
  pub(crate) y: Option<f32>,
  pub(crate) anchor: TextAnchor,
  pub(crate) spans: Vec<TextSpan>,
  pub(crate) text_flow: TextFlow,
  pub(crate) text: String,
}

impl TextChunk {
  pub fn x(&self) -> core::result::Result<Option<f32>, console_core_never::Never> {
    Ok(self.x)
  }

  pub fn y(&self) -> core::result::Result<Option<f32>, console_core_never::Never> {
    Ok(self.y)
  }

  pub fn anchor(&self) -> core::result::Result<TextAnchor, console_core_never::Never> {
    Ok(self.anchor)
  }

  pub fn spans(&self) -> core::result::Result<&[TextSpan], console_core_never::Never> {
    Ok(&self.spans)
  }

  pub fn text_flow(&self) -> core::result::Result<TextFlow, console_core_never::Never> {
    Ok(self.text_flow.clone())
  }

  pub fn text(&self) -> core::result::Result<&str, console_core_never::Never> {
    Ok(&self.text)
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum WritingMode {
  LeftToRight,
  TopToBottom,
}

#[derive(Clone, Debug)]
pub struct Text {
  pub(crate) id: String,
  pub(crate) rendering_mode: TextRendering,
  pub(crate) dx: Vec<f32>,
  pub(crate) dy: Vec<f32>,
  pub(crate) rotate: Vec<f32>,
  pub(crate) writing_mode: WritingMode,
  pub(crate) chunks: Vec<TextChunk>,
  pub(crate) abs_transform: Transform,
  pub(crate) bounding_box: FloatRectangle,
  pub(crate) abs_bounding_box: FloatRectangle,
  pub(crate) stroke_bounding_box: FloatRectangle,
  pub(crate) abs_stroke_bounding_box: FloatRectangle,
  pub(crate) flattened: Box<Group>,
  #[cfg(feature = "text")]
  pub(crate) layouted: Vec<Span>,
}

impl Text {
  pub fn id(&self) -> core::result::Result<&str, console_core_never::Never> {
    Ok(&self.id)
  }

  pub fn rendering_mode(&self) -> core::result::Result<TextRendering, console_core_never::Never> {
    Ok(self.rendering_mode)
  }

  pub fn dx(&self) -> core::result::Result<&[f32], console_core_never::Never> {
    Ok(&self.dx)
  }

  pub fn dy(&self) -> core::result::Result<&[f32], console_core_never::Never> {
    Ok(&self.dy)
  }

  pub fn rotate(&self) -> core::result::Result<&[f32], console_core_never::Never> {
    Ok(&self.rotate)
  }

  pub fn writing_mode(&self) -> core::result::Result<WritingMode, console_core_never::Never> {
    Ok(self.writing_mode)
  }

  pub fn chunks(&self) -> core::result::Result<&[TextChunk], console_core_never::Never> {
    Ok(&self.chunks)
  }

  pub fn abs_transform(&self) -> core::result::Result<Transform, console_core_never::Never> {
    Ok(self.abs_transform)
  }

  pub fn bounding_box(&self) -> core::result::Result<FloatRectangle, console_core_never::Never> {
    Ok(self.bounding_box)
  }

  pub fn abs_bounding_box(&self) -> core::result::Result<FloatRectangle, console_core_never::Never> {
    Ok(self.abs_bounding_box)
  }

  pub fn stroke_bounding_box(&self) -> core::result::Result<FloatRectangle, console_core_never::Never> {
    Ok(self.stroke_bounding_box)
  }

  pub fn abs_stroke_bounding_box(&self) -> core::result::Result<FloatRectangle, console_core_never::Never> {
    Ok(self.abs_stroke_bounding_box)
  }

  pub fn flattened(&self) -> core::result::Result<&Group, console_core_never::Never> {
    Ok(&self.flattened)
  }

  #[cfg(feature = "text")]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "Existing text consumers iterate this borrowed slice through the public accessor"
    )
  )]
  pub fn layouted(&self) -> &[Span] {
    &self.layouted
  }

  pub(crate) fn subroots<'a>(
    &'a self,
    f: &mut dyn FnMut(&'a Group),
  ) -> core::result::Result<(), console_core_never::Never> {
    f(&self.flattened);

    Ok(())
  }
}
