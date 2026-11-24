use super::*;
use crate::diagram_theme::{ResolvedStyleProperty, SvgShadowEffect};

#[derive(Debug, Default)]
pub(crate) struct SequencePreparedMessageTheme {
    pub(crate) typed_stroke: Option<String>,
    pub(crate) typed_stroke_width: Option<f32>,
    pub(crate) typed_stroke_width_won: bool,
    pub(crate) selected_property: Option<ResolvedStyleProperty>,
    pub(crate) stroke_overridden: bool,
    pub(crate) effect: Option<SvgShadowEffect>,
    effect_requested: bool,
    effect_binding_used: bool,
    effect_cleared: bool,
    effect_unhandled: bool,
    selected_style: Option<ResolvedThemeStyle>,
}

impl SequencePreparedMessageTheme {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        has_lines: bool,
        stroke_overridden: bool,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<SequencePreparedMessageTheme> {
        use crate::diagram_theme::{
            FamilyThemeMechanism, FamilyThemeSelectorShape, ResolvedStyleProperty, ThemeTarget,
            ThemeVariant,
        };

        if !has_lines {
            return Ok(Self {
                stroke_overridden,
                ..Self::default()
            });
        }

        let Some(theme_2) = theme else {
            return Ok(Self {
                stroke_overridden,
                ..Self::default()
            });
        };

        let has_rule_routes = theme_2.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Message,
                    selector: FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                    ..
                } | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Message,
                    ..
                }
            )
        });

        if !has_rule_routes {
            return Ok(Self {
                stroke_overridden,
                ..Self::default()
            });
        }

        let style = theme_2.style_with_work_meter(
            ThemeTarget::Message,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;




        let selected_property = if style.stroke_resolution().winner().is_some() {
            Some(ResolvedStyleProperty::Stroke)
        } else if style.fill_resolution().winner().is_some() {
            Some(ResolvedStyleProperty::Fill)
        } else {
            None
        };
        let typed_stroke = (!stroke_overridden)
            .then(|| match selected_property {
                Some(ResolvedStyleProperty::Stroke) => typed_static_sequence_stroke(theme_2, &style),
                Some(ResolvedStyleProperty::Fill) => typed_static_sequence_fill(theme_2, &style),
                _ => None,
            })
            .flatten();
        let SequencePreparedEffect {
            effect,
            effect_requested,
            effect_binding_used,
            effect_cleared,
            effect_unhandled,
        } = prepare_sequence_effect(theme_2, ThemeTarget::Message, style.effect_resolution());
        Ok(SequencePreparedMessageTheme {
            effect,
            typed_stroke,
            typed_stroke_width: style.stroke_width(),
            typed_stroke_width_won: style.stroke_width_resolution().winner().is_some(),
            selected_property,
            stroke_overridden,
            effect_requested,
            effect_binding_used,
            effect_cleared,
            effect_unhandled,
            selected_style: Some(style),
        })
    }

    #[must_use]
    pub(crate) fn fresh_receipt(&self) -> crate::sequence::SequenceMessageThemeReceipt {
        let mut receipt = crate::sequence::SequenceMessageThemeReceipt::default();

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

#[derive(Debug, Default)]
pub(crate) struct SequencePreparedLifelineTheme {
    pub(crate) typed_stroke: Option<String>,
    pub(crate) typed_stroke_width: Option<f32>,
    pub(crate) typed_stroke_width_won: bool,
    pub(crate) selected_property: Option<ResolvedStyleProperty>,
    pub(crate) stroke_overridden: bool,
    pub(crate) effect: Option<SvgShadowEffect>,
    effect_requested: bool,
    effect_binding_used: bool,
    effect_cleared: bool,
    effect_unhandled: bool,
    selected_style: Option<ResolvedThemeStyle>,
}

impl SequencePreparedLifelineTheme {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        has_lifelines: bool,
        paint_overridden: bool,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<SequencePreparedLifelineTheme> {
        use crate::diagram_theme::{
            FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemePaintKind,
            FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedStyleProperty, ThemeTarget,
            ThemeVariant,
        };

        if !has_lifelines {
            return Ok(Self {
                stroke_overridden: paint_overridden,
                ..Self::default()
            });
        }

        let Some(theme_2) = theme else {
            return Ok(Self {
                stroke_overridden: paint_overridden,
                ..Self::default()
            });
        };

        let mut has_routes = false;
        let mut has_typed_fill = false;
        let mut has_typed_stroke = false;
        let mut has_typed_stroke_width = false;

        for route in theme_2.family_mechanism_routes().iter().copied() {
            if matches!(
                route.mechanism(),
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Lifeline,
                    ..
                }
            ) {
                has_routes = true;
            }

            let FamilyThemeMechanism::RuleFacet {
                target: ThemeTarget::Lifeline,
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

            has_routes = true;

            match facet {
                FamilyThemeRuleFacet::Fill(kind) => {
                    has_typed_fill |= route.disposition() == FamilyThemeDisposition::TypedAdapter
                        && matches!(
                            kind,
                            FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                        );
                }
                FamilyThemeRuleFacet::Stroke(kind) => {
                    has_typed_stroke |= route.disposition() == FamilyThemeDisposition::TypedAdapter
                        && matches!(
                            kind,
                            FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                        );
                }
                FamilyThemeRuleFacet::StrokeWidth => {
                    has_typed_stroke_width =
                        route.disposition() == FamilyThemeDisposition::TypedAdapter;
                }
                FamilyThemeRuleFacet::StrokeDasharray | FamilyThemeRuleFacet::StrokeLinecap | FamilyThemeRuleFacet::StrokeLinejoin | FamilyThemeRuleFacet::Opacity | FamilyThemeRuleFacet::FillOpacity | FamilyThemeRuleFacet::StrokeOpacity | FamilyThemeRuleFacet::Radius | FamilyThemeRuleFacet::Padding | FamilyThemeRuleFacet::Typography(_) | FamilyThemeRuleFacet::Effect => {}
            }
        }

        if !has_routes {
            return Ok(Self {
                stroke_overridden: paint_overridden,
                ..Self::default()
            });
        }

        let style = theme_2.style_with_work_meter(
            ThemeTarget::Lifeline,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let selected_property = if style.stroke_resolution().winner().is_some() {
            Some(ResolvedStyleProperty::Stroke)
        } else if style.fill_resolution().winner().is_some() {
            Some(ResolvedStyleProperty::Fill)
        } else {
            None
        };
        let typed_stroke = (!paint_overridden)
            .then(|| match selected_property {
                Some(ResolvedStyleProperty::Stroke) if has_typed_stroke => {
                    css_paint(style.stroke())
                }
                Some(ResolvedStyleProperty::Fill) if has_typed_fill => css_paint(style.fill()),
                ::core::option::Option::None | ::core::option::Option::Some(_) => None,
            })
            .flatten();
        let typed_stroke_width = has_typed_stroke_width
            .then(|| style.stroke_width())
            .flatten();



        let typed_stroke_width_won =
            has_typed_stroke_width && style.stroke_width_resolution().winner().is_some();
        let SequencePreparedEffect {
            effect,
            effect_requested,
            effect_binding_used,
            effect_cleared,
            effect_unhandled,
        } = prepare_sequence_effect(theme_2, ThemeTarget::Lifeline, style.effect_resolution());
        Ok(SequencePreparedLifelineTheme {
            effect,
            typed_stroke,
            typed_stroke_width,
            typed_stroke_width_won,
            selected_property,
            stroke_overridden: paint_overridden,
            effect_requested,
            effect_binding_used,
            effect_cleared,
            effect_unhandled,
            selected_style: Some(style),
        })
    }

    #[must_use]
    pub(crate) fn fresh_receipt(&self) -> crate::sequence::SequenceLifelineThemeReceipt {
        let mut receipt = crate::sequence::SequenceLifelineThemeReceipt::default();

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, Specified, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;

    #[test]
    fn prepared_lifeline_clear_keeps_a_width_winner_without_numeric_override() {
        let mut patch = ThemeStylePatch::default();
        patch.stroke.width = Specified::Clear;
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Lifeline, patch)),
            ))
            .unwrap();
        let resolved = theme.resolve(crate::DiagramFamilyId::SEQUENCE);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let prepared =
            SequencePreparedLifelineTheme::resolve(Some(&resolved), true, false, &meter).unwrap();
        assert!(prepared.typed_stroke_width_won);
        assert_eq!(prepared.typed_stroke_width, None);
    }
}
