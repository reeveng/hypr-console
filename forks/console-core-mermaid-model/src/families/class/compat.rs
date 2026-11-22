//! Pure Mermaid-compatible JSON projection of declared family data.

use super::ClassDiagram;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};

pub fn project(
    model: &ClassDiagram,
    meta: &CompatJsonMetadata<'_>,
) -> Result<serde_json::Value, CompatProjectionError> {
    let mut value =
        serde_json::to_value(model).expect("Class typed model must remain JSON-serializable");

    match value
        .get_mut("relations")
        .and_then(serde_json::Value::as_array_mut) {
        Some(relations) => {
            for relation in relations {
                match relation.as_object_mut() {
                    Some(relation) => {
                        for field in ["relationTitle1", "relationTitle2"] {
                            match {
                                ::core::primitive::bool::then_some(relation.get(field).is_some_and(serde_json::Value::is_null), ())
                            } {
                                ::core::option::Option::Some(()) => {
                                    relation.insert(
                                        field.to_string(),
                                        serde_json::Value::String("none".to_string()),
                                    );
                                },
                                ::core::option::Option::None => {},
                            }
                        }
                    },
                    ::core::option::Option::None => {},
                }
            }
        },
        ::core::option::Option::None => {},
    }

    let object = value
        .as_object_mut()
        .expect("Class typed model must serialize to a JSON object");
    object.remove("namespaceFacadeAliases");
    object.insert(
        "type".to_string(),
        serde_json::Value::String(meta.diagram_type.to_string()),
    );
    Ok(value)
}
