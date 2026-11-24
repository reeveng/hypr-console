use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::observation::{
    SequenceRuleObservation, observe_control_rule, observe_lifeline_rule, observe_message_rule,
    observe_sequence_number_rule, observe_static_rect_rule, observe_typography_rule,
};
use super::receipts::{
    SequenceActorThemeReceipt, SequenceControlThemeEmission, SequenceLifelineThemeEmission,
    SequenceMessageThemeEmission, SequenceNumberLabelThemeEmission,
    SequenceStaticRectThemeEmission, SequenceThemeEvidenceState, SequenceTypographyThemeReceipt,
};
use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability,
    ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeOutcomes, FamilyThemeResidualReason, SingleTitleFillPlan,
    TerminalVariantDomain, UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    unsupported_residual_for_facet as unsupported_reason_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Records actual Sequence writer emission so program routes alone cannot manufacture evidence.
#[derive(Debug, Default)]
pub(crate) struct SequenceThemeEvidenceRecorder {
    state: Option<RefCell<SequenceThemeEvidenceState>>,
    title_fill: Option<SingleTitleFillPlan>,
    title_fallback: Option<Box<str>>,
    expected_title: Option<Box<str>>,
    title_fill_owned: bool,
    title_terminal: OnceLock<()>,
}

impl SequenceThemeEvidenceRecorder {
    #[must_use]
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            state: enabled.then(|| RefCell::new(SequenceThemeEvidenceState::default())),
            ..Self::default()
        }
    }

    pub(crate) fn with_title(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_model::presentation::PresentationConfig,
        title: Option<&str>,
        work: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut recorder = Self::new(theme.is_some());
        recorder.expected_title = title.map(Into::into);

        let theme_2 = match theme {
            Some(matched_theme) => matched_theme,
            ::core::option::Option::None => return Ok(recorder),
        };

        let source_owned = config.config_paint_overrides_typed_default("themeVariables.textColor");
        let title_fill = SingleTitleFillPlan::resolve(
            theme_2,
            title.is_some_and(|title_2| !title_2.trim().is_empty()),
            source_owned,
            FamilyThemeOutcomes::default(),
            work,
        )?;
        recorder.title_fill_owned = source_owned || title_fill.fill_css().is_some();

        if title.is_some_and(|title_3| !title_3.trim().is_empty()) && !recorder.title_fill_owned {
            let style = theme_2.style_with_work_meter(
                ThemeTarget::Text,
                ThemeVariant::Default,
                None,
                work,
            )?;


            recorder.title_fallback = match style.fill() {
                Some(CanvasPaint::Solid(color)) => Some(color.as_css().into_boxed_str()),
                Some(CanvasPaint::Transparent) => Some(<::std::boxed::Box<::core::primitive::str> as ::core::convert::From<&::core::primitive::str>>::from("transparent")),
                _ => None,
            };
        }

        recorder.title_fill = Some(title_fill);
        Ok(recorder)
    }

    #[must_use]
    pub(crate) fn title_fill_css(&self) -> Option<&str> {
        self.title_fill
            .as_ref()
            .and_then(SingleTitleFillPlan::fill_css)
            .or(self.title_fallback.as_deref())
    }

    #[must_use]
    pub(crate) fn record_title_emission(&self, title: &str, fill: Option<&str>) -> bool {
        Some(title) == self.expected_title.as_deref()
            && fill == self.title_fill_css()
            && self.title_terminal.set(()).is_ok()
            && self
                .title_fill
                .as_ref()
                .is_none_or(SingleTitleFillPlan::record_proven_terminal)
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Terminal recording receives independent emitted actor surfaces"
    )]
    pub(crate) fn record_actor_emission(
        &self,
        actor_count: usize,
        actor_fill_emitted: bool,
        actor_fill_unhandled: bool,
        actor_fill_overridden: bool,
        actor_stroke_emitted: bool,
        actor_stroke_unhandled: bool,
        actor_stroke_overridden: bool,
        actor_style_receipt: SequenceActorThemeReceipt,
    ) {
        let state = match &self.state {
            Some(matched_state) => matched_state,
            ::core::option::Option::None => return,
        };

        let mut state_2 = state.borrow_mut();
        state_2.actor_count = actor_count;
        state_2.actor_fill_emitted = actor_count != 0 && actor_fill_emitted;
        state_2.actor_fill_unhandled = actor_count != 0 && actor_fill_unhandled;
        state_2.actor_fill_overridden = actor_count != 0 && actor_fill_overridden;
        state_2.actor_stroke_emitted = actor_count != 0 && actor_stroke_emitted;
        state_2.actor_stroke_unhandled = actor_count != 0 && actor_stroke_unhandled;
        state_2.actor_stroke_overridden = actor_count != 0 && actor_stroke_overridden;
        state_2.actor_style_receipt = actor_style_receipt;
    }

    pub(crate) fn record_note_emission(&self, emission: SequenceStaticRectThemeEmission) {
        let state = match &self.state {
            Some(matched_state) => matched_state,
            ::core::option::Option::None => return,
        };

        let mut state_2 = state.borrow_mut();
        state_2.note = emission;
    }

    pub(crate) fn record_lifeline_emission(&self, emission: SequenceLifelineThemeEmission) {
        let state = match &self.state {
            Some(matched_state) => matched_state,
            ::core::option::Option::None => return,
        };

        let mut state_2 = state.borrow_mut();
        state_2.lifeline = emission;
    }

    pub(crate) fn record_message_emission(&self, emission: SequenceMessageThemeEmission) {
        let state = match &self.state {
            Some(matched_state) => matched_state,
            ::core::option::Option::None => return,
        };

        let mut state_2 = state.borrow_mut();
        state_2.message = emission;
    }

    pub(crate) fn record_sequence_number_emission(
        &self,
        emission: SequenceNumberLabelThemeEmission,
    ) {
        let state = match &self.state {
            Some(matched_state) => matched_state,
            ::core::option::Option::None => return,
        };

        let mut state_2 = state.borrow_mut();
        state_2.sequence_number = emission;
    }

    pub(crate) fn record_control_emission(
        &self,
        target: ThemeTarget,
        emission: SequenceControlThemeEmission,
    ) {
        let state = match &self.state {
            Some(matched_state) => matched_state,
            ::core::option::Option::None => return,
        };

        let mut state_2 = state.borrow_mut();

        if target == ThemeTarget::Loop {
            state_2.loop_frame = emission;
        } else {
            state_2.keyword_background = emission;
        }
    }

    pub(crate) fn record_activation_emission(&self, emission: SequenceStaticRectThemeEmission) {
        let state = match &self.state {
            Some(matched_state) => matched_state,
            ::core::option::Option::None => return,
        };

        let mut state_2 = state.borrow_mut();
        state_2.activation = emission;
    }

    pub(crate) fn record_typography_emission(&self, receipt: SequenceTypographyThemeReceipt) {
        let state = match &self.state {
            Some(matched_state) => matched_state,
            ::core::option::Option::None => return,
        };

        let mut state = state.borrow_mut();
        debug_assert!(
            state.typography.is_none(),
            "Sequence role typography is sealed once per terminal SVG"
        );
        state.typography = Some(receipt);
    }

    /// Reconcile unsupported text requests only after the final SVG has completed.
    pub(crate) fn record_unsupported_text_emission(
        &self,
        theme: Option<&ResolvedDiagramTheme>,
        title_present: bool,
        work: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let theme_2 = match theme {
            Some(matched_theme) => matched_theme,
            ::core::option::Option::None => return Ok(()),
        };

        if !theme_2.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Text | ThemeTarget::Title,
                    ..
                } | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Text | ThemeTarget::Title
                } | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Text | ThemeTarget::Title,
                    ..
                }
            )
        }) {
            return Ok(());
        }

        let state = match &self.state {
            Some(matched_state) => matched_state,
            ::core::option::Option::None => return Ok(()),
        };

        let mut state_2 = state.borrow_mut();

        let typography = {
            let optional_typography = state_2.typography.as_ref();

            match optional_typography {
                Some(matched_typography) => matched_typography,
                ::core::option::Option::None => return Ok(()),
            }
        };

        let title_count = usize::from(title_present);
        let text_count = typography
            .role_label_candidate_count()
            .saturating_add(state_2.sequence_number.receipt.text_candidates)
            .saturating_add(title_count);
        let number = &state_2.sequence_number;
        let number_fill_covered = number.receipt.text_candidates == 0
            || (number.fill_emitted
                && !number.fill_overridden
                && number.receipt.typed_fill_applied());
        let text_fill_covered = (title_count == 0 || self.title_fill_owned)
            && typography.role_fills_cover_all_labels()
            && number_fill_covered;
        let mut evidence = FamilyThemeOutcomes::default();
        let domains = [
            UnsupportedTerminalDomain::direct(
                ThemeTarget::Text,
                TerminalVariantDomain::uniform(text_count, ThemeVariant::Default),
            )
            .with_fill_fully_overridden(text_fill_covered),
            UnsupportedTerminalDomain::direct(
                ThemeTarget::Title,
                TerminalVariantDomain::uniform(title_count, ThemeVariant::Default),
            ),
        ];
        let domains_2 = if self.title_fill.is_some() {
            { let [_explicit_part @ .., _] = &domains; let _explicit_slice: &[_] = _explicit_part; _explicit_slice }
        } else {
            { let _explicit_part = &domains; let _explicit_slice: &[_] = _explicit_part; _explicit_slice }
        };
        reconcile_unsupported_terminal_domains(theme_2, &mut evidence, domains_2, work)?;
        state_2.unsupported_text = Some(evidence);
        Ok(())
    }

    pub(crate) fn record_into(
        &self,
        theme: Option<&ResolvedDiagramTheme>,
        canonical: &mut FamilyThemeEvidence,
    ) {
        let evidence = canonical;

        match &self.title_fill {
            Some(title_fill) => {
                title_fill.record_evidence(evidence);
            },
            ::core::option::Option::None => {},
        }

        let theme_2 = match theme {
            Some(matched_theme) => matched_theme,
            ::core::option::Option::None => return,
        };

        let state = match &self.state {
            Some(matched_state) => matched_state,
            ::core::option::Option::None => return,
        };

        let state_2 = state.borrow();

        let mut actor_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut lifeline_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut message_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut sequence_number_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut loop_rules =
            BTreeMap::<ThemeTarget, BTreeMap<usize, SequenceRuleObservation>>::new();
        let mut note_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut activation_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut typography_rules =
            BTreeMap::<ThemeTarget, BTreeMap<usize, SequenceRuleObservation>>::new();
        let base_typography_routes = theme_2
            .family_mechanism_routes()
            .iter()
            .copied()
            .filter_map(|route| match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(property) => {
                    Some((property, route.disposition()))
                }
                FamilyThemeMechanism::RuleFacet { .. } | FamilyThemeMechanism::OrdinalPalette { .. } | FamilyThemeMechanism::EffectBinding { .. } => None,
            })
            .collect::<Vec<_>>();






        for route in theme_2.family_mechanism_routes().iter().copied() {
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Actor,
                ..
            } = route.mechanism()
            {
                actor_rules.entry(rule_index).or_default();
            }

            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Lifeline,
                selector:
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                ..
            } = route.mechanism()
            {
                lifeline_rules.entry(rule_index).or_default();
            }

            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Message,
                selector:
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                ..
            } = route.mechanism()
            {
                message_rules.entry(rule_index).or_default();
            }

            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::SequenceNumberLabel,
                ..
            } = route.mechanism()
            {
                sequence_number_rules.entry(rule_index).or_default();
            }

            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: target @ (ThemeTarget::Loop | ThemeTarget::LoopLabelBackground),
                selector:
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                ..
            } = route.mechanism()
            {
                loop_rules
                    .entry(target)
                    .or_default()
                    .entry(rule_index)
                    .or_default();
            }

            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Note,
                selector:
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                ..
            } = route.mechanism()
            {
                note_rules.entry(rule_index).or_default();
            }

            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Activation,
                selector:
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                ..
            } = route.mechanism()
            {
                activation_rules.entry(rule_index).or_default();
            }

            if let FamilyThemeMechanism::RuleFacet {
                rule_index, target, ..
            } = route.mechanism()
                && crate::sequence::SequenceTypographyRole::from_target(target).is_some()
            {
                typography_rules
                    .entry(target)
                    .or_default()
                    .entry(rule_index)
                    .or_default();
            }
        }

        for route in theme_2.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(_) => {}
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target:
                        target @ (ThemeTarget::ActorLabel
                        | ThemeTarget::MessageLabel
                        | ThemeTarget::NoteLabel
                        | ThemeTarget::LoopLabel),
                    selector,
                    facet,
                } => {
                    let role = {
                        let optional_role = crate::sequence::SequenceTypographyRole::from_target(target);

                        match optional_role {
                            Some(matched_role) => matched_role,
                            ::core::option::Option::None => continue,
                        }
                    };

                    let receipt = {
                        let optional_receipt = state_2.typography.as_ref().map(|receipt| receipt.role(role));

                        match optional_receipt {
                            Some(matched_receipt_2) => matched_receipt_2,
                            ::core::option::Option::None => continue,
                        }
                    };

                    let Some(observation) = typography_rules
                        .get_mut(&target)
                        .and_then(|rules| rules.get_mut(&rule_index))
                    else {
                        continue;
                    };

                    observe_typography_rule(
                        receipt,
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Lifeline,
                    selector,
                    facet,
                } if matches!(
                    selector,
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default)
                    }
                ) =>
                {
                    let observation = {
                        let optional_observation = lifeline_rules.get_mut(&rule_index);

                        match optional_observation {
                            Some(matched_observation) => matched_observation,
                            ::core::option::Option::None => continue,
                        }
                    };

                    observe_lifeline_rule(
                        &state_2.lifeline,
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Message,
                    selector,
                    facet,
                } if matches!(
                    selector,
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default)
                    }
                ) =>
                {
                    let observation = {
                        let optional_observation = message_rules.get_mut(&rule_index);

                        match optional_observation {
                            Some(matched_observation) => matched_observation,
                            ::core::option::Option::None => continue,
                        }
                    };

                    observe_message_rule(
                        &state_2.message,
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::SequenceNumberLabel,
                    selector,
                    facet,
                } => {
                    let observation = sequence_number_rules.entry(rule_index).or_default();
                    observe_sequence_number_rule(
                        &state_2.sequence_number,
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: target @ (ThemeTarget::Loop | ThemeTarget::LoopLabelBackground),
                    selector,
                    facet,
                } if matches!(
                    selector,
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default)
                    }
                ) =>
                {
                    let Some(observation) = loop_rules
                        .get_mut(&target)
                        .and_then(|rules| rules.get_mut(&rule_index))
                    else {
                        continue;
                    };

                    observe_control_rule(
                        if target == ThemeTarget::Loop {
                            &state_2.loop_frame
                        } else {
                            &state_2.keyword_background
                        },
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Actor,
                    selector,
                    facet,
                } => {
                    let observation = actor_rules.entry(rule_index).or_default();

                    if state_2.actor_count == 0
                        || !state_2
                            .actor_style_receipt
                            .route_won(rule_index, selector, facet)
                    {
                        continue;
                    }

                    observation.applicable = true;

                    if state_2.actor_fill_overridden && matches!(facet, FamilyThemeRuleFacet::Fill(_))
                    {
                        continue;
                    }

                    if state_2.actor_stroke_overridden
                        && matches!(facet, FamilyThemeRuleFacet::Stroke(_))
                    {
                        continue;
                    }

                    if state_2.actor_style_receipt.stroke_width_overridden
                        && facet == FamilyThemeRuleFacet::StrokeWidth
                    {
                        continue;
                    }

                    match route.disposition() {
                        FamilyThemeDisposition::TypedAdapter
                            if matches!(
                                facet,
                                FamilyThemeRuleFacet::Fill(
                                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                                )
                            ) =>
                        {
                            if state_2.actor_fill_unhandled {
                                observation
                                    .residual
                                    .get_or_insert(FamilyThemeResidualReason::UnsupportedPaint);
                                continue;
                            }

                            if !state_2.actor_fill_emitted {
                                continue;
                            }

                            match facet {
                                FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Transparent) => {
                                    observation
                                        .capabilities
                                        .insert(ThemeCapability::TransparentPaint);
                                }
                                FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid) => {
                                    observation.capabilities.insert(ThemeCapability::SolidPaint);
                                }
                                _ => {
                                    observation
                                        .residual
                                        .get_or_insert(FamilyThemeResidualReason::UnsupportedPaint);
                                }
                            }
                        }
                        FamilyThemeDisposition::TypedAdapter
                            if matches!(
                                facet,
                                FamilyThemeRuleFacet::Stroke(
                                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                                )
                            ) =>
                        {
                            if state_2.actor_stroke_unhandled {
                                observation
                                    .residual
                                    .get_or_insert(FamilyThemeResidualReason::UnsupportedPaint);
                                continue;
                            }

                            if !state_2.actor_stroke_emitted {
                                continue;
                            }

                            match facet {
                                FamilyThemeRuleFacet::Stroke(FamilyThemePaintKind::Transparent) => {
                                    observation
                                        .capabilities
                                        .insert(ThemeCapability::TransparentPaint);
                                }
                                FamilyThemeRuleFacet::Stroke(FamilyThemePaintKind::Solid) => {
                                    observation.capabilities.insert(ThemeCapability::SolidPaint);
                                }
                                _ => {
                                    observation
                                        .residual
                                        .get_or_insert(FamilyThemeResidualReason::UnsupportedPaint);
                                }
                            }
                        }
                        FamilyThemeDisposition::TypedAdapter
                            if matches!(
                                facet,
                                FamilyThemeRuleFacet::StrokeWidth | FamilyThemeRuleFacet::Radius
                            ) =>
                        {
                            if state_2.actor_style_receipt.geometry_unhandled.get() {
                                observation
                                    .residual
                                    .get_or_insert(FamilyThemeResidualReason::UnsupportedGeometry);
                            } else if !state_2.actor_style_receipt.geometry_complete() {
                                observation.incomplete = true;
                            } else {
                                observation.capabilities.insert(
                                    if facet == FamilyThemeRuleFacet::StrokeWidth {
                                        ThemeCapability::BorderStyling
                                    } else {
                                        ThemeCapability::RoundedGeometry
                                    },
                                );
                            }
                        }
                        FamilyThemeDisposition::TypedAdapter
                            if facet == FamilyThemeRuleFacet::Effect =>
                        {
                            let receipt = &state_2.actor_style_receipt;

                            if receipt.effect_unhandled || receipt.geometry_unhandled.get() {
                                observation
                                    .residual
                                    .get_or_insert(FamilyThemeResidualReason::UnsupportedEffect);
                            } else if !receipt.effect_complete() {
                                observation.incomplete = true;
                            } else {
                                observation
                                    .capabilities
                                    .insert(ThemeCapability::SemanticRules);

                                if !receipt.effect_cleared {
                                    observation.capabilities.extend([
                                        ThemeCapability::Shadow,
                                        ThemeCapability::SvgFilter,
                                    ]);
                                }
                            }
                        }
                        FamilyThemeDisposition::TypedAdapter => observation.incomplete = true,
                        FamilyThemeDisposition::Unsupported => {
                            observation
                                .residual
                                .get_or_insert(unsupported_reason_for_facet(facet));
                        }
                    }
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: target @ (ThemeTarget::Note | ThemeTarget::Activation),
                    selector,
                    facet,
                } if matches!(
                    selector,
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default)
                    }
                ) =>
                {
                    let (surface, observation) = match target {
                        ThemeTarget::Note => {
                            (&state_2.note, note_rules.entry(rule_index).or_default())
                        }
                        ThemeTarget::Activation => (
                            &state_2.activation,
                            activation_rules.entry(rule_index).or_default(),
                        ),
                        _ => unreachable!("guarded by the static rectangle target pattern"),
                    };
                    observe_static_rect_rule(
                        surface,
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Actor,
                } => {
                    let key = theme_2.family_mechanism_key(route);

                    if state_2.actor_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if matches!(route.disposition(), FamilyThemeDisposition::Unsupported) {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target:
                        target @ (ThemeTarget::ActorLabel
                        | ThemeTarget::NoteLabel
                        | ThemeTarget::LoopLabel),
                    ..
                } => {
                    let key = theme_2.family_mechanism_key(route);

                    match &state_2.typography {
                        Some(typography) => {
                            let receipt = typography.role(
                                crate::sequence::SequenceTypographyRole::from_target(target)
                                    .expect("matched text effect target"),
                            );

                            if receipt.label_candidate_count() == 0
                                || (receipt.effect_requested && !receipt.effect_binding_used)
                            {
                                evidence.mark_not_applicable(key);
                            } else if receipt.effect_unhandled {
                                evidence
                                    .mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                            } else if receipt.effect_complete()
                                && receipt.effect_applications.get() == 0
                            {
                                evidence.mark_not_applicable(key);
                            } else if receipt.effect_complete() {
                                evidence.mark_applied_with_capabilities(
                                    key,
                                    [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                                );
                            }
                        },
                        ::core::option::Option::None => {},
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Actor,
                    ..
                } => {
                    let key = theme_2.family_mechanism_key(route);
                    let receipt = &state_2.actor_style_receipt;

                    if state_2.actor_count == 0
                        || (receipt.effect_requested && !receipt.effect_binding_used)
                    {
                        evidence.mark_not_applicable(key);
                    } else if receipt.effect_unhandled || receipt.geometry_unhandled.get() {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    } else if receipt.effect_complete() {
                        evidence.mark_applied_with_capabilities(
                            key,
                            [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: target @ (ThemeTarget::Loop | ThemeTarget::LoopLabelBackground),
                    ..
                } => {
                    let key = theme_2.family_mechanism_key(route);
                    let receipt = if target == ThemeTarget::Loop {
                        &state_2.loop_frame.receipt
                    } else {
                        &state_2.keyword_background.receipt
                    };

                    if receipt.surface_candidates.get() == 0
                        || (receipt.effect_requested && !receipt.effect_binding_used)
                    {
                        evidence.mark_not_applicable(key);
                    } else if receipt.effect_unhandled.get() {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    } else if receipt.effect_complete() {
                        evidence.mark_applied_with_capabilities(
                            key,
                            [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Lifeline,
                    ..
                } => {
                    let key = theme_2.family_mechanism_key(route);
                    let receipt = &state_2.lifeline.receipt;

                    if receipt.candidate_count() == 0
                        || (receipt.effect_requested && !receipt.effect_binding_used)
                    {
                        evidence.mark_not_applicable(key);
                    } else if receipt.effect_unhandled {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    } else if receipt.effect_complete() {
                        evidence.mark_applied_with_capabilities(
                            key,
                            [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Message,
                    ..
                } => {
                    let key = theme_2.family_mechanism_key(route);
                    let receipt = &state_2.message.receipt;

                    if receipt.candidate_count() == 0
                        || (receipt.effect_requested && !receipt.effect_binding_used)
                    {
                        evidence.mark_not_applicable(key);
                    } else if receipt.effect_unhandled {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    } else if receipt.effect_complete() {
                        evidence.mark_applied_with_capabilities(
                            key,
                            [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: target @ (ThemeTarget::Note | ThemeTarget::Activation),
                    ..
                } => {
                    let key = theme_2.family_mechanism_key(route);
                    let surface = if target == ThemeTarget::Note {
                        &state_2.note
                    } else {
                        &state_2.activation
                    };
                    let receipt = &surface.receipt;

                    if surface.surface_count == 0
                        || (receipt.effect_requested && !receipt.effect_binding_used)
                    {
                        evidence.mark_not_applicable(key);
                    } else if receipt.effect_unhandled {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    } else if receipt.effect_complete(surface.surface_count) {
                        evidence.mark_applied_with_capabilities(
                            key,
                            [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                        );
                    }
                }
                FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {





                }
            }
        }

        for (rule_index, observation) in actor_rules {
            let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Actor,
            };

            if state_2.actor_count == 0 || !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {

            } else if observation.capabilities.is_empty() {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_applied_with_capabilities(key, observation.capabilities);
            }
        }

        finish_line_rules(
            evidence,
            ThemeTarget::Lifeline,
            state_2.lifeline.receipt.candidate_count(),
            lifeline_rules,
        );
        finish_line_rules(
            evidence,
            ThemeTarget::Message,
            state_2.message.receipt.candidate_count(),
            message_rules,
        );
        finish_sequence_number_rules(evidence, &state_2.sequence_number, sequence_number_rules);

        for (target, rules) in loop_rules {
            finish_control_rules(
                evidence,
                target,
                if target == ThemeTarget::Loop {
                    &state_2.loop_frame
                } else {
                    &state_2.keyword_background
                },
                rules,
            );
        }

        finish_static_rect_rules(evidence, ThemeTarget::Note, &state_2.note, note_rules);
        finish_static_rect_rules(
            evidence,
            ThemeTarget::Activation,
            &state_2.activation,
            activation_rules,
        );
        finish_base_typography(evidence, state_2.typography.as_ref(), &base_typography_routes);
        finish_typography_rules(evidence, state_2.typography.as_ref(), typography_rules);

        match &state_2.unsupported_text {
            Some(unsupported_text) => {
                evidence.merge_borrowed(unsupported_text);
            },
            ::core::option::Option::None => {},
        }
    }
}

fn finish_base_typography(
    evidence: &mut FamilyThemeEvidence,
    receipt: Option<&SequenceTypographyThemeReceipt>,
    routes: &[(ThemeTypographyProperty, FamilyThemeDisposition)],
) {
    match routes {
        [] => {
            return;
        },
        [_, ..] => {},
    }

    let receipt_2 = match receipt {
        Some(matched_receipt) => matched_receipt,
        ::core::option::Option::None => return,
    };

    for (property, disposition) in routes {
        let key = crate::diagram_theme::FamilyThemeMechanismKey::Typography(*property);
        let relevant_occurrences = match property {
            ThemeTypographyProperty::FontStack | ThemeTypographyProperty::FontSize => {
                receipt_2.base_property_relevant_occurrences(*property)
            }
            ThemeTypographyProperty::FontWeight
            | ThemeTypographyProperty::FontStyle
            | ThemeTypographyProperty::LineHeight
            | ThemeTypographyProperty::LetterSpacing
            | ThemeTypographyProperty::WordSpacing
            | ThemeTypographyProperty::Transform
            | ThemeTypographyProperty::Decoration
            | ThemeTypographyProperty::TextAlign
            | ThemeTypographyProperty::WhiteSpace
            | ThemeTypographyProperty::Wrap => receipt_2.base_relevant_occurrences(),
        };

        match disposition {
            FamilyThemeDisposition::Unsupported => {
                match relevant_occurrences {
                    0 => {
                        evidence.mark_not_applicable(key);
                    },
                    _ => {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                    },
                }
            }
            FamilyThemeDisposition::TypedAdapter => {
                if relevant_occurrences == 0 || !receipt_2.base.has_typed_property(*property) {
                    evidence.mark_not_applicable(key);
                } else if receipt_2.base_property_incomplete(*property) {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                } else if receipt_2.base_property_applied(*property) {
                    evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
                } else {



                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                }
            }
        }
    }
}

fn finish_typography_rules(
    evidence: &mut FamilyThemeEvidence,
    receipt: Option<&SequenceTypographyThemeReceipt>,
    rules_by_target: BTreeMap<ThemeTarget, BTreeMap<usize, SequenceRuleObservation>>,
) {
    for (target, rules) in rules_by_target {
        let role = crate::sequence::SequenceTypographyRole::from_target(target)
            .expect("Sequence typography rules are keyed only by role targets");
        let candidate_count =
            receipt.map_or(0, |receipt_2| receipt_2.role(role).label_candidate_count());

        for (rule_index, observation) in rules {
            let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target,
            };

            if candidate_count == 0 || !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {

            } else if observation.capabilities.is_empty() {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_applied_with_capabilities(key, observation.capabilities);
            }
        }
    }
}

fn finish_line_rules(
    evidence: &mut FamilyThemeEvidence,
    target: ThemeTarget,
    candidate_count: usize,
    rules: BTreeMap<usize, SequenceRuleObservation>,
) {
    for (rule_index, observation) in rules {
        let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
            index: rule_index,
            target,
        };

        if candidate_count == 0 || !observation.applicable {
            evidence.mark_not_applicable(key);
        } else if let Some(reason) = observation.residual {
            evidence.mark_residual(key, reason);
        } else if observation.incomplete {

        } else if observation.capabilities.is_empty() {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, observation.capabilities);
        }
    }
}

fn finish_sequence_number_rules(
    evidence: &mut FamilyThemeEvidence,
    sequence_number: &SequenceNumberLabelThemeEmission,
    rules: BTreeMap<usize, SequenceRuleObservation>,
) {
    for (rule_index, observation) in rules {
        let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
            index: rule_index,
            target: ThemeTarget::SequenceNumberLabel,
        };

        if sequence_number.receipt.text_candidates == 0 || !observation.applicable {
            evidence.mark_not_applicable(key);
        } else if let Some(reason) = observation.residual {
            evidence.mark_residual(key, reason);
        } else if observation.incomplete {

        } else if observation.capabilities.is_empty() {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, observation.capabilities);
        }
    }
}

fn finish_control_rules(
    evidence: &mut FamilyThemeEvidence,
    target: ThemeTarget,
    surface: &SequenceControlThemeEmission,
    rules: BTreeMap<usize, SequenceRuleObservation>,
) {
    for (rule_index, observation) in rules {
        let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
            index: rule_index,
            target,
        };

        if surface.receipt.surface_candidates.get() == 0 || !observation.applicable {
            evidence.mark_not_applicable(key);
        } else if let Some(reason) = observation.residual {
            evidence.mark_residual(key, reason);
        } else if observation.incomplete {

        } else if observation.capabilities.is_empty() {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, observation.capabilities);
        }
    }
}

fn finish_static_rect_rules(
    evidence: &mut FamilyThemeEvidence,
    target: ThemeTarget,
    surface: &SequenceStaticRectThemeEmission,
    rules: BTreeMap<usize, SequenceRuleObservation>,
) {
    for (rule_index, observation) in rules {
        let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
            index: rule_index,
            target,
        };

        if surface.surface_count == 0 || !observation.applicable {
            evidence.mark_not_applicable(key);
        } else if let Some(reason) = observation.residual {
            evidence.mark_residual(key, reason);
        } else if observation.incomplete {

        } else if observation.capabilities.is_empty() {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, observation.capabilities);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, FamilyThemeMechanismKey, ThemeRule,
        ThemeRuleSet, ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;

    #[test]
    fn record_into_preserves_canonical_outcomes_and_reuses_terminal_text_receipts() {
        let mut rules = ThemeRuleSet::default();
        for target in [ThemeTarget::Actor, ThemeTarget::Title] {
            let paint = CanvasPaint::solid("#123456").unwrap();
            rules = rules.with_rule(ThemeRule::new(
                target,
                if target == ThemeTarget::Title {
                    ThemeStylePatch::default().with_stroke(paint)
                } else {
                    ThemeStylePatch::default().with_fill(paint)
                },
            ));
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap()
            .resolve(DiagramFamilyId::SEQUENCE);
        let actor = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Actor,
        };
        let title = FamilyThemeMechanismKey::Rule {
            index: 1,
            target: ThemeTarget::Title,
        };
        let recorder = SequenceThemeEvidenceRecorder::new(true);
        recorder.record_typography_emission(SequenceTypographyThemeReceipt::default());
        recorder
            .record_unsupported_text_emission(
                Some(&theme),
                true,
                &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
            )
            .unwrap();
        let mut canonical = FamilyThemeEvidence::from_theme(Some(&theme));
        canonical.mark_residual(
            actor.clone(),
            FamilyThemeResidualReason::UnsupportedGeometry,
        );

        recorder.record_into(Some(&theme), &mut canonical);
        assert!(canonical.applied().is_empty());
        assert!(canonical.not_applicable_mechanisms().is_empty());
        assert_eq!(canonical.residuals().len(), 2);
        assert_eq!(canonical.residuals()[0].key(), &actor);
        assert_eq!(
            canonical.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedGeometry,
        );
        assert_eq!(canonical.residuals()[1].key(), &title);

        let mut second = FamilyThemeEvidence::from_theme(Some(&theme));
        second.mark_applied_with_capabilities(title.clone(), [ThemeCapability::SolidPaint]);
        recorder.record_into(Some(&theme), &mut second);
        assert_eq!(second.applied(), &[title]);
        assert_eq!(second.not_applicable_mechanisms(), &[actor]);
        assert!(second.residuals().is_empty());
    }

    #[test]
    fn title_evidence_requires_matching_text_and_paint_from_one_terminal() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Title,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )),
                ),
            )
            .unwrap()
            .resolve(DiagramFamilyId::SEQUENCE);
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let recorder = SequenceThemeEvidenceRecorder::with_title(
            Some(&theme),
            &merman_model::presentation::PresentationConfig::empty_object(),
            Some("Title"),
            &work,
        )
        .unwrap();
        assert!(!recorder.record_title_emission("Other", Some("#123456")));
        assert!(!recorder.record_title_emission("Title", Some("#987654")));
        let mut evidence = FamilyThemeEvidence::from_theme(Some(&theme));
        recorder.record_into(Some(&theme), &mut evidence);
        assert!(evidence.applied().is_empty());
        assert!(recorder.record_title_emission("Title", Some("#123456")));
        assert!(!recorder.record_title_emission("Title", Some("#123456")));
        recorder.record_into(Some(&theme), &mut evidence);
        assert_eq!(evidence.applied().len(), 1);
    }
}
