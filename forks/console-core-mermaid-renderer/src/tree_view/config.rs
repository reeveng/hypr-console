use crate::config::{config_bool, config_f64, config_string};
use serde_json::Value;
use std::collections::HashMap;

const DEFAULT_ROW_INDENT: f64 = 10.0;
const DEFAULT_PADDING_X: f64 = 5.0;
const DEFAULT_PADDING_Y: f64 = 5.0;
pub(super) const DEFAULT_LINE_THICKNESS: f64 = 1.0;
const DEFAULT_USE_MAX_WIDTH: bool = true;

pub(crate) struct TreeViewConfigView<'a> {
    tree_view_config: &'a Value,
}

impl<'a> TreeViewConfigView<'a> {
    #[must_use]
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        Self {
            tree_view_config: effective_config.get("treeView").unwrap_or(&Value::Null),
        }
    }

    #[must_use]
    pub(crate) fn layout_settings(
        &self,
        theme: &super::theme::TreeViewCssBinding,
    ) -> TreeViewLayoutSettings {
        TreeViewLayoutSettings {
            row_indent: match self
                .tree_view_f64("rowIndent") {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => DEFAULT_ROW_INDENT,
            }
                .max(0.0),
            padding_x: match self
                .tree_view_f64("paddingX") {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => DEFAULT_PADDING_X,
            }
                .max(0.0),
            padding_y: match self
                .tree_view_f64("paddingY") {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => DEFAULT_PADDING_Y,
            }
                .max(0.0),
            line_thickness: theme.line_thickness,
            use_max_width: match self
                .tree_view_bool("useMaxWidth") {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => DEFAULT_USE_MAX_WIDTH,
            },
            font_family: theme.font_family.clone(),
            label_font_size: theme.label_font_size,
            show_icons: match self.tree_view_bool("showIcons") {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => false,
            },
            default_icon_pack: match self.tree_view_string("defaultIconPack") {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => ::std::string::String::new(),
            },
            filename_icons: self.tree_view_string_map("filenameIcons"),
            extension_icons: self.tree_view_string_map("extensionIcons"),
        }
    }

    #[must_use]
    fn tree_view_bool(&self, key: &str) -> Option<bool> {
        config_bool(self.tree_view_config, &[key])
    }

    #[must_use]
    fn tree_view_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.tree_view_config, &[key])
    }

    #[must_use]
    fn tree_view_string(&self, key: &str) -> Option<String> {
        config_string(self.tree_view_config, &[key])
    }

    #[must_use]
    fn tree_view_string_map(&self, key: &str) -> HashMap<String, String> {
        self.tree_view_config
            .get(key)
            .and_then(Value::as_object)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|(key_2, value)| {
                        value.as_str().map(|value_2| (key_2.clone(), value_2.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TreeViewLayoutSettings {
    pub(crate) row_indent: f64,
    pub(crate) padding_x: f64,
    pub(crate) padding_y: f64,
    pub(crate) line_thickness: f64,
    pub(crate) use_max_width: bool,
    pub(crate) font_family: String,
    pub(crate) label_font_size: f64,
    pub(crate) show_icons: bool,
    pub(crate) default_icon_pack: String,
    pub(crate) filename_icons: HashMap<String, String>,
    pub(crate) extension_icons: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tree_view_layout_settings_preserve_defaults_and_theme_font_size() {
        let cfg = json!({});
        let binding = super::super::theme::TreeViewCssBinding::from_config(&cfg);
        let settings = TreeViewConfigView::new(&cfg).layout_settings(&binding);

        assert_eq!(settings.row_indent, DEFAULT_ROW_INDENT);
        assert_eq!(settings.padding_x, DEFAULT_PADDING_X);
        assert_eq!(settings.padding_y, DEFAULT_PADDING_Y);
        assert_eq!(settings.line_thickness, DEFAULT_LINE_THICKNESS);
        assert!(settings.use_max_width);
        assert_eq!(
            settings.font_family,
            "\"trebuchet ms\",verdana,arial,sans-serif"
        );
        assert_eq!(settings.label_font_size, 16.0);
        assert!(!settings.show_icons);
        assert_eq!(settings.default_icon_pack, "");
        assert!(settings.filename_icons.is_empty());
        assert!(settings.extension_icons.is_empty());
    }

    #[test]
    fn tree_view_layout_settings_project_configured_values() {
        let cfg = json!({
            "treeView": {
                "rowIndent": "12",
                "paddingX": 7,
                "paddingY": 8,
                "lineThickness": 2,
                "useMaxWidth": false,
                "showIcons": true,
                "defaultIconPack": "logos",
                "filenameIcons": { "Dockerfile": "docker" },
                "extensionIcons": { ".ts": "typescript" }
            },
            "themeVariables": {
                "treeView": {
                    "labelFontSize": "21px"
                }
            }
        });
        let binding = super::super::theme::TreeViewCssBinding::from_config(&cfg);
        let settings = TreeViewConfigView::new(&cfg).layout_settings(&binding);

        assert_eq!(settings.row_indent, 12.0);
        assert_eq!(settings.padding_x, 7.0);
        assert_eq!(settings.padding_y, 8.0);
        assert_eq!(settings.line_thickness, 2.0);
        assert!(!settings.use_max_width);
        assert_eq!(settings.label_font_size, 21.0);
        assert!(settings.show_icons);
        assert_eq!(settings.default_icon_pack, "logos");
        assert_eq!(
            settings
                .filename_icons
                .get("Dockerfile")
                .map(String::as_str),
            Some("docker")
        );
        assert_eq!(
            settings.extension_icons.get(".ts").map(String::as_str),
            Some("typescript")
        );
    }

    #[test]
    fn tree_view_layout_settings_clamp_negative_geometry() {
        let cfg = json!({
            "treeView": {
                "rowIndent": -12,
                "paddingX": -7,
                "paddingY": -8,
                "lineThickness": -2
            }
        });
        let binding = super::super::theme::TreeViewCssBinding::from_config(&cfg);
        let settings = TreeViewConfigView::new(&cfg).layout_settings(&binding);

        assert_eq!(settings.row_indent, 0.0);
        assert_eq!(settings.padding_x, 0.0);
        assert_eq!(settings.padding_y, 0.0);
        assert_eq!(settings.line_thickness, 0.0);
    }
}
