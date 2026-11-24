use super::*;

#[derive(Debug, Clone, Copy)]
struct StateEdgeBoundaryNode {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[must_use]
fn state_edge_outside_node(
    node: &StateEdgeBoundaryNode,
    point: &crate::model::LayoutPoint,
) -> bool {
    let dx = (point.x - node.x).abs();
    let dy = (point.y - node.y).abs();
    let w = node.width / 2.0;
    let h = node.height / 2.0;
    dx >= w || dy >= h
}

#[must_use]
fn state_edge_rect_intersection(
    node: &StateEdgeBoundaryNode,
    inside_point: &crate::model::LayoutPoint,
    outside_point: &crate::model::LayoutPoint,
) -> crate::model::LayoutPoint {
    let x = node.x;
    let y = node.y;
    let w = node.width / 2.0;
    let h = node.height / 2.0;

    let q_abs = (outside_point.y - inside_point.y).abs();
    let r_abs = (outside_point.x - inside_point.x).abs();

    if (y - outside_point.y).abs() * w > (x - outside_point.x).abs() * h {
        let q = if inside_point.y < outside_point.y {
            outside_point.y - h - y
        } else {
            y - h - outside_point.y
        };
        let r = if ::core::matches!(::core::primitive::f64::classify(q_abs), ::core::num::FpCategory::Zero) {
            0.0
        } else {
            (r_abs * q) / q_abs
        };
        let mut res = crate::model::LayoutPoint {
            x: if inside_point.x < outside_point.x {
                inside_point.x + r
            } else {
                inside_point.x - r_abs + r
            },
            y: if inside_point.y < outside_point.y {
                inside_point.y + q_abs - q
            } else {
                inside_point.y - q_abs + q
            },
        };

        if r.abs() <= 1e-9 {
            res.x = outside_point.x;
            res.y = outside_point.y;
        }

        if ::core::matches!(::core::primitive::f64::classify(r_abs), ::core::num::FpCategory::Zero) {
            res.x = outside_point.x;
        }

        if ::core::matches!(::core::primitive::f64::classify(q_abs), ::core::num::FpCategory::Zero) {
            res.y = outside_point.y;
        }

        return res;
    }

    let r = if inside_point.x < outside_point.x {
        outside_point.x - w - x
    } else {
        x - w - outside_point.x
    };
    let q = if ::core::matches!(::core::primitive::f64::classify(r_abs), ::core::num::FpCategory::Zero) {
        0.0
    } else {
        (q_abs * r) / r_abs
    };
    let mut ix = if inside_point.x < outside_point.x {
        inside_point.x + r_abs - r
    } else {
        inside_point.x - r_abs + r
    };
    let mut iy = if inside_point.y < outside_point.y {
        inside_point.y + q
    } else {
        inside_point.y - q
    };

    if r.abs() <= 1e-9 {
        ix = outside_point.x;
        iy = outside_point.y;
    }

    if ::core::matches!(::core::primitive::f64::classify(r_abs), ::core::num::FpCategory::Zero) {
        ix = outside_point.x;
    }

    if ::core::matches!(::core::primitive::f64::classify(q_abs), ::core::num::FpCategory::Zero) {
        iy = outside_point.y;
    }

    crate::model::LayoutPoint { x: ix, y: iy }
}

#[must_use]
fn state_edge_cut_path_at_intersect(
    input: &[crate::model::LayoutPoint],
    boundary: &StateEdgeBoundaryNode,
) -> Vec<crate::model::LayoutPoint> {
    match input {
        [] => {
            return Vec::new();
        },
        [_, ..] => {},
    }

    let mut out: Vec<crate::model::LayoutPoint> = Vec::new();
    let mut last_point_outside = input[0].clone();
    let mut is_inside = false;
    const EPS: f64 = 1e-9;

    for point in input {
        if !state_edge_outside_node(boundary, point) && !is_inside {


            let inter = state_edge_rect_intersection(boundary, point, &last_point_outside);

            if !out
                .iter()
                .any(|p| (p.x - inter.x).abs() <= EPS && (p.y - inter.y).abs() <= EPS)
            {
                out.push(inter);
            }

            is_inside = true;
        } else {
            last_point_outside = point.clone();

            if !is_inside {
                out.push(point.clone());
            }
        }
    }

    out
}

#[must_use]
fn state_edge_boundary_for_cluster(
    ctx: &StateRenderCtx<'_>,
    cluster_id: &str,
    ox: f64,
    oy: f64,
) -> Option<StateEdgeBoundaryNode> {
    let mut resolved = cluster_id;

    if !ctx.layout_clusters_by_id.contains_key(resolved) {


        if let Some(rest) = resolved.strip_prefix("state-")
            && let Some((base, suffix)) = rest.rsplit_once('-')
            && !base.is_empty()
            && !suffix.is_empty()
            && suffix.bytes().all(|b| b.is_ascii_digit())
        {
            resolved = base;
        }
    }

    let n = ctx.layout_clusters_by_id.get(resolved).copied()?;
    Some(StateEdgeBoundaryNode {
        x: n.x - ox,
        y: n.y - oy,
        width: n.width,
        height: n.height,
    })
}

#[must_use]
fn state_edge_boundary_for_layout_node(
    ctx: &StateRenderCtx<'_>,
    node_id: &str,
    ox: f64,
    oy: f64,
) -> Option<StateEdgeBoundaryNode> {
    let n = ctx.layout_nodes_by_id.get(node_id).copied()?;
    Some(StateEdgeBoundaryNode {
        x: n.x - ox,
        y: n.y - oy,
        width: n.width,
        height: n.height,
    })
}

#[must_use]
fn state_edge_clip_self_loop_points_to_node(
    ctx: &StateRenderCtx<'_>,
    le: &crate::model::LayoutEdge,
    input: &[crate::model::LayoutPoint],
    origin_x: f64,
    origin_y: f64,
) -> Option<Vec<crate::model::LayoutPoint>> {
    if le.from != le.to || input.len() < 3 {
        return None;
    }





    if le.from_cluster.is_none()
        && le.to_cluster.is_none()
        && (ctx.layout_clusters_by_id.contains_key(le.from.as_str())
            || ctx
                .nodes_by_id
                .get(le.from.as_str())
                .copied()
                .is_some_and(|node| node.is_group && node.shape != "noteGroup"))
    {
        return None;
    }

    let boundary = state_edge_boundary_for_layout_node(ctx, le.from.as_str(), origin_x, origin_y)?;
    let center = crate::model::LayoutPoint {
        x: boundary.x,
        y: boundary.y,
    };
    let inner = &input[1..input.len() - 1];

    match inner {
        [] => {
            return None;
        },
        [_, ..] => {},
    }

    let mut out = Vec::with_capacity(inner.len() + 2);
    out.push(state_edge_rect_intersection(&boundary, &center, &inner[0]));
    out.extend(inner.iter().cloned());
    out.push(state_edge_rect_intersection(
        &boundary,
        &center,
        &inner[inner.len() - 1],
    ));
    Some(out)
}

#[must_use]
fn state_edge_find_adjacent_point(
    point_a: &crate::model::LayoutPoint,
    point_b: &crate::model::LayoutPoint,
    distance: f64,
) -> crate::model::LayoutPoint {
    let x_diff = point_b.x - point_a.x;
    let y_diff = point_b.y - point_a.y;
    let length = (x_diff * x_diff + y_diff * y_diff).sqrt();
    let ratio = distance / length;
    crate::model::LayoutPoint {
        x: point_b.x - ratio * x_diff,
        y: point_b.y - ratio * y_diff,
    }
}

#[must_use]
fn state_edge_is_corner_point(
    prev: &crate::model::LayoutPoint,
    curr: &crate::model::LayoutPoint,
    next: &crate::model::LayoutPoint,
) -> bool {
    (<::core::primitive::f64 as ::core::cmp::PartialOrd>::partial_cmp(&prev.x, &curr.x) == ::core::option::Option::Some(::core::cmp::Ordering::Equal)
        && <::core::primitive::f64 as ::core::cmp::PartialOrd>::partial_cmp(&curr.y, &next.y) == ::core::option::Option::Some(::core::cmp::Ordering::Equal)
        && (curr.x - next.x).abs() > 5.0
        && (curr.y - prev.y).abs() > 5.0)
        || (<::core::primitive::f64 as ::core::cmp::PartialOrd>::partial_cmp(&prev.y, &curr.y) == ::core::option::Option::Some(::core::cmp::Ordering::Equal)
            && <::core::primitive::f64 as ::core::cmp::PartialOrd>::partial_cmp(&curr.x, &next.x) == ::core::option::Option::Some(::core::cmp::Ordering::Equal)
            && (curr.x - prev.x).abs() > 5.0
            && (curr.y - next.y).abs() > 5.0)
}

#[must_use]
fn state_edge_fix_corners(
    line_data: &[crate::model::LayoutPoint],
) -> Vec<crate::model::LayoutPoint> {
    if line_data.len() < 3 {
        return line_data.to_vec();
    }

    let mut out = Vec::with_capacity(line_data.len());

    for (idx, point) in line_data.iter().enumerate() {
        let is_corner = idx > 0
            && idx + 1 < line_data.len()
            && state_edge_is_corner_point(&line_data[idx - 1], point, &line_data[idx + 1]);

        if !is_corner {
            out.push(point.clone());
            continue;
        }

        let prev_point = &line_data[idx - 1];
        let next_point = &line_data[idx + 1];
        let corner_point = point;
        let new_prev_point = state_edge_find_adjacent_point(prev_point, corner_point, 5.0);
        let new_next_point = state_edge_find_adjacent_point(next_point, corner_point, 5.0);
        let x_diff = new_next_point.x - new_prev_point.x;
        let y_diff = new_next_point.y - new_prev_point.y;
        out.push(new_prev_point.clone());

        let mut new_corner_point = corner_point.clone();

        if (next_point.x - prev_point.x).abs() > 10.0 && (next_point.y - prev_point.y).abs() >= 10.0
        {
            let a = std::f64::consts::SQRT_2 * 2.0;
            let r = 5.0;

            if <::core::primitive::f64 as ::core::cmp::PartialOrd>::partial_cmp(&corner_point.x, &new_prev_point.x) == ::core::option::Option::Some(::core::cmp::Ordering::Equal) {
                new_corner_point = crate::model::LayoutPoint {
                    x: if x_diff < 0.0 {
                        new_prev_point.x - r + a
                    } else {
                        new_prev_point.x + r - a
                    },
                    y: if y_diff < 0.0 {
                        new_prev_point.y - a
                    } else {
                        new_prev_point.y + a
                    },
                };
            } else {
                new_corner_point = crate::model::LayoutPoint {
                    x: if x_diff < 0.0 {
                        new_prev_point.x - a
                    } else {
                        new_prev_point.x + a
                    },
                    y: if y_diff < 0.0 {
                        new_prev_point.y - r + a
                    } else {
                        new_prev_point.y + r - a
                    },
                };
            }
        }

        out.push(new_corner_point);
        out.push(new_next_point);
    }

    out
}

#[must_use]
fn state_marker_offset_for(arrow_type_end: Option<&str>) -> Option<f64> {
    match arrow_type_end {
        Some("arrow_barb_neo") => Some(5.5),
        _ => None,
    }
}

#[must_use]
fn state_rounded_end_marker_offset_points(
    input: &[crate::model::LayoutPoint],
    arrow_type_end: Option<&str>,
) -> Vec<crate::model::LayoutPoint> {
    let offset = {
        let optional_offset = state_marker_offset_for(arrow_type_end);

        match optional_offset {
            Some(matched_offset) => matched_offset,
            ::core::option::Option::None => return input.to_vec(),
        }
    };

    if input.len() < 2 {
        return input.to_vec();
    }




    let mut out = input.to_vec();
    let end = &input[input.len() - 1];
    let previous = &input[input.len() - 2];
    let angle = (end.y - previous.y).atan2(end.x - previous.x);
    out[input.len() - 1] = crate::model::LayoutPoint {
        x: end.x - offset * angle.cos(),
        y: end.y - offset * angle.sin(),
    };
    out
}

#[must_use]
fn state_line_with_end_marker_offset_points(
    input: &[crate::model::LayoutPoint],
    arrow_type_end: Option<&str>,
) -> Vec<crate::model::LayoutPoint> {
    #[must_use]
    fn calculate_delta_and_angle(
        a: &crate::model::LayoutPoint,
        b: &crate::model::LayoutPoint,
    ) -> (f64, f64, f64) {
        let delta_x = b.x - a.x;
        let delta_y = b.y - a.y;
        let angle = (delta_y / delta_x).atan();
        (angle, delta_x, delta_y)
    }

    let end_marker_height = {
        let optional_end_marker_height = state_marker_offset_for(arrow_type_end);

        match optional_end_marker_height {
            Some(matched_end_marker_height) => matched_end_marker_height,
            ::core::option::Option::None => return input.to_vec(),
        }
    };

    if input.len() < 2 {
        return input.to_vec();
    }

    let start = &input[0];
    let end = &input[input.len() - 1];
    let x_direction_is_left = start.x < end.x;
    let y_direction_is_down = start.y < end.y;
    let extra_room = 1.0;

    let mut out = Vec::with_capacity(input.len());

    for (idx, point) in input.iter().enumerate() {
        let mut offset_x = 0.0;
        let mut offset_y = 0.0;

        if idx == input.len() - 1 {
            let (angle, delta_x, delta_y) =
                calculate_delta_and_angle(&input[input.len() - 1], &input[input.len() - 2]);
            offset_x = end_marker_height * angle.cos() * if delta_x >= 0.0 { 1.0 } else { -1.0 };
            offset_y =
                end_marker_height * angle.sin().abs() * if delta_y >= 0.0 { 1.0 } else { -1.0 };
        }

        let diff_x = (point.x - end.x).abs();
        let diff_y = (point.y - end.y).abs();

        if diff_x < end_marker_height && diff_x > 0.0 && diff_y < end_marker_height {
            let mut adjustment = end_marker_height + extra_room - diff_x;
            adjustment *= if !x_direction_is_left { -1.0 } else { 1.0 };
            offset_x -= adjustment;
        }

        if diff_y < end_marker_height && diff_y > 0.0 && diff_x < end_marker_height {
            let mut adjustment = end_marker_height + extra_room - diff_y;
            adjustment *= if !y_direction_is_down { -1.0 } else { 1.0 };
            offset_y -= adjustment;
        }

        out.push(crate::model::LayoutPoint {
            x: point.x + offset_x,
            y: point.y + offset_y,
        });
    }

    out
}

pub(super) struct StatePreparedEdgeGeometry {
    original_label_path_points: Vec<crate::model::LayoutPoint>,
    pub(super) data_points: Vec<crate::model::LayoutPoint>,
    label_path_points: Vec<crate::model::LayoutPoint>,
    rendered_d: String,
    points_were_explicitly_updated: bool,
}

#[must_use]
fn state_edge_finish_geometry(
    original_label_path_points: Vec<crate::model::LayoutPoint>,
    data_points: Vec<crate::model::LayoutPoint>,
    label_path_points: Vec<crate::model::LayoutPoint>,
    arrow_type_end: Option<&str>,
    points_were_explicitly_updated: bool,
) -> StatePreparedEdgeGeometry {


    let mut points_for_curve = label_path_points
        .iter()
        .filter(|point| !point.y.is_nan())
        .cloned()
        .collect::<Vec<_>>();
    points_for_curve = state_edge_fix_corners(&points_for_curve);
    points_for_curve = state_line_with_end_marker_offset_points(&points_for_curve, arrow_type_end);

    StatePreparedEdgeGeometry {
        original_label_path_points,
        data_points,
        label_path_points,
        rendered_d: curve_basis_path_d(&points_for_curve),
        points_were_explicitly_updated,
    }
}

#[must_use]
pub(super) fn state_edge_prepare_geometry(
    ctx: &StateRenderCtx<'_>,
    le: &crate::model::LayoutEdge,
    arrow_type_end: Option<&str>,
    origin_x: f64,
    origin_y: f64,
) -> StatePreparedEdgeGeometry {
    if ctx.uses_elk_adapter_dom {
        let points = ctx
            .elk_edge_paths
            .get(&le.id)
            .map(Vec::as_slice)
            .unwrap_or(&le.points);
        let data_points: Vec<_> = points
            .iter()
            .map(|point| crate::model::LayoutPoint {
                x: point.x - origin_x,
                y: point.y - origin_y,
            })
            .collect();
        let curve = if le.points.is_empty() {
            "linear"
        } else {
            "rounded"
        };
        let mut curve_points = data_points.clone();
        let marker_end = if curve == "rounded" {
            curve_points = state_rounded_end_marker_offset_points(&curve_points, arrow_type_end);
            None
        } else {
            arrow_type_end
        };
        let rendered_d =
            super::super::edge_path::render_path(&mut curve_points, curve, None, marker_end);
        return StatePreparedEdgeGeometry {
            original_label_path_points: data_points.clone(),
            label_path_points: data_points.clone(),
            data_points,
            rendered_d,
            points_were_explicitly_updated: false,
        };
    }

    let mut raw_local_points: Vec<crate::model::LayoutPoint> = Vec::new();

    for p in &le.points {
        raw_local_points.push(crate::model::LayoutPoint {
            x: p.x - origin_x,
            y: p.y - origin_y,
        });
    }



    let data_points =
        state_edge_clip_self_loop_points_to_node(ctx, le, &raw_local_points, origin_x, origin_y)
            .unwrap_or_else(|| raw_local_points.clone());
    let mut label_path_points = data_points.clone();
    let mut points_were_explicitly_updated = false;



    if let Some(tc) = le.to_cluster.as_deref()
        && let Some(boundary) = state_edge_boundary_for_cluster(ctx, tc, origin_x, origin_y)
    {
        label_path_points = state_edge_cut_path_at_intersect(&raw_local_points, &boundary);
        points_were_explicitly_updated = true;
    }

    if let Some(fc) = le.from_cluster.as_deref()
        && let Some(boundary) = state_edge_boundary_for_cluster(ctx, fc, origin_x, origin_y)
    {
        let mut rev = label_path_points;
        rev.reverse();
        rev = state_edge_cut_path_at_intersect(&rev, &boundary);
        rev.reverse();
        label_path_points = rev;
        points_were_explicitly_updated = true;
    }

    state_edge_finish_geometry(
        raw_local_points,
        data_points,
        label_path_points,
        arrow_type_end,
        points_were_explicitly_updated,
    )
}

#[allow(clippy::too_many_arguments)]
fn write_state_edge_path(
    out: &mut impl SvgOutput,
    ctx: &StateRenderCtx<'_>,
    le: &crate::model::LayoutEdge,
    edge_id: &str,
    classes: &str,
    marker_end: Option<&str>,
    arrow_type_end: Option<&str>,
    origin_x: f64,
    origin_y: f64,
    work_meter: &crate::resources::OperationWorkMeter,
) -> Result<()> {
    if !ctx.uses_elk_adapter_dom && le.points.len() < 2 {
        return Ok(());
    }

    let geometry = state_edge_prepare_geometry(ctx, le, arrow_type_end, origin_x, origin_y);
    let hopped_d = ctx.elk_line_hop_paths.get(edge_id).map(String::as_str);
    let rendered_d = hopped_d.unwrap_or(geometry.rendered_d.as_str());
    let data_points = base64::engine::general_purpose::STANDARD
        .encode(serde_json::to_vec(&geometry.data_points).unwrap_or_default());
    let mut original_style = String::new();

    if state_data_look(ctx) == "neo"
        && let Some(length) = super::super::svg_path_length_from_d(&geometry.rendered_d)
    {
        super::super::edge_path::write_neo_edge_mask(
            &mut original_style,
            length,
            None,
            arrow_type_end,
            classes
                .split_whitespace()
                .any(|class| class == "edge-pattern-dashed"),
            false,
        );
    }

    original_style.push_str("fill:none;;;fill:none");

    if let Some(style) = ctx
        .style_plan
        .edge(edge_id)
        .map(crate::state::StateEdgeStylePlan::path_style_attr)
        .filter(|style| !style.is_empty())
    {
        original_style.push(';');
        original_style.push_str(style);
    }

    let style = if let Some(path) = hopped_d {
        super::super::line_hops::rewrite_style_after_line_hop(&original_style, path, work_meter)?
    } else {
        std::borrow::Cow::Borrowed(original_style.as_str())
    };
    let _ = write!(
        out,
        r#"<path d="{}" id="{}" class="{}" style="{}" data-edge="true" data-et="edge" data-id="{}" data-points="{}" data-look="{}""#,
        rendered_d,
        state_scoped_dom_id(ctx, edge_id).attr(),
        escape_xml_display(classes),
        escape_xml_display(&style),
        escape_xml_display(edge_id),
        data_points,
        escape_xml_display(state_data_look(ctx))
    );

    if let Some(marker_end_2) = marker_end {
        let _ = write!(out, r#" marker-end="{}""#, escape_xml_display(marker_end_2));
    }

    out.push_str("/>");
    out.checkpoint()?;
    Ok(())
}

pub(super) fn render_state_edge_path(
    out: &mut impl SvgOutput,
    ctx: &StateRenderCtx<'_>,
    edge: &StateSvgEdge,
    origin_x: f64,
    origin_y: f64,
    work_meter: &crate::resources::OperationWorkMeter,
) -> Result<()> {
    let mut classes = if ctx.uses_elk_adapter_dom
        && edge
            .classes
            .split_whitespace()
            .any(|class| class == "note-edge")
    {
        "edge-thickness-normal edge-pattern-dashed".to_string()
    } else {
        "edge-thickness-normal edge-pattern-solid".to_string()
    };

    for c in edge.classes.split_whitespace() {
        if c.trim().is_empty() {
            continue;
        }

        classes.push(' ');
        classes.push_str(c.trim());
    }

    let marker_end = match edge.arrow_type_end.trim() {
        "arrow_barb" | "arrow_barb_neo" => {
            let suffix = if state_data_look(ctx) == "neo" {
                "-margin"
            } else {
                ""
            };
            ctx.style_plan
                .edge(edge.id.as_str())
                .filter(|style| !style.marker_style_attr().is_empty())
                .and_then(crate::state::StateEdgeStylePlan::marker_ordinal)
                .map(|ordinal| {
                    format!(
                        "url(#{})",
                        super::state_transition_marker_id(ctx.diagram_id, ordinal)
                    )
                })
                .or_else(|| {
                    Some(format!(
                        "url(#{}_stateDiagram-barbEnd{suffix})",
                        ctx.diagram_id
                    ))
                })
        }
        _ => None,
    };

    let le = {
        let optional_le = ctx.layout_edges_by_id.get(edge.id.as_str()).copied();

        match optional_le {
            Some(matched_le) => matched_le,
            ::core::option::Option::None => return Ok(()),
        }
    };

    write_state_edge_path(
        out,
        ctx,
        le,
        edge.id.as_str(),
        &classes,
        marker_end.as_deref(),
        Some(edge.arrow_type_end.as_str()),
        origin_x,
        origin_y,
        work_meter,
    )
}

pub(super) fn render_state_edge_label(
    out: &mut impl SvgOutput,
    ctx: &StateRenderCtx<'_>,
    edge: &StateSvgEdge,
    origin_x: f64,
    origin_y: f64,
) {
    #[must_use]
    fn fallback_background_metadata_attr(id: &str, fill: Option<&str>) -> String {
        let fill = match fill {
            Some(matched_fill) => matched_fill,
            ::core::option::Option::None => return String::new(),
        };

        format!(
            r#" {}="state-transition-label-background:{}" {}="{}""#,
            crate::svg::fallback::FALLBACK_OCCURRENCE_DATA_ATTR,
            escape_attr(id),
            crate::svg::fallback::FALLBACK_BACKGROUND_FILL_DATA_ATTR,
            escape_attr(fill),
        )
    }

    #[must_use]
    fn edge_label_div_style(label_w: f64, prefix: &str, background: &str) -> String {


        let max_width = crate::text::MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX;

        if label_w >= max_width - 1e-3 {
            format!(
                "{}{}display: table; white-space: break-spaces; line-height: 1.5; max-width: {}px; text-align: center; width: {}px;",
                prefix,
                background,
                fmt_display(max_width),
                fmt_display(max_width),
            )
        } else {
            format!(
                "{}{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;",
                prefix,
                background,
                fmt_display(max_width),
            )
        }
    }

    fn write_empty_edge_label(
        out: &mut impl SvgOutput,
        id: &str,
        html_labels: bool,
        html_style: &str,
        fallback_background_metadata_attr: &str,
    ) {
        if html_labels {
            let _ = write!(
                out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"><foreignObject{} width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
                escape_attr(id),
                fallback_background_metadata_attr,
                escape_attr(html_style)
            );
        } else {
            let _ = write!(
                out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"></g></g>"#,
                escape_attr(id)
            );
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "The SVG writer takes geometry, resolved styles, and terminal evidence separately."
    )]
    fn write_visible_edge_label(
        out: &mut impl SvgOutput,
        id: &str,
        label_text: &str,
        measured_geometry: Option<&crate::state::StateNativeLabelGeometry>,
        label_pos: crate::model::LayoutPoint,
        w: f64,
        h: f64,
        html_labels: bool,
        label_style: &str,
        label_div_prefix: &str,
        html_background_style: &str,
        fallback_background_fill: Option<&str>,
        svg_background_style: &str,
    ) {
        let w = w.max(0.0);
        let h = h.max(0.0);
        let measured_geometry = (!html_labels).then_some(measured_geometry).flatten();
        let (w, h) = if let Some(geometry) = measured_geometry {
            debug_assert!((geometry.width_px() - w).abs() <= 1e-6);
            debug_assert!((geometry.height_px() - h).abs() <= 1e-6);
            (geometry.width_px(), geometry.height_px())
        } else {
            (w, h)
        };

        if html_labels {
            let fallback_background_metadata_attr =
                fallback_background_metadata_attr(id, fallback_background_fill);
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><foreignObject{} width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel">{}</span></div></foreignObject></g></g>"#,
                fmt_display(label_pos.x),
                fmt_display(label_pos.y),
                escape_attr(id),
                fmt_display(-w / 2.0),
                fmt_display(-h / 2.0),
                fallback_background_metadata_attr,
                fmt_display(w),
                fmt_display(h),
                escape_attr(&edge_label_div_style(
                    w,
                    label_div_prefix,
                    html_background_style,
                )),
                state_edge_label_html(label_text)
            );
        } else {
            let (label_dom, text_origin_y) = if let Some(geometry) = measured_geometry {
                (
                    state_measured_svg_text_label(
                        geometry,
                        true,
                        (!label_style.is_empty()).then_some(label_style),
                    ),
                    0.0,
                )
            } else {
                (
                    state_svg_text_label(
                        label_text,
                        true,
                        (!label_style.is_empty()).then_some(label_style),
                    ),
                    h / 2.0,
                )
            };
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><g><rect class="background" style="stroke: none;{}" x="0" y="0" width="{}" height="{}"/><g transform="translate({}, {})">{}</g></g></g></g>"#,
                fmt_display(label_pos.x),
                fmt_display(label_pos.y),
                escape_attr(id),
                fmt_display(-w / 2.0),
                fmt_display(-h / 2.0),
                escape_attr(svg_background_style),
                fmt_display(w),
                fmt_display(h),
                fmt_display(w / 2.0),
                fmt_display(text_origin_y),
                label_dom
            );
        }
    }

    let edge_style = ctx.style_plan.edge(edge.id.as_str());
    let label_style = match edge_style
        .map(crate::state::StateEdgeStylePlan::label_style_attr) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => "",
    };
    let label_div_prefix = match edge_style
        .map(crate::state::StateEdgeStylePlan::label_div_style_prefix) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => "",
    };
    let html_background_style = edge_style
        .map(crate::state::StateEdgeStylePlan::label_background_div_style_prefix)
        .filter(|style| !style.is_empty())
        .unwrap_or_else(|| {
            ctx.style_plan
                .transition_label_background_div_style_prefix()
        });
    let fallback_background_fill = edge_style
        .and_then(crate::state::StateEdgeStylePlan::label_background_fallback_fill)
        .or_else(|| ctx.style_plan.transition_label_background_fallback_fill());
    let svg_background_style = edge_style
        .map(crate::state::StateEdgeStylePlan::label_background_style_attr)
        .filter(|style| !style.is_empty())
        .unwrap_or_else(|| ctx.style_plan.transition_label_background_style_attr());
    let empty_edge_label_style = edge_label_div_style(0.0, label_div_prefix, html_background_style);
    let empty_fallback_background_metadata_attr =
        fallback_background_metadata_attr(&edge.id, fallback_background_fill);
    let label_text = if ctx.uses_elk_adapter_dom {
        crate::text::mermaid_html_breaks_to_newlines(&edge.label)
    } else {
        std::borrow::Cow::Borrowed(edge.label.as_str())
    };
    let label_text_2 = label_text.trim();

    if label_text_2.is_empty() {
        if ctx.uses_elk_adapter_dom {
            return;
        }

        write_empty_edge_label(
            out,
            &edge.id,
            ctx.html_labels,
            empty_edge_label_style.as_str(),
            empty_fallback_background_metadata_attr.as_str(),
        );
        return;
    }

    let le = {
        let optional_le = ctx.layout_edges_by_id.get(edge.id.as_str()).copied();

        match optional_le {
            Some(matched_le) => matched_le,
            ::core::option::Option::None => return,
        }
    };

    let lbl = {
        let optional_lbl = le.label.as_ref();

        match optional_lbl {
            Some(matched_lbl) => matched_lbl,
            ::core::option::Option::None => return,
        }
    };

    let geometry = state_edge_prepare_geometry(
        ctx,
        le,
        Some(edge.arrow_type_end.as_str()),
        origin_x,
        origin_y,
    );
    let label_position = super::super::edge_label_geometry::position_edge_label(
        crate::model::LayoutPoint {
            x: lbl.x - origin_x,
            y: lbl.y - origin_y,
        },
        Some(&geometry.original_label_path_points),
        &geometry.label_path_points,
        &geometry.rendered_d,
        geometry.points_were_explicitly_updated,
    );
    let w = lbl.width.max(0.0);
    let h = lbl.height.max(0.0);

    write_visible_edge_label(
        out,
        &edge.id,
        label_text_2,
        ctx.label_sidecar
            .measured_native_geometry(crate::state::StateLabelOwner::Edge(&edge.id)),
        label_position,
        w,
        h,
        ctx.html_labels,
        label_style,
        label_div_prefix,
        html_background_style,
        fallback_background_fill,
        svg_background_style,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_rounded_end_marker_offset_moves_only_the_terminal_point() {
        let input = vec![
            crate::model::LayoutPoint { x: 0.0, y: 0.0 },
            crate::model::LayoutPoint { x: 0.0, y: 20.0 },
            crate::model::LayoutPoint { x: 20.0, y: 20.0 },
        ];

        let output = state_rounded_end_marker_offset_points(&input, Some("arrow_barb_neo"));

        assert_eq!(output.len(), input.len());
        assert_eq!((output[0].x, output[0].y), (0.0, 0.0));
        assert_eq!((output[1].x, output[1].y), (0.0, 20.0));
        assert!((output[2].x - 14.5).abs() <= 1e-9);
        assert!((output[2].y - 20.0).abs() <= 1e-9);
    }

    #[test]
    fn state_line_with_end_marker_offset_shortens_neo_barb_terminal_point() {
        let input = vec![
            crate::model::LayoutPoint { x: 0.0, y: 0.0 },
            crate::model::LayoutPoint { x: 10.0, y: 0.0 },
        ];

        let output = state_line_with_end_marker_offset_points(&input, Some("arrow_barb_neo"));

        assert_eq!(output.len(), 2);
        assert!((output[0].x - 0.0).abs() <= 1e-9);
        assert!((output[0].y - 0.0).abs() <= 1e-9);
        assert!((output[1].x - 4.5).abs() <= 1e-9);
        assert!((output[1].y - 0.0).abs() <= 1e-9);
    }

    #[test]
    fn state_updated_path_labels_use_points_before_curve_projection() {
        let label_path_points = vec![
            crate::model::LayoutPoint { x: 0.0, y: 0.0 },
            crate::model::LayoutPoint { x: 0.0, y: 20.0 },
            crate::model::LayoutPoint { x: 20.0, y: 20.0 },
        ];
        let geometry = state_edge_finish_geometry(
            label_path_points.clone(),
            label_path_points.clone(),
            label_path_points.clone(),
            Some("arrow_barb_neo"),
            true,
        );

        assert_eq!(geometry.label_path_points.len(), label_path_points.len());
        for (actual, expected) in geometry.label_path_points.iter().zip(&label_path_points) {
            assert_eq!((actual.x, actual.y), (expected.x, expected.y));
        }
        assert_ne!(
            geometry.rendered_d,
            curve_basis_path_d(&geometry.label_path_points),
            "fixCorners and marker offsets should affect only the rendered curve"
        );
        let position = crate::svg::parity::edge_label_geometry::position_edge_label(
            crate::model::LayoutPoint { x: 99.0, y: 99.0 },
            Some(&geometry.original_label_path_points),
            &geometry.label_path_points,
            &geometry.rendered_d,
            geometry.points_were_explicitly_updated,
        );
        assert_eq!((position.x, position.y), (99.0, 99.0));
    }

    #[test]
    fn state_updated_path_labels_preserve_offsets_from_the_unclipped_route() {
        let original = vec![
            crate::model::LayoutPoint { x: -20.0, y: 0.0 },
            crate::model::LayoutPoint { x: 0.0, y: 0.0 },
            crate::model::LayoutPoint { x: 0.0, y: 20.0 },
            crate::model::LayoutPoint { x: 20.0, y: 20.0 },
        ];
        let clipped = original[1..].to_vec();
        let geometry = state_edge_finish_geometry(
            original,
            clipped.clone(),
            clipped,
            Some("arrow_barb_neo"),
            true,
        );
        let position = crate::svg::parity::edge_label_geometry::position_edge_label(
            crate::model::LayoutPoint { x: 99.0, y: 99.0 },
            Some(&geometry.original_label_path_points),
            &geometry.label_path_points,
            &geometry.rendered_d,
            geometry.points_were_explicitly_updated,
        );
        assert_eq!((position.x, position.y), (99.0, 109.0));
    }
}
