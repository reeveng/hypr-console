use super::config::{
    MIN_TOP_LANE_HORIZONTAL_PADDING, TOP_LANE_MIN_HEADER_MARGIN, TOP_LANE_TITLE_BAND_HEIGHT,
};
use super::geometry::Rect;
use super::working::WorkingLayout;
use crate::model::SwimlaneTitleRect;
use std::collections::{HashMap, HashSet};

#[must_use]
fn group_depth(layout: &WorkingLayout, id: &str) -> usize {
    let mut depth = 0;
    let mut seen = HashSet::from([id]);
    let mut current = layout
        .nodes
        .get(id)
        .and_then(|node| node.parent_id.as_deref());

    while let Some(parent) = current {
        if !seen.insert(parent) {
            break;
        }

        let node = {
            let optional_node = layout.nodes.get(parent);

            match optional_node {
                Some(matched_node) => matched_node,
                ::core::option::Option::None => break,
            }
        };

        if !node.is_group() {
            break;
        }

        depth += 1;
        current = node.parent_id.as_deref();
    }

    depth
}

#[must_use]
fn child_bounds(layout: &WorkingLayout, group_id: &str, include_label_width: bool) -> Option<Rect> {
    let mut bounds: Option<Rect> = None;
    let mut label_top = f64::INFINITY;
    let mut label_bottom = f64::NEG_INFINITY;

    for child in layout.children_of(group_id) {
        let rect = Rect::from_center(child.x, child.y, child.width, child.height);

        if !include_label_width && child.kind == super::working::WorkingNodeKind::EdgeLabel {
            label_top = label_top.min(rect.top);
            label_bottom = label_bottom.max(rect.bottom);
            continue;
        }

        match &mut bounds {
            Some(current) => {
                current.union(rect);
            },
            ::core::option::Option::None => {
                bounds = Some(rect);
            },
        }
    }

    match &mut bounds {
        Some(bounds_2) => {
            bounds_2.top = bounds_2.top.min(label_top);
            bounds_2.bottom = bounds_2.bottom.max(label_bottom);
        },
        ::core::option::Option::None => {},
    }

    bounds
}

fn assign_title_rect(layout: &mut WorkingLayout, id: &str) {
    let lane = {
        let optional_lane = layout.nodes.get_mut(id);

        match optional_lane {
            Some(matched_lane) => matched_lane,
            ::core::option::Option::None => return,
        }
    };

    let Some(content_top) = lane.content_top else {
        lane.title_rect = None;
        return;
    };

    if lane.width <= 0.0 || lane.height <= 0.0 {
        lane.title_rect = None;
        return;
    }

    let top = lane.y - lane.height / 2.0;
    let header_bottom = content_top.min(lane.y + lane.height / 2.0);
    let title_height = TOP_LANE_TITLE_BAND_HEIGHT.min((header_bottom - top).max(0.0));
    let bottom = top + title_height;
    lane.title_rect = (bottom > top).then_some(SwimlaneTitleRect {
        left: lane.x - lane.width / 2.0,
        right: lane.x + lane.width / 2.0,
        top,
        bottom,
    });
}

pub(super) fn assign_canonical_group_bounds(layout: &mut WorkingLayout) {
    let mut group_ids: Vec<String> = layout
        .nodes
        .values()
        .filter(|node| node.is_group())
        .map(|node| node.id.clone())
        .collect();
    group_ids.sort_by_key(|id| std::cmp::Reverse(group_depth(layout, id)));

    let mut source_bounds = HashMap::new();

    for id in &group_ids {
        let bounds = {
            let optional_bounds = child_bounds(layout, id, false);

            match optional_bounds {
                Some(matched_bounds) => matched_bounds,
                ::core::option::Option::None => continue,
            }
        };

        let group = {
            let optional_group = layout.nodes.get_mut(id);

            match optional_group {
                Some(matched_group) => matched_group,
                ::core::option::Option::None => continue,
            }
        };

        let horizontal_padding = if group.parent_id.is_some() {
            group.padding
        } else {
            2.0 * group.padding.max(MIN_TOP_LANE_HORIZONTAL_PADDING)
        };
        let center = bounds.center();
        group.x = center.x;
        group.y = center.y;
        group.width = bounds.width() + horizontal_padding;
        group.height = bounds.height() + group.padding;
        source_bounds.insert(id.clone(), bounds);
    }

    let lane_model = super::lanes::LaneModel::new(&layout.nodes);
    let top_lane_ids: Vec<String> = layout
        .nodes
        .values()
        .filter(|node| lane_model.is_lane(&node.id))
        .map(|node| node.id.clone())
        .collect();
    let mut global_min_y = f64::INFINITY;
    let mut global_max_y = f64::NEG_INFINITY;
    let mut max_padding: f64 = 0.0;

    for id in &top_lane_ids {
        let lane = {
            let optional_lane = layout.nodes.get(id);

            match optional_lane {
                Some(matched_lane) => matched_lane,
                ::core::option::Option::None => continue,
            }
        };

        max_padding = max_padding.max(lane.padding);

        {
            match source_bounds.get(id) {
                Some(bounds) => {
                    global_min_y = global_min_y.min(bounds.top);
                    global_max_y = global_max_y.max(bounds.bottom);
                },
                ::core::option::Option::None => {},
            };
        }
    }

    if !global_min_y.is_finite() || !global_max_y.is_finite() {
        return;
    }

    let vertical_margin = max_padding.max(TOP_LANE_MIN_HEADER_MARGIN);
    let lane_height = global_max_y - global_min_y + 2.0 * vertical_margin;
    let center_y = (global_min_y + global_max_y) / 2.0;

    for id in &top_lane_ids {
        {
            match layout.nodes.get_mut(id) {
                Some(lane) => {
                    lane.y = center_y;
                    lane.height = lane_height;
                    lane.content_top = Some(global_min_y);
                },
                ::core::option::Option::None => {},
            };
        }
    }

    let mut sorted: Vec<String> = top_lane_ids
        .iter()
        .filter(|id| source_bounds.contains_key(*id))
        .cloned()
        .collect();
    sorted.sort_by(|left, right| {
        match layout.nodes[left]
            .x
            .partial_cmp(&layout.nodes[right].x) {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => std::cmp::Ordering::Equal,
        }
    });
    let centers: Vec<f64> = sorted
        .iter()
        .map(|id| {
            let bounds = source_bounds[id];
            (bounds.left + bounds.right) / 2.0
        })
        .collect();
    let base_widths: Vec<f64> = sorted
        .iter()
        .map(|id| {
            let bounds = source_bounds[id];
            let padding = layout.nodes[id]
                .padding
                .max(MIN_TOP_LANE_HORIZONTAL_PADDING);
            bounds.width() + 2.0 * padding
        })
        .collect();
    let mut widths = base_widths.clone();

    if sorted.len() > 1 {
        let distances: Vec<f64> = centers.windows(2).map(|pair| pair[1] - pair[0]).collect();
        let mut offsets = vec![0.0; sorted.len()];

        for (index, distances_element) in <[_]>::iter(&distances).enumerate() {
            offsets[index + 1] = 2.0 * *distances_element - offsets[index];
        }

        let mut lower: f64 = 0.0;
        let mut upper = f64::INFINITY;

        for index in 0..sorted.len() {
            if index.wrapping_rem(2) == 0 {
                lower = lower.max(base_widths[index] - offsets[index]);
            } else {
                upper = upper.min(offsets[index] - base_widths[index]);
            }
        }

        let seed = if lower <= upper {
            (lower + upper) / 2.0
        } else {
            lower
        };

        for index in 0..sorted.len() {
            let solved = offsets[index] + if index.wrapping_rem(2) == 0 { seed } else { -seed };
            widths[index] = widths[index].max(solved);
        }
    }

    for (id, width) in sorted.iter().zip(widths) {
        match layout.nodes.get_mut(id) {
            Some(lane) => {
                lane.width = width;
            },
            ::core::option::Option::None => {},
        }

        assign_title_rect(layout, id);
    }
}

pub(super) fn recompute_nested_group_bounds(layout: &mut WorkingLayout) {
    let mut group_ids: Vec<String> = layout
        .nodes
        .values()
        .filter(|node| node.is_group() && node.parent_id.is_some())
        .map(|node| node.id.clone())
        .collect();
    group_ids.sort_by_key(|id| std::cmp::Reverse(group_depth(layout, id)));

    for id in group_ids {
        let bounds = {
            let optional_bounds = child_bounds(layout, &id, true);

            match optional_bounds {
                Some(matched_bounds) => matched_bounds,
                ::core::option::Option::None => continue,
            }
        };

        {
            match layout.nodes.get_mut(&id) {
                Some(group) => {
                    let center = bounds.center();
                    group.x = center.x;
                    group.y = center.y;
                    group.width = bounds.width() + group.padding;
                    group.height = bounds.height() + group.padding;
                },
                ::core::option::Option::None => {},
            };
        }
    }
}
