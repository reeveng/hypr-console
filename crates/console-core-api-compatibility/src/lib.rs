//! Native widths and positional constructors retained for existing callers.
//! Rust requires inherent methods to be defined in the crate that owns their type.
//! The macro supplies the reviewed constructors in the crate that owns the types.
//! [`paths_geometry_constructors!`] retains positional constructors and
//! transform methods used by path and rasterizer clients. [`paths_builder_coordinates!`] keeps the
//! positional path drawing methods available to existing Rust callers.

#![no_std]

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  allow(
    explicit051_no_machine_width,
    reason = "an exported signature that already accepts the platform's unsigned integer must keep its type and ABI for existing callers"
  )
)]
pub type NativeSize = usize;

#[cfg_attr(
  dylint_lib = "explicit051_no_machine_width",
  allow(
    explicit051_no_machine_width,
    reason = "an exported signature that already accepts the platform's signed integer must keep its type and ABI for existing callers"
  )
)]
pub type NativeOffset = isize;

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  allow(
    explicit080_macro_budget,
    reason = "one macro installs the reviewed positional constructors and methods in the crate that owns Point and Transform"
  )
)]
#[macro_export]
macro_rules! paths_geometry_constructors {
  () => {
    impl Point {
      #[doc = r#"Constructs a point from its horizontal and vertical coordinates.

```
use console_core_never::Never;
use console_core_paths::Point;

let build: fn(f32, f32) -> Result<Point, Never> = Point::from_xy;
let Ok(point) = build(-0.0, 7.5);
assert_eq!(point.x.to_bits(), (-0.0_f32).to_bits());
assert_eq!(point.y.to_bits(), 7.5_f32.to_bits());
```"#]
      #[cfg_attr(dylint_lib = "explicit024_no_swappable_params", allow(
        explicit024_no_swappable_params,
        reason = "existing Point::from_xy callers pass horizontal and vertical coordinates in that order"
      ))]
      pub fn from_xy(x: f32, y: f32) -> ::core::result::Result<Self, ::console_core_never::Never> {
        ::core::result::Result::Ok(Self { x, y })
      }
    }
    impl Transform {
      #[doc = r#"Constructs the affine matrix from its six coefficients in path API order.

```
use console_core_never::Never;
use console_core_paths::Transform;

let build: fn(f32, f32, f32, f32, f32, f32) -> Result<Transform, Never> = Transform::from_row;
let Ok(matrix) = build(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
assert_eq!([matrix.sx, matrix.ky, matrix.kx, matrix.sy, matrix.tx, matrix.ty],
           [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
```"#]
      #[cfg_attr(dylint_lib = "explicit024_no_swappable_params", allow(
        explicit024_no_swappable_params,
        reason = "existing SVG and rasterizer callers supply the six path matrix coefficients in sx, ky, kx, sy, tx, ty order"
      ))]
      pub fn from_row(
        sx: f32,
        ky: f32,
        kx: f32,
        sy: f32,
        tx: f32,
        ty: f32,
      ) -> ::core::result::Result<Self, ::console_core_never::Never> {
        ::core::result::Result::Ok(Self { sx, ky, kx, sy, tx, ty })
      }

      #[doc = r#"Constructs an identity transform displaced by the given offsets.

```
use console_core_never::Never;
use console_core_paths::Transform;

let build: fn(f32, f32) -> Result<Transform, Never> = Transform::from_translate;
let Ok(offset) = build(-0.0, 5.5);
assert_eq!(offset.tx.to_bits(), (-0.0_f32).to_bits());
assert_eq!(offset.ty.to_bits(), 5.5_f32.to_bits());
assert_eq!([offset.sx, offset.ky, offset.kx, offset.sy], [1.0, 0.0, 0.0, 1.0]);
```"#]
      #[cfg_attr(dylint_lib = "explicit024_no_swappable_params", allow(
        explicit024_no_swappable_params,
        reason = "existing text, drawing and rasterizer callers pass horizontal then vertical translation"
      ))]
      pub fn from_translate(tx: f32, ty: f32) -> ::core::result::Result<Self, ::console_core_never::Never> {
        Self::from_translation(Point { x: tx, y: ty })
      }

      #[doc = r#"Constructs a transform with independent horizontal and vertical scale factors.

```
use console_core_never::Never;
use console_core_paths::Transform;

let build: fn(f32, f32) -> Result<Transform, Never> = Transform::from_scale;
let Ok(scale) = build(3.0, -0.0);
assert_eq!(scale.sx.to_bits(), 3.0_f32.to_bits());
assert_eq!(scale.sy.to_bits(), (-0.0_f32).to_bits());
assert_eq!([scale.kx, scale.ky, scale.tx, scale.ty].map(f32::to_bits), [0; 4]);
```"#]
      #[cfg_attr(dylint_lib = "explicit024_no_swappable_params", allow(
        explicit024_no_swappable_params,
        reason = "existing Transform::from_scale callers pass horizontal and vertical scale factors in that order"
      ))]
      pub fn from_scale(sx: f32, sy: f32) -> ::core::result::Result<Self, ::console_core_never::Never> {
        Self::from_scale_factors(crate::ScaleFactors { x: sx, y: sy })
      }
    }
    #[cfg_attr(dylint_lib = "explicit024_no_swappable_params", allow(
      explicit024_no_swappable_params,
      reason = "published transform methods take horizontal then vertical values and forward them to the named fields of their typed counterparts"
    ))]
    #[cfg_attr(dylint_lib = "explicit067_no_unit_less_quantity", allow(
      explicit067_no_unit_less_quantity,
      reason = "the compatibility methods retain scalar coordinates and ratios accepted by existing callers"
    ))]
    impl Transform {
      #[must_use]
      pub fn pre_translate(&self, tx: f32, ty: f32) -> ::core::result::Result<Self, ::console_core_never::Never> {
        self.pre_translate_by(Point { x: tx, y: ty })
      }

      #[must_use]
      pub fn post_translate(&self, tx: f32, ty: f32) -> ::core::result::Result<Self, ::console_core_never::Never> {
        self.post_translate_by(Point { x: tx, y: ty })
      }

      #[must_use]
      pub fn pre_scale(&self, sx: f32, sy: f32) -> ::core::result::Result<Self, ::console_core_never::Never> {
        self.pre_scale_by(crate::ScaleFactors { x: sx, y: sy })
      }

      #[must_use]
      pub fn post_scale(&self, sx: f32, sy: f32) -> ::core::result::Result<Self, ::console_core_never::Never> {
        self.post_scale_by(crate::ScaleFactors { x: sx, y: sy })
      }
    }
  };
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  allow(
    explicit080_macro_budget,
    reason = "these four existing drawing entry points forward coordinates to the typed PathBuilder methods in their owning crate"
  )
)]
#[macro_export]
macro_rules! paths_builder_coordinates {
  () => {
    #[cfg_attr(dylint_lib = "explicit024_no_swappable_params", allow(
      explicit024_no_swappable_params,
      reason = "published drawing methods accept x then y for each control point and endpoint"
    ))]
    #[cfg_attr(dylint_lib = "explicit067_no_unit_less_quantity", allow(
      explicit067_no_unit_less_quantity,
      reason = "existing drawing callers pass scalar path coordinates and each pair immediately becomes a Point"
    ))]
    impl PathBuilder {
      pub fn move_to(&mut self, x: f32, y: f32) -> ::core::result::Result<(), ::console_core_never::Never> {
        self.move_to_pt(Point { x, y })
      }
      pub fn line_to(&mut self, x: f32, y: f32) -> ::core::result::Result<(), ::console_core_never::Never> {
        self.line_to_pt(Point { x, y })
      }
      pub fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) -> ::core::result::Result<(), ::console_core_never::Never> {
        self.quad_to_pt(Point { x: x1, y: y1 }, Point { x, y })
      }
      pub fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) -> ::core::result::Result<(), ::console_core_never::Never> {
        self.cubic_to_pt(Point { x: x1, y: y1 }, Point { x: x2, y: y2 }, Point { x, y })
      }
    }
  };
}
