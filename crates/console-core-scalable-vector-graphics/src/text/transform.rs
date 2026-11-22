use console_core_never::Never;

pub(crate) fn skrifa_to_tsp_transform(
  t: skrifa::color::Transform,
) -> Result<console_core_paths::Transform, Never> {
  console_core_paths::Transform::from_row(t.xx, t.yx, t.xy, t.yy, t.dx, t.dy)
}

pub(crate) fn tsp_to_skrifa_transform(
  t: console_core_paths::Transform,
) -> Result<skrifa::color::Transform, Never> {
  Ok(skrifa::color::Transform {
    xx: t.sx,
    yx: t.ky,
    xy: t.kx,
    yy: t.sy,
    dx: t.tx,
    dy: t.ty,
  })
}

pub(crate) struct PathCubics<'a> {
  segments: console_core_paths::PathSegmentsIter<'a>,
  previous: console_core_paths::Point,
  subpath_start: console_core_paths::Point,
}

impl<'a> PathCubics<'a> {
  pub(crate) fn new(path: &'a console_core_paths::Path) -> Result<Self, console_core_never::Never> {
    let Ok(segments) = path.segments();
    let Ok(points) = path.points();
    let first = match points.first() {
      Some(point) => *point,
      None => {
        let Ok(zero) = console_core_paths::Point::zero();

        zero
      }
    };

    Ok(Self { segments, previous: first, subpath_start: first })
  }
}

impl Iterator for PathCubics<'_> {
  type Item = kurbo::CubicBez;

  fn next(&mut self) -> Option<Self::Item> {
    use console_core_paths::PathSegment;

    for segment in &mut self.segments {
      let Ok(start) = kurbo_point(self.previous);
      let curve = match segment {
        PathSegment::MoveTo(point) => {
          self.subpath_start = point;
          self.previous = point;
          continue;
        }
        PathSegment::LineTo(point) => {
          let Ok(end) = kurbo_point(point);
          let Ok(line) = cubic_line(kurbo::Line::new(start, end));

          line
        }
        PathSegment::QuadTo(control, point) => {
          let Ok(control_2) = kurbo_point(control);
          let Ok(end) = kurbo_point(point);

          kurbo::QuadBez::new(start, control_2, end).raise()
        }
        PathSegment::CubicTo(first, second, point) => {
          let Ok(first_control) = kurbo_point(first);
          let Ok(second_control) = kurbo_point(second);
          let Ok(end) = kurbo_point(point);

          kurbo::CubicBez::new(start, first_control, second_control, end)
        }
        PathSegment::Close => {
          let Ok(end) = kurbo_point(self.subpath_start);
          let Ok(line) = cubic_line(kurbo::Line::new(start, end));

          line
        }
      };
      let Ok(x) = console_core_number_conversion::nearest_f32_from_f64(curve.p3.x);
      let Ok(y) = console_core_number_conversion::nearest_f32_from_f64(curve.p3.y);
      self.previous = console_core_paths::Point { x, y };

      return Some(curve);
    }

    None
  }
}

fn kurbo_point(point: console_core_paths::Point) -> Result<kurbo::Point, console_core_never::Never> {
  Ok(kurbo::Point::new(f64::from(point.x), f64::from(point.y)))
}

fn cubic_line(line: kurbo::Line) -> Result<kurbo::CubicBez, console_core_never::Never> {
  use kurbo::ParamCurve;

  Ok(kurbo::CubicBez::new(line.p0, line.eval(0.33), line.eval(0.66), line.p1))
}
