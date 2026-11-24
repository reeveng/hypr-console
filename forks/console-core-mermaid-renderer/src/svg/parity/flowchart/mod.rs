use super::*;
use console_core_perfect_hash::collections::{FixedMap, FixedSet};

mod agentflow;
mod css;
mod defs;
mod document;
mod document_ids;
mod edge;
mod edge_effect;
mod edge_geom;
mod edge_style_plan;
mod hierarchy;
mod label;
mod label_effect;
mod node_effect;
mod node_inventory;
mod node_style;
mod paint;
pub(crate) use node_inventory::{FlowchartNodeLayoutView, FlowchartPreparedNodes};
mod render;
mod render_config;
mod render_input;
mod step;
mod style;
mod swimlane;
mod types;
mod util;
mod viewbox;
mod viewbox_node_bounds;

use css::*;
use edge::*;
pub(in crate::svg::parity::flowchart) use edge_geom::{
    FlowchartEdgePathGeomRequest, flowchart_compute_edge_path_geom,
};
pub(crate) use edge_style_plan::FlowchartEdgeStylePlan;
use hierarchy::*;
pub(super) use label::*;
pub(crate) use render_config::{
    FlowchartRenderConfig, flowchart_node_label_fill_config_override,
    prepare_flowchart_render_config,
};
pub(super) use style::*;

use render::{
    FlowchartRootRenderSession, render_flowchart_edge_path, render_flowchart_elk_root_groups,
    render_flowchart_node, render_flowchart_root,
};
pub(super) use render::{render_flowchart_cluster, render_flowchart_edge_label};
use types::*;
use util::{OptionalStyleAttr, OptionalStyleXmlAttr};






pub(in crate::svg::parity::flowchart) const FLOWCHART_EDGE_LABEL_WRAP_WIDTH: f64 = 200.0;



#[inline]
#[must_use]
fn escape_attr(text: &str) -> super::util::EscapeAttrDisplay<&str> {
    escape_attr_display(text)
}

#[must_use]
pub(in crate::svg::parity::flowchart) fn flowchart_config_diagram_look(
    config: &merman_model::presentation::PresentationConfig,
) -> crate::config::DiagramLook<'_> {
    crate::config::mermaid_config_diagram_look(config)
}



mod svg_emit;
pub(super) use svg_emit::{flowchart_edge_geometry, render_flowchart_svg_artifact};
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(super) use swimlane::{render_swimlane_svg_artifact, swimlane_edge_geometry};

#[cfg(all(test, feature = "layout-elk"))]
mod browser_measurement_tests;

/// Shared by the title element and its stylesheet; Agentflow owns its title class.
#[must_use]
fn title_css_class(diagram_type: &str) -> &'static str {
    if diagram_type == "agentflow" {
        "agentflowTitleText"
    } else {
        "flowchartTitleText"
    }
}

/// Agentflow projects into the same body, edge, and glyph terminals as Flowchart.
#[must_use]
fn supports_neo_effects(ctx: &FlowchartRenderCtx<'_>) -> bool {
    ctx.compatibility.look.is_neo()
        && ctx.swimlane_direction.is_none()
        && matches!(
            merman_model::family::diagram_type_family_id(ctx.diagram_type),
            Some(crate::DiagramFamilyId::FLOWCHART | crate::DiagramFamilyId::AGENTFLOW)
        )
}
