//! Flowchart root renderer.

use super::super::defs::FlowchartMarkerEmissionPlan;
use super::super::*;
use super::edge_label::render_swimlane_edge_label_node;
use crate::svg::parity::timing::RenderTiming;

#[must_use]
pub(in crate::svg::parity::flowchart) fn flowchart_elk_renders_empty_subgraph_as_cluster(
    ctx: &FlowchartRenderCtx<'_>,
) -> bool {
    ctx.uses_elk_adapter_dom
}

pub(in crate::svg::parity::flowchart) struct FlowchartRootRenderSession<
    'details,
    'cache,
    'marker,
    'data,
> {
    pub(in crate::svg::parity::flowchart) timing: RenderTiming,
    pub(in crate::svg::parity::flowchart) details: &'details mut FlowchartRenderDetails,
    pub(in crate::svg::parity::flowchart) edge_cache:
        &'cache mut FixedMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    pub(in crate::svg::parity::flowchart) hierarchy_plan: &'marker FlowchartHierarchyPlan<'data>,
    pub(in crate::svg::parity::flowchart) marker_plan: &'marker FlowchartMarkerEmissionPlan,
    pub(in crate::svg::parity::flowchart) edge_labels:
        &'cache mut super::super::paint::FlowchartEdgeLabels,
    pub(in crate::svg::parity::flowchart) step_badges: &'marker super::super::step::StepBadges,
}

struct FlowchartRootFrame<'data, 'plan> {
    cluster_id: Option<&'data str>,
    parent_origin_x: f64,
    parent_origin_y: f64,
    origin_x: f64,
    origin_y: f64,
    content_origin_y: f64,
    dom_order: &'plan [&'data str],
    next_dom_index: usize,
    initialized: bool,
    nested_start: Option<merman_model::runtime::OperationTimer>,
}

impl<'data, 'plan> FlowchartRootFrame<'data, 'plan> {
    #[must_use]
    fn new(
        cluster_id: Option<&'data str>,
        parent_origin_x: f64,
        parent_origin_y: f64,
        nested_start: Option<merman_model::runtime::OperationTimer>,
    ) -> Self {
        Self {
            cluster_id,
            parent_origin_x,
            parent_origin_y,
            origin_x: parent_origin_x,
            origin_y: parent_origin_y,
            content_origin_y: parent_origin_y,
            dom_order: &[],
            next_dom_index: 0,
            initialized: false,
            nested_start,
        }
    }
}

pub(in crate::svg::parity::flowchart) fn render_flowchart_root<'data, 'plan>(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &'data FlowchartRenderCtx<'data>,
    cluster_id: Option<&'data str>,
    parent_origin_x: f64,
    parent_origin_y: f64,
    session: &mut FlowchartRootRenderSession<'_, '_, 'plan, 'data>,
) -> crate::Result<()> {
    ctx.checkpoint_emit()?;
    let mut stack = vec![FlowchartRootFrame::new(
        cluster_id,
        parent_origin_x,
        parent_origin_y,
        None,
    )];

    while let Some(frame) = stack.pop() {
        ctx.checkpoint_emit()?;
        let mut frame_2 = Some(frame);

        if !frame_2.as_ref().is_some_and(|frame_3| frame_3.initialized)
            && let Some(frame_4) = frame_2.as_mut()
        {
            initialize_flowchart_root_frame(out, ctx, session, frame_4)?;
        }

        let mut pushed_nested = false;

        'inner_while_2: while frame_2
            .as_ref()
            .is_some_and(|frame_5| frame_5.next_dom_index < frame_5.dom_order.len())
        {
            ctx.checkpoint_emit()?;
            let id = {
                let frame_6 = {
                    let optional_frame_6 = frame_2.as_mut();

                    match optional_frame_6 {
                        Some(matched_frame_6) => matched_frame_6,
                        ::core::option::Option::None => break 'inner_while_2,
                    }
                };

                let id = frame_6.dom_order[frame_6.next_dom_index];
                frame_6.next_dom_index += 1;
                id
            };

            if let Some(edge) = ctx.swimlane_edge_label_edges_by_node_id.get(id).copied() {
                let current = {
                    let optional_current = frame_2.as_ref();

                    match optional_current {
                        Some(matched_current) => matched_current,
                        ::core::option::Option::None => break 'inner_while_2,
                    }
                };

                render_swimlane_edge_label_node(
                    out,
                    ctx,
                    id,
                    edge,
                    current.origin_x,
                    current.content_origin_y,
                    session.edge_labels,
                )?;

                ctx.checkpoint_emit()?;
                out.checkpoint()?;
                continue 'inner_while_2;
            }

            if ctx.subgraphs_by_id.contains_key(id)
                && ctx.subgraph_has_children(id)
                && !ctx.is_subgraph_collapsed(id)
            {


                if ctx.recursive_clusters.contains(id) {
                    let nested_start = session.timing.start();

                    if let Some(parent) = frame_2.take() {
                        let child = FlowchartRootFrame::new(
                            Some(id),
                            parent.origin_x,
                            parent.origin_y,
                            nested_start,
                        );
                        stack.push(parent);
                        stack.push(child);
                        pushed_nested = true;
                    }

                    break 'inner_while_2;
                }

                continue 'inner_while_2;
            }

            let node_start = session.timing.start();

            let current = {
                let optional_current = frame_2.as_ref();

                match optional_current {
                    Some(matched_current) => matched_current,
                    ::core::option::Option::None => break 'inner_while_2,
                }
            };

            render_flowchart_node(
                out,
                ctx,
                id,
                current.origin_x,
                current.content_origin_y,
                session.timing,
                &mut *session.details,
            )?;
            ctx.checkpoint_emit()?;
            out.checkpoint()?;

            if let Some(s) = node_start {
                session.details.nodes += s.elapsed();
            }
        }

        if pushed_nested {
            continue;
        }

        if let Some(frame_7) = frame_2.take() {
            out.push_str("</g>");
            session.step_badges.write_root(out, ctx, frame_7.cluster_id)?;
            out.push_str("</g>");
            out.checkpoint()?;

            if let Some(start) = frame_7.nested_start {
                session.details.nested_roots += start.elapsed();
            }
        }
    }

    Ok(())
}

#[must_use]
fn edge_label_is_empty(
    ctx: &FlowchartRenderCtx<'_>,
    edge: super::super::render_input::FlowchartRenderEdgeRef<'_>,
) -> bool {
    let label_text = match ctx
        .model
        .edge_label_for_occurrence(edge.key, edge.edge) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => "",
    };
    crate::flowchart::flowchart_label_is_empty_for_render(label_text)
}

pub(in crate::svg::parity::flowchart) fn render_flowchart_elk_root_groups(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    session: &mut FlowchartRootRenderSession<'_, '_, '_, '_>,
) -> crate::Result<()> {
    ctx.checkpoint_emit()?;
    session.details.root_calls += 1;






    out.push_str(r#"<g class="root">"#);
    render_flowchart_elk_subgraphs(out, ctx, session)?;

    let _g_edges_select = detail_guard(session.timing, &mut session.details.edges_select);
    let edges = session.hierarchy_plan.ordered_edges();
    drop(_g_edges_select);

    render_flowchart_elk_edge_paths(out, ctx, session, edges)?;
    render_flowchart_elk_edge_labels(out, ctx, session, edges)?;
    render_flowchart_elk_nodes(out, ctx, session)?;
    session.step_badges.write_root(out, ctx, None)?;
    out.push_str("</g>");
    out.checkpoint()?;
    Ok(())
}

fn render_flowchart_elk_subgraphs(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    session: &mut FlowchartRootRenderSession<'_, '_, '_, '_>,
) -> crate::Result<()> {
    ctx.checkpoint_emit()?;
    let _g_clusters = detail_guard(session.timing, &mut session.details.clusters);
    let checked_value = session.hierarchy_plan.root(None)?;
    let clusters_to_draw = checked_value.clusters();

    match clusters_to_draw {
        [] => {
            out.push_str(r#"<g class="clusters"/>"#);
            return out.checkpoint();
        },
        [_, ..] => {},
    }

    out.push_str(r#"<g class="clusters">"#);

    for &cluster in clusters_to_draw {
        ctx.checkpoint_emit()?;
        render_flowchart_cluster(out, ctx, cluster, 0.0, 0.0)?;
        out.checkpoint()?;
    }

    out.push_str("</g>");
    out.checkpoint()
}

fn render_flowchart_elk_nodes(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    session: &mut FlowchartRootRenderSession<'_, '_, '_, '_>,
) -> crate::Result<()> {
    ctx.checkpoint_emit()?;
    out.push_str(r#"<g class="nodes">"#);

    let _g_dom_order = detail_guard(session.timing, &mut session.details.dom_order);
    let checked_value = session.hierarchy_plan.root(None)?;
    let dom_order = checked_value.dom_order();
    drop(_g_dom_order);

    for &id in dom_order {
        ctx.checkpoint_emit()?;

        if ctx.subgraphs_by_id.contains_key(id)
            && !ctx.is_subgraph_collapsed(id)
            && (ctx.subgraph_has_children(id)
                || flowchart_elk_renders_empty_subgraph_as_cluster(ctx))
        {
            continue;
        }

        let node_start = session.timing.start();
        render_flowchart_node(
            out,
            ctx,
            id,
            0.0,
            0.0,
            session.timing,
            &mut *session.details,
        )?;
        ctx.checkpoint_emit()?;
        out.checkpoint()?;

        if let Some(s) = node_start {
            session.details.nodes += s.elapsed();
        }
    }

    out.push_str("</g>");
    out.checkpoint()
}

fn render_flowchart_elk_edge_paths(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    session: &mut FlowchartRootRenderSession<'_, '_, '_, '_>,
    edges: &[super::super::render_input::FlowchartRenderEdgeRef<'_>],
) -> crate::Result<()> {
    ctx.checkpoint_emit()?;
    super::super::paint::prepare_rough_edge_paths(ctx, edges, session.edge_cache)?;

    if ctx.hand_drawn_seed.seed().may_use_math_random() {
        session
            .edge_labels
            .update(ctx, session.hierarchy_plan, session.edge_cache, edges)?;
    }

    let _g_edge_paths = detail_guard(session.timing, &mut session.details.edge_paths);

    match edges {
        [] => {
            out.push_str(r#"<g class="edges edgePaths"/>"#);
            return out.checkpoint();
        },
        [_, ..] => {},
    }

    out.push_str(r#"<g class="edges edgePaths">"#);
    let mut scratch = FlowchartEdgeDataPointsScratch::default();

    for &e in edges {
        ctx.checkpoint_emit()?;
        render_flowchart_edge_path(
            out,
            ctx,
            session.marker_plan,
            e,
            0.0,
            0.0,
            &mut scratch,
            &*session.edge_cache,
        )?;
        ctx.checkpoint_emit()?;
        out.checkpoint()?;
    }

    out.push_str("</g>");
    out.checkpoint()
}

fn render_flowchart_elk_edge_labels(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    session: &mut FlowchartRootRenderSession<'_, '_, '_, '_>,
    edges: &[super::super::render_input::FlowchartRenderEdgeRef<'_>],
) -> crate::Result<()> {
    ctx.checkpoint_emit()?;
    let _g_edge_labels = detail_guard(session.timing, &mut session.details.edge_labels);

    match edges {
        [] => {
            out.push_str(r#"<g class="edgeLabels"/>"#);
            return out.checkpoint();
        },
        [_, ..] => {},
    }

    out.push_str(r#"<g class="edgeLabels">"#);

    for &e in edges {
        ctx.checkpoint_emit()?;

        if edge_label_is_empty(ctx, e) {
            continue;
        }

        render_flowchart_edge_label(out, ctx, e, 0.0, 0.0, session.edge_labels)?;

        out.checkpoint()?;
    }

    out.push_str("</g>");
    out.checkpoint()
}

fn initialize_flowchart_root_frame<'data, 'plan>(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &'data FlowchartRenderCtx<'data>,
    session: &mut FlowchartRootRenderSession<'_, '_, 'plan, 'data>,
    frame: &mut FlowchartRootFrame<'data, 'plan>,
) -> crate::Result<()> {
    ctx.checkpoint_emit()?;
    session.details.root_calls += 1;
    let root_emission = session.hierarchy_plan.root(frame.cluster_id)?;

    let (origin_x, origin_y, transform_attr) = if let Some(cid) = frame.cluster_id {
        if let Some(off) = session.hierarchy_plan.root_offsets(cid) {
            let rel_x = off.origin_x - frame.parent_origin_x;
            let rel_y = off.abs_top_transform - frame.parent_origin_y;
            (
                off.origin_x,
                off.origin_y,
                format!(
                    r#" transform="translate({},{})""#,
                    fmt_display(rel_x),
                    fmt_display(rel_y)
                ),
            )
        } else {

            (
                frame.parent_origin_x,
                frame.parent_origin_y,
                r#" transform="translate(0,0)""#.to_string(),
            )
        }
    } else {
        (0.0, 0.0, String::new())
    };

    frame.origin_x = origin_x;
    frame.origin_y = origin_y;
    frame.content_origin_y = origin_y;

    let _ = write!(out, r#"<g class="root"{}>"#, transform_attr);

    let _g_clusters = detail_guard(session.timing, &mut session.details.clusters);
    let clusters_to_draw = root_emission.clusters();

    match clusters_to_draw {
        [] => {
            out.push_str(r#"<g class="clusters"/>"#);
        },
        [_, ..] => {
            out.push_str(r#"<g class="clusters">"#);

            for &cluster in clusters_to_draw {
                ctx.checkpoint_emit()?;
                render_flowchart_cluster(out, ctx, cluster, origin_x, frame.content_origin_y)?;
                out.checkpoint()?;
            }

            out.push_str("</g>");
        },
    }

    drop(_g_clusters);

    let _g_edges_select = detail_guard(session.timing, &mut session.details.edges_select);
    let edges = root_emission.edges();
    drop(_g_edges_select);

    super::super::paint::prepare_rough_edge_paths(ctx, edges, session.edge_cache)?;

    if ctx.hand_drawn_seed.seed().may_use_math_random() {
        session
            .edge_labels
            .update(ctx, session.hierarchy_plan, session.edge_cache, edges)?;
    }

    let _g_edge_paths = detail_guard(session.timing, &mut session.details.edge_paths);
    let edge_group_class = if ctx.swimlane_direction.is_some() {
        "edges edgePaths"
    } else {
        "edgePaths"
    };

    match edges {
        [] => {
            let _ = write!(out, r#"<g class="{}"/>"#, edge_group_class);
        },
        [_, ..] => {
            let _ = write!(out, r#"<g class="{}">"#, edge_group_class);
            let mut scratch = FlowchartEdgeDataPointsScratch::default();

            for &e in edges {
                ctx.checkpoint_emit()?;
                render_flowchart_edge_path(
                    out,
                    ctx,
                    session.marker_plan,
                    e,
                    origin_x,
                    frame.content_origin_y,
                    &mut scratch,
                    &*session.edge_cache,
                )?;
                ctx.checkpoint_emit()?;
                out.checkpoint()?;
            }

            out.push_str("</g>");
        },
    }

    drop(_g_edge_paths);

    let _g_edge_labels = detail_guard(session.timing, &mut session.details.edge_labels);

    if ctx.swimlane_direction.is_some() || edges.is_empty() {
        out.push_str(r#"<g class="edgeLabels"/>"#);
    } else {
        out.push_str(r#"<g class="edgeLabels">"#);

        if !ctx.edge_html_labels
            && !crate::flowchart::flowchart_dagre_merges_self_loop_segments(&ctx.model.keyword)
        {


            for &e in edges {
                ctx.checkpoint_emit()?;
                render_flowchart_edge_label(
                    out,
                    ctx,
                    e,
                    origin_x,
                    frame.content_origin_y,
                    session.edge_labels,
                )?;

                if edge_label_is_empty(ctx, e) {
                    out.push_str(r#"<g><rect class="background" style="stroke: none"/></g>"#);
                    out.checkpoint()?;
                }

                out.checkpoint()?;
            }
        } else if !ctx.edge_html_labels {




            for &e in edges {
                ctx.checkpoint_emit()?;

                if edge_label_is_empty(ctx, e) {
                    out.push_str(r#"<g><rect class="background" style="stroke: none"/></g>"#);
                    out.checkpoint()?;
                }
            }

            for &e in edges {
                ctx.checkpoint_emit()?;
                render_flowchart_edge_label(
                    out,
                    ctx,
                    e,
                    origin_x,
                    frame.content_origin_y,
                    session.edge_labels,
                )?;

                out.checkpoint()?;
            }
        } else {


            for &e in edges {
                ctx.checkpoint_emit()?;
                render_flowchart_edge_label(
                    out,
                    ctx,
                    e,
                    origin_x,
                    frame.content_origin_y,
                    session.edge_labels,
                )?;

                out.checkpoint()?;
            }
        }

        out.push_str("</g>");
    }

    drop(_g_edge_labels);

    out.push_str(r#"<g class="nodes">"#);

    let _g_dom_order = detail_guard(session.timing, &mut session.details.dom_order);
    let dom_order = root_emission.dom_order();
    drop(_g_dom_order);

    frame.dom_order = dom_order;
    frame.initialized = true;
    out.checkpoint()
}
