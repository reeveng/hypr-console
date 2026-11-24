//! State diagram (stateDiagram-v2) layout.
//!
//! Source semantics: Mermaid 12.0.0.

type StateDiagramModel = merman_model::families::state::StateDiagramRenderModel;
type StateNode = merman_model::families::state::StateDiagramRenderNode;

mod label;
pub(crate) use label::{
    StateLabelMeasurement, measure_state_markdown_label, state_edge_label_xhtml,
    state_label_source_style_facts, state_markdown_label_plain_text, state_node_label_xhtml,
    state_value_to_label_text,
};

mod edge_label_geometry;
pub(crate) use edge_label_geometry::StateNativeLabelGeometry;

mod label_artifact;
pub(crate) use label_artifact::{
    StateLabelMetricsRequest, StateLabelOwner, StateLabelSidecar, StateLabelSidecarBuilder,
    StateLabelSourceKind,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RectWithTitleGeometry {
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) label_x: f64,
    pub(crate) label_y: f64,
    pub(crate) title_x: f64,
    pub(crate) description_x: f64,
    pub(crate) description_y: f64,
    pub(crate) divider_y: f64,
}

impl RectWithTitleGeometry {
    #[must_use]
    pub(crate) fn from_metrics(
        title_width: f64,
        title_height: f64,
        description_width: f64,
        description_height: f64,
        padding: f64,
    ) -> Self {
        let title_width_2 = title_width.max(0.0);
        let title_height_2 = title_height.max(0.0);
        let description_width_2 = description_width.max(0.0);
        let description_height_2 = description_height.max(0.0);
        let padding_2 = padding.max(0.0);
        let half_padding = padding_2 / 2.0;
        let label_width = title_width_2.max(description_width_2);
        let description_y = title_height_2 + half_padding + 5.0;
        let label_height = description_y + description_height_2;

        Self {
            width: (label_width + padding_2).max(1.0),
            height: (label_height + padding_2).max(1.0),
            label_x: -label_width / 2.0,
            label_y: -label_height / 2.0 - half_padding + 3.0,
            title_x: ((label_width - title_width_2) / 2.0).max(0.0),
            description_x: ((label_width - description_width_2) / 2.0).max(0.0),
            description_y,
            divider_y: -label_height / 2.0 + title_height_2,
        }
    }
}

mod compatibility;
mod config;
mod effect_plan;
#[cfg(feature = "layout-elk")]
mod elk;
mod layout;
mod style_plan;
mod theme_evidence;

pub(crate) use compatibility::{
    StateCompatibilityPlan, StateTerminalPaintProperty, StateTerminalSurface,
};
pub(crate) use config::StateConfigView;
pub(crate) use effect_plan::{StateEffectPlan, StateNodeEffectPlan};
pub(crate) use style_plan::{
    ResolvedLabelTypography, StateEdgeStylePlan, StateNodeStylePlan, StateStylePlan,
};
pub(crate) use theme_evidence::{StateAppliedThemeOutcome, StateThemeOutcomes, StateThemeSurface};

pub(crate) use layout::layout_state_diagram_typed_with_work_meter;
pub use layout::{
    debug_build_state_diagram_dagre_graph, debug_extract_state_diagram_cluster_graph,
};

#[cfg(all(test, feature = "diagram-state"))]
mod tests {
    use super::RectWithTitleGeometry;

    #[test]
    fn rect_with_title_geometry_matches_upstream_equations_for_non_default_padding() {
        let geometry = RectWithTitleGeometry::from_metrics(80.0, 24.0, 60.0, 18.0, 12.0);

        assert_eq!(geometry.width, 92.0);
        assert_eq!(geometry.height, 65.0);
        assert_eq!(geometry.label_x, -40.0);
        assert_eq!(geometry.label_y, -29.5);
        assert_eq!(geometry.title_x, 0.0);
        assert_eq!(geometry.description_x, 10.0);
        assert_eq!(geometry.description_y, 35.0);
        assert_eq!(geometry.divider_y, -2.5);
    }
}
