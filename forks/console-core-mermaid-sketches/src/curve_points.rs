use kurbo::bezier;
use std::borrow::Borrow;
use std::cmp::{max_by, min_by};
use std::fmt::Display;
use std::ops::MulAssign;

use crate::work::{finite, UnlimitedWork, WorkControl, WorkError};
use euclid::default::Point2D;
use euclid::point2;
use num_traits::Float;

#[must_use]
pub(crate) fn distance_to_segment_squared<F, P>(p: P, v: P, w: P) -> F
where
    F: Float + PartialOrd + Display,
    P: Borrow<Point2D<F>>,
{
    let v_ = v.borrow();
    let w_ = w.borrow();
    let p_ = p.borrow();
    let l2 = v_.distance_to(*w_).powi(2);

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(l2 == F::zero(), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            p_.distance_to(*v_).powi(2)
        },
        ::core::option::Option::None => {
            let mut t = ((p_.x - v_.x) * (w_.x - v_.x) + (p_.y - v_.y) * (w_.y - v_.y)) / l2;
            t = max_by(
                F::zero(),
                min_by(F::one(), t, |a, b| {
                    a.partial_cmp(b)
                        .unwrap_or_else(|| panic!("can not compare {} and {}", a, b))
                }),
                |a, b| {
                    a.partial_cmp(b)
                        .unwrap_or_else(|| panic!("can not compare {} and {}", a, b))
                },
            );
            p_.distance_to(v_.lerp(*w_, t)).powi(2)
        },
    }
}

#[must_use]
pub(crate) fn flatness<F>(points: &[Point2D<F>], offset: usize) -> F
where
    F: Float + MulAssign,
{
    let points = [points[offset], points[offset + 1], points[offset + 2], points[offset + 3]];
    let two = F::from(2).unwrap();
    let three = F::from(3).unwrap();
    bezier::cubic_control_deviation(points.map(|point| point.x), two, three)
        + bezier::cubic_control_deviation(points.map(|point| point.y), two, three)
}

fn simplify_points_with_control<F: Float + Display>(
    points: &[Point2D<F>],
    start: usize,
    end: usize,
    epsilon: F,
    output: &mut Vec<Point2D<F>>,
    control: &mut impl WorkControl,
) -> Result<(), WorkError> {
    finite(&[epsilon])?;

    match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(start), &::core::convert::identity(end)); _explicit_ordering } {
        ::core::cmp::Ordering::Equal => {
            return Ok(());
        },
        ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Greater => {},
    }

    control.charge(1)?;
    let mut pending = vec![(start, end)];

    while let Some((start, end)) = pending.pop() {
        control.charge(1)?;
        let first = points[start];
        let last = points[end - 1];
        finite(&[first.x, first.y, last.x, last.y])?;
        let length_squared = first.distance_to(last).powi(2);
        finite(&[length_squared])?;
        let mut maximum = F::zero();
        let mut split = start;

        for (index, point) in points.iter().enumerate().take(end - 1).skip(start + 1) {
            control.charge(1)?;
            finite(&[
                point.x,
                point.y,
                (point.x - first.x) * (last.x - first.x) + (point.y - first.y) * (last.y - first.y),
            ])?;
            let distance = distance_to_segment_squared(*point, first, last);
            finite(&[distance])?;

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(distance > maximum, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    maximum = distance;
                    split = index;
                },
                ::core::option::Option::None => {},
            }
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(maximum.sqrt() > epsilon, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(split <= start || split >= end - 1, ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        return Err(WorkError::InvalidGeometry);
                    },
                    ::core::option::Option::None => {},
                }

                control.charge(2)?;
                pending.push((split, end));
                pending.push((start, split + 1));
            },
            ::core::option::Option::None => {
                control.charge(2)?;

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(output.is_empty(), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        output.push(first);
                    },
                    ::core::option::Option::None => {},
                }

                output.push(last);
            },
        }
    }

    Ok(())
}

pub(crate) fn simplify_with_control<F: Float + Display>(
    points: &[Point2D<F>],
    distance: F,
    control: &mut impl WorkControl,
) -> Result<Vec<Point2D<F>>, WorkError> {
    let mut output = Vec::new();
    simplify_points_with_control(points, 0, points.len(), distance, &mut output, control)?;
    Ok(output)
}

fn get_points_on_bezier_curve_with_splitting<F>(
    points: &[Point2D<F>],
    offset: usize,
    tolerance: F,
    new_points: &mut Vec<Point2D<F>>,
    control: &mut impl WorkControl,
) -> Result<(), WorkError>
where
    F: Float + MulAssign,
{
    control.charge(1)?;

    let mut pending = vec![[
        points[offset],
        points[offset + 1],
        points[offset + 2],
        points[offset + 3],
    ]];
    while let Some(curve) = pending.pop() {
        control.charge(1)?;

        for point in curve {
            finite(&[point.x, point.y])?;
        }

        let flatness = flatness(&curve, 0);
        finite(&[flatness])?;

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(flatness < tolerance, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                control.charge(2)?;
                let p0 = curve[0];

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(!new_points.is_empty(), ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        let d = new_points.last().unwrap().distance_to(p0);

                        match {
                            let _explicit_decision = ::core::primitive::bool::then_some(d > F::one(), ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                new_points.push(p0);
                            },
                            ::core::option::Option::None => {},
                        }
                    },
                    ::core::option::Option::None => {
                        new_points.push(p0);
                    },
                }

                new_points.push(curve[3]);
            },
            ::core::option::Option::None => {
                let t = F::from(0.5).unwrap();
                let [a, b, c, middle, d, e, f] = bezier::split_cubic(curve, |first, last| first.lerp(last, t));
                let left = [a, b, c, middle];
                let right = [middle, d, e, f];

                match {
                    let _explicit_decision = ::core::primitive::bool::then_some(left == curve || right == curve, ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {
                        return Err(WorkError::InvalidGeometry);
                    },
                    ::core::option::Option::None => {},
                }

                control.charge(2)?;
                pending.push(right);
                pending.push(left);
            },
        }
    }

    Ok(())
}

#[must_use]
pub(crate) fn points_on_bezier_curves<F>(
    points: &[Point2D<F>],
    tolerance: F,
    distance: Option<F>,
) -> Vec<Point2D<F>>
where
    F: Float + MulAssign + Display,
{
    points_on_bezier_curves_with_control(points, tolerance, distance, &mut UnlimitedWork)
        .expect("invalid bezier sampling")
}

pub(crate) fn points_on_bezier_curves_with_control<F>(
    points: &[Point2D<F>],
    tolerance: F,
    distance: Option<F>,
    control: &mut impl WorkControl,
) -> Result<Vec<Point2D<F>>, WorkError>
where
    F: Float + MulAssign + Display,
{
    finite(&[tolerance])?;

    match {
        let _explicit_decision = ::core::primitive::bool::then_some(tolerance <= F::zero(), ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            return Err(WorkError::InvalidGeometry);
        },
        ::core::option::Option::None => {},
    }

    let mut new_points = vec![];
    let num_segments = points.len() / 3;

    for i in 0..num_segments {
        let offset = i * 3;
        get_points_on_bezier_curve_with_splitting(
            points,
            offset,
            tolerance,
            &mut new_points,
            control,
        )?;
    }

    match distance {
        Some(dst) => {
            match {
                let _explicit_decision = ::core::primitive::bool::then_some(dst > F::zero(), ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    return simplify_with_control(&new_points, dst, control);
                },
                ::core::option::Option::None => {},
            }
        },
        ::core::option::Option::None => {},
    }

    Ok(new_points)
}

#[must_use]
pub(crate) fn curve_to_bezier<F>(
    points_in: &[Point2D<F>],
    curve_tightness: F,
) -> Option<Vec<Point2D<F>>>
where
    F: Float,
{
    match {
        let _explicit_decision = ::core::primitive::bool::then_some(points_in.len() < 3, ());
        _explicit_decision
    } {
        ::core::option::Option::Some(()) => {
            None
        },
        ::core::option::Option::None => {
            let mut out = vec![];

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(points_in.len() == 3, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {
                    let mut points_updated = vec![];
                    points_updated.extend_from_slice(points_in);
                    points_updated.push(*points_in.last().unwrap());
                    out = curve_to_bezier(&points_updated, curve_tightness).unwrap();
                },
                ::core::option::Option::None => {
                    let mut points = vec![];
                    points.push(points_in[0]);
                    points.push(points_in[0]);

                    for i in 1..points_in.len() {
                        points.push(points_in[i]);

                        match {
                            let _explicit_decision = ::core::primitive::bool::then_some(i == points_in.len() - 1, ());
                            _explicit_decision
                        } {
                            ::core::option::Option::Some(()) => {
                                points.push(points_in[i]);
                            },
                            ::core::option::Option::None => {},
                        }
                    }

                    let s = F::one() - curve_tightness;
                    out.push(points[0]);

                    for i in 1..points.len() - 2 {
                        let span = [points[i - 1], points[i], points[i + 1], points[i + 2]];
                        let divisor = F::from(6).unwrap();
                        let [first_x, second_x] = bezier::cardinal_controls(span.map(|p| p.x), s, divisor);
                        let [first_y, second_y] = bezier::cardinal_controls(span.map(|p| p.y), s, divisor);
                        let b_3 = points[i + 1];
                        out.push(point2(first_x, first_y));
                        out.push(point2(second_x, second_y));
                        out.push(b_3);
                    }
                },
            }

            Some(out)
        },
    }
}

#[cfg(test)]
mod tests {
    use euclid::point2;

    #[test]
    fn distance_to_segment_squared() {
        let expected = 1.0;
        let result = super::distance_to_segment_squared(
            point2(0.0, 1.0),
            point2(-1.0, 0.0),
            point2(1.0, 0.0),
        );
        assert_eq!(expected, result);
    }

    #[test]
    fn flatness() {
        let expected = 9.0;
        let result = super::flatness(
            &[
                point2(0.0, 1.0),
                point2(1.0, 3.0),
                point2(2.0, 3.0),
                point2(3.0, 4.0),
            ],
            0,
        );
        assert_eq!(expected, result);
    }

    #[test]
    fn points_on_bezier_curves_with_distance() {
        let input = vec![
            point2(70.0, 240.0),
            point2(145.0, 60.0),
            point2(275.0, 90.0),
            point2(300.0, 230.0),
        ];
        let result = super::points_on_bezier_curves(&input, 0.2, Some(0.15));
        assert_eq!(result.first(), Some(&point2(70.0, 240.0)));
        assert_eq!(result.last(), Some(&point2(300.0, 230.0)));
        assert!(result.len() > 10);
    }

    #[test]
    fn curve_to_bezier() {
        let expected = vec![
            point2(20.0, 240.0),
            point2(32.5, 211.5),
            point2(60.833333333333336, 94.0),
            point2(95.0, 69.0),
            point2(129.16666666666666, 44.0),
            point2(199.16666666666666, 71.5),
            point2(225.0, 90.0),
            point2(250.83333333333334, 108.5),
            point2(239.16666666666666, 158.33333333333334),
            point2(250.0, 180.0),
            point2(260.8333333333333, 201.66666666666666),
            point2(268.3333333333333, 236.66666666666666),
            point2(290.0, 220.0),
            point2(311.6666666666667, 203.33333333333334),
            point2(365.0, 103.33333333333333),
            point2(380.0, 80.0),
        ];
        let input = vec![
            point2(20.0, 240.0),
            point2(95.0, 69.0),
            point2(225.0, 90.0),
            point2(250.0, 180.0),
            point2(290.0, 220.0),
            point2(380.0, 80.0),
        ];
        let result = super::curve_to_bezier(&input, 0.0).unwrap();
        assert_eq!(result, expected);
    }
}
