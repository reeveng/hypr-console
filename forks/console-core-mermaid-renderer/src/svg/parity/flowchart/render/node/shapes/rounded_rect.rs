//! Flowchart v2 rounded rectangle shape.

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::{fmt, fmt_display};

use super::super::roughjs::roughjs_hachure_paths_for_svg_path;

const FLOWCHART_NODE_HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const FLOWCHART_NODE_HAND_DRAWN_FILL_WEIGHT: f32 = 1.5;
const FLOWCHART_NODE_HAND_DRAWN_HACHURE_GAP: f32 = 1.5;

pub(in crate::svg::parity::flowchart::render::node) fn render_rounded_rect(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) -> super::super::emission::FlowchartNodeShapeEmissionReceipt {
    let w = common.layout_node.width.max(1.0);
    let h = common.layout_node.height.max(1.0);


    let radius = ctx
        .compatibility
        .rounded_rect_radius_truthy
        .then_some(&ctx.compatibility.rounded_rect_radius);

    if common.look_is_hand_drawn() && radius.is_none() {

        return super::render_process_rectangle(out, common, details);
    }

    let rough_paths = if common.look_is_hand_drawn() {

        let path_data = rounded_rect_path_data(self::RoundedRectPathDataArguments { width: w, height: h }, &ctx.compatibility.rounded_rect_radius);
        super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_hachure_paths_for_svg_path(
                &path_data,
                common.stroke_width,
                common.stroke_dasharray,
                FLOWCHART_NODE_HAND_DRAWN_FILL_WEIGHT,
                FLOWCHART_NODE_HAND_DRAWN_HACHURE_GAP,
                FLOWCHART_NODE_HAND_DRAWN_ROUGHNESS,
                common.work_meter,
                common.hand_drawn_seed,
            )
        })
    } else {
        None
    };

    match rough_paths {
        Some((fill_d, stroke_d)) => {
            let _ = write!(
                out,
                r#"<g class="basic label-container" style="{}">"#,
                escape_attr(common.rough_group_style)
            );
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"/>"#,
                escape_attr(&fill_d),
                escape_attr(common.fill_color),
                fmt_display(<f64 as ::core::convert::From<::core::primitive::f32>>::from(FLOWCHART_NODE_HAND_DRAWN_FILL_WEIGHT)),
            );
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
                escape_attr(&stroke_d),
                escape_attr(common.stroke_color),
                common.stroke_width,
                escape_attr(common.stroke_dasharray),
            );
            out.push_str("</g>");
            super::super::emission::FlowchartNodeShapeEmissionReceipt::unverified()
        },
        ::core::option::Option::None => {
            let numeric_radius = common
                .typed_corner_radius
                .or(ctx.compatibility.rounded_rect_radius_numeric);
            let _ = write!(
                out,
                r#"<rect class="basic label-container"{} style="{}"#,
                common.effect_filter_attr,
                escape_attr(common.style),
            );
            common.write_effect_style(out);
            common.write_rectangle_corner_style(out);
            let _ = write!(
                out,
                r#"" x="{}" y="{}" width="{}" height="{}""#,
                fmt(-w / 2.0),
                fmt(-h / 2.0),
                fmt(w),
                fmt(h),
            );

            if common.typed_corner_radius.is_some()
                || common.source_corner_radii.iter().any(Option::is_some)
            {
                common.write_rectangle_radii(out, numeric_radius);
            } else if let Some(radius_2) = radius {
                let radius_3 = radius_text(radius_2);
                let _ = write!(
                    out,
                    r#" rx="{}" ry="{}""#,
                    escape_attr(&radius_3),
                    escape_attr(&radius_3)
                );
            }

            out.push_str("/>");
            super::super::emission::FlowchartNodeShapeEmissionReceipt::classic_process(true)
                .with_effect_reach(common.effect_selected)
        },
    }
}

#[must_use]
fn radius_text(radius: &serde_json::Value) -> String {
    match radius {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Number(number) => {
            fmt_display(match number.as_f64() {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => 0.0,
            }).to_string()
        }
        _ => radius.to_string(),
    }
}

#[must_use]
fn rounded_rect_path_data(RoundedRectPathDataArguments { width, height }: RoundedRectPathDataArguments, radius: &serde_json::Value) -> String {
    let x = -width / 2.0;
    let y = -height / 2.0;
    let numeric_radius = match radius {
        serde_json::Value::Bool(value) => f64::from(u8::from(*value)),
        serde_json::Value::String(value) if value.trim().is_empty() => 0.0,
        _ => match crate::config::json_f64(radius) {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => f64::NAN,
        },
    };
    let add_radius = |coordinate: f64| match radius.as_str() {
        Some(text) => format!("{}{text}", fmt_display(coordinate)),
        None => fmt_display(coordinate + numeric_radius).to_string(),
    };
    let radius_2 = radius_text(radius);
    format!(
        "M {} {} H {} A {} {} 0 0 1 {} {} V {} A {} {} 0 0 1 {} {} H {} A {} {} 0 0 1 {} {} V {} A {} {} 0 0 1 {} {} Z",
        add_radius(x),
        fmt_display(y),
        fmt_display(x + width - numeric_radius),
        radius_2,
        radius_2,
        fmt_display(x + width),
        add_radius(y),
        fmt_display(y + height - numeric_radius),
        radius_2,
        radius_2,
        fmt_display(x + width - numeric_radius),
        fmt_display(y + height),
        add_radius(x),
        radius_2,
        radius_2,
        fmt_display(x),
        fmt_display(y + height - numeric_radius),
        add_radius(y),
        radius_2,
        radius_2,
        add_radius(x),
        fmt_display(y),
    )
}

struct RoundedRectPathDataArguments {
    width: f64,
    height: f64,
}

pub(super) fn advance_rough_geometry(
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeGeometry<'_>,
) {
    if !common.look_is_hand_drawn() {
        return;
    }

    if !ctx.compatibility.rounded_rect_radius_truthy {
        return super::process::advance_rough_geometry(common);
    }

    let path = rounded_rect_path_data(
        self::RoundedRectPathDataArguments { width: common.layout_node.width.max(1.0),
        height: common.layout_node.height.max(1.0) },
        &ctx.compatibility.rounded_rect_radius,
    );
    let _ = super::super::roughjs::roughjs_prepare_hachure_paths_for_svg_path(
        super::super::roughjs::RoughGeometryTarget::Query,
        &path,
        common.stroke_width,
        common.stroke_dasharray,
        FLOWCHART_NODE_HAND_DRAWN_FILL_WEIGHT,
        FLOWCHART_NODE_HAND_DRAWN_HACHURE_GAP,
        FLOWCHART_NODE_HAND_DRAWN_ROUGHNESS,
        common.work_meter,
        common.hand_drawn_seed,
    );
}
