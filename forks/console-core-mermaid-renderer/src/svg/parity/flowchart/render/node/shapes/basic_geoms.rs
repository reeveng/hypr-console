//! Flowchart v2 basic geometry shapes.

use crate::svg::parity::flowchart::{OptionalStyleAttr, escape_attr};
use crate::svg::parity::{fmt, fmt_display};

use super::super::geom::path_from_points;
use super::super::roughjs::{
    roughjs_hachure_paths_for_svg_path, roughjs_paths_for_circle,
    roughjs_paths_for_circle_with_roughness,
};

const FLOWCHART_DIAMOND_HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const FLOWCHART_DIAMOND_HAND_DRAWN_FILL_WEIGHT: f32 = 1.5;
const FLOWCHART_DIAMOND_HAND_DRAWN_HACHURE_GAP: f32 = 1.5;

pub(in crate::svg::parity::flowchart::render::node) fn render_diamond(
    out: &mut impl crate::svg::parity::SvgOutput,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) -> super::super::emission::FlowchartNodeShapeEmissionReceipt {
    let w = common.layout_node.width.max(1.0);
    let h = common.layout_node.height.max(1.0);
    let tx = -w / 2.0 + 0.5;
    let ty = h / 2.0;

    let rough_paths = if common.look_is_hand_drawn() {



        let path_data = diamond_path(self::DiamondPathArguments { w, h });
        super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_hachure_paths_for_svg_path(
                &path_data,
                common.stroke_width,
                common.stroke_dasharray,
                FLOWCHART_DIAMOND_HAND_DRAWN_FILL_WEIGHT,
                FLOWCHART_DIAMOND_HAND_DRAWN_HACHURE_GAP,
                FLOWCHART_DIAMOND_HAND_DRAWN_ROUGHNESS,
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
                r#"<g transform="translate({},{})" style="{}"><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"#,
                fmt(tx),
                fmt(ty),
                escape_attr(common.rough_group_style),
                escape_attr(&fill_d),
                escape_attr(common.fill_color),
                fmt_display(<f64 as ::core::convert::From<::core::primitive::f32>>::from(FLOWCHART_DIAMOND_HAND_DRAWN_FILL_WEIGHT)),
                escape_attr(&stroke_d),
                escape_attr(common.stroke_color),
                fmt_display(<f64 as ::core::convert::From<::core::primitive::f32>>::from(common.stroke_width)),
                escape_attr(common.stroke_dasharray),
            );
            return super::super::emission::FlowchartNodeShapeEmissionReceipt::unverified();
        },
        ::core::option::Option::None => {},
    }

    let effect_style;
    let body_style = if common.effect_filter_style.is_empty() {
        common.style
    } else {
        effect_style = format!("{}{}", common.style, common.effect_filter_style);
        &effect_style
    };
    let _ = write!(
        out,
        r#"<polygon points="{},0 {},{} {},{} 0,{}" class="label-container" transform="translate({},{})"{}{} />"#,
        fmt(w / 2.0),
        fmt(w),
        fmt(-h / 2.0),
        fmt(w / 2.0),
        fmt(-h),
        fmt(-h / 2.0),
        fmt(tx),
        fmt(ty),
        OptionalStyleAttr(body_style),
        common.effect_filter_attr,
    );
    super::super::emission::FlowchartNodeShapeEmissionReceipt::classic_polygon()
        .with_effect_reach(common.effect_selected)
}

pub(in crate::svg::parity::flowchart::render::node) fn render_circle(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) -> crate::Result<super::super::emission::FlowchartNodeShapeEmissionReceipt> {
    let w = common.layout_node.width.max(1.0);
    let h = common.layout_node.height.max(1.0);
    let r = (w.min(h) / 2.0).max(0.5);

    if common.look_is_hand_drawn() {
        let paths = super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_circle(
                r * 2.0,
                common.stroke_width,
                common.stroke_dasharray,
                true,
                common.work_meter,
                common.hand_drawn_seed,
            )
        });
        ctx.checkpoint_emit()?;

        match paths {
            Some(paths_2) => {
                let _ = write!(
                    out,
                    r#"<g class="basic label-container" style="{}">"#,
                    escape_attr(&hand_drawn_circle_style(common.node_styles)),
                );
                write_rough_circle_paths(out, common, &paths_2, common.stroke_width);
                out.push_str("</g>");
                out.checkpoint()?;
                return Ok(super::super::emission::FlowchartNodeShapeEmissionReceipt::unverified());
            },
            ::core::option::Option::None => {},
        }
    }

    let _ = write!(
        out,
        r#"<circle class="basic label-container"{} style="{}{}" r="{}" cx="0" cy="0"/>"#,
        common.effect_filter_attr,
        escape_attr(common.style),
        common.effect_filter_style,
        fmt(r),
    );
    out.checkpoint()?;
    Ok(
        super::super::emission::FlowchartNodeShapeEmissionReceipt::unverified()
            .with_effect_reach(common.effect_selected),
    )
}

pub(in crate::svg::parity::flowchart::render::node) fn render_double_circle(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) -> crate::Result<super::super::emission::FlowchartNodeShapeEmissionReceipt> {
    let geometry = crate::flowchart::DoubleCircleGeometry::from_outer_diameter(
        common.layout_node.width.min(common.layout_node.height),
        common.look_is_neo(),
    );

    if common.look_is_hand_drawn() {

        let outer = super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_circle_with_roughness(
                geometry.outer_radius * 2.0,
                2.5,
                common.stroke_dasharray,
                true,
                0.2,
                common.work_meter,
                common.hand_drawn_seed,
            )
        });
        ctx.checkpoint_emit()?;
        let inner = super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_circle_with_roughness(
                geometry.inner_radius * 2.0,
                1.5,
                common.stroke_dasharray,
                true,
                0.2,
                common.work_meter,
                common.hand_drawn_seed,
            )
        });
        ctx.checkpoint_emit()?;

        {
            match (outer, inner) {
                (Some(outer_2), Some(inner_2)) => {
                    let classes = std::iter::once("default")
                        .chain(common.node_classes.iter().map(String::as_str))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let _ = write!(
                        out,
                        r#"<g class="{}" style="{}"><g>"#,
                        escape_attr(&classes),
                        escape_attr(&hand_drawn_circle_style(common.node_styles)),
                    );
                    write_rough_circle_paths(out, common, &outer_2, 2.5);
                    out.push_str("</g><g>");
                    write_rough_circle_paths(out, common, &inner_2, 1.5);
                    out.push_str("</g></g>");
                    out.checkpoint()?;
                    return Ok(super::super::emission::FlowchartNodeShapeEmissionReceipt::unverified());
                },
                (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {},
            };
        }
    }

    let _ = write!(
        out,
        r#"<g class="basic label-container"{} style="{}{}"><circle class="outer-circle" cx="0" cy="0" r="{}" style="{}"#,
        common.effect_filter_attr,
        escape_attr(common.style),
        common.effect_filter_style,
        fmt(geometry.outer_radius),
        escape_attr(common.style),
    );
    common.write_child_filter_style(out);
    let _ = write!(
        out,
        r#""/><circle class="inner-circle" cx="0" cy="0" r="{}" style="{}"#,
        fmt(geometry.inner_radius),
        escape_attr(common.style)
    );
    common.write_child_filter_style(out);
    out.push_str(r#""/></g>"#);
    out.checkpoint()?;
    Ok(
        super::super::emission::FlowchartNodeShapeEmissionReceipt::unverified()
            .with_effect_reach(common.effect_selected),
    )
}

#[must_use]
fn hand_drawn_circle_style(styles: &[String]) -> String {


    styles
        .iter()
        .flat_map(|raw| crate::flowchart::flowchart_split_mermaid_style_decls(raw))
        .filter_map(|decl| {
            crate::mermaid_style::parse_safe_style_decl(decl)
                .map(|(key, value)| format!("{key}:{value}"))
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn write_rough_circle_paths(
    out: &mut impl crate::svg::parity::SvgOutput,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    paths: &(String, String),
    stroke_width: f32,
) {
    let _ = write!(
        out,
        r#"<path d="{}" stroke="{}" stroke-width="1.5" fill="none" stroke-dasharray="0 0"/>"#,
        escape_attr(&paths.0),
        escape_attr(common.fill_color),
    );


    if common.stroke_color != "none" {
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
            escape_attr(&paths.1),
            escape_attr(common.stroke_color),
            fmt_display(<f64 as ::core::convert::From<::core::primitive::f32>>::from(stroke_width)),
            escape_attr(common.stroke_dasharray),
        );
    }
}

#[must_use]
fn diamond_path(DiamondPathArguments { w, h }: DiamondPathArguments) -> String {
    let rough_points = [
        (0.0, -h / 2.0),
        (w / 2.0, 0.0),
        (w, -h / 2.0),
        (w / 2.0, -h),
    ];
    path_from_points(&rough_points)
}

struct DiamondPathArguments {
    w: f64,
    h: f64,
}

pub(super) fn advance_rough_geometry(
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeGeometry<'_>,
) -> crate::Result<()> {
    use super::super::roughjs::{
        RoughGeometryTarget, roughjs_prepare_hachure_paths_for_svg_path,
        roughjs_prepare_paths_for_circle_with_roughness,
    };
    use crate::flowchart::FlowchartShape;

    if !common.look_is_hand_drawn() {
        return Ok(());
    }

    let checked_value = FlowchartShape::resolve(common.shape)?;

    match checked_value {
        FlowchartShape::Diamond => {
            let path = diamond_path(
                self::DiamondPathArguments { w: common.layout_node.width.max(1.0),
                h: common.layout_node.height.max(1.0) },
            );
            let _ = roughjs_prepare_hachure_paths_for_svg_path(
                RoughGeometryTarget::Query,
                &path,
                common.stroke_width,
                common.stroke_dasharray,
                FLOWCHART_DIAMOND_HAND_DRAWN_FILL_WEIGHT,
                FLOWCHART_DIAMOND_HAND_DRAWN_HACHURE_GAP,
                FLOWCHART_DIAMOND_HAND_DRAWN_ROUGHNESS,
                common.work_meter,
                common.hand_drawn_seed,
            );
        }
        FlowchartShape::Circle => {
            let diameter = common
                .layout_node
                .width
                .max(1.0)
                .min(common.layout_node.height.max(1.0))
                .max(1.0);
            let _ = roughjs_prepare_paths_for_circle_with_roughness(
                RoughGeometryTarget::Query,
                diameter,
                common.stroke_width,
                common.stroke_dasharray,
                true,
                0.7,
                common.work_meter,
                common.hand_drawn_seed,
            );
        }
        FlowchartShape::DoubleCircle => {
            let geometry = crate::flowchart::DoubleCircleGeometry::from_outer_diameter(
                common.layout_node.width.min(common.layout_node.height),
                common.look_is_neo(),
            );
            let _ = roughjs_prepare_paths_for_circle_with_roughness(
                RoughGeometryTarget::Query,
                geometry.outer_radius * 2.0,
                2.5,
                common.stroke_dasharray,
                true,
                0.2,
                common.work_meter,
                common.hand_drawn_seed,
            );
            ctx.work_meter
                .checkpoint(merman_model::OperationPhase::Layout)
                .map_err(crate::Error::from)?;
            let _ = roughjs_prepare_paths_for_circle_with_roughness(
                RoughGeometryTarget::Query,
                geometry.inner_radius * 2.0,
                1.5,
                common.stroke_dasharray,
                true,
                0.2,
                common.work_meter,
                common.hand_drawn_seed,
            );
            ctx.work_meter
                .checkpoint(merman_model::OperationPhase::Layout)
                .map_err(crate::Error::from)?;
        }
        _ => {}
    }

    Ok(())
}
