//! Adapter for already-measured flat diagrams using the shared Swimlane provider.

use super::config::{DEFAULT_LANE_ID, DEFAULT_LANE_PADDING, SwimlaneConfig};
use super::working::{WorkingEdge, WorkingLayout, WorkingNode, WorkingNodeKind};
use crate::Result;
use crate::model::{LayoutEdge, LayoutNode, SwimlaneDirection, SwimlaneLayout};
use crate::resources::OperationWorkMeter;
use indexmap::IndexMap;
use merman_model::presentation::PresentationConfig;
use std::sync::Arc;

pub(crate) fn layout_flat(
    measured: &[(&LayoutNode, &str)],
    edges: &[LayoutEdge],
    config: &PresentationConfig,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<SwimlaneLayout> {
    work_meter.preflight(super::swimlane_layout_preflight_work_units(
        measured.len(),
        edges.len(),
    ))?;
    work_meter.charge(super::swimlane_core_layout_work_units(
        measured.len(),
        edges.len(),
    ))?;
    let mut nodes = IndexMap::new();

    for (node, shape) in measured {
        nodes.insert(
            node.id.clone(),
            WorkingNode {
                id: node.id.clone(),
                label: String::new(),
                label_type: <::std::string::String as ::core::convert::From<&::core::primitive::str>>::from("markdown"),
                shape: (*shape).to_owned(),
                kind: WorkingNodeKind::Content,
                producer_lane_role: None,
                parent_id: Some(<::std::string::String as ::core::convert::From<&::core::primitive::str>>::from(DEFAULT_LANE_ID)),
                top_lane_id: Some(<::std::string::String as ::core::convert::From<&::core::primitive::str>>::from(DEFAULT_LANE_ID)),
                requested_dir: None,
                padding: 0.0,
                x: node.x,
                y: node.y,
                width: node.width,
                height: node.height,
                label_width: match node.label_width {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => 0.0,
                },
                label_height: match node.label_height {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => 0.0,
                },
                layer: 0,
                order: 0,
                content_top: None,
                title_rect: None,
            },
        );
    }


    if !nodes.is_empty() {
        nodes.insert(
            <::std::string::String as ::core::convert::From<&::core::primitive::str>>::from(DEFAULT_LANE_ID),
            WorkingNode {
                id: <::std::string::String as ::core::convert::From<&::core::primitive::str>>::from(DEFAULT_LANE_ID),
                label: String::new(),
                label_type: <::std::string::String as ::core::convert::From<&::core::primitive::str>>::from("text"),
                shape: <::std::string::String as ::core::convert::From<&::core::primitive::str>>::from("swimlane"),
                kind: WorkingNodeKind::Group,
                producer_lane_role: None,
                parent_id: None,
                top_lane_id: None,
                requested_dir: Some(<::std::string::String as ::core::convert::From<&::core::primitive::str>>::from("TB")),
                padding: DEFAULT_LANE_PADDING,
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
                label_width: 0.0,
                label_height: 0.0,
                layer: 0,
                order: 0,
                content_top: None,
                title_rect: None,
            },
        );
    }

    let original_edges: Vec<_> = edges
        .iter()
        .map(|edge| WorkingEdge {
            id: edge.id.clone(),
            from: edge.from.clone(),
            to: edge.to.clone(),
            reference_id: edge.id.clone(),
            label_node_id: None,
            reversed_for_layout: false,
            points: Vec::new(),
        })
        .collect();
    let mut working = WorkingLayout {
        direction: SwimlaneDirection::Tb,
        top_lane_order: if nodes.is_empty() {
            Vec::new()
        } else {
            vec![<::std::string::String as ::core::convert::From<&::core::primitive::str>>::from(DEFAULT_LANE_ID)]
        },
        nodes,
        graph_edges: original_edges.clone(),
        original_edges,
    };
    super::run_layout_core(
        &mut working,
        SwimlaneConfig::from_config(config),
        work_meter,
    )?;
    Ok(super::project_layout(
        &working,
        &std::collections::HashMap::new(),
    ))
}
