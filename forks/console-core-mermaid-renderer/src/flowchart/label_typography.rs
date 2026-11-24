//! Static label typography shared by layout, source resolution, and terminal writers.

use std::borrow::Cow;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeRuleFacet, FontStack,
    ResolvedDiagramTheme, ResolvedStyleProperty, Specified, ThemeTarget, ThemeTypographyProperty,
    ThemeVariant,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use crate::text::{TextStyle, resolve_css_font_weight};

#[derive(Debug, Clone, Default)]
struct LabelTypography {
    font_stack: Option<FontStack>,
    font_size: Option<f64>,
    font_weight: Option<u16>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FlowchartLabelTypography {
    node: LabelTypography,
    edge: LabelTypography,
    invalid_config_weight: bool,
}

impl FlowchartLabelTypography {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        meter: Option<&OperationWorkMeter>,
    ) -> Result<Self, OperationWorkError> {
        let theme_2 = match theme {
            Some(matched_theme) => matched_theme,
            ::core::option::Option::None => return Ok(Self::default()),
        };

        let routes = theme_2.family_mechanism_routes();

        match meter {
            Some(meter_2) => {
                meter_2.charge(routes.len())?;
            },
            ::core::option::Option::None => {},
        }

        if !routes.iter().any(|route| {
            route.disposition() == FamilyThemeDisposition::TypedAdapter
                && matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::NodeLabel | ThemeTarget::EdgeLabel,
                        facet: FamilyThemeRuleFacet::Typography(
                            ThemeTypographyProperty::FontStack
                                | ThemeTypographyProperty::FontSize
                                | ThemeTypographyProperty::FontWeight
                        ),
                        ..
                    }
                )
        }) {
            return Ok(Self::default());
        }

        let typography = |target| -> Result<LabelTypography, OperationWorkError> {
            let style = match meter {
                Some(meter_3) => {
                    theme_2.style_with_work_meter(target, ThemeVariant::Default, None, meter_3)?
                }
                None => theme_2.style(target, ThemeVariant::Default, None),
            };
            let selected = |requested| {
                style.winner_rule_properties().any(|(property, origin)| {
                    property == ResolvedStyleProperty::Typography(requested)
                        && theme_2.rule_facet_disposition(
                            origin.rule_index(),
                            FamilyThemeRuleFacet::Typography(requested),
                        ) == Some(FamilyThemeDisposition::TypedAdapter)
                })
            };
            let patch = style.typography_resolution().patch();


            let prepared_fonts = matches!(
                theme_2.family_id(),
                crate::DiagramFamilyId::FLOWCHART | crate::DiagramFamilyId::AGENTFLOW
            );
            Ok(LabelTypography {
                font_stack: (prepared_fonts
                    && selected(ThemeTypographyProperty::FontStack)
                    && matches!(patch.font_stack, Specified::Value(_)))
                .then(|| style.typography().font_stack().clone()),
                font_size: (prepared_fonts
                    && selected(ThemeTypographyProperty::FontSize)
                    && matches!(patch.font_size_px, Specified::Value(_)))
                .then(|| f64::from(style.typography().font_size_px())),
                font_weight: selected(ThemeTypographyProperty::FontWeight)
                    .then(|| style.typography().font_weight()),
            })
        };
        Ok(Self {
            node: { let checked_value = typography(ThemeTarget::NodeLabel)?; checked_value },
            edge: { let checked_value_2 = typography(ThemeTarget::EdgeLabel)?; checked_value_2 },
            invalid_config_weight: false,
        })
    }

    #[must_use]
    pub(crate) fn with_config(
        mut self,
        ownership: super::FlowchartTypographyConfigOwnership,
    ) -> Self {
        for style in [&mut self.node, &mut self.edge] {
            if ownership.font_stack {
                style.font_stack = None;
            }

            if ownership.font_size {
                style.font_size = None;
            }

            match ownership.font_weight {
                Some(weight) => {
                    style.font_weight = style.font_weight.map(|_| weight);
                },
                ::core::option::Option::None => {},
            }
        }

        self.invalid_config_weight = ownership.invalid_font_weight;
        self
    }

    #[must_use]
    pub(crate) fn config_is_verified(&self) -> bool {
        !self.invalid_config_weight
    }

    #[must_use]
    fn style(&self, target: ThemeTarget) -> Option<&LabelTypography> {
        match target {
            ThemeTarget::NodeLabel => Some(&self.node),
            ThemeTarget::EdgeLabel => Some(&self.edge),
            _ => None,
        }
    }

    #[must_use]
    pub(crate) fn get(&self, target: ThemeTarget) -> Option<u16> {
        let checked_value = self.style(target)?;
        checked_value.font_weight
    }

    #[must_use]
    pub(crate) fn has_fonts(&self, target: ThemeTarget) -> bool {
        self.style(target)
            .is_some_and(|style| style.font_stack.is_some() || style.font_size.is_some())
    }

    #[must_use]
    pub(crate) fn requested(&self, target: ThemeTarget) -> bool {
        self.has_fonts(target) || self.get(target).is_some()
    }

    #[must_use]
    pub(crate) fn parent_style<'a>(
        &self,
        target: ThemeTarget,
        parent: &'a TextStyle,
    ) -> Cow<'a, TextStyle> {
        self.parent_style_with_source(target, parent, false, false)
    }

    #[must_use]
    pub(crate) fn source_parent_style<'a>(
        &self,
        target: ThemeTarget,
        parent: &'a TextStyle,
        class_defs: &indexmap::IndexMap<String, Vec<String>>,
        classes: &[String],
        inline_styles: &[String],
        node_defaults: bool,
    ) -> Cow<'a, TextStyle> {
        if !self.has_fonts(target) {
            return self.parent_style(target, parent);
        }

        let [stack, size] = super::style::flowchart_declared_font_facets(
            class_defs,
            classes,
            inline_styles,
            node_defaults,
        );
        self.parent_style_with_source(target, parent, stack, size)
    }

    #[must_use]
    pub(crate) fn node_source_parent_style<'a>(
        &self,
        parent: &'a TextStyle,
        class_defs: &indexmap::IndexMap<String, Vec<String>>,
        source: (&[String], &[String], bool),
        structural: &super::FlowchartStructuralLabelTypography,
    ) -> Cow<'a, TextStyle> {
        let [stack, size] =
            super::style::flowchart_declared_font_facets(class_defs, source.0, source.1, source.2);
        let parent_2 = self.parent_style_with_source(
            ThemeTarget::NodeLabel,
            parent,
            stack || !structural.font_stack_status().is_absent(),
            size || !structural.font_size_status().is_absent(),
        );
        structural.apply_to_parent(parent_2)
    }

    #[must_use]
    pub(crate) fn parent_style_with_source<'a>(
        &self,
        target: ThemeTarget,
        parent: &'a TextStyle,
        source_font_stack: bool,
        source_font_size: bool,
    ) -> Cow<'a, TextStyle> {
        let style = {
            let optional_style = self.style(target);

            match optional_style {
                Some(matched_style) => matched_style,
                ::core::option::Option::None => return Cow::Borrowed(parent),
            }
        };

        if !self.requested(target) {
            return Cow::Borrowed(parent);
        }



        let mut parent_2 = parent.clone();

        if !source_font_stack && let Some(stack) = &style.font_stack {
            parent_2.font_family = Some(stack.as_css());
        }

        if !source_font_size && let Some(size) = style.font_size {
            parent_2.font_size = size;
        }

        match style.font_weight {
            Some(weight) => {
                parent_2.font_weight = Some(weight.to_string());
            },
            ::core::option::Option::None => {},
        }

        Cow::Owned(parent_2)
    }

    #[must_use]
    pub(crate) fn apply_cow<'a>(
        &self,
        target: ThemeTarget,
        mut source: Cow<'a, TextStyle>,
    ) -> Cow<'a, TextStyle> {
        let base = {
            let optional_base = self.get(target);

            match optional_base {
                Some(matched_base) => matched_base,
                ::core::option::Option::None => return source,
            }
        };

        let weight = match source.font_weight.as_deref() {
            Some(value) => match resolve_css_font_weight(value, base) {
                Some(weight) => weight,

                None => return source,
            },
            None => base,
        };
        source.to_mut().font_weight = Some(weight.to_string());
        source
    }

    pub(crate) fn append_style(
        &self,
        target: ThemeTarget,
        effective: &TextStyle,
        source_font_stack: super::FlowchartSourceFacetStatus,
        source_font_size: super::FlowchartSourceFacetStatus,
        style: &mut String,
    ) {
        let selected = {
            let optional_selected = self.style(target);

            match optional_selected {
                Some(matched_selected) => matched_selected,
                ::core::option::Option::None => return,
            }
        };

        use std::fmt::Write as _;

        if selected.font_stack.is_some()
            && source_font_stack.is_absent()
            && let Some(stack) = &effective.font_family
        {
            let _ = write!(style, ";font-family:{stack} !important;");
        }

        if selected.font_size.is_some() && source_font_size.is_absent() {
            let _ = write!(style, ";font-size:{}px !important;", effective.font_size);
        }

        if selected.font_weight.is_some()
            && let Some(weight) = effective
                .font_weight
                .as_deref()
                .and_then(|s| match s.parse::<u16>() { ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_), ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None })
                .filter(|weight| (1..=1000).contains(weight))
        {
            let _ = write!(style, ";font-weight:{weight} !important;");
        }
    }

    #[must_use]
    pub(crate) fn apply<'a>(
        &self,
        target: ThemeTarget,
        source: &'a TextStyle,
    ) -> Cow<'a, TextStyle> {
        self.apply_cow(target, Cow::Borrowed(source))
    }
}
