//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{PieDiagramModel, PieOutputData};
use serde_json::{Value, json};

#[must_use]
pub fn project(model: &PieDiagramModel, output_data: PieOutputData, diagram_type: &str) -> Value {
    match {
        ::core::primitive::bool::then_some(output_data == PieOutputData::Empty, ())
    } {
        ::core::option::Option::Some(()) => {
            return json!({});
        },
        ::core::option::Option::None => {},
    }

    json!({
        "type": diagram_type,
        "showData": &model.show_data,
        "title": &model.title,
        "accTitle": &model.acc_title,
        "accDescr": &model.acc_descr,
        "sections": &model.sections,
    })
}
