use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_model::families::c4::C4Boundary;

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemePaintKind, FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme,
    ResolvedStyleProperty, Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeOutcomes, FamilyThemeResidualReason, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

const MERMAID_BOUNDARY_FILL: &str = "none";
const MERMAID_BOUNDARY_STROKE: &str = "#444444";
const MERMAID_BOUNDARY_RADIUS_PX: f64 = 2.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum C4BoundaryPaintOwner {
    Mermaid,
    Typed { rule_index: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct C4BoundaryPaintExpectation {
    token: Box<str>,
    owner: C4BoundaryPaintOwner,
}

impl C4BoundaryPaintExpectation {
    #[must_use]
    fn mermaid(token: impl Into<Box<str>>) -> Self {
        Self {
            token: <_ as ::core::convert::Into<::std::boxed::Box<::core::primitive::str>>>::into(token),
            owner: C4BoundaryPaintOwner::Mermaid,
        }
    }

    #[must_use]
    fn typed(paint: &C4TypedPaint) -> Self {
        Self {
            token: paint.token.clone(),
            owner: C4BoundaryPaintOwner::Typed {
                rule_index: paint.rule_index,
            },
        }
    }

    #[must_use]
    const fn typed_rule_index(&self) -> Option<usize> {
        match self.owner {
            C4BoundaryPaintOwner::Mermaid => None,
            C4BoundaryPaintOwner::Typed { rule_index } => Some(rule_index),
        }
    }
}

#[derive(Debug, Clone)]
struct C4BoundaryTerminalExpectation {
    alias: Box<str>,
    fill: C4BoundaryPaintExpectation,
    stroke: C4BoundaryPaintExpectation,
    radius_token: Box<str>,
    radius_rule_index: Option<usize>,
}

impl C4BoundaryTerminalExpectation {
    #[must_use]
    fn baseline(
        boundary: &C4Boundary,
        config: &merman_model::presentation::PresentationConfig,
    ) -> Self {

        Self {
            alias: boundary.alias.clone().into_boxed_str(),
            fill: C4BoundaryPaintExpectation::mermaid(
                boundary
                    .bg_color
                    .as_deref()
                    .or_else(|| config.get_str("c4.boundary_bg_color"))
                    .unwrap_or(MERMAID_BOUNDARY_FILL),
            ),
            stroke: C4BoundaryPaintExpectation::mermaid(
                boundary
                    .border_color
                    .as_deref()
                    .or_else(|| config.get_str("c4.boundary_border_color"))
                    .unwrap_or(MERMAID_BOUNDARY_STROKE),
            ),
            radius_token: baseline_radius_token(),
            radius_rule_index: None,
        }
    }
}

#[derive(Debug)]
struct C4TypedPaint {
    token: Box<str>,
    rule_index: usize,
}

/// Final C4 explicit-boundary paint and radius shared by terminal SVG emission and family evidence.
#[derive(Debug)]
pub(crate) struct C4ClusterThemePlan {
    explicit_boundary_count: usize,
    boundaries: BTreeMap<Box<str>, C4BoundaryTerminalExpectation>,
    evidence: FamilyThemeOutcomes,
    pending: BTreeMap<FamilyThemeMechanismKey, C4ClusterPendingEvidence>,
    terminal_receipt: OnceLock<C4ClusterThemeReceipt>,
}

impl C4ClusterThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        boundaries: &[C4Boundary],
        config: &merman_model::presentation::PresentationConfig,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let explicit_boundaries = boundaries
            .iter()
            .filter(|boundary| boundary.alias != "global")
            .collect::<Vec<_>>();
        let explicit_boundary_count = explicit_boundaries.len();

        let theme_2 = match theme {
            Some(matched_theme) => matched_theme,
            ::core::option::Option::None => return Ok(Self::baseline(boundaries, config)),
        };

        let static_style = theme_2.style_with_work_meter(
            ThemeTarget::Cluster,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let static_winner_rules = static_style
            .winner_rule_properties()
            .map(|(property, origin)| (property, origin.rule_index()))
            .collect::<BTreeMap<_, _>>();
        let static_winners = static_winner_rules
            .iter()
            .map(|(property, rule_index)| (*rule_index, *property))
            .collect::<BTreeSet<_>>();
        let static_radius_candidate = typed_radius_token(theme_2, &static_style);
        let direct_static_radius_rule = static_style
            .radius_resolution()
            .winner()
            .filter(|origin| {
                theme_2.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Radius)
                    == Some(FamilyThemeDisposition::TypedAdapter)
            })
            .map(|origin| origin.rule_index());
        let static_fill_candidate = typed_paint(
            theme_2,
            static_style.fill_resolution(),
            FamilyThemeRuleFacet::fill,
        );
        let static_stroke_candidate = typed_paint(
            theme_2,
            static_style.stroke_resolution(),
            FamilyThemeRuleFacet::stroke,
        );

        let has_ordinal_cluster_rules = theme_2
            .family_rules()
            .any(|(_, rule)| rule.target() == ThemeTarget::Cluster && rule.ordinal().is_some());
        let mut occurrence_winners = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut static_radius_wins_every_boundary =
            explicit_boundary_count != 0 && direct_static_radius_rule.is_some();
        let mut typed_fill_rules = BTreeSet::new();
        let mut typed_stroke_rules = BTreeSet::new();
        let mut source_owned_fill_rules = BTreeSet::new();
        let mut source_owned_stroke_rules = BTreeSet::new();
        let mut terminal_expectations = BTreeMap::new();
        let source_owned_fill = explicit_boundaries
            .iter()
            .map(|boundary| {
                boundary.bg_color.is_some() || config.get_str("c4.boundary_bg_color").is_some()
            })
            .collect::<Vec<_>>();

        for (boundary_index, boundary) in explicit_boundaries.iter().enumerate() {
            let winner_rules = if has_ordinal_cluster_rules {
                theme_2
                    .style_with_work_meter(
                        ThemeTarget::Cluster,
                        ThemeVariant::Default,
                        Some(boundary_index + 1),
                        work_meter,
                    )?
                    .winner_rule_properties()
                    .map(|(property, origin)| (property, origin.rule_index()))
                    .collect::<BTreeMap<_, _>>()
            } else {
                static_winner_rules.clone()
            };
            occurrence_winners.extend(
                winner_rules
                    .iter()
                    .map(|(property, rule_index)| (*rule_index, *property)),
            );
            static_radius_wins_every_boundary &=
                winner_rules.get(&ResolvedStyleProperty::Radius).copied()
                    == direct_static_radius_rule;

            let mut expectation = C4BoundaryTerminalExpectation::baseline(boundary, config);
            let (fill, source_owned_fill_2) = paint_expectation(
                boundary
                    .bg_color
                    .as_deref()
                    .or_else(|| config.get_str("c4.boundary_bg_color")),
                MERMAID_BOUNDARY_FILL,
                static_fill_candidate.as_ref(),
                winner_rules.get(&ResolvedStyleProperty::Fill).copied(),
            );
            expectation.fill = fill;

            match expectation.fill.typed_rule_index() {
                Some(rule_index) => {
                    typed_fill_rules.insert(rule_index);
                },
                ::core::option::Option::None => {},
            }

            match source_owned_fill_2 {
                Some(rule_index) => {
                    source_owned_fill_rules.insert(rule_index);
                },
                ::core::option::Option::None => {},
            }

            let (stroke, source_owned_stroke) = paint_expectation(
                boundary
                    .border_color
                    .as_deref()
                    .or_else(|| config.get_str("c4.boundary_border_color")),
                MERMAID_BOUNDARY_STROKE,
                static_stroke_candidate.as_ref(),
                winner_rules.get(&ResolvedStyleProperty::Stroke).copied(),
            );
            expectation.stroke = stroke;

            match expectation.stroke.typed_rule_index() {
                Some(rule_index) => {
                    typed_stroke_rules.insert(rule_index);
                },
                ::core::option::Option::None => {},
            }

            match source_owned_stroke {
                Some(rule_index) => {
                    source_owned_stroke_rules.insert(rule_index);
                },
                ::core::option::Option::None => {},
            }

            terminal_expectations.insert(expectation.alias.clone(), expectation);
        }

        let terminal_radius_rule = static_radius_wins_every_boundary
            .then_some(direct_static_radius_rule)
            .flatten();
        let radius_token = match terminal_radius_rule {
            ::core::option::Option::Some(_) => {
                static_radius_candidate.unwrap_or_else(baseline_radius_token)
            },
            ::core::option::Option::None => {
                baseline_radius_token()
            },
        };

        for expectation in terminal_expectations.values_mut() {
            expectation.radius_token = radius_token.clone();
            expectation.radius_rule_index = terminal_radius_rule;
        }

        let mut evidence = FamilyThemeOutcomes::default();
        let mut observations = BTreeMap::<usize, C4ClusterRuleObservation>::new();

        for route in theme_2.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Cluster,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();

                    if !selector.ordinal_domain_intersects_occurrence_count(explicit_boundary_count)
                    {
                        continue;
                    }

                    let property = resolved_style_property_for_facet(facet);
                    let qualified_variant = matches!(
                        selector,
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                            | FamilyThemeSelectorShape::Ordinal {
                                variant: Some(_),
                                ..
                            }
                    );
                    let route_won = match selector {
                        FamilyThemeSelectorShape::Static { variant: None } => {
                            static_winners.contains(&(rule_index, property))
                        }
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                        | FamilyThemeSelectorShape::Ordinal { .. } => {
                            occurrence_winners.contains(&(rule_index, property))
                        }
                    };

                    if !route_won && !qualified_variant {
                        continue;
                    }

                    observation.applicable = true;

                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Radius,
                        ) if terminal_radius_rule == Some(rule_index) => {
                            observation.pending.radius = true;
                            observation
                                .pending
                                .capabilities
                                .insert(ThemeCapability::RoundedGeometry);
                        }
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            facet_2 @ FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) if typed_fill_rules.contains(&rule_index) => {
                            observation.pending.fill = true;
                            add_paint_capabilities(&mut observation.pending.capabilities, facet_2);
                        }
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) if source_owned_fill_rules.contains(&rule_index) => {}
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            facet_3 @ FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) if typed_stroke_rules.contains(&rule_index) => {
                            observation.pending.stroke = true;
                            add_paint_capabilities(&mut observation.pending.capabilities, facet_3);
                        }
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) if source_owned_stroke_rules.contains(&rule_index) => {}
                        (FamilyThemeDisposition::Unsupported, _, facet_4) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet_4));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Cluster,
                } => {}
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Cluster,
                    ..
                } => {}
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        reconcile_unsupported_terminal_domains(
            theme_2,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Cluster,
                TerminalVariantDomain::uniform(explicit_boundary_count, ThemeVariant::Default),
            )
            .with_source_owned_fill(&source_owned_fill)],
            work_meter,
        )?;

        let mut pending = BTreeMap::new();

        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Cluster,
            };

            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {

            } else if observation.pending.requires_terminal_proof() {
                pending.insert(key, observation.pending);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            explicit_boundary_count,
            boundaries: terminal_expectations,
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    #[must_use]
    pub(crate) fn baseline(
        boundaries: &[C4Boundary],
        config: &merman_model::presentation::PresentationConfig,
    ) -> Self {
        let boundaries_2 = boundaries
            .iter()
            .filter(|boundary| boundary.alias != "global")
            .map(|boundary| C4BoundaryTerminalExpectation::baseline(boundary, config))
            .map(|expectation| (expectation.alias.clone(), expectation))
            .collect::<BTreeMap<_, _>>();
        Self {
            explicit_boundary_count: boundaries_2.len(),
            boundaries: boundaries_2,
            evidence: FamilyThemeOutcomes::default(),
            pending: BTreeMap::new(),
            terminal_receipt: OnceLock::new(),
        }
    }

    #[must_use]
    pub(crate) fn boundary_tokens(&self, alias: &str) -> Option<(&str, &str, &str)> {
        let expectation = self.boundaries.get(alias)?;
        Some((
            &expectation.fill.token,
            &expectation.stroke.token,
            &expectation.radius_token,
        ))
    }

    #[must_use]
    pub(crate) fn has_separate_background(&self, alias: &str) -> bool {
        self.boundaries
            .get(alias)
            .is_some_and(|boundary| boundary_fill_is_visible(&boundary.fill.token))
    }

    #[must_use]
    pub(crate) fn begin_terminal_receipt(&self) -> Option<C4ClusterThemeReceipt> {
        (!self.pending.is_empty()).then(|| {
            C4ClusterThemeReceipt::new(self.explicit_boundary_count, self.boundaries.clone())
        })
    }

    #[must_use]
    pub(crate) fn record_terminal(&self, receipt: C4ClusterThemeReceipt) -> bool {
        !self.pending.is_empty()
            && receipt.proves_complete(self.explicit_boundary_count)
            && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);

        let receipt = {
            let optional_receipt = self.terminal_receipt.get();

            match optional_receipt {
                Some(matched_receipt) => matched_receipt,
                ::core::option::Option::None => return,
            }
        };

        for (key, pending) in &self.pending {
            let FamilyThemeMechanismKey::Rule { index, .. } = key else {
                continue;
            };

            if receipt.proves_rule(*index, pending) {
                evidence.mark_applied_with_capabilities(
                    key.clone(),
                    pending.capabilities.iter().copied(),
                );
            }
        }
    }
}

#[must_use]
fn baseline_radius_token() -> Box<str> {
    MERMAID_BOUNDARY_RADIUS_PX.to_string().into_boxed_str()
}

#[must_use]
fn boundary_fill_is_visible(fill: &str) -> bool {
    !fill.eq_ignore_ascii_case("none") && !fill.eq_ignore_ascii_case("transparent")
}

#[must_use]
fn typed_radius_token(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<Box<str>> {
    let origin = style.radius_resolution().winner()?;

    if theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Radius)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }

    match style.radius_resolution().specified() {
        Specified::Value(value) => Some(value.to_string().into_boxed_str()),
        Specified::Clear => Some(baseline_radius_token()),
        Specified::Unspecified => None,
    }
}

#[must_use]
fn typed_paint(
    theme: &ResolvedDiagramTheme,
    resolution: &crate::diagram_theme::ResolvedProperty<CanvasPaint>,
    facet: fn(&Specified<CanvasPaint>) -> Option<FamilyThemeRuleFacet>,
) -> Option<C4TypedPaint> {
    let origin = resolution.winner()?;
    let specified = resolution.specified();
    let facet_2 = facet(specified)?;

    if theme.rule_facet_disposition(origin.rule_index(), facet_2)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }

    let token = match specified {
        Specified::Value(CanvasPaint::Transparent) => <::std::boxed::Box<::core::primitive::str> as ::core::convert::From<&::core::primitive::str>>::from("transparent"),
        Specified::Value(CanvasPaint::Solid(color)) => color.as_css().into_boxed_str(),
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(
            CanvasPaint::LinearGradient(_)
            | CanvasPaint::RadialGradient(_)
            | CanvasPaint::Pattern(_),
        ) => return None,
    };
    Some(C4TypedPaint {
        token,
        rule_index: origin.rule_index(),
    })
}

#[must_use]
fn paint_expectation(
    source_token: Option<&str>,
    baseline_token: &str,
    typed_candidate: Option<&C4TypedPaint>,
    occurrence_winner: Option<usize>,
) -> (C4BoundaryPaintExpectation, Option<usize>) {
    let typed_winner =
        typed_candidate.filter(|candidate| occurrence_winner == Some(candidate.rule_index));

    match source_token {
        Some(source_token_2) => {
            return (
                C4BoundaryPaintExpectation::mermaid(source_token_2),
                typed_winner.map(|candidate| candidate.rule_index),
            );
        },
        ::core::option::Option::None => {},
    }

    match typed_winner {
        Some(typed_winner_2) => {
            return (C4BoundaryPaintExpectation::typed(typed_winner_2), None);
        },
        ::core::option::Option::None => {},
    }

    (C4BoundaryPaintExpectation::mermaid(baseline_token), None)
}

fn add_paint_capabilities(
    capabilities: &mut BTreeSet<ThemeCapability>,
    facet: FamilyThemeRuleFacet,
) {
    let kind = match facet {
        FamilyThemeRuleFacet::Fill(kind) | FamilyThemeRuleFacet::Stroke(kind) => kind,
        FamilyThemeRuleFacet::StrokeWidth | FamilyThemeRuleFacet::StrokeDasharray | FamilyThemeRuleFacet::StrokeLinecap | FamilyThemeRuleFacet::StrokeLinejoin | FamilyThemeRuleFacet::Opacity | FamilyThemeRuleFacet::FillOpacity | FamilyThemeRuleFacet::StrokeOpacity | FamilyThemeRuleFacet::Radius | FamilyThemeRuleFacet::Padding | FamilyThemeRuleFacet::Typography(_) | FamilyThemeRuleFacet::Effect => return,
    };

    match kind {
        FamilyThemePaintKind::Transparent => {
            capabilities.insert(ThemeCapability::TransparentPaint);
        }
        FamilyThemePaintKind::Solid => {
            capabilities.insert(ThemeCapability::SolidPaint);
        }
        FamilyThemePaintKind::Clear
        | FamilyThemePaintKind::LinearGradient
        | FamilyThemePaintKind::RadialGradient
        | FamilyThemePaintKind::Pattern => {}
    }

    if matches!(facet, FamilyThemeRuleFacet::Stroke(_)) {
        capabilities.insert(ThemeCapability::BorderStyling);
    }
}

#[derive(Debug, Default)]
struct C4ClusterRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    pending: C4ClusterPendingEvidence,
}

#[derive(Debug, Default)]
struct C4ClusterPendingEvidence {
    radius: bool,
    fill: bool,
    stroke: bool,
    capabilities: BTreeSet<ThemeCapability>,
}

impl C4ClusterPendingEvidence {
    #[must_use]
    const fn requires_terminal_proof(&self) -> bool {
        self.radius || self.fill || self.stroke
    }
}

/// Writer-owned proof that every explicit C4 boundary emitted its resolved terminal theme state.
#[derive(Debug)]
pub(crate) struct C4ClusterThemeReceipt {
    expected_boundary_count: usize,
    expectations: BTreeMap<Box<str>, C4BoundaryTerminalExpectation>,
    next_boundary_ordinal: usize,
    terminal_aliases: BTreeSet<Box<str>>,
    terminals_match: bool,
    radius_rules: BTreeSet<usize>,
    fill_rules: BTreeSet<usize>,
    stroke_rules: BTreeSet<usize>,
    backgrounds: BTreeSet<Box<str>>,
}

impl C4ClusterThemeReceipt {
    #[must_use]
    fn new(
        expected_boundary_count: usize,
        expectations: BTreeMap<Box<str>, C4BoundaryTerminalExpectation>,
    ) -> Self {
        Self {
            expected_boundary_count,
            expectations,
            next_boundary_ordinal: 0,
            terminal_aliases: BTreeSet::new(),
            terminals_match: true,
            radius_rules: BTreeSet::new(),
            fill_rules: BTreeSet::new(),
            stroke_rules: BTreeSet::new(),
            backgrounds: BTreeSet::new(),
        }
    }

    pub(crate) fn record_checkpointed_background(&mut self, alias: &str, fill: &str, radius: &str) {
        self.terminals_match &= self.expectations.get(alias).is_some_and(|expected| {
            boundary_fill_is_visible(&expected.fill.token)
                && expected.fill.token.as_ref() == fill
                && expected.radius_token.as_ref() == radius
        }) && self.backgrounds.insert(<::std::boxed::Box<::core::primitive::str> as ::core::convert::From<&::core::primitive::str>>::from(alias));
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_checkpointed_boundary(
        &mut self,
        boundary_ordinal: usize,
        boundary_alias: &str,
        emitted_fill_token: &str,
        emitted_stroke_token: &str,
        emitted_rx_token: &str,
        emitted_ry_token: &str,
    ) {
        if boundary_ordinal != self.next_boundary_ordinal {
            self.terminals_match = false;
            return;
        }

        self.next_boundary_ordinal = self.next_boundary_ordinal.saturating_add(1);

        let Some(expectation) = self.expectations.get(boundary_alias) else {
            self.terminals_match = false;
            return;
        };

        let terminal_matches = expectation.alias.as_ref() == boundary_alias
            && if boundary_fill_is_visible(&expectation.fill.token) {
                emitted_fill_token == "none" && self.backgrounds.contains(boundary_alias)
            } else {
                expectation.fill.token.as_ref() == emitted_fill_token
            }
            && expectation.stroke.token.as_ref() == emitted_stroke_token
            && expectation.radius_token.as_ref() == emitted_rx_token
            && expectation.radius_token.as_ref() == emitted_ry_token
            && self.terminal_aliases.insert(<::std::boxed::Box<::core::primitive::str> as ::core::convert::From<&::core::primitive::str>>::from(boundary_alias));
        self.terminals_match &= terminal_matches;

        if !terminal_matches {
            return;
        }

        match expectation.radius_rule_index {
            Some(rule_index) => {
                self.radius_rules.insert(rule_index);
            },
            ::core::option::Option::None => {},
        }

        match expectation.fill.typed_rule_index() {
            Some(rule_index) => {
                self.fill_rules.insert(rule_index);
            },
            ::core::option::Option::None => {},
        }

        match expectation.stroke.typed_rule_index() {
            Some(rule_index) => {
                self.stroke_rules.insert(rule_index);
            },
            ::core::option::Option::None => {},
        }
    }

    #[must_use]
    fn proves_complete(&self, expected_boundary_count: usize) -> bool {
        self.expected_boundary_count == expected_boundary_count
            && self.expectations.len() == expected_boundary_count
            && self.next_boundary_ordinal == expected_boundary_count
            && self.terminal_aliases.len() == expected_boundary_count
            && self.terminals_match
    }

    #[must_use]
    fn proves_rule(&self, rule_index: usize, pending: &C4ClusterPendingEvidence) -> bool {
        self.proves_complete(self.expected_boundary_count)
            && (!pending.radius || self.radius_rules.contains(&rule_index))
            && (!pending.fill || self.fill_rules.contains(&rule_index))
            && (!pending.stroke || self.stroke_rules.contains(&rule_index))
    }
}
