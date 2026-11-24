use crate::dagre::self_loop::compact_self_loop_geometry;
use crate::geom::Size;
use crate::layout_work::DugongOperationWorkControl as DagreOperationWorkControl;
use crate::math::MathRenderer;
use crate::model::{
    FlowchartLayout, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::resources::OperationWorkMeter;
#[cfg(test)]
use crate::resources::RenderResourcePolicy;
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use dugong::graphlib::{Graph, GraphOptions, is_javascript_array_index};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
use indexmap::IndexMap;
#[cfg(test)]
use merman_model::families::flowchart::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility};
use merman_model::presentation::PresentationConfig;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::config::{FlowchartConfigView, FlowchartLayoutSettings};
use super::label::compute_bounds_controlled;
use super::node::{NodeLayoutDimensionsRequest, node_layout_dimensions};
use super::{
    FlowEdge, FlowSubgraph, FlowchartModel, FlowchartRenderContext, FlowchartRenderModelRef,
};
use super::{
    FlowchartLabelMetricsRequest, FlowchartSvgLabelOwner, FlowchartSvgLabelSidecarBuilder,
    FlowchartSvgWidthMode, flowchart_apply_html_node_class_box_metrics,
    flowchart_effective_text_style_for_classes,
    flowchart_effective_text_style_for_classes_with_provenance,
    flowchart_effective_text_style_for_node_classes_with_provenance, flowchart_node_svg_width_mode,
    measure_flowchart_svg_label_for_layout_with_metrics_style_and_terminal_foreground,
    measure_flowchart_svg_label_for_layout_with_terminal_foreground,
};

type FlowSubgraphIndex<'a> = HashMap<&'a str, &'a FlowSubgraph>;

#[must_use]
fn rank_dir_from_flow(direction: &str) -> RankDir {
    match direction.trim().to_uppercase().as_str() {
        "TB" | "TD" => RankDir::TB,
        "BT" => RankDir::BT,
        "LR" => RankDir::LR,
        "RL" => RankDir::RL,
        _ => RankDir::TB,
    }
}

#[must_use]
fn normalize_dir(s: &str) -> String {
    s.trim().to_uppercase()
}

#[must_use]
fn toggled_dir(parent: &str) -> String {
    let parent_2 = normalize_dir(parent);

    if parent_2 == "TB" || parent_2 == "TD" {
        "LR".to_string()
    } else {
        "TB".to_string()
    }
}

#[must_use]
fn flow_dir_from_rankdir(rankdir: RankDir) -> &'static str {
    match rankdir {
        RankDir::TB => "TB",
        RankDir::BT => "BT",
        RankDir::LR => "LR",
        RankDir::RL => "RL",
    }
}

#[must_use]
fn effective_cluster_dir(sg: &FlowSubgraph, parent_dir: &str, inherit_dir: bool) -> String {
    match sg.dir.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(dir) => {
            return normalize_dir(dir);
        },
        ::core::option::Option::None => {},
    }

    if inherit_dir {
        return normalize_dir(parent_dir);
    }

    toggled_dir(parent_dir)
}

fn compute_effective_dir_by_id(
    subgraphs_in_order: &[FlowSubgraph],
    subgraphs_by_id: &FlowSubgraphIndex<'_>,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    diagram_dir: &str,
    inherit_dir: bool,
    work_control: &mut DagreOperationWorkControl,
) -> Result<HashMap<String, String>> {
    fn compute_one_iterative(
        id: &str,
        subgraphs_by_id: &FlowSubgraphIndex<'_>,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        diagram_dir: &str,
        inherit_dir: bool,
        memo: &mut HashMap<String, String>,
        work_control: &mut DagreOperationWorkControl,
    ) -> Result<()> {
        if memo.contains_key(id) {
            return Ok(());
        }

        let mut path: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut cur = id.to_string();
        let mut parent_dir = loop {
            work_control.charge_adapter(1)?;

            match memo.get(&cur) {
                Some(dir) => {
                    break dir.clone();
                },
                ::core::option::Option::None => {},
            }

            if !seen.insert(cur.clone()) {
                let dir = toggled_dir(diagram_dir);
                memo.insert(cur.clone(), dir.clone());
                break dir;
            }

            path.push(cur.clone());

            let parent = {
                let optional_parent = g.parent(&cur).filter(|p| subgraphs_by_id.contains_key(*p));

                match optional_parent {
                    Some(matched_parent) => matched_parent,
                    ::core::option::Option::None => break normalize_dir(diagram_dir),
                }
            };

            cur = parent.to_string();
        };

        for node in path.into_iter().rev() {
            work_control.charge_adapter(1)?;

            match memo.get(&node) {
                Some(dir) => {
                    parent_dir = dir.clone();
                    continue;
                },
                ::core::option::Option::None => {},
            }

            let dir = subgraphs_by_id
                .get(node.as_str())
                .map(|sg| effective_cluster_dir(sg, &parent_dir, inherit_dir))
                .unwrap_or_else(|| toggled_dir(&parent_dir));
            memo.insert(node, dir.clone());
            parent_dir = dir;
        }

        Ok(())
    }

    let mut memo: HashMap<String, String> = HashMap::new();




    for subgraph in subgraphs_in_order {
        work_control.charge_adapter(1)?;
        compute_one_iterative(
            &subgraph.id,
            subgraphs_by_id,
            g,
            diagram_dir,
            inherit_dir,
            &mut memo,
            work_control,
        )?;
    }

    Ok(memo)
}

#[must_use]
fn dir_to_rankdir(dir: &str) -> RankDir {
    match normalize_dir(dir).as_str() {
        "TB" | "TD" => RankDir::TB,
        "BT" => RankDir::BT,
        "LR" => RankDir::LR,
        "RL" => RankDir::RL,
        _ => RankDir::TB,
    }
}

const SELF_LOOP_ID_EXTRA: &str = "selfLoopId";
const SELF_LOOP_NODE_EXTRA: &str = "selfLoopNode";
const SELF_LOOP_ORDER_EXTRA: &str = "selfLoopOrder";
const EDGE_OWNER_INDEX_EXTRA: &str = "mermanEdgeOwnerIndex";

#[derive(Debug, Clone, PartialEq, Eq)]
struct FlowchartSelfLoopSegmentMeta {
    logical_edge_id: String,
    owner_index: usize,
    node_id: String,
    order: u8,
}

struct FlowchartLayoutEdgeCandidate {
    edge: LayoutEdge,
    owner_index: usize,
    self_loop: Option<FlowchartSelfLoopSegmentMeta>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FlowchartLayoutSegmentKey {
    owner_index: usize,
    self_loop_order: Option<u8>,
}

impl FlowchartLayoutSegmentKey {
    #[must_use]
    fn new(owner_index: usize, self_loop: Option<&FlowchartSelfLoopSegmentMeta>) -> Self {
        Self {
            owner_index,
            self_loop_order: self_loop.map(|meta| meta.order),
        }
    }
}

#[must_use]
fn flowchart_layout_edge_key(
    edge: &FlowEdge,
    self_loop: Option<&FlowchartSelfLoopSegmentMeta>,
) -> String {
    match self_loop {
        Some(meta) => format!("{}-cyclic-special-{}", meta.node_id, meta.order),
        None => edge.id.clone(),
    }
}

fn annotate_flowchart_self_loop_segment(
    label: &mut EdgeLabel,
    meta: &FlowchartSelfLoopSegmentMeta,
) {
    label.extras.insert(
        SELF_LOOP_ID_EXTRA.to_string(),
        Value::String(meta.logical_edge_id.clone()),
    );
    label.extras.insert(
        SELF_LOOP_NODE_EXTRA.to_string(),
        Value::String(meta.node_id.clone()),
    );
    label
        .extras
        .insert(SELF_LOOP_ORDER_EXTRA.to_string(), Value::from(meta.order));
}

fn annotate_flowchart_edge_owner(label: &mut EdgeLabel, owner_index: usize) {
    label
        .extras
        .insert(EDGE_OWNER_INDEX_EXTRA.to_string(), Value::from(owner_index));
}

#[must_use]
fn flowchart_edge_owner_index(label: &EdgeLabel) -> Option<usize> {
    match usize::try_from(label.extras.get(EDGE_OWNER_INDEX_EXTRA)?.as_u64()?) { ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_), ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None }
}

#[must_use]
fn flowchart_layout_segment_key(label: &EdgeLabel) -> Option<FlowchartLayoutSegmentKey> {
    let owner_index = flowchart_edge_owner_index(label)?;
    let self_loop_order = label
        .extras
        .get(SELF_LOOP_ORDER_EXTRA)
        .and_then(Value::as_u64)
        .and_then(|order| match u8::try_from(order) { ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_), ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None });
    Some(FlowchartLayoutSegmentKey {
        owner_index,
        self_loop_order,
    })
}

#[must_use]
fn flowchart_self_loop_segment_meta(label: &EdgeLabel) -> Option<FlowchartSelfLoopSegmentMeta> {
    let logical_edge_id = label.extras.get(SELF_LOOP_ID_EXTRA)?.as_str()?.to_string();
    let node_id = label
        .extras
        .get(SELF_LOOP_NODE_EXTRA)?
        .as_str()?
        .to_string();
    let order = match u8::try_from(label.extras.get(SELF_LOOP_ORDER_EXTRA)?.as_u64()?) { ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_), ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None }?;
    Some(FlowchartSelfLoopSegmentMeta {
        logical_edge_id,
        owner_index: flowchart_edge_owner_index(label)?,
        node_id,
        order,
    })
}

#[must_use]
fn merge_flowchart_self_loop_segments(
    model_edges: &[FlowEdge],
    layout_nodes: &[LayoutNode],
    rankdir: &str,
    layout_edges: Vec<FlowchartLayoutEdgeCandidate>,
) -> Vec<FlowchartLayoutEdgeCandidate> {
    let mut output = Vec::with_capacity(layout_edges.len());
    let mut segments_by_owner: IndexMap<usize, Vec<FlowchartLayoutEdgeCandidate>> = IndexMap::new();

    for candidate in layout_edges {
        if candidate.self_loop.is_some() {
            segments_by_owner
                .entry(candidate.owner_index)
                .or_default()
                .push(candidate);
        } else {
            output.push(candidate);
        }
    }

    let nodes_by_id: HashMap<&str, &LayoutNode> = layout_nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();

    for (owner_index, mut segments) in segments_by_owner {
        if segments.len() != 3 {
            output.extend(segments);
            continue;
        }

        segments.sort_by_key(|candidate| {
            candidate
                .self_loop
                .as_ref()
                .map_or(u8::MAX, |meta| meta.order)
        });

        if segments
            .iter()
            .filter_map(|candidate| candidate.self_loop.as_ref().map(|meta| meta.order))
            .ne([0_u8, 1, 2])
        {
            output.extend(segments);
            continue;
        }

        let first = &segments[0].edge;
        let first_meta = segments[0]
            .self_loop
            .as_ref()
            .expect("sorted self-loop segment metadata");
        let middle = &segments[1].edge;
        let last = &segments[2].edge;

        let Some(original) = model_edges.get(owner_index) else {
            output.extend(segments);
            continue;
        };

        let node_id = first_meta.node_id.as_str();

        let Some(node) = nodes_by_id.get(node_id).copied() else {
            output.extend(segments);
            continue;
        };

        let helper_ids = [
            format!("{node_id}---{node_id}---1"),
            format!("{node_id}---{node_id}---2"),
        ];
        let mut hints = helper_ids
            .iter()
            .filter_map(|id| nodes_by_id.get(id.as_str()).copied())
            .map(|helper| LayoutPoint {
                x: helper.x,
                y: helper.y,
            })
            .collect::<Vec<_>>();

        if hints.is_empty() {
            hints.extend(
                [first, middle, last]
                    .into_iter()
                    .flat_map(|edge| edge.points.iter().cloned()),
            );
        }

        let label_width = middle.label.as_ref().map_or(0.0, |label| label.width);
        let label_height = middle.label.as_ref().map_or(0.0, |label| label.height);
        let geometry = compact_self_loop_geometry(
            &LayoutPoint {
                x: node.x,
                y: node.y,
            },
            Size::new(node.width, node.height),
            dir_to_rankdir(rankdir),
            &hints,
            0.0,
            Size::new(label_width, label_height),
        );
        let label = middle.label.as_ref().map(|_| LayoutLabel {
            x: geometry.label_center.x,
            y: geometry.label_center.y,
            width: label_width,
            height: label_height,
        });

        output.push(FlowchartLayoutEdgeCandidate {
            owner_index,
            self_loop: None,
            edge: LayoutEdge {
                id: original.id.clone(),
                from: original.from.clone(),
                to: original.to.clone(),
                from_cluster: first
                    .from_cluster
                    .clone()
                    .or_else(|| middle.from_cluster.clone())
                    .or_else(|| last.from_cluster.clone()),
                to_cluster: first
                    .to_cluster
                    .clone()
                    .or_else(|| middle.to_cluster.clone())
                    .or_else(|| last.to_cluster.clone()),
                points: geometry.points,
                label,
                start_label_left: None,
                start_label_right: None,
                end_label_left: None,
                end_label_right: None,
                start_marker: None,
                end_marker: None,
                stroke_dasharray: None,
            },
        });
    }

    output
}

#[must_use]
fn render_edge_label_for_layout<'a>(
    model: &'a FlowchartRenderModelRef<'_>,
    semantic_index: usize,
    edge: &'a FlowEdge,
    self_loop: Option<&FlowchartSelfLoopSegmentMeta>,
) -> Option<&'a str> {
    match self_loop {
        ::core::option::Option::Some(_) => {
            edge.label.as_deref()
        },
        ::core::option::Option::None => {
            model.edge_label_for_render(semantic_index, edge)
        },
    }
}

#[must_use]
fn edge_label_is_non_empty(
    model: &FlowchartRenderModelRef<'_>,
    semantic_index: usize,
    edge: &FlowEdge,
    self_loop: Option<&FlowchartSelfLoopSegmentMeta>,
) -> bool {
    render_edge_label_for_layout(model, semantic_index, edge, self_loop)
        .is_some_and(|text| !crate::flowchart::flowchart_label_is_empty_for_render(text))
}

fn lowest_common_parent(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    a: &str,
    b: &str,
    work_control: &mut DagreOperationWorkControl,
) -> Result<Option<String>> {
    if !graph.options().compound {
        return Ok(None);
    }

    let mut ancestors = HashSet::new();
    let mut current = graph.parent(a);

    while let Some(parent) = current {
        work_control.charge_adapter(1)?;
        ancestors.insert(parent.to_string());
        current = graph.parent(parent);
    }

    let mut current_2 = graph.parent(b);

    while let Some(parent) = current_2 {
        work_control.charge_adapter(1)?;

        if ancestors.contains(parent) {
            return Ok(Some(parent.to_string()));
        }

        current_2 = graph.parent(parent);
    }

    Ok(None)
}

fn extract_descendants(
    id: &str,
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut DagreOperationWorkControl,
) -> Result<Vec<String>> {
    let initial_children = child_id_snapshot_work_upper_bound(graph, id, work_control)?;
    work_control.charge_adapter(initial_children)?;
    let mut out = Vec::new();
    let mut visited = HashSet::new();
    let mut stack = graph
        .children_iter(id)
        .map(str::to_string)
        .collect::<Vec<_>>();

    while let Some(node) = stack.pop() {
        work_control.charge_adapter(1)?;

        if !visited.insert(node.clone()) {
            continue;
        }

        let child_count = child_id_snapshot_work_upper_bound(graph, &node, work_control)?;
        work_control.charge_adapter(child_count)?;
        stack.extend(graph.children_iter(&node).map(str::to_string));
        out.push(node);
    }

    Ok(out)
}

fn edge_in_cluster(
    edge: &dugong::graphlib::EdgeKey,
    cluster_id: &str,
    descendants: &HashMap<String, Vec<String>>,
    work_control: &mut DagreOperationWorkControl,
) -> Result<bool> {
    if edge.v == cluster_id || edge.w == cluster_id {
        return Ok(false);
    }

    let cluster_descendants = {
        let optional_cluster_descendants = descendants.get(cluster_id);

        match optional_cluster_descendants {
            Some(matched_cluster_descendants) => matched_cluster_descendants,
            ::core::option::Option::None => return Ok(false),
        }
    };

    for descendant in cluster_descendants {
        work_control.charge_adapter(1)?;

        if descendant == &edge.v {
            return Ok(true);
        }
    }

    for descendant in cluster_descendants {
        work_control.charge_adapter(1)?;

        if descendant == &edge.w {
            return Ok(true);
        }
    }

    Ok(false)
}

#[derive(Debug, Clone)]
struct FlowchartClusterDbEntry {
    anchor_id: String,
    external_connections: bool,
}

fn flowchart_find_common_edges(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    id1: &str,
    id2: &str,
    work_control: &mut DagreOperationWorkControl,
) -> Result<Vec<(String, String)>> {
    let edge_snapshot_work =
        work_control.checked_add(graph.edge_slot_count(), graph.edge_count())?;
    work_control.charge_adapter(edge_snapshot_work)?;
    let edges1 = graph
        .edges()
        .filter(|edge| edge.v == id1 || edge.w == id1)
        .map(|edge| (edge.v.clone(), edge.w.clone()))
        .collect::<Vec<_>>();
    work_control.charge_adapter(edge_snapshot_work)?;
    let edges2 = graph
        .edges()
        .filter(|edge| edge.v == id2 || edge.w == id2)
        .map(|edge| (edge.v.clone(), edge.w.clone()))
        .collect::<Vec<_>>();

    work_control.charge_adapter(edges1.len())?;
    let edges1_prim = edges1
        .into_iter()
        .map(|(v, w)| {
            (
                if v == id1 { id2.to_string() } else { v },


                if w == id1 { id1.to_string() } else { w },
            )
        })
        .collect::<Vec<_>>();

    let mut out = Vec::new();

    for edge in edges1_prim {
        let mut found = false;

        'over_candidate_2: for candidate in &edges2 {
            work_control.charge_adapter(1)?;

            if candidate == &edge {
                found = true;
                break 'over_candidate_2;
            }
        }

        if found {
            out.push(edge);
        }
    }

    Ok(out)
}

fn flowchart_find_non_cluster_child(
    id: &str,
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    cluster_id: &str,
    work_control: &mut DagreOperationWorkControl,
) -> Result<Option<String>> {
    let child_count = graph.child_count(id);

    match child_count {
        0 => {
            return Ok(Some(id.to_string()));
        },
        _ => {},
    }

    work_control.charge_adapter(child_id_snapshot_work_upper_bound(graph, id, work_control)?)?;
    let mut reserve: Option<String> = None;
    let mut visited: HashSet<String> = HashSet::new();
    let mut stack = graph
        .children(id)
        .into_iter()
        .rev()
        .map(str::to_string)
        .collect::<Vec<_>>();

    while let Some(node) = stack.pop() {
        work_control.charge_adapter(1)?;

        if !visited.insert(node.clone()) {
            continue;
        }

        let child_count_2 = graph.child_count(&node);

        match child_count_2 {
            0 => {},
            _ => {
                work_control.charge_adapter(child_id_snapshot_work_upper_bound(
                    graph,
                    &node,
                    work_control,
                )?)?;
                stack.extend(graph.children(&node).into_iter().rev().map(str::to_string));
                continue;
            },
        }

        if !flowchart_find_common_edges(graph, cluster_id, &node, work_control)?.is_empty() {
            reserve = Some(node);
        } else {
            return Ok(Some(node));
        }
    }

    Ok(reserve)
}

fn flowchart_is_node_in_extractable_cluster(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    node_id: &str,
    root_id: &str,
    cluster_db: &HashMap<String, FlowchartClusterDbEntry>,
    work_control: &mut DagreOperationWorkControl,
) -> Result<bool> {
    let mut parent = graph.parent(node_id);

    while let Some(parent_id) = parent {
        work_control.charge_adapter(1)?;

        if parent_id == root_id {
            break;
        }

        if cluster_db
            .get(parent_id)
            .is_some_and(|entry| !entry.external_connections)
        {
            return Ok(true);
        }

        parent = graph.parent(parent_id);
    }

    Ok(false)
}

fn flowchart_find_safe_anchor_node(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    cluster_id: &str,
    excluded_cluster: &str,
    descendants: &HashMap<String, Vec<String>>,
    cluster_db: &HashMap<String, FlowchartClusterDbEntry>,
    work_control: &mut DagreOperationWorkControl,
) -> Result<Option<String>> {
    work_control.charge_adapter(graph.child_order_slot_count(cluster_id))?;

    for child in graph.children_iter(cluster_id) {
        if child == excluded_cluster {
            continue;
        }

        match descendants.get(excluded_cluster) {
            Some(excluded_descendants) => {
                let mut excluded = false;

                'over_descendant_2: for descendant in excluded_descendants {
                    work_control.charge_adapter(1)?;

                    if descendant == child {
                        excluded = true;
                        break 'over_descendant_2;
                    }
                }

                if excluded {
                    continue;
                }
            },
            ::core::option::Option::None => {},
        }

        let candidate = {
            let optional_candidate = flowchart_find_non_cluster_child(child, graph, cluster_id, work_control)?;

            match optional_candidate {
                Some(matched_candidate) => matched_candidate,
                ::core::option::Option::None => continue,
            }
        };

        if !flowchart_is_node_in_extractable_cluster(
            graph,
            &candidate,
            cluster_id,
            cluster_db,
            work_control,
        )? {
            return Ok(Some(candidate));
        }
    }

    Ok(None)
}

fn adjust_flowchart_clusters_and_edges(
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut DagreOperationWorkControl,
) -> Result<std::collections::HashMap<String, bool>> {
    use serde_json::Value;

    fn is_descendant(
        node_id: &str,
        cluster_id: &str,
        descendants: &HashMap<String, Vec<String>>,
        work_control: &mut DagreOperationWorkControl,
    ) -> Result<bool> {
        let cluster_descendants = {
            let optional_cluster_descendants = descendants.get(cluster_id);

            match optional_cluster_descendants {
                Some(matched_cluster_descendants) => matched_cluster_descendants,
                ::core::option::Option::None => return Ok(false),
            }
        };

        for descendant in cluster_descendants {
            work_control.charge_adapter(1)?;

            if descendant == node_id {
                return Ok(true);
            }
        }

        Ok(false)
    }

    let node_snapshot_work = node_id_snapshot_work_upper_bound(graph, work_control)?;
    work_control.charge_adapter(node_snapshot_work)?;
    let node_ids = graph.node_ids();
    let mut descendants: HashMap<String, Vec<String>> = HashMap::new();
    let mut cluster_db: std::collections::HashMap<String, FlowchartClusterDbEntry> =
        std::collections::HashMap::new();
    let mut cluster_ids = Vec::new();
    work_control.charge_adapter(node_ids.len())?;

    for id in node_ids {
        if graph.child_count(&id) == 0 {
            continue;
        }

        descendants.insert(id.clone(), extract_descendants(&id, graph, work_control)?);
        let anchor_id = flowchart_find_non_cluster_child(&id, graph, &id, work_control)?
            .unwrap_or_else(|| id.clone());
        cluster_ids.push(id.clone());
        cluster_db.insert(
            id,
            FlowchartClusterDbEntry {
                anchor_id,
                external_connections: false,
            },
        );
    }

    for id in &cluster_ids {
        let mut has_external = false;

        'over_edge_2: for edge in graph.edges() {
            work_control.charge_adapter(1)?;
            let v_is_descendant = is_descendant(&edge.v, id, &descendants, work_control)?;
            let w_is_descendant = is_descendant(&edge.w, id, &descendants, work_control)?;

            if v_is_descendant ^ w_is_descendant {
                has_external = true;
                break 'over_edge_2;
            }
        }

        {
            match cluster_db.get_mut(id) {
                Some(entry) => {
                    entry.external_connections = has_external;
                },
                ::core::option::Option::None => {},
            };
        }
    }

    for id in &cluster_ids {
        let non_cluster_child = {
            let optional_non_cluster_child = cluster_db.get(id).map(|e| e.anchor_id.clone());

            match optional_non_cluster_child {
                Some(matched_non_cluster_child) => matched_non_cluster_child,
                ::core::option::Option::None => continue,
            }
        };

        let parent = graph.parent(&non_cluster_child);

        if parent.is_some_and(|p| p != id.as_str())
            && parent.is_some_and(|p| cluster_db.contains_key(p))
            && parent.is_some_and(|p| !cluster_db.get(p).is_some_and(|e| e.external_connections))
            && let Some(p) = parent
            && let Some(entry) = cluster_db.get_mut(id)
        {
            entry.anchor_id = p.to_string();
        }

        work_control.charge_adapter(graph.edge_count())?;
        let mut has_direct_outgoing_edge = false;

        for edge in graph.edges() {
            has_direct_outgoing_edge |= edge.v == *id;
        }

        let needs_safe_anchor = cluster_db
            .get(id)
            .is_some_and(|entry| entry.external_connections)
            && has_direct_outgoing_edge
            && flowchart_is_node_in_extractable_cluster(
                graph,
                &non_cluster_child,
                id,
                &cluster_db,
                work_control,
            )?;

        if needs_safe_anchor
            && let Some(excluded_cluster) = graph.parent(&non_cluster_child)
            && let Some(safe_anchor) = flowchart_find_safe_anchor_node(
                graph,
                id,
                excluded_cluster,
                &descendants,
                &cluster_db,
                work_control,
            )?
            && let Some(entry) = cluster_db.get_mut(id)
        {
            entry.anchor_id = safe_anchor;
        }
    }

    #[must_use]
    fn get_anchor_id(
        id: &str,
        cluster_db: &std::collections::HashMap<String, FlowchartClusterDbEntry>,
    ) -> String {
        let entry = {
            let optional_entry = cluster_db.get(id);

            match optional_entry {
                Some(matched_entry) => matched_entry,
                ::core::option::Option::None => return id.to_string(),
            }
        };

        if !entry.external_connections {
            return id.to_string();
        }

        entry.anchor_id.clone()
    }

    let edge_snapshot_work =
        work_control.checked_add(graph.edge_slot_count(), graph.edge_count())?;
    work_control.charge_adapter(edge_snapshot_work)?;
    let edge_keys = graph.edge_keys();

    for ek in edge_keys {
        if !cluster_db.contains_key(&ek.v) && !cluster_db.contains_key(&ek.w) {
            continue;
        }

        let mut edge_label = {
            let optional_edge_label = graph.edge_by_key(&ek).cloned();

            match optional_edge_label {
                Some(matched_edge_label) => matched_edge_label,
                ::core::option::Option::None => continue,
            }
        };

        let v = get_anchor_id(&ek.v, &cluster_db);
        let w = get_anchor_id(&ek.w, &cluster_db);




        let _ = graph.remove_edge_key(&ek);

        if v != ek.v {
            if let Some(parent) = graph.parent(&v)
                && let Some(entry) = cluster_db.get_mut(parent)
            {
                entry.external_connections = true;
            }

            edge_label
                .extras
                .insert("fromCluster".to_string(), Value::String(ek.v.clone()));
        }

        if w != ek.w {
            if let Some(parent) = graph.parent(&w)
                && let Some(entry) = cluster_db.get_mut(parent)
            {
                entry.external_connections = true;
            }

            edge_label
                .extras
                .insert("toCluster".to_string(), Value::String(ek.w.clone()));
        }

        graph.set_edge_named(v, w, ek.name, Some(edge_label));
    }

    Ok(cluster_db
        .into_iter()
        .map(|(id, entry)| (id, entry.external_connections))
        .collect())
}

fn checked_n_log_n(value: usize, work_control: &DagreOperationWorkControl) -> Result<usize> {
    match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(value), &::core::convert::identity(1)); _explicit_ordering } {
        ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => {
            return Ok(0);
        },
        ::core::cmp::Ordering::Greater => {},
    }

    let passes = usize::BITS as usize - (value - 1).leading_zeros() as usize;
    work_control.checked_mul(value, passes)
}

fn ordered_key_update_work_upper_bound(
    entry_bound: usize,
    update_count: usize,
    work_control: &DagreOperationWorkControl,
) -> Result<usize> {
    let height = match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(entry_bound), &::core::convert::identity(1)); _explicit_ordering } {
        ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => {
            1
        },
        ::core::cmp::Ordering::Greater => {
            let log = usize::BITS as usize - (entry_bound - 1).leading_zeros() as usize;
            work_control.checked_add(log, 1)?
        },
    };
    work_control.checked_mul(update_count, height)
}

fn node_id_snapshot_work_upper_bound(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &DagreOperationWorkControl,
) -> Result<usize> {
    work_control.checked_add(graph.node_order_slot_count(), graph.node_count())
}

fn child_id_snapshot_work_upper_bound(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    parent: &str,
    work_control: &DagreOperationWorkControl,
) -> Result<usize> {
    work_control.checked_add(
        graph.child_order_slot_count(parent),
        graph.child_count(parent),
    )
}

fn charge_node_insertion_ordering(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    node_id: &str,
    work_control: &mut DagreOperationWorkControl,
) -> Result<()> {
    if graph.has_node(node_id) || !is_javascript_array_index(node_id) {
        return Ok(());
    }

    let final_node_count = work_control.checked_add(graph.node_count(), 1)?;
    let ordered_maps = ::core::primitive::usize::wrapping_add(1, usize::from(graph.options().compound));
    let work = ordered_key_update_work_upper_bound(final_node_count, ordered_maps, work_control)?;
    work_control.charge_adapter(work)
}

fn charge_edge_endpoint_insertions(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    v: &str,
    w: &str,
    work_control: &mut DagreOperationWorkControl,
) -> Result<()> {
    let v_missing = !graph.has_node(v);
    let w_missing = w != v && !graph.has_node(w);
    let missing_count = ::core::primitive::usize::wrapping_add(usize::from(v_missing), usize::from(w_missing));

    match missing_count {
        0 => {
            return Ok(());
        },
        _ => {},
    }

    let numeric_count = ::core::primitive::usize::wrapping_add(usize::from(v_missing && is_javascript_array_index(v)), usize::from(w_missing && is_javascript_array_index(w)));
    let final_node_count = work_control.checked_add(graph.node_count(), missing_count)?;
    let ordered_updates =
        work_control.checked_mul(numeric_count, ::core::primitive::usize::wrapping_add(1, usize::from(graph.options().compound)))?;
    let ordered_work =
        ordered_key_update_work_upper_bound(final_node_count, ordered_updates, work_control)?;
    work_control.charge_adapter(work_control.checked_add(missing_count, ordered_work)?)
}

fn remove_node_work_upper_bound(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    node_id: &str,
    work_control: &DagreOperationWorkControl,
) -> Result<usize> {
    let node_slots = graph.node_slot_count();
    let edge_slots = graph.edge_slot_count();
    let incident_slots = if graph.options().directed {
        work_control.checked_mul(edge_slots, 2)?
    } else {
        edge_slots
    };
    let cache_and_tombstone_work = work_control.checked_add(
        work_control.checked_mul(node_slots, 7)?,
        work_control.checked_mul(edge_slots, 6)?,
    )?;
    let incident_work = work_control.checked_add(
        work_control.checked_mul(incident_slots, 2)?,
        checked_n_log_n(incident_slots, work_control)?,
    )?;
    let compound = graph.options().compound;
    let previous_order_slots = if compound {
        graph.parent(node_id).map_or_else(
            || graph.root_child_order_slot_count(),
            |parent| graph.child_order_slot_count(parent),
        )
    } else {
        0
    };
    let promoted_order_slots = if compound {
        graph.child_order_slot_count(node_id)
    } else {
        0
    };
    let object_order_scan_work = work_control.checked_add(
        graph.node_order_slot_count(),
        work_control.checked_add(previous_order_slots, promoted_order_slots)?,
    )?;
    let numeric_node = usize::from(is_javascript_array_index(node_id));
    let promoted_numeric = if compound {
        graph.array_index_child_count(node_id)
    } else {
        0
    };
    let numeric_updates = work_control.checked_add(
        work_control.checked_mul(numeric_node, ::core::primitive::usize::wrapping_add(1, usize::from(compound)))?,
        promoted_numeric,
    )?;
    let ordered_map_work = ordered_key_update_work_upper_bound(
        graph.array_index_node_count(),
        numeric_updates,
        work_control,
    )?;
    work_control.checked_add(
        work_control.checked_add(
            work_control.checked_add(cache_and_tombstone_work, incident_work)?,
            object_order_scan_work,
        )?,
        ordered_map_work,
    )
}

fn unparented_parent_batch_work_upper_bound(
    node_slots: usize,
    existing_parent_count: usize,
    assignment_count: usize,
    numeric_assignment_count: usize,
    work_control: &DagreOperationWorkControl,
) -> Result<usize> {
    match assignment_count {
        0 => {
            return Ok(0);
        },
        _ => {},
    }

    let height = match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(node_slots), &::core::convert::identity(1)); _explicit_ordering } {
        ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => {
            0
        },
        ::core::cmp::Ordering::Greater => {
            usize::BITS as usize - (node_slots - 1).leading_zeros() as usize
        },
    };


    let find_work = work_control.checked_mul(work_control.checked_add(height, 1)?, 4)?;
    let forest_items = work_control.checked_add(existing_parent_count, assignment_count)?;
    let union_work = work_control.checked_mul(forest_items, find_work)?;
    let linear_work = work_control.checked_add(
        work_control.checked_mul(node_slots, 6)?,
        work_control.checked_mul(assignment_count, 2)?,
    )?;



    let ordered_updates = work_control.checked_mul(numeric_assignment_count, 2)?;
    let ordered_work =
        ordered_key_update_work_upper_bound(node_slots, ordered_updates, work_control)?;
    work_control.checked_add(
        work_control.checked_add(union_work, linear_work)?,
        ordered_work,
    )
}

#[derive(Debug, Clone, Copy, Default)]
struct UnparentedParentBatchShape {
    existing_parent_count: usize,
    numeric_assignment_count: usize,
}

fn apply_unparented_parent_assignments(
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    assignments: &[(usize, usize)],
    shape: UnparentedParentBatchShape,
    work_control: &mut DagreOperationWorkControl,
) -> Result<()> {
    match assignments {
        [] => {
            return Ok(());
        },
        [_, ..] => {},
    }

    let work = unparented_parent_batch_work_upper_bound(
        graph.node_slot_count(),
        shape.existing_parent_count,
        assignments.len(),
        shape.numeric_assignment_count,
        work_control,
    )?;
    work_control.charge_adapter(work)?;

    match graph.try_set_unparented_parents_ix(assignments) {
        Ok(_) => Ok(()),
        Err(dugong::graphlib::GraphError::ParentCycle {
            child_ix,
            parent_ix,
        }) => {
            let child = graph.node_id_by_ix(child_ix).unwrap_or("<removed>");
            let parent = graph.node_id_by_ix(parent_ix).unwrap_or("<removed>");
            Err(Error::InvalidModel {
                message: format!("Setting {parent} as parent of {child} would create a cycle"),
            })
        }
        Err(error) => Err(Error::InvalidModel {
            message: error.to_string(),
        }),
    }
}

fn set_parent_controlled(
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    child: &str,
    parent: &str,
    work_control: &mut DagreOperationWorkControl,
) -> Result<()> {
    let child_exists = graph.has_node(child);
    let parent_exists = graph.has_node(parent);
    let inserted_nodes = if child == parent {
        usize::from(!child_exists)
    } else {
        ::core::primitive::usize::wrapping_add(usize::from(!child_exists), usize::from(!parent_exists))
    };
    let final_node_slots = work_control.checked_add(graph.node_slot_count(), inserted_nodes)?;
    let child_is_numeric = is_javascript_array_index(child);
    let relation_changes =
        !child_exists || graph.parent(child) != Some(parent) || !child_is_numeric;
    let previous_order_slots = if relation_changes && !child_is_numeric {
        if child_exists {
            graph.parent(child).map_or_else(
                || graph.root_child_order_slot_count(),
                |previous_parent| graph.child_order_slot_count(previous_parent),
            )
        } else {
            work_control.checked_add(graph.root_child_order_slot_count(), 1)?
        }
    } else {
        0
    };
    let inserted_numeric_nodes = if child == parent {
        usize::from(!child_exists && child_is_numeric)
    } else {
        ::core::primitive::usize::wrapping_add(usize::from(!child_exists && child_is_numeric), usize::from(!parent_exists && is_javascript_array_index(parent)))
    };
    let final_numeric_nodes =
        work_control.checked_add(graph.array_index_node_count(), inserted_numeric_nodes)?;
    let insertion_updates = work_control.checked_mul(
        inserted_numeric_nodes,
        ::core::primitive::usize::wrapping_add(1, usize::from(graph.options().compound)),
    )?;
    let relation_updates = if relation_changes && child_is_numeric {
        2
    } else {
        0
    };
    let ordered_map_work = ordered_key_update_work_upper_bound(
        final_numeric_nodes,
        work_control.checked_add(insertion_updates, relation_updates)?,
        work_control,
    )?;
    let base_work = work_control.checked_add(
        final_node_slots,
        work_control.checked_add(inserted_nodes, 1)?,
    )?;
    let work = work_control.checked_add(
        work_control.checked_add(base_work, previous_order_slots)?,
        ordered_map_work,
    )?;
    work_control.charge_adapter(work)?;
    graph.set_parent_ref(child, parent);
    Ok(())
}

fn copy_cluster(
    cluster_id: &str,
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    new_graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    root_id: &str,
    descendants: &HashMap<String, Vec<String>>,
    work_control: &mut DagreOperationWorkControl,
) -> Result<()> {
    #[derive(Debug)]
    struct CopyFrame {
        node: String,
        owner_cluster: String,
        expanded: bool,
    }

    let mut nodes: Vec<(String, String)> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut stack: Vec<CopyFrame> = Vec::new();

    if cluster_id != root_id {
        stack.push(CopyFrame {
            node: cluster_id.to_string(),
            owner_cluster: cluster_id.to_string(),
            expanded: true,
        });
    }

    let child_count = graph.child_count(cluster_id);
    let child_snapshot = child_id_snapshot_work_upper_bound(graph, cluster_id, work_control)?;
    work_control.charge_adapter(work_control.checked_add(child_snapshot, child_count)?)?;
    stack.extend(
        graph
            .children(cluster_id)
            .into_iter()
            .rev()
            .map(|child| CopyFrame {
                node: child.to_string(),
                owner_cluster: cluster_id.to_string(),
                expanded: false,
            }),
    );

    while let Some(frame) = stack.pop() {
        work_control.charge_adapter(1)?;

        if frame.expanded {
            nodes.push((frame.node, frame.owner_cluster));
            continue;
        }

        if !visited.insert(frame.node.clone()) {
            continue;
        }

        let child_count_2 = graph.child_count(&frame.node);

        match child_count_2 {
            0 => {
                nodes.push((frame.node, frame.owner_cluster));
                continue;
            },
            _ => {},
        }

        let child_snapshot_2 = child_id_snapshot_work_upper_bound(graph, &frame.node, work_control)?;
        let child_clone_work = work_control.checked_add(child_snapshot_2, child_count_2)?;
        work_control.charge_adapter(work_control.checked_add(child_clone_work, 1)?)?;
        stack.push(CopyFrame {
            node: frame.node.clone(),
            owner_cluster: frame.node.clone(),
            expanded: true,
        });
        stack.extend(
            graph
                .children(&frame.node)
                .into_iter()
                .rev()
                .map(|child| CopyFrame {
                    node: child.to_string(),
                    owner_cluster: frame.node.clone(),
                    expanded: false,
                }),
        );
    }

    for (node, owner_cluster) in nodes {
        work_control.charge_adapter(1)?;

        if !graph.has_node(&node) {
            continue;
        }

        let data = graph.node(&node).cloned().unwrap_or_default();
        charge_node_insertion_ordering(new_graph, &node, work_control)?;
        new_graph.set_node(node.clone(), data);

        if let Some(parent) = graph.parent(&node)
            && parent != root_id
        {
            set_parent_controlled(new_graph, &node, parent, work_control)?;
        }

        if owner_cluster != root_id && node != owner_cluster {
            set_parent_controlled(new_graph, &node, &owner_cluster, work_control)?;
        }

        let edge_snapshot_work =
            work_control.checked_add(graph.edge_slot_count(), graph.edge_count())?;
        work_control.charge_adapter(edge_snapshot_work)?;

        'over_edge_key_2: for edge_key in graph.edge_keys() {
            if !edge_in_cluster(&edge_key, root_id, descendants, work_control)? {
                continue 'over_edge_key_2;
            }

            let label = {
                let optional_label_2 = graph.edge_by_key(&edge_key).cloned();

                match optional_label_2 {
                    Some(matched_label_2) => matched_label_2,
                    ::core::option::Option::None => continue 'over_edge_key_2,
                }
            };

            let root_descendants = descendants.get(root_id).map(Vec::as_slice).unwrap_or(&[]);
            let mut v_in = edge_key.v == root_id;

            if !v_in {
                'over_descendant_3: for descendant in root_descendants {
                    work_control.charge_adapter(1)?;

                    if descendant == &edge_key.v {
                        v_in = true;
                        break 'over_descendant_3;
                    }
                }
            }

            let mut w_in = edge_key.w == root_id;

            if !w_in {
                'over_descendant_3: for descendant in root_descendants {
                    work_control.charge_adapter(1)?;

                    if descendant == &edge_key.w {
                        w_in = true;
                        break 'over_descendant_3;
                    }
                }
            }

            if v_in && w_in {
                if !new_graph.has_edge(&edge_key.v, &edge_key.w, edge_key.name.as_deref()) {
                    charge_edge_endpoint_insertions(
                        new_graph,
                        &edge_key.v,
                        &edge_key.w,
                        work_control,
                    )?;
                    new_graph.set_edge_named(edge_key.v, edge_key.w, edge_key.name, Some(label));
                }
            } else {
                let new_v = if v_in {
                    root_id.to_string()
                } else {
                    edge_key.v
                };
                let new_w = if w_in {
                    root_id.to_string()
                } else {
                    edge_key.w
                };
                charge_edge_endpoint_insertions(graph, &new_v, &new_w, work_control)?;
                graph.set_edge_named(new_v, new_w, edge_key.name, Some(label));
            }
        }

        let removal_work = remove_node_work_upper_bound(graph, &node, work_control)?;
        work_control.charge_adapter(removal_work)?;
        let _ = graph.remove_node(&node);
    }

    Ok(())
}

fn extract_clusters_recursively(
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    subgraphs_by_id: &FlowSubgraphIndex<'_>,
    external_connections_by_id: &std::collections::HashMap<String, bool>,
    extracted: &mut std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
    extracted_order: &mut Vec<String>,
    _depth: usize,
    work_control: &mut DagreOperationWorkControl,
) -> Result<()> {



    struct ExtractFrame {
        id: String,
        graph: Graph<NodeLabel, EdgeLabel, GraphLabel>,
        expanded: bool,
    }

    type ExtractedCluster = (String, Graph<NodeLabel, EdgeLabel, GraphLabel>);

    fn extract_one_level(
        graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        subgraphs_by_id: &FlowSubgraphIndex<'_>,
        external_connections_by_id: &std::collections::HashMap<String, bool>,
        work_control: &mut DagreOperationWorkControl,
    ) -> Result<Vec<ExtractedCluster>> {
        let snapshot_work = node_id_snapshot_work_upper_bound(graph, work_control)?;
        work_control.charge_adapter(snapshot_work)?;
        let node_ids = graph.node_ids();
        let mut descendants = HashMap::new();
        work_control.charge_adapter(node_ids.len())?;

        for id in &node_ids {
            if graph.child_count(id) == 0 {
                continue;
            }

            descendants.insert(id.clone(), { let checked_value = extract_descendants(id, graph, work_control)?; checked_value });
        }

        let mut extracted_here: Vec<ExtractedCluster> = Vec::new();

        work_control.charge_adapter(node_ids.len())?;
        let candidates: Vec<String> = node_ids
            .into_iter()
            .filter(|id| graph.has_node(id))
            .filter(|id| graph.child_count(id) != 0)








            .filter(|id| {
                external_connections_by_id
                    .get(id.as_str())
                    .is_some_and(|external| !external)
            })
            .collect();

        for id in candidates {
            if !graph.has_node(&id) {
                continue;
            }

            if graph.child_count(&id) == 0 {
                continue;
            }

            let mut cluster_graph: Graph<NodeLabel, EdgeLabel, GraphLabel> =
                Graph::new(GraphOptions {
                    multigraph: true,
                    compound: true,
                    directed: true,
                });




            let dir = subgraphs_by_id
                .get(id.as_str())
                .and_then(|sg| sg.dir.as_deref())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(normalize_dir)
                .unwrap_or_else(|| toggled_dir(flow_dir_from_rankdir(graph.graph().rankdir)));

            cluster_graph.set_graph(GraphLabel {
                rankdir: dir_to_rankdir(&dir),











                nodesep: 50.0,
                ranksep: 50.0,
                marginx: 8.0,
                marginy: 8.0,
                acyclicer: None,
                ..Default::default()
            });

            copy_cluster(
                &id,
                graph,
                &mut cluster_graph,
                &id,
                &descendants,
                work_control,
            )?;
            extracted_here.push((id, cluster_graph));
        }

        Ok(extracted_here)
    }

    let extracted_here = extract_one_level(
        graph,
        subgraphs_by_id,
        external_connections_by_id,
        work_control,
    )?;
    let mut stack: Vec<ExtractFrame> = extracted_here
        .into_iter()
        .rev()
        .map(|(id, graph_2)| ExtractFrame {
            id,
            graph: graph_2,
            expanded: false,
        })
        .collect();

    while let Some(mut frame) = stack.pop() {
        if frame.expanded {
            extracted_order.push(frame.id.clone());
            extracted.insert(frame.id, frame.graph);
            continue;
        }

        let children = extract_one_level(
            &mut frame.graph,
            subgraphs_by_id,
            external_connections_by_id,
            work_control,
        )?;
        frame.expanded = true;
        stack.push(frame);
        stack.extend(children.into_iter().rev().map(|(id, graph_3)| ExtractFrame {
            id,
            graph: graph_3,
            expanded: false,
        }));
    }

    Ok(())
}

#[cfg(test)]
pub(crate) fn layout_flowchart_typed(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<FlowchartLayout> {
    layout_flowchart_typed_with_work_meter(
        model,
        effective_config,
        measurer,
        math_renderer,
        Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )),
    )
}

#[cfg(test)]
pub(crate) fn layout_flowchart_typed_with_work_meter(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<FlowchartLayout> {
    let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
        model,
        effective_config,
        false,
        work_meter.as_ref(),
    )?;
    layout_flowchart_typed_with_render_labels_and_svg_label_sidecar_and_work_meter(
        model,
        &FlowchartRenderContext::default(),
        effective_config,
        measurer,
        math_renderer,
        None,
        &edge_style_plan,
        work_meter,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "Layout receives independent label artifacts and cumulative work controls"
)]
pub(crate) fn layout_flowchart_typed_with_render_labels_and_svg_label_sidecar_and_work_meter(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    svg_label_sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &crate::svg::FlowchartEdgeStylePlan,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<FlowchartLayout> {
    let mut work_control = DagreOperationWorkControl::new(work_meter);
    layout_flowchart_with_model(
        model,
        render_label_sources,
        effective_config,
        measurer,
        math_renderer,
        svg_label_sidecar,
        edge_style_plan,
        &mut work_control,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "Layout receives independent label artifacts and cumulative work controls"
)]
fn layout_flowchart_with_model(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    svg_label_sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &crate::svg::FlowchartEdgeStylePlan,
    work_control: &mut DagreOperationWorkControl,
) -> Result<FlowchartLayout> {
    let render_model = FlowchartRenderModelRef::new(model, render_label_sources);
    let model_2 = &render_model;



    work_control.charge_adapter(model_2.nodes.len())?;
    super::validate_flowchart_model_shapes(model_2)?;
    let effective_config_value = effective_config.as_value();




    work_control.charge_adapter(model_2.edges.len())?;
    let mut projected_edges: Vec<FlowEdge> = Vec::with_capacity(model_2.edges.len());
    let mut projected_edge_owner_indices: Vec<usize> = Vec::with_capacity(model_2.edges.len());

    for (edge_index, edge) in model_2.edges.iter().enumerate() {
        {
            match super::project_flowchart_edge(edge, render_label_sources) {
                Some(projected) => {
                    projected_edges.push(projected);
                    projected_edge_owner_indices.push(edge_index);
                },
                ::core::option::Option::None => {},
            };
        }
    }

    work_control.charge_adapter(projected_edges.len())?;




    let self_loop_count = projected_edges.iter().filter(|e| e.from == e.to).count();
    let derived_render_edges = work_control.checked_add(
        projected_edges.len(),
        work_control.checked_mul(self_loop_count, 2)?,
    )?;
    work_control.charge_adapter(derived_render_edges)?;
    let mut render_edges: Vec<std::borrow::Cow<'_, FlowEdge>> =
        Vec::with_capacity(derived_render_edges);
    let mut render_edge_self_loop_meta: Vec<Option<FlowchartSelfLoopSegmentMeta>> =
        Vec::with_capacity(derived_render_edges);
    let mut render_edge_owner_indices = Vec::with_capacity(derived_render_edges);
    let mut self_loop_label_node_ids: Vec<String> = Vec::new();
    let mut self_loop_label_node_id_set: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    let merge_self_loops = super::flowchart_dagre_merges_self_loop_segments(&model_2.keyword);
    let mut unmerged_first_slots = HashMap::new();

    for (projected_index, e) in projected_edges.iter().enumerate() {
        let edge_index = projected_edge_owner_indices[projected_index];

        if e.from != e.to {
            render_edges.push(std::borrow::Cow::Borrowed(e));
            render_edge_self_loop_meta.push(None);
            render_edge_owner_indices.push(edge_index);
            continue;
        }

        let mut helper_edges = super::flowchart_self_loop_helper_edges(e);
        helper_edges.edge_mid.label = model_2
            .edge_label_for_render(edge_index, e)
            .map(str::to_owned);

        if self_loop_label_node_id_set.insert(helper_edges.special_id_1.clone()) {
            self_loop_label_node_ids.push(helper_edges.special_id_1.clone());
        }

        if self_loop_label_node_id_set.insert(helper_edges.special_id_2.clone()) {
            self_loop_label_node_ids.push(helper_edges.special_id_2.clone());
        }



        let first_slot = if merge_self_loops {
            None
        } else {
            work_control.charge_adapter(1)?;


            Some(
                *unmerged_first_slots
                    .entry(e.from.as_str())
                    .or_insert(render_edges.len()),
            )
        };

        for (order, segment) in [
            helper_edges.edge1,
            helper_edges.edge_mid,
            helper_edges.edge2,
        ]
        .into_iter()
        .enumerate()
        {
            let meta = Some(FlowchartSelfLoopSegmentMeta {
                logical_edge_id: e.id.clone(),
                owner_index: edge_index,
                node_id: e.from.clone(),
                order: order as u8,
            });
            let slot = first_slot
                .map(|first| work_control.checked_add(first, order))
                .transpose()?
                .unwrap_or(render_edges.len());

            if slot == render_edges.len() {
                render_edges.push(std::borrow::Cow::Owned(segment));
                render_edge_owner_indices.push(edge_index);
                render_edge_self_loop_meta.push(meta);
            } else {
                render_edges[slot] = std::borrow::Cow::Owned(segment);
                render_edge_owner_indices[slot] = edge_index;
                render_edge_self_loop_meta[slot] = meta;
            }
        }
    }

    let FlowchartLayoutSettings {
        nodesep,
        ranksep,
        node_padding,
        state_padding,
        wrapping_width,
        edge_label_wrapping_width,
        cluster_title_wrapping_width,
        edge_html_labels,
        node_wrap_mode,
        edge_wrap_mode,
        cluster_wrap_mode,
        cluster_padding,
        title_margin_top,
        title_margin_bottom,
        title_total_margin,
        y_shift,
        inherit_dir,
        text_style,
        html_label_text_style,
    } = svg_label_sidecar.map_or_else(
        || FlowchartConfigView::new(effective_config_value).layout_settings(),
        |sidecar| sidecar.layout_settings(effective_config_value),
    );
    let edge_label_padding = svg_label_sidecar.map_or_else(
        super::FlowchartEdgeLabelPadding::default,
        FlowchartSvgLabelSidecarBuilder::edge_label_padding,
    );
    let look_is_neo = crate::config::config_diagram_look(effective_config_value).is_neo();
    let node_label_base_style = if node_wrap_mode == WrapMode::HtmlLike {
        &html_label_text_style
    } else {
        &text_style
    };
    let label_typography = svg_label_sidecar.map_or_else(
        super::FlowchartLabelTypography::default,
        FlowchartSvgLabelSidecarBuilder::label_typography,
    );
    let cluster_label_base_style = if cluster_wrap_mode == WrapMode::HtmlLike {
        &html_label_text_style
    } else {
        &text_style
    };
    let edge_label_base_style = if edge_wrap_mode == WrapMode::HtmlLike {
        &html_label_text_style
    } else {
        &text_style
    };

    let diagram_direction = normalize_dir(model_2.direction.as_deref().unwrap_or("TB"));
    let has_subgraphs = !model_2.subgraphs.is_empty();
    work_control.charge_adapter(model_2.subgraphs.len())?;



    let mut subgraphs_by_id: FlowSubgraphIndex<'_> = HashMap::with_capacity(model_2.subgraphs.len());
    let mut subgraph_index_by_id: HashMap<&str, usize> =
        HashMap::with_capacity(model_2.subgraphs.len());
    let mut canonical_subgraphs_in_order: Vec<(usize, &FlowSubgraph)> = Vec::new();
    let mut subgraph_members_by_id: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut nonempty_subgraph_ids: HashSet<&str> = HashSet::new();

    for (index, subgraph) in model_2.subgraphs.iter().enumerate() {
        let id = subgraph.id.as_str();

        if !subgraphs_by_id.contains_key(id) {
            subgraphs_by_id.insert(id, subgraph);
            subgraph_index_by_id.insert(id, index);
            canonical_subgraphs_in_order.push((index, subgraph));
        }

        subgraph_members_by_id
            .entry(id)
            .or_default()
            .extend(subgraph.nodes.iter().map(String::as_str));

        if !subgraph.nodes.is_empty() {
            nonempty_subgraph_ids.insert(id);
        }
    }

    work_control.charge_adapter(model_2.subgraphs.len())?;
    let subgraph_ids: std::collections::HashSet<&str> =
        model_2.subgraphs.iter().map(|sg| sg.id.as_str()).collect();
    let collapsed_subgraph_ids: HashSet<&str> = model_2
        .subgraphs
        .iter()
        .filter(|sg| model_2.is_subgraph_collapsed(sg.id.as_str()))
        .filter(|sg| model_2.collapsed_replacement(sg.id.as_str()).is_none())
        .map(|sg| sg.id.as_str())
        .collect();



    let cluster_subgraph_ids: HashSet<&str> = model_2
        .subgraphs
        .iter()
        .filter(|sg| !collapsed_subgraph_ids.contains(sg.id.as_str()))
        .filter(|sg| model_2.collapsed_replacement(sg.id.as_str()).is_none())
        .filter(|sg| !sg.nodes.is_empty())
        .map(|sg| sg.id.as_str())
        .collect();
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,


        compound: true,
        directed: true,
    });
    g.set_graph(GraphLabel {
        rankdir: rank_dir_from_flow(&diagram_direction),
        nodesep,
        ranksep,
        marginx: 8.0,
        marginy: 8.0,
        acyclicer: None,
        ..Default::default()
    });

    let mut empty_subgraph_ids: Vec<String> = Vec::new();
    let mut cluster_node_labels: std::collections::HashMap<String, NodeLabel> =
        std::collections::HashMap::new();
    work_control.charge_adapter(canonical_subgraphs_in_order.len())?;

    for (_, sg) in &canonical_subgraphs_in_order {
        if model_2.collapsed_replacement(sg.id.as_str()).is_some() {
            continue;
        }

        if collapsed_subgraph_ids.contains(sg.id.as_str()) {
            continue;
        }

        if !nonempty_subgraph_ids.contains(sg.id.as_str()) {


            empty_subgraph_ids.push(sg.id.clone());
            continue;
        }




        cluster_node_labels.insert(sg.id.clone(), NodeLabel::default());
    }

    let mut leaf_node_labels: std::collections::HashMap<String, NodeLabel> =
        std::collections::HashMap::new();
    let mut leaf_label_metrics_by_id: HashMap<String, (f64, f64)> = HashMap::new();
    let collapsed_leaf_count = collapsed_subgraph_ids.len();
    let leaf_label_capacity = work_control.checked_add(
        model_2.nodes.len(),
        work_control.checked_add(empty_subgraph_ids.len(), collapsed_leaf_count)?,
    )?;
    work_control.charge_adapter(leaf_label_capacity)?;
    leaf_label_metrics_by_id.reserve(leaf_label_capacity);
    work_control.charge_adapter(model_2.nodes.len())?;

    for (node_index, n) in model_2.nodes.iter().enumerate() {



        if subgraph_ids.contains(n.id.as_str())
            || model_2.collapsed_replacement(n.id.as_str()).is_some()
        {
            continue;
        }

        let raw_label = model_2.node_label_for_render(n).unwrap_or(&n.id);
        let wrapping_width_2 = super::bpmn::wrapping_width(n.layout_shape.as_deref(), wrapping_width);
        let label_type = n.label_type.as_deref().unwrap_or("text");
        let label_dom = (
            edge_html_labels,
            super::FlowchartLabelDom::node(
                n.layout_shape.as_deref().unwrap_or("squareRect"),
                raw_label,
                label_type,
                node_wrap_mode == WrapMode::HtmlLike,
                crate::config::config_diagram_look(effective_config_value).is_hand_drawn(),
            ),
        );
        let structural = super::FlowchartStructuralLabelTypography::prepare(
            &model_2.class_defs,
            label_dom.0,
            label_dom.1,
            |units| work_control.charge_adapter(units),
        )?;
        let node_parent = label_typography.node_source_parent_style(
            node_label_base_style,
            &model_2.class_defs,
            (&n.classes, &n.styles, true),
            &structural,
        );
        let mut node_text_style = flowchart_effective_text_style_for_node_classes_with_provenance(
            node_parent.as_ref(),
            &model_2.class_defs,
            &n.classes,
            &n.styles,
        );
        structural.apply_to_content(&mut node_text_style.style);
        let svg_width_mode = flowchart_node_svg_width_mode(
            raw_label,
            label_type,
            node_wrap_mode,
            n.layout_shape.as_deref().unwrap_or("squareRect"),
        );
        let mut metrics = measure_flowchart_svg_label_for_layout_with_terminal_foreground(
            svg_label_sidecar,
            Some(FlowchartSvgLabelOwner::Node(node_index)),
            Some(n.id.as_str()),
            FlowchartLabelMetricsRequest {
                measurer,
                raw_label,
                label_type,
                style: node_text_style.as_ref(),
                max_width_px: Some(wrapping_width_2),
                wrap_mode: node_wrap_mode,
                config: effective_config,
                math_renderer,
            },
            node_text_style.terminal_foreground(),
            svg_width_mode,
        );

        if node_wrap_mode == WrapMode::HtmlLike && edge_html_labels {
            flowchart_apply_html_node_class_box_metrics(
                &mut metrics,
                raw_label,
                label_type,
                node_text_style.as_ref(),
                &model_2.class_defs,
                &n.classes,
            );
        }

        metrics = metrics.with_label_min_width(
            raw_label,
            super::flowchart_node_label_min_width(
                raw_label,
                n.layout_shape.as_deref(),
                effective_config,
            ),
            None,
        );
        leaf_label_metrics_by_id.insert(n.id.clone(), (metrics.width, metrics.height));
        let (width, height) = node_layout_dimensions(NodeLayoutDimensionsRequest {
            layout_shape: n.layout_shape.as_deref(),
            layout_direction: &diagram_direction,
            metrics,
            has_label: !raw_label.is_empty(),
            wrapping_width: wrapping_width_2,
            node_constraint: n.constraint.as_deref(),
            padding: node_padding,
            look_is_neo,
            state_padding,
            node_icon: n.icon.as_deref(),
            node_img: n.img.as_deref(),
            node_pos: n.pos.as_deref(),
            node_asset_width: n.asset_width,
            node_asset_height: n.asset_height,
        });
        leaf_node_labels.insert(
            n.id.clone(),
            NodeLabel {
                width,
                height,
                ..Default::default()
            },
        );
    }

    work_control.charge_adapter(canonical_subgraphs_in_order.len())?;

    for &(subgraph_index, sg) in &canonical_subgraphs_in_order {
        if nonempty_subgraph_ids.contains(sg.id.as_str())
            || collapsed_subgraph_ids.contains(sg.id.as_str())
            || model_2.collapsed_replacement(sg.id.as_str()).is_some()
        {
            continue;
        }

        let label_type = sg.label_type.as_deref().unwrap_or("text");
        let (classes, styles) = model_2.effective_subgraph_css(subgraph_index, sg);
        let title = model_2.subgraph_title_for_render(subgraph_index, sg);
        let label_dom = (
            edge_html_labels,
            super::FlowchartLabelDom::node(
                "squareRect",
                title,
                label_type,
                node_wrap_mode == WrapMode::HtmlLike,
                crate::config::config_diagram_look(effective_config_value).is_hand_drawn(),
            ),
        );
        let structural = super::FlowchartStructuralLabelTypography::prepare(
            &model_2.class_defs,
            label_dom.0,
            label_dom.1,
            |units| work_control.charge_adapter(units),
        )?;
        let parent_style = label_typography.node_source_parent_style(
            cluster_label_base_style,
            &model_2.class_defs,
            (classes, styles, false),
            &structural,
        );
        let mut sg_text_style = flowchart_effective_text_style_for_classes_with_provenance(
            parent_style.as_ref(),
            &model_2.class_defs,
            classes,
            styles,
        );
        structural.apply_to_content(&mut sg_text_style.style);




        let mut metrics = measure_flowchart_svg_label_for_layout_with_terminal_foreground(
            svg_label_sidecar,
            Some(FlowchartSvgLabelOwner::EmptySubgraphNode(subgraph_index)),
            Some(sg.id.as_str()),
            FlowchartLabelMetricsRequest {
                measurer,
                raw_label: title,
                label_type,
                style: sg_text_style.as_ref(),
                max_width_px: Some(wrapping_width),
                wrap_mode: node_wrap_mode,
                config: effective_config,
                math_renderer,
            },
            sg_text_style.terminal_foreground(),
            FlowchartSvgWidthMode::Bbox,
        );

        if node_wrap_mode == WrapMode::HtmlLike && edge_html_labels {
            flowchart_apply_html_node_class_box_metrics(
                &mut metrics,
                title,
                label_type,
                sg_text_style.as_ref(),
                &model_2.class_defs,
                classes,
            );
        }

        leaf_label_metrics_by_id.insert(sg.id.clone(), (metrics.width, metrics.height));
        let (width, height) = node_layout_dimensions(NodeLayoutDimensionsRequest {
            layout_shape: Some("squareRect"),
            layout_direction: &diagram_direction,
            metrics,
            has_label: !title.is_empty(),
            wrapping_width,
            node_constraint: None,
            padding: cluster_padding,
            look_is_neo: false,
            state_padding,
            node_icon: None,
            node_img: None,
            node_pos: None,
            node_asset_width: None,
            node_asset_height: None,
        });
        leaf_node_labels.insert(
            sg.id.clone(),
            NodeLabel {
                width,
                height,
                ..Default::default()
            },
        );
    }




    work_control.charge_adapter(collapsed_leaf_count)?;

    for &(subgraph_index, sg) in &canonical_subgraphs_in_order {
        if !collapsed_subgraph_ids.contains(sg.id.as_str()) {
            continue;
        }

        let label_type = sg.label_type.as_deref().unwrap_or("text");
        let (classes, styles) = model_2.effective_subgraph_css(subgraph_index, sg);
        let title = model_2.subgraph_title_for_render(subgraph_index, sg);
        let label_dom = (
            edge_html_labels,
            super::FlowchartLabelDom::node(
                "collapsedGroup",
                title,
                label_type,
                node_wrap_mode == WrapMode::HtmlLike,
                crate::config::config_diagram_look(effective_config_value).is_hand_drawn(),
            ),
        );
        let structural = super::FlowchartStructuralLabelTypography::prepare(
            &model_2.class_defs,
            label_dom.0,
            label_dom.1,
            |units| work_control.charge_adapter(units),
        )?;
        let base_style = if node_wrap_mode == WrapMode::HtmlLike {
            &html_label_text_style
        } else {
            &text_style
        };
        let base_style_2 = label_typography.node_source_parent_style(
            base_style,
            &model_2.class_defs,
            (classes, styles, false),
            &structural,
        );
        let mut text_style_for_label = flowchart_effective_text_style_for_classes(
            base_style_2.as_ref(),
            &model_2.class_defs,
            classes,
            styles,
        );
        structural.apply_to_content(&mut text_style_for_label);
        let mut metrics = measure_flowchart_svg_label_for_layout_with_terminal_foreground(
            svg_label_sidecar,
            Some(FlowchartSvgLabelOwner::SubgraphTitle(subgraph_index)),
            Some(sg.id.as_str()),
            FlowchartLabelMetricsRequest {
                measurer,
                raw_label: title,
                label_type,
                style: text_style_for_label.as_ref(),
                max_width_px: Some(wrapping_width),
                wrap_mode: node_wrap_mode,
                config: effective_config,
                math_renderer,
            },
            None,
            FlowchartSvgWidthMode::Bbox,
        );

        if node_wrap_mode == WrapMode::HtmlLike && edge_html_labels {
            flowchart_apply_html_node_class_box_metrics(
                &mut metrics,
                title,
                label_type,
                text_style_for_label.as_ref(),
                &model_2.class_defs,
                classes,
            );
        }

        leaf_label_metrics_by_id.insert(sg.id.clone(), (metrics.width, metrics.height));
        leaf_node_labels.insert(
            sg.id.clone(),
            NodeLabel {
                width: (metrics.width + 16.0).max(80.0),
                height: metrics.height + 44.0,
                ..Default::default()
            },
        );
    }









    let mut inserted: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut parent_assigned: std::collections::HashSet<String> = std::collections::HashSet::new();
    let insert_one = |id: &str,
                      g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
                      inserted_2: &mut std::collections::HashSet<String>,
                      work_control_2: &mut DagreOperationWorkControl|
     -> Result<()> {
        if inserted_2.contains(id) {
            return Ok(());
        }

        match cluster_node_labels.get(id).cloned() {
            Some(lbl) => {
                charge_node_insertion_ordering(g, id, work_control_2)?;
                g.set_node(id.to_string(), lbl);
                inserted_2.insert(id.to_string());
                return Ok(());
            },
            ::core::option::Option::None => {},
        }

        match leaf_node_labels.get(id).cloned() {
            Some(lbl) => {
                charge_node_insertion_ordering(g, id, work_control_2)?;
                g.set_node(id.to_string(), lbl);
                inserted_2.insert(id.to_string());
            },
            ::core::option::Option::None => {},
        }

        Ok(())
    };

    let mut existing_parent_count = 0usize;

    if has_subgraphs {


        let mut parent_by_id: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let membership_count = model_2.subgraphs.iter().try_fold(0usize, |total, subgraph| {
            work_control.checked_add(total, subgraph.nodes.len())
        })?;
        let hierarchy_validation_work =
            work_control.checked_add(membership_count, model_2.subgraphs.len())?;
        work_control.charge_adapter(hierarchy_validation_work)?;

        for sg in model_2.subgraphs.iter().rev() {
            for child in &sg.nodes {
                parent_by_id.insert(child.clone(), sg.id.clone());
            }
        }

        let insert_with_parent = |id: &str,
                                  g_2: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
                                  inserted_3: &mut std::collections::HashSet<String>,
                                  parent_assigned_2: &mut std::collections::HashSet<String>,
                                  parent_assignments: &mut Vec<(usize, usize)>,
                                  numeric_parent_assignments: &mut usize,
                                  work_control_3: &mut DagreOperationWorkControl|
         -> Result<()> {
            insert_one(id, g_2, inserted_3, work_control_3)?;

            if !parent_assigned_2.insert(id.to_string()) {
                return Ok(());
            }

            match parent_by_id.get(id) {
                Some(parent) => {
                    charge_node_insertion_ordering(g_2, parent, work_control_3)?;
                    g_2.ensure_node_ref(parent);

                    {
                        match (g_2.node_ix(id), g_2.node_ix(parent)) {
                            (Some(child_ix), Some(parent_ix)) => {
                                parent_assignments.push((child_ix, parent_ix));
                                *numeric_parent_assignments += usize::from(is_javascript_array_index(id));
                            },
                            (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {},
                        };
                    }
                },
                ::core::option::Option::None => {},
            }

            Ok(())
        };

        let insertion_visits = work_control.checked_add(
            work_control.checked_add(model_2.subgraphs.len(), model_2.nodes.len())?,
            model_2.vertex_calls.len(),
        )?;
        let insertion_work = work_control.checked_add(insertion_visits, parent_by_id.len())?;
        work_control.charge_adapter(insertion_work)?;
        let mut parent_assignments = Vec::with_capacity(parent_by_id.len());
        let mut numeric_parent_assignments = 0usize;

        for sg in model_2.subgraphs.iter().rev() {
            if model_2.collapsed_replacement(sg.id.as_str()).is_some() {
                continue;
            }

            insert_with_parent(
                sg.id.as_str(),
                &mut g,
                &mut inserted,
                &mut parent_assigned,
                &mut parent_assignments,
                &mut numeric_parent_assignments,
                work_control,
            )?;
        }

        for n in &model_2.nodes {
            if model_2.collapsed_replacement(n.id.as_str()).is_some() {
                continue;
            }

            insert_with_parent(
                n.id.as_str(),
                &mut g,
                &mut inserted,
                &mut parent_assigned,
                &mut parent_assignments,
                &mut numeric_parent_assignments,
                work_control,
            )?;
        }

        for id in &model_2.vertex_calls {
            if model_2.collapsed_replacement(id.as_str()).is_some() {
                continue;
            }

            insert_with_parent(
                id.as_str(),
                &mut g,
                &mut inserted,
                &mut parent_assigned,
                &mut parent_assignments,
                &mut numeric_parent_assignments,
                work_control,
            )?;
        }

        apply_unparented_parent_assignments(
            &mut g,
            &parent_assignments,
            UnparentedParentBatchShape {
                existing_parent_count: 0,
                numeric_assignment_count: numeric_parent_assignments,
            },
            work_control,
        )?;
        existing_parent_count = parent_assignments.len();
    } else {

        let insertion_work =
            work_control.checked_add(model_2.nodes.len(), model_2.vertex_calls.len())?;
        work_control.charge_adapter(insertion_work)?;

        for n in &model_2.nodes {
            insert_one(n.id.as_str(), &mut g, &mut inserted, work_control)?;
        }

        for id in &model_2.vertex_calls {
            insert_one(id.as_str(), &mut g, &mut inserted, work_control)?;
        }
    }



    work_control.charge_adapter(self_loop_label_node_ids.len())?;
    let mut self_loop_parent_assignments = Vec::new();
    let mut numeric_self_loop_parent_assignments = 0usize;

    for id in &self_loop_label_node_ids {
        if !g.has_node(id) {
            charge_node_insertion_ordering(&g, id, work_control)?;
            g.set_node(
                id.clone(),
                NodeLabel {




                    width: <f64 as ::core::convert::From<::core::primitive::f32>>::from(0.1_f32),
                    height: <f64 as ::core::convert::From<::core::primitive::f32>>::from(0.1_f32),
                    ..Default::default()
                },
            );
        }

        let base = {
            let optional_base = id.split_once("---");

            match optional_base {
                Some((matched_base, _)) => matched_base,
                ::core::option::Option::None => continue,
            }
        };

        {
            match g.parent(base) {
                Some(p) => {
                    let child_ix = g
                        .node_ix(id)
                        .expect("a self-loop helper node is present after insertion");
                    let parent_ix = g
                        .node_ix(p)
                        .expect("the base node parent is present in the layout graph");
                    self_loop_parent_assignments.push((child_ix, parent_ix));
                    numeric_self_loop_parent_assignments += usize::from(is_javascript_array_index(id));
                },
                ::core::option::Option::None => {},
            };
        }
    }

    apply_unparented_parent_assignments(
        &mut g,
        &self_loop_parent_assignments,
        UnparentedParentBatchShape {
            existing_parent_count,
            numeric_assignment_count: numeric_self_loop_parent_assignments,
        },
        work_control,
    )?;

    let effective_dir_by_id = if has_subgraphs {
        compute_effective_dir_by_id(
            &model_2.subgraphs,
            &subgraphs_by_id,
            &g,
            &diagram_direction,
            inherit_dir,
            work_control,
        )?
    } else {
        HashMap::new()
    };

    work_control.charge_adapter(render_edges.len())?;

    for ((e, self_loop_meta), edge_owner_index) in render_edges
        .iter()
        .zip(&render_edge_self_loop_meta)
        .zip(&render_edge_owner_indices)
    {


        let edge_key = flowchart_layout_edge_key(e, self_loop_meta.as_ref());

        let from = e.from.clone();
        let to = e.to.clone();

        if edge_label_is_non_empty(model_2, *edge_owner_index, e, self_loop_meta.as_ref()) {
            let label_text =
                match render_edge_label_for_layout(model_2, *edge_owner_index, e, self_loop_meta.as_ref()) {
                    ::core::option::Option::Some(explicit_present) => explicit_present,
                    ::core::option::Option::None => "",
                };
            let label_type = e.label_type.as_deref().unwrap_or("text");
            let edge_key_identity = crate::flowchart::FlowchartEdgeKey::new(*edge_owner_index);
            let edge_parent = edge_style_plan.label_parent_style_for(
                edge_key_identity,
                edge_label_base_style,
                &label_typography,
            )?;
            let edge_label_base_style_2 = edge_parent.as_ref();
            let edge_text_style = edge_style_plan
                .edge_label_text_style_for(edge_key_identity, edge_label_base_style_2)?;
            let metrics = if label_type == "markdown" && edge_wrap_mode != WrapMode::HtmlLike {
                crate::text::measure_wrapped_markdown_with_inline_styles(
                    measurer,
                    label_text,
                    edge_text_style.as_ref(),
                    Some(edge_label_wrapping_width),
                    edge_wrap_mode,
                )
            } else if edge_wrap_mode == WrapMode::SvgLike {
                let render_id = self_loop_meta
                    .as_ref()
                    .map_or(e.id.as_str(), |meta| meta.logical_edge_id.as_str());
                measure_flowchart_svg_label_for_layout_with_metrics_style_and_terminal_foreground(
                    svg_label_sidecar,
                    Some(FlowchartSvgLabelOwner::Edge(*edge_owner_index)),
                    Some(render_id),
                    FlowchartLabelMetricsRequest {
                        measurer,
                        raw_label: label_text,
                        label_type,

                        style: edge_label_base_style_2,
                        max_width_px: Some(edge_label_wrapping_width),
                        wrap_mode: edge_wrap_mode,
                        config: effective_config,
                        math_renderer,
                    },
                    edge_text_style.as_ref(),
                    edge_text_style.terminal_foreground(),
                    FlowchartSvgWidthMode::Bbox,
                )
            } else {
                measure_flowchart_svg_label_for_layout_with_terminal_foreground(
                    svg_label_sidecar,
                    Some(FlowchartSvgLabelOwner::Edge(*edge_owner_index)),
                    Some(e.id.as_str()),
                    FlowchartLabelMetricsRequest {
                        measurer,
                        raw_label: label_text,
                        label_type,
                        style: edge_text_style.as_ref(),
                        max_width_px: Some(edge_label_wrapping_width),
                        wrap_mode: edge_wrap_mode,
                        config: effective_config,
                        math_renderer,
                    },
                    edge_text_style.terminal_foreground(),
                    FlowchartSvgWidthMode::Bbox,
                )
            };
            let (label_width, label_height) = if edge_html_labels {
                (metrics.width.max(1.0), metrics.height.max(1.0))
            } else {


                (
                    (metrics.width + 4.0).max(1.0),
                    (metrics.height + 4.0).max(1.0),
                )
            };
            let (label_width_2, label_height_2) =
                edge_label_padding.padded_size(label_width, label_height);

            let minlen = e.length.max(1);
            let mut el = EdgeLabel {
                width: label_width_2,
                height: label_height_2,
                labelpos: LabelPos::C,

                labeloffset: 10.0,
                minlen,
                weight: 1.0,
                ..Default::default()
            };

            match self_loop_meta {
                Some(meta) => {
                    annotate_flowchart_self_loop_segment(&mut el, meta);
                },
                ::core::option::Option::None => {},
            }

            annotate_flowchart_edge_owner(&mut el, *edge_owner_index);

            g.set_edge_named(from, to, Some(edge_key), Some(el));
        } else {
            let mut el = EdgeLabel {
                width: 0.0,
                height: 0.0,
                labelpos: LabelPos::C,

                labeloffset: 10.0,
                minlen: e.length.max(1),
                weight: 1.0,
                ..Default::default()
            };

            match self_loop_meta {
                Some(meta) => {
                    annotate_flowchart_self_loop_segment(&mut el, meta);
                },
                ::core::option::Option::None => {},
            }

            annotate_flowchart_edge_owner(&mut el, *edge_owner_index);
            g.set_edge_named(from, to, Some(edge_key), Some(el));
        }
    }

    let external_connections_by_id = if has_subgraphs {
        adjust_flowchart_clusters_and_edges(&mut g, work_control)?
    } else {
        std::collections::HashMap::new()
    };

    let mut edge_endpoints_by_segment: HashMap<FlowchartLayoutSegmentKey, (String, String)> =
        HashMap::new();
    let mut edge_key_by_segment: HashMap<FlowchartLayoutSegmentKey, String> = HashMap::new();
    let edge_snapshot_work = work_control.checked_add(g.edge_slot_count(), g.edge_count())?;
    work_control.charge_adapter(edge_snapshot_work)?;

    for ek in g.edge_keys() {
        let edge_key = {
            let optional_edge_key = ek.name.as_deref();

            match optional_edge_key {
                Some(matched_edge_key) => matched_edge_key,
                ::core::option::Option::None => continue,
            }
        };

        let label = {
            let optional_label = g.edge_by_key(&ek);

            match optional_label {
                Some(matched_label) => matched_label,
                ::core::option::Option::None => continue,
            }
        };

        let segment_key = {
            let optional_segment_key = flowchart_layout_segment_key(label);

            match optional_segment_key {
                Some(matched_segment_key) => matched_segment_key,
                ::core::option::Option::None => continue,
            }
        };

        edge_key_by_segment.insert(segment_key, edge_key.to_string());
        edge_endpoints_by_segment.insert(segment_key, (ek.v.clone(), ek.w.clone()));
    }

    let mut extracted_graphs: std::collections::HashMap<
        String,
        Graph<NodeLabel, EdgeLabel, GraphLabel>,
    > = std::collections::HashMap::new();
    let mut extracted_order = Vec::new();

    if has_subgraphs {
        extract_clusters_recursively(
            &mut g,
            &subgraphs_by_id,
            &external_connections_by_id,
            &mut extracted_graphs,
            &mut extracted_order,
            0,
            work_control,
        )?;


        let edge_snapshot_work_2 = work_control.checked_add(g.edge_slot_count(), g.edge_count())?;
        work_control.charge_adapter(edge_snapshot_work_2)?;
        edge_key_by_segment.clear();
        edge_endpoints_by_segment.clear();

        for ek in g.edge_keys() {
            let edge_key = {
                let optional_edge_key = ek.name.as_deref();

                match optional_edge_key {
                    Some(matched_edge_key) => matched_edge_key,
                    ::core::option::Option::None => continue,
                }
            };

            let label = {
                let optional_label = g.edge_by_key(&ek);

                match optional_label {
                    Some(matched_label) => matched_label,
                    ::core::option::Option::None => continue,
                }
            };

            let segment_key = {
                let optional_segment_key = flowchart_layout_segment_key(label);

                match optional_segment_key {
                    Some(matched_segment_key) => matched_segment_key,
                    ::core::option::Option::None => continue,
                }
            };

            edge_key_by_segment.insert(segment_key, edge_key.to_string());
            edge_endpoints_by_segment.insert(segment_key, (ek.v, ek.w));
        }
    }





    let mut dom_node_order_by_root: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut dom_order_work = node_id_snapshot_work_upper_bound(&g, work_control)?;

    for id in &extracted_order {
        let graph = extracted_graphs
            .get(id)
            .expect("the extraction order references an extracted graph");
        let graph_work = node_id_snapshot_work_upper_bound(graph, work_control)?;
        dom_order_work = work_control.checked_add(dom_order_work, graph_work)?;
    }

    work_control.charge_adapter(dom_order_work)?;
    dom_node_order_by_root.insert(String::new(), g.node_ids());

    for id in &extracted_order {
        let cg = extracted_graphs
            .get(id)
            .expect("the extraction order references an extracted graph");
        dom_node_order_by_root.insert(id.clone(), cg.node_ids());
    }

    type Rect = crate::geom::Box2;

    struct ClusterTitleMetricsContext<'a> {
        model: &'a FlowchartRenderModelRef<'a>,
        subgraphs_by_id: &'a FlowSubgraphIndex<'a>,
        subgraph_index_by_id: &'a HashMap<&'a str, usize>,
        class_defs: &'a indexmap::IndexMap<String, Vec<String>>,
        measurer: &'a dyn TextMeasurer,
        text_style: &'a TextStyle,
        html_label_text_style: &'a TextStyle,
        title_wrapping_width: f64,
        wrap_mode: WrapMode,
        config: &'a PresentationConfig,
        math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
        svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
    }

    #[must_use]
    fn cluster_title_metrics_for_layout(
        id: &str,
        ctx: &ClusterTitleMetricsContext<'_>,
    ) -> Option<(f64, f64)> {
        let sg = ctx.subgraphs_by_id.get(id)?;
        let subgraph_index = ctx.subgraph_index_by_id.get(id).copied()?;
        let title = ctx.model.subgraph_title_for_render(subgraph_index, sg);
        let label_type = sg.label_type.as_deref().unwrap_or("text");
        let title_width_limit = (label_type == "markdown").then_some(ctx.title_wrapping_width);
        let base_style = if ctx.wrap_mode == WrapMode::HtmlLike {
            ctx.html_label_text_style
        } else {
            ctx.text_style
        };
        let (classes, styles) = ctx.model.effective_subgraph_css(subgraph_index, sg);
        let text_style = flowchart_effective_text_style_for_classes_with_provenance(
            base_style,
            ctx.class_defs,
            classes,
            styles,
        );
        let owner = Some(FlowchartSvgLabelOwner::SubgraphTitle(subgraph_index));
        let metrics = measure_flowchart_svg_label_for_layout_with_terminal_foreground(
            ctx.svg_label_sidecar,
            owner,
            Some(id),
            FlowchartLabelMetricsRequest {
                measurer: ctx.measurer,
                raw_label: title,
                label_type,
                style: text_style.as_ref(),
                max_width_px: title_width_limit,
                wrap_mode: ctx.wrap_mode,
                config: ctx.config,
                math_renderer: ctx.math_renderer,
            },
            text_style.terminal_foreground(),
            FlowchartSvgWidthMode::Bbox,
        );
        Some((metrics.width.max(1.0), metrics.height.max(1.0)))
    }

    fn extracted_graph_bbox_rect(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        title_total_margin: f64,
        extracted: &std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
        subgraph_id_set: &std::collections::HashSet<&str>,
        title_metrics_ctx: &ClusterTitleMetricsContext<'_>,
        cluster_padding: f64,
        work_control: &mut DagreOperationWorkControl,
    ) -> Result<Option<Rect>> {
        fn graph_content_rect(
            g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
            extracted: &std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
            subgraph_id_set: &std::collections::HashSet<&str>,
            title_total_margin: f64,
            title_metrics_ctx: &ClusterTitleMetricsContext<'_>,
            cluster_padding: f64,
            work_control: &mut DagreOperationWorkControl,
        ) -> Result<Option<Rect>> {
            let node_snapshot_work = node_id_snapshot_work_upper_bound(g, work_control)?;
            work_control.charge_adapter(node_snapshot_work)?;
            let mut out: Option<Rect> = None;

            for id in g.node_ids() {
                let n = {
                    let optional_n = g.node(&id);

                    match optional_n {
                        Some(matched_n) => matched_n,
                        ::core::option::Option::None => continue,
                    }
                };

                let (x, y) = {
                    let outcome_x = (n.x, n.y);

                    match outcome_x {
                        (Some(matched_x), Some(matched_y)) => (matched_x, matched_y),
                        (::core::option::Option::None, _)
                        | (_, ::core::option::Option::None) => continue,
                    }
                };

                let mut width = n.width;
                let mut height = n.height;
                let is_cluster_node = extracted.contains_key(&id) && g.child_count(&id) == 0;
                let is_non_recursive_cluster =
                    subgraph_id_set.contains(id.as_str()) && g.child_count(&id) != 0;







                if !is_cluster_node && is_non_recursive_cluster {
                    if title_total_margin > 0.0 {
                        height = (height + title_total_margin).max(1.0);
                    }

                    {
                        match cluster_title_metrics_for_layout(&id, title_metrics_ctx) {
                            Some((title_w, title_h)) => {
                                width = width.max(title_w + cluster_padding);
                                height = height.max(title_h + title_total_margin);
                            },
                            ::core::option::Option::None => {},
                        };
                    }
                }

                let r = Rect::from_center(x, y, width, height);

                match out {
                    Some(ref mut cur) => {
                        cur.union(r);
                    },
                    ::core::option::Option::None => {
                        out = Some(r);
                    },
                }
            }

            let edge_snapshot_work =
                work_control.checked_add(g.edge_slot_count(), g.edge_count())?;
            work_control.charge_adapter(edge_snapshot_work)?;

            for ek in g.edge_keys() {
                let e = {
                    let optional_e = g.edge_by_key(&ek);

                    match optional_e {
                        Some(matched_e) => matched_e,
                        ::core::option::Option::None => continue,
                    }
                };

                let (x, y) = {
                    let outcome_x = (e.x, e.y);

                    match outcome_x {
                        (Some(matched_x), Some(matched_y)) => (matched_x, matched_y),
                        (::core::option::Option::None, _)
                        | (_, ::core::option::Option::None) => continue,
                    }
                };

                if e.width <= 0.0 && e.height <= 0.0 {
                    continue;
                }

                let r = Rect::from_center(x, y, e.width, e.height);

                match out {
                    Some(ref mut cur) => {
                        cur.union(r);
                    },
                    ::core::option::Option::None => {
                        out = Some(r);
                    },
                }
            }

            Ok(out)
        }

        graph_content_rect(
            g,
            extracted,
            subgraph_id_set,
            title_total_margin,
            title_metrics_ctx,
            cluster_padding,
            work_control,
        )
    }

    fn apply_mermaid_subgraph_title_shifts(
        graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        extracted: &std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
        subgraph_id_set: &std::collections::HashSet<&str>,
        y_shift: f64,
    ) {
        if y_shift.abs() < 1e-9 {
            return;
        }





        for id in graph.node_ids() {



            let is_cluster_node = extracted.contains_key(&id) && graph.child_count(&id) == 0;
            let delta_y = if is_cluster_node {
                y_shift * 2.0
            } else if subgraph_id_set.contains(id.as_str()) && graph.child_count(&id) != 0 {
                0.0
            } else {
                y_shift
            };

            if delta_y.abs() > 1e-9 {
                let y = {
                    let optional_y = graph.node(&id).and_then(|n| n.y);

                    match optional_y {
                        Some(matched_y) => matched_y,
                        ::core::option::Option::None => continue,
                    }
                };

                {
                    match graph.node_mut(&id) {
                        Some(n) => {
                            n.y = Some(y + delta_y);
                        },
                        ::core::option::Option::None => {},
                    };
                }
            }
        }


        for ek in graph.edge_keys() {
            let e = {
                let optional_e = graph.edge_mut_by_key(&ek);

                match optional_e {
                    Some(matched_e) => matched_e,
                    ::core::option::Option::None => continue,
                }
            };

            match e.y {
                Some(y) => {
                    e.y = Some(y + y_shift);
                },
                ::core::option::Option::None => {},
            }

            for p in &mut e.points {
                p.y += y_shift;
            }
        }
    }

    struct RecursiveLayoutContext<'a> {
        extracted:
            &'a mut std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
        subgraph_id_set: &'a std::collections::HashSet<&'a str>,
        y_shift: f64,
        cluster_node_labels: &'a std::collections::HashMap<String, NodeLabel>,
        title_total_margin: f64,
        title_metrics_ctx: &'a ClusterTitleMetricsContext<'a>,
        cluster_padding: f64,
        work_control: &'a mut DagreOperationWorkControl,
    }

    fn layout_graph_with_recursive_clusters(
        graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        graph_cluster_id: Option<&str>,
        _depth: usize,
        ctx: &mut RecursiveLayoutContext<'_>,
    ) -> Result<()> {
        struct LayoutFrame {
            cluster_id: Option<String>,
            child_ids: Option<Vec<String>>,
        }

        #[must_use]
        fn recursive_child_ids(
            graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
            extracted: &std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
        ) -> Vec<String> {
            graph
                .node_ids()
                .into_iter()



                .filter(|id| graph.child_count(id) == 0 && extracted.contains_key(id))
                .collect()
        }

        fn inject_parent_cluster(
            graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
            cluster_id: &str,
            ctx: &mut RecursiveLayoutContext<'_>,
        ) -> Result<()> {
            let inserts_cluster = usize::from(!graph.has_node(cluster_id));
            let final_node_order_slots = ctx
                .work_control
                .checked_add(graph.node_order_slot_count(), inserts_cluster)?;
            let final_node_count = ctx
                .work_control
                .checked_add(graph.node_count(), inserts_cluster)?;
            let snapshot_work = ctx
                .work_control
                .checked_add(final_node_order_slots, final_node_count)?;
            ctx.work_control.charge_adapter(snapshot_work)?;

            match inserts_cluster {
                0 => {},
                _ => {
                    let lbl = ctx
                        .cluster_node_labels
                        .get(cluster_id)
                        .cloned()
                        .unwrap_or_default();
                    charge_node_insertion_ordering(graph, cluster_id, ctx.work_control)?;
                    graph.set_node(cluster_id.to_string(), lbl);
                },
            }

            let node_ids = graph.node_ids();
            let cluster_ix = graph
                .node_ix(cluster_id)
                .expect("the injected cluster node is present");
            let mut parent_assignments = Vec::new();
            let mut existing_parent_count = usize::from(graph.parent(cluster_id).is_some());
            let mut numeric_parent_assignments = 0usize;

            for node_id in node_ids {
                if node_id == cluster_id {
                    continue;
                }

                if graph.parent(&node_id).is_some() {
                    existing_parent_count += 1;
                } else {
                    let child_ix = graph
                        .node_ix(&node_id)
                        .expect("a node snapshot contains only live nodes");
                    parent_assignments.push((child_ix, cluster_ix));
                    numeric_parent_assignments += usize::from(is_javascript_array_index(&node_id));
                }
            }

            apply_unparented_parent_assignments(
                graph,
                &parent_assignments,
                UnparentedParentBatchShape {
                    existing_parent_count,
                    numeric_assignment_count: numeric_parent_assignments,
                },
                ctx.work_control,
            )?;
            Ok(())
        }

        fn update_child_cluster_bounds(
            graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
            ids: &[String],
            ctx: &mut RecursiveLayoutContext<'_>,
        ) -> Result<()> {
            for id in ids {
                let child = {
                    let optional_child = ctx.extracted.get(id);

                    match optional_child {
                        Some(matched_child) => matched_child,
                        ::core::option::Option::None => continue,
                    }
                };





                if let Some(r) = { let checked_value = extracted_graph_bbox_rect(
                    child,
                    ctx.title_total_margin,
                    ctx.extracted,
                    ctx.subgraph_id_set,
                    ctx.title_metrics_ctx,
                    ctx.cluster_padding,
                    ctx.work_control,
                )?; checked_value } {
                    {
                        match graph.node_mut(id) {
                            Some(n) => {
                                n.width = r.width().max(1.0);
                                n.height = r.height().max(1.0);
                            },
                            ::core::option::Option::None => {},
                        };
                    }
                } else if let Some(n_child) = child.node(id)
                    && let Some(n) = graph.node_mut(id)
                {
                    n.width = n_child.width.max(1.0);
                    n.height = n_child.height.max(1.0);
                }
            }

            Ok(())
        }

        fn layout_one_graph(
            graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
            ctx: &mut RecursiveLayoutContext<'_>,
        ) -> Result<()> {
            match dugong::layout_controlled(graph, &mut *ctx.work_control) {
                Err(error) => {
                    return Err(ctx.work_control.map_dugong_error(error));
                },
                ::core::result::Result::Ok(_) => {},
            }




            if ctx.y_shift.abs() < 1e-9 {
                return Ok(());
            }

            ctx.work_control.charge_adapter(graph.edge_slot_count())?;
            let point_work = graph.edges().try_fold(0usize, |total, edge_key| {
                let points = graph
                    .edge_by_key(edge_key)
                    .map_or(0, |edge| edge.points.len());
                ctx.work_control.checked_add(total, points)
            })?;
            let node_snapshot_work = node_id_snapshot_work_upper_bound(graph, ctx.work_control)?;
            let edge_snapshot_work = ctx
                .work_control
                .checked_add(graph.edge_slot_count(), graph.edge_count())?;
            let graph_work = ctx
                .work_control
                .checked_add(node_snapshot_work, edge_snapshot_work)?;
            let shift_work = ctx.work_control.checked_add(graph_work, point_work)?;
            ctx.work_control.charge_adapter(shift_work)?;
            apply_mermaid_subgraph_title_shifts(
                graph,
                ctx.extracted,
                ctx.subgraph_id_set,
                ctx.y_shift,
            );
            Ok(())
        }

        if ctx.extracted.is_empty() {
            return layout_one_graph(graph, ctx);
        }

        let mut stack = vec![LayoutFrame {
            cluster_id: graph_cluster_id.map(str::to_string),
            child_ids: None,
        }];

        while let Some(frame) = stack.pop() {
            let Some(child_ids) = frame.child_ids else {
                let cluster_id = frame.cluster_id;
                let graph_snapshot_work = match &cluster_id {
                    Some(id) => ctx.extracted.get(id).map_or(Ok(0), |graph| {
                        node_id_snapshot_work_upper_bound(graph, ctx.work_control)
                    })?,
                    None => node_id_snapshot_work_upper_bound(graph, ctx.work_control)?,
                };
                ctx.work_control.charge_adapter(graph_snapshot_work)?;

                let Some((child_ids, parent_nodesep, parent_ranksep)) = (match &cluster_id {
                    Some(id) => ctx.extracted.get(id).map(|g| {
                        (
                            recursive_child_ids(g, ctx.extracted),
                            g.graph().nodesep,
                            g.graph().ranksep,
                        )
                    }),
                    None => Some((
                        recursive_child_ids(graph, ctx.extracted),
                        graph.graph().nodesep,
                        graph.graph().ranksep,
                    )),
                }) else {
                    continue;
                };

                let mut child_frames = Vec::with_capacity(child_ids.len());

                for child_id in child_ids.iter().rev() {




                    match ctx.extracted.get_mut(child_id) {
                        Some(child) => {
                            child.graph_mut().nodesep = parent_nodesep;
                            child.graph_mut().ranksep = parent_ranksep + 25.0;
                        },
                        ::core::option::Option::None => {},
                    }

                    child_frames.push(LayoutFrame {
                        cluster_id: Some(child_id.clone()),
                        child_ids: None,
                    });
                }

                stack.push(LayoutFrame {
                    cluster_id,
                    child_ids: Some(child_ids),
                });
                stack.extend(child_frames);
                continue;
            };

            match frame.cluster_id {
                Some(cluster_id) => {
                    let mut current = {
                        let optional_current = ctx.extracted.remove(&cluster_id);

                        match optional_current {
                            Some(matched_current) => matched_current,
                            ::core::option::Option::None => continue,
                        }
                    };

                    update_child_cluster_bounds(&mut current, &child_ids, ctx)?;
                    inject_parent_cluster(&mut current, &cluster_id, ctx)?;
                    layout_one_graph(&mut current, ctx)?;
                    ctx.extracted.insert(cluster_id, current);
                },
                ::core::option::Option::None => {
                    update_child_cluster_bounds(graph, &child_ids, ctx)?;
                    layout_one_graph(graph, ctx)?;
                },
            }
        }

        Ok(())
    }

    {
        let title_metrics_ctx = ClusterTitleMetricsContext {
            model: model_2,
            subgraphs_by_id: &subgraphs_by_id,
            subgraph_index_by_id: &subgraph_index_by_id,
            class_defs: &model_2.class_defs,
            measurer,
            text_style: &text_style,
            html_label_text_style: &html_label_text_style,
            title_wrapping_width: cluster_title_wrapping_width,
            wrap_mode: cluster_wrap_mode,
            config: effective_config,
            math_renderer,
            svg_label_sidecar,
        };
        let mut recursive_layout_ctx = RecursiveLayoutContext {
            extracted: &mut extracted_graphs,
            subgraph_id_set: &cluster_subgraph_ids,
            y_shift,
            cluster_node_labels: &cluster_node_labels,
            title_total_margin,
            title_metrics_ctx: &title_metrics_ctx,
            cluster_padding,
            work_control,
        };
        layout_graph_with_recursive_clusters(&mut g, None, 0, &mut recursive_layout_ctx)?;
    }

    let mut leaf_rects: std::collections::HashMap<String, Rect> = std::collections::HashMap::new();
    let mut base_pos: std::collections::HashMap<String, (f64, f64)> =
        std::collections::HashMap::new();
    let mut edge_override_points: std::collections::HashMap<
        FlowchartLayoutSegmentKey,
        Vec<LayoutPoint>,
    > = std::collections::HashMap::new();
    let mut edge_override_label: std::collections::HashMap<
        FlowchartLayoutSegmentKey,
        Option<LayoutLabel>,
    > = std::collections::HashMap::new();
    let mut edge_override_from_cluster: std::collections::HashMap<
        FlowchartLayoutSegmentKey,
        Option<String>,
    > = std::collections::HashMap::new();
    let mut edge_override_to_cluster: std::collections::HashMap<
        FlowchartLayoutSegmentKey,
        Option<String>,
    > = std::collections::HashMap::new();
    let mut edge_override_endpoints: std::collections::HashMap<
        FlowchartLayoutSegmentKey,
        (String, String),
    > = std::collections::HashMap::new();
    let mut edge_override_self_loop_meta: std::collections::HashMap<
        FlowchartLayoutSegmentKey,
        FlowchartSelfLoopSegmentMeta,
    > = std::collections::HashMap::new();
    let leaf_node_id_work = work_control.checked_add(
        model_2.nodes.len(),
        work_control.checked_add(self_loop_label_node_ids.len(), empty_subgraph_ids.len())?,
    )?;
    work_control.charge_adapter(leaf_node_id_work)?;
    let mut leaf_node_ids: std::collections::HashSet<String> = model_2
        .nodes
        .iter()
        .filter(|n| !subgraph_ids.contains(n.id.as_str()))
        .map(|n| n.id.clone())
        .collect();

    for id in &self_loop_label_node_ids {
        leaf_node_ids.insert(id.clone());
    }

    for id in &empty_subgraph_ids {
        leaf_node_ids.insert(id.clone());
    }

    for id in &collapsed_subgraph_ids {
        leaf_node_ids.insert((*id).to_string());
    }

    struct PlaceGraphInputs<'a> {
        extracted_graphs:
            &'a std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
        subgraph_ids: &'a std::collections::HashSet<&'a str>,
        leaf_node_ids: &'a std::collections::HashSet<String>,
    }

    struct PlaceGraphOutputs<'a> {
        base_pos: &'a mut std::collections::HashMap<String, (f64, f64)>,
        leaf_rects: &'a mut std::collections::HashMap<String, Rect>,
        cluster_rects_from_graph: &'a mut std::collections::HashMap<String, Rect>,
        extracted_cluster_rects: &'a mut std::collections::HashMap<String, Rect>,
        extracted_cluster_base_widths: &'a mut std::collections::HashMap<String, f64>,
        edge_override_points:
            &'a mut std::collections::HashMap<FlowchartLayoutSegmentKey, Vec<LayoutPoint>>,
        edge_override_label:
            &'a mut std::collections::HashMap<FlowchartLayoutSegmentKey, Option<LayoutLabel>>,
        edge_override_from_cluster:
            &'a mut std::collections::HashMap<FlowchartLayoutSegmentKey, Option<String>>,
        edge_override_to_cluster:
            &'a mut std::collections::HashMap<FlowchartLayoutSegmentKey, Option<String>>,
        edge_override_endpoints:
            &'a mut std::collections::HashMap<FlowchartLayoutSegmentKey, (String, String)>,
        edge_override_self_loop_meta: &'a mut std::collections::HashMap<
            FlowchartLayoutSegmentKey,
            FlowchartSelfLoopSegmentMeta,
        >,
    }

    fn place_graph(
        graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        offset: (f64, f64),
        is_root: bool,
        inputs: &PlaceGraphInputs<'_>,
        out: &mut PlaceGraphOutputs<'_>,
        work_control: &mut DagreOperationWorkControl,
    ) -> Result<()> {
        struct PlaceFrame<'a> {
            graph: &'a Graph<NodeLabel, EdgeLabel, GraphLabel>,
            offset: (f64, f64),
            is_root: bool,
        }

        fn subtree_rect_iterative(
            graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
            id: &str,
            work_control: &mut DagreOperationWorkControl,
        ) -> Result<Option<Rect>> {
            let mut visiting: std::collections::HashSet<String> = std::collections::HashSet::new();

            if !visiting.insert(id.to_string()) {
                return Ok(None);
            }

            let mut out: Option<Rect> = None;
            let child_count = child_id_snapshot_work_upper_bound(graph, id, work_control)?;
            work_control.charge_adapter(child_count)?;
            let mut stack: Vec<String> =
                graph.children(id).into_iter().map(str::to_string).collect();

            while let Some(node) = stack.pop() {
                work_control.charge_adapter(1)?;

                if let Some(n) = graph.node(&node)
                    && let (Some(x), Some(y)) = (n.x, n.y)
                {
                    let r = Rect::from_center(x, y, n.width, n.height);

                    match out {
                        Some(ref mut cur) => {
                            cur.union(r);
                        },
                        ::core::option::Option::None => {
                            out = Some(r);
                        },
                    }
                }

                let child_count = child_id_snapshot_work_upper_bound(graph, &node, work_control)?;
                work_control.charge_adapter(
                    work_control.checked_add(child_count, graph.child_count(&node))?,
                )?;

                for child in graph.children(&node) {
                    if visiting.insert(child.to_string()) {
                        stack.push(child.to_string());
                    }
                }
            }

            Ok(out)
        }

        let mut stack = vec![PlaceFrame {
            graph,
            offset,
            is_root,
        }];
        while let Some(frame) = stack.pop() {
            let node_snapshot_work = node_id_snapshot_work_upper_bound(frame.graph, work_control)?;
            work_control.charge_adapter(node_snapshot_work)?;

            'over_id_2_: for id in frame.graph.node_ids() {
                let n = {
                    let optional_n = frame.graph.node(&id);

                    match optional_n {
                        Some(matched_n) => matched_n,
                        ::core::option::Option::None => continue 'over_id_2_,
                    }
                };

                let x = (match n.x {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => 0.0,
                }) + frame.offset.0;
                let y = (match n.y {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => 0.0,
                }) + frame.offset.1;

                if inputs.leaf_node_ids.contains(&id) {
                    out.base_pos.insert(id.clone(), (x, y));
                    out.leaf_rects
                        .insert(id.clone(), Rect::from_center(x, y, n.width, n.height));
                    continue 'over_id_2_;
                }
            }






            work_control.charge_adapter(node_snapshot_work)?;

            'over_id_2: for id in frame.graph.node_ids() {
                if !inputs.subgraph_ids.contains(id.as_str()) {
                    continue 'over_id_2;
                }

                if inputs.extracted_graphs.contains_key(&id) {
                    continue 'over_id_2;
                }

                if out.cluster_rects_from_graph.contains_key(&id) {
                    continue 'over_id_2;
                }

                if let Some(n) = frame.graph.node(&id)
                    && let (Some(x), Some(y)) = (n.x, n.y)
                    && n.width > 0.0
                    && n.height > 0.0
                {
                    let mut r = Rect::from_center(x, y, n.width, n.height);
                    r.translate(frame.offset.0, frame.offset.1);
                    out.cluster_rects_from_graph.insert(id, r);
                    continue 'over_id_2;
                }

                let mut r = {
                    let optional_r = subtree_rect_iterative(frame.graph, &id, work_control)?;

                    match optional_r {
                        Some(matched_r) => matched_r,
                        ::core::option::Option::None => continue 'over_id_2,
                    }
                };

                r.translate(frame.offset.0, frame.offset.1);
                out.cluster_rects_from_graph.insert(id, r);
            }

            work_control.charge_adapter(frame.graph.edge_slot_count())?;
            let edge_point_work = frame.graph.edges().try_fold(0usize, |total, edge_key| {
                let points = frame
                    .graph
                    .edge_by_key(edge_key)
                    .map_or(0, |edge| edge.points.len());
                work_control.checked_add(total, points)
            })?;
            let edge_snapshot_work = work_control
                .checked_add(frame.graph.edge_slot_count(), frame.graph.edge_count())?;
            let edge_work = work_control.checked_add(edge_snapshot_work, edge_point_work)?;
            work_control.charge_adapter(edge_work)?;

            'over_ek_2: for ek in frame.graph.edge_keys() {
                let lbl = {
                    let optional_lbl = frame.graph.edge_by_key(&ek);

                    match optional_lbl {
                        Some(matched_lbl) => matched_lbl,
                        ::core::option::Option::None => continue 'over_ek_2,
                    }
                };

                let segment_key = {
                    let optional_segment_key = flowchart_layout_segment_key(lbl);

                    match optional_segment_key {
                        Some(matched_segment_key) => matched_segment_key,
                        ::core::option::Option::None => continue 'over_ek_2,
                    }
                };

                let owner_index = segment_key.owner_index;

                if let (Some(x), Some(y)) = (lbl.x, lbl.y)
                    && (lbl.width > 0.0 || lbl.height > 0.0)
                {
                    let lx = x + frame.offset.0;
                    let ly = y + frame.offset.1;
                    let leaf_id = format!("edge-label::{owner_index}");
                    out.base_pos.insert(leaf_id.clone(), (lx, ly));
                    out.leaf_rects
                        .insert(leaf_id, Rect::from_center(lx, ly, lbl.width, lbl.height));
                }

                if !frame.is_root {
                    let points = lbl
                        .points
                        .iter()
                        .map(|p| LayoutPoint {
                            x: p.x + frame.offset.0,
                            y: p.y + frame.offset.1,
                        })
                        .collect::<Vec<_>>();
                    let label_pos = match (lbl.x, lbl.y) {
                        (Some(x), Some(y)) if lbl.width > 0.0 || lbl.height > 0.0 => {
                            Some(LayoutLabel {
                                x: x + frame.offset.0,
                                y: y + frame.offset.1,
                                width: lbl.width,
                                height: lbl.height,
                            })
                        }
                        _ => None,
                    };
                    out.edge_override_points.insert(segment_key, points);
                    out.edge_override_label.insert(segment_key, label_pos);
                    let from_cluster = lbl
                        .extras
                        .get("fromCluster")
                        .and_then(|v| v.as_str().map(|s| s.to_string()));
                    let to_cluster = lbl
                        .extras
                        .get("toCluster")
                        .and_then(|v| v.as_str().map(|s| s.to_string()));
                    out.edge_override_from_cluster
                        .insert(segment_key, from_cluster);
                    out.edge_override_to_cluster.insert(segment_key, to_cluster);
                    out.edge_override_endpoints
                        .insert(segment_key, (ek.v.clone(), ek.w.clone()));

                    {
                        match flowchart_self_loop_segment_meta(lbl) {
                            Some(meta) => {
                                out.edge_override_self_loop_meta.insert(segment_key, meta);
                            },
                            ::core::option::Option::None => {},
                        };
                    }
                }
            }

            let mut child_frames = Vec::new();
            work_control.charge_adapter(node_snapshot_work)?;

            'over_id_2: for id in frame.graph.node_ids() {



                if frame.graph.child_count(&id) != 0 {
                    continue 'over_id_2;
                }

                let child = {
                    let optional_child_2 = inputs.extracted_graphs.get(&id);

                    match optional_child_2 {
                        Some(matched_child_2) => matched_child_2,
                        ::core::option::Option::None => continue 'over_id_2,
                    }
                };

                let n = {
                    let optional_n_2 = frame.graph.node(&id);

                    match optional_n_2 {
                        Some(matched_n_2) => matched_n_2,
                        ::core::option::Option::None => continue 'over_id_2,
                    }
                };

                let (px, py) = {
                    let outcome_px = (n.x, n.y);

                    match outcome_px {
                        (Some(matched_px), Some(matched_py)) => (matched_px, matched_py),
                        (::core::option::Option::None, _)
                        | (_, ::core::option::Option::None) => continue 'over_id_2,
                    }
                };

                let parent_x = px + frame.offset.0;
                let parent_y = py + frame.offset.1;

                let cnode = {
                    let optional_cnode = child.node(&id);

                    match optional_cnode {
                        Some(matched_cnode) => matched_cnode,
                        ::core::option::Option::None => continue 'over_id_2,
                    }
                };

                let (cx, cy) = {
                    let outcome_cx = (cnode.x, cnode.y);

                    match outcome_cx {
                        (Some(matched_cx), Some(matched_cy)) => (matched_cx, matched_cy),
                        (::core::option::Option::None, _)
                        | (_, ::core::option::Option::None) => continue 'over_id_2,
                    }
                };

                let child_offset = (parent_x - cx, parent_y - cy);




                let r = Rect::from_center(parent_x, parent_y, n.width, n.height);
                out.extracted_cluster_rects.insert(id.clone(), r);
                out.extracted_cluster_base_widths
                    .insert(id.clone(), cnode.width.max(1.0));
                child_frames.push(PlaceFrame {
                    graph: child,
                    offset: child_offset,
                    is_root: false,
                });
            }

            for child in child_frames.into_iter().rev() {
                stack.push(child);
            }
        }

        Ok(())
    }

    let mut cluster_rects_from_graph: std::collections::HashMap<String, Rect> =
        std::collections::HashMap::new();
    let mut extracted_cluster_rects: std::collections::HashMap<String, Rect> =
        std::collections::HashMap::new();
    let mut extracted_cluster_base_widths: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();

    {
        let place_graph_inputs = PlaceGraphInputs {
            extracted_graphs: &extracted_graphs,
            subgraph_ids: &cluster_subgraph_ids,
            leaf_node_ids: &leaf_node_ids,
        };
        let mut place_graph_outputs = PlaceGraphOutputs {
            base_pos: &mut base_pos,
            leaf_rects: &mut leaf_rects,
            cluster_rects_from_graph: &mut cluster_rects_from_graph,
            extracted_cluster_rects: &mut extracted_cluster_rects,
            extracted_cluster_base_widths: &mut extracted_cluster_base_widths,
            edge_override_points: &mut edge_override_points,
            edge_override_label: &mut edge_override_label,
            edge_override_from_cluster: &mut edge_override_from_cluster,
            edge_override_to_cluster: &mut edge_override_to_cluster,
            edge_override_endpoints: &mut edge_override_endpoints,
            edge_override_self_loop_meta: &mut edge_override_self_loop_meta,
        };
        place_graph(
            &g,
            (0.0, 0.0),
            true,
            &place_graph_inputs,
            &mut place_graph_outputs,
            work_control,
        )?;
    }

    let mut extra_children: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    work_control.charge_adapter(render_edges.len())?;
    let labeled_edges: std::collections::HashSet<usize> = render_edges
        .iter()
        .zip(&render_edge_self_loop_meta)
        .zip(&render_edge_owner_indices)
        .filter(|((edge, self_loop), owner_index)| {
            edge_label_is_non_empty(model_2, **owner_index, edge.as_ref(), self_loop.as_ref())
        })
        .map(|(_, owner_index)| *owner_index)
        .collect();

    fn collect_extra_children(
        graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        labeled_edges: &std::collections::HashSet<usize>,
        implicit_root: Option<&str>,
        out: &mut std::collections::HashMap<String, Vec<String>>,
        work_control: &mut DagreOperationWorkControl,
    ) -> Result<()> {
        let edge_snapshot_work =
            work_control.checked_add(graph.edge_slot_count(), graph.edge_count())?;
        work_control.charge_adapter(edge_snapshot_work)?;

        for ek in graph.edge_keys() {
            let label = {
                let optional_label = graph.edge_by_key(&ek);

                match optional_label {
                    Some(matched_label) => matched_label,
                    ::core::option::Option::None => continue,
                }
            };

            let owner_index = {
                let optional_owner_index = flowchart_edge_owner_index(label);

                match optional_owner_index {
                    Some(matched_owner_index) => matched_owner_index,
                    ::core::option::Option::None => continue,
                }
            };

            if !labeled_edges.contains(&owner_index) {
                continue;
            }






            let parent = lowest_common_parent(graph, &ek.v, &ek.w, work_control)?
                .or_else(|| implicit_root.map(|s| s.to_string()));

            let parent = match parent {
                Some(matched_parent) => matched_parent,
                ::core::option::Option::None => continue,
            };

            out.entry(parent)
                .or_default()
                .push(format!("edge-label::{owner_index}"));
        }

        Ok(())
    }

    collect_extra_children(&g, &labeled_edges, None, &mut extra_children, work_control)?;

    for cluster_id in &extracted_order {
        let cg = extracted_graphs
            .get(cluster_id)
            .expect("the extraction order references an extracted graph");
        collect_extra_children(
            cg,
            &labeled_edges,
            Some(cluster_id.as_str()),
            &mut extra_children,
            work_control,
        )?;
    }




    work_control.charge_adapter(self_loop_label_node_ids.len())?;

    for id in &self_loop_label_node_ids {
        {
            match g.parent(id) {
                Some(p) => {
                    extra_children
                        .entry(p.to_string())
                        .or_default()
                        .push(id.clone());
                },
                ::core::option::Option::None => {},
            };
        }
    }

    let mut out_nodes: Vec<LayoutNode> = Vec::new();
    work_control.charge_adapter(model_2.nodes.len())?;

    for n in &model_2.nodes {
        if subgraph_ids.contains(n.id.as_str())
            || model_2.collapsed_replacement(n.id.as_str()).is_some()
        {
            continue;
        }

        let (x, y) = base_pos
            .get(&n.id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing positioned node {}", n.id),
            })?;
        let (width, height) = leaf_rects
            .get(&n.id)
            .map(|r| (r.width(), r.height()))
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing sized node {}", n.id),
            })?;
        out_nodes.push(LayoutNode {
            id: n.id.clone(),
            x,
            y,
            width,
            height,
            is_cluster: false,
            label_width: leaf_label_metrics_by_id.get(&n.id).map(|v| v.0),
            label_height: leaf_label_metrics_by_id.get(&n.id).map(|v| v.1),
        });
    }

    work_control.charge_adapter(empty_subgraph_ids.len())?;

    for id in &empty_subgraph_ids {
        let (x, y) = base_pos
            .get(id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing positioned node {id}"),
            })?;
        let (width, height) = leaf_rects
            .get(id)
            .map(|r| (r.width(), r.height()))
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing sized node {id}"),
            })?;
        out_nodes.push(LayoutNode {
            id: id.clone(),
            x,
            y,
            width,
            height,
            is_cluster: false,
            label_width: leaf_label_metrics_by_id.get(id).map(|v| v.0),
            label_height: leaf_label_metrics_by_id.get(id).map(|v| v.1),
        });
    }

    work_control.charge_adapter(collapsed_subgraph_ids.len())?;

    for id in &collapsed_subgraph_ids {
        let (x, y) = base_pos
            .get(*id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing positioned collapsed subgraph {id}"),
            })?;
        let (width, height) = leaf_rects
            .get(*id)
            .map(|r| (r.width(), r.height()))
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing sized collapsed subgraph {id}"),
            })?;
        out_nodes.push(LayoutNode {
            id: (*id).to_string(),
            x,
            y,
            width,
            height,
            is_cluster: false,
            label_width: leaf_label_metrics_by_id.get(*id).map(|v| v.0),
            label_height: leaf_label_metrics_by_id.get(*id).map(|v| v.1),
        });
    }

    work_control.charge_adapter(self_loop_label_node_ids.len())?;

    for id in &self_loop_label_node_ids {
        let (x, y) = base_pos
            .get(id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing positioned node {id}"),
            })?;
        let (width, height) = leaf_rects
            .get(id)
            .map(|r| (r.width(), r.height()))
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing sized node {id}"),
            })?;
        out_nodes.push(LayoutNode {
            id: id.clone(),
            x,
            y,
            width,
            height,
            is_cluster: false,
            label_width: None,
            label_height: None,
        });
    }

    let mut clusters: Vec<LayoutCluster> = Vec::new();

    let mut cluster_rects: std::collections::HashMap<String, Rect> =
        std::collections::HashMap::new();
    let mut cluster_base_widths: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
    let mut visiting: std::collections::HashSet<String> = std::collections::HashSet::new();
    let cluster_membership_work = model_2.subgraphs.iter().try_fold(0usize, |total, subgraph| {
        work_control.checked_add(total, subgraph.nodes.len())
    })?;
    let extra_cluster_work = extra_children
        .values()
        .try_fold(0usize, |total, children| {
            work_control.checked_add(total, children.len())
        })?;





    let cluster_work = work_control.checked_add(
        work_control.checked_add(
            work_control.checked_mul(model_2.subgraphs.len(), 3)?,
            work_control.checked_mul(cluster_membership_work, 2)?,
        )?,
        extra_cluster_work,
    )?;
    work_control.charge_adapter(cluster_work)?;

    struct ClusterRectContext<'a> {
        model: &'a FlowchartRenderModelRef<'a>,
        subgraphs_by_id: &'a FlowSubgraphIndex<'a>,
        subgraph_index_by_id: &'a HashMap<&'a str, usize>,
        subgraph_members_by_id: &'a HashMap<&'a str, Vec<&'a str>>,
        class_defs: &'a indexmap::IndexMap<String, Vec<String>>,
        leaf_rects: &'a std::collections::HashMap<String, Rect>,
        extra_children: &'a std::collections::HashMap<String, Vec<String>>,
        measurer: &'a dyn TextMeasurer,
        text_style: &'a TextStyle,
        html_label_text_style: &'a TextStyle,
        title_wrapping_width: f64,
        wrap_mode: WrapMode,
        config: &'a PresentationConfig,
        math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
        svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
        cluster_padding: f64,
        title_total_margin: f64,
    }

    struct ClusterRectState<'a> {
        cluster_rects: &'a mut std::collections::HashMap<String, Rect>,
        cluster_base_widths: &'a mut std::collections::HashMap<String, f64>,
        visiting: &'a mut std::collections::HashSet<String>,
    }

    fn compute_cluster_rect(
        id: &str,
        ctx: &ClusterRectContext<'_>,
        state: &mut ClusterRectState<'_>,
    ) -> Result<(Rect, f64)> {
        match state.cluster_rects.get(id).copied() {
            Some(r) => {
                let base_width = state
                    .cluster_base_widths
                    .get(id)
                    .copied()
                    .unwrap_or_else(|| r.width());
                return Ok((r, base_width));
            },
            ::core::option::Option::None => {},
        }

        struct ClusterRectFrame {
            id: String,
            expanded: bool,
        }

        let mut stack = vec![ClusterRectFrame {
            id: id.to_string(),
            expanded: false,
        }];
        while let Some(frame) = stack.pop() {
            if state.cluster_rects.contains_key(&frame.id) {
                continue;
            }

            if !frame.expanded {
                if !state.visiting.insert(frame.id.clone()) {
                    return Err(Error::InvalidModel {
                        message: format!("cycle in subgraph membership involving {}", frame.id),
                    });
                }

                if !ctx.subgraphs_by_id.contains_key(frame.id.as_str()) {
                    return Err(Error::InvalidModel {
                        message: format!("missing subgraph definition for {}", frame.id),
                    });
                }

                stack.push(ClusterRectFrame {
                    id: frame.id.clone(),
                    expanded: true,
                });
                let members = match ctx
                    .subgraph_members_by_id
                    .get(frame.id.as_str())
                    .map(Vec::as_slice) {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => &[],
                };

                for member in members.iter().rev() {
                    if ctx.subgraphs_by_id.contains_key(*member)
                        && !state.cluster_rects.contains_key(*member)
                    {
                        stack.push(ClusterRectFrame {
                            id: (*member).to_string(),
                            expanded: false,
                        });
                    }
                }

                continue;
            }

            let Some(sg) = ctx.subgraphs_by_id.get(frame.id.as_str()) else {
                return Err(Error::InvalidModel {
                    message: format!("missing subgraph definition for {}", frame.id),
                });
            };

            let mut content: Option<Rect> = None;
            let members = match ctx
                .subgraph_members_by_id
                .get(frame.id.as_str())
                .map(Vec::as_slice) {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => &[],
            };

            for member in members {
                let member_rect = if let Some(r) = ctx.leaf_rects.get(*member).copied() {
                    Some(r)
                } else if ctx.subgraphs_by_id.contains_key(*member) {
                    let checked_value = state.cluster_rects.get(*member).copied().ok_or_else(|| {
                        Error::InvalidModel {
                            message: format!("missing computed subgraph rect for {member}"),
                        }
                    })?;
                    Some(checked_value)
                } else {
                    None
                };

                match member_rect {
                    Some(r) => {
                        match content {
                            Some(ref mut cur) => {
                                cur.union(r);
                            },
                            ::core::option::Option::None => {
                                content = Some(r);
                            },
                        }
                    },
                    ::core::option::Option::None => {},
                }
            }

            match ctx.extra_children.get(&frame.id) {
                Some(extra) => {
                    for child in extra {
                        {
                            match ctx.leaf_rects.get(child).copied() {
                                Some(r) => {
                                    match content {
                                        Some(ref mut cur) => {
                                            cur.union(r);
                                        },
                                        ::core::option::Option::None => {
                                            content = Some(r);
                                        },
                                    }
                                },
                                ::core::option::Option::None => {},
                            };
                        }
                    }
                },
                ::core::option::Option::None => {},
            }

            let label_type = sg.label_type.as_deref().unwrap_or("text");
            let subgraph_index = ctx
                .subgraph_index_by_id
                .get(frame.id.as_str())
                .copied()
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("missing canonical subgraph index for {}", frame.id),
                })?;
            let title = ctx.model.subgraph_title_for_render(subgraph_index, sg);
            let title_width_limit = (label_type == "markdown").then_some(ctx.title_wrapping_width);
            let base_style = if ctx.wrap_mode == WrapMode::HtmlLike {
                ctx.html_label_text_style
            } else {
                ctx.text_style
            };
            let (classes, styles) = ctx.model.effective_subgraph_css(subgraph_index, sg);
            let text_style = flowchart_effective_text_style_for_classes_with_provenance(
                base_style,
                ctx.class_defs,
                classes,
                styles,
            );
            let owner = Some(FlowchartSvgLabelOwner::SubgraphTitle(subgraph_index));
            let title_metrics = measure_flowchart_svg_label_for_layout_with_terminal_foreground(
                ctx.svg_label_sidecar,
                owner,
                owner.map(|_| sg.id.as_str()),
                FlowchartLabelMetricsRequest {
                    measurer: ctx.measurer,
                    raw_label: title,
                    label_type,
                    style: text_style.as_ref(),
                    max_width_px: title_width_limit,
                    wrap_mode: ctx.wrap_mode,
                    config: ctx.config,
                    math_renderer: ctx.math_renderer,
                },
                text_style.terminal_foreground(),
                FlowchartSvgWidthMode::Bbox,
            );
            let mut rect = match content {
                Some(r) => {
                    r
                },
                ::core::option::Option::None => {
                    Rect::from_center(
                        0.0,
                        0.0,
                        title_metrics.width.max(1.0),
                        title_metrics.height.max(1.0),
                    )
                },
            };


            rect.pad(ctx.cluster_padding);



            let base_width = rect.width();



            let min_width = title_metrics.width.max(1.0) + ctx.cluster_padding;

            if rect.width() < min_width {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, min_width, rect.height());
            }


            if ctx.title_total_margin > 0.0 {
                let (cx, cy) = rect.center();
                rect =
                    Rect::from_center(cx, cy, rect.width(), rect.height() + ctx.title_total_margin);
            }


            let min_height = title_metrics.height.max(1.0) + ctx.title_total_margin;

            if rect.height() < min_height {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, rect.width(), min_height);
            }

            state.visiting.remove(&frame.id);
            state.cluster_rects.insert(frame.id.clone(), rect);
            state.cluster_base_widths.insert(frame.id, base_width);
        }

        let rect = state
            .cluster_rects
            .get(id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing computed subgraph rect for {id}"),
            })?;
        let base_width = state
            .cluster_base_widths
            .get(id)
            .copied()
            .unwrap_or_else(|| rect.width());
        Ok((rect, base_width))
    }

    struct ClusterTitleAdjustContext<'a> {
        model: &'a FlowchartRenderModelRef<'a>,
        class_defs: &'a indexmap::IndexMap<String, Vec<String>>,
        subgraph_index_by_id: &'a HashMap<&'a str, usize>,
        measurer: &'a dyn TextMeasurer,
        text_style: &'a TextStyle,
        html_label_text_style: &'a TextStyle,
        title_wrapping_width: f64,
        wrap_mode: WrapMode,
        config: &'a PresentationConfig,
        math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
        svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
        title_total_margin: f64,
        cluster_padding: f64,
    }

    #[must_use]
    fn adjust_cluster_rect_for_title(
        mut rect: Rect,
        declaration_ordinal: usize,
        sg: &FlowSubgraph,
        title: &str,
        label_type: &str,
        add_title_total_margin: bool,
        ctx: &ClusterTitleAdjustContext<'_>,
    ) -> Rect {
        let title_width_limit = (label_type == "markdown").then_some(ctx.title_wrapping_width);
        let base_style = if ctx.wrap_mode == WrapMode::HtmlLike {
            ctx.html_label_text_style
        } else {
            ctx.text_style
        };
        let (classes, styles) = ctx.model.effective_subgraph_css(declaration_ordinal, sg);
        let text_style = flowchart_effective_text_style_for_classes_with_provenance(
            base_style,
            ctx.class_defs,
            classes,
            styles,
        );
        let owner = ctx
            .subgraph_index_by_id
            .get(sg.id.as_str())
            .copied()
            .map(FlowchartSvgLabelOwner::SubgraphTitle);
        let title_metrics = measure_flowchart_svg_label_for_layout_with_terminal_foreground(
            ctx.svg_label_sidecar,
            owner,
            Some(sg.id.as_str()),
            FlowchartLabelMetricsRequest {
                measurer: ctx.measurer,
                raw_label: title,
                label_type,
                style: text_style.as_ref(),
                max_width_px: title_width_limit,
                wrap_mode: ctx.wrap_mode,
                config: ctx.config,
                math_renderer: ctx.math_renderer,
            },
            text_style.terminal_foreground(),
            FlowchartSvgWidthMode::Bbox,
        );
        let title_w = title_metrics.width.max(1.0);
        let title_h = title_metrics.height.max(1.0);



        let min_w = title_w + ctx.cluster_padding;

        if rect.width() < min_w {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, min_w, rect.height());
        }


        if add_title_total_margin && ctx.title_total_margin > 0.0 {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, rect.width(), rect.height() + ctx.title_total_margin);
        }


        let min_h = title_h + ctx.title_total_margin;

        if rect.height() < min_h {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, rect.width(), min_h);
        }

        rect
    }

    let cluster_rect_ctx = ClusterRectContext {
        model: model_2,
        subgraphs_by_id: &subgraphs_by_id,
        subgraph_index_by_id: &subgraph_index_by_id,
        subgraph_members_by_id: &subgraph_members_by_id,
        class_defs: &model_2.class_defs,
        leaf_rects: &leaf_rects,
        extra_children: &extra_children,
        measurer,
        text_style: &text_style,
        html_label_text_style: &html_label_text_style,
        title_wrapping_width: cluster_title_wrapping_width,
        wrap_mode: cluster_wrap_mode,
        config: effective_config,
        math_renderer,
        svg_label_sidecar,
        cluster_padding,
        title_total_margin,
    };
    let mut cluster_rect_state = ClusterRectState {
        cluster_rects: &mut cluster_rects,
        cluster_base_widths: &mut cluster_base_widths,
        visiting: &mut visiting,
    };
    let title_adjust_ctx = ClusterTitleAdjustContext {
        model: model_2,
        class_defs: &model_2.class_defs,
        subgraph_index_by_id: &subgraph_index_by_id,
        measurer,
        text_style: &text_style,
        html_label_text_style: &html_label_text_style,
        title_wrapping_width: cluster_title_wrapping_width,
        wrap_mode: cluster_wrap_mode,
        config: effective_config,
        math_renderer,
        svg_label_sidecar,
        title_total_margin,
        cluster_padding,
    };

    for &(subgraph_index, sg) in &canonical_subgraphs_in_order {
        if !cluster_subgraph_ids.contains(sg.id.as_str()) {
            continue;
        }

        let title = model_2.subgraph_title_for_render(subgraph_index, sg);

        let (rect, base_width) = if extracted_graphs.contains_key(&sg.id) {



            let rect = extracted_cluster_rects
                .get(&sg.id)
                .copied()
                .unwrap_or_else(|| {
                    compute_cluster_rect(&sg.id, &cluster_rect_ctx, &mut cluster_rect_state)
                        .map(|v| v.0)
                        .unwrap_or_else(|_| Rect::from_center(0.0, 0.0, 1.0, 1.0))
                });
            let base_width = extracted_cluster_base_widths
                .get(&sg.id)
                .copied()
                .unwrap_or_else(|| rect.width());
            let rect_2 = adjust_cluster_rect_for_title(
                rect,
                subgraph_index,
                sg,
                title,
                sg.label_type.as_deref().unwrap_or("text"),
                false,
                &title_adjust_ctx,
            );
            (rect_2, base_width)
        } else if let Some(r) = cluster_rects_from_graph.get(&sg.id).copied() {
            let base_width = r.width();
            let rect = adjust_cluster_rect_for_title(
                r,
                subgraph_index,
                sg,
                title,
                sg.label_type.as_deref().unwrap_or("text"),
                true,
                &title_adjust_ctx,
            );
            (rect, base_width)
        } else {
            compute_cluster_rect(&sg.id, &cluster_rect_ctx, &mut cluster_rect_state)?
        };
        let (cx, cy) = rect.center();

        let label_type = sg.label_type.as_deref().unwrap_or("text");
        let title_width_limit = (label_type == "markdown").then_some(cluster_title_wrapping_width);
        let base_style = if cluster_wrap_mode == WrapMode::HtmlLike {
            &html_label_text_style
        } else {
            &text_style
        };
        let (classes, styles) = model_2.effective_subgraph_css(subgraph_index, sg);
        let title_text_style = flowchart_effective_text_style_for_classes_with_provenance(
            base_style,
            &model_2.class_defs,
            classes,
            styles,
        );
        let title_metrics = measure_flowchart_svg_label_for_layout_with_terminal_foreground(
            svg_label_sidecar,
            Some(FlowchartSvgLabelOwner::SubgraphTitle(subgraph_index)),
            Some(sg.id.as_str()),
            FlowchartLabelMetricsRequest {
                measurer,
                raw_label: title,
                label_type,
                style: title_text_style.as_ref(),
                max_width_px: title_width_limit,
                wrap_mode: cluster_wrap_mode,
                config: effective_config,
                math_renderer,
            },
            title_text_style.terminal_foreground(),
            FlowchartSvgWidthMode::Bbox,
        );
        let title_label = LayoutLabel {
            x: cx,
            y: cy - rect.height() / 2.0 + title_margin_top + title_metrics.height / 2.0,
            width: title_metrics.width,
            height: title_metrics.height,
        };





        let title_w = title_metrics.width.max(1.0);
        let diff = if base_width <= title_w {
            (title_w - base_width) / 2.0 - cluster_padding / 2.0
        } else {
            -cluster_padding / 2.0
        };
        let offset_y = title_metrics.height - cluster_padding / 2.0;

        let effective_dir = effective_dir_by_id
            .get(&sg.id)
            .cloned()
            .unwrap_or_else(|| effective_cluster_dir(sg, &diagram_direction, inherit_dir));

        clusters.push(LayoutCluster {
            id: sg.id.clone(),
            x: cx,
            y: cy,
            width: rect.width(),
            height: rect.height(),
            diff,
            offset_y,
            title: sg.title.clone(),
            title_label,
            requested_dir: sg.dir.as_ref().map(|s| normalize_dir(s)),
            effective_dir,
            padding: cluster_padding,
            title_margin_top,
            title_margin_bottom,
        });

        out_nodes.push(LayoutNode {
            id: sg.id.clone(),
            x: cx,

            y: cy,
            width: rect.width(),
            height: rect.height(),
            is_cluster: true,
            label_width: None,
            label_height: None,
        });
    }

    let cluster_sort_work = checked_n_log_n(clusters.len(), work_control)?;
    work_control.charge_adapter(cluster_sort_work)?;
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

    let mut out_edge_candidates: Vec<FlowchartLayoutEdgeCandidate> = Vec::new();
    work_control.charge_adapter(g.edge_slot_count())?;
    let root_edge_point_work = g.edges().try_fold(0usize, |total, edge_key| {
        let points = g.edge_by_key(edge_key).map_or(0, |edge| edge.points.len());
        work_control.checked_add(total, points)
    })?;
    let mut extracted_edge_point_work = 0usize;

    for id in &extracted_order {
        let graph = extracted_graphs
            .get(id)
            .expect("the extraction order references an extracted graph");
        work_control.charge_adapter(graph.edge_slot_count())?;
        extracted_edge_point_work =
            graph
                .edges()
                .try_fold(extracted_edge_point_work, |inner_total, edge_key| {
                    let points = graph
                        .edge_by_key(edge_key)
                        .map_or(0, |edge| edge.points.len());
                    work_control.checked_add(inner_total, points)
                })?;
    }

    let edge_projection_work = work_control.checked_add(
        render_edges.len(),
        work_control.checked_add(root_edge_point_work, extracted_edge_point_work)?,
    )?;
    work_control.charge_adapter(edge_projection_work)?;

    for ((e, expected_self_loop_meta), owner_index) in render_edges
        .iter()
        .zip(&render_edge_self_loop_meta)
        .zip(&render_edge_owner_indices)
    {
        let segment_key =
            FlowchartLayoutSegmentKey::new(*owner_index, expected_self_loop_meta.as_ref());
        let (
            points,
            label_pos,
            mut from_cluster,
            mut to_cluster,
            layout_from,
            layout_to,
            actual_self_loop_meta,
        ) = if let Some(points) = edge_override_points.get(&segment_key) {
            let from_cluster = match (edge_override_from_cluster
                .get(&segment_key)
                .cloned(), None) {
                (::core::option::Option::Some(option_value), alternate) => { ::core::mem::drop(alternate); option_value },
                (::core::option::Option::None, alternate) => alternate,
            };
            let to_cluster = match (edge_override_to_cluster
                .get(&segment_key)
                .cloned(), None) {
                (::core::option::Option::Some(option_value), alternate) => { ::core::mem::drop(alternate); option_value },
                (::core::option::Option::None, alternate) => alternate,
            };
            (
                points.clone(),
                match (edge_override_label
                    .get(&segment_key)
                    .cloned(), None) {
                    (::core::option::Option::Some(option_value), alternate) => { ::core::mem::drop(alternate); option_value },
                    (::core::option::Option::None, alternate) => alternate,
                },
                from_cluster,
                to_cluster,
                edge_override_endpoints
                    .get(&segment_key)
                    .map(|(from, _)| from.clone())
                    .unwrap_or_else(|| e.from.clone()),
                edge_override_endpoints
                    .get(&segment_key)
                    .map(|(_, to)| to.clone())
                    .unwrap_or_else(|| e.to.clone()),
                edge_override_self_loop_meta.get(&segment_key).cloned(),
            )
        } else {
            let (v, w) = {
                let optional_v = edge_endpoints_by_segment.get(&segment_key).cloned();

                match optional_v {
                    Some((matched_v, matched_w)) => (matched_v, matched_w),
                    ::core::option::Option::None => continue,
                }
            };

            let edge_key = {
                let optional_edge_key = edge_key_by_segment.get(&segment_key).map(String::as_str);

                match optional_edge_key {
                    Some(matched_edge_key) => matched_edge_key,
                    ::core::option::Option::None => continue,
                }
            };

            let label = {
                let optional_label = g.edge(&v, &w, Some(edge_key));

                match optional_label {
                    Some(matched_label) => matched_label,
                    ::core::option::Option::None => continue,
                }
            };

            let from_cluster = label
                .extras
                .get("fromCluster")
                .and_then(|v_2| v_2.as_str().map(|s| s.to_string()));
            let to_cluster = label
                .extras
                .get("toCluster")
                .and_then(|v_3| v_3.as_str().map(|s| s.to_string()));
            let points = label
                .points
                .iter()
                .map(|p| LayoutPoint { x: p.x, y: p.y })
                .collect::<Vec<_>>();
            let label_pos = match (label.x, label.y) {
                (Some(x), Some(y)) if label.width > 0.0 || label.height > 0.0 => {
                    Some(LayoutLabel {
                        x,
                        y,
                        width: label.width,
                        height: label.height,
                    })
                }
                _ => None,
            };
            let self_loop_meta = flowchart_self_loop_segment_meta(label);
            (
                points,
                label_pos,
                from_cluster,
                to_cluster,
                v,
                w,
                self_loop_meta,
            )
        };




        if expected_self_loop_meta.is_some()
            && actual_self_loop_meta.as_ref() != expected_self_loop_meta.as_ref()
        {
            continue;
        }




        if cluster_subgraph_ids.contains(e.from.as_str())
            && actual_self_loop_meta
                .as_ref()
                .is_some_and(|meta| meta.order == 0)
        {
            from_cluster = Some(e.from.clone());
        }

        if cluster_subgraph_ids.contains(e.to.as_str())
            && actual_self_loop_meta
                .as_ref()
                .is_some_and(|meta| meta.order == 2)
        {
            to_cluster = Some(e.to.clone());
        }

        out_edge_candidates.push(FlowchartLayoutEdgeCandidate {
            owner_index: *owner_index,
            edge: LayoutEdge {
                id: e.id.clone(),
                from: layout_from,
                to: layout_to,
                from_cluster,
                to_cluster,
                points,
                label: label_pos,
                start_label_left: None,
                start_label_right: None,
                end_label_left: None,
                end_label_right: None,
                start_marker: None,
                end_marker: None,
                stroke_dasharray: None,
            },
            self_loop: actual_self_loop_meta,
        });
    }

    let merged_edges = if merge_self_loops {
        let merge_work = work_control.checked_add(
            work_control.checked_add(model_2.edges.len(), out_edge_candidates.len())?,
            out_nodes.len(),
        )?;
        work_control.charge_adapter(merge_work)?;
        merge_flowchart_self_loop_segments(
            &model_2.edges,
            &out_nodes,
            &diagram_direction,
            out_edge_candidates,
        )
    } else {
        work_control.charge_adapter(out_edge_candidates.len())?;
        out_edge_candidates
    };
    let edge_owner_keys = merged_edges
        .iter()
        .map(|candidate| match &candidate.self_loop {
            Some(segment) => crate::flowchart::FlowchartEdgeKey::self_loop_segment(
                candidate.owner_index,
                segment.order,
            ),
            None => crate::flowchart::FlowchartEdgeKey::new(candidate.owner_index),
        })
        .collect();
    let mut out_edges = merged_edges
        .into_iter()
        .map(|candidate| candidate.edge)
        .collect::<Vec<_>>();







    let endpoint_work = work_control.checked_add(
        work_control.checked_add(model_2.nodes.len(), out_nodes.len())?,
        out_edges.len(),
    )?;
    work_control.charge_adapter(endpoint_work)?;
    let mut node_shape_by_id: HashMap<&str, &str> = HashMap::new();

    for n in &model_2.nodes {
        match n.layout_shape.as_deref() {
            Some(s) => {
                node_shape_by_id.insert(n.id.as_str(), s);
            },
            ::core::option::Option::None => {},
        }
    }

    let mut layout_node_by_id: HashMap<&str, &LayoutNode> = HashMap::new();

    for n in &out_nodes {
        layout_node_by_id.insert(n.id.as_str(), n);
    }

    #[must_use]
    fn diamond_intersection(node: &LayoutNode, toward: &LayoutPoint) -> Option<LayoutPoint> {
        let vx = toward.x - node.x;
        let vy = toward.y - node.y;

        if !(vx.is_finite() && vy.is_finite()) {
            return None;
        }

        if vx.abs() <= 1e-12 && vy.abs() <= 1e-12 {
            return None;
        }

        let hw = (node.width / 2.0).max(1e-9);
        let hh = (node.height / 2.0).max(1e-9);
        let denom = vx.abs() / hw + vy.abs() / hh;

        if !(denom.is_finite() && denom > 0.0) {
            return None;
        }

        let t = 1.0 / denom;
        Some(LayoutPoint {
            x: node.x + vx * t,
            y: node.y + vy * t,
        })
    }

    for e in &mut out_edges {
        if e.points.len() < 2 {
            continue;
        }

        if let Some(node) = layout_node_by_id.get(e.from.as_str())
            && !node.is_cluster
        {
            let shape = node_shape_by_id
                .get(e.from.as_str())
                .copied()
                .unwrap_or("squareRect");

            if matches!(shape, "diamond" | "question" | "diam")
                && let Some(p) = diamond_intersection(node, &e.points[1])
            {
                e.points[0] = p;
            }
        }

        if let Some(node) = layout_node_by_id.get(e.to.as_str())
            && !node.is_cluster
        {
            let shape = node_shape_by_id
                .get(e.to.as_str())
                .copied()
                .unwrap_or("squareRect");

            if matches!(shape, "diamond" | "question" | "diam") {
                let n = e.points.len();

                {
                    match diamond_intersection(node, &e.points[n - 2]) {
                        Some(p) => {
                            e.points[n - 1] = p;
                        },
                        ::core::option::Option::None => {},
                    };
                }
            }
        }
    }

    let bounds = compute_bounds_controlled(&out_nodes, &out_edges, |units| {
        work_control.charge_adapter(units)
    })?;

    Ok(FlowchartLayout {
        nodes: out_nodes,
        edges: out_edges,
        edge_owners: crate::flowchart::FlowchartEdgeOwners::new(edge_owner_keys),
        clusters,
        bounds,
        dom_node_order_by_root,
        uses_elk_adapter_dom: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "diagram-flowchart")]
    use merman_core::{MermaidEngine, ParseOptions};
    use merman_model::presentation::RenderSemanticModel;
    use merman_model::{CancelReason, OperationControl, OperationPhase};
    use std::sync::mpsc;
    use std::time::Duration;

    const DEEP_SUBGRAPH_DEPTH: usize = 10_000;
    #[cfg(feature = "diagram-flowchart")]
    const NON_LATTICE_COMPUTED_LENGTH_PX: f64 = 73.123_456_789;

    #[cfg(feature = "diagram-flowchart")]
    struct NonLatticeComputedLengthMeasurer;

    #[cfg(feature = "diagram-flowchart")]
    impl TextMeasurer for NonLatticeComputedLengthMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> crate::text::TextMetrics {
            crate::text::TextMetrics {
                width: 40.0,
                height: 18.0,
                line_count: 1,
            }
        }

        fn measure_svg_text_computed_length_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            NON_LATTICE_COMPUTED_LENGTH_PX
        }
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn dagre_wrapped_labels_fit_rectangle_and_diamond_nodes() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                r#"%%{init: {"flowchart": {"wrappingWidth": 120}}}%%
flowchart TD
    A["Receive a new deployment request from the release pipeline"] --> B{"Did every required validation and security check pass successfully?"}
    B -->|Yes| C["Publish the approved application version to the production environment"]
    B -->|No| D["Return the detailed validation errors to the requesting developer"]
"#,
                ParseOptions::default(),
            )
            .expect("parse ok")
            .expect("diagram detected").into_presentation();
        let RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("expected Flowchart render model");
        };
        let measurer = crate::text::DeterministicTextMeasurer::default()
            .with_width_callback(|text, _| text.chars().count() as f64 * 10.0);
        let layout = layout_flowchart_typed(
            model,
            &parsed.metadata().effective_config,
            measurer.as_ref(),
            None,
        )
        .expect("layout ok");

        for id in ["A", "C", "D"] {
            let node = layout
                .nodes
                .iter()
                .find(|node| node.id == id)
                .unwrap_or_else(|| panic!("node {id}"));
            let label_width = node.label_width.expect("rectangle label width");
            let label_height = node.label_height.expect("rectangle label height");
            assert!(
                label_height > 24.0,
                "node {id} label did not wrap: {node:?}"
            );
            assert!(label_width < node.width, "node {id}: {node:?}");
            assert!(label_height < node.height, "node {id}: {node:?}");
        }

        let diamond = layout
            .nodes
            .iter()
            .find(|node| node.id == "B")
            .expect("diamond node B");
        let label_width = diamond.label_width.expect("diamond label width");
        let label_height = diamond.label_height.expect("diamond label height");
        assert!(
            label_height > 24.0,
            "diamond label did not wrap: {diamond:?}"
        );
        assert_eq!(diamond.width, diamond.height);
        assert!(
            label_width + label_height < diamond.width,
            "diamond label does not fit inside its sloped sides: {diamond:?}"
        );
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn dagre_preserves_operation_computed_length_precision() {

        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "%%{init: {\"htmlLabels\": false, \"flowchart\": {\"htmlLabels\": false, \"minNodeWidth\": 0}}}%%\nflowchart TB\nA[alpha]\n",
                ParseOptions::default(),
            )
            .expect("parse ok")
            .expect("diagram detected").into_presentation();
        let RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("expected Flowchart render model");
        };
        let layout = layout_flowchart_typed(
            model,
            &parsed.metadata().effective_config,
            &NonLatticeComputedLengthMeasurer,
            None,
        )
        .expect("layout ok");
        let node = layout
            .nodes
            .iter()
            .find(|node| node.id == "A")
            .expect("node A");

        assert_eq!(node.label_width, Some(NON_LATTICE_COMPUTED_LENGTH_PX));
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn dagre_nonempty_subgraph_title_reuses_its_prepared_layout_measurement() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\nsubgraph S[service title]\n  A\nend\n",
                ParseOptions::default(),
            )
            .expect("parse ok")
            .expect("diagram detected").into_presentation();
        let RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("expected Flowchart render model");
        };
        let labels = FlowchartRenderContext::default();
        let builder = FlowchartSvgLabelSidecarBuilder::default();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("render session");
        let measurer = session.text_measurer(crate::environment::TextMeasurementPhase::Layout);
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
            model,
            &parsed.metadata().effective_config,
            false,
            &meter,
        )
        .expect("prepare edge styles");
        let mut work_control = DagreOperationWorkControl::new(meter);

        layout_flowchart_with_model(
            model,
            &labels,
            &parsed.metadata().effective_config,
            &measurer,
            None,
            Some(&builder),
            &edge_style_plan,
            &mut work_control,
        )
        .expect("layout ok");

        assert!(builder.prepared_count() > 0);
        assert!(
            builder.prepared_hit_count() > 0,
            "the cluster rect/title stages must reuse the same built-in measurement"
        );
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn dagre_html_nbsp_only_edge_labels_respect_source_provenance() {
        let nbsp = '\u{00A0}';
        let source = format!("flowchart LR\nA -- \"&nbsp;\" --> B\nC -- \"{nbsp}\" --> D\n");
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
            .expect("parse ok")
            .expect("diagram detected")
            .into_presentation();
        let RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("expected Flowchart render model");
        };
        let layout = layout_flowchart_typed(
            model,
            &parsed.metadata().effective_config,
            &crate::text::DeterministicTextMeasurer::default(),
            None,
        )
        .expect("layout ok");

        assert_eq!(layout.edges.len(), 2);
        let entity_label = layout
            .edges
            .iter()
            .find(|edge| edge.from == "A")
            .and_then(|edge| edge.label.as_ref())
            .expect("entity-authored NBSP label must reach Dagre");
        assert!(entity_label.width > 0.0, "{entity_label:?}");
        assert!(entity_label.height > 0.0, "{entity_label:?}");

        let direct_label = layout
            .edges
            .iter()
            .find(|edge| edge.from == "C")
            .and_then(|edge| edge.label.as_ref());
        assert!(direct_label.is_none(), "{direct_label:?}");
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn dagre_operation_meter_honors_below_equal_and_above_boundaries() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TB\nsubgraph Outer\nsubgraph Inner\nA-->B\nend\nB-->C\nend\nC-->C\n",
                ParseOptions::default(),
            )
            .expect("parse ok")
            .expect("diagram detected")
            .into_presentation();
        let RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("expected Flowchart render model");
        };
        let config = &parsed.metadata().effective_config;
        let measurer = crate::text::DeterministicTextMeasurer::default();

        let unbounded_meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let expected = layout_flowchart_typed_with_work_meter(
            model,
            config,
            &measurer,
            None,
            Arc::clone(&unbounded_meter),
        )
        .expect("unbounded Dagre layout succeeds");
        let exact = unbounded_meter.used();
        assert!(exact > 1);

        let below_meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, exact - 1)
                .unwrap(),
        ));
        let error = layout_flowchart_typed_with_work_meter(
            model,
            config,
            &measurer,
            None,
            Arc::clone(&below_meter),
        )
        .expect_err("one unit below the exact Dagre work must fail");
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected ResourceLimitExceeded");
        };
        assert_eq!(limit.actual, exact);
        assert_eq!(limit.max, exact - 1);
        assert!(below_meter.used() < exact);

        for limit in [exact, exact + 1] {
            let meter = Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, limit)
                    .unwrap(),
            ));
            let actual = layout_flowchart_typed_with_work_meter(
                model,
                config,
                &measurer,
                None,
                Arc::clone(&meter),
            )
            .expect("equal and above Dagre work budgets succeed");
            assert_eq!(meter.used(), exact);
            assert_eq!(
                actual.dom_node_order_by_root,
                expected.dom_node_order_by_root
            );
            assert_eq!(actual.uses_elk_adapter_dom, expected.uses_elk_adapter_dom);
            assert_eq!(
                serde_json::to_value(&actual).unwrap(),
                serde_json::to_value(&expected).unwrap()
            );
        }
    }

    #[test]
    fn dagre_adapter_rejects_before_a_tombstoned_edge_snapshot() {
        let mut graph = compound_graph();
        for id in ["a", "b", "c", "d"] {
            graph.set_node(id, NodeLabel::default());
        }
        for (from, to) in [("a", "b"), ("b", "c"), ("c", "d"), ("d", "a")] {
            graph.set_edge(from, to);
        }
        assert!(graph.remove_edge("b", "c", None));

        let node_count = graph.node_count();
        let edge_count = graph.edge_count();
        let before_edge_snapshot = graph.node_order_slot_count() + node_count + node_count;
        let edge_snapshot_work = graph.edge_slot_count() + edge_count;
        assert!(
            graph.edge_slot_count() > edge_count,
            "the removed non-tail edge must leave a tombstone"
        );
        let attempted_after_edge_snapshot = before_edge_snapshot + edge_snapshot_work;
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    crate::ResourceLimitId::MaxLayoutWorkUnits,
                    attempted_after_edge_snapshot - 1,
                )
                .unwrap(),
        ));
        let mut work_control = DagreOperationWorkControl::new(Arc::clone(&meter));

        let error = adjust_flowchart_clusters_and_edges(&mut graph, &mut work_control)
            .expect_err("the complete edge snapshot must be admitted before it is cloned");
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected ResourceLimitExceeded");
        };
        assert_eq!(meter.used(), before_edge_snapshot);
        assert_eq!(limit.actual, attempted_after_edge_snapshot);
        assert_eq!(limit.max, attempted_after_edge_snapshot - 1);

        let sticky = work_control
            .charge_adapter(1)
            .expect_err("a rejected owner tranche must remain sticky");
        let Error::ResourceLimitExceeded(sticky) = sticky else {
            panic!("expected sticky ResourceLimitExceeded");
        };
        assert_eq!(sticky.actual, attempted_after_edge_snapshot);
        assert_eq!(meter.used(), before_edge_snapshot);
    }

    #[test]
    fn dagre_adapter_checked_work_overflow_maps_to_the_resource_contract() {
        let mut work_control = DagreOperationWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )));
        let error = work_control.checked_mul(usize::MAX, 2).unwrap_err();
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected ResourceLimitExceeded");
        };
        assert_eq!(limit.limit, "max_layout_work_units");
        let Error::ResourceLimitExceeded(sticky) = work_control
            .charge_adapter(1)
            .expect_err("arithmetic overflow must remain sticky")
        else {
            panic!("expected sticky ResourceLimitExceeded");
        };
        assert_eq!(sticky, limit);
    }

    #[test]
    fn dagre_kernel_rejection_remains_sticky_after_error_mapping() {
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, 1)
                .unwrap(),
        ));
        let mut work_control = DagreOperationWorkControl::new(Arc::clone(&meter));

        let interrupted = dugong::WorkControl::charge(&mut work_control, 2)
            .expect_err("the kernel charge must exceed the operation budget");
        let Error::ResourceLimitExceeded(mapped) = work_control.map_dugong_error(interrupted)
        else {
            panic!("expected ResourceLimitExceeded");
        };
        let Error::ResourceLimitExceeded(sticky) = work_control
            .charge_adapter(1)
            .expect_err("mapping the kernel error must not consume its sticky rejection")
        else {
            panic!("expected sticky ResourceLimitExceeded");
        };

        assert_eq!(sticky, mapped);
        assert_eq!(meter.used(), 0);
    }

    #[test]
    fn dagre_kernel_interruption_preserves_operation_cancellation() {
        let control = OperationControl::new();
        control.cancel();
        let meter = Arc::new(OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control,
        ));
        let mut work_control = DagreOperationWorkControl::new(Arc::clone(&meter));

        let interrupted = dugong::WorkControl::charge(&mut work_control, 1)
            .expect_err("the cancelled operation must interrupt the kernel");
        let Error::Cancelled(cancelled) = work_control.map_dugong_error(interrupted) else {
            panic!("expected structured operation cancellation");
        };

        assert_eq!(cancelled.phase, OperationPhase::Layout);
        assert_eq!(cancelled.reason, CancelReason::Requested);
        assert_eq!(meter.used(), 0);
    }

    #[test]
    fn dagre_kernel_invalid_tree_maps_to_invalid_model() {
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let mut work_control = DagreOperationWorkControl::new(Arc::clone(&meter));

        let Error::InvalidModel { message } =
            work_control.map_dugong_error(dugong::LayoutError::InvalidNetworkSimplexTree)
        else {
            panic!("expected InvalidModel");
        };

        assert_eq!(
            message,
            "network simplex encountered an invalid mutable tree state"
        );
        assert_eq!(meter.used(), 0);
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn dagre_kernel_arithmetic_overflow_maps_to_the_resource_contract() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync("flowchart TB\nA-->B\n", ParseOptions::default())
            .expect("parse ok")
            .expect("diagram detected")
            .into_presentation();
        let RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("expected Flowchart render model");
        };
        let mut model = model.clone();
        model.edges[0].length = usize::MAX;

        let error = layout_flowchart_typed_with_work_meter(
            &model,
            &parsed.metadata().effective_config,
            &crate::text::DeterministicTextMeasurer::default(),
            None,
            Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input(),
            )),
        )
        .expect_err("Dugong arithmetic overflow must fail closed");
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected ResourceLimitExceeded");
        };
        assert_eq!(limit.limit, "max_layout_work_units");
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn unknown_shape_is_rejected_instead_of_becoming_a_rectangle() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TB\nA[known]\n",
                ParseOptions::default(),
            )
            .expect("parse ok")
            .expect("diagram detected")
            .into_presentation();
        let RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("expected Flowchart render model");
        };
        let mut model = model.clone();
        model.nodes[0].layout_shape = Some("definitely-unknown".to_string());
        let error = layout_flowchart_typed(
            &model,
            &parsed.metadata().effective_config,
            &crate::text::DeterministicTextMeasurer::default(),
            None,
        )
        .expect_err("unknown Flowchart shapes must not silently become rectangles");
        let Error::InvalidModel { message } = error else {
            panic!("expected InvalidModel for unknown Flowchart shape");
        };
        assert_eq!(
            message,
            "No such shape: definitely-unknown. Please check your syntax."
        );
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn shape_validation_scan_is_admitted_before_invalid_model_reporting() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TB\nA[known]\nB[also known]\n",
                ParseOptions::default(),
            )
            .expect("parse ok")
            .expect("diagram detected")
            .into_presentation();
        let RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("expected Flowchart render model");
        };
        let mut model = model.clone();
        model.nodes[1].layout_shape = Some("definitely-unknown".to_string());

        let error = layout_flowchart_typed_with_work_meter(
            &model,
            &parsed.metadata().effective_config,
            &crate::text::DeterministicTextMeasurer::default(),
            None,
            Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, 1)
                    .unwrap(),
            )),
        )
        .expect_err("the node scan must be rejected before inspecting an invalid shape");
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected ResourceLimitExceeded");
        };
        assert_eq!(limit.actual, 2);
        assert_eq!(limit.max, 1);
    }

    #[derive(Debug)]
    struct DeepTraversalOutcome {
        descendant_count: usize,
        anchor: Option<String>,
        root_dir: Option<String>,
        child_dir: Option<String>,
        copied_node_count: usize,
    }

    fn compound_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        Graph::new(GraphOptions {
            multigraph: true,
            compound: true,
            directed: true,
        })
    }

    #[test]
    fn numeric_node_insertion_ordering_is_admitted_before_mutation() {
        let graph = compound_graph();
        let exact = 2usize;
        let below_meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, exact - 1)
                .unwrap(),
        ));
        let mut below = DagreOperationWorkControl::new(Arc::clone(&below_meter));
        let error = charge_node_insertion_ordering(&graph, "1", &mut below)
            .expect_err("the numeric node's two ordered-map insertions require admission");
        assert!(matches!(error, Error::ResourceLimitExceeded(_)));
        assert_eq!(below_meter.used(), 0);
        assert!(!graph.has_node("1"));

        for limit in [exact, exact + 1] {
            let meter = Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, limit)
                    .unwrap(),
            ));
            let mut work_control = DagreOperationWorkControl::new(Arc::clone(&meter));
            charge_node_insertion_ordering(&graph, "1", &mut work_control)
                .expect("equal and above numeric insertion budgets succeed");
            assert_eq!(meter.used(), exact);
            assert!(!graph.has_node("1"));
        }
    }

    #[test]
    fn implicit_numeric_edge_endpoints_are_admitted_before_mutation() {
        let graph = compound_graph();
        let exact = 10usize;
        let below_meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, exact - 1)
                .unwrap(),
        ));
        let mut below = DagreOperationWorkControl::new(Arc::clone(&below_meter));
        let error = charge_edge_endpoint_insertions(&graph, "1", "2", &mut below)
            .expect_err("both missing numeric endpoints require admission before set_edge");
        assert!(matches!(error, Error::ResourceLimitExceeded(_)));
        assert_eq!(below_meter.used(), 0);
        assert_eq!(graph.node_count(), 0);

        for limit in [exact, exact + 1] {
            let meter = Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(crate::ResourceLimitId::MaxLayoutWorkUnits, limit)
                    .unwrap(),
            ));
            let mut work_control = DagreOperationWorkControl::new(Arc::clone(&meter));
            charge_edge_endpoint_insertions(&graph, "1", "2", &mut work_control)
                .expect("equal and above endpoint budgets succeed");
            assert_eq!(meter.used(), exact);
            assert_eq!(graph.node_count(), 0);
        }
    }

    #[test]
    fn child_snapshot_work_includes_ordinary_bucket_tombstones() {
        let mut graph = compound_graph();
        for id in ["parent", "other", "a", "b", "c"] {
            graph.set_node(id, NodeLabel::default());
        }
        for child in ["a", "b", "c"] {
            graph.set_parent_ref(child, "parent");
        }
        graph.set_parent_ref("b", "other");

        let work_control = DagreOperationWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )));
        assert_eq!(graph.child_count("parent"), 2);
        assert_eq!(graph.child_order_slot_count("parent"), 3);
        assert_eq!(
            child_id_snapshot_work_upper_bound(&graph, "parent", &work_control).unwrap(),
            5
        );
    }

    #[test]
    fn remove_node_numeric_child_promotion_curve_charges_the_logarithmic_term() {
        for width in (0..=10).map(|shift| 1usize << shift) {
            let mut numeric = compound_graph();
            let mut ordinary = compound_graph();
            numeric.set_node("parent", NodeLabel::default());
            ordinary.set_node("parent", NodeLabel::default());
            for index in 0..width {
                let numeric_id = index.to_string();
                numeric.set_node(&numeric_id, NodeLabel::default());
                numeric.set_parent_ref(&numeric_id, "parent");

                let ordinary_id = format!("child-{index}");
                ordinary.set_node(&ordinary_id, NodeLabel::default());
                ordinary.set_parent_ref(&ordinary_id, "parent");
            }

            let meter = Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input(),
            ));
            let work_control = DagreOperationWorkControl::new(meter);
            let numeric_bound =
                remove_node_work_upper_bound(&numeric, "parent", &work_control).unwrap();
            let ordinary_bound =
                remove_node_work_upper_bound(&ordinary, "parent", &work_control).unwrap();
            let ordered_term =
                ordered_key_update_work_upper_bound(width, width, &work_control).unwrap();




            let root_tombstone_savings = ordinary
                .root_child_order_slot_count()
                .saturating_sub(numeric.root_child_order_slot_count());
            assert_eq!(
                numeric_bound + root_tombstone_savings,
                ordinary_bound + ordered_term
            );
        }
    }

    #[test]
    fn flowchart_third_self_loop_segment_uses_mermaid_11_16_graph_key() {
        let edge = FlowEdge {
            id: "A-cyclic-special-2".to_string(),
            from: "A---A---2".to_string(),
            to: "A".to_string(),
            label: Some(String::new()),
            label_type: None,
            edge_type: None,
            arrow: "-->".to_string(),
            start_marker: FlowEdgeMarker::None,
            end_marker: FlowEdgeMarker::Point,
            is_user_defined_id: false,
            stroke: None,
            stroke_kind: FlowEdgeStroke::Normal,
            visibility: FlowEdgeVisibility::Visible,
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            step: None,
            length: 1,
        };
        let meta = FlowchartSelfLoopSegmentMeta {
            owner_index: 0,
            logical_edge_id: "L_A_A_0".to_string(),
            node_id: "A".to_string(),
            order: 2,
        };

        assert_eq!(
            flowchart_layout_edge_key(&edge, Some(&meta)),
            "A-cyclic-special-2"
        );
    }

    fn deep_compound_graph(depth: usize) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = compound_graph();
        for i in (0..depth).rev() {
            graph.set_parent(format!("n{}", i + 1), format!("n{i}"));
        }
        graph
    }

    fn deep_subgraphs(depth: usize) -> Vec<FlowSubgraph> {
        let mut subgraphs = Vec::with_capacity(depth);
        for i in 0..depth {
            subgraphs.push(FlowSubgraph {
                metadata: None,
                id: format!("n{i}"),
                title: format!("n{i}"),
                dir: None,
                has_explicit_dir: false,
                label_type: None,
                classes: Vec::new(),
                styles: Vec::new(),
                nodes: vec![format!("n{}", i + 1)],
            });
        }
        subgraphs
    }

    #[test]
    fn parent_batch_handles_a_long_acyclic_chain_without_parent_walks() {
        let count = 8_192usize;
        let mut graph = compound_graph();
        graph.set_node("root", NodeLabel::default());
        for index in 0..count {
            graph.set_node(format!("n{index}"), NodeLabel::default());
        }
        let root_ix = graph.node_ix("root").unwrap();
        let assignments = (0..count)
            .map(|index| {
                let child_ix = graph.node_ix(&format!("n{index}")).unwrap();
                let parent_ix = if index == 0 {
                    root_ix
                } else {
                    graph.node_ix(&format!("n{}", index - 1)).unwrap()
                };
                (child_ix, parent_ix)
            })
            .collect::<Vec<_>>();
        let mut work_control = DagreOperationWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )));

        apply_unparented_parent_assignments(
            &mut graph,
            &assignments,
            UnparentedParentBatchShape::default(),
            &mut work_control,
        )
        .expect("the deep functional-parent chain is acyclic");
        assert_eq!(graph.parent("n0"), Some("root"));
        assert_eq!(graph.parent(&format!("n{}", count - 1)), Some("n8190"));
    }

    #[test]
    fn parent_batch_matches_mermaid_graphlib_assignment_order() {



        fn sequential_parent_walk_reference(
            order: &[&str],
            parent_by_id: &HashMap<String, String>,
        ) -> Option<(String, String)> {
            let mut assigned_parent: HashMap<String, String> = HashMap::new();
            for &child in order {
                let Some(parent) = parent_by_id.get(child) else {
                    continue;
                };
                let mut current = Some(parent.as_str());
                while let Some(id) = current {
                    if id == child {
                        return Some((child.to_string(), parent.clone()));
                    }
                    current = assigned_parent.get(id).map(String::as_str);
                }
                assigned_parent.insert(child.to_string(), parent.clone());
            }
            None
        }

        let cases = [
            (
                [("a", "root"), ("b", "root"), ("c", "a"), ("d", "a")],
                vec!["a", "b", "c", "d"],
            ),
            (
                [("a", "b"), ("b", "c"), ("c", "a"), ("d", "root")],
                vec!["a", "b", "c", "d"],
            ),
            (
                [("a", "b"), ("b", "c"), ("c", "a"), ("d", "root")],
                vec!["b", "c", "a", "d"],
            ),
            (
                [("a", "b"), ("b", "a"), ("c", "b"), ("d", "root")],
                vec!["c", "a", "b", "d"],
            ),
            (
                [("a", "a"), ("b", "root"), ("c", "b"), ("d", "c")],
                vec!["b", "c", "a", "d"],
            ),
            (
                [("a", "b"), ("b", "c"), ("c", "a"), ("d", "root")],
                vec!["a", "a", "b", "c", "d"],
            ),
        ];

        for (assignments, order) in cases {
            let parent_by_id = assignments
                .into_iter()
                .map(|(child, parent)| (child.to_string(), parent.to_string()))
                .collect::<HashMap<_, _>>();
            let expected = sequential_parent_walk_reference(&order, &parent_by_id);
            let mut graph = compound_graph();
            for id in order
                .iter()
                .copied()
                .chain(parent_by_id.values().map(String::as_str))
            {
                graph.set_node(id, NodeLabel::default());
            }
            let mut assigned = HashSet::new();
            let parent_assignments = order
                .iter()
                .copied()
                .filter_map(|child| {
                    let parent = parent_by_id.get(child)?;
                    assigned.insert(child).then(|| {
                        (
                            graph.node_ix(child).unwrap(),
                            graph.node_ix(parent).unwrap(),
                        )
                    })
                })
                .collect::<Vec<_>>();
            let mut work_control = DagreOperationWorkControl::new(Arc::new(
                OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
            ));
            let actual = apply_unparented_parent_assignments(
                &mut graph,
                &parent_assignments,
                UnparentedParentBatchShape::default(),
                &mut work_control,
            );

            match expected {
                None => actual.expect("the reference parent sequence is acyclic"),
                Some((child, parent)) => {
                    let Error::InvalidModel { message } = actual.unwrap_err() else {
                        panic!("expected InvalidModel for order {order:?}");
                    };
                    assert_eq!(
                        message,
                        format!("Setting {parent} as parent of {child} would create a cycle"),
                        "cycle assignment mismatch for order {order:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn parent_batch_adapter_charges_the_documented_width_and_depth_bound() {
        for size in (0..=10).map(|shift| 1usize << shift) {
            let mut width_graph = compound_graph();
            width_graph.set_node("root", NodeLabel::default());
            for index in 0..size {
                width_graph.set_node(format!("child-{index}"), NodeLabel::default());
            }
            let root_ix = width_graph.node_ix("root").unwrap();
            let width_assignments = (0..size)
                .map(|index| {
                    (
                        width_graph.node_ix(&format!("child-{index}")).unwrap(),
                        root_ix,
                    )
                })
                .collect::<Vec<_>>();
            let width_meter = Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input(),
            ));
            let mut width_control = DagreOperationWorkControl::new(Arc::clone(&width_meter));
            let expected_width_work = unparented_parent_batch_work_upper_bound(
                width_graph.node_slot_count(),
                0,
                width_assignments.len(),
                0,
                &width_control,
            )
            .unwrap();

            apply_unparented_parent_assignments(
                &mut width_graph,
                &width_assignments,
                UnparentedParentBatchShape::default(),
                &mut width_control,
            )
            .expect("the unbounded width batch succeeds");

            assert_eq!(width_meter.used(), expected_width_work);
            assert_eq!(width_graph.child_count("root"), size);
            assert_eq!(
                width_graph.children("root").first().copied(),
                Some("child-0")
            );
            let last_child = format!("child-{}", size - 1);
            assert_eq!(
                width_graph.children("root").last().copied(),
                Some(last_child.as_str())
            );

            let mut depth_graph = compound_graph();
            for index in 0..=size {
                depth_graph.set_node(format!("node-{index}"), NodeLabel::default());
            }
            let depth_assignments = (1..=size)
                .rev()
                .map(|index| {
                    (
                        depth_graph.node_ix(&format!("node-{index}")).unwrap(),
                        depth_graph.node_ix(&format!("node-{}", index - 1)).unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            let depth_meter = Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input(),
            ));
            let mut depth_control = DagreOperationWorkControl::new(Arc::clone(&depth_meter));
            let expected_depth_work = unparented_parent_batch_work_upper_bound(
                depth_graph.node_slot_count(),
                0,
                depth_assignments.len(),
                0,
                &depth_control,
            )
            .unwrap();

            apply_unparented_parent_assignments(
                &mut depth_graph,
                &depth_assignments,
                UnparentedParentBatchShape::default(),
                &mut depth_control,
            )
            .expect("the unbounded depth batch succeeds");

            assert_eq!(depth_meter.used(), expected_depth_work);
            assert_eq!(depth_graph.parent("node-0"), None);
            let deepest = format!("node-{size}");
            let expected_parent = format!("node-{}", size - 1);
            assert_eq!(depth_graph.parent(&deepest), Some(expected_parent.as_str()));
        }
    }

    #[test]
    fn parent_batch_adapter_charges_only_real_existing_and_numeric_work() {
        let control = DagreOperationWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )));

        assert_eq!(
            unparented_parent_batch_work_upper_bound(4, 0, 3, 0, &control).unwrap(),
            66
        );
        assert_eq!(
            unparented_parent_batch_work_upper_bound(4, 0, 3, 2, &control).unwrap(),
            78
        );
        assert_eq!(
            unparented_parent_batch_work_upper_bound(4, 3, 3, 0, &control).unwrap(),
            102
        );
        assert_eq!(
            unparented_parent_batch_work_upper_bound(4, 3, 0, 0, &control).unwrap(),
            0
        );
    }

    #[test]
    fn flowchart_cluster_traversals_handle_deep_subgraphs_with_small_stack() {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("flowchart-deep-subgraph-traversal".to_string())
            .stack_size(512 * 1024)
            .spawn(move || {
                let mut graph = deep_compound_graph(DEEP_SUBGRAPH_DEPTH);
                let mut work_control = DagreOperationWorkControl::new(Arc::new(
                    OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
                ));
                let descendants = extract_descendants("n0", &graph, &mut work_control)
                    .expect("unbounded work control accepts descendant extraction");
                let anchor =
                    flowchart_find_non_cluster_child("n0", &graph, "n0", &mut work_control)
                        .expect("unbounded work control accepts anchor lookup");
                let deep_subgraphs = deep_subgraphs(DEEP_SUBGRAPH_DEPTH);
                let deep_subgraphs_by_id = deep_subgraphs
                    .iter()
                    .map(|subgraph| (subgraph.id.as_str(), subgraph))
                    .collect();
                let dirs = compute_effective_dir_by_id(
                    &deep_subgraphs,
                    &deep_subgraphs_by_id,
                    &graph,
                    "TB",
                    false,
                    &mut work_control,
                )
                .expect("unbounded work control accepts effective-direction planning");

                let mut copied = compound_graph();
                let descendants_by_id = HashMap::from([("n0".to_string(), descendants.clone())]);
                copy_cluster(
                    "n0",
                    &mut graph,
                    &mut copied,
                    "n0",
                    &descendants_by_id,
                    &mut work_control,
                )
                .expect("unbounded work control accepts cluster copying");

                tx.send(DeepTraversalOutcome {
                    descendant_count: descendants.len(),
                    anchor,
                    root_dir: dirs.get("n0").cloned(),
                    child_dir: dirs.get("n1").cloned(),
                    copied_node_count: copied.node_count(),
                })
                .unwrap();
            })
            .expect("spawn deep subgraph traversal test");

        let outcome = rx
            .recv_timeout(Duration::from_secs(15))
            .expect("deep subgraph traversal should finish without stack overflow");

        assert_eq!(outcome.descendant_count, DEEP_SUBGRAPH_DEPTH);
        assert_eq!(outcome.anchor, Some(format!("n{DEEP_SUBGRAPH_DEPTH}")));
        assert_eq!(outcome.root_dir.as_deref(), Some("LR"));
        assert_eq!(outcome.child_dir.as_deref(), Some("TB"));
        assert_eq!(outcome.copied_node_count, DEEP_SUBGRAPH_DEPTH);
    }

    #[test]
    fn extract_descendants_handles_deeply_nested_subgraphs() {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(move || {
                let mut g = Graph::new(GraphOptions {
                    compound: true,
                    ..Default::default()
                });
                for i in (0..DEEP_SUBGRAPH_DEPTH).rev() {
                    g.set_parent(format!("n{}", i + 1), format!("n{i}"));
                }
                let mut work_control = DagreOperationWorkControl::new(Arc::new(
                    OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
                ));
                let descendants = extract_descendants("n0", &g, &mut work_control)
                    .expect("unbounded work control accepts descendant extraction");
                let _ = tx.send(descendants);
            })
            .unwrap();
        let descendants = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("extract_descendants overflowed the stack on deep nesting");
        assert_eq!(descendants.len(), DEEP_SUBGRAPH_DEPTH);
    }
}
