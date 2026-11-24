use crate::{MermaidConfig, MermaidThemeId, OperationControl, OperationControlResult, family};
use serde_json::Value;
use std::borrow::Cow;

pub(super) const APPEARANCE_KEYS: [&str; 3] = ["theme", "look", "layout"];

#[derive(Clone, Copy)]
pub(super) enum AppearanceOrigin {
    Source,
    Initialize,
    Compatibility,
    Default,
}

#[must_use]
pub(super) fn select_appearance_value<'a>(
    section: &str,
    key: &str,
    source: &'a MermaidConfig,
    initialize: &'a MermaidConfig,
    compatibility: Option<&'a MermaidConfig>,
    defaults: &'a MermaidConfig,
) -> Option<(Cow<'a, Value>, AppearanceOrigin)> {
    [
        (source, AppearanceOrigin::Source),
        (initialize, AppearanceOrigin::Initialize),
    ]
    .into_iter()
    .find_map(|(layer, origin)| {
        read_appearance(layer, section, key).map(|value| (Cow::Borrowed(value), origin))
    })
    .or_else(|| {
        compatibility
            .and_then(|layer| read_appearance(layer, section, key))
            .map(|value| (Cow::Borrowed(value), AppearanceOrigin::Compatibility))
    })

    .or_else(|| {
        (section == "mindmap" && key == "layout").then(|| {
            (
                Cow::Owned(Value::String("cose-bilkent".to_string())),
                AppearanceOrigin::Default,
            )
        })
    })
    .or_else(|| {
        read_appearance(defaults, section, key)
            .map(|value| (Cow::Borrowed(value), AppearanceOrigin::Default))
    })
}

/// Selects the identifiers used by semantic consumers without evaluating a theme.
pub(crate) fn resolve_semantic_appearance(
    diagram_type: &str,
    source: &MermaidConfig,
    initialize: &MermaidConfig,
    defaults: &MermaidConfig,
    effective: &mut MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<()> {
    let section = family::config_namespace_for_diagram_type(diagram_type).unwrap_or(diagram_type);

    for key in APPEARANCE_KEYS {
        control.checkpoint()?;

        let value = {
            let optional_value = select_appearance_value(section, key, source, initialize, None, defaults);

            match optional_value {
                Some((matched_value, _)) => matched_value,
                ::core::option::Option::None => continue,
            }
        };

        let value_2 = value.as_ref();
        let section_has_key = effective
            .as_value()
            .get(section)
            .and_then(|section_2| section_2.get(key))
            .is_some();

        if effective.as_value().get(key) != Some(value_2) {
            effective.set_value(
                key,
                { let checked_value = crate::json_value::clone_value_nonrecursive_with_control(value_2, control)?; checked_value },
            );
        }

        if section_has_key && effective.as_value()[section].get(key) != Some(value_2) {
            effective.set_value(
                &format!("{section}.{key}"),
                { let checked_value_2 = crate::json_value::clone_value_nonrecursive_with_control(value_2, control)?; checked_value_2 },
            );
        }
    }

    control.checkpoint()
}

#[must_use]
fn read_appearance<'a>(layer: &'a MermaidConfig, section: &str, key: &str) -> Option<&'a Value> {
    let root = layer.as_value();
    [
        root.get(section).and_then(|section_2| section_2.get(key)),
        root.get(key),
    ]
    .into_iter()
    .flatten()
    .find(|value| match key {
        "theme" => value
            .as_str()
            .is_some_and(|name| name == "null" || MermaidThemeId::NAMES.contains(&name)),
        "look" => value
            .as_str()
            .is_some_and(|look| matches!(look, "classic" | "handDrawn" | "neo")),

        "layout" => true,
        _ => false,
    })
}
