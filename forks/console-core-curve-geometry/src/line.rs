// Copyright 2018 the Kurbo Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Lines.

use core::ops::{Add, Mul, Range, Sub};

use console_core_fixed_capacity_vectors::FixedVec;
use console_core_line::LineSegment;

#[cfg(not(feature = "std"))]
use crate::common::FloatFuncs;

use crate::{
    Affine, MAX_EXTREMA, Nearest, ParamCurve, ParamCurveArclen, ParamCurveArea,
    ParamCurveCurvature, ParamCurveDeriv, ParamCurveExtrema, ParamCurveNearest, PathEl, Point,
    Rect, Shape, Vec2,
};

/// A single line.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Line {
    /// The line's start point.
    pub p0: Point,
    /// The line's end point.
    pub p1: Point,
}

impl Line {
    /// Create a new line.
    #[inline(always)]
    pub fn new(p0: impl Into<Point>, p1: impl Into<Point>) -> Line {
        Line {
            p0: p0.into(),
            p1: p1.into(),
        }
    }

    const fn segment(&self) -> LineSegment<f64> {
        LineSegment {
            start: self.p0.coordinates(),
            end: self.p1.coordinates(),
        }
    }

    const fn from_segment(segment: LineSegment<f64>) -> Self {
        Self {
            p0: Point::from_coordinates(segment.start),
            p1: Point::from_coordinates(segment.end),
        }
    }

    /// Returns a copy of this `Line` with the end points swapped so that it
    /// points in the opposite direction.
    #[must_use]
    #[inline(always)]
    pub const fn reversed(&self) -> Line {
        let Ok(reversed) = self.segment().reversed();
        Self::from_segment(reversed)
    }

    /// The length of the line.
    #[inline]
    pub fn length(self) -> f64 {
        let Ok(length_squared) = self.segment().length_squared();
        length_squared.sqrt()
    }

    /// The midpoint of the line.
    ///
    /// This is the same as calling [`Point::midpoint`] with
    /// the endpoints of this line.
    #[must_use]
    #[inline]
    pub fn midpoint(&self) -> Point {
        let Ok(point) = self.segment().midpoint();
        Point::from(point)
    }

    /// Computes the point where two lines, if extended to infinity, would cross.
    pub fn crossing_point(self, other: Line) -> Option<Point> {
        let other = other.segment();
        let Ok(crossing) = self.segment().crossing(other);
        let Ok(point) = other.point_at(crossing?.second_fraction);
        Some(Point::from(point))
    }

    /// Is this line `finite`?
    ///
    /// [finite]: f64::is_finite
    #[inline]
    pub const fn is_finite(self) -> bool {
        self.p0.is_finite() && self.p1.is_finite()
    }

    /// Is this line `NaN`?
    ///
    /// [NaN]: f64::is_nan
    #[inline]
    pub const fn is_nan(self) -> bool {
        self.p0.is_nan() || self.p1.is_nan()
    }
}

impl From<Line> for LineSegment<f64> {
    fn from(line: Line) -> Self {
        line.segment()
    }
}

impl From<LineSegment<f64>> for Line {
    fn from(segment: LineSegment<f64>) -> Self {
        Self::from_segment(segment)
    }
}

impl From<(Point, Point)> for Line {
    #[inline(always)]
    fn from((from, to): (Point, Point)) -> Self {
        Line::new(from, to)
    }
}

impl From<(Point, Vec2)> for Line {
    #[inline(always)]
    fn from((origin, displacement): (Point, Vec2)) -> Self {
        Line::new(origin, origin + displacement)
    }
}

impl ParamCurve for Line {
    #[inline]
    fn eval(&self, t: f64) -> Point {
        let Ok(point) = self.segment().point_at(t);
        Point::from(point)
    }

    #[inline]
    fn subsegment(&self, range: Range<f64>) -> Line {
        Line {
            p0: self.eval(range.start),
            p1: self.eval(range.end),
        }
    }

    #[inline(always)]
    fn start(&self) -> Point {
        self.p0
    }

    #[inline(always)]
    fn end(&self) -> Point {
        self.p1
    }
}

impl ParamCurveDeriv for Line {
    type DerivResult = ConstPoint;

    #[inline]
    fn deriv(&self) -> ConstPoint {
        ConstPoint((self.p1 - self.p0).to_point())
    }
}

impl ParamCurveArclen for Line {
    #[inline]
    fn arclen(&self, _accuracy: f64) -> f64 {
        self.length()
    }

    #[inline]
    fn inv_arclen(&self, arclen: f64, _accuracy: f64) -> f64 {
        arclen / self.length()
    }
}

impl ParamCurveArea for Line {
    #[inline]
    fn signed_area(&self) -> f64 {
        self.p0.to_vec2().cross(self.p1.to_vec2()) * 0.5
    }
}

impl ParamCurveNearest for Line {
    #[inline]
    fn nearest(&self, point: Point, _accuracy: f64) -> Nearest {
        let Ok(projection) = self.segment().nearest(point.coordinates());
        Nearest {
            distance_sq: projection.distance_squared,
            t: projection.fraction,
        }
    }
}

impl ParamCurveCurvature for Line {
    #[inline(always)]
    fn curvature(&self, _t: f64) -> f64 {
        0.0
    }
}

impl ParamCurveExtrema for Line {
    #[inline]
    fn extrema(&self) -> FixedVec<f64, MAX_EXTREMA> {
        FixedVec::new()
    }
}

/// A trivial "curve" that is just a constant.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConstPoint(Point);

impl ConstPoint {
    /// Is this point [finite]?
    ///
    /// [finite]: f64::is_finite
    #[inline]
    pub const fn is_finite(self) -> bool {
        self.0.is_finite()
    }

    /// Is this point [NaN]?
    ///
    /// [NaN]: f64::is_nan
    #[inline]
    pub const fn is_nan(self) -> bool {
        self.0.is_nan()
    }
}

impl ParamCurve for ConstPoint {
    #[inline(always)]
    fn eval(&self, _t: f64) -> Point {
        self.0
    }

    #[inline(always)]
    fn subsegment(&self, _range: Range<f64>) -> ConstPoint {
        *self
    }
}

impl ParamCurveDeriv for ConstPoint {
    type DerivResult = ConstPoint;

    #[inline(always)]
    fn deriv(&self) -> ConstPoint {
        ConstPoint(Point::new(0.0, 0.0))
    }
}

impl ParamCurveArclen for ConstPoint {
    #[inline(always)]
    fn arclen(&self, _accuracy: f64) -> f64 {
        0.0
    }

    #[inline(always)]
    fn inv_arclen(&self, _arclen: f64, _accuracy: f64) -> f64 {
        0.0
    }
}

impl Mul<Line> for Affine {
    type Output = Line;

    #[inline]
    fn mul(self, other: Line) -> Line {
        Line {
            p0: self * other.p0,
            p1: self * other.p1,
        }
    }
}

impl Add<Vec2> for Line {
    type Output = Line;

    #[inline]
    fn add(self, v: Vec2) -> Line {
        Line::new(self.p0 + v, self.p1 + v)
    }
}

impl Sub<Vec2> for Line {
    type Output = Line;

    #[inline]
    fn sub(self, v: Vec2) -> Line {
        Line::new(self.p0 - v, self.p1 - v)
    }
}

/// An iterator yielding the path for a single line.
#[doc(hidden)]
pub struct LinePathIter {
    line: Line,
    ix: usize,
}

impl Shape for Line {
    type PathElementsIter<'iter> = LinePathIter;

    #[inline]
    fn path_elements(&self, _tolerance: f64) -> LinePathIter {
        LinePathIter { line: *self, ix: 0 }
    }

    /// Returning zero here is consistent with the contract (area is
    /// only meaningful for closed shapes), but an argument can be made
    /// that the contract should be tightened to include the Green's
    /// theorem contribution.
    #[inline(always)]
    fn area(&self) -> f64 {
        0.0
    }

    #[inline]
    fn perimeter(&self, _accuracy: f64) -> f64 {
        self.length()
    }

    /// Same consideration as `area`.
    #[inline(always)]
    fn winding(&self, _pt: Point) -> i32 {
        0
    }

    #[inline(always)]
    fn bounding_box(&self) -> Rect {
        let Ok(bounds) = self.segment().bounds();
        Rect::from_bounds(bounds)
    }

    #[inline(always)]
    fn as_line(&self) -> Option<Line> {
        Some(*self)
    }
}

impl Iterator for LinePathIter {
    type Item = PathEl;

    fn next(&mut self) -> Option<PathEl> {
        self.ix += 1;
        match self.ix {
            1 => Some(PathEl::MoveTo(self.line.p0)),
            2 => Some(PathEl::LineTo(self.line.p1)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Line, ParamCurveArclen, Point};

    #[test]
    fn line_reversed() {
        let l = Line::new((0.0, 0.0), (1.0, 1.0));
        let f = l.reversed();

        assert_eq!(l.p0, f.p1);
        assert_eq!(l.p1, f.p0);

        // Reversing it again should result in the original line
        assert_eq!(l, f.reversed());
    }

    #[test]
    fn line_arclen() {
        let l = Line::new((0.0, 0.0), (1.0, 1.0));
        let true_len = 2.0f64.sqrt();
        let epsilon = 1e-9;
        assert!(l.arclen(epsilon) - true_len < epsilon);

        let t = l.inv_arclen(true_len / 3.0, epsilon);
        assert!((t - 1.0 / 3.0).abs() < epsilon);
    }

    #[test]
    fn line_midpoint() {
        let l = Line::new((0.0, 0.0), (2.0, 4.0));
        assert_eq!(l.midpoint(), Point::new(1.0, 2.0));
    }

    #[test]
    fn line_is_finite() {
        assert!(
            (Line {
                p0: Point::new(0., 0.),
                p1: Point::new(1., 1.)
            })
            .is_finite()
        );

        assert!(
            !(Line {
                p0: Point::new(0., 0.),
                p1: Point::new(f64::INFINITY, 1.)
            })
            .is_finite()
        );

        assert!(
            !(Line {
                p0: Point::new(0., 0.),
                p1: Point::new(0., f64::INFINITY)
            })
            .is_finite()
        );
    }

    #[test]
    fn line_nearest() {
        use crate::{ParamCurve, ParamCurveNearest};

        const EPSILON: f64 = 1e-9;

        let line = Line::new((-4., 0.), (2., 1.));

        // Projects onto the line segment end point.
        let point = Point::new(4., 0.);
        let nearest = line.nearest(point, 0.);
        assert_eq!(nearest.t, 1.);
        assert!((nearest.distance_sq - line.p1.distance_squared(point)).abs() < EPSILON);

        // Projects onto the line segment start point.
        let point = Point::new(0., -50.);
        let nearest = line.nearest(point, 0.);
        assert_eq!(nearest.t, 0.);
        assert!((nearest.distance_sq - line.p0.distance_squared(point)).abs() < EPSILON);

        // Projects onto the line segment proper (not just onto one of its extrema).
        let point = Point::new(-1., 0.5);
        let nearest = line.nearest(point, 0.);
        assert!(nearest.t > 0. && nearest.t < 1.);
        // Ensure evaluating and calculating distance manually has the same result.
        assert!(
            (line.eval(nearest.t).distance_squared(point) - nearest.distance_sq).abs() < EPSILON
        );

        // Test minimality while avoiding reimplementing projection in this test by checking that
        // moving to a slightly different point on the segment increases the distance.
        assert!(line.eval(nearest.t * 0.95).distance_squared(point) > nearest.distance_sq);
        assert!(line.eval(nearest.t * 1.05).distance_squared(point) > nearest.distance_sq);
    }
}
