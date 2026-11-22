//! Pure Mermaid-compatible JSON projection of declared family data.

use super::ErrorDiagramModel;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use serde_json::{Value, json};

pub fn project(
    model: &ErrorDiagramModel,
    _meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let mut output = json!({ "type": model.diagram_type });

    match &model.error_message {
        Some(message) => {
            output["errorMessage"] = json!(message);
        },
        ::core::option::Option::None => {},
    }

    Ok(output)
}
