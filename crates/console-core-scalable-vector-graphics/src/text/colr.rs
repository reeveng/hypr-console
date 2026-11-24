use console_core_scalable_vector_graphics_values::Color;
use skrifa::instance::LocationRef;
use skrifa::prelude::Size;
use skrifa::raw::types::Point;
use skrifa::{
  MetadataProvider,
  color::{Brush, ColorStop, Extend, Transform},
  outline::DrawSettings,
};
use std::fmt::Write as _;

use super::transform::{skrifa_to_tsp_transform, tsp_to_skrifa_transform};

struct Builder<'a> {
  path: &'a mut String,
  min: Point<f32>,
  max: Point<f32>,
}

impl<'a> Builder<'a> {
  fn new(path: &'a mut String) -> Result<Self, Never> {
    Ok(Self {
      path,
      min: Point {
        x: f32::MAX,
        y: f32::MAX,
      },
      max: Point {
        x: f32::MIN,
        y: f32::MIN,
      },
    })
  }

  fn add_point(&mut self, point: Point<f32>) -> Result<(), Never> {
    self.min.x = self.min.x.min(point.x);
    self.min.y = self.min.y.min(point.y);
    self.max.x = self.max.x.max(point.x);
    self.max.y = self.max.y.max(point.y);

    Ok(())
  }

  fn bounds(&self) -> Result<Option<console_core_paths::FloatRectangle>, Never> {
    match ::core::primitive::bool::then_some(
      self.min.x <= self.max.x && self.min.y <= self.max.y,
      (),
    ) {
      ::core::option::Option::Some(()) => console_core_paths::FloatRectangle::from_ltrb(
        self.min.x, self.min.y, self.max.x, self.max.y,
      ),
      ::core::option::Option::None => Ok(None),
    }
  }

  fn finish(&mut self) -> Result<(), Never> {
    self.path.pop();

    Ok(())
  }
}

fn append_path_command(
  buffer: &mut String,
  command: std::fmt::Arguments<'_>,
) -> Result<(), Never> {
  match buffer.write_fmt(command) {
    Ok(()) => {}
    Err(problem) => {
      console_core_logging::warn!("Could not format COLR path coordinates: {problem}")
    }
  }

  Ok(())
}

impl skrifa::outline::OutlinePen for Builder<'_> {
  fn move_to(&mut self, x: f32, y: f32) {
    let Ok(()) = self.add_point(Point { x, y });
    let Ok(()) = append_path_command(self.path, format_args!("M {x} {y} "));
  }

  fn line_to(&mut self, x: f32, y: f32) {
    let Ok(()) = self.add_point(Point { x, y });
    let Ok(()) = append_path_command(self.path, format_args!("L {x} {y} "));
  }

  fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
    let Ok(()) = self.add_point(Point { x: cx0, y: cy0 });
    let Ok(()) = self.add_point(Point { x, y });
    let Ok(()) = append_path_command(self.path, format_args!("Q {cx0} {cy0} {x} {y} "));
  }

  fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
    let Ok(()) = self.add_point(Point { x: cx0, y: cy0 });
    let Ok(()) = self.add_point(Point { x: cx1, y: cy1 });
    let Ok(()) = self.add_point(Point { x, y });
    let Ok(()) = append_path_command(
      self.path,
      format_args!("C {cx0} {cy0} {cx1} {cy1} {x} {y} "),
    );
  }

  fn close(&mut self) {
    self.path.push_str("Z ");
  }
}

use console_core_extensible_markup_writer::Error as XmlFault;
use console_core_never::Never;

trait XmlWriterExt {
  fn write_color_attribute(&mut self, name: &str, ts: Color) -> Result<(), XmlFault>;

  fn write_transform_attribute(&mut self, name: &str, ts: Transform) -> Result<(), XmlFault>;

  fn write_spread_method_attribute(&mut self, method: Extend) -> Result<(), XmlFault>;
}

impl XmlWriterExt for console_core_extensible_markup_writer::XmlWriter {
  fn write_color_attribute(&mut self, name: &str, color: Color) -> Result<(), XmlFault> {
    self.write_attribute_fmt(
      name,
      format_args!("rgb({}, {}, {})", color.red, color.green, color.blue),
    )
  }

  fn write_transform_attribute(&mut self, name: &str, ts: Transform) -> Result<(), XmlFault> {
    match ::core::primitive::bool::then_some(ts == Transform::default(), ()) {
      ::core::option::Option::Some(()) => {
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    self.write_attribute_fmt(
      name,
      format_args!(
        "matrix({} {} {} {} {} {})",
        ts.xx, ts.yx, ts.xy, ts.yy, ts.dx, ts.dy
      ),
    )
  }

  fn write_spread_method_attribute(&mut self, extend: Extend) -> Result<(), XmlFault> {
    self.write_attribute(
      "spreadMethod",
      match extend {
        Extend::Pad => "pad",
        Extend::Repeat => "repeat",
        Extend::Reflect => "reflect",
        Extend::Unknown => return Ok(()),
      },
    )
  }
}

pub(crate) struct GlyphPainter<'a> {
  pub(crate) font: &'a skrifa::FontRef<'a>,

  pub(crate) location: LocationRef<'a>,
  pub(crate) svg: &'a mut console_core_extensible_markup_writer::XmlWriter,
  pub(crate) write_fault: Option<XmlFault>,
  pub(crate) path_buf: &'a mut String,
  pub(crate) gradient_index: u64,
  pub(crate) clip_path_index: u64,
  pub(crate) foreground_color: Color,
  pub(crate) transform: Transform,
  pub(crate) outline_transform: Transform,
  pub(crate) transforms_stack: Vec<Transform>,

  pub(crate) clip_stack: Vec<Option<console_core_paths::FloatRectangle>>,
}

impl<'a> GlyphPainter<'a> {
  fn record<E>(&mut self, result: Result<(), E>) -> Result<(), Never>
  where
    XmlFault: From<E>,
  {
    match result {
      Ok(()) => {}
      Err(fault) => {
        let _first = self.write_fault.get_or_insert(XmlFault::from(fault));
      }
    }

    Ok(())
  }

  fn write_gradient_stops(&mut self, stops: &[ColorStop]) -> Result<(), XmlFault> {
    for stop in stops {
      let Ok(color) = self.palette_index_to_color(stop.palette_index, PaintAlpha(stop.alpha));
      let opacity = f32::from(color.alpha) / 255.0;

      self.svg.start_element("stop")?;
      self.svg.write_attribute("offset", &stop.offset)?;
      self.svg.write_color_attribute("stop-color", color)?;
      self.svg.write_attribute("stop-opacity", &opacity)?;
      self.svg.end_element()?;
    }

    Ok(())
  }

  fn paint_solid(&mut self, color: Color) -> Result<(), XmlFault> {
    let opacity = f32::from(color.alpha) / 255.0;

    self.svg.start_element("path")?;
    self.svg.write_color_attribute("fill", color)?;
    self.svg.write_attribute("fill-opacity", &opacity)?;
    self
      .svg
      .write_transform_attribute("transform", self.outline_transform)?;
    self.svg.write_attribute("d", self.path_buf)?;
    self.svg.end_element()
  }

  fn paint_gradient(
    &mut self,
    geometry: GradientGeometry,
    stops: &[ColorStop],
    extend: Extend,
  ) -> Result<(), XmlFault> {
    let id = match geometry {
      GradientGeometry::Linear { .. } => format!("lg{}", self.gradient_index),
      GradientGeometry::Radial { .. } => format!("rg{}", self.gradient_index),
    };
    self.gradient_index = self.gradient_index.wrapping_add(1);

    match geometry {
      GradientGeometry::Linear { start, end } => {
        self.svg.start_element("linearGradient")?;
        self.svg.write_attribute("id", &id)?;

        for (name, value) in [
          ("x1", start.x),
          ("y1", start.y),
          ("x2", end.x),
          ("y2", end.y),
        ] {
          self.svg.write_attribute(name, &value)?;
        }
      }
      GradientGeometry::Radial { start, end } => {
        self.svg.start_element("radialGradient")?;
        self.svg.write_attribute("id", &id)?;

        for (name, value) in [
          ("cx", end.center.x),
          ("cy", end.center.y),
          ("r", end.radius),
          ("fr", start.radius),
          ("fx", start.center.x),
          ("fy", start.center.y),
        ] {
          self.svg.write_attribute(name, &value)?;
        }
      }
    }

    let Ok(gradient_transform) = paint_transform(self.outline_transform, self.transform);
    self
      .svg
      .write_attribute("gradientUnits", &"userSpaceOnUse")?;
    self.svg.write_spread_method_attribute(extend)?;
    self
      .svg
      .write_transform_attribute("gradientTransform", gradient_transform)?;
    self.write_gradient_stops(stops)?;
    self.svg.end_element()?;
    self.svg.start_element("path")?;
    self
      .svg
      .write_attribute_fmt("fill", format_args!("url(#{id})"))?;
    self
      .svg
      .write_transform_attribute("transform", self.outline_transform)?;
    self.svg.write_attribute("d", self.path_buf)?;
    self.svg.end_element()
  }
}

enum GradientGeometry {
  Linear {
    start: Point<f32>,
    end: Point<f32>,
  },
  Radial {
    start: GradientCircle,
    end: GradientCircle,
  },
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The gradient circle radius is in the font outline's coordinate units"
  )
)]
struct GradientCircle {
  center: Point<f32>,
  radius: f32,
}

#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The COLR alpha multiplier is a unitless factor from the color paint"
  )
)]
struct PaintAlpha(f32);

fn paint_transform(
  outline_transform: Transform,
  transform: Transform,
) -> ::core::result::Result<Transform, ::console_core_never::Never> {
  let Ok(outline_transform_2) = skrifa_to_tsp_transform(outline_transform);
  let Ok(gradient_transform) = skrifa_to_tsp_transform(transform);

  let Ok(inverse) = outline_transform_2.invert();
  let outline_inverse = match inverse {
    Some(inverse_2) => inverse_2,
    None => {
      console_core_logging::warn!("Could not invert the COLR outline transform for its gradient");

      console_core_paths::Transform::default()
    }
  };
  let Ok(concatenated) = outline_inverse.pre_concat(gradient_transform);

  tsp_to_skrifa_transform(concatenated)
}

fn map_rect(
  rect: console_core_paths::FloatRectangle,
  transform: console_core_paths::Transform,
) -> Result<Option<console_core_paths::FloatRectangle>, Never> {
  let Ok(left) = rect.left();
  let Ok(top) = rect.top();
  let Ok(right) = rect.right();
  let Ok(bottom) = rect.bottom();
  let mut points = [
    console_core_paths::Point { x: left, y: top },
    console_core_paths::Point { x: right, y: top },
    console_core_paths::Point { x: left, y: bottom },
    console_core_paths::Point {
      x: right,
      y: bottom,
    },
  ];
  let Ok(()) = transform.map_points(&mut points);
  let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];

  for point in points {
    let [min_x, min_y, max_x, max_y] = bounds;
    bounds = [
      min_x.min(point.x),
      min_y.min(point.y),
      max_x.max(point.x),
      max_y.max(point.y),
    ];
  }

  let [min_x, min_y, max_x, max_y] = bounds;

  console_core_paths::FloatRectangle::from_ltrb(min_x, min_y, max_x, max_y)
}

fn intersect_rects(
  a: console_core_paths::FloatRectangle,
  b: console_core_paths::FloatRectangle,
) -> ::core::result::Result<Option<console_core_paths::FloatRectangle>, ::console_core_never::Never>
{
  let Ok(left) = a.left();
  let Ok(b_left) = b.left();
  let Ok(rect) = console_core_paths::FloatRectangle::from_ltrb(
    left.max(b_left),
    ({
      let Ok(explicit_value) = a.top();
      explicit_value
    })
    .max({
      let Ok(explicit_value) = b.top();
      explicit_value
    }),
    ({
      let Ok(explicit_value) = a.right();
      explicit_value
    })
    .min({
      let Ok(explicit_value) = b.right();
      explicit_value
    }),
    ({
      let Ok(explicit_value) = a.bottom();
      explicit_value
    })
    .min({
      let Ok(explicit_value) = b.bottom();
      explicit_value
    }),
  );

  ::core::result::Result::Ok(rect)
}

impl GlyphPainter<'_> {
  fn clip_with_path(&mut self, path: &str) -> Result<(), XmlFault> {
    let id = format!("cp{}", self.clip_path_index);
    self.clip_path_index = self.clip_path_index.wrapping_add(1);

    self.svg.start_element("clipPath")?;
    self.svg.write_attribute("id", &id)?;
    self.svg.start_element("path")?;
    self
      .svg
      .write_transform_attribute("transform", self.outline_transform)?;
    self.svg.write_attribute("d", &path)?;
    self.svg.end_element()?;
    self.svg.end_element()?;
    self.svg.start_element("g")?;
    self
      .svg
      .write_attribute_fmt("clip-path", format_args!("url(#{id})"))?;

    Ok(())
  }

  fn outline_glyph(
    &mut self,
    glyph_id: skrifa::GlyphId,
  ) -> Result<Option<console_core_paths::FloatRectangle>, Never> {
    self.path_buf.clear();
    self.outline_transform = self.transform;
    let glyphs = self.font.outline_glyphs();
    let outliner = match glyphs.get(glyph_id) {
      Some(outliner) => outliner,
      None => return Ok(None),
    };
    let Ok(mut builder) = Builder::new(self.path_buf);
    let settings = DrawSettings::unhinted(Size::unscaled(), self.location);

    match outliner.draw(settings, &mut builder) {
      Ok(_) => {
        let Ok(()) = builder.finish();
        builder.bounds()
      }
      Err(problem) => {
        console_core_logging::debug!("Could not draw COLR outline {glyph_id:?}: {problem}");
        self.path_buf.clear();

        Ok(None)
      }
    }
  }

  fn paint_brush(&mut self, brush: Brush<'_>) -> Result<(), Never> {
    let result = match brush {
      Brush::Solid {
        palette_index,
        alpha,
      } => {
        let Ok(color) = self.palette_index_to_color(palette_index, PaintAlpha(alpha));
        self.paint_solid(color)
      }
      Brush::LinearGradient {
        p0,
        p1,
        color_stops,
        extend,
      } => self.paint_gradient(
        GradientGeometry::Linear { start: p0, end: p1 },
        color_stops,
        extend,
      ),
      Brush::RadialGradient {
        c0,
        r0,
        c1,
        r1,
        color_stops,
        extend,
      } => self.paint_gradient(
        GradientGeometry::Radial {
          start: GradientCircle {
            center: c0,
            radius: r0,
          },
          end: GradientCircle {
            center: c1,
            radius: r1,
          },
        },
        color_stops,
        extend,
      ),
      Brush::SweepGradient { .. } => Ok(()),
    };

    self.record(result)
  }

  fn root_bounds(
    &self,
    bounds: Option<console_core_paths::FloatRectangle>,
  ) -> Result<Option<console_core_paths::FloatRectangle>, Never> {
    match bounds {
      Some(rect) => {
        let Ok(transform) = skrifa_to_tsp_transform(self.outline_transform);

        map_rect(rect, transform)
      }
      None => Ok(None),
    }
  }

  fn palette_index_to_color(
    &self,
    palette_index: u16,
    alpha: PaintAlpha,
  ) -> Result<Color, Never> {
    const FOREGROUND_INDEX: u16 = 0xffff;
    let Ok(index) = console_core_number_conversion::index(palette_index);
    let palettes = self.font.color_palettes();
    let mut color = match palette_index {
      FOREGROUND_INDEX => self.foreground_color,
      _ => match palettes.get(0).and_then(|palette| {
        palette.colors().get(index).map(|color| Color {
          red: color.red,
          blue: color.blue,
          green: color.green,
          alpha: color.alpha,
        })
      }) {
        Some(color) => color,
        None => self.foreground_color,
      },
    };
    let Ok(alpha_byte) =
      console_core_number_conversion::toward_zero_u8(f64::from(f32::from(color.alpha) * alpha.0));
    color.alpha = alpha_byte;

    Ok(color)
  }
}

impl<'a> skrifa::color::ColorPainter for GlyphPainter<'a> {
  fn push_transform(&mut self, transform: Transform) {
    self.transforms_stack.push(self.transform);
    self.transform = self.transform * transform;
  }

  fn pop_transform(&mut self) {
    match self.transforms_stack.pop() {
      Some(ts) => {
        self.transform = ts;
      }
      None => {}
    }
  }

  fn fill_glyph(
    &mut self,
    glyph_id: skrifa::GlyphId,
    brush_transform: Option<Transform>,
    brush: Brush<'_>,
  ) {
    let Ok(_bounds) = self.outline_glyph(glyph_id);

    match brush_transform {
      Some(brush_transform_2) => {
        self.push_transform(brush_transform_2);
        let Ok(()) = self.paint_brush(brush);
        self.pop_transform();
      }
      None => {
        let Ok(()) = self.paint_brush(brush);
      }
    }
  }

  fn push_clip_glyph(&mut self, glyph_id: skrifa::GlyphId) {
    let Ok(bounds) = self.outline_glyph(glyph_id);

    let path = self.path_buf.clone();
    let clip = self.clip_with_path(&path);
    let Ok(()) = self.record(clip);
    let Ok(root_bounds) = self.root_bounds(bounds);
    self.clip_stack.push(root_bounds);
  }

  fn push_clip_box(&mut self, clip_box: skrifa::raw::types::BoundingBox<f32>) {
    let x_min = clip_box.x_min;
    let x_max = clip_box.x_max;
    let y_min = clip_box.y_min;
    let y_max = clip_box.y_max;

    let clip_path = format!(
      "M {} {} L {} {} L {} {} L {} {} Z",
      x_min, y_min, x_max, y_min, x_max, y_max, x_min, y_max
    );

    self.outline_transform = self.transform;
    let clip = self.clip_with_path(&clip_path);
    let Ok(()) = self.record(clip);
    let Ok(rect) = console_core_paths::FloatRectangle::from_ltrb(x_min, y_min, x_max, y_max);
    let Ok(bounds) = self.root_bounds(rect);
    self.clip_stack.push(bounds);
  }

  fn pop_clip(&mut self) {
    {
      let outcome = self.svg.end_element();
      let Ok(()) = self.record(outcome);
    };

    self.clip_stack.pop();
  }

  fn fill(&mut self, brush: Brush<'_>) {
    match ::core::primitive::bool::then_some(self.clip_stack.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        console_core_logging::warn!("Unclipped COLR fills are not supported.");
        return;
      }
      ::core::option::Option::None => {}
    }

    let mut region: Option<console_core_paths::FloatRectangle> = None;

    for bounds in &self.clip_stack {
      let bounds_3 = match bounds {
        Some(bounds_2) => bounds_2,
        None => return,
      };
      region = Some(match region {
        Some(region_2) => match intersect_rects(region_2, *bounds_3) {
          ::core::result::Result::Ok(Some(r)) => r,

          ::core::result::Result::Ok(None) => return,
        },
        None => *bounds_3,
      });
    }

    let region_4 = match region {
      Some(region_3) => region_3,
      None => return,
    };

    self.path_buf.clear();
    let Ok(left) = region_4.left();
    let Ok(top) = region_4.top();
    let Ok(right) = region_4.right();
    let Ok(region_top) = region_4.top();
    let Ok(region_right) = region_4.right();
    let Ok(bottom) = region_4.bottom();
    let Ok(region_left) = region_4.left();
    let Ok(region_bottom) = region_4.bottom();
    let Ok(()) = append_path_command(
      self.path_buf,
      format_args!(
        "M {left} {top} L {right} {region_top} L {region_right} {bottom} L {region_left} {region_bottom} Z"
      ),
    );

    self.outline_transform = Transform::default();
    let Ok(()) = self.paint_brush(brush);
  }

  fn push_layer(&mut self, composite_mode: skrifa::color::CompositeMode) {
    use skrifa::color::CompositeMode;

    let composite_mode_2 = match composite_mode {
      CompositeMode::SrcOver => "normal",
      CompositeMode::Screen => "screen",
      CompositeMode::Overlay => "overlay",
      CompositeMode::Darken => "darken",
      CompositeMode::Lighten => "lighten",
      CompositeMode::ColorDodge => "color-dodge",
      CompositeMode::ColorBurn => "color-burn",
      CompositeMode::HardLight => "hard-light",
      CompositeMode::SoftLight => "soft-light",
      CompositeMode::Difference => "difference",
      CompositeMode::Exclusion => "exclusion",
      CompositeMode::Multiply => "multiply",
      CompositeMode::HslHue => "hue",
      CompositeMode::HslSaturation => "saturation",
      CompositeMode::HslColor => "color",
      CompositeMode::HslLuminosity => "luminosity",
      CompositeMode::Clear
      | CompositeMode::Src
      | CompositeMode::Dest
      | CompositeMode::DestOver
      | CompositeMode::SrcIn
      | CompositeMode::DestIn
      | CompositeMode::SrcOut
      | CompositeMode::DestOut
      | CompositeMode::SrcAtop
      | CompositeMode::DestAtop
      | CompositeMode::Xor
      | CompositeMode::Plus
      | CompositeMode::Unknown => "normal",
    };

    {
      let outcome = self.svg.start_element("g");
      let Ok(()) = self.record(outcome);
    };

    {
      let outcome = self.svg.write_attribute_fmt(
        "style",
        format_args!("mix-blend-mode: {}; isolation: isolate", composite_mode_2),
      );
      let Ok(()) = self.record(outcome);
    };
  }

  fn pop_layer(&mut self) {
    {
      let outcome = self.svg.end_element();
      let Ok(()) = self.record(outcome);
    };
  }
}
