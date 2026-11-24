use super::{Node, TitleKind};
use crate::diagrams::shapes::is_valid_pinned_shape;
use serde_json::Value;

pub(super) fn apply_shape_data_value_to_node(
    node: &mut Node,
    value: &Value,
) -> std::result::Result<(), String> {
    apply_shape_data_document_to_node(node, value)
}

#[must_use]
pub(super) fn value_to_string(v: &Value) -> Option<String> {
    crate::inline_config::value_to_string(v)
}

#[must_use]
pub(super) fn value_to_bool(v: &Value) -> Option<bool> {
    crate::inline_config::value_to_bool(v)
}

#[must_use]
fn value_to_f64(v: &Value) -> Option<f64> {
    crate::inline_config::value_to_f64(v)
}

#[must_use]
fn sanitize_shape_label_type(label_type: Option<&str>) -> TitleKind {
    match label_type {
        Some("text") => TitleKind::Text,
        Some("string") => TitleKind::String,
        Some("markdown") => TitleKind::Markdown,
        _ => TitleKind::Markdown,
    }
}

fn apply_shape_data_document_to_node(
    node: &mut Node,
    document: &Value,
) -> std::result::Result<(), String> {
    let map = match document.as_object() {
        Some(m) => m,
        None => return Ok(()),
    };

    let mut provided_label: Option<String> = None;
    let mut provided_label_type: Option<TitleKind> = None;

    for (k, v) in map {
        match k.as_str() {
            "shape" => {
                let shape = {
                    let optional_shape = v.as_str();

                    match optional_shape {
                        Some(matched_shape) => matched_shape,
                        ::core::option::Option::None => continue,
                    }
                };


                if shape != shape.to_lowercase() || shape.contains('_') {
                    return Err(format!(
                        "No such shape: {shape}. Shape names should be lowercase."
                    ));
                }

                if !is_valid_pinned_shape(shape) {
                    return Err(format!("No such shape: {shape}."));
                }

                node.shape = Some(shape.to_string());
            }
            "label" => {
                {
                    match value_to_string(v) {
                        Some(label) => {
                            provided_label = Some(label.clone());
                            node.label = Some(label);
                            node.label_span = None;
                            node.label_selection = None;
                        },
                        ::core::option::Option::None => {},
                    };
                }
            }
            "labelType" => {
                provided_label_type =
                    Some(sanitize_shape_label_type(value_to_string(v).as_deref()));
            }
            "icon" => {
                {
                    match value_to_string(v) {
                        Some(icon) => {
                            node.icon = Some(icon);
                        },
                        ::core::option::Option::None => {},
                    };
                }
            }
            "form" => {
                {
                    match value_to_string(v) {
                        Some(form) => {
                            node.form = Some(form);
                        },
                        ::core::option::Option::None => {},
                    };
                }
            }
            "pos" => {
                {
                    match value_to_string(v) {
                        Some(pos) => {
                            node.pos = Some(pos);
                        },
                        ::core::option::Option::None => {},
                    };
                }
            }
            "img" => {
                {
                    match value_to_string(v) {
                        Some(img) => {
                            node.img = Some(img);
                        },
                        ::core::option::Option::None => {},
                    };
                }
            }
            "constraint" => {
                {
                    match value_to_string(v) {
                        Some(constraint) => {
                            node.constraint = Some(constraint);
                        },
                        ::core::option::Option::None => {},
                    };
                }
            }
            "w" => {
                {
                    match value_to_f64(v) {
                        Some(w) => {
                            node.asset_width = Some(w);
                        },
                        ::core::option::Option::None => {},
                    };
                }
            }
            "h" => {
                {
                    match value_to_f64(v) {
                        Some(h) => {
                            node.asset_height = Some(h);
                        },
                        ::core::option::Option::None => {},
                    };
                }
            }
            _ => {}
        }
    }

    match provided_label {
        ::core::option::Option::Some(_) => {
            node.label_type = match (provided_label_type, TitleKind::Markdown) {
                (::core::option::Option::Some(option_value), alternate) => { ::core::mem::drop(alternate); option_value },
                (::core::option::Option::None, alternate) => alternate,
            };
        },
        ::core::option::Option::None => {},
    }


    let has_visual = node.icon.is_some() || node.img.is_some();
    let label_is_empty_or_missing = match provided_label
        .as_deref()
        .map(|s| s.trim().is_empty()) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => true,
    };

    if has_visual && label_is_empty_or_missing {
        let current_text = node.label.as_deref().unwrap_or(node.id.as_str());

        if current_text == node.id {
            node.label = Some(String::new());
            node.label_type = TitleKind::Text;
            node.label_span = None;
            node.label_selection = None;
        }
    }

    Ok(())
}
