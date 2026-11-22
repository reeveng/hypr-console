use kurbo::{Point, Vec2, bezier};
use std::borrow::Borrow;
use std::f64::consts::PI;

use crate::work::{finite, UnlimitedWork, WorkControl, WorkError};
use svgtypes::{ArcSize, ArcSweep, CoordinateMode, PathSegment};

/// Convert relative SVG path commands to absolute commands.
#[must_use]
pub(crate) fn absolutize(
    path_segments: impl Iterator<Item = impl Borrow<PathSegment>>,
) -> impl Iterator<Item = PathSegment> {
    absolutize_with_control(path_segments, &mut UnlimitedWork).expect("invalid absolute SVG path")
}

pub(crate) fn absolutize_with_control(
    path_segments: impl Iterator<Item = impl Borrow<PathSegment>>,
    control: &mut impl WorkControl,
) -> Result<std::vec::IntoIter<PathSegment>, WorkError> {
    let mut result = vec![];
    let (mut cx, mut cy, mut subx, mut suby) = (0.0, 0.0, 0.0, 0.0);

    for segment in path_segments {
        control.charge(1)?;
        finite_segment(*segment.borrow())?;

        match *segment.borrow() {
            PathSegment::MoveTo { abs: CoordinateMode::Absolute, x, y } => {
                result.push(*segment.borrow());
                cx = x;
                cy = y;
                subx = x;
                suby = y;
            }
            PathSegment::MoveTo { abs: CoordinateMode::Relative, x, y } => {
                cx += x;
                cy += y;
                result.push(PathSegment::MoveTo {
                    abs: CoordinateMode::Absolute,
                    x: cx,
                    y: cy,
                });
                subx = cx;
                suby = cy;
            }
            PathSegment::LineTo { abs: CoordinateMode::Absolute, x, y } => {
                result.push(*segment.borrow());
                cx = x;
                cy = y;
            }
            PathSegment::LineTo { abs: CoordinateMode::Relative, x, y } => {
                cx += x;
                cy += y;
                result.push(PathSegment::LineTo {
                    abs: CoordinateMode::Absolute,
                    x: cx,
                    y: cy,
                });
            }
            PathSegment::CurveTo {
                abs: CoordinateMode::Absolute,
                x1: _,
                y1: _,
                x2: _,
                y2: _,
                x,
                y,
            } => {
                result.push(*segment.borrow());
                cx = x;
                cy = y;
            }
            PathSegment::CurveTo {
                abs: CoordinateMode::Relative,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                result.push(PathSegment::CurveTo {
                    abs: CoordinateMode::Absolute,
                    x1: x1 + cx,
                    y1: y1 + cy,
                    x2: x2 + cx,
                    y2: y2 + cy,
                    x: x + cx,
                    y: y + cy,
                });
                cx += x;
                cy += y;
            }
            PathSegment::Quadratic {
                abs: CoordinateMode::Absolute,
                x1: _,
                y1: _,
                x,
                y,
            } => {
                result.push(*segment.borrow());
                cx = x;
                cy = y;
            }
            PathSegment::Quadratic {
                abs: CoordinateMode::Relative,
                x1,
                y1,
                x,
                y,
            } => {
                result.push(PathSegment::Quadratic {
                    abs: CoordinateMode::Absolute,
                    x1: x1 + cx,
                    y1: y1 + cy,
                    x: x + cx,
                    y: y + cy,
                });
                cx += x;
                cy += y;
            }
            PathSegment::EllipticalArc {
                abs: CoordinateMode::Absolute,
                rx: _,
                ry: _,
                x_axis_rotation: _,
                large_arc: _,
                sweep: _,
                x,
                y,
            } => {
                result.push(*segment.borrow());
                cx = x;
                cy = y;
            }
            PathSegment::EllipticalArc {
                abs: CoordinateMode::Relative,
                rx,
                ry,
                x_axis_rotation,
                large_arc,
                sweep,
                x,
                y,
            } => {
                cx += x;
                cy += y;
                result.push(PathSegment::EllipticalArc {
                    abs: CoordinateMode::Absolute,
                    rx,
                    ry,
                    x_axis_rotation,
                    large_arc,
                    sweep,
                    x: cx,
                    y: cy,
                });
            }
            PathSegment::HorizontalLineTo { abs: CoordinateMode::Absolute, x } => {
                result.push(*segment.borrow());
                cx = x;
            }
            PathSegment::HorizontalLineTo { abs: CoordinateMode::Relative, x } => {
                cx += x;
                result.push(PathSegment::HorizontalLineTo { abs: CoordinateMode::Absolute, x: cx });
            }
            PathSegment::VerticalLineTo { abs: CoordinateMode::Absolute, y } => {
                result.push(*segment.borrow());
                cy = y;
            }
            PathSegment::VerticalLineTo { abs: CoordinateMode::Relative, y } => {
                cy += y;
                result.push(PathSegment::VerticalLineTo { abs: CoordinateMode::Absolute, y: cy });
            }
            PathSegment::SmoothCurveTo {
                abs: CoordinateMode::Absolute,
                x2: _,
                y2: _,
                x,
                y,
            } => {
                result.push(*segment.borrow());
                cx = x;
                cy = y;
            }
            PathSegment::SmoothCurveTo {
                abs: CoordinateMode::Relative,
                x2,
                y2,
                x,
                y,
            } => {
                result.push(PathSegment::SmoothCurveTo {
                    abs: CoordinateMode::Absolute,
                    x2: x2 + cx,
                    y2: y2 + cy,
                    x: x + cx,
                    y: y + cy,
                });
                cx += x;
                cy += y;
            }
            PathSegment::SmoothQuadratic { abs: CoordinateMode::Absolute, x, y } => {
                result.push(*segment.borrow());
                cx = x;
                cy = y;
            }
            PathSegment::SmoothQuadratic { abs: CoordinateMode::Relative, x, y } => {
                cx += x;
                cy += y;
                result.push(PathSegment::SmoothQuadratic {
                    abs: CoordinateMode::Absolute,
                    x: cx,
                    y: cy,
                });
            }
            PathSegment::ClosePath { .. } => {
                result.push(PathSegment::ClosePath { abs: CoordinateMode::Absolute });
                cx = subx;
                cy = suby;
            }
        }
    }

    Ok(result.into_iter())
}

/// Normalize absolute SVG path commands into `M/L/C/Z`.
#[must_use]
pub(crate) fn normalize(
    path_segments: impl Iterator<Item = impl Borrow<PathSegment>>,
) -> impl Iterator<Item = PathSegment> {
    normalize_with_control(path_segments, &mut UnlimitedWork).expect("invalid normalized SVG path")
}

pub(crate) fn normalize_with_control(
    path_segments: impl Iterator<Item = impl Borrow<PathSegment>>,
    control: &mut impl WorkControl,
) -> Result<std::vec::IntoIter<PathSegment>, WorkError> {
    let mut out = vec![];

    let mut cx = 0.0;
    let mut cy = 0.0;
    let mut subx = 0.0;
    let mut suby = 0.0;
    let mut lcx = 0.0;
    let mut lcy = 0.0;
    let mut last_type: Option<PathSegment> = None;

    for segment in path_segments {
        control.charge(4)?;
        finite_segment(*segment.borrow())?;
        let output_start = out.len();

        match *segment.borrow() {
            PathSegment::MoveTo { abs: CoordinateMode::Absolute, x, y } => {
                out.push(*segment.borrow());
                cx = x;
                cy = y;
                subx = x;
                suby = y;
            }
            PathSegment::CurveTo {
                abs: CoordinateMode::Absolute,
                x1: _,
                y1: _,
                x2,
                y2,
                x,
                y,
            } => {
                out.push(*segment.borrow());
                cx = x;
                cy = y;
                lcx = x2;
                lcy = y2;
            }
            PathSegment::LineTo { abs: CoordinateMode::Absolute, x, y } => {
                out.push(*segment.borrow());
                cx = x;
                cy = y;
            }
            PathSegment::HorizontalLineTo { abs: CoordinateMode::Absolute, x } => {
                cx = x;
                out.push(PathSegment::LineTo {
                    abs: CoordinateMode::Absolute,
                    x: cx,
                    y: cy,
                });
            }
            PathSegment::VerticalLineTo { abs: CoordinateMode::Absolute, y } => {
                cy = y;
                out.push(PathSegment::LineTo {
                    abs: CoordinateMode::Absolute,
                    x: cx,
                    y: cy,
                });
            }
            PathSegment::SmoothCurveTo {
                abs: CoordinateMode::Absolute,
                x2,
                y2,
                x,
                y,
            } => {
                let (cx1, cy1) = match last_type {
                    Some(lt) => {
                        match {
                            let _explicit_decision = ::core::primitive::bool::then_some(matches!(lt, PathSegment::CurveTo { .. })
                            || matches!(lt, PathSegment::SmoothCurveTo { .. }), ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                (bezier::reflected_control(cx, lcx), bezier::reflected_control(cy, lcy))
                            },
                            ::core::option::Option::None => {
                                (cx, cy)
                            },
                        }
                    },
                    ::core::option::Option::None => {
                        (cx, cy)
                    },
                };
                out.push(PathSegment::CurveTo {
                    abs: CoordinateMode::Absolute,
                    x1: cx1,
                    y1: cy1,
                    x2,
                    y2,
                    x,
                    y,
                });
                lcx = x2;
                lcy = y2;
                cx = x;
                cy = y;
            }
            PathSegment::SmoothQuadratic { abs: CoordinateMode::Absolute, x, y } => {
                let (x1, y1) = match last_type {
                    Some(lt) => {
                        match {
                            let _explicit_decision = ::core::primitive::bool::then_some(matches!(lt, PathSegment::Quadratic { .. })
                            || matches!(lt, PathSegment::SmoothQuadratic { .. }), ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                (bezier::reflected_control(cx, lcx), bezier::reflected_control(cy, lcy))
                            },
                            ::core::option::Option::None => {
                                (cx, cy)
                            },
                        }
                    },
                    ::core::option::Option::None => {
                        (cx, cy)
                    },
                };
                let [_, cx1, cx2, _] = bezier::raise_quadratic_svg([cx, x1, x]);
                let [_, cy1, cy2, _] = bezier::raise_quadratic_svg([cy, y1, y]);
                out.push(PathSegment::CurveTo {
                    abs: CoordinateMode::Absolute,
                    x1: cx1,
                    y1: cy1,
                    x2: cx2,
                    y2: cy2,
                    x,
                    y,
                });
                lcx = x1;
                lcy = y1;
                cx = x;
                cy = y;
            }
            PathSegment::Quadratic {
                abs: CoordinateMode::Absolute,
                x1,
                y1,
                x,
                y,
            } => {
                let [_, cx1, cx2, _] = bezier::raise_quadratic_svg([cx, x1, x]);
                let [_, cy1, cy2, _] = bezier::raise_quadratic_svg([cy, y1, y]);
                out.push(PathSegment::CurveTo {
                    abs: CoordinateMode::Absolute,
                    x1: cx1,
                    y1: cy1,
                    x2: cx2,
                    y2: cy2,
                    x,
                    y,
                });
                lcx = x1;
                lcy = y1;
                cx = x;
                cy = y;
            }
            PathSegment::EllipticalArc {
                abs: CoordinateMode::Absolute,
                rx,
                ry,
                x_axis_rotation,
                large_arc,
                sweep,
                x,
                y,
            } => {
                let r1 = rx.abs();
                let r2 = ry.abs();
                let angle = x_axis_rotation;
                let large_arc_flag = large_arc;
                let sweep_flag = sweep;

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(r1 == 0.0 || r2 == 0.0, ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        out.push(PathSegment::CurveTo {
                            abs: CoordinateMode::Absolute,
                            x1: cx,
                            y1: cy,
                            x2: x,
                            y2: y,
                            x,
                            y,
                        });
                        cx = x;
                        cy = y;
                    },
                    ::core::option::Option::None => match {
                        let _explicit_decision = ::core::primitive::bool::then_some(cx != x || cy != y, ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {
                            let curves = arc_to_cubic_curves(
                                cx,
                                cy,
                                x,
                                y,
                                r1,
                                r2,
                                angle,
                                large_arc_flag,
                                sweep_flag,
                                None,
                            );

                            for curve in curves.iter() {
                                out.push(PathSegment::CurveTo {
                                    abs: CoordinateMode::Absolute,
                                    x1: curve[0],
                                    y1: curve[1],
                                    x2: curve[2],
                                    y2: curve[3],
                                    x: curve[4],
                                    y: curve[5],
                                });
                            }

                            cx = x;
                            cy = y;
                        },
                        ::core::option::Option::None => {},
                    },
                }
            }
            PathSegment::ClosePath { abs: CoordinateMode::Absolute } => {
                out.push(*segment.borrow());
                cx = subx;
                cy = suby;
            }
            _ => panic!("Not expecting none absolute path!"),
        }

        last_type = Some(*segment.borrow());

        for output in &out[output_start..] {
            finite_segment(*output)?;
        }
    }

    Ok(out.into_iter())
}

pub(crate) fn finite_segment(segment: PathSegment) -> Result<(), WorkError> {
    match segment {
        PathSegment::MoveTo { x, y, .. }
        | PathSegment::LineTo { x, y, .. }
        | PathSegment::SmoothQuadratic { x, y, .. } => finite(&[x, y]),
        PathSegment::HorizontalLineTo { x, .. } => finite(&[x]),
        PathSegment::VerticalLineTo { y, .. } => finite(&[y]),
        PathSegment::CurveTo {
            x1,
            y1,
            x2,
            y2,
            x,
            y,
            ..
        } => finite(&[x1, y1, x2, y2, x, y]),
        PathSegment::SmoothCurveTo { x2, y2, x, y, .. } => finite(&[x2, y2, x, y]),
        PathSegment::Quadratic { x1, y1, x, y, .. } => finite(&[x1, y1, x, y]),
        PathSegment::EllipticalArc {
            rx,
            ry,
            x_axis_rotation,
            x,
            y,
            ..
        } => finite(&[rx, ry, x_axis_rotation, x, y]),
        PathSegment::ClosePath { .. } => Ok(()),
    }
}

#[must_use]
fn rotate(x: f64, y: f64, angle_rad: f64) -> (f64, f64) {
    let rotated_x = x * angle_rad.cos() - y * angle_rad.sin();
    let rotated_y = x * angle_rad.sin() + y * angle_rad.cos();
    (rotated_x, rotated_y)
}

#[must_use]
fn normalized_arc_angle(ratio: f64) -> f64 {
    // path-data-parser normalizes each asin input with parseFloat(value.toFixed(9)).
    let mut buffer = ryu_js::Buffer::new();
    buffer
        .format_to_fixed(ratio, 9)
        .parse::<f64>()
        .expect("ryu-js formats a valid floating-point value")
        .asin()
}

#[allow(clippy::too_many_arguments)]
#[must_use]
fn arc_to_cubic_curves(
    mut x1: f64,
    mut y1: f64,
    mut x2: f64,
    mut y2: f64,
    mut r1: f64,
    mut r2: f64,
    angle: f64,
    large_arc_flag: ArcSize,
    sweep_flag: ArcSweep,
    recursive: Option<[f64; 4]>,
) -> Vec<Vec<f64>> {
    let angle_rad = angle.to_radians();
    let mut params: Vec<Vec<f64>> = vec![];
    let (mut f1, mut f2, cx, cy);

    match recursive {
        Some(rec) => {
            f1 = rec[0];
            f2 = rec[1];
            cx = rec[2];
            cy = rec[3];
        },
        ::core::option::Option::None => {
            (x1, y1) = rotate(x1, y1, -angle_rad);
            (x2, y2) = rotate(x2, y2, -angle_rad);
            let x = (x1 - x2) / 2.0;
            let y = (y1 - y2) / 2.0;
            let mut h = (x * x) / (r1 * r1) + (y * y) / (r2 * r2);

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(h > 1.0, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    h = h.sqrt();
                    r1 *= h;
                    r2 *= h;
                },
                ::core::option::Option::None => {},
            }

            let sign = match (large_arc_flag, sweep_flag) {
                (ArcSize::Large, ArcSweep::Positive) | (ArcSize::Small, ArcSweep::Negative) => -1.0,
                (ArcSize::Large, ArcSweep::Negative) | (ArcSize::Small, ArcSweep::Positive) => 1.0,
            };
            let r1_pow = r1.powi(2);
            let r2_pow = r2.powi(2);
            let left = r1_pow * r2_pow - r1_pow * y * y - r2_pow * x * x;
            let right = r1_pow * y * y + r2_pow * x * x;

            let k = sign * (left / right).abs().sqrt();

            cx = k * r1 * y / r2 + (x1 + x2) / 2.0;
            cy = k * -r2 * x / r1 + (y1 + y2) / 2.0;

            f1 = normalized_arc_angle((y1 - cy) / r2);
            f2 = normalized_arc_angle((y2 - cy) / r2);

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(x1 < cx, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    f1 = PI - f1;
                },
                ::core::option::Option::None => {},
            }

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(x2 < cx, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    f2 = PI - f2;
                },
                ::core::option::Option::None => {},
            }

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(f1 < 0.0, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    f1 += PI * 2.0;
                },
                ::core::option::Option::None => {},
            }

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(f2 < 0.0, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    f2 += PI * 2.0;
                },
                ::core::option::Option::None => {},
            }

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(sweep_flag == ArcSweep::Positive && f1 > f2, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    f1 -= PI * 2.0;
                },
                ::core::option::Option::None => {},
            }

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(sweep_flag == ArcSweep::Negative && f2 > f1, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    f2 -= PI * 2.0;
                },
                ::core::option::Option::None => {},
            }
        },
    }

    let df = f2 - f1;

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(df.abs() > (PI * 120.0 / 180.0), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            let f2old = f2;
            let x2old = x2;
            let y2old = y2;

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(sweep_flag == ArcSweep::Positive && f2 > f1, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    f2 = f1 + (PI * 120.0 / 180.0);
                },
                ::core::option::Option::None => {
                    f2 = f1 - (PI * 120.0 / 180.0);
                },
            }

            x2 = cx + r1 * f2.cos();
            y2 = cy + r2 * f2.sin();
            params = arc_to_cubic_curves(
                x2,
                y2,
                x2old,
                y2old,
                r1,
                r2,
                angle,
                ArcSize::Small,
                sweep_flag,
                Some([f2, f2old, cx, cy]),
            );
        },
        ::core::option::Option::None => {},
    }

    let [first, second] = bezier::elliptical_controls(
        Point::new(x1, y1),
        Point::new(x2, y2),
        Vec2::new(r1, r2),
        [f1, f2],
    );
    let m2 = vec![first.x, first.y];
    let m3 = vec![second.x, second.y];
    let m4 = vec![x2, y2];

    match recursive {
        ::core::option::Option::Some(_) => {
            let mut ret_val = vec![m2, m3, m4];
            ret_val.append(&mut params);
            ret_val
        },
        ::core::option::Option::None => {
            let mut ret_val = vec![m2, m3, m4];
            ret_val.append(&mut params);
            let mut curves = vec![];

            for i in (0..ret_val.len()).step_by(3) {
                let r1 = rotate(ret_val[i][0], ret_val[i][1], angle_rad);
                let r2 = rotate(ret_val[i + 1][0], ret_val[i + 1][1], angle_rad);
                let r3 = rotate(ret_val[i + 2][0], ret_val[i + 2][1], angle_rad);
                curves.push(vec![r1.0, r1.1, r2.0, r2.1, r3.0, r3.1]);
            }

            curves
        },
    }
}

#[cfg(test)]
mod tests {
    use svgtypes::{ArcSize, ArcSweep, CoordinateMode, PathParser, PathSegment};

    use super::{absolutize, normalize, normalized_arc_angle};

    #[test]
    fn absolutize_handles_relative_segments() {
        let path: String = "m 0 0 c 3 -0.6667 6 -1.3333 9 -2 a 1 1 0 0 0 -8 -1 a 1 1 0 0 0 -2 0 l 0 4 v 2 h 8 q 4 -10 9 -5 t -6 8 z".into();
        let path_segments: Vec<PathSegment> = PathParser::from(path.as_ref()).flatten().collect();
        let mut absolute = absolutize(path_segments.iter());

        assert_eq!(
            absolute.next().unwrap(),
            PathSegment::MoveTo {
                abs: CoordinateMode::Absolute,
                x: 0.0,
                y: 0.0
            }
        );
        assert_eq!(
            absolute.next().unwrap(),
            PathSegment::CurveTo {
                abs: CoordinateMode::Absolute,
                x1: 3.0,
                y1: -0.6667,
                x2: 6.0,
                y2: -1.3333,
                x: 9.0,
                y: -2.0
            }
        );
        assert_eq!(
            absolute.next().unwrap(),
            PathSegment::EllipticalArc {
                abs: CoordinateMode::Absolute,
                rx: 1.0,
                ry: 1.0,
                x_axis_rotation: 0.0,
                large_arc: ArcSize::Small,
                sweep: ArcSweep::Negative,
                x: 1.0,
                y: -3.0
            }
        );
        assert_eq!(
            absolute.next().unwrap(),
            PathSegment::EllipticalArc {
                abs: CoordinateMode::Absolute,
                rx: 1.0,
                ry: 1.0,
                x_axis_rotation: 0.0,
                large_arc: ArcSize::Small,
                sweep: ArcSweep::Negative,
                x: -1.0,
                y: -3.0
            }
        );
        assert_eq!(
            absolute.next().unwrap(),
            PathSegment::LineTo {
                abs: CoordinateMode::Absolute,
                x: -1.0,
                y: 1.0
            }
        );
        assert_eq!(
            absolute.next().unwrap(),
            PathSegment::VerticalLineTo { abs: CoordinateMode::Absolute, y: 3.0 }
        );
        assert_eq!(
            absolute.next().unwrap(),
            PathSegment::HorizontalLineTo { abs: CoordinateMode::Absolute, x: 7.0 }
        );
        assert_eq!(
            absolute.next().unwrap(),
            PathSegment::Quadratic {
                abs: CoordinateMode::Absolute,
                x1: 11.0,
                y1: -7.0,
                x: 16.0,
                y: -2.0
            }
        );
        assert_eq!(
            absolute.next().unwrap(),
            PathSegment::SmoothQuadratic {
                abs: CoordinateMode::Absolute,
                x: 10.0,
                y: 6.0
            }
        );
        assert_eq!(
            absolute.next().unwrap(),
            PathSegment::ClosePath { abs: CoordinateMode::Absolute }
        );
    }

    #[test]
    fn normalize_converts_to_cubic_segments() {
        let path: String = "M5.5.5A.5.5 0 016 0h4a.5.5 0 010 1H9v1.07a7.002 7.002 0 013.537 12.26l.817.816a.5.5 0 01-.708.708l-.924-.925A6.967 6.967 0 018 16a6.967 6.967 0 01-3.722-1.07l-.924.924a.5.5 0 01-.708-.708l.817-.816A7.002 7.002 0 017 2.07V1H5.999a.5.5 0 01-.5-.5z".into();
        let path_segments: Vec<PathSegment> = PathParser::from(path.as_ref()).flatten().collect();
        let absolute = absolutize(path_segments.iter());
        let mut normalized = normalize(absolute);

        assert_eq!(
            normalized.next().unwrap(),
            PathSegment::MoveTo {
                abs: CoordinateMode::Absolute,
                x: 5.5,
                y: 0.5
            }
        );
        assert!(matches!(
            normalized.next().unwrap(),
            PathSegment::CurveTo { abs: CoordinateMode::Absolute, .. }
        ));
    }

    #[test]
    fn arc_angles_preserve_javascript_decimal_rounding_boundaries() {
        for (ratio, normalized) in [
            (0.9999999995_f64, 0.999999999_f64),
            (1.0 / 1024.0, 0.000976563),
            (-1.0 / 1024.0, -0.000976563),
        ] {
            assert_eq!(normalized_arc_angle(ratio), normalized.asin());
        }
    }

    #[test]
    fn normalize_keeps_small_rounded_rectangle_arcs_finite() {
        let path = "M -31.4 -31.5 H 31.4 A 0.1 0.1 0 0 1 31.5 -31.4 V 31.4 A 0.1 0.1 0 0 1 31.4 31.5 H -31.4 A 0.1 0.1 0 0 1 -31.5 31.4 V -31.4 A 0.1 0.1 0 0 1 -31.4 -31.5 Z";
        let segments: Vec<_> = PathParser::from(path).map(Result::unwrap).collect();
        let curves: Vec<_> = normalize(absolutize(segments.iter()))
            .filter_map(|segment| match segment {
                PathSegment::CurveTo {
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                    ..
                } => Some([x1, y1, x2, y2, x, y]),
                _ => None,
            })
            .collect();

        assert_eq!(curves.len(), 4);
        for (curve, endpoint) in
            curves
                .iter()
                .zip([[31.5, -31.4], [31.4, 31.5], [-31.5, 31.4], [-31.4, -31.5]])
        {
            assert!(
                curve.iter().all(|coordinate| coordinate.is_finite()),
                "{curve:?}"
            );
            assert_eq!(curve[4..], endpoint);
        }
    }
}
