use console_core_never::Never;
use console_core_paths::{Path, PathBuilder, Point};
use console_core_scalable_vector_graphics_values::Length;

use super::svgtree::{AId, EId, SvgNode};
use super::{converter, units};
use crate::{ApproxEqUlps, IsValidLength, FloatRectangle};

pub(crate) fn convert(node: SvgNode, state: &converter::State) -> Result<Option<Path>, Never> {
  let Ok(tag) = node.tag_name();

  match tag {
    Some(EId::Rectangle) => convert_rect(node, state),
    Some(EId::Circle) => convert_circle(node, state),
    Some(EId::Ellipse) => convert_ellipse(node, state),
    Some(EId::Line) => convert_line(node, state),
    Some(EId::Polyline) => convert_polyline(node),
    Some(EId::Polygon) => convert_polygon(node),
    Some(EId::Path) => convert_path(node),
    Some(_) | None => Ok(None),
  }
}

pub(crate) fn convert_path(
  node: SvgNode,
) -> core::result::Result<Option<Path>, console_core_never::Never> {
  let Ok(attribute) = node.attribute(AId::D);
  let value: &str = match attribute {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(mut builder) = console_core_paths::PathBuilder::new();

  use console_core_scalable_vector_graphics_values::{SimplePathSegment, SimplifyingPathParser};

  for command in SimplifyingPathParser::from(value) {
    let segment = match command {
      Ok(segment) => segment,
      Err(problem) => {
        console_core_logging::warn!("Error during path parsing: {problem}");
        break;
      }
    };

    match segment {
      SimplePathSegment::MoveTo { x, y } => {
        let Ok(point) = path_point(kurbo::Point::new(x, y));
        let Ok(()) = builder.move_to_pt(point);
      }
      SimplePathSegment::LineTo { x, y } => {
        let Ok(point) = path_point(kurbo::Point::new(x, y));
        let Ok(()) = builder.line_to_pt(point);
      }
      SimplePathSegment::Quadratic { x1, y1, x, y } => {
        let Ok(control) = path_point(kurbo::Point::new(x1, y1));
        let Ok(end) = path_point(kurbo::Point::new(x, y));
        let Ok(()) = builder.quad_to_pt(control, end);
      }
      SimplePathSegment::CurveTo {
        x1,
        y1,
        x2,
        y2,
        x,
        y,
      } => {
        let Ok(first) = path_point(kurbo::Point::new(x1, y1));
        let Ok(second) = path_point(kurbo::Point::new(x2, y2));
        let Ok(end) = path_point(kurbo::Point::new(x, y));
        let Ok(()) = builder.cubic_to_pt(first, second, end);
      }
      SimplePathSegment::ClosePath => {
        let Ok(()) = builder.close();
      }
    }
  }

  builder.finish()
}

fn path_point(point: kurbo::Point) -> Result<Point, Never> {
  let Ok(x) = console_core_number_conversion::nearest_f32_from_f64(point.x);
  let Ok(y) = console_core_number_conversion::nearest_f32_from_f64(point.y);

  Ok(Point { x, y })
}

fn convert_rect(
  node: SvgNode,
  state: &converter::State,
) -> core::result::Result<Option<Path>, console_core_never::Never> {
  let Ok(zero) = Length::zero();
  let Ok(width) = node.convert_user_length(AId::Width, state, zero);
  let Ok(length_zero) = Length::zero();
  let Ok(height) = node.convert_user_length(AId::Height, state, length_zero);

  match width.length_validity() {
    crate::LengthValidity::Invalid => {
      console_core_logging::warn!(
        "Rect '{}' has an invalid 'width' value. Skipped.",
        ({
          let Ok(explicit_value) = node.element_id();
          explicit_value
        })
      );
      return Ok(None);
    }
    crate::LengthValidity::Valid => {}
  }

  match height.length_validity() {
    crate::LengthValidity::Invalid => {
      console_core_logging::warn!(
        "Rect '{}' has an invalid 'height' value. Skipped.",
        ({
          let Ok(explicit_value) = node.element_id();
          explicit_value
        })
      );
      return Ok(None);
    }
    crate::LengthValidity::Valid => {}
  }

  let Ok(x) = node.convert_user_length(AId::X, state, {
    let Ok(explicit_value) = Length::zero();
    explicit_value
  });
  let Ok(y) = node.convert_user_length(AId::Y, state, {
    let Ok(explicit_value) = Length::zero();
    explicit_value
  });

  let Ok((mut rx, mut ry)) = resolve_rx_ry(node, state);

  match ::core::primitive::bool::then_some(rx > width / 2.0, ()) {
    ::core::option::Option::Some(()) => {
      rx = width / 2.0;
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(ry > height / 2.0, ()) {
    ::core::option::Option::Some(()) => {
      ry = height / 2.0;
    }
    ::core::option::Option::None => {}
  }

  let path = match ::core::primitive::bool::then_some(rx.approx_eq_ulps(&0.0, 4), ()) {
    ::core::option::Option::Some(()) => {
      let Ok(rect) = FloatRectangle::from_xywh(x, y, width, height);
      let Ok(path_builder) = console_core_paths::PathBuilder::from_rect(match rect {
        Some(value) => value,
        None => return Ok(None),
      });

      path_builder
    }
    ::core::option::Option::None => {
      let Ok(mut builder) = console_core_paths::PathBuilder::new();
      let Ok(()) = builder.move_to(x + rx, y);

      let Ok(()) = builder.line_to(x + width - rx, y);
      let Ok(()) = append_quarter_arc(
        &mut builder,
        Radii { x: rx, y: ry },
        Point {
          x: x + width,
          y: y + ry,
        },
      );

      let Ok(()) = builder.line_to(x + width, y + height - ry);
      let Ok(()) = append_quarter_arc(
        &mut builder,
        Radii { x: rx, y: ry },
        Point {
          x: x + width - rx,
          y: y + height,
        },
      );

      let Ok(()) = builder.line_to(x + rx, y + height);
      let Ok(()) = append_quarter_arc(
        &mut builder,
        Radii { x: rx, y: ry },
        Point {
          x,
          y: y + height - ry,
        },
      );

      let Ok(()) = builder.line_to(x, y + ry);
      let Ok(()) =
        append_quarter_arc(&mut builder, Radii { x: rx, y: ry }, Point { x: x + rx, y });

      let Ok(()) = builder.close();

      let Ok(finish) = builder.finish();

      match finish {
        Some(value) => value,
        None => return Ok(None),
      }
    }
  };

  Ok(Some(path))
}

fn resolve_rx_ry(
  node: SvgNode,
  state: &converter::State,
) -> core::result::Result<(f32, f32), console_core_never::Never> {
  let Ok(mut rx_opt) = node.attribute::<Length>(AId::Rx);
  let Ok(mut ry_opt) = node.attribute::<Length>(AId::Ry);

  match rx_opt {
    Some(v) => match ::core::primitive::bool::then_some(v.number.is_sign_negative(), ()) {
      ::core::option::Option::Some(()) => {
        rx_opt = None;
      }
      ::core::option::Option::None => {}
    },
    None => {}
  }

  match ry_opt {
    Some(v) => match ::core::primitive::bool::then_some(v.number.is_sign_negative(), ()) {
      ::core::option::Option::Some(()) => {
        ry_opt = None;
      }
      ::core::option::Option::None => {}
    },
    None => {}
  }

  Ok(match (rx_opt, ry_opt) {
    (None, None) => (0.0, 0.0),
    (Some(rx), None) => {
      let Ok(rx_2) = units::convert_user_length(rx, node, AId::Rx, state);
      (rx_2, rx_2)
    }
    (None, Some(ry)) => {
      let Ok(ry_2) = units::convert_user_length(ry, node, AId::Ry, state);
      (ry_2, ry_2)
    }
    (Some(rx), Some(ry)) => {
      let Ok(rx_3) = units::convert_user_length(rx, node, AId::Rx, state);
      let Ok(ry_3) = units::convert_user_length(ry, node, AId::Ry, state);
      (rx_3, ry_3)
    }
  })
}

fn convert_line(
  node: SvgNode,
  state: &converter::State,
) -> core::result::Result<Option<Path>, console_core_never::Never> {
  let Ok(zero) = Length::zero();
  let Ok(x1) = node.convert_user_length(AId::X1, state, zero);
  let Ok(length_zero) = Length::zero();
  let Ok(y1) = node.convert_user_length(AId::Y1, state, length_zero);
  let Ok(x2) = node.convert_user_length(AId::X2, state, {
    let Ok(explicit_value) = Length::zero();
    explicit_value
  });
  let Ok(y2) = node.convert_user_length(AId::Y2, state, {
    let Ok(explicit_value) = Length::zero();
    explicit_value
  });

  let Ok(mut builder) = console_core_paths::PathBuilder::new();
  let Ok(()) = ::console_core_paths::PathBuilder::move_to_pt(
    &mut builder,
    ::console_core_paths::Point { x: x1, y: y1 },
  );
  let Ok(()) = ::console_core_paths::PathBuilder::line_to_pt(
    &mut builder,
    ::console_core_paths::Point { x: x2, y: y2 },
  );

  let Ok(finish) = builder.finish();

  Ok(finish)
}

fn convert_polyline(
  node: SvgNode,
) -> core::result::Result<Option<Path>, console_core_never::Never> {
  let builder = match {
    let Ok(explicit_value) = points_to_path(node, "Polyline");
    explicit_value
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(finish) = builder.finish();

  Ok(finish)
}

fn convert_polygon(
  node: SvgNode,
) -> core::result::Result<Option<Path>, console_core_never::Never> {
  let mut builder = match {
    let Ok(explicit_value) = points_to_path(node, "Polygon");
    explicit_value
  } {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(()) = builder.close();

  let Ok(finish) = builder.finish();

  Ok(finish)
}

fn points_to_path(node: SvgNode, shape: &str) -> Result<Option<PathBuilder>, Never> {
  use console_core_scalable_vector_graphics_values::PointsParser;

  let Ok(id) = node.element_id();
  let Ok(attribute) = node.attribute::<&str>(AId::Points);
  let text = match attribute {
    Some(text) => text,
    None => {
      console_core_logging::warn!("{shape} {id:?} has invalid points");
      return Ok(None);
    }
  };
  let mut points = PointsParser::from(text);
  let first = match points.next() {
    Some((x, y)) => kurbo::Point::new(x, y),
    None => {
      console_core_logging::warn!("{shape} {id:?} has fewer than 2 points");
      return Ok(None);
    }
  };
  let Ok(mut builder) = PathBuilder::new();
  let Ok(point) = path_point(first);
  let Ok(()) = builder.move_to_pt(point);

  for (x, y) in points {
    let Ok(point_2) = path_point(kurbo::Point::new(x, y));
    let Ok(()) = builder.line_to_pt(point_2);
  }

  let Ok(length) = ::console_core_number_conversion::fixed_index(match builder.len() {
    ::core::result::Result::Ok(length) => length,
  });

  match ::core::primitive::bool::then_some(
    (match ::console_core_number_conversion::index(length) {
      ::core::result::Result::Ok(native_position) => native_position,
    }) < 2,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      console_core_logging::warn!("{shape} {id:?} has fewer than 2 points");
      Ok(None)
    }
    ::core::option::Option::None => Ok(Some(builder)),
  }
}

fn convert_circle(
  node: SvgNode,
  state: &converter::State,
) -> core::result::Result<Option<Path>, console_core_never::Never> {
  let Ok(zero) = Length::zero();
  let Ok(cx) = node.convert_user_length(AId::Cx, state, zero);
  let Ok(length_zero) = Length::zero();
  let Ok(cy) = node.convert_user_length(AId::Cy, state, length_zero);
  let Ok(r) = node.convert_user_length(AId::R, state, {
    let Ok(explicit_value) = Length::zero();
    explicit_value
  });

  match r.length_validity() {
    crate::LengthValidity::Invalid => {
      console_core_logging::warn!(
        "Circle '{}' has an invalid 'r' value. Skipped.",
        ({
          let Ok(explicit_value) = node.element_id();
          explicit_value
        })
      );
      return Ok(None);
    }
    crate::LengthValidity::Valid => {}
  }

  Ok({
    let Ok(explicit_value) = ellipse_to_path(Point { x: cx, y: cy }, Radii { x: r, y: r });
    explicit_value
  })
}

fn convert_ellipse(
  node: SvgNode,
  state: &converter::State,
) -> core::result::Result<Option<Path>, console_core_never::Never> {
  let Ok(zero) = Length::zero();
  let Ok(cx) = node.convert_user_length(AId::Cx, state, zero);
  let Ok(length_zero) = Length::zero();
  let Ok(cy) = node.convert_user_length(AId::Cy, state, length_zero);
  let Ok((rx, ry)) = resolve_rx_ry(node, state);

  match rx.length_validity() {
    crate::LengthValidity::Invalid => {
      console_core_logging::warn!(
        "Ellipse '{}' has an invalid 'rx' value. Skipped.",
        ({
          let Ok(explicit_value) = node.element_id();
          explicit_value
        })
      );
      return Ok(None);
    }
    crate::LengthValidity::Valid => {}
  }

  match ry.length_validity() {
    crate::LengthValidity::Invalid => {
      console_core_logging::warn!(
        "Ellipse '{}' has an invalid 'ry' value. Skipped.",
        ({
          let Ok(explicit_value) = node.element_id();
          explicit_value
        })
      );
      return Ok(None);
    }
    crate::LengthValidity::Valid => {}
  }

  Ok({
    let Ok(explicit_value) = ellipse_to_path(Point { x: cx, y: cy }, Radii { x: rx, y: ry });
    explicit_value
  })
}

#[derive(Clone, Copy)]
#[cfg_attr(
  dylint_lib = "explicit067_no_unit_less_quantity",
  expect(
    explicit067_no_unit_less_quantity,
    reason = "The axis radii are resolved lengths in SVG user space"
  )
)]
struct Radii {
  x: f32,
  y: f32,
}

fn ellipse_to_path(center: Point, radius: Radii) -> Result<Option<Path>, Never> {
  let Point { x: cx, y: cy } = center;
  let Radii { x: rx, y: ry } = radius;
  let Ok(mut builder) = PathBuilder::new();
  let Ok(()) = builder.move_to(cx + rx, cy);

  for point in [
    Point { x: cx, y: cy + ry },
    Point { x: cx - rx, y: cy },
    Point { x: cx, y: cy - ry },
    Point { x: cx + rx, y: cy },
  ] {
    let Ok(()) = append_quarter_arc(&mut builder, radius, point);
  }

  let Ok(()) = builder.close();

  builder.finish()
}

#[cfg_attr(
  dylint_lib = "explicit047_no_captured_permission",
  expect(
    explicit047_no_captured_permission,
    reason = "Kurbo calls the cubic sink synchronously in order and never retains the mutable path builder"
  )
)]
fn append_quarter_arc(builder: &mut PathBuilder, radius: Radii, end: Point) -> Result<(), Never> {
  let Ok(last_point) = builder.last_point();
  let start = match last_point {
    Some(start) => start,
    None => return Ok(()),
  };
  let svg_arc = kurbo::SvgArc {
    from: kurbo::Point::new(f64::from(start.x), f64::from(start.y)),
    to: kurbo::Point::new(f64::from(end.x), f64::from(end.y)),
    radii: kurbo::Vec2::new(f64::from(radius.x), f64::from(radius.y)),
    x_rotation: 0.0,
    large_arc: false,
    sweep: true,
  };

  match kurbo::Arc::from_svg_arc(&svg_arc) {
    Some(arc) => {
      arc.to_cubic_beziers(0.1, |first, second, end_2| {
        let Ok(control1) = path_point(first);
        let Ok(control2) = path_point(second);
        let Ok(target) = path_point(end_2);
        let Ok(()) = builder.cubic_to_pt(control1, control2, target);
      });
    }
    None => {
      let Ok(()) = builder.line_to_pt(end);
    }
  }

  Ok(())
}
