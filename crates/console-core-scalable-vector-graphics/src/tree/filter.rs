//! SVG filter types.

use console_core_finite_numbers::PositiveF32;
use console_core_never::Never;
use console_core_paths::Point;

use crate::{BlendMode, Color, Group, NonEmptyString, NonZeroF32, NonZeroRect, Opacity};

#[derive(Debug)]
pub struct Filter {
  pub(crate) id: NonEmptyString,
  pub(crate) rect: NonZeroRect,
  pub(crate) primitives: Vec<Primitive>,
}

impl Filter {
  pub fn id(&self) -> core::result::Result<&str, console_core_never::Never> {
    let Ok(id) = self.id.get();

    Ok(id)
  }

  pub fn rect(&self) -> core::result::Result<NonZeroRect, console_core_never::Never> {
    Ok(self.rect)
  }

  pub fn primitives(&self) -> core::result::Result<&[Primitive], console_core_never::Never> {
    Ok(&self.primitives)
  }
}

#[derive(Clone, Debug)]
pub struct Primitive {
  pub(crate) rect: NonZeroRect,
  pub(crate) color_interpolation: ColorInterpolation,
  pub(crate) result: String,
  pub(crate) kind: Kind,
}

impl Primitive {
  pub fn rect(&self) -> core::result::Result<NonZeroRect, console_core_never::Never> {
    Ok(self.rect)
  }

  pub fn color_interpolation(
    &self,
  ) -> core::result::Result<ColorInterpolation, console_core_never::Never> {
    Ok(self.color_interpolation)
  }

  pub fn result(&self) -> core::result::Result<&str, console_core_never::Never> {
    Ok(&self.result)
  }

  pub fn kind(&self) -> core::result::Result<&Kind, console_core_never::Never> {
    Ok(&self.kind)
  }
}

#[derive(Clone, Debug)]
pub enum Kind {
  Blend(Blend),
  ColorMatrix(ColorMatrix),
  ComponentTransfer(ComponentTransfer),
  Composite(Composite),
  ConvolveMatrix(ConvolveMatrix),
  DiffuseLighting(DiffuseLighting),
  DisplacementMap(DisplacementMap),
  DropShadow(DropShadow),
  Flood(Flood),
  GaussianBlur(GaussianBlur),
  Image(Image),
  Merge(Merge),
  Morphology(Morphology),
  Offset(Offset),
  SpecularLighting(SpecularLighting),
  Tile(Tile),
  Turbulence(Turbulence),
}

impl Kind {
  pub fn has_input(
    &self,
    input: &Input,
  ) -> core::result::Result<crate::HasInputAnswer, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some({
        match self {
          Kind::Blend(fe) => fe.input1 == *input || fe.input2 == *input,
          Kind::ColorMatrix(fe) => fe.input == *input,
          Kind::ComponentTransfer(fe) => fe.input == *input,
          Kind::Composite(fe) => fe.input1 == *input || fe.input2 == *input,
          Kind::ConvolveMatrix(fe) => fe.input == *input,
          Kind::DiffuseLighting(fe) => fe.input == *input,
          Kind::DisplacementMap(fe) => fe.input1 == *input || fe.input2 == *input,
          Kind::DropShadow(fe) => fe.input == *input,
          Kind::Flood(_) => false,
          Kind::GaussianBlur(fe) => fe.input == *input,
          Kind::Image(_) => false,
          Kind::Merge(fe) => fe.inputs.iter().any(|i| i == input),
          Kind::Morphology(fe) => fe.input == *input,
          Kind::Offset(fe) => fe.input == *input,
          Kind::SpecularLighting(fe) => fe.input == *input,
          Kind::Tile(fe) => fe.input == *input,
          Kind::Turbulence(_) => false,
        }
      }, ()) {
        ::core::option::Option::Some(()) => crate::HasInputAnswer::Yes,
        ::core::option::Option::None => crate::HasInputAnswer::No,
      },
    )
  }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Input {
  SourceGraphic,
  SourceAlpha,
  Reference(String),
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum ColorInterpolation {
  SRGB,
  #[default]
  LinearRGB,
}

#[derive(Clone, Debug)]
pub struct Blend {
  pub(crate) input1: Input,
  pub(crate) input2: Input,
  pub(crate) mode: BlendMode,
}

impl Blend {
  pub fn input1(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input1)
  }

  pub fn input2(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input2)
  }

  pub fn mode(&self) -> core::result::Result<BlendMode, console_core_never::Never> {
    Ok(self.mode)
  }
}

#[derive(Clone, Debug)]
pub struct ColorMatrix {
  pub(crate) input: Input,
  pub(crate) kind: ColorMatrixKind,
}

impl ColorMatrix {
  pub fn input(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input)
  }

  pub fn kind(&self) -> core::result::Result<&ColorMatrixKind, console_core_never::Never> {
    Ok(&self.kind)
  }
}

#[derive(Clone, Debug)]
pub enum ColorMatrixKind {
  Matrix(Vec<f32>),
  Saturate(PositiveF32),
  HueRotate(f32),
  LuminanceToAlpha,
}

impl Default for ColorMatrixKind {
  fn default() -> Self {
    ColorMatrixKind::Matrix(vec![
      1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
      1.0, 0.0,
    ])
  }
}

#[derive(Clone, Debug)]
pub struct ComponentTransfer {
  pub(crate) input: Input,
  pub(crate) func_r: TransferFunction,
  pub(crate) func_g: TransferFunction,
  pub(crate) func_b: TransferFunction,
  pub(crate) func_a: TransferFunction,
}

impl ComponentTransfer {
  pub fn input(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input)
  }

  pub fn func_r(&self) -> core::result::Result<&TransferFunction, console_core_never::Never> {
    Ok(&self.func_r)
  }

  pub fn func_g(&self) -> core::result::Result<&TransferFunction, console_core_never::Never> {
    Ok(&self.func_g)
  }

  pub fn func_b(&self) -> core::result::Result<&TransferFunction, console_core_never::Never> {
    Ok(&self.func_b)
  }

  pub fn func_a(&self) -> core::result::Result<&TransferFunction, console_core_never::Never> {
    Ok(&self.func_a)
  }
}

#[derive(Clone, Debug)]
pub enum TransferFunction {
  Identity,

  Table(Vec<f32>),

  Discrete(Vec<f32>),

  Linear {
    slope: f32,
    intercept: f32,
  },

  Gamma {
    amplitude: f32,
    exponent: f32,
    offset: f32,
  },
}

#[derive(Clone, Debug)]
pub struct Composite {
  pub(crate) input1: Input,
  pub(crate) input2: Input,
  pub(crate) operator: CompositeOperator,
}

impl Composite {
  pub fn input1(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input1)
  }

  pub fn input2(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input2)
  }

  pub fn operator(&self) -> core::result::Result<CompositeOperator, console_core_never::Never> {
    Ok(self.operator)
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CompositeOperator {
  Over,
  In,
  Out,
  Atop,
  Xor,
  Arithmetic { k1: f32, k2: f32, k3: f32, k4: f32 },
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The convolution bias is a dimensionless additive offset to normalized channels"
  )
)]
pub struct ConvolveMatrix {
  pub(crate) input: Input,
  pub(crate) matrix: ConvolveMatrixData,
  pub(crate) divisor: NonZeroF32,
  pub(crate) bias: f32,
  pub(crate) edge_mode: EdgeMode,
  pub(crate) preserve_alpha: bool,
}

impl ConvolveMatrix {
  pub fn input(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input)
  }

  pub fn matrix(&self) -> core::result::Result<&ConvolveMatrixData, console_core_never::Never> {
    Ok(&self.matrix)
  }

  pub fn divisor(&self) -> core::result::Result<NonZeroF32, console_core_never::Never> {
    Ok(self.divisor)
  }

  pub fn bias(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.bias)
  }

  pub fn edge_mode(&self) -> core::result::Result<EdgeMode, console_core_never::Never> {
    Ok(self.edge_mode)
  }

  pub fn preserve_alpha(
    &self,
  ) -> core::result::Result<crate::PreserveAlphaAnswer, console_core_never::Never> {
    Ok(match ::core::primitive::bool::then_some(self.preserve_alpha, ()) {
      ::core::option::Option::Some(()) => crate::PreserveAlphaAnswer::Yes,
      ::core::option::Option::None => crate::PreserveAlphaAnswer::No,
    })
  }
}

#[derive(Clone, Debug)]
pub struct ConvolveMatrixData {
  pub(crate) target_x: u32,
  pub(crate) target_y: u32,
  pub(crate) columns: u32,
  pub(crate) rows: u32,
  pub(crate) data: Vec<f32>,
}

impl ConvolveMatrixData {
  pub fn target_x(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.target_x)
  }

  pub fn target_y(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.target_y)
  }

  pub fn columns(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.columns)
  }

  pub fn rows(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.rows)
  }

  pub fn data(&self) -> core::result::Result<&[f32], console_core_never::Never> {
    Ok(&self.data)
  }
}

pub(crate) struct KernelLocation {
  pub(crate) x: u32,
  pub(crate) y: u32,
}

pub(crate) struct KernelDimensions {
  pub(crate) columns: u32,
  pub(crate) rows: u32,
}

impl ConvolveMatrixData {
  pub(crate) fn new(
    target: KernelLocation,
    dimensions: KernelDimensions,
    data: Vec<f32>,
  ) -> Result<Option<Self>, Never> {
    let KernelDimensions { columns, rows } = dimensions;
    let count = match columns.checked_mul(rows) {
      Some(count) => count,
      None => return Ok(None),
    };
    let Ok(count_at_storage) = console_core_number_conversion::wrapped_index(count);

    match ::core::primitive::bool::then_some(count_at_storage == data.len() && target.x < columns && target.y < rows, ()) {
      ::core::option::Option::Some(()) => Ok(Some(Self {
        target_x: target.x,
        target_y: target.y,
        columns,
        rows,
        data,
      })),
      ::core::option::Option::None => Ok(None),
    }
  }

  #[cfg_attr(
    dylint_lib = "explicit024_no_swappable_params",
    expect(
      explicit024_no_swappable_params,
      reason = "The public kernel lookup takes column and row in established x, y order"
    )
  )]
  pub fn get(&self, x: u32, y: u32) -> Result<Option<f32>, Never> {
    match ::core::primitive::bool::then_some(x < self.columns && y < self.rows, ()) {
      ::core::option::Option::Some(()) => {
        let offset = match y.checked_mul(self.columns).and_then(|row| row.checked_add(x)) {
          Some(offset) => offset,
          None => return Ok(None),
        };
        let Ok(index) = console_core_number_conversion::wrapped_index(offset);

        Ok(self.data.get(index).copied())
      }
      ::core::option::Option::None => Ok(None),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{ConvolveMatrixData, KernelDimensions, KernelLocation};

  #[test]
  fn convolution_dimensions_cannot_wrap_to_an_empty_kernel() -> std::io::Result<()> {
    let Ok(matrix) = ConvolveMatrixData::new(
      KernelLocation { x: 32_768, y: 32_768 },
      KernelDimensions { columns: 65_536, rows: 65_536 },
      Vec::new(),
    );

    match matrix {
      None => Ok(()),
      Some(_) => Err(std::io::Error::other("overflowing convolution dimensions were accepted")),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum EdgeMode {
  None,
  Duplicate,
  Wrap,
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The displacement scale is in the resolved filter user coordinate system"
  )
)]
pub struct DisplacementMap {
  pub(crate) input1: Input,
  pub(crate) input2: Input,
  pub(crate) scale: f32,
  pub(crate) x_channel_selector: ColorChannel,
  pub(crate) y_channel_selector: ColorChannel,
}

impl DisplacementMap {
  pub fn input1(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input1)
  }

  pub fn input2(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input2)
  }

  pub fn scale(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.scale)
  }

  pub fn x_channel_selector(
    &self,
  ) -> core::result::Result<ColorChannel, console_core_never::Never> {
    Ok(self.x_channel_selector)
  }

  pub fn y_channel_selector(
    &self,
  ) -> core::result::Result<ColorChannel, console_core_never::Never> {
    Ok(self.y_channel_selector)
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ColorChannel {
  R,
  G,
  B,
  A,
}

#[derive(Clone, Debug)]
pub struct DropShadow {
  pub(crate) input: Input,
  pub(crate) offset: Point,
  pub(crate) std_dev_x: PositiveF32,
  pub(crate) std_dev_y: PositiveF32,
  pub(crate) color: Color,
  pub(crate) opacity: Opacity,
}

impl DropShadow {
  pub fn input(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input)
  }

  pub fn dx(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.offset.x)
  }

  pub fn dy(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.offset.y)
  }

  pub fn std_dev_x(&self) -> core::result::Result<PositiveF32, console_core_never::Never> {
    Ok(self.std_dev_x)
  }

  pub fn std_dev_y(&self) -> core::result::Result<PositiveF32, console_core_never::Never> {
    Ok(self.std_dev_y)
  }

  pub fn color(&self) -> core::result::Result<Color, console_core_never::Never> {
    Ok(self.color)
  }

  pub fn opacity(&self) -> core::result::Result<Opacity, console_core_never::Never> {
    Ok(self.opacity)
  }
}

#[derive(Clone, Copy, Debug)]
pub struct Flood {
  pub(crate) color: Color,
  pub(crate) opacity: Opacity,
}

impl Flood {
  pub fn color(&self) -> core::result::Result<Color, console_core_never::Never> {
    Ok(self.color)
  }

  pub fn opacity(&self) -> core::result::Result<Opacity, console_core_never::Never> {
    Ok(self.opacity)
  }
}

#[derive(Clone, Debug)]
pub struct GaussianBlur {
  pub(crate) input: Input,
  pub(crate) std_dev_x: PositiveF32,
  pub(crate) std_dev_y: PositiveF32,
}

impl GaussianBlur {
  pub fn input(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input)
  }

  pub fn std_dev_x(&self) -> core::result::Result<PositiveF32, console_core_never::Never> {
    Ok(self.std_dev_x)
  }

  pub fn std_dev_y(&self) -> core::result::Result<PositiveF32, console_core_never::Never> {
    Ok(self.std_dev_y)
  }
}

#[derive(Clone, Debug)]
pub struct Image {
  pub(crate) root: Group,
}

impl Image {
  pub fn root(&self) -> core::result::Result<&Group, console_core_never::Never> {
    Ok(&self.root)
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "Surface scale converts normalized alpha into filter user units and the diffuse coefficient is dimensionless"
  )
)]
pub struct DiffuseLighting {
  pub(crate) input: Input,
  pub(crate) surface_scale: f32,
  pub(crate) diffuse_constant: f32,
  pub(crate) lighting_color: Color,
  pub(crate) light_source: LightSource,
}

impl DiffuseLighting {
  pub fn input(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input)
  }

  pub fn surface_scale(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.surface_scale)
  }

  pub fn diffuse_constant(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.diffuse_constant)
  }

  pub fn lighting_color(&self) -> core::result::Result<Color, console_core_never::Never> {
    Ok(self.lighting_color)
  }

  pub fn light_source(&self) -> core::result::Result<LightSource, console_core_never::Never> {
    Ok(self.light_source)
  }
}

#[derive(Clone, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "Surface scale converts alpha into filter user units and both specular coefficients are dimensionless"
  )
)]
pub struct SpecularLighting {
  pub(crate) input: Input,
  pub(crate) surface_scale: f32,
  pub(crate) specular_constant: f32,
  pub(crate) specular_exponent: f32,
  pub(crate) lighting_color: Color,
  pub(crate) light_source: LightSource,
}

impl SpecularLighting {
  pub fn input(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input)
  }

  pub fn surface_scale(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.surface_scale)
  }

  pub fn specular_constant(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.specular_constant)
  }

  pub fn specular_exponent(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.specular_exponent)
  }

  pub fn lighting_color(&self) -> core::result::Result<Color, console_core_never::Never> {
    Ok(self.lighting_color)
  }

  pub fn light_source(&self) -> core::result::Result<LightSource, console_core_never::Never> {
    Ok(self.light_source)
  }
}

#[derive(Clone, Copy, Debug)]
pub enum LightSource {
  DistantLight(DistantLight),
  PointLight(PointLight),
  SpotLight(SpotLight),
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The public azimuth and elevation are angles in SVG degrees"
  )
)]
pub struct DistantLight {
  pub azimuth: f32,

  pub elevation: f32,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The public x, y and z light coordinates are in resolved filter user units"
  )
)]
pub struct PointLight {
  pub x: f32,

  pub y: f32,

  pub z: f32,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The public source and target coordinates are in resolved filter user units"
  )
)]
pub struct SpotLight {
  pub x: f32,

  pub y: f32,

  pub z: f32,

  pub points_at_x: f32,

  pub points_at_y: f32,

  pub points_at_z: f32,

  pub specular_exponent: PositiveF32,

  pub limiting_cone_angle: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct Merge {
  pub(crate) inputs: Vec<Input>,
}

impl Merge {
  pub fn inputs(&self) -> core::result::Result<&[Input], console_core_never::Never> {
    Ok(&self.inputs)
  }
}

#[derive(Clone, Debug)]
pub struct Morphology {
  pub(crate) input: Input,
  pub(crate) operator: MorphologyOperator,
  pub(crate) radius_x: PositiveF32,
  pub(crate) radius_y: PositiveF32,
}

impl Morphology {
  pub fn input(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input)
  }

  pub fn operator(&self) -> core::result::Result<MorphologyOperator, console_core_never::Never> {
    Ok(self.operator)
  }

  pub fn radius_x(&self) -> core::result::Result<PositiveF32, console_core_never::Never> {
    Ok(self.radius_x)
  }

  pub fn radius_y(&self) -> core::result::Result<PositiveF32, console_core_never::Never> {
    Ok(self.radius_y)
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MorphologyOperator {
  Erode,
  Dilate,
}

#[derive(Clone, Debug)]
pub struct Offset {
  pub(crate) input: Input,
  pub(crate) offset: Point,
}

impl Offset {
  pub fn input(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input)
  }

  pub fn dx(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.offset.x)
  }

  pub fn dy(&self) -> core::result::Result<f32, console_core_never::Never> {
    Ok(self.offset.y)
  }
}

#[derive(Clone, Debug)]
pub struct Tile {
  pub(crate) input: Input,
}

impl Tile {
  pub fn input(&self) -> core::result::Result<&Input, console_core_never::Never> {
    Ok(&self.input)
  }
}

#[derive(Clone, Copy, Debug)]
pub struct Turbulence {
  pub(crate) base_frequency_x: PositiveF32,
  pub(crate) base_frequency_y: PositiveF32,
  pub(crate) num_octaves: u32,
  pub(crate) seed: i32,
  pub(crate) stitch_tiles: bool,
  pub(crate) kind: TurbulenceKind,
}

impl Turbulence {
  pub fn base_frequency_x(&self) -> core::result::Result<PositiveF32, console_core_never::Never> {
    Ok(self.base_frequency_x)
  }

  pub fn base_frequency_y(&self) -> core::result::Result<PositiveF32, console_core_never::Never> {
    Ok(self.base_frequency_y)
  }

  pub fn num_octaves(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.num_octaves)
  }

  pub fn seed(&self) -> core::result::Result<i32, console_core_never::Never> {
    Ok(self.seed)
  }

  pub fn stitch_tiles(
    &self,
  ) -> core::result::Result<crate::StitchTilesAnswer, console_core_never::Never> {
    Ok(match ::core::primitive::bool::then_some(self.stitch_tiles, ()) {
      ::core::option::Option::Some(()) => crate::StitchTilesAnswer::Yes,
      ::core::option::Option::None => crate::StitchTilesAnswer::No,
    })
  }

  pub fn kind(&self) -> core::result::Result<TurbulenceKind, console_core_never::Never> {
    Ok(self.kind)
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TurbulenceKind {
  FractalNoise,
  Turbulence,
}
