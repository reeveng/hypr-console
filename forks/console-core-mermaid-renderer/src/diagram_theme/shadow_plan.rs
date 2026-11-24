//! Bounded ordered shadow lowering shared by family-owned paint terminals.

use super::{
    EffectColorSpace, EffectComposition, EffectGraph, EffectInput, EffectPrimitive,
    ThemeColorValue, ThemeResourceLimitExceeded, ThemeResourcePolicy,
};

const DROP_SHADOW_PAINT_BOUNDS_SIGMAS: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SvgFilterRegion {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    units: SvgFilterUnits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SvgFilterUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

impl SvgFilterRegion {
    #[must_use]
    pub(crate) fn try_bounded(x: f32, y: f32, width: f32, height: f32) -> Option<Self> {
        let region = Self {
            x,
            y,
            width,
            height,
            units: SvgFilterUnits::ObjectBoundingBox,
        };
        ([region.x, region.y, region.width, region.height]
            .into_iter()
            .all(f32::is_finite)
            && region.width > 0.0
            && region.height > 0.0)
            .then_some(region)
    }

    #[must_use]
    pub(crate) const fn as_array(self) -> [f32; 4] {
        [self.x, self.y, self.width, self.height]
    }

    #[must_use]
    pub(crate) const fn units(self) -> SvgFilterUnits {
        self.units
    }

    #[must_use]
    pub(crate) fn try_bounded_user_space(x: f32, y: f32, width: f32, height: f32) -> Option<Self> {
        let mut region = Self::try_bounded(x, y, width, height)?;
        region.units = SvgFilterUnits::UserSpaceOnUse;
        Some(region)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct EffectOutsets {
    pub(crate) top: f64,
    pub(crate) right: f64,
    pub(crate) bottom: f64,
    pub(crate) left: f64,
}

impl EffectOutsets {
    pub(crate) fn include(&mut self, other: Self) {
        self.top = self.top.max(other.top);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
        self.left = self.left.max(other.left);
    }

    #[must_use]
    pub(crate) fn with_shadow_stages(
        self,
        stages: impl IntoIterator<Item = (EffectInput, [f32; 2], [f32; 2])>,
    ) -> Option<Self> {
        let mut outsets = self;

        for (input, offset, deviation) in stages {
            let input_2 = match input {
                EffectInput::SourceGraphic => self,
                EffectInput::Previous => outsets,
            };
            let extension = Self::for_drop_shadow(0.0, match offset { [_explicit_element, ..] => _explicit_element }, match offset { [_, _explicit_element, ..] => _explicit_element }, deviation)?;
            outsets = Self {
                top: input_2.top + extension.top,
                right: input_2.right + extension.right,
                bottom: input_2.bottom + extension.bottom,
                left: input_2.left + extension.left,
            };
        }

        Some(outsets)
    }

    #[must_use]
    pub(crate) fn with_independent_shadows(
        self,
        stages: impl IntoIterator<Item = ([f32; 2], [f32; 2])>,
    ) -> Option<Self> {
        let mut outsets = self;

        for (offset, deviation) in stages {
            let extension = Self::for_drop_shadow(0.0, match offset { [_explicit_element, ..] => _explicit_element }, match offset { [_, _explicit_element, ..] => _explicit_element }, deviation)?;
            outsets.include(Self {
                top: self.top + extension.top,
                right: self.right + extension.right,
                bottom: self.bottom + extension.bottom,
                left: self.left + extension.left,
            });
        }

        Some(outsets)
    }

    #[must_use]
    fn for_drop_shadow(
        stroke_width: f64,
        offset_x: f32,
        offset_y: f32,
        std_deviation: [f32; 2],
    ) -> Option<Self> {
        if !stroke_width.is_finite()
            || stroke_width < 0.0
            || !offset_x.is_finite()
            || !offset_y.is_finite()
            || std_deviation
                .into_iter()
                .any(|value| !value.is_finite() || value < 0.0)
        {
            return None;
        }

        let half_stroke = stroke_width / 2.0;
        let offset_x_2 = f64::from(offset_x);
        let offset_y_2 = f64::from(offset_y);


        let [blur_x, blur_y] =
            std_deviation.map(|value| f64::from(value) * DROP_SHADOW_PAINT_BOUNDS_SIGMAS);
        let outsets = Self {
            top: half_stroke + blur_y + (-offset_y_2).max(0.0),
            right: half_stroke + blur_x + offset_x_2.max(0.0),
            bottom: half_stroke + blur_y + offset_y_2.max(0.0),
            left: half_stroke + blur_x + (-offset_x_2).max(0.0),
        };
        [outsets.top, outsets.right, outsets.bottom, outsets.left]
            .into_iter()
            .all(f64::is_finite)
            .then_some(outsets)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SvgShadowStage {
    pub(crate) input: EffectInput,
    pub(crate) offset_x: f32,
    pub(crate) offset_y: f32,
    pub(crate) std_deviation: f32,
    pub(crate) color: ThemeColorValue,
}

#[derive(Debug, Clone)]
pub(crate) struct SvgShadowEffect {
    id: String,
    color_space: EffectColorSpace,
    composition: EffectComposition,
    stages: Vec<SvgShadowStage>,
}

impl SvgShadowEffect {
    #[must_use]
    pub(crate) fn from_graph(graph: &EffectGraph) -> Option<Self> {
        let stages = graph
            .primitives()
            .iter()
            .map(|primitive| {
                let EffectPrimitive::DropShadow {
                    input,
                    offset_x,
                    offset_y,
                    blur_radius,
                    spread,
                    color,
                } = primitive
                else {
                    return None;
                };

                if !::core::matches!(::core::primitive::f32::classify(*spread), ::core::num::FpCategory::Zero) {
                    return None;
                }

                Some(SvgShadowStage {
                    input: *input,
                    offset_x: *offset_x,
                    offset_y: *offset_y,
                    std_deviation: *blur_radius,
                    color: color.clone(),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            id: graph.id().to_owned(),
            color_space: graph.color_space(),
            composition: graph.composition(),
            stages,
        })
    }

    #[must_use]
    pub(crate) fn stages(&self) -> &[SvgShadowStage] {
        &self.stages
    }

    #[must_use]
    pub(crate) const fn color_space(&self) -> EffectColorSpace {
        self.color_space
    }

    #[must_use]
    pub(crate) const fn composition(&self) -> EffectComposition {
        self.composition
    }

    #[must_use]
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    fn paint_outsets(&self, source: EffectOutsets) -> Option<EffectOutsets> {
        if self.composition == EffectComposition::Independent {
            return source.with_independent_shadows(
                self.stages
                    .iter()
                    .map(|stage| ([stage.offset_x, stage.offset_y], [stage.std_deviation; 2])),
            );
        }

        source.with_shadow_stages(self.stages.iter().map(|stage| {
            (
                stage.input,
                [stage.offset_x, stage.offset_y],
                [stage.std_deviation; 2],
            )
        }))
    }

    pub(crate) fn materialize_rect(
        &self,
        resources: &ThemeResourcePolicy,
        width: f64,
        height: f64,
        stroke_width: f64,
    ) -> Result<Option<MaterializedShadowEffect>, ThemeResourceLimitExceeded> {
        if !stroke_width.is_finite() || stroke_width < 0.0 {
            return Ok(None);
        }

        let half_stroke = stroke_width / 2.0;
        self.materialize_with_source_outsets(
            resources,
            width,
            height,
            EffectOutsets {
                top: half_stroke,
                right: half_stroke,
                bottom: half_stroke,
                left: half_stroke,
            },
        )
    }

    /// The family supplies the actual shape's stroke envelope, including any miter joins.
    pub(crate) fn materialize_with_source_outsets(
        &self,
        resources: &ThemeResourcePolicy,
        width: f64,
        height: f64,
        source: EffectOutsets,
    ) -> Result<Option<MaterializedShadowEffect>, ThemeResourceLimitExceeded> {
        if !width.is_finite()
            || width <= 0.0
            || !height.is_finite()
            || height <= 0.0
            || ![source.top, source.right, source.bottom, source.left]
                .into_iter()
                .all(|v| v.is_finite() && v >= 0.0)
        {
            return Ok(None);
        }

        let outsets = {
            let optional_outsets = self.paint_outsets(source);

            match optional_outsets {
                Some(matched_outsets) => matched_outsets,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let min_x = -outsets.left / width;
        let min_y = -outsets.top / height;
        let max_x = 1.0 + outsets.right / width;
        let max_y = 1.0 + outsets.bottom / height;

        let x = {
            let optional_x = round_down_f32(min_x);

            match optional_x {
                Some(matched_x) => matched_x,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let y = {
            let optional_y = round_down_f32(min_y);

            match optional_y {
                Some(matched_y) => matched_y,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let max_x_2 = {
            let optional_max_x = round_up_f32(max_x);

            match optional_max_x {
                Some(matched_max_x) => matched_max_x,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let max_y_2 = {
            let optional_max_y = round_up_f32(max_y);

            match optional_max_y {
                Some(matched_max_y) => matched_max_y,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let region_width = {
            let optional_region_width = round_up_f32(f64::from(max_x_2) - f64::from(x));

            match optional_region_width {
                Some(matched_region_width) => matched_region_width,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let region_height = {
            let optional_region_height = round_up_f32(f64::from(max_y_2) - f64::from(y));

            match optional_region_height {
                Some(matched_region_height) => matched_region_height,
                ::core::option::Option::None => return Ok(None),
            }
        };

        if region_width <= 0.0 || region_height <= 0.0 {
            return Ok(None);
        }

        let region = {
            let optional_region = SvgFilterRegion::try_bounded(x, y, region_width, region_height);

            match optional_region {
                Some(matched_region) => matched_region,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let magnitude = region
            .as_array()
            .into_iter()
            .map(f32::abs)
            .fold(0.0_f32, f32::max);
        resources.check_materialized_filter_region_magnitude(magnitude)?;

        Ok(Some(MaterializedShadowEffect { region, outsets }))
    }

    /// Materializes a filter in the path's local user-space coordinates. This is required for
    /// line-like terminals whose object bounding box has a zero width or height.
    pub(crate) fn materialize_user_space(
        &self,
        resources: &ThemeResourcePolicy,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        source: EffectOutsets,
    ) -> Result<Option<MaterializedShadowEffect>, ThemeResourceLimitExceeded> {
        if ![min_x, min_y, max_x, max_y].into_iter().all(f64::is_finite)
            || min_x > max_x
            || min_y > max_y
            || ![source.top, source.right, source.bottom, source.left]
                .into_iter()
                .all(|v| v.is_finite() && v >= 0.0)
        {
            return Ok(None);
        }

        let outsets = {
            let optional_outsets = self.paint_outsets(source);

            match optional_outsets {
                Some(matched_outsets) => matched_outsets,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let x = {
            let optional_x = round_down_f32(min_x - outsets.left);

            match optional_x {
                Some(matched_x) => matched_x,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let y = {
            let optional_y = round_down_f32(min_y - outsets.top);

            match optional_y {
                Some(matched_y) => matched_y,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let max_x_2 = {
            let optional_max_x = round_up_f32(max_x + outsets.right);

            match optional_max_x {
                Some(matched_max_x) => matched_max_x,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let max_y_2 = {
            let optional_max_y = round_up_f32(max_y + outsets.bottom);

            match optional_max_y {
                Some(matched_max_y) => matched_max_y,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let width = {
            let optional_width = round_up_f32(f64::from(max_x_2) - f64::from(x));

            match optional_width {
                Some(matched_width) => matched_width,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let height = {
            let optional_height = round_up_f32(f64::from(max_y_2) - f64::from(y));

            match optional_height {
                Some(matched_height) => matched_height,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let region = {
            let optional_region = SvgFilterRegion::try_bounded_user_space(x, y, width, height);

            match optional_region {
                Some(matched_region) => matched_region,
                ::core::option::Option::None => return Ok(None),
            }
        };

        let magnitude = region
            .as_array()
            .into_iter()
            .map(f32::abs)
            .fold(0.0_f32, f32::max);
        resources.check_materialized_filter_region_magnitude(magnitude)?;
        Ok(Some(MaterializedShadowEffect { region, outsets }))
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MaterializedShadowEffect {
    region: SvgFilterRegion,
    outsets: EffectOutsets,
}

impl MaterializedShadowEffect {
    #[must_use]
    pub(crate) const fn region(self) -> SvgFilterRegion {
        self.region
    }

    #[must_use]
    pub(crate) const fn outsets(self) -> EffectOutsets {
        self.outsets
    }
}

#[must_use]
fn round_down_f32(value: f64) -> Option<f32> {
    let rounded = value as f32;

    if !value.is_finite() || !rounded.is_finite() {
        return None;
    }

    if f64::from(rounded) <= value {
        return Some(rounded);
    }

    let outward = rounded.next_down();
    outward.is_finite().then_some(outward)
}

#[must_use]
fn round_up_f32(value: f64) -> Option<f32> {
    let rounded = value as f32;

    if !value.is_finite() || !rounded.is_finite() {
        return None;
    }

    if f64::from(rounded) >= value {
        return Some(rounded);
    }

    let outward = rounded.next_up();
    outward.is_finite().then_some(outward)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_shadow_bounds_union_without_cascade_or_reset() {
        let source = EffectOutsets {
            top: 2.0,
            right: 2.0,
            bottom: 2.0,
            left: 2.0,
        };
        assert_eq!(
            source.with_independent_shadows([([-12.0, 0.0], [2.0; 2]), ([12.0, 0.0], [2.0; 2]),]),
            Some(EffectOutsets {
                top: 10.0,
                right: 22.0,
                bottom: 10.0,
                left: 22.0
            })
        );
    }

    fn glow(blur_radius: f32) -> SvgShadowEffect {
        SvgShadowEffect::from_graph(
            &EffectGraph::new(
                "glow",
                [EffectPrimitive::DropShadow {
                    input: EffectInput::SourceGraphic,
                    offset_x: 3.0,
                    offset_y: -2.0,
                    blur_radius,
                    spread: 0.0,
                    color: ThemeColorValue::parse("#00f2ff").unwrap(),
                }],
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn user_space_filter_bounds_enclose_horizontal_vertical_and_point_sources() {
        let effect = glow(6.0);
        for [min_x, min_y, max_x, max_y] in [
            [10.125, 20.125, 110.625, 20.125],
            [20.125, 10.125, 20.125, 110.625],
            [20.125, 20.125, 20.125, 20.125],
        ] {
            let materialized = effect
                .materialize_user_space(
                    &ThemeResourcePolicy::default(),
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                    EffectOutsets {
                        top: 2.0,
                        right: 2.0,
                        bottom: 2.0,
                        left: 2.0,
                    },
                )
                .unwrap()
                .unwrap();
            assert_eq!(
                materialized.region().units(),
                SvgFilterUnits::UserSpaceOnUse
            );
            let [x, y, width, height] = materialized.region().as_array().map(f64::from);
            assert!(width > 0.0 && height > 0.0);
            assert!(x <= min_x - 26.0 && y <= min_y - 28.0);
            assert!(x + width >= max_x + 29.0 && y + height >= max_y + 26.0);
        }
    }

    #[test]
    fn user_space_filter_rejects_invalid_geometry_without_fabricating_an_extent() {
        let effect = glow(6.0);
        for bounds in [
            [f64::NAN, 0.0, 10.0, 10.0],
            [0.0, 0.0, f64::INFINITY, 10.0],
            [10.0, 0.0, 0.0, 10.0],
            [0.0, 10.0, 10.0, 0.0],
        ] {
            assert!(
                effect
                    .materialize_user_space(
                        &ThemeResourcePolicy::default(),
                        bounds[0],
                        bounds[1],
                        bounds[2],
                        bounds[3],
                        EffectOutsets::default(),
                    )
                    .unwrap()
                    .is_none()
            );
        }
        assert!(
            effect
                .materialize_user_space(
                    &ThemeResourcePolicy::default(),
                    0.0,
                    0.0,
                    10.0,
                    0.0,
                    EffectOutsets {
                        left: -1.0,
                        ..Default::default()
                    },
                )
                .unwrap()
                .is_none()
        );
        let mut zero_effect = glow(0.0);
        zero_effect.stages[0].offset_x = 0.0;
        zero_effect.stages[0].offset_y = 0.0;
        assert!(
            zero_effect
                .materialize_user_space(
                    &ThemeResourcePolicy::default(),
                    1.0,
                    1.0,
                    1.0,
                    1.0,
                    EffectOutsets::default(),
                )
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn source_graphic_resets_accumulated_shadow_outsets() {
        let stages = [
            (EffectInput::SourceGraphic, 50.0, -50.0, 8.0),
            (EffectInput::Previous, 20.0, 10.0, 10.0),
            (EffectInput::SourceGraphic, 3.0, -2.0, 2.0),
            (EffectInput::Previous, -4.0, 5.0, 1.0),
        ];
        let graph = EffectGraph::new(
            "reset",
            stages
                .into_iter()
                .map(
                    |(input, offset_x, offset_y, blur_radius)| EffectPrimitive::DropShadow {
                        input,
                        offset_x,
                        offset_y,
                        blur_radius,
                        spread: 0.0,
                        color: ThemeColorValue::parse("#204060").unwrap(),
                    },
                ),
        )
        .unwrap();
        let effect = SvgShadowEffect::from_graph(&graph).unwrap();
        let result = effect
            .materialize_rect(&ThemeResourcePolicy::default(), 50.0, 30.0, 4.0)
            .unwrap()
            .unwrap();
        assert_eq!(
            result.outsets(),
            EffectOutsets {
                top: 16.0,
                right: 17.0,
                bottom: 19.0,
                left: 18.0
            }
        );
    }
}
