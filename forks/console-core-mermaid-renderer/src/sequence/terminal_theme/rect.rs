use super::*;
use crate::diagram_theme::SvgShadowEffect;

#[derive(Debug, Default)]
pub(crate) struct SequencePreparedStaticRectTheme {
    pub(crate) stroke_width: Option<f32>,
    pub(crate) typed_fill: Option<String>,
    pub(crate) typed_stroke: Option<String>,
    pub(crate) fill_overridden: bool,
    pub(crate) stroke_overridden: bool,
    pub(crate) radius: Option<f32>,
    pub(crate) effect: Option<SvgShadowEffect>,
    effect_requested: bool,
    effect_binding_used: bool,
    effect_cleared: bool,
    effect_unhandled: bool,
    selected_style: Option<ResolvedThemeStyle>,
}

pub(crate) struct SequenceActivationPaint<'a> {
    pub(crate) typed_fill: Option<&'a str>,
    pub(crate) typed_stroke: Option<&'a str>,
    pub(crate) fill_overridden: bool,
    pub(crate) stroke_overridden: bool,
}

impl SequencePreparedStaticRectTheme {
    #[must_use]
    pub(crate) fn activation_paint(
        &self,
        compat: &super::super::SequenceCompatBinding,
    ) -> SequenceActivationPaint<'_> {
        let fill_overridden = self.fill_overridden || compat.palette_fill_overridden;
        let stroke_overridden = self.stroke_overridden || compat.palette_stroke_overridden;
        SequenceActivationPaint {
            typed_fill: self.typed_fill.as_deref().filter(|_| !fill_overridden),
            typed_stroke: self.typed_stroke.as_deref().filter(|_| !stroke_overridden),
            fill_overridden,
            stroke_overridden,
        }
    }

    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        target: crate::diagram_theme::ThemeTarget,
        has_surfaces: bool,
        fill_overridden: bool,
        stroke_overridden: bool,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<SequencePreparedStaticRectTheme> {
        use crate::diagram_theme::{
            FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemePaintKind,
            FamilyThemeRuleFacet, FamilyThemeSelectorShape, ThemeVariant,
        };

        debug_assert!(matches!(
            target,
            crate::diagram_theme::ThemeTarget::Note | crate::diagram_theme::ThemeTarget::Activation
        ));

        if !has_surfaces {
            return Ok(Self {
                fill_overridden,
                stroke_overridden,
                ..Self::default()
            });
        }

        let Some(theme_2) = theme else {
            return Ok(Self {
                fill_overridden,
                stroke_overridden,
                ..Self::default()
            });
        };

        let mut has_rule_routes = false;
        let mut has_typed_fill = false;
        let mut has_typed_stroke = false;

        for route in theme_2.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                target: route_target,
                selector:
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                facet,
                ..
            } = route.mechanism()
            else {
                continue;
            };

            if route_target != target {
                continue;
            }

            has_rule_routes = true;

            if route.disposition() != FamilyThemeDisposition::TypedAdapter {
                continue;
            }

            match facet {
                FamilyThemeRuleFacet::Fill(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                ) => has_typed_fill = true,
                FamilyThemeRuleFacet::Stroke(
                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                ) => has_typed_stroke = true,
                _ => {}
            }
        }

        let style = if has_rule_routes {
            let style =
                theme_2.style_with_work_meter(target, ThemeVariant::Default, None, work_meter)?;
            Some(style)
        } else {
            None
        };
        let typed_fill = (!fill_overridden && has_typed_fill)
            .then(|| style.as_ref().and_then(|style_2| css_paint(style_2.fill())))
            .flatten();
        let typed_stroke = (!stroke_overridden && has_typed_stroke)
            .then(|| style.as_ref().and_then(|style_3| css_paint(style_3.stroke())))
            .flatten();
        let stroke_width = style.as_ref().and_then(|style_4| style_4.stroke_width());
        let radius = style.as_ref().and_then(|style_5| style_5.radius());
        let default_effect_resolution = Default::default();
        let resolution = style
            .as_ref()
            .map(|style_6| style_6.effect_resolution())
            .unwrap_or(&default_effect_resolution);
        let SequencePreparedEffect {
            effect,
            effect_requested,
            effect_binding_used,
            effect_cleared,
            effect_unhandled,
        } = prepare_sequence_effect(theme_2, target, resolution);
        Ok(SequencePreparedStaticRectTheme {
            stroke_width,
            radius,
            effect,
            typed_fill,
            typed_stroke,
            fill_overridden,
            stroke_overridden,
            effect_requested,
            effect_binding_used,
            effect_cleared,
            effect_unhandled,
            selected_style: style,
        })
    }

    #[must_use]
    pub(crate) fn fresh_receipt(&self) -> crate::sequence::SequenceStaticRectThemeReceipt {
        let mut receipt = crate::sequence::SequenceStaticRectThemeReceipt::default();

        if let Some(style) = &self.selected_style {
            receipt.record_static_style(style);
        }

        receipt.effect_requested = self.effect_requested;
        receipt.effect_binding_used = self.effect_binding_used;
        receipt.effect_cleared = self.effect_cleared;
        receipt.effect_unhandled = self.effect_unhandled;
        receipt
    }
}
