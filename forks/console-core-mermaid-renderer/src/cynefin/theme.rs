use std::sync::OnceLock;

use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, InheritedFontStackOutcome, InheritedFontStackPlan,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CynefinTextRole {
    DomainLabel,
    Subtitle,
    Item,
    TransitionLabel,
    Title,
}

/// Text visibility is observed during emission, without a separate layout coverage pass.
#[derive(Debug, Default)]
pub(crate) struct CynefinSurfaceReceipt {
    requested: bool,
    visible_text: bool,
    visible_text_fill: usize,
    overridden_text_fill: usize,
    pub(crate) paint_evidence: FamilyThemeOutcomes,
}

impl CynefinSurfaceReceipt {
    pub(crate) fn record_text(&mut self, role: CynefinTextRole, _emitted_class: &str, text: &str) {
        if !self.requested || text.trim().is_empty() {
            return;
        }

        self.visible_text = true;
        self.visible_text_fill += usize::from(matches!(
            role,
            CynefinTextRole::Subtitle | CynefinTextRole::Item | CynefinTextRole::TransitionLabel
        ));
    }

    pub(crate) fn record_text_fill_override(&mut self) {
        self.overridden_text_fill += 1;
    }

    #[must_use]
    fn has_visible_text(&self) -> bool {
        self.visible_text
    }

    #[must_use]
    fn has_visible_text_fill_surface(&self) -> bool {
        self.visible_text_fill > self.overridden_text_fill
    }
}

/// Final inherited Cynefin font shared by layout measurement, terminal CSS, and evidence.
#[derive(Debug)]
pub(crate) struct CynefinTypographyThemePlan {
    common_css: crate::svg::PreparedCommonCss,
    colors: super::CynefinTheme,
    base_colors: super::CynefinTheme,
    config: PresentationConfig,
    inherited_font_stack: InheritedFontStackPlan,
    evidence: FamilyThemeOutcomes,
    unsupported_routes: Box<[CynefinUnsupportedRoute]>,
    text_fill: Option<DirectStaticPaint>,
    text_fill_routes: Box<[(FamilyThemeMechanismKey, usize)]>,
    config_owns_text_fill: bool,
    terminal_receipt: OnceLock<CynefinSurfaceReceipt>,
}

#[derive(Debug)]
struct CynefinUnsupportedRoute {
    key: FamilyThemeMechanismKey,
    reason: FamilyThemeResidualReason,
}

impl CynefinTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let text_fill = theme.and_then(|theme_2| {
            let style = theme_2.style(ThemeTarget::Text, ThemeVariant::Default, None);
            resolve_direct_static_fill(
                theme_2,
                &style,
                &[ThemeTarget::Text],
                DirectStaticSelectorDomain::Default,
            )
        });
        let text_fill_routes = theme
            .map(|theme_3| {
                theme_3
                    .family_mechanism_routes()
                    .iter()
                    .copied()
                    .filter_map(|route| match route.mechanism() {
                        FamilyThemeMechanism::RuleFacet {
                            rule_index,
                            target: ThemeTarget::Text,
                            facet: crate::diagram_theme::FamilyThemeRuleFacet::Fill(_),
                            selector,
                            ..
                        } if route.disposition() == FamilyThemeDisposition::TypedAdapter
                            && matches!(
                                selector,
                                crate::diagram_theme::FamilyThemeSelectorShape::Static {
                                    variant: None | Some(ThemeVariant::Default)
                                }
                            ) =>
                        {
                            Some((theme_3.family_mechanism_key(route), rule_index))
                        }
                        FamilyThemeMechanism::BaseTypography(_) | FamilyThemeMechanism::RuleFacet { .. } | FamilyThemeMechanism::OrdinalPalette { .. } | FamilyThemeMechanism::EffectBinding { .. } => None,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            })
            .unwrap_or_default();
        let config_owns_text_fill = theme.is_some()
            && (effective_config.config_paint_overrides_typed_default("themeVariables.textColor")
                || effective_config
                    .config_paint_overrides_typed_default("themeVariables.cynefin.textColor"));
        let mut evidence = FamilyThemeOutcomes::default();
        let mut unsupported_routes = cynefin_unsupported_routes(theme, &mut evidence).into_vec();

        if let Some(theme_4) = theme
            && theme_4.ordinal_palette_disposition(ThemeTarget::Text)
                == Some(FamilyThemeDisposition::Unsupported)
        {


            let mut fallbacks = FamilyThemeOutcomes::default();
            reconcile_unsupported_terminal_domains(
                theme_4,
                &mut fallbacks,
                &[UnsupportedTerminalDomain::static_fallbacks_only(
                    ThemeTarget::Text,
                    ThemeVariant::Default,
                )
                .with_source_owned_fill(&[config_owns_text_fill])],
                work_meter,
            )?;
            let palette_key = FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Text,
            };

            if fallbacks.not_applicable_mechanisms().contains(&palette_key) {
                evidence.mark_not_applicable(palette_key.clone());
            }

            for residual in fallbacks
                .residuals()
                .iter()
                .filter(|item| item.key() == &palette_key)
            {
                unsupported_routes.push(CynefinUnsupportedRoute {
                    key: residual.key().clone(),
                    reason: residual.reason(),
                });
            }
        }

        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let mut colors = super::cynefin_theme(effective_config.as_value());
        let base_colors = colors.clone();

        if !config_owns_text_fill && let Some(fill) = &text_fill {
            colors.text_color = fill.css().to_owned();
        }

        let common_css = crate::svg::PreparedCommonCss::new(
            effective_config.as_value(),
            Some(inherited_font_stack.font_family_css()),
        );
        Ok(Self {
            common_css,
            colors,
            base_colors,
            config: effective_config.clone(),
            inherited_font_stack,
            evidence,
            unsupported_routes: unsupported_routes.into_boxed_slice(),
            text_fill,
            text_fill_routes,
            config_owns_text_fill,
            terminal_receipt: OnceLock::new(),
        })
    }

    #[must_use]
    pub(crate) fn font_family_css(&self) -> &str {
        self.common_css.font_family()
    }

    #[must_use]
    pub(crate) fn common_css(&self) -> &crate::svg::PreparedCommonCss {
        &self.common_css
    }

    #[must_use]
    pub(crate) fn colors(&self) -> &super::CynefinTheme {
        &self.colors
    }

    #[must_use]
    pub(crate) fn base_colors(&self) -> &super::CynefinTheme {
        &self.base_colors
    }

    #[must_use]
    pub(crate) fn config(&self) -> &PresentationConfig {
        &self.config
    }

    #[must_use]
    pub(crate) fn begin_terminal_receipt(&self) -> CynefinSurfaceReceipt {
        CynefinSurfaceReceipt {
            requested: self.inherited_font_stack.typography_requested()
                || !self.unsupported_routes.is_empty()
                || !self.text_fill_routes.is_empty(),
            ..CynefinSurfaceReceipt::default()
        }
    }

    #[must_use]
    pub(crate) fn record_terminal(&self, receipt: CynefinSurfaceReceipt) -> bool {
        self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);

        match self.terminal_receipt.get() {
            Some(receipt) => {
                evidence.merge_borrowed(&receipt.paint_evidence);
            },
            ::core::option::Option::None => {},
        }

        for route in &self.unsupported_routes {
            let absent_text_fallback = matches!(
                route.key,
                FamilyThemeMechanismKey::OrdinalPalette {
                    target: ThemeTarget::Text
                }
            ) && self
                .terminal_receipt
                .get()
                .is_some_and(|receipt| !receipt.has_visible_text_fill_surface());

            if absent_text_fallback {
                evidence.mark_not_applicable(route.key.clone());
            } else {
                evidence.mark_residual(route.key.clone(), route.reason);
            }
        }

        let Some(receipt) = self.terminal_receipt.get() else {
            for (key, _) in &self.text_fill_routes {
                if self.config_owns_text_fill {
                    evidence.mark_not_applicable(key.clone());
                } else {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            }

            return;
        };

        self.inherited_font_stack
            .record_unsupported_typography_evidence(evidence, receipt.has_visible_text());
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);

        if !receipt.has_visible_text() {
            evidence.mark_not_applicable(key);
            return;
        }

        if self.inherited_font_stack.typed_font_stack_active() {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
        } else {
            match self.inherited_font_stack.outcome() {
                InheritedFontStackOutcome::ConfigOwned => {
                    evidence.mark_not_applicable(key);
                }
                InheritedFontStackOutcome::Typed | InheritedFontStackOutcome::Unsupported => {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                }
                InheritedFontStackOutcome::Inactive => {}
            }
        }

        for (key_2, rule_index) in &self.text_fill_routes {
            if self.config_owns_text_fill || !receipt.has_visible_text_fill_surface() {
                evidence.mark_not_applicable(key_2.clone());
            } else if self
                .text_fill
                .as_ref()
                .is_some_and(|fill| fill.rule_index() == *rule_index)
            {
                match self.text_fill.as_ref() {
                    Some(fill) => {
                        evidence.mark_applied_with_capabilities(key_2.clone(), [fill.capability()]);
                    },
                    ::core::option::Option::None => {
                        evidence
                            .mark_residual(key_2.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                    },
                }
            } else {
                evidence.mark_not_applicable(key_2.clone());
            }
        }
    }
}

fn cynefin_unsupported_routes(
    theme: Option<&ResolvedDiagramTheme>,
    evidence: &mut FamilyThemeOutcomes,
) -> Box<[CynefinUnsupportedRoute]> {
    let theme_2 = match theme {
        Some(matched_theme) => matched_theme,
        ::core::option::Option::None => return Box::new([]),
    };

    theme_2
        .family_mechanism_routes()
        .iter()
        .copied()
        .filter_map(|route| {
            if matches!(route.mechanism(),
                FamilyThemeMechanism::RuleFacet { target, .. }
                | FamilyThemeMechanism::OrdinalPalette { target }
                | FamilyThemeMechanism::EffectBinding { target, .. }
                if super::owns_target(target))
            {
                return None;
            }

            if route.disposition() != FamilyThemeDisposition::Unsupported {
                return None;
            }

            if let FamilyThemeMechanism::RuleFacet {
                target: ThemeTarget::Text,
                selector:
                    crate::diagram_theme::FamilyThemeSelectorShape::Static {
                        variant: Some(variant),
                    }
                    | crate::diagram_theme::FamilyThemeSelectorShape::Ordinal {
                        variant: Some(variant),
                        ..
                    },
                ..
            } = route.mechanism()
                && variant != ThemeVariant::Default
            {
                evidence.mark_not_applicable(theme_2.family_mechanism_key(route));
                return None;
            }

            let reason = match route.mechanism() {
                FamilyThemeMechanism::RuleFacet { facet, .. } => {
                    unsupported_residual_for_facet(facet)
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Text,
                } => return None,
                FamilyThemeMechanism::OrdinalPalette { .. } => {
                    FamilyThemeResidualReason::UnsupportedOrdinalPalette
                }
                FamilyThemeMechanism::EffectBinding { .. } => {
                    FamilyThemeResidualReason::UnsupportedEffect
                }
                FamilyThemeMechanism::BaseTypography(_) => return None,
            };
            Some(CynefinUnsupportedRoute {
                key: theme_2.family_mechanism_key(route),
                reason,
            })
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_preserves_scoped_roles_and_shared_measurement_values() {
        let config = crate::test_support::presentation_config(serde_json::json!({
            "themeVariables": {
                "fontFamily": "Config Sans", "textColor": "root-text",
                "lineColor": "root-line", "primaryTextColor": "root-label",
                "cynefin": {
                    "textColor": "var(--text)", "boundaryColor": "currentColor",
                    "complexBg": "var(--complex)", "domainFontSize": "23",
                    "itemFontSize": 15
                }
            }
        }));
        let plan = CynefinTypographyThemePlan::resolve(
            None,
            &config,
            &OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            ),
        )
        .unwrap();
        assert_eq!(plan.colors().text_color, "var(--text)");
        assert_eq!(plan.colors().boundary_color, "currentColor");
        assert_eq!(plan.colors().arrow_color, "root-line");
        assert_eq!(plan.colors().label_color, "root-label");
        assert_eq!(plan.colors().complex_bg, "var(--complex)");
        assert_eq!(plan.colors().domain_font_size, 23.0);
        assert_eq!(plan.colors().item_font_size, 15.0);
        assert_eq!(plan.font_family_css(), "Config Sans");
    }
}
