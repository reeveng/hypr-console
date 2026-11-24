//! Fixed Mindmap visual inputs prepared before layout and consumed by SVG emission.

use merman_model::presentation::PresentationConfig;

use super::theme::{
    MindmapNodeFillOwnership, MindmapNodeFillSource, MindmapNodePalettePlan,
    MindmapWriterThemeTokens,
};

#[derive(Debug)]
pub(crate) struct MindmapSectionCss {
    pub(crate) fill: String,
    pub(crate) inverse: String,
    pub(crate) label: String,
}

#[derive(Debug)]
pub(crate) struct MindmapCssBinding {
    pub(crate) common: crate::svg::PreparedCommonCss,
    pub(crate) sections: Vec<MindmapSectionCss>,
    pub(crate) neutral_label: String,
    pub(crate) label_background: Option<String>,
    pub(crate) label_text: Option<String>,
    pub(crate) theme: Box<str>,
    pub(crate) look: Box<str>,
    pub(crate) root_fill: String,
    pub(crate) root_label: String,
    pub(crate) tokens: MindmapWriterThemeTokens,
    pub(crate) stroke_width: String,
    pub(crate) drop_shadow: String,
    pub(crate) use_gradient: bool,
    pub(crate) gradient: Option<(String, String)>,
    pub(crate) shadow_flood: &'static str,
    pub(crate) cluster_background: Box<str>,
    pub(crate) cluster_border: Box<str>,
    pub(crate) node_fill_ownership: MindmapNodeFillOwnership,
    pub(crate) neo_fill_source: MindmapNodeFillSource,
}

impl MindmapCssBinding {
    #[must_use]
    pub(super) fn new(config: &PresentationConfig, plan: &MindmapNodePalettePlan) -> Self {
        let raw = config.as_value();
        let token = |key: &str, fallback: &str| {
            config
                .get_str(&format!("themeVariables.{key}"))
                .unwrap_or(fallback)
                .to_owned()
        };
        let theme = match config.get_str("theme") {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => "",
        };
        let use_gradient = match raw
            .pointer("/themeVariables/useGradient")
            .and_then(crate::config::json_bool) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => false,
        };
        let gradient = use_gradient
            .then(|| {
                Some((
                    config.get_str("themeVariables.gradientStart")?.to_owned(),
                    config.get_str("themeVariables.gradientStop")?.to_owned(),
                ))
            })
            .flatten();
        let root_fill = token("git0", "hsl(240, 100%, 46.2745098039%)");
        let root_label = token("gitBranchLabel0", "#ffffff");
        let tokens = plan.writer_theme_tokens(
            &token("mainBkg", &root_fill),
            &super::theme::mindmap_node_border_css(config),
        );
        const INVERSE: [&str; 12] = [
            "hsl(60, 100%, 86.2745098039%)",
            "hsl(240, 100%, 83.5294117647%)",
            "hsl(260, 100%, 86.2745098039%)",
            "hsl(90, 100%, 86.2745098039%)",
            "hsl(120, 100%, 86.2745098039%)",
            "hsl(150, 100%, 86.2745098039%)",
            "hsl(180, 100%, 86.2745098039%)",
            "hsl(210, 100%, 86.2745098039%)",
            "hsl(270, 100%, 86.2745098039%)",
            "hsl(330, 100%, 86.2745098039%)",
            "hsl(0, 100%, 86.2745098039%)",
            "hsl(30, 100%, 86.2745098039%)",
        ];
        let sections = (0..super::theme::mindmap_theme_color_limit(config))
            .map(|slot| MindmapSectionCss {
                fill: super::theme::mindmap_color_scale_css(config, slot),
                inverse: token(&format!("cScaleInv{slot}"), INVERSE[slot % INVERSE.len()]),
                label: token(
                    &format!("cScaleLabel{slot}"),
                    if slot == 0 || slot == 3 {
                        "#ffffff"
                    } else {
                        "black"
                    },
                ),
            })
            .collect();
        let theme_id = merman_model::config::MermaidThemeId::parse(theme).unwrap_or_default();
        Self {
            common: crate::svg::PreparedCommonCss::new(raw, Some(plan.font_family_css())),
            sections,
            neutral_label: token("cScaleLabel1", "black"),
            label_background: crate::config::config_string(
                raw,
                &["themeVariables", "mindmap", "labelBackgroundColor"],
            ),
            label_text: crate::config::config_string(
                raw,
                &["themeVariables", "mindmap", "labelTextColor"],
            ),
            theme: <::std::boxed::Box<::core::primitive::str> as ::core::convert::From<&::core::primitive::str>>::from(theme),
            look: <::std::boxed::Box<::core::primitive::str> as ::core::convert::From<&::core::primitive::str>>::from(crate::config::mermaid_config_diagram_look(config)
                .as_str()),
            root_fill,
            root_label,
            tokens,
            stroke_width: match crate::config::config_css_number_or_string(
                raw,
                &["themeVariables", "strokeWidth"],
            ) {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => <::std::string::String as ::core::convert::From<&::core::primitive::str>>::from("2"),
            },
            drop_shadow: match crate::config::config_css_number_or_string(
                raw,
                &["themeVariables", "dropShadow"],
            ) {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => <::std::string::String as ::core::convert::From<&::core::primitive::str>>::from("none"),
            },
            use_gradient,
            gradient,
            shadow_flood: if theme.contains("dark") {
                "#FFFFFF"
            } else {
                "#000000"
            },
            cluster_background: <::std::boxed::Box<::core::primitive::str> as ::core::convert::From<::std::string::String>>::from(token("clusterBkg", "#ffffde")),
            cluster_border: <::std::boxed::Box<::core::primitive::str> as ::core::convert::From<::std::string::String>>::from(token("clusterBorder", "#aaaa33")),
            node_fill_ownership: MindmapNodeFillOwnership::from_config(config),
            neo_fill_source: if use_gradient
                || matches!(
                    theme_id,
                    merman_model::config::MermaidThemeId::Redux
                        | merman_model::config::MermaidThemeId::ReduxDark
                        | merman_model::config::MermaidThemeId::Neutral
                ) {
                MindmapNodeFillSource::MainBackground
            } else {
                MindmapNodeFillSource::ColorScale
            },
        }
    }

    #[must_use]
    pub(crate) fn model_look<'a>(&'a self, model_look: &'a str) -> &'a str {
        let model_look_2 = model_look.trim();

        if model_look_2.is_empty() || model_look_2 == "default" {
            &self.look
        } else {
            model_look_2
        }
    }

    #[must_use]
    pub(super) fn label_fill_owned(
        &self,
        config: &PresentationConfig,
        node: &merman_model::families::mindmap::MindmapDiagramRenderNode,
    ) -> bool {
        let html = crate::config::config_effective_html_labels(config.as_value());
        let has_class = |class: &str| {
            node.css_classes
                .split_whitespace()
                .any(|value| value == class)
        };
        let root = has_class("section-root");
        let neo = self.model_look(&node.look) == "neo" && has_class("mindmap-node");


        if !html && !neo && !root && has_class("disabled") {
            return true;
        }

        if html && !neo && node.icon.is_some() && self.label_text.is_some() {
            return config
                .config_paint_overrides_typed_default("themeVariables.mindmap.labelTextColor");
        }

        let node_border_role = || {
            if config.get_str("themeVariables.nodeBorder").is_some() {
                "themeVariables.nodeBorder"
            } else {
                "themeVariables.gitBranchLabel0"
            }
        };
        let role = if (neo && matches!(self.theme.as_ref(), "redux" | "redux-dark"))
            || (root && html && self.theme.contains("redux"))
        {
            node_border_role().to_string()
        } else if root && !neo {
            <::std::string::String as ::core::convert::From<&::core::primitive::str>>::from("themeVariables.gitBranchLabel0")
        } else if neo && self.theme.as_ref() == "neutral" {
            <::std::string::String as ::core::convert::From<&::core::primitive::str>>::from("themeVariables.cScaleLabel1")
        } else {
            let slot = if root && neo {
                Some(0)
            } else {
                node.css_classes
                    .split_whitespace()
                    .filter_map(|class| {
                        let checked_value = class.strip_prefix("section-")?;
                        let section = match checked_value.parse::<i32>() { ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_), ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None }?;
                        let section_2 = if section >= super::theme::MINDMAP_SECTION_COUNT as i32 {
                            section % super::theme::MINDMAP_SECTION_COUNT as i32
                        } else {
                            section
                        };
                        match usize::try_from(section_2 + 1) { ::core::result::Result::Ok(value_) => ::core::option::Option::Some(value_), ::core::result::Result::Err(_outside_integer_range) => ::core::option::Option::None }
                            .filter(|slot| *slot < self.sections.len())
                    })
                    .max()
            };

            match slot.map(|slot_2| format!("themeVariables.cScaleLabel{slot}", slot = slot_2)) {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => <::std::string::String as ::core::convert::From<&::core::primitive::str>>::from("themeVariables.textColor"),
            }
        };
        config.config_paint_overrides_typed_default(&role)
    }
}
