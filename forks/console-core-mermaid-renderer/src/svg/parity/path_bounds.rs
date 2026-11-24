



#[derive(Debug, Clone, Copy)]
pub(super) struct SvgPathBounds {
    pub(super) min_x: f64,
    pub(super) min_y: f64,
    pub(super) max_x: f64,
    pub(super) max_y: f64,
}

impl SvgPathBounds {
    fn include_point(&mut self, x: f64, y: f64) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CubicBezier {
    pub(super) x0: f64,
    pub(super) y0: f64,
    pub(super) x1: f64,
    pub(super) y1: f64,
    pub(super) x2: f64,
    pub(super) y2: f64,
    pub(super) x3: f64,
    pub(super) y3: f64,
}

pub(super) fn svg_path_bounds_include_cubic(b: &mut SvgPathBounds, c: CubicBezier) {
    let curve = kurbo::CubicBez::new((c.x0, c.y0), (c.x1, c.y1), (c.x2, c.y2), (c.x3, c.y3));
    let bounds = kurbo::bezier::cubic_bounds_for_svg(curve);
    b.include_point(bounds.x0, bounds.y0);
    b.include_point(bounds.x1, bounds.y1);
}

#[must_use]
pub(super) fn svg_path_bounds_from_d(d: &str) -> Option<SvgPathBounds> {
    use std::f64::consts::PI;

    fn skip_sep(bytes: &[u8], i: &mut usize) {
        while *i < bytes.len() {
            match bytes[*i] {
                b' ' | b'\n' | b'\r' | b'\t' | b',' => *i += 1,
                _ => break,
            }
        }
    }

    fn parse_number(d: &str, bytes: &[u8], i: &mut usize) -> Option<f64> {
        skip_sep(bytes, i);

        if *i >= bytes.len() {
            return None;
        }

        let start = *i;

        if matches!(bytes[*i], b'+' | b'-') {
            *i += 1;
        }

        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }

        if *i < bytes.len() && bytes[*i] == b'.' {
            *i += 1;
            while *i < bytes.len() && bytes[*i].is_ascii_digit() {
                *i += 1;
            }
        }

        if *i < bytes.len() && matches!(bytes[*i], b'e' | b'E') {
            *i += 1;
            if *i < bytes.len() && matches!(bytes[*i], b'+' | b'-') {
                *i += 1;
            }

            while *i < bytes.len() && bytes[*i].is_ascii_digit() {
                *i += 1;
            }
        }

        match d[start..*i].parse::<f64>() { ::core::result::Result::Ok(value) => ::core::option::Option::Some(value), ::core::result::Result::Err(_invalid_float) => ::core::option::Option::None }
    }

    fn try_parse_number(d: &str, bytes: &[u8], i: &mut usize) -> Option<f64> {
        let save = *i;
        let v = parse_number(d, bytes, i);

        match v {
            ::core::option::Option::None => {
                *i = save;
            },
            ::core::option::Option::Some(_) => {},
        }

        v
    }

    fn parse_pair(d: &str, bytes: &[u8], i: &mut usize) -> Option<(f64, f64)> {
        let x = parse_number(d, bytes, i)?;
        let y = parse_number(d, bytes, i)?;
        Some((x, y))
    }

    fn try_parse_pair(d: &str, bytes: &[u8], i: &mut usize) -> Option<(f64, f64)> {
        let save = *i;
        let v = parse_pair(d, bytes, i);

        match v {
            ::core::option::Option::None => {
                *i = save;
            },
            ::core::option::Option::Some(_) => {},
        }

        v
    }

    fn quadratic_include_bounds(
        b: &mut SvgPathBounds,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    ) {
        let [_, cx1, cx2, _] = kurbo::bezier::raise_quadratic([x0, x1, x2], 2.0 / 3.0);
        let [_, cy1, cy2, _] = kurbo::bezier::raise_quadratic([y0, y1, y2], 2.0 / 3.0);
        svg_path_bounds_include_cubic(
            b,
            CubicBezier {
                x0,
                y0,
                x1: cx1,
                y1: cy1,
                x2: cx2,
                y2: cy2,
                x3: x2,
                y3: y2,
            },
        );
    }

    #[must_use]
    fn normalize_angle(mut a: f64) -> f64 {
        let two_pi = 2.0 * PI;
        a %= two_pi;

        if a < 0.0 {
            a += two_pi;
        }

        a
    }

    #[must_use]
    fn angle_between(theta: f64, start: f64, delta: f64) -> bool {
        let two_pi = 2.0 * PI;
        let eps = 1e-9;
        let t = normalize_angle(theta - start);

        if delta >= 0.0 {
            t <= delta + eps
        } else {
            t >= two_pi + delta - eps
        }
    }

    #[must_use]
    fn vec_angle(ux: f64, uy: f64, vx: f64, vy: f64) -> f64 {
        let dot = ux * vx + uy * vy;
        let det = ux * vy - uy * vx;
        det.atan2(dot)
    }

    #[derive(Debug, Clone, Copy)]
    struct ArcBoundsInput {
        x0: f64,
        y0: f64,
        rx0: f64,
        ry0: f64,
        x_axis_rotation_deg: f64,
        large_arc: bool,
        sweep: bool,
        x1: f64,
        y1: f64,
    }

    fn arc_include_bounds(b: &mut SvgPathBounds, arc: ArcBoundsInput) {
        let ArcBoundsInput {
            x0,
            y0,
            rx0,
            ry0,
            x_axis_rotation_deg,
            large_arc,
            sweep,
            x1,
            y1,
        } = arc;


        if rx0.abs() < 1e-12 || ry0.abs() < 1e-12 {
            b.include_point(x0, y0);
            b.include_point(x1, y1);
            return;
        }

        let phi = x_axis_rotation_deg.to_radians();
        let (cos_phi, sin_phi) = (phi.cos(), phi.sin());
        let mut rx = rx0.abs();
        let mut ry = ry0.abs();

        let dx2 = (x0 - x1) / 2.0;
        let dy2 = (y0 - y1) / 2.0;

        let x1p = cos_phi * dx2 + sin_phi * dy2;
        let y1p = -sin_phi * dx2 + cos_phi * dy2;

        let x1p2 = x1p * x1p;
        let y1p2 = y1p * y1p;


        let lam = x1p2 / (rx * rx) + y1p2 / (ry * ry);

        if lam > 1.0 {
            let s = lam.sqrt();
            rx *= s;
            ry *= s;
        }

        let rx2 = rx * rx;
        let ry2 = ry * ry;

        let num = (rx2 * ry2) - (rx2 * y1p2) - (ry2 * x1p2);
        let den = (rx2 * y1p2) + (ry2 * x1p2);

        if den.abs() < 1e-24 {
            b.include_point(x0, y0);
            b.include_point(x1, y1);
            return;
        }

        let mut sq = num / den;

        if sq < 0.0 {
            sq = 0.0;
        }

        let sign = if large_arc == sweep { -1.0 } else { 1.0 };
        let coef = sign * sq.sqrt();

        let cxp = coef * (rx * y1p) / ry;
        let cyp = coef * (-ry * x1p) / rx;

        let cx = cos_phi * cxp - sin_phi * cyp + (x0 + x1) / 2.0;
        let cy = sin_phi * cxp + cos_phi * cyp + (y0 + y1) / 2.0;

        let ux = (x1p - cxp) / rx;
        let uy = (y1p - cyp) / ry;
        let vx = (-x1p - cxp) / rx;
        let vy = (-y1p - cyp) / ry;

        let theta1 = vec_angle(1.0, 0.0, ux, uy);
        let mut delta = vec_angle(ux, uy, vx, vy);

        if !sweep && delta > 0.0 {
            delta -= 2.0 * PI;
        } else if sweep && delta < 0.0 {
            delta += 2.0 * PI;
        }

        let start = theta1;
        let end = theta1 + delta;

        let arc_point = |theta: f64| -> (f64, f64) {
            let (ct, st) = (theta.cos(), theta.sin());
            let x = cx + rx * ct * cos_phi - ry * st * sin_phi;
            let y = cy + rx * ct * sin_phi + ry * st * cos_phi;
            (x, y)
        };

        b.include_point(x0, y0);
        b.include_point(x1, y1);
        let (sx, sy) = arc_point(start);
        let (ex, ey) = arc_point(end);
        b.include_point(sx, sy);
        b.include_point(ex, ey);




        let tx_base = (-ry * sin_phi).atan2(rx * cos_phi);

        for k in 0..2 {
            let t = tx_base + <f64 as ::core::convert::From<::core::primitive::i32>>::from(k) * PI;

            if angle_between(t, start, delta) {
                let (x, y) = arc_point(t);
                b.include_point(x, y);
            }
        }

        let ty_base = (ry * cos_phi).atan2(rx * sin_phi);

        for k in 0..2 {
            let t = ty_base + <f64 as ::core::convert::From<::core::primitive::i32>>::from(k) * PI;

            if angle_between(t, start, delta) {
                let (x, y) = arc_point(t);
                b.include_point(x, y);
            }
        }
    }

    let bytes = d.as_bytes();
    let mut i = 0usize;
    let mut cmd: u8 = 0;
    let mut cx = 0.0f64;
    let mut cy = 0.0f64;
    let (mut sx, mut sy) = (0.0f64, 0.0f64);
    let mut last_cubic_ctrl: Option<(f64, f64)> = None;
    let mut last_quad_ctrl: Option<(f64, f64)> = None;
    let mut prev_cmd: u8 = 0;
    let mut b: Option<SvgPathBounds> = None;

    while i < bytes.len() {
        skip_sep(bytes, &mut i);

        if i >= bytes.len() {
            break;
        }

        let ch = bytes[i];

        if ch.is_ascii_alphabetic() {
            cmd = ch;
            i += 1;
        } else if cmd == 0 {
            return None;
        }

        let is_rel = cmd.is_ascii_lowercase();
        let ucmd = cmd.to_ascii_uppercase();

        fn ensure_bounds(b: &mut Option<SvgPathBounds>, x: f64, y: f64) -> &mut SvgPathBounds {
            b.get_or_insert(SvgPathBounds {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            })
        }

        match ucmd {
            b'M' => {

                let (mut x, mut y) = parse_pair(d, bytes, &mut i)?;

                if is_rel {
                    x += cx;
                    y += cy;
                }

                cx = x;
                cy = y;
                sx = x;
                sy = y;
                ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
                prev_cmd = ucmd;

                while let Some((mut nx, mut ny)) = try_parse_pair(d, bytes, &mut i) {
                    if is_rel {
                        nx += cx;
                        ny += cy;
                    }

                    cx = nx;
                    cy = ny;
                    ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                    prev_cmd = b'L';
                }
            }
            b'Z' => {

                let cur = ensure_bounds(&mut b, cx, cy);
                cur.include_point(cx, cy);
                cur.include_point(sx, sy);
                cx = sx;
                cy = sy;
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
                prev_cmd = ucmd;
            }
            b'L' => {
                let (mut x, mut y) = parse_pair(d, bytes, &mut i)?;

                if is_rel {
                    x += cx;
                    y += cy;
                }

                cx = x;
                cy = y;
                ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
                prev_cmd = ucmd;

                while let Some((mut nx, mut ny)) = try_parse_pair(d, bytes, &mut i) {
                    if is_rel {
                        nx += cx;
                        ny += cy;
                    }

                    cx = nx;
                    cy = ny;
                    ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                    prev_cmd = ucmd;
                }
            }
            b'H' => {
                let mut x = parse_number(d, bytes, &mut i)?;

                if is_rel {
                    x += cx;
                }

                cx = x;
                ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
                prev_cmd = ucmd;

                while let Some(mut nx) = try_parse_number(d, bytes, &mut i) {
                    if is_rel {
                        nx += cx;
                    }

                    cx = nx;
                    ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                    prev_cmd = ucmd;
                }
            }
            b'V' => {
                let mut y = parse_number(d, bytes, &mut i)?;

                if is_rel {
                    y += cy;
                }

                cy = y;
                ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
                prev_cmd = ucmd;

                while let Some(mut ny) = try_parse_number(d, bytes, &mut i) {
                    if is_rel {
                        ny += cy;
                    }

                    cy = ny;
                    ensure_bounds(&mut b, cx, cy).include_point(cx, cy);
                    prev_cmd = ucmd;
                }
            }
            b'C' => {
                let mut x1 = parse_number(d, bytes, &mut i)?;
                let mut y1 = parse_number(d, bytes, &mut i)?;
                let mut x2 = parse_number(d, bytes, &mut i)?;
                let mut y2 = parse_number(d, bytes, &mut i)?;
                let mut x3 = parse_number(d, bytes, &mut i)?;
                let mut y3 = parse_number(d, bytes, &mut i)?;

                if is_rel {
                    x1 += cx;
                    y1 += cy;
                    x2 += cx;
                    y2 += cy;
                    x3 += cx;
                    y3 += cy;
                }

                let cur = ensure_bounds(&mut b, cx, cy);
                svg_path_bounds_include_cubic(
                    cur,
                    CubicBezier {
                        x0: cx,
                        y0: cy,
                        x1,
                        y1,
                        x2,
                        y2,
                        x3,
                        y3,
                    },
                );
                cx = x3;
                cy = y3;
                last_cubic_ctrl = Some((x2, y2));
                last_quad_ctrl = None;
                prev_cmd = ucmd;

                'inner_loop_2: loop {
                    let save = i;

                    let mut nx1 = {
                        let optional_nx1 = try_parse_number(d, bytes, &mut i);

                        match optional_nx1 {
                            Some(matched_nx1) => matched_nx1,
                            ::core::option::Option::None => break 'inner_loop_2,
                        }
                    };

                    let Some(mut ny1) = try_parse_number(d, bytes, &mut i) else {
                        i = save;
                        break 'inner_loop_2;
                    };

                    let Some(mut nx2) = try_parse_number(d, bytes, &mut i) else {
                        i = save;
                        break 'inner_loop_2;
                    };

                    let Some(mut ny2) = try_parse_number(d, bytes, &mut i) else {
                        i = save;
                        break 'inner_loop_2;
                    };

                    let Some(mut nx3) = try_parse_number(d, bytes, &mut i) else {
                        i = save;
                        break 'inner_loop_2;
                    };

                    let Some(mut ny3) = try_parse_number(d, bytes, &mut i) else {
                        i = save;
                        break 'inner_loop_2;
                    };

                    if is_rel {
                        nx1 += cx;
                        ny1 += cy;
                        nx2 += cx;
                        ny2 += cy;
                        nx3 += cx;
                        ny3 += cy;
                    }

                    let cur_2 = ensure_bounds(&mut b, cx, cy);
                    svg_path_bounds_include_cubic(
                        cur_2,
                        CubicBezier {
                            x0: cx,
                            y0: cy,
                            x1: nx1,
                            y1: ny1,
                            x2: nx2,
                            y2: ny2,
                            x3: nx3,
                            y3: ny3,
                        },
                    );
                    cx = nx3;
                    cy = ny3;
                    last_cubic_ctrl = Some((nx2, ny2));
                    last_quad_ctrl = None;
                    prev_cmd = ucmd;
                }
            }
            b'S' => {
                let (x1, y1) = if matches!(prev_cmd, b'C' | b'S') {
                    match last_cubic_ctrl {
                        Some((px, py)) => {
                            (2.0 * cx - px, 2.0 * cy - py)
                        },
                        ::core::option::Option::None => {
                            (cx, cy)
                        },
                    }
                } else {
                    (cx, cy)
                };

                let mut x2 = parse_number(d, bytes, &mut i)?;
                let mut y2 = parse_number(d, bytes, &mut i)?;
                let mut x3 = parse_number(d, bytes, &mut i)?;
                let mut y3 = parse_number(d, bytes, &mut i)?;

                if is_rel {
                    x2 += cx;
                    y2 += cy;
                    x3 += cx;
                    y3 += cy;
                }

                let cur = ensure_bounds(&mut b, cx, cy);
                svg_path_bounds_include_cubic(
                    cur,
                    CubicBezier {
                        x0: cx,
                        y0: cy,
                        x1,
                        y1,
                        x2,
                        y2,
                        x3,
                        y3,
                    },
                );
                cx = x3;
                cy = y3;
                last_cubic_ctrl = Some((x2, y2));
                last_quad_ctrl = None;
                prev_cmd = ucmd;
            }
            b'Q' => {
                let mut x1 = parse_number(d, bytes, &mut i)?;
                let mut y1 = parse_number(d, bytes, &mut i)?;
                let mut x2 = parse_number(d, bytes, &mut i)?;
                let mut y2 = parse_number(d, bytes, &mut i)?;

                if is_rel {
                    x1 += cx;
                    y1 += cy;
                    x2 += cx;
                    y2 += cy;
                }

                let cur = ensure_bounds(&mut b, cx, cy);
                quadratic_include_bounds(cur, cx, cy, x1, y1, x2, y2);
                cx = x2;
                cy = y2;
                last_quad_ctrl = Some((x1, y1));
                last_cubic_ctrl = None;
                prev_cmd = ucmd;
            }
            b'T' => {
                let (x1, y1) = if matches!(prev_cmd, b'Q' | b'T') {
                    match last_quad_ctrl {
                        Some((qx, qy)) => {
                            (2.0 * cx - qx, 2.0 * cy - qy)
                        },
                        ::core::option::Option::None => {
                            (cx, cy)
                        },
                    }
                } else {
                    (cx, cy)
                };
                let (mut x2, mut y2) = parse_pair(d, bytes, &mut i)?;

                if is_rel {
                    x2 += cx;
                    y2 += cy;
                }

                let cur = ensure_bounds(&mut b, cx, cy);
                quadratic_include_bounds(cur, cx, cy, x1, y1, x2, y2);
                cx = x2;
                cy = y2;
                last_quad_ctrl = Some((x1, y1));
                last_cubic_ctrl = None;
                prev_cmd = ucmd;
            }
            b'A' => {
                let rx = parse_number(d, bytes, &mut i)?;
                let ry = parse_number(d, bytes, &mut i)?;
                let rot = parse_number(d, bytes, &mut i)?;
                let laf = parse_number(d, bytes, &mut i)?;
                let sf = parse_number(d, bytes, &mut i)?;
                let (mut x1, mut y1) = parse_pair(d, bytes, &mut i)?;

                if is_rel {
                    x1 += cx;
                    y1 += cy;
                }

                let large_arc = laf.abs() > 0.5;
                let sweep = sf.abs() > 0.5;

                match b.as_mut() {
                    Some(cur) => {
                        arc_include_bounds(
                            cur,
                            ArcBoundsInput {
                                x0: cx,
                                y0: cy,
                                rx0: rx,
                                ry0: ry,
                                x_axis_rotation_deg: rot,
                                large_arc,
                                sweep,
                                x1,
                                y1,
                            },
                        );
                    },
                    ::core::option::Option::None => {
                        let mut cur = SvgPathBounds {
                            min_x: cx,
                            min_y: cy,
                            max_x: cx,
                            max_y: cy,
                        };
                        arc_include_bounds(
                            &mut cur,
                            ArcBoundsInput {
                                x0: cx,
                                y0: cy,
                                rx0: rx,
                                ry0: ry,
                                x_axis_rotation_deg: rot,
                                large_arc,
                                sweep,
                                x1,
                                y1,
                            },
                        );
                        b = Some(cur);
                    },
                }

                cx = x1;
                cy = y1;
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
                prev_cmd = ucmd;
            }
            _ => return None,
        }
    }

    b
}

/// Returns the rendered centerline length for an SVG path.
///
/// Mermaid derives Neo edge masking from `SVGPathElement.getTotalLength()`. Kurbo parses the same
/// SVG path grammar, including arcs, and evaluates Bezier arc length without flattening the path
/// into a local polyline approximation.
#[must_use]
pub(super) fn svg_path_length_from_d(d: &str) -> Option<f64> {
    use kurbo::Shape;

    let path = kurbo::BezPath::from_svg(d).ok()?;




    let length = f64::from(path.perimeter(1.0e-6) as f32);
    length.is_finite().then_some(length)
}

#[cfg(test)]
mod tests {
    use super::svg_path_length_from_d;

    #[test]
    fn total_length_uses_svg_dom_float_precision() {
        assert_eq!(
            svg_path_length_from_d("M0,0L0.1,0"),
            Some(0.10000000149011612)
        );
        assert_eq!(svg_path_length_from_d("M0,0L1,1"), Some(1.4142135381698608));
    }

    #[test]
    fn neo_marker_mask_length_is_stable_across_platforms() {

        let d = "M214.9,268.158L214.9,287.23L214.9,300.159Q214.9,307.23 221.971,307.23L293.662,307.23Q300.733,307.23 300.733,314.301L300.733,343.162Q300.733,347.23 304.801,347.23L308.87,347.23";
        let length = svg_path_length_from_d(d).unwrap();
        assert_eq!(length, 166.18093872070312);
        let mut style = String::new();
        super::super::edge_path::write_neo_edge_mask(
            &mut style,
            length,
            None,
            Some("arrow_point"),
            false,
            false,
        );
        assert_eq!(
            style,
            "stroke-dasharray: 0 0 162.18093872070313 4; stroke-dashoffset: 0;"
        );
    }
}
