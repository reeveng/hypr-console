pub mod filter;
mod geom;
mod text;

use console_core_paths::{Point, StrictNum};
use std::collections::HashSet;
use std::fmt::Display;
use std::sync::Arc;

pub use console_core_finite_numbers::{self, NonZeroPositiveF32, NormalizedF32, PositiveF32};
pub use float_cmp::ApproxEqUlps;

pub use console_core_paths;

pub use self::geom::*;
pub use self::text::*;

use crate::OptionLog;

pub type Opacity = NormalizedF32;

#[derive(Debug)]
pub(crate) struct NonEmptyString(String);

impl NonEmptyString {
  pub(crate) fn with_numeric_suffix(
    prefix: &str,
    suffix: u64,
  ) -> Result<Self, console_core_never::Never> {
    Ok(Self(format!("{prefix}{suffix}")))
  }

  pub(crate) fn new(
    string: String,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    match ::core::primitive::bool::then_some(string.trim().is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    Ok(Some(NonEmptyString(string)))
  }

  pub(crate) fn get(&self) -> core::result::Result<&str, console_core_never::Never> {
    Ok(&self.0)
  }

  pub(crate) fn take(self) -> core::result::Result<String, console_core_never::Never> {
    Ok(self.0)
  }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "This type stores the nonzero unitless divisor used by SVG convolution kernels"
  )
)]
pub struct NonZeroF32(f32);

impl NonZeroF32 {
  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The public constructor accepts the unitless scalar read from filter data"
    )
  )]
  pub fn new(n: f32) -> core::result::Result<Option<Self>, console_core_never::Never> {
    Ok(match ::core::primitive::bool::then_some(n.approx_eq_ulps(&0.0, 4), ()) {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => Some(NonZeroF32(n)),
    })
  }

  #[inline]
  pub fn get(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.0)
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Units {
  UserSpaceOnUse,
  ObjectBoundingBox,
}

impl std::fmt::Display for Units {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Units::UserSpaceOnUse => write!(f, "userSpaceOnUse"),
      Units::ObjectBoundingBox => write!(f, "objectBoundingBox"),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[derive(Default)]
pub(crate) enum Visibility {
  #[default]
  Visible,
  Hidden,
  Collapse,
}

impl From<bool> for Visibility {
  fn from(visible: bool) -> Self {
    match visible.then_some(()) {
      Some(()) => Self::Visible,
      None => Self::Hidden,
    }
  }
}


#[derive(Clone, Copy, PartialEq, Debug)]
#[derive(Default)]
pub enum ShapeRendering {
  OptimizeSpeed,
  CrispEdges,
  #[default]
  GeometricPrecision,
}

impl ShapeRendering {
  pub fn use_shape_antialiasing(
    self,
  ) -> core::result::Result<crate::UseShapeAntialiasingAnswer, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some({
        match self {
          ShapeRendering::OptimizeSpeed => false,
          ShapeRendering::CrispEdges => false,
          ShapeRendering::GeometricPrecision => true,
        }
      }, ()) {
        ::core::option::Option::Some(()) => crate::UseShapeAntialiasingAnswer::Yes,
        ::core::option::Option::None => crate::UseShapeAntialiasingAnswer::No,
      },
    )
  }
}


impl std::str::FromStr for ShapeRendering {
  type Err = &'static str;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "optimizeSpeed" => Ok(ShapeRendering::OptimizeSpeed),
      "crispEdges" => Ok(ShapeRendering::CrispEdges),
      "geometricPrecision" => Ok(ShapeRendering::GeometricPrecision),
      _ => Err("invalid"),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[derive(Default)]
pub enum TextRendering {
  OptimizeSpeed,
  #[default]
  OptimizeLegibility,
  GeometricPrecision,
}


impl std::str::FromStr for TextRendering {
  type Err = &'static str;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "optimizeSpeed" => Ok(TextRendering::OptimizeSpeed),
      "optimizeLegibility" => Ok(TextRendering::OptimizeLegibility),
      "geometricPrecision" => Ok(TextRendering::GeometricPrecision),
      _ => Err("invalid"),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[derive(Default)]
pub enum ImageRendering {
  #[default]
  OptimizeQuality,
  OptimizeSpeed,

  Smooth,
  HighQuality,
  CrispEdges,
  Pixelated,
}


impl std::str::FromStr for ImageRendering {
  type Err = &'static str;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "optimizeQuality" => Ok(ImageRendering::OptimizeQuality),
      "optimizeSpeed" => Ok(ImageRendering::OptimizeSpeed),
      "smooth" => Ok(ImageRendering::Smooth),
      "high-quality" => Ok(ImageRendering::HighQuality),
      "crisp-edges" => Ok(ImageRendering::CrispEdges),
      "pixelated" => Ok(ImageRendering::Pixelated),
      _ => Err("invalid"),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[derive(Default)]
pub enum BlendMode {
  #[default]
  Normal,
  Multiply,
  Screen,
  Overlay,
  Darken,
  Lighten,
  ColorDodge,
  ColorBurn,
  HardLight,
  SoftLight,
  Difference,
  Exclusion,
  Hue,
  Saturation,
  Color,
  Luminosity,
}


impl Display for BlendMode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let blend_mode = match self {
      BlendMode::Normal => "normal",
      BlendMode::Multiply => "multiply",
      BlendMode::Screen => "screen",
      BlendMode::Overlay => "overlay",
      BlendMode::Darken => "darken",
      BlendMode::Lighten => "lighten",
      BlendMode::ColorDodge => "color-dodge",
      BlendMode::ColorBurn => "color-burn",
      BlendMode::HardLight => "hard-light",
      BlendMode::SoftLight => "soft-light",
      BlendMode::Difference => "difference",
      BlendMode::Exclusion => "exclusion",
      BlendMode::Hue => "hue",
      BlendMode::Saturation => "saturation",
      BlendMode::Color => "color",
      BlendMode::Luminosity => "luminosity",
    };
    write!(f, "{blend_mode}")
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[derive(Default)]
pub enum SpreadMethod {
  #[default]
  Pad,
  Reflect,
  Repeat,
}


#[derive(Debug)]
pub struct BaseGradient {
  pub(crate) id: NonEmptyString,
  pub(crate) units: Units,
  pub(crate) transform: Transform,
  pub(crate) spread_method: SpreadMethod,
  pub(crate) stops: Vec<Stop>,
}

impl BaseGradient {
  pub fn id(&self) -> core::result::Result<&str, console_core_never::Never> {
    let Ok(id) = self.id.get();

    Ok(id)
  }

  pub fn transform(&self) -> core::result::Result<Transform, console_core_never::Never> {
    Ok(self.transform)
  }

  pub fn spread_method(&self) -> core::result::Result<SpreadMethod, console_core_never::Never> {
    Ok(self.spread_method)
  }

  pub fn stops(&self) -> core::result::Result<&[Stop], console_core_never::Never> {
    Ok(&self.stops)
  }
}

#[derive(Debug)]
pub struct LinearGradient {
  pub(crate) base: BaseGradient,
  pub(crate) start: Point,
  pub(crate) end: Point,
}

impl LinearGradient {
  pub fn x1(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.start.x)
  }

  pub fn y1(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.start.y)
  }

  pub fn x2(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.end.x)
  }

  pub fn y2(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.end.y)
  }
}

impl std::ops::Deref for LinearGradient {
  type Target = BaseGradient;

  fn deref(&self) -> &Self::Target {
    &self.base
  }
}

#[derive(Debug)]
pub struct RadialGradient {
  pub(crate) base: BaseGradient,
  pub(crate) center: Point,
  pub(crate) r: PositiveF32,
  pub(crate) focal: Point,
  pub(crate) fr: PositiveF32,
}

impl RadialGradient {
  pub fn cx(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.center.x)
  }

  pub fn cy(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.center.y)
  }

  pub fn r(&self) -> core::result::Result<PositiveF32, console_core_never::Never> {
    Ok(self.r)
  }

  pub fn fx(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.focal.x)
  }

  pub fn fy(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.focal.y)
  }

  pub fn fr(&self) -> core::result::Result<PositiveF32, console_core_never::Never> {
    Ok(self.fr)
  }
}

impl std::ops::Deref for RadialGradient {
  type Target = BaseGradient;

  fn deref(&self) -> &Self::Target {
    &self.base
  }
}

pub type StopOffset = NormalizedF32;

#[derive(Clone, Copy, Debug)]
pub struct Stop {
  pub(crate) offset: StopOffset,
  pub(crate) color: Color,
  pub(crate) opacity: Opacity,
}

impl Stop {
  pub fn offset(&self) -> core::result::Result<StopOffset, console_core_never::Never> {
    Ok(self.offset)
  }

  pub fn color(&self) -> core::result::Result<Color, console_core_never::Never> {
    Ok(self.color)
  }

  pub fn opacity(&self) -> core::result::Result<Opacity, console_core_never::Never> {
    Ok(self.opacity)
  }
}

#[derive(Debug)]
pub struct Pattern {
  pub(crate) id: NonEmptyString,
  pub(crate) units: Units,
  pub(crate) content_units: Units,
  pub(crate) transform: Transform,
  pub(crate) rect: NonZeroRect,
  pub(crate) view_box: Option<ViewBox>,
  pub(crate) root: Group,
}

impl Pattern {
  pub fn id(&self) -> core::result::Result<&str, console_core_never::Never> {
    let Ok(id) = self.id.get();

    Ok(id)
  }

  pub fn transform(&self) -> core::result::Result<Transform, console_core_never::Never> {
    Ok(self.transform)
  }

  pub fn rect(&self) -> core::result::Result<NonZeroRect, console_core_never::Never> {
    Ok(self.rect)
  }

  pub fn root(&self) -> core::result::Result<&Group, console_core_never::Never> {
    Ok(&self.root)
  }
}

pub type StrokeWidth = NonZeroPositiveF32;

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The newtype represents the unitless miter-length to stroke-width ratio"
  )
)]
pub struct StrokeMiterlimit(f32);

impl StrokeMiterlimit {
  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The SVG miter limit is the unitless ratio of miter length to stroke width"
    )
  )]
  pub fn new(ratio: f32) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self(match ::core::primitive::bool::then_some(ratio.is_finite() && ratio >= 1.0, ()) {
      ::core::option::Option::Some(()) => ratio,
      ::core::option::Option::None => 1.0,
    }))
  }

  #[inline]
  pub fn get(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.0)
  }
}

impl Default for StrokeMiterlimit {
  #[inline]
  fn default() -> Self {
    let Ok(stroke_miterlimit) = StrokeMiterlimit::new(4.0);

    stroke_miterlimit
  }
}

impl From<f32> for StrokeMiterlimit {
  #[inline]
  fn from(n: f32) -> Self {
    let Ok(stroke_miterlimit) = Self::new(n);

    stroke_miterlimit
  }
}

impl PartialEq for StrokeMiterlimit {
  #[inline]
  fn eq(&self, other: &Self) -> bool {
    self.0.approx_eq_ulps(&other.0, 4)
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[derive(Default)]
pub enum LineCap {
  #[default]
  Butt,
  Round,
  Square,
}


#[derive(Clone, Copy, PartialEq, Debug)]
#[derive(Default)]
pub enum LineJoin {
  #[default]
  Miter,
  MiterClip,
  Round,
  Bevel,
}


#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "This newtype stores the signed dash offset in resolved SVG user units"
  )
)]
pub(crate) struct StrokeDashOffset(pub(crate) f32);

#[derive(Clone, Debug)]
pub struct Stroke {
  pub(crate) paint: Paint,
  pub(crate) dasharray: Option<Vec<f32>>,
  pub(crate) dashoffset: StrokeDashOffset,
  pub(crate) miterlimit: StrokeMiterlimit,
  pub(crate) opacity: Opacity,
  pub(crate) width: StrokeWidth,
  pub(crate) linecap: LineCap,
  pub(crate) linejoin: LineJoin,

  pub(crate) context_element: Option<ContextElement>,
}

impl Stroke {
  pub fn paint(&self) -> core::result::Result<&Paint, console_core_never::Never> {
    Ok(&self.paint)
  }

  pub fn dasharray(&self) -> core::result::Result<Option<&[f32]>, console_core_never::Never> {
    Ok(self.dasharray.as_deref())
  }

  pub fn dashoffset(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.dashoffset.0)
  }

  pub fn miterlimit(&self) -> core::result::Result<StrokeMiterlimit, console_core_never::Never> {
    Ok(self.miterlimit)
  }

  pub fn opacity(&self) -> core::result::Result<Opacity, console_core_never::Never> {
    Ok(self.opacity)
  }

  pub fn width(&self) -> core::result::Result<StrokeWidth, console_core_never::Never> {
    Ok(self.width)
  }

  pub fn linecap(&self) -> core::result::Result<LineCap, console_core_never::Never> {
    Ok(self.linecap)
  }

  pub fn linejoin(&self) -> core::result::Result<LineJoin, console_core_never::Never> {
    Ok(self.linejoin)
  }

  pub fn to_tiny_skia(
    &self,
  ) -> core::result::Result<console_core_paths::Stroke, console_core_never::Never> {
    let mut stroke = console_core_paths::Stroke {
      width: self.width.get(),
      miter_limit: ({
        let Ok(explicit_value) = self.miterlimit.get();
        explicit_value
      }),
      line_cap: match self.linecap {
        LineCap::Butt => console_core_paths::LineCap::Butt,
        LineCap::Round => console_core_paths::LineCap::Round,
        LineCap::Square => console_core_paths::LineCap::Square,
      },
      line_join: match self.linejoin {
        LineJoin::Miter => console_core_paths::LineJoin::Miter,
        LineJoin::MiterClip => console_core_paths::LineJoin::MiterClip,
        LineJoin::Round => console_core_paths::LineJoin::Round,
        LineJoin::Bevel => console_core_paths::LineJoin::Bevel,
      },

      dash: None,
    };

    match self.dasharray {
      Some(ref list) => {
        let Ok(stroke_dash) = console_core_paths::StrokeDash::new(list.clone(), self.dashoffset.0);
        stroke.dash = stroke_dash;
      }
      None => {}
    }

    Ok(stroke)
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[derive(Default)]
pub enum FillRule {
  #[default]
  NonZero,
  EvenOdd,
}


#[derive(Clone, Copy, Debug)]
pub(crate) enum ContextElement {
  UseNode,

  PathNode(Transform, Option<NonZeroRect>),
}

#[derive(Clone, Debug)]
pub struct Fill {
  pub(crate) paint: Paint,
  pub(crate) opacity: Opacity,
  pub(crate) rule: FillRule,

  pub(crate) context_element: Option<ContextElement>,
}

impl Fill {
  pub fn paint(&self) -> core::result::Result<&Paint, console_core_never::Never> {
    Ok(&self.paint)
  }

  pub fn opacity(&self) -> core::result::Result<Opacity, console_core_never::Never> {
    Ok(self.opacity)
  }

  pub fn rule(&self) -> core::result::Result<FillRule, console_core_never::Never> {
    Ok(self.rule)
  }
}

impl Default for Fill {
  fn default() -> Self {
    let Ok(black) = Color::black();

    Fill {
      paint: Paint::Color(black),
      opacity: Opacity::ONE,
      rule: FillRule::default(),
      context_element: None,
    }
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Color {
  pub red: u8,
  pub green: u8,
  pub blue: u8,
}

impl Color {
  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The published constructor takes conventional red, green and blue channel order"
    )
  )]
  pub fn new_rgb(
    red: u8,
    green: u8,
    blue: u8,
  ) -> core::result::Result<Color, console_core_never::Never> {
    Ok(Color { red, green, blue })
  }

  #[inline]
  pub fn black() -> core::result::Result<Color, console_core_never::Never> {
    let Ok(color) = Color::new_rgb(0, 0, 0);

    Ok(color)
  }

  #[inline]
  pub fn white() -> core::result::Result<Color, console_core_never::Never> {
    let Ok(color) = Color::new_rgb(255, 255, 255);

    Ok(color)
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The public paint variants point to independently reusable immutable document definitions"
  )
)]
pub enum Paint {
  Color(Color),
  LinearGradient(Arc<LinearGradient>),
  RadialGradient(Arc<RadialGradient>),
  Pattern(Arc<Pattern>),
}

impl PartialEq for Paint {
  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Each paint server keeps identity independently from its authored attributes"
    )
  )]
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Color(lc), Self::Color(rc)) => lc == rc,
      (Self::LinearGradient(lg1), Self::LinearGradient(lg2)) => Arc::ptr_eq(lg1, lg2),
      (Self::RadialGradient(rg1), Self::RadialGradient(rg2)) => Arc::ptr_eq(rg1, rg2),
      (Self::Pattern(p1), Self::Pattern(p2)) => Arc::ptr_eq(p1, p2),
      _ => false,
    }
  }
}

#[derive(Debug)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "A compiled clip may reuse another immutable clip already present in the document"
  )
)]
pub struct ClipPath {
  pub(crate) id: NonEmptyString,
  pub(crate) transform: Transform,
  pub(crate) clip_path: Option<Arc<ClipPath>>,
  pub(crate) root: Group,
}

impl ClipPath {
  pub(crate) fn empty(
    id: NonEmptyString,
  ) -> core::result::Result<Self, console_core_never::Never> {
    Ok(ClipPath {
      id,
      transform: Transform::default(),
      clip_path: None,
      root: ({
        let Ok(explicit_value) = Group::empty();
        explicit_value
      }),
    })
  }

  pub fn id(&self) -> core::result::Result<&str, console_core_never::Never> {
    let Ok(id) = self.id.get();

    Ok(id)
  }

  pub fn transform(&self) -> core::result::Result<Transform, console_core_never::Never> {
    Ok(self.transform)
  }

  pub fn clip_path(&self) -> core::result::Result<Option<&ClipPath>, console_core_never::Never> {
    Ok(self.clip_path.as_deref())
  }

  pub fn root(&self) -> core::result::Result<&Group, console_core_never::Never> {
    Ok(&self.root)
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[derive(Default)]
pub enum MaskType {
  #[default]
  Luminance,

  Alpha,
}


#[derive(Debug)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "A compiled mask may reuse another immutable mask already present in the document"
  )
)]
pub struct Mask {
  pub(crate) id: NonEmptyString,
  pub(crate) rect: NonZeroRect,
  pub(crate) kind: MaskType,
  pub(crate) mask: Option<Arc<Mask>>,
  pub(crate) root: Group,
}

impl Mask {
  pub fn id(&self) -> core::result::Result<&str, console_core_never::Never> {
    let Ok(id) = self.id.get();

    Ok(id)
  }

  pub fn rect(&self) -> core::result::Result<NonZeroRect, console_core_never::Never> {
    Ok(self.rect)
  }

  pub fn kind(&self) -> core::result::Result<MaskType, console_core_never::Never> {
    Ok(self.kind)
  }

  pub fn mask(&self) -> core::result::Result<Option<&Mask>, console_core_never::Never> {
    Ok(self.mask.as_deref())
  }

  pub fn root(&self) -> core::result::Result<&Group, console_core_never::Never> {
    Ok(&self.root)
  }
}

#[derive(Clone, Debug)]
pub enum Node {
  Group(Box<Group>),
  Path(Box<Path>),
  Image(Box<Image>),
  Text(Box<Text>),
}

impl Node {
  pub fn id(&self) -> core::result::Result<&str, console_core_never::Never> {
    Ok(match self {
      Node::Group(e) => e.id.as_str(),
      Node::Path(e) => e.id.as_str(),
      Node::Image(e) => e.id.as_str(),
      Node::Text(e) => e.id.as_str(),
    })
  }

  pub fn abs_transform(&self) -> core::result::Result<Transform, console_core_never::Never> {
    Ok(match self {
      Node::Group(group) => {
        let Ok(abs_transform) = group.abs_transform();

        abs_transform
      }
      Node::Path(path) => {
        let Ok(abs_transform) = path.abs_transform();

        abs_transform
      }
      Node::Image(image) => {
        let Ok(abs_transform) = image.abs_transform();

        abs_transform
      }
      Node::Text(text) => {
        let Ok(abs_transform) = text.abs_transform();

        abs_transform
      }
    })
  }

  pub fn bounding_box(&self) -> core::result::Result<Rect, console_core_never::Never> {
    Ok(match self {
      Node::Group(group) => {
        let Ok(bounding_box) = group.bounding_box();

        bounding_box
      }
      Node::Path(path) => {
        let Ok(bounding_box) = path.bounding_box();

        bounding_box
      }
      Node::Image(image) => {
        let Ok(bounding_box) = image.bounding_box();

        bounding_box
      }
      Node::Text(text) => {
        let Ok(bounding_box) = text.bounding_box();

        bounding_box
      }
    })
  }

  pub fn abs_bounding_box(&self) -> core::result::Result<Rect, console_core_never::Never> {
    Ok(match self {
      Node::Group(group) => {
        let Ok(abs_bounding_box) = group.abs_bounding_box();

        abs_bounding_box
      }
      Node::Path(path) => {
        let Ok(abs_bounding_box) = path.abs_bounding_box();

        abs_bounding_box
      }
      Node::Image(image) => {
        let Ok(abs_bounding_box) = image.abs_bounding_box();

        abs_bounding_box
      }
      Node::Text(text) => {
        let Ok(abs_bounding_box) = text.abs_bounding_box();

        abs_bounding_box
      }
    })
  }

  pub fn stroke_bounding_box(&self) -> core::result::Result<Rect, console_core_never::Never> {
    Ok(match self {
      Node::Group(group) => {
        let Ok(stroke_bounding_box) = group.stroke_bounding_box();

        stroke_bounding_box
      }
      Node::Path(path) => {
        let Ok(stroke_bounding_box) = path.stroke_bounding_box();

        stroke_bounding_box
      }

      Node::Image(image) => {
        let Ok(bounding_box) = image.bounding_box();

        bounding_box
      }
      Node::Text(text) => {
        let Ok(stroke_bounding_box) = text.stroke_bounding_box();

        stroke_bounding_box
      }
    })
  }

  pub fn abs_stroke_bounding_box(&self) -> core::result::Result<Rect, console_core_never::Never> {
    Ok(match self {
      Node::Group(group) => {
        let Ok(abs_stroke_bounding_box) = group.abs_stroke_bounding_box();

        abs_stroke_bounding_box
      }
      Node::Path(path) => {
        let Ok(abs_stroke_bounding_box) = path.abs_stroke_bounding_box();

        abs_stroke_bounding_box
      }

      Node::Image(image) => {
        let Ok(abs_bounding_box) = image.abs_bounding_box();

        abs_bounding_box
      }
      Node::Text(text) => {
        let Ok(abs_stroke_bounding_box) = text.abs_stroke_bounding_box();

        abs_stroke_bounding_box
      }
    })
  }

  pub fn abs_layer_bounding_box(
    &self,
  ) -> core::result::Result<Option<NonZeroRect>, console_core_never::Never> {
    Ok(match self {
      Node::Group(group) => {
        let Ok(abs_layer_bounding_box) = group.abs_layer_bounding_box();

        Some(abs_layer_bounding_box)
      }

      Node::Path(path) => {
        let Ok(abs_bounding_box) = path.abs_bounding_box();
        let Ok(non_zero_rect) = abs_bounding_box.to_non_zero_rect();

        non_zero_rect
      }
      Node::Image(image) => {
        let Ok(abs_bounding_box) = image.abs_bounding_box();
        let Ok(non_zero_rect) = abs_bounding_box.to_non_zero_rect();

        non_zero_rect
      }
      Node::Text(text) => {
        let Ok(abs_bounding_box) = text.abs_bounding_box();
        let Ok(non_zero_rect) = abs_bounding_box.to_non_zero_rect();

        non_zero_rect
      }
    })
  }

  pub fn subroots<'a, F: FnMut(&'a Group)>(
    &'a self,
    mut f: F,
  ) -> core::result::Result<(), console_core_never::Never> {
    match self {
      Node::Group(group) => {
        let Ok(subroots) = group.subroots(&mut f);

        subroots
      }
      Node::Path(path) => {
        let Ok(subroots) = path.subroots(&mut f);

        subroots
      }
      Node::Image(image) => {
        let Ok(subroots) = image.subroots(&mut f);

        subroots
      }
      Node::Text(text) => {
        let Ok(subroots) = text.subroots(&mut f);

        subroots
      }
    }

    Ok(())
  }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum GroupPaintContext {
  OwnBounds,
  ParentBounds,
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Groups preserve the identity of reusable clip, mask and filter definitions"
  )
)]
pub struct Group {
  pub(crate) id: String,
  pub(crate) transform: Transform,
  pub(crate) abs_transform: Transform,
  pub(crate) opacity: Opacity,
  pub(crate) blend_mode: BlendMode,
  pub(crate) isolation: crate::IsolateAnswer,
  pub(crate) clip_path: Option<Arc<ClipPath>>,

  pub(crate) paint_context: GroupPaintContext,
  pub(crate) mask: Option<Arc<Mask>>,
  pub(crate) filters: Vec<Arc<filter::Filter>>,
  pub(crate) bounding_box: Rect,
  pub(crate) abs_bounding_box: Rect,
  pub(crate) stroke_bounding_box: Rect,
  pub(crate) abs_stroke_bounding_box: Rect,
  pub(crate) layer_bounding_box: NonZeroRect,
  pub(crate) abs_layer_bounding_box: NonZeroRect,
  pub(crate) children: Vec<Node>,
}

impl Group {
  #[cfg_attr(
    dylint_lib = "explicit004_no_panic",
    expect(
      explicit004_no_panic,
      reason = "The constructor builds a checked zero-area origin box and a fully representable unit layer box from constants"
    )
  )]
  pub(crate) fn empty() -> core::result::Result<Self, console_core_never::Never> {
    let Ok(rect) = Rect::from_xywh(0.0, 0.0, 0.0, 0.0);
    let dummy = rect.expect("The origin admits a zero-area SVG bounding box");
    let Ok(unit_rect) = NonZeroRect::from_xywh(0.0, 0.0, 1.0, 1.0);
    let layer = unit_rect.expect("The unit layer box has strictly positive bounds");

    Ok(Group {
      id: String::new(),
      transform: Transform::default(),
      abs_transform: Transform::default(),
      opacity: Opacity::ONE,
      blend_mode: BlendMode::Normal,
      isolation: crate::IsolateAnswer::No,
      clip_path: None,
      mask: None,
      filters: Vec::new(),
      paint_context: GroupPaintContext::ParentBounds,
      bounding_box: dummy,
      abs_bounding_box: dummy,
      stroke_bounding_box: dummy,
      abs_stroke_bounding_box: dummy,
      layer_bounding_box: layer,
      abs_layer_bounding_box: layer,
      children: Vec::new(),
    })
  }

  pub fn id(&self) -> core::result::Result<&str, console_core_never::Never> {
    Ok(&self.id)
  }

  pub fn transform(&self) -> core::result::Result<Transform, console_core_never::Never> {
    Ok(self.transform)
  }

  pub fn abs_transform(&self) -> core::result::Result<Transform, console_core_never::Never> {
    Ok(self.abs_transform)
  }

  pub fn opacity(&self) -> core::result::Result<Opacity, console_core_never::Never> {
    Ok(self.opacity)
  }

  pub fn blend_mode(&self) -> core::result::Result<BlendMode, console_core_never::Never> {
    Ok(self.blend_mode)
  }

  pub fn isolate(&self) -> core::result::Result<crate::IsolateAnswer, console_core_never::Never> {
    Ok(self.isolation)
  }

  pub fn clip_path(&self) -> core::result::Result<Option<&ClipPath>, console_core_never::Never> {
    Ok(self.clip_path.as_deref())
  }

  pub fn mask(&self) -> core::result::Result<Option<&Mask>, console_core_never::Never> {
    Ok(self.mask.as_deref())
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The public view borrows the immutable filter handles without creating owners"
    )
  )]
  pub fn filters(
    &self,
  ) -> core::result::Result<&[Arc<filter::Filter>], console_core_never::Never> {
    Ok(&self.filters)
  }

  pub fn bounding_box(&self) -> core::result::Result<Rect, console_core_never::Never> {
    Ok(self.bounding_box)
  }

  pub fn abs_bounding_box(&self) -> core::result::Result<Rect, console_core_never::Never> {
    Ok(self.abs_bounding_box)
  }

  pub fn stroke_bounding_box(&self) -> core::result::Result<Rect, console_core_never::Never> {
    Ok(self.stroke_bounding_box)
  }

  pub fn abs_stroke_bounding_box(&self) -> core::result::Result<Rect, console_core_never::Never> {
    Ok(self.abs_stroke_bounding_box)
  }

  pub fn layer_bounding_box(
    &self,
  ) -> core::result::Result<NonZeroRect, console_core_never::Never> {
    Ok(self.layer_bounding_box)
  }

  pub fn abs_layer_bounding_box(
    &self,
  ) -> core::result::Result<NonZeroRect, console_core_never::Never> {
    Ok(self.abs_layer_bounding_box)
  }

  pub fn children(&self) -> core::result::Result<&[Node], console_core_never::Never> {
    Ok(&self.children)
  }

  pub fn should_isolate(
    &self,
  ) -> core::result::Result<crate::ShouldIsolateAnswer, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.isolation == crate::IsolateAnswer::Yes
        || self.opacity != Opacity::ONE
        || self.clip_path.is_some()
        || self.mask.is_some()
        || !self.filters.is_empty()
        || self.blend_mode != BlendMode::Normal, ())
      {
        ::core::option::Option::Some(()) => crate::ShouldIsolateAnswer::Yes,
        ::core::option::Option::None => crate::ShouldIsolateAnswer::No,
      },
    )
  }

  pub fn has_children(&self) -> core::result::Result<crate::Presence, console_core_never::Never> {
    Ok(match ::core::primitive::bool::then_some(!self.children.is_empty(), ()) {
      ::core::option::Option::Some(()) => crate::Presence::Present,
      ::core::option::Option::None => crate::Presence::Absent,
    })
  }

  pub fn filters_bounding_box(
    &self,
  ) -> core::result::Result<Option<NonZeroRect>, console_core_never::Never> {
    let mut full_region = BBox::default();

    for filter in &self.filters {
      let Ok(expand) = full_region.expand(filter.rect);
      full_region = expand;
    }

    let Ok(non_zero_rect) = full_region.to_non_zero_rect();

    Ok(non_zero_rect)
  }

  fn subroots<'a>(
    &'a self,
    f: &mut dyn FnMut(&'a Group),
  ) -> core::result::Result<(), console_core_never::Never> {
    match self.clip_path {
      Some(ref clip) => {
        f(&clip.root);

        match clip.clip_path {
          Some(ref sub_clip) => {
            f(&sub_clip.root);
          }
          None => {}
        }
      }
      None => {}
    }

    match self.mask {
      Some(ref mask) => {
        f(&mask.root);

        match mask.mask {
          Some(ref sub_mask) => {
            f(&sub_mask.root);
          }
          None => {}
        }
      }
      None => {}
    }

    for filter in &self.filters {
      for primitive in &filter.primitives {
        match primitive.kind {
          filter::Kind::Image(ref image) => {
            let Ok(root) = image.root();
            f(root);
          }
          filter::Kind::Blend(_)
          | filter::Kind::ColorMatrix(_)
          | filter::Kind::ComponentTransfer(_)
          | filter::Kind::Composite(_)
          | filter::Kind::ConvolveMatrix(_)
          | filter::Kind::DiffuseLighting(_)
          | filter::Kind::DisplacementMap(_)
          | filter::Kind::DropShadow(_)
          | filter::Kind::Flood(_)
          | filter::Kind::GaussianBlur(_)
          | filter::Kind::Merge(_)
          | filter::Kind::Morphology(_)
          | filter::Kind::Offset(_)
          | filter::Kind::SpecularLighting(_)
          | filter::Kind::Tile(_)
          | filter::Kind::Turbulence(_) => {}
        }
      }
    }

    Ok(())
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[derive(Default)]
pub enum PaintOrder {
  #[default]
  FillAndStroke,
  StrokeAndFill,
}


#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "A drawing path shares its immutable outline between transformed nodes and cloned trees"
  )
)]
pub struct Path {
  pub(crate) id: String,
  pub(crate) visible: Visibility,
  pub(crate) fill: Option<Fill>,
  pub(crate) stroke: Option<Stroke>,
  pub(crate) paint_order: PaintOrder,
  pub(crate) rendering_mode: ShapeRendering,
  pub(crate) data: Arc<console_core_paths::Path>,
  pub(crate) abs_transform: Transform,
  pub(crate) bounding_box: Rect,
  pub(crate) abs_bounding_box: Rect,
  pub(crate) stroke_bounding_box: Rect,
  pub(crate) abs_stroke_bounding_box: Rect,
}

impl Path {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The constructor takes the immutable geometric outline already held by the caller"
    )
  )]
  pub(crate) fn new_simple(
    data: Arc<console_core_paths::Path>,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(path) = Self::new(
      String::new(),
      Visibility::Visible,
      None,
      None,
      PaintOrder::default(),
      ShapeRendering::default(),
      data,
      Transform::default(),
    );

    Ok(path)
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Shape, fill and stroke nodes can keep a single immutable outline alongside separate paint properties"
    )
  )]
  #[expect(clippy::too_many_arguments, reason = "Path construction requires the outline, paint and placement together to validate bounds")]
  pub(crate) fn new(
    id: String,
    visible: Visibility,
    fill: Option<Fill>,
    stroke: Option<Stroke>,
    paint_order: PaintOrder,
    rendering_mode: ShapeRendering,
    data: Arc<console_core_paths::Path>,
    abs_transform: Transform,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(compute_tight_bounds) = data.compute_tight_bounds();
    let bounding_box = match compute_tight_bounds {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(calculate_stroke_bbox) = Path::calculate_stroke_bbox(stroke.as_ref(), &data);
    let stroke_bounding_box = match calculate_stroke_bbox {
      ::core::option::Option::Some(present) => present,
      ::core::option::Option::None => bounding_box,
    };

    let abs_bounding_box: Rect;
    let abs_stroke_bounding_box: Rect;

    let Ok(has_skew) = abs_transform.has_skew();

    match has_skew {
      console_core_paths::HasSkewAnswer::Yes => {
        let path2 = data.as_ref().clone();
        let Ok(transform) = path2.transform(abs_transform);
        let path2_2 = match transform {
          Some(value) => value,
          None => return Ok(None),
        };
        let Ok(path2_2_compute_tight_bounds) = path2_2.compute_tight_bounds();
        abs_bounding_box = match path2_2_compute_tight_bounds {
          Some(value) => value,
          None => return Ok(None),
        };
        let Ok(path_calculate_stroke_bbox) =
          Path::calculate_stroke_bbox(stroke.as_ref(), &path2_2);
        abs_stroke_bounding_box = match path_calculate_stroke_bbox {
          ::core::option::Option::Some(option_value) => option_value,
          ::core::option::Option::None => abs_bounding_box,
        };
      }
      console_core_paths::HasSkewAnswer::No => {
        let Ok(transform) = bounding_box.transform(abs_transform);
        abs_bounding_box = match transform {
          Some(value) => value,
          None => return Ok(None),
        };
        let Ok(stroke_bounding_box_transform) = stroke_bounding_box.transform(abs_transform);
        abs_stroke_bounding_box = match stroke_bounding_box_transform {
          Some(value) => value,
          None => return Ok(None),
        };
      }
    }

    Ok(Some(Path {
      id,
      visible,
      fill,
      stroke,
      paint_order,
      rendering_mode,
      data,
      abs_transform,
      bounding_box,
      abs_bounding_box,
      stroke_bounding_box,
      abs_stroke_bounding_box,
    }))
  }

  pub fn id(&self) -> core::result::Result<&str, console_core_never::Never> {
    Ok(&self.id)
  }

  pub fn is_visible(
    &self,
  ) -> core::result::Result<crate::IsVisibleAnswer, console_core_never::Never> {
    Ok(match self.visible {
      Visibility::Visible => crate::IsVisibleAnswer::Yes,
      Visibility::Hidden | Visibility::Collapse => crate::IsVisibleAnswer::No,
    })
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

  pub fn rendering_mode(
    &self,
  ) -> core::result::Result<ShapeRendering, console_core_never::Never> {
    Ok(self.rendering_mode)
  }

  pub fn data(
    &self,
  ) -> core::result::Result<&console_core_paths::Path, console_core_never::Never> {
    Ok(self.data.as_ref())
  }

  pub fn abs_transform(&self) -> core::result::Result<Transform, console_core_never::Never> {
    Ok(self.abs_transform)
  }

  pub fn bounding_box(&self) -> core::result::Result<Rect, console_core_never::Never> {
    Ok(self.bounding_box)
  }

  pub fn abs_bounding_box(&self) -> core::result::Result<Rect, console_core_never::Never> {
    Ok(self.abs_bounding_box)
  }

  pub fn stroke_bounding_box(&self) -> core::result::Result<Rect, console_core_never::Never> {
    Ok(self.stroke_bounding_box)
  }

  pub fn abs_stroke_bounding_box(&self) -> core::result::Result<Rect, console_core_never::Never> {
    Ok(self.abs_stroke_bounding_box)
  }

  fn calculate_stroke_bbox(
    stroke: Option<&Stroke>,
    path: &console_core_paths::Path,
  ) -> core::result::Result<Option<Rect>, console_core_never::Never> {
    let Ok(mut stroke_2) = match stroke {
      Some(value) => value,
      None => return Ok(None),
    }
    .to_tiny_skia();

    stroke_2.dash = None;

    let Ok(path_stroke) = path.stroke(&stroke_2, 1.0);

    match path_stroke {
      Some(stroked_path) => {
        let Ok(compute_tight_bounds) = stroked_path.compute_tight_bounds();
        return Ok(compute_tight_bounds);
      }
      None => {}
    }

    Ok(None)
  }

  fn subroots<'a>(
    &'a self,
    f: &mut dyn FnMut(&'a Group),
  ) -> core::result::Result<(), console_core_never::Never> {
    match self.fill.as_ref().map(|f_2| &f_2.paint) {
      Some(Paint::Pattern(patt)) => {
        let Ok(root) = patt.root();
        f(root);
      }
      Some(Paint::Color(_) | Paint::LinearGradient(_) | Paint::RadialGradient(_)) | None => {}
    }

    match self.stroke.as_ref().map(|f_3| &f_3.paint) {
      Some(Paint::Pattern(patt)) => {
        let Ok(root) = patt.root();
        f(root);
      }
      Some(Paint::Color(_) | Paint::LinearGradient(_) | Paint::RadialGradient(_)) | None => {}
    }

    Ok(())
  }
}

#[derive(Clone)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The public raster variants retain reusable immutable encoded bytes for multiple image nodes"
  )
)]
#[expect(clippy::large_enum_variant, reason = "The public image variants preserve their existing byte and tree representations")]
pub enum ImageKind {
  JPEG(Arc<Vec<u8>>),

  PNG(Arc<Vec<u8>>),

  GIF(Arc<Vec<u8>>),

  WEBP(Arc<Vec<u8>>),

  SVG(Tree),
}

impl ImageKind {
  pub(crate) fn actual_size(
    &self,
  ) -> core::result::Result<Option<Size>, console_core_never::Never> {
    Ok(match self {
      ImageKind::JPEG(data)
      | ImageKind::PNG(data)
      | ImageKind::GIF(data)
      | ImageKind::WEBP(data) => {
        let dimensions = match console_core_picture_sizes::size(data) {
          Ok(dimensions) => dimensions,
          Err(problem) => {
            console_core_logging::warn!("SVG image has invalid dimensions: {problem}");
            return Ok(None);
          }
        };
        let Ok(width) = console_core_number_conversion::nearest_f32_from_u64(u64::from(dimensions.width));
        let Ok(height) = console_core_number_conversion::nearest_f32_from_u64(u64::from(dimensions.height));
        let Ok(size) = Size::from_wh(width, height);

        size.log_none(|| console_core_logging::warn!("SVG image has invalid size"))
      }
      ImageKind::SVG(svg) => Some(svg.size),
    })
  }
}

impl std::fmt::Debug for ImageKind {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      ImageKind::JPEG(_) => f.write_str("ImageKind::JPEG(..)"),
      ImageKind::PNG(_) => f.write_str("ImageKind::PNG(..)"),
      ImageKind::GIF(_) => f.write_str("ImageKind::GIF(..)"),
      ImageKind::WEBP(_) => f.write_str("ImageKind::WEBP(..)"),
      ImageKind::SVG(_) => f.write_str("ImageKind::SVG(..)"),
    }
  }
}

#[derive(Clone, Debug)]
pub struct Image {
  pub(crate) id: String,
  pub(crate) visible: Visibility,
  pub(crate) size: Size,
  pub(crate) rendering_mode: ImageRendering,
  pub(crate) kind: ImageKind,
  pub(crate) abs_transform: Transform,
  pub(crate) abs_bounding_box: NonZeroRect,
}

impl Image {
  pub fn id(&self) -> core::result::Result<&str, console_core_never::Never> {
    Ok(&self.id)
  }

  pub fn is_visible(
    &self,
  ) -> core::result::Result<crate::IsVisibleAnswer, console_core_never::Never> {
    Ok(match self.visible {
      Visibility::Visible => crate::IsVisibleAnswer::Yes,
      Visibility::Hidden | Visibility::Collapse => crate::IsVisibleAnswer::No,
    })
  }

  pub fn size(&self) -> core::result::Result<Size, console_core_never::Never> {
    Ok(self.size)
  }

  pub fn rendering_mode(
    &self,
  ) -> core::result::Result<ImageRendering, console_core_never::Never> {
    Ok(self.rendering_mode)
  }

  pub fn kind(&self) -> core::result::Result<&ImageKind, console_core_never::Never> {
    Ok(&self.kind)
  }

  pub fn abs_transform(&self) -> core::result::Result<Transform, console_core_never::Never> {
    Ok(self.abs_transform)
  }

  #[cfg_attr(
    dylint_lib = "explicit004_no_panic",
    expect(
      explicit004_no_panic,
      reason = "Size validates finite positive dimensions and an origin at zero leaves those dimensions representable"
    )
  )]
  pub fn bounding_box(&self) -> core::result::Result<Rect, console_core_never::Never> {
    let Ok(rect) = self.size.to_rect(0.0, 0.0);

    Ok(rect.expect("The validated image size is representable at the origin"))
  }

  pub fn abs_bounding_box(&self) -> core::result::Result<Rect, console_core_never::Never> {
    let Ok(rect) = self.abs_bounding_box.to_rect();

    Ok(rect)
  }

  fn subroots<'a>(
    &'a self,
    f: &mut dyn FnMut(&'a Group),
  ) -> core::result::Result<(), console_core_never::Never> {
    match self.kind {
      ImageKind::SVG(ref tree) => {
        f(&tree.root);
      }
      ImageKind::JPEG(_) | ImageKind::PNG(_) | ImageKind::GIF(_) | ImageKind::WEBP(_) => {}
    }

    Ok(())
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The definition table and drawing nodes retain the same immutable resource identities"
  )
)]
pub struct Tree {
  pub(crate) size: Size,
  pub(crate) root: Group,
  pub(crate) linear_gradients: Vec<Arc<LinearGradient>>,
  pub(crate) radial_gradients: Vec<Arc<RadialGradient>>,
  pub(crate) patterns: Vec<Arc<Pattern>>,
  pub(crate) clip_paths: Vec<Arc<ClipPath>>,
  pub(crate) masks: Vec<Arc<Mask>>,
  pub(crate) filters: Vec<Arc<filter::Filter>>,
  #[cfg(feature = "text")]
  pub(crate) console_core_font_database: Arc<console_core_font_database::Database>,
}

impl Tree {
  pub fn size(&self) -> core::result::Result<Size, console_core_never::Never> {
    Ok(self.size)
  }

  pub fn root(&self) -> core::result::Result<&Group, console_core_never::Never> {
    Ok(&self.root)
  }

  pub fn node_by_id(
    &self,
    id: &str,
  ) -> core::result::Result<Option<&Node>, console_core_never::Never> {
    match ::core::primitive::bool::then_some(id.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    let Ok(mut nodes) = TreeNodes::new(&self.root, TreeWalk::Children);

    Ok(nodes.find(|node| {
      let Ok(candidate) = node.id();

      candidate == id
    }))
  }

  pub fn has_text_nodes(
    &self,
  ) -> core::result::Result<crate::HasTextNodesAnswer, console_core_never::Never> {
    let Ok(mut nodes) = TreeNodes::new(&self.root, TreeWalk::References);

    Ok(match ::core::primitive::bool::then_some(nodes.any(|node| matches!(node, Node::Text(_))), ()) {
      ::core::option::Option::Some(()) => crate::HasTextNodesAnswer::Yes,
      ::core::option::Option::None => crate::HasTextNodesAnswer::No,
    })
  }

  pub fn has_defs_nodes(
    &self,
  ) -> core::result::Result<crate::HasDefsNodesAnswer, console_core_never::Never> {
    let Ok(linear_gradients) = self.linear_gradients();

    Ok(
      match ::core::primitive::bool::then_some(!linear_gradients.is_empty()
        || !({
          let Ok(explicit_value) = self.radial_gradients();
          explicit_value
        })
        .is_empty()
        || !({
          let Ok(explicit_value) = self.patterns();
          explicit_value
        })
        .is_empty()
        || !({
          let Ok(explicit_value) = self.filters();
          explicit_value
        })
        .is_empty()
        || !({
          let Ok(explicit_value) = self.clip_paths();
          explicit_value
        })
        .is_empty()
        || !({
          let Ok(explicit_value) = self.masks();
          explicit_value
        })
        .is_empty(), ())
      {
        ::core::option::Option::Some(()) => crate::HasDefsNodesAnswer::Yes,
        ::core::option::Option::None => crate::HasDefsNodesAnswer::No,
      },
    )
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The public view borrows the tree's immutable gradient handles without creating owners"
    )
  )]
  pub fn linear_gradients(
    &self,
  ) -> core::result::Result<&[Arc<LinearGradient>], console_core_never::Never> {
    Ok(&self.linear_gradients)
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The public view borrows the tree's immutable gradient handles without creating owners"
    )
  )]
  pub fn radial_gradients(
    &self,
  ) -> core::result::Result<&[Arc<RadialGradient>], console_core_never::Never> {
    Ok(&self.radial_gradients)
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The public view borrows the tree's immutable pattern handles without creating owners"
    )
  )]
  pub fn patterns(&self) -> core::result::Result<&[Arc<Pattern>], console_core_never::Never> {
    Ok(&self.patterns)
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The public view borrows the tree's immutable clip handles without creating owners"
    )
  )]
  pub fn clip_paths(&self) -> core::result::Result<&[Arc<ClipPath>], console_core_never::Never> {
    Ok(&self.clip_paths)
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The public view borrows the tree's immutable mask handles without creating owners"
    )
  )]
  pub fn masks(&self) -> core::result::Result<&[Arc<Mask>], console_core_never::Never> {
    Ok(&self.masks)
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The public view borrows the immutable filter handles without creating owners"
    )
  )]
  pub fn filters(
    &self,
  ) -> core::result::Result<&[Arc<filter::Filter>], console_core_never::Never> {
    Ok(&self.filters)
  }

  #[cfg(feature = "text")]
  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The established accessor exposes the immutable catalogue borrowed by the parsed tree"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(explicit002_infallible_result, reason = "Font consumers borrow the existing shared catalogue through this public accessor")
  )]
  pub fn console_core_font_database(&self) -> &Arc<console_core_font_database::Database> {
    &self.console_core_font_database
  }

  pub(crate) fn collect_definitions(&mut self) -> Result<(), console_core_never::Never> {
    let Ok(()) = self.collect_paint_servers();
    let Ok(()) = self.collect_effects();

    Ok(())
  }

  fn collect_paint_servers(&mut self) -> Result<(), console_core_never::Never> {
    let Ok(mut linear) = Definitions::new(&mut self.linear_gradients);
    let Ok(mut radial) = Definitions::new(&mut self.radial_gradients);
    let Ok(mut patterns) = Definitions::new(&mut self.patterns);
    let Ok(nodes) = TreeNodes::new(&self.root, TreeWalk::ChildrenThenReferences);

    for node in nodes {
      let path = match node {
        Node::Path(path) => path,
        Node::Group(_) | Node::Image(_) | Node::Text(_) => continue,
      };
      let paints = [
        path.fill.as_ref().map(|fill| &fill.paint),
        path.stroke.as_ref().map(|stroke| &stroke.paint),
      ];

      for paint in paints.into_iter().flatten() {
        match paint {
          Paint::Color(_) => {}
          Paint::LinearGradient(gradient) => {
            let Ok(()) = linear.insert(gradient);
          }
          Paint::RadialGradient(gradient) => {
            let Ok(()) = radial.insert(gradient);
          }
          Paint::Pattern(pattern) => {
            let Ok(()) = patterns.insert(pattern);
          }
        }
      }
    }

    Ok(())
  }

  fn collect_effects(&mut self) -> Result<(), console_core_never::Never> {
    let Ok(mut clips) = Definitions::new(&mut self.clip_paths);
    let Ok(mut masks) = Definitions::new(&mut self.masks);
    let Ok(mut filters) = Definitions::new(&mut self.filters);
    let Ok(nodes) = TreeNodes::new(&self.root, TreeWalk::ReferencesThenChildren);

    for node in nodes {
      let group = match node {
        Node::Group(group) => group,
        Node::Path(_) | Node::Image(_) | Node::Text(_) => continue,
      };

      match &group.clip_path {
        Some(clip) => {
          let Ok(()) = clips.insert(clip);

          match &clip.clip_path {
            Some(link) => {
              let Ok(()) = clips.insert(link);
            }
            None => {}
          }
        }
        None => {}
      }

      match &group.mask {
        Some(mask) => {
          let Ok(()) = masks.insert(mask);

          match &mask.mask {
            Some(link) => {
              let Ok(()) = masks.insert(link);
            }
            None => {}
          }
        }
        None => {}
      }

      for filter in &group.filters {
        let Ok(()) = filters.insert(filter);
      }
    }

    Ok(())
  }
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The definition table holds the exact same immutable objects used by drawing nodes"
  )
)]
struct Definitions<'a, T> {
  seen: HashSet<*const T>,
  entries: &'a mut Vec<Arc<T>>,
}

impl<'a, T> Definitions<'a, T> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Pointer identity distinguishes separate definitions with equal attributes"
    )
  )]
  fn new(entries: &'a mut Vec<Arc<T>>) -> Result<Self, console_core_never::Never> {
    Ok(Self {
      seen: entries.iter().map(Arc::as_ptr).collect(),
      entries,
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Each immutable definition is placed once in the tree table and remains shared by referencing nodes"
    )
  )]
  fn insert(&mut self, definition: &Arc<T>) -> Result<(), console_core_never::Never> {
    match ::core::primitive::bool::then_some(self.seen.insert(Arc::as_ptr(definition)), ()) {
      ::core::option::Option::Some(()) => self.entries.push(Arc::clone(definition)),
      ::core::option::Option::None => {}
    }

    Ok(())
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TreeWalk {
  Children,
  References,
  ChildrenThenReferences,
  ReferencesThenChildren,
  #[cfg(feature = "writer")]
  TextPathDefinitions,
}

pub(crate) struct TreeNodes<'a> {
  walk: TreeWalk,
  pending: Vec<PendingGroup<'a>>,
  visited: HashSet<*const Group>,
  groups: Vec<&'a Group>,
}

enum PendingGroup<'a> {
  Unvisited(&'a Group),
  Children(std::slice::Iter<'a, Node>),
}

impl<'a> TreeNodes<'a> {
  pub(crate) fn new(root: &'a Group, walk: TreeWalk) -> Result<Self, console_core_never::Never> {
    Ok(Self {
      walk,
      pending: vec![PendingGroup::Unvisited(root)],
      visited: HashSet::new(),
      groups: Vec::new(),
    })
  }

  fn enqueue(&mut self, node: &'a Node) -> Result<(), console_core_never::Never> {
    let groups = &mut self.groups;
    let child_group = match node {
      Node::Group(group) => Some(group.as_ref()),
      Node::Path(_) | Node::Image(_) | Node::Text(_) => None,
    };

    match self.walk {
      TreeWalk::Children => groups.extend(child_group),
      TreeWalk::References => {
        let sinks = &mut *groups;
        let Ok(()) = node.subroots(move |group| sinks.push(group));
      }
      TreeWalk::ChildrenThenReferences => {
        groups.extend(child_group);
        let sinks = &mut *groups;
        let Ok(()) = node.subroots(move |group| sinks.push(group));
      }
      TreeWalk::ReferencesThenChildren => {
        let sinks = &mut *groups;
        let Ok(()) = node.subroots(move |group| sinks.push(group));
        groups.extend(child_group);
      }
      #[cfg(feature = "writer")]
      TreeWalk::TextPathDefinitions => {
        groups.extend(child_group);
        let sinks = &mut *groups;
        let Ok(()) = node.subroots(move |group| sinks.push(group));
      }
    }

    for group in groups.drain(..).rev() {
      self.pending.push(PendingGroup::Unvisited(group));
    }

    Ok(())
  }
}

impl<'a> Iterator for TreeNodes<'a> {
  type Item = &'a Node;

  #[cfg_attr(
    dylint_lib = "explicit053_no_while",
    expect(
      explicit053_no_while,
      reason = "The heap stack of child iterators drains when all reachable nodes have been visited"
    )
  )]
  fn next(&mut self) -> Option<Self::Item> {
    while let Some(frame) = self.pending.pop() {
      let mut children = match frame {
        PendingGroup::Children(children) => children,
        PendingGroup::Unvisited(group) => match ::core::primitive::bool::then_some(self.visited.insert(std::ptr::from_ref(group)), ()) {
          ::core::option::Option::Some(()) => group.children.iter(),
          ::core::option::Option::None => continue,
        },
      };

      match children.next() {
        Some(node) => {
          self.pending.push(PendingGroup::Children(children));
          let Ok(()) = self.enqueue(node);

          return Some(node);
        }
        None => {}
      }
    }

    None
  }
}

fn child_effect_bounds(child: &Node, bounds: Rect) -> Result<Rect, console_core_never::Never> {
  match child {
    Node::Group(group) => match bounds.transform(group.transform) {
      Ok(Some(mapped)) => Ok(mapped),
      Ok(None) => Ok(bounds),
    },
    Node::Path(_) | Node::Image(_) | Node::Text(_) => Ok(bounds),
  }
}

impl Group {
  pub(crate) fn calculate_object_bbox(
    &mut self,
  ) -> core::result::Result<Option<NonZeroRect>, console_core_never::Never> {
    let mut bbox = BBox::default();

    for child in &self.children {
      let Ok(mut c_bbox) = child.bounding_box();

      match child {
        Node::Group(group) => {
          let Ok(transform) = c_bbox.transform(group.transform);

          match transform {
            Some(r) => {
              c_bbox = r;
            }
            None => {}
          }
        }
        Node::Path(_) | Node::Image(_) | Node::Text(_) => {}
      }

      let Ok(expand) = bbox.expand(c_bbox);
      bbox = expand;
    }

    let Ok(non_zero_rect) = bbox.to_non_zero_rect();

    Ok(non_zero_rect)
  }

  pub(crate) fn calculate_bounding_boxes(
    &mut self,
  ) -> core::result::Result<Option<()>, console_core_never::Never> {
    let mut bbox = BBox::default();
    let mut abs_bbox = BBox::default();
    let mut stroke_bbox = BBox::default();
    let mut abs_stroke_bbox = BBox::default();
    let mut layer_bbox = BBox::default();

    for child in &self.children {
      let Ok(local) = child.bounding_box();
      let Ok(local_bbox) = child_effect_bounds(child, local);
      let Ok(expanded) = bbox.expand(local_bbox);
      bbox = expanded;

      let Ok(abs_bounding_box) = child.abs_bounding_box();
      let Ok(expand) = abs_bbox.expand(abs_bounding_box);
      abs_bbox = expand;

      let Ok(stroke) = child.stroke_bounding_box();
      let Ok(local_stroke) = child_effect_bounds(child, stroke);
      let Ok(expanded_stroke) = stroke_bbox.expand(local_stroke);
      stroke_bbox = expanded_stroke;

      let Ok(abs_stroke_bounding_box) = child.abs_stroke_bounding_box();
      let Ok(abs_stroke_bbox_expand) = abs_stroke_bbox.expand(abs_stroke_bounding_box);
      abs_stroke_bbox = abs_stroke_bbox_expand;

      match child {
        Node::Group(group) => {
          let r = group.layer_bounding_box;

          let Ok(transform) = r.transform(group.transform);

          match transform {
            Some(r_2) => {
              let Ok(layer_bbox_expand) = layer_bbox.expand(r_2);
              layer_bbox = layer_bbox_expand;
            }
            None => {}
          }
        }
        Node::Path(_) | Node::Image(_) | Node::Text(_) => {
          let Ok(stroke_bounding_box) = child.stroke_bounding_box();
          let Ok(layer_bbox_expand) = layer_bbox.expand(stroke_bounding_box);
          layer_bbox = layer_bbox_expand;
        }
      }
    }

    let Ok(rect) = bbox.to_rect();

    match rect {
      Some(bbox_2) => {
        self.bounding_box = bbox_2;
        let Ok(abs_bbox_rect) = abs_bbox.to_rect();
        self.abs_bounding_box = match abs_bbox_rect {
          Some(value) => value,
          None => return Ok(None),
        };
        let Ok(stroke_bbox_rect) = stroke_bbox.to_rect();
        self.stroke_bounding_box = match stroke_bbox_rect {
          Some(value) => value,
          None => return Ok(None),
        };
        let Ok(abs_stroke_bbox_rect) = abs_stroke_bbox.to_rect();
        self.abs_stroke_bounding_box = match abs_stroke_bbox_rect {
          Some(value) => value,
          None => return Ok(None),
        };
      }
      None => {}
    }

    let Ok(filters_bounding_box) = self.filters_bounding_box();

    match filters_bounding_box {
      Some(filter_bbox) => {
        self.layer_bounding_box = filter_bbox;
      }
      None => {
        let Ok(non_zero_rect) = layer_bbox.to_non_zero_rect();
        self.layer_bounding_box = match non_zero_rect {
          Some(value) => value,
          None => return Ok(None),
        };
      }
    }

    let Ok(transform) = self.layer_bounding_box.transform(self.abs_transform);
    self.abs_layer_bounding_box = match transform {
      Some(value) => value,
      None => return Ok(None),
    };

    Ok(Some(()))
  }
}
