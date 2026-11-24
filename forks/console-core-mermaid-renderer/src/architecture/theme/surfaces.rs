use std::collections::BTreeSet;

use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::{FamilyThemeMechanismKey, ResolvedDiagramTheme, ThemeTarget};
use crate::family::FamilyThemeOutcomes;
use crate::resources::{OperationWorkError, OperationWorkMeter};

mod styles;

pub(crate) use styles::{
    ArchitectureArrowSide, ArchitectureEdgeTerminal, ArchitectureEdgeTerminalEmission,
    ArchitectureGroupTerminal, ArchitectureGroupTerminalEmission,
    ArchitecturePaintTerminalEmission, ArchitectureServiceTerminal,
    ArchitectureServiceTerminalEmission, ArchitectureSurfaceRenderState,
    ArchitectureTextTerminalEmission,
};
use styles::{
    ArchitectureSurfaceOwnership, is_architecture_terminal_surface_target, mechanism_target,
};

const TEXT_COLOR_PATH: &str = "themeVariables.textColor";
const ARCH_EDGE_ARROW_COLOR_PATH: &str = "themeVariables.archEdgeArrowColor";
const ARCH_EDGE_COLOR_PATH: &str = "themeVariables.archEdgeColor";
const LINE_COLOR_PATH: &str = "themeVariables.lineColor";

/// Family-local plan for real Architecture service, SVG-text, and arrow terminals.
#[derive(Debug)]
pub(super) struct ArchitectureSurfaceThemePlan {
    theme: Option<ResolvedDiagramTheme>,
    title_keys: Box<[FamilyThemeMechanismKey]>,
    service_stroke_source_owned: bool,
    text_fill_source_owned: bool,
    arrow_fill_source_owned: bool,
}

impl ArchitectureSurfaceThemePlan {
    #[must_use]
    pub(super) fn baseline() -> Self {
        Self {
            theme: None,
            title_keys: Box::new([]),
            service_stroke_source_owned: false,
            text_fill_source_owned: false,
            arrow_fill_source_owned: false,
        }
    }

    #[must_use]
    pub(super) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
    ) -> Self {
        let title_keys = match theme
            .map(|theme_2| {
                theme_2
                    .family_mechanism_routes()
                    .iter()
                    .copied()
                    .filter(|route| mechanism_target(route.mechanism()) == Some(ThemeTarget::Title))
                    .map(|route| theme_2.family_mechanism_key(route))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
            }) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => ::std::vec::Vec::new(),
        }
            .into_boxed_slice();
        Self {
            theme: theme.cloned(),
            title_keys,
            service_stroke_source_owned: super::mermaid_owns_group_stroke(effective_config),
            text_fill_source_owned: effective_config
                .config_paint_overrides_typed_default(TEXT_COLOR_PATH),
            arrow_fill_source_owned: mermaid_owns_arrow_fill(effective_config),
        }
    }

    pub(super) fn prepare_styles(
        &self,
        services: impl IntoIterator<Item = ArchitectureServiceTerminal>,
        groups: impl IntoIterator<Item = ArchitectureGroupTerminal>,
        edges: impl IntoIterator<Item = ArchitectureEdgeTerminal>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Option<ArchitectureSurfaceRenderState>, OperationWorkError> {
        let theme = {
            let optional_theme = self.theme.as_ref();

            match optional_theme {
                Some(matched_theme) => matched_theme,
                ::core::option::Option::None => return Ok(None),
            }
        };

        if !theme.family_mechanism_routes().iter().any(|route| {
            mechanism_target(route.mechanism()).is_some_and(is_architecture_terminal_surface_target)
        }) {
            return Ok(None);
        }

        ArchitectureSurfaceRenderState::build(
            theme,
            ArchitectureSurfaceOwnership {
                service_stroke: self.service_stroke_source_owned,
                text_fill: self.text_fill_source_owned,
                arrow_fill: self.arrow_fill_source_owned,
            },
            services,
            groups,
            edges,
            work_meter,
        )
        .map(Some)
    }

    pub(super) fn finish_evidence(
        &self,
        evidence: &mut FamilyThemeOutcomes,
        emission: Option<&ArchitectureSurfaceRenderState>,
    ) {


        for key in &self.title_keys {
            evidence.mark_not_applicable(key.clone());
        }

        match emission {
            Some(emission_2) => {
                emission_2.apply_to_evidence(evidence);
            },
            ::core::option::Option::None => {},
        }
    }
}

#[must_use]
fn mermaid_owns_arrow_fill(config: &PresentationConfig) -> bool {
    if config.config_paint_overrides_typed_default(ARCH_EDGE_ARROW_COLOR_PATH) {
        return true;
    }

    if config.config_paint_overrides_typed_default(ARCH_EDGE_COLOR_PATH)
        && config.get_str(ARCH_EDGE_ARROW_COLOR_PATH) == config.get_str(ARCH_EDGE_COLOR_PATH)
    {
        return true;
    }

    config.config_paint_overrides_typed_default(LINE_COLOR_PATH)
        && config.get_str(ARCH_EDGE_COLOR_PATH) == config.get_str(LINE_COLOR_PATH)
        && config.get_str(ARCH_EDGE_ARROW_COLOR_PATH) == config.get_str(ARCH_EDGE_COLOR_PATH)
}
