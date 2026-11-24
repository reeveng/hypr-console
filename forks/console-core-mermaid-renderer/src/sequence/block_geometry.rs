use super::SequenceOperationCheckpoints;
use super::constants::{
    SEQUENCE_FRAME_GEOM_PAD_PX, SEQUENCE_FRAME_SIDE_PAD_PX, SEQUENCE_SELF_MESSAGE_FRAME_EXTRA_Y_PX,
};
use crate::Result;
use crate::model::{LayoutEdge, LayoutNode, SequenceBlockLayout};
use merman_model::families::sequence::{SequenceDiagramModel, SequenceMessage};
use console_core_perfect_hash::collections::FixedMap;

pub(crate) fn frame_x_from_actors(
    model: &SequenceDiagramModel,
    nodes_by_id: &FixedMap<&str, &LayoutNode>,
    checkpoints: SequenceOperationCheckpoints<'_>,
) -> Result<Option<(f64, f64)>> {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;

    for (actor_index, actor_id) in model.actor_order.iter().enumerate() {
        checkpoints.checkpoint_loop(actor_index)?;
        let node_id = format!("actor-top-{actor_id}");

        let n = {
            let optional_n = nodes_by_id.get(node_id.as_str()).copied();

            match optional_n {
                Some(matched_n) => matched_n,
                ::core::option::Option::None => return Ok(None),
            }
        };

        min_x = min_x.min(n.x);
        max_x = max_x.max(n.x);
    }

    if !min_x.is_finite() || !max_x.is_finite() {
        return Ok(None);
    }

    checkpoints.checkpoint()?;
    Ok(Some((
        min_x - SEQUENCE_FRAME_SIDE_PAD_PX,
        max_x + SEQUENCE_FRAME_SIDE_PAD_PX,
    )))
}

/// Resolves the terminal horizontal frame used by both viewport admission and SVG emission.
///
/// Layout-owned nested coordinates are authoritative when present. Geometry reconstruction remains
/// the compatibility fallback for older or incomplete layouts, and the historical `critical`
/// option-section expansion is applied after either route resolves.
#[must_use]
pub(crate) fn resolved_block_frame_x(
    geometry: SequenceBlockGeometry<'_>,
    layout: &SequenceBlockLayout,
    actor_nodes_by_id: &FixedMap<&str, &LayoutNode>,
    default_frame: (f64, f64),
    critical_section_count: Option<usize>,
) -> Option<(f64, f64, f64)> {
    geometry.frame_y_range()?;
    let (fallback_left, fallback_right, min_actor_left) = match geometry
        .frame_x(actor_nodes_by_id) {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => (default_frame.0, default_frame.1, f64::INFINITY),
    };
    let (mut frame_left, frame_right) = match layout
        .start_x
        .zip(layout.stop_x) {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => (fallback_left, fallback_right),
    };

    if critical_section_count.is_some_and(|section_count| section_count > 1)
        && min_actor_left.is_finite()
    {

        frame_left = frame_left.min(min_actor_left - 9.0);
    }

    Some((frame_left, frame_right, min_actor_left))
}

#[derive(Debug, Clone, Copy)]
enum SelfOnlyActor<'a> {
    None,
    One(&'a str),
    Mixed,
}

impl<'a> SelfOnlyActor<'a> {
    #[must_use]
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::None, value) | (value, Self::None) => value,
            (Self::One(left), Self::One(right)) if left == right => Self::One(left),
            _ => Self::Mixed,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SequenceBlockGeometry<'a> {
    geom_min_x: f64,
    geom_max_x: f64,
    min_actor_center_x: f64,
    max_actor_center_x: f64,
    min_actor_left_x: f64,
    frame_min_y: f64,
    frame_max_y: f64,
    self_only_actor: SelfOnlyActor<'a>,
}

impl<'a> SequenceBlockGeometry<'a> {
    #[must_use]
    pub(crate) fn empty() -> Self {
        Self {
            geom_min_x: f64::INFINITY,
            geom_max_x: f64::NEG_INFINITY,
            min_actor_center_x: f64::INFINITY,
            max_actor_center_x: f64::NEG_INFINITY,
            min_actor_left_x: f64::INFINITY,
            frame_min_y: f64::INFINITY,
            frame_max_y: f64::NEG_INFINITY,
            self_only_actor: SelfOnlyActor::None,
        }
    }

    #[must_use]
    pub(crate) fn from_message(
        msg: &'a SequenceMessage,
        actor_nodes_by_id: &FixedMap<&str, &LayoutNode>,
        edges_by_id: &FixedMap<&str, &LayoutEdge>,
        nodes_by_id: &FixedMap<&str, &LayoutNode>,
    ) -> Self {
        let mut geometry = Self::empty();

        let note_node_id = format!("note-{}", msg.id);
        let note = nodes_by_id.get(note_node_id.as_str()).copied();

        match note {
            Some(note_2) => {
                geometry.geom_min_x = note_2.x - note_2.width / 2.0 - SEQUENCE_FRAME_GEOM_PAD_PX;
                geometry.geom_max_x = note_2.x + note_2.width / 2.0 + SEQUENCE_FRAME_GEOM_PAD_PX;
            },
            ::core::option::Option::None => {},
        }

        let mut frame_y_range =
            note.map(|note_3| (note_3.y - note_3.height / 2.0, note_3.y + note_3.height / 2.0));

        match (msg.from.as_deref(), msg.to.as_deref()) {
            (Some(from), Some(to)) => {
                geometry.self_only_actor = if from == to {
                    SelfOnlyActor::One(from)
                } else {
                    SelfOnlyActor::Mixed
                };

                for actor_id in [from, to] {
                    let actor = {
                        let optional_actor = actor_nodes_by_id.get(actor_id).copied();

                        match optional_actor {
                            Some(matched_actor) => matched_actor,
                            ::core::option::Option::None => continue,
                        }
                    };

                    geometry.min_actor_center_x = geometry.min_actor_center_x.min(actor.x);
                    geometry.max_actor_center_x = geometry.max_actor_center_x.max(actor.x);
                    geometry.min_actor_left_x =
                        geometry.min_actor_left_x.min(actor.x - actor.width / 2.0);
                }

                let edge_id = format!("msg-{}", msg.id);

                {
                    match edges_by_id.get(edge_id.as_str()).copied() {
                        Some(edge) => {
                            for point in &edge.points {
                                geometry.geom_min_x = geometry.geom_min_x.min(point.x);
                                geometry.geom_max_x = geometry.geom_max_x.max(point.x);
                            }

                            match edge.label.as_ref() {
                                Some(label) => {
                                    geometry.geom_min_x = geometry
                                        .geom_min_x
                                        .min(label.x - label.width / 2.0 - SEQUENCE_FRAME_GEOM_PAD_PX);
                                    geometry.geom_max_x = geometry
                                        .geom_max_x
                                        .max(label.x + label.width / 2.0 + SEQUENCE_FRAME_GEOM_PAD_PX);
                                },
                                ::core::option::Option::None => {},
                            }

                            {
                                match edge.points.first().map(|point| point.y) {
                                    Some(line_y) => {
                                        let frame_extra = if from == to {
                                            SEQUENCE_SELF_MESSAGE_FRAME_EXTRA_Y_PX
                                        } else {
                                            0.0
                                        };
                                        frame_y_range = Some((line_y, line_y + frame_extra));
                                    },
                                    ::core::option::Option::None => {},
                                };
                            }
                        },
                        ::core::option::Option::None => {},
                    };
                }
            },
            (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {},
        }

        match frame_y_range {
            Some((min_y, max_y)) => {
                geometry.frame_min_y = min_y;
                geometry.frame_max_y = max_y;
            },
            ::core::option::Option::None => {},
        }

        geometry
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.geom_min_x = self.geom_min_x.min(other.geom_min_x);
        self.geom_max_x = self.geom_max_x.max(other.geom_max_x);
        self.min_actor_center_x = self.min_actor_center_x.min(other.min_actor_center_x);
        self.max_actor_center_x = self.max_actor_center_x.max(other.max_actor_center_x);
        self.min_actor_left_x = self.min_actor_left_x.min(other.min_actor_left_x);
        self.frame_min_y = self.frame_min_y.min(other.frame_min_y);
        self.frame_max_y = self.frame_max_y.max(other.frame_max_y);
        self.self_only_actor = self.self_only_actor.merge(other.self_only_actor);
    }

    #[must_use]
    pub(crate) fn merged(mut self, other: Self) -> Self {
        self.merge(other);
        self
    }

    #[must_use]
    pub(crate) fn frame_x(
        self,
        actor_nodes_by_id: &FixedMap<&str, &LayoutNode>,
    ) -> Option<(f64, f64, f64)> {
        if !self.min_actor_center_x.is_finite() || !self.max_actor_center_x.is_finite() {
            return None;
        }

        let mut x1 = self.min_actor_center_x - SEQUENCE_FRAME_SIDE_PAD_PX;
        let mut x2 = self.max_actor_center_x + SEQUENCE_FRAME_SIDE_PAD_PX;

        if self.geom_min_x.is_finite() {
            x1 = x1.min(self.geom_min_x);
        }

        if self.geom_max_x.is_finite() {
            x2 = x2.max(self.geom_max_x);
        }

        if let SelfOnlyActor::One(actor_id) = self.self_only_actor
            && let Some(actor) = actor_nodes_by_id.get(actor_id).copied()
        {
            let left = actor.x - actor.width / 2.0;
            let right = actor.x + actor.width / 2.0;
            let minimum_x1 = left - 5.0;
            let minimum_x2 = right + 15.0;

            if (x2 - x1) < (minimum_x2 - minimum_x1) - 1.0 {
                x1 = x1.min(minimum_x1);
                x2 = x2.max(minimum_x2);
            }
        }

        Some((x1, x2, self.min_actor_left_x))
    }

    #[must_use]
    pub(crate) fn frame_y_range(self) -> Option<(f64, f64)> {
        (self.frame_min_y.is_finite() && self.frame_max_y.is_finite())
            .then_some((self.frame_min_y, self.frame_max_y))
    }

    #[cfg(test)]
    pub(super) fn test_y_range(min_y: f64, max_y: f64) -> Self {
        Self {
            frame_min_y: min_y,
            frame_max_y: max_y,
            ..Self::empty()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_frame_uses_layout_coordinates_and_keeps_critical_left_expansion() {
        let geometry = SequenceBlockGeometry {
            min_actor_center_x: 50.0,
            max_actor_center_x: 50.0,
            min_actor_left_x: 20.0,
            frame_min_y: 10.0,
            frame_max_y: 20.0,
            ..SequenceBlockGeometry::empty()
        };
        let layout = SequenceBlockLayout {
            start_y: 10.0,
            stop_y: 20.0,
            start_x: Some(30.0),
            stop_x: Some(90.0),
            section_ys_by_id: FixedMap::default(),
        };
        let actors = FixedMap::default();

        assert_eq!(
            resolved_block_frame_x(geometry, &layout, &actors, (0.0, 100.0), None),
            Some((30.0, 90.0, 20.0))
        );
        assert_eq!(
            resolved_block_frame_x(geometry, &layout, &actors, (0.0, 100.0), Some(2)),
            Some((11.0, 90.0, 20.0))
        );
    }
}
