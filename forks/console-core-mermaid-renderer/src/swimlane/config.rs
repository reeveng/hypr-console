use merman_model::presentation::PresentationConfig;
use serde_json::Value;

use crate::config::{config_bool, value_at};

pub(super) const DEFAULT_LANE_ID: &str = "__swimlane_default__";
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(super) const GROUP_PADDING: f64 = 8.0;
pub(super) const DEFAULT_LANE_PADDING: f64 = 20.0;
pub(super) const TOP_LANE_TITLE_BAND_HEIGHT: f64 = 21.0;
pub(super) const MIN_TOP_LANE_HORIZONTAL_PADDING: f64 = 20.0;
pub(super) const TOP_LANE_MIN_HEADER_MARGIN: f64 = 36.0;
pub(super) const LR_TITLE_BAND_SIZE: f64 = 36.0;
pub(super) const EDGE_LABEL_CLEARANCE: f64 = 16.0;
pub(super) const MIN_ARROWHEAD_SPACING: f64 = 12.0;
pub(super) const ROUTE_TOUCH_TOLERANCE: f64 = 6.0;

pub(super) const ROUTER_NODE_PADDING: f64 = 8.0;
pub(super) const HORIZONTAL_PIPE_MARGIN: f64 = 15.0;
pub(super) const VERTICAL_PIPE_MARGIN: f64 = 15.0;
pub(super) const ROUTING_MARGIN: f64 = 25.0;
pub(super) const ANCHOR_OFFSET: f64 = 20.0;
pub(super) const VERTICAL_SIDE_BIAS: f64 = 3.0;
pub(super) const CROSSING_PENALTY: f64 = 1_000.0;
pub(super) const BEND_PENALTY: f64 = 50.0;
pub(super) const MIN_PORT_SPACING: f64 = 8.0;
pub(super) const MAX_PORT_SPACING: f64 = 20.0;
pub(super) const DIRECTION_SIGNIFICANCE: f64 = 10.0;
pub(super) const OPPOSITE_MOVE_THRESHOLD: f64 = 5.0;
pub(super) const WRONG_VERTICAL_DIRECTION_FACTOR: f64 = 100.0;
pub(super) const WRONG_HORIZONTAL_DIRECTION_FACTOR: f64 = 50.0;
pub(super) const EPSILON: f64 = 1.0e-6;

#[derive(Debug, Clone, Copy)]
pub(super) struct SwimlaneConfig {
    pub node_gap: f64,
    pub layer_gap: f64,
    pub ignore_cross_lane_edges: bool,
    pub optimize_ranks_by_crossings: bool,
    pub automatic_lane_ordering: bool,
}

#[must_use]
fn number_at(root: &Value, path: &[&str]) -> Option<f64> {
    let value = value_at(root, path)?;
    value.as_f64().or_else(|| {
        value
            .as_str()
            .and_then(|text| match text.trim().parse::<f64>() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_invalid_float) => ::core::option::Option::None })
    })
}

impl SwimlaneConfig {
    #[must_use]
    pub fn from_config(config: &PresentationConfig) -> Self {
        let root = config.as_value();
        Self {


            node_gap: match number_at(root, &["flowchart", "nodeSpacing"]) {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => 40.0,
            },
            layer_gap: match number_at(root, &["flowchart", "rankSpacing"]) {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => 100.0,
            },
            ignore_cross_lane_edges: match config_bool(root, &["swimlane", "ignoreCrossLaneEdges"]) {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => true,
            },
            optimize_ranks_by_crossings: match config_bool(
                root,
                &["swimlane", "optimizeRanksByCrossings"],
            ) {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => true,
            },
            automatic_lane_ordering: match config_bool(root, &["swimlane", "automaticLaneOrdering"]) {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => false,
            },
        }
    }
}
