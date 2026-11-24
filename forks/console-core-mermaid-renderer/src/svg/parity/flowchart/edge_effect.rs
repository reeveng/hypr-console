//! Edge-owned shadow geometry shared by viewport preparation and edge emission.

use console_core_perfect_hash::collections::FixedMap;

use super::*;
use crate::diagram_theme::{EffectOutsets, MaterializedShadowEffect, ThemeResourcePolicy};

#[derive(Debug, Default)]
pub(super) struct FlowchartEdgeEffects {
    edges: FixedMap<crate::flowchart::FlowchartEdgeKey, MaterializedShadowEffect>,
}

impl FlowchartEdgeEffects {
    pub(super) fn prepare(
        ctx: &FlowchartRenderCtx<'_>,
        render_edges: &[super::render_input::FlowchartRenderEdge<'_>],
        edge_cache: &FixedMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
        resources: &ThemeResourcePolicy,
        marker_plan: &super::defs::FlowchartMarkerEmissionPlan,
    ) -> crate::Result<Self> {
        let mut plan = Self::default();

        if !ctx.effect_eligibility.edge {
            return Ok(plan);
        }




        let look = ctx.compatibility.look;

        if look.as_str() != "classic" && !super::supports_neo_effects(ctx) {
            return Ok(plan);
        }

        let effect = {
            let optional_effect = ctx.edge_theme.effect();

            match optional_effect {
                Some(matched_effect) => matched_effect,
                ::core::option::Option::None => return Ok(plan),
            }
        };

        let ancestor_classes: &[&str] =
            if ctx.uses_elk_adapter_dom || ctx.swimlane_direction.is_some() {
                &["root", "edges", "edgePaths"]
            } else {
                &["root", "edgePaths"]
            };

        if !super::style::edge_shadow_structural_styles_are_bounded(
            ctx.class_defs,
            ancestor_classes,
        ) {
            return Ok(plan);
        }

        for render_edge in render_edges {
            let edge = render_edge.as_ref();

            let cache_entry = {
                let optional_cache_entry = edge_cache.get(&edge.key);

                match optional_cache_entry {
                    Some(matched_cache_entry) => matched_cache_entry,
                    ::core::option::Option::None => continue,
                }
            };

            ctx.work_meter.charge(1)?;
            let source_style = ctx.edge_style_plan.edge_for(edge.key)?;

            if !source_style.source_filter_status().is_absent()
                || !source_style.edge_shadow_source_is_bounded()
            {
                continue;
            }

            let Some(paint_outset) = { let checked_value = ctx
                .edge_style_plan
                .stroke_width_for(edge.key)?; checked_value }
                .paint_outset()
            else {
                continue;
            };

            let marker_outset = {
                let optional_marker_outset = marker_plan.edge_paint_outset_for(edge.key);

                match optional_marker_outset {
                    Some(matched_marker_outset) => matched_marker_outset,
                    ::core::option::Option::None => continue,
                }
            };


            let paint_outset_2 = (4.0 * paint_outset).max(marker_outset);
            let source = EffectOutsets {
                top: paint_outset_2,
                right: paint_outset_2,
                bottom: paint_outset_2,
                left: paint_outset_2,
            };

            let pb = match cache_entry.geom.pb {
                Some(matched_pb) => matched_pb,
                ::core::option::Option::None => continue,
            };

            let Some(shadow) = effect.materialize_user_space(
                resources, pb.min_x, pb.min_y, pb.max_x, pb.max_y, source,
            )?
            else {
                continue;
            };

            plan.edges.insert(edge.key, shadow);
        }

        Ok(plan)
    }

    #[must_use]
    pub(super) fn edge(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> Option<&MaterializedShadowEffect> {
        self.edges.get(&key)
    }

    #[must_use]
    pub(super) fn expected_applications(&self) -> usize {
        self.edges.len()
    }

    pub(super) fn include_bounds(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
        origin_x: f64,
        abs_top_transform: f64,
        include: &mut impl FnMut(f64, f64, f64, f64),
    ) {
        let shadow = {
            let optional_shadow = self.edges.get(&key);

            match optional_shadow {
                Some(matched_shadow) => matched_shadow,
                ::core::option::Option::None => return,
            }
        };

        let region = shadow.region().as_array();
        include(
            f64::from(match region { [_explicit_element, ..] => _explicit_element }) + origin_x,
            f64::from(match region { [_, _explicit_element, ..] => _explicit_element }) + abs_top_transform,
            f64::from(match region { [_explicit_element, ..] => _explicit_element }) + f64::from(match region { [_, _, _explicit_element, ..] => _explicit_element }) + origin_x,
            f64::from(match region { [_, _explicit_element, ..] => _explicit_element }) + f64::from(match region { [_, _, _, _explicit_element, ..] => _explicit_element }) + abs_top_transform,
        );
    }
}
