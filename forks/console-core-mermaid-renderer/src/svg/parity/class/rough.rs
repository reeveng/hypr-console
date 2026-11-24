use super::super::roughjs_common::{controlled, ops_to_svg_path_d, parse_hex_color_to_srgba};
use super::super::*;

#[must_use]
fn class_parse_stroke_dash_pair(stroke_dasharray: &str) -> (f64, f64) {
    let dash = stroke_dasharray.trim().replace(',', " ");
    let mut nums = dash
        .split_whitespace()
        .filter_map(|t| match t.parse::<f64>() { ::core::result::Result::Ok(value) => ::core::option::Option::Some(value), ::core::result::Result::Err(_invalid_float) => ::core::option::Option::None });

    match (nums.next(), nums.next()) {
        (Some(a), Some(b)) => (a, b),
        (Some(a), None) => (a, a),
        _ => (0.0, 0.0),
    }
}

fn roughjs46_diverge_point(options: &mut roughr::core::Options) -> f64 {
    0.2 + options.random() * 0.2
}

#[must_use]
pub(super) fn class_rough_seed(
    base_randomness: &roughr::core::RoughRandomness,
    _diagram_id: &str,
    _dom_id: &str,
) -> roughr::core::RoughRandomness {
    base_randomness.clone()
}

#[allow(clippy::too_many_arguments)]
#[must_use]
pub(super) fn class_rough_hachure_rect_paths(
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &crate::resources::OperationWorkMeter,
    randomness: &roughr::core::RoughRandomness,
) -> Option<(String, String)> {
    let fill_2 =
        match parse_hex_color_to_srgba(fill) {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => roughr::Srgba::new(0.0, 0.0, 0.0, 1.0),
        };
    let stroke_2 =
        match parse_hex_color_to_srgba(stroke) {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => roughr::Srgba::new(0.0, 0.0, 0.0, 1.0),
        };
    let (dash0, dash1) = class_parse_stroke_dash_pair(stroke_dasharray);
    let options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(0.7)
        .fill(fill_2)
        .fill_style(roughr::core::FillStyle::Hachure)

        .fill_weight(1.5)
        .hachure_gap(1.5)
        .stroke(stroke_2)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    let drawable =
        controlled::generate(work_meter, merman_model::OperationPhase::Emit, |control| {
            roughr::generator::Generator::default().rectangle_with_control::<f64>(
                left,
                top,
                width,
                height,
                &Some(options),
                control,
            )
        })
        .ok()?;
    let mut fill_d = None;
    let mut stroke_d = None;

    for set in drawable.sets {
        let d = ops_to_svg_path_d(&set);

        match set.op_set_type {
            roughr::core::OpSetType::FillPath | roughr::core::OpSetType::FillSketch => {
                fill_d = Some(d);
            }
            roughr::core::OpSetType::Path => {
                stroke_d = Some(d);
            }
        }
    }

    Some(({ let checked_value = fill_d?; checked_value }, { let checked_value_2 = stroke_d?; checked_value_2 }))
}

#[allow(clippy::too_many_arguments)]
#[must_use]
pub(super) fn class_rough_hand_drawn_line_path(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &crate::resources::OperationWorkMeter,
    randomness: &roughr::core::RoughRandomness,
) -> Option<String> {
    let stroke_2 =
        match parse_hex_color_to_srgba(stroke) {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => roughr::Srgba::new(0.0, 0.0, 0.0, 1.0),
        };
    let (dash0, dash1) = class_parse_stroke_dash_pair(stroke_dasharray);
    let mut options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(0.7)
        .stroke(stroke_2)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .build()
        .ok()?;

    controlled::generate(work_meter, merman_model::OperationPhase::Emit, |control| {
        roughr::renderer::line_with_control::<f64>(x1, y1, x2, y2, &mut options, control)
    })
    .ok()
    .map(|opset| ops_to_svg_path_d(&opset))
}

#[must_use]
pub(super) fn class_rough_hand_drawn_stroke_path_for_svg_path(
    svg_path_data: &str,
    roughness: f32,
    work_meter: &crate::resources::OperationWorkMeter,
    randomness: &roughr::core::RoughRandomness,
) -> Option<String> {
    let mut options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(roughness)
        .disable_multi_stroke(false)
        .build()
        .ok()?;

    controlled::generate(work_meter, merman_model::OperationPhase::Emit, |control| {
        let segments =
            roughr::points_on_path::normalized_path_with_control(svg_path_data, control)?;
        roughr::renderer::svg_normalized_segments_with_control::<f64>(
            &segments,
            &mut options,
            control,
        )
    })
    .ok()
    .map(|opset| ops_to_svg_path_d(&opset))
}

#[must_use]
pub(super) fn class_rough_line_double_path_and_bounds(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    randomness: &roughr::core::RoughRandomness,
) -> (String, super::super::path_bounds::SvgPathBounds) {
    let mut options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .build()
        .expect("class rough line options must be valid");
    class_rough_line_double_path_and_bounds_with_options(x1, y1, x2, y2, &mut options)
}

fn class_rough_line_double_path_and_bounds_with_options(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    options: &mut roughr::core::Options,
) -> (String, super::super::path_bounds::SvgPathBounds) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let mut d = String::new();
    let mut pb = super::super::path_bounds::SvgPathBounds {
        min_x: x1,
        min_y: y1,
        max_x: x1,
        max_y: y1,
    };

    for idx in 0..2 {
        let diverge = roughjs46_diverge_point(options);

        for _ in 0..10 {
            let _ = options.random();
        }

        let c1x = x1 + dx * diverge;
        let c1y = y1 + dy * diverge;
        let c2x = x1 + dx * 2.0 * diverge;
        let c2y = y1 + dy * 2.0 * diverge;

        if idx > 0 {
            d.push(' ');
        }

        let _ = write!(
            &mut d,
            "M{} {} C{} {}, {} {}, {} {}",
            fmt(x1),
            fmt(y1),
            fmt(c1x),
            fmt(c1y),
            fmt(c2x),
            fmt(c2y),
            fmt(x2),
            fmt(y2),
        );
        super::super::path_bounds::svg_path_bounds_include_cubic(
            &mut pb,
            super::super::path_bounds::CubicBezier {
                x0: x1,
                y0: y1,
                x1: c1x,
                y1: c1y,
                x2: c2x,
                y2: c2y,
                x3: x2,
                y3: y2,
            },
        );
    }

    (d, pb)
}

#[must_use]
pub(super) fn class_rough_rect_stroke_path_and_bounds(
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    randomness: &roughr::core::RoughRandomness,
) -> (String, super::super::path_bounds::SvgPathBounds) {
    let right = left + width;
    let bottom = top + height;
    let mut options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .build()
        .expect("class rough rectangle options must be valid");
    let mut out = String::new();
    let mut pb_opt: Option<super::super::path_bounds::SvgPathBounds> = None;

    for (idx, (ax, ay, bx, by)) in [
        (left, top, right, top),
        (right, top, right, bottom),
        (right, bottom, left, bottom),
        (left, bottom, left, top),
    ]
    .into_iter()
    .enumerate()
    {
        let (segment, seg_pb) =
            class_rough_line_double_path_and_bounds_with_options(ax, ay, bx, by, &mut options);

        if idx > 0 {
            out.push(' ');
        }

        out.push_str(&segment);

        if let Some(pb) = pb_opt.as_mut() {
            pb.min_x = pb.min_x.min(seg_pb.min_x);
            pb.min_y = pb.min_y.min(seg_pb.min_y);
            pb.max_x = pb.max_x.max(seg_pb.max_x);
            pb.max_y = pb.max_y.max(seg_pb.max_y);
        } else {
            pb_opt = Some(seg_pb);
        }
    }

    (
        out,
        match (pb_opt, super::super::path_bounds::SvgPathBounds {
                min_x: left,
                min_y: top,
                max_x: right,
                max_y: bottom,
            }) {
            (::core::option::Option::Some(present), _) => present,
            (::core::option::Option::None, alternate) => alternate,
        },
    )
}

#[cfg(test)]
mod control_tests {
    use super::*;
    use crate::resources::{
        OperationWorkError, OperationWorkMeter, RenderResourcePolicy, ResourceLimitCause,
        ResourceLimitId,
    };
    use merman_model::{OperationControl, OperationPhase};

    #[test]
    fn class_rough_generation_preserves_work_and_cancellation_terminals() {
        for cancelled in [false, true] {
            for primitive in 0..3 {
                let control = OperationControl::new();
                if cancelled {
                    control.cancel();
                }
                let meter = OperationWorkMeter::new_with_control(
                    RenderResourcePolicy::unbounded_for_trusted_input()
                        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 32)
                        .unwrap(),
                    control,
                );
                let randomness = roughr::core::RoughRandomness::new(
                    roughr::core::RoughJsSeed::new(1.0),
                    roughr::core::RoughMathRandom::new(7),
                );
                let result = match primitive {
                    0 => class_rough_hachure_rect_paths(
                        0.0,
                        0.0,
                        4096.0,
                        4096.0,
                        "#fff",
                        "#000",
                        1.3,
                        "0 0",
                        &meter,
                        &randomness,
                    )
                    .map(|_| ()),
                    1 => {
                        for _ in 0..8 {
                            if class_rough_hand_drawn_line_path(
                                0.0,
                                0.0,
                                40.0,
                                20.0,
                                "#000",
                                1.3,
                                "0 0",
                                &meter,
                                &randomness,
                            )
                            .is_none()
                            {
                                break;
                            }
                        }
                        class_rough_hand_drawn_line_path(
                            0.0,
                            0.0,
                            40.0,
                            20.0,
                            "#000",
                            1.3,
                            "0 0",
                            &meter,
                            &randomness,
                        )
                        .map(|_| ())
                    }
                    _ => class_rough_hand_drawn_stroke_path_for_svg_path(
                        "M0 0 C0 40 60 40 60 0 Z M80 0 L100 0 Z",
                        0.3,
                        &meter,
                        &randomness,
                    )
                    .map(|_| ()),
                };
                assert!(result.is_none());
                let error = meter.checkpoint(OperationPhase::Emit).unwrap_err();
                if cancelled {
                    assert!(
                        matches!(error, OperationWorkError::Cancelled(error) if error.phase == OperationPhase::Emit)
                    );
                    assert_eq!(meter.used(), 0);
                } else {
                    assert!(
                        matches!(error, OperationWorkError::ResourceLimitExceeded(error)
                        if error.limit == ResourceLimitId::MaxLayoutWorkUnits.as_str()
                            && error.cause == ResourceLimitCause::Ceiling)
                    );
                    assert!(meter.used() <= 32);
                }
            }
        }
    }
}
