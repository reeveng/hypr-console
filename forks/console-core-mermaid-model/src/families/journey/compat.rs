//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{JourneyDiagramModel, JourneyOutputData};
use serde_json::{Value, json};

#[must_use]
pub fn project(
    model: &JourneyDiagramModel,
    output_data: JourneyOutputData,
    diagram_type: &str,
) -> Value {
    match {
        ::core::primitive::bool::then_some(output_data == JourneyOutputData::Empty, ())
    } {
        ::core::option::Option::Some(()) => {
            return json!({});
        },
        ::core::option::Option::None => {},
    }

    let mut tasks =
        serde_json::to_value(&model.tasks).expect("Journey tasks must remain JSON-serializable");

    for task in tasks
        .as_array_mut()
        .expect("Journey tasks must serialize to an array")
    {
        match task.as_object_mut() {
            Some(task) => {
                task.remove("sectionIndex");
            },
            ::core::option::Option::None => {},
        }
    }

    json!({
        "type": diagram_type,
        "title": &model.title,
        "accTitle": &model.acc_title,
        "accDescr": &model.acc_descr,
        "sections": &model.sections,
        "tasks": tasks,
        "actors": &model.actors,
    })
}
