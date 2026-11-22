//! Pure Mermaid-compatible JSON projection of declared family data.

use super::{FlowEdge, FlowchartModel};
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use crate::warnings::{FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID, legacy_warning_messages};
use serde_json::{Value, json};

pub fn project(
    model: &FlowchartModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    let step_count = model.step_count();
    let flowchart_config = meta.config.as_value().get("flowchart");
    let step_duration = flowchart_config
        .and_then(|config| config.get("stepDuration"))
        .and_then(Value::as_f64)
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .unwrap_or(1.0);
    let show_step_number = flowchart_config
        .and_then(|config| config.get("stepNumbers"))
        .and_then(Value::as_bool)
        == Some(true);
    let mut value =
        serde_json::to_value(model).expect("Flowchart typed model must remain JSON-serializable");
    let root = value.as_object_mut().ok_or_else(|| {
        CompatProjectionError::invalid_model(
            meta.diagram_type.to_string(),
            "flowchart typed model did not serialize to an object".to_string(),
        )
    })?;

    match root.get_mut("edges").and_then(Value::as_array_mut) {
        Some(edges) => {
            for edge in edges {
                match edge.as_object_mut() {
                    Some(edge) => {
                        edge.remove("startMarker");
                        edge.remove("endMarker");
                        edge.remove("strokeKind");
                        edge.remove("visibility");

                        if let Some(step) = edge
                            .get("step")
                            .and_then(Value::as_f64)
                            .filter(|step| FlowEdge::is_valid_step(*step))
                        {
                            edge.insert("step".to_string(), positive_number(step));
                            edge.insert("stepCount".to_string(), positive_number(step_count));
                            edge.insert("stepDuration".to_string(), positive_number(step_duration));

                            match ::core::primitive::bool::then_some(show_step_number, ()) {
                                ::core::option::Option::Some(()) => {
                                    edge.insert("showStepNumber".to_string(), Value::Bool(true));
                                },
                                ::core::option::Option::None => {},
                            }
                        } else {
                            edge.remove("step");
                        }
                    },
                    ::core::option::Option::None => {},
                }
            }
        },
        ::core::option::Option::None => {},
    }

    match root.remove("droppedSubGraphMembers") {
        Some(dropped) => {
            root.insert(
                "other".to_string(),
                json!({"droppedSubGraphMembers": dropped}),
            );
        },
        ::core::option::Option::None => {},
    }

    match root.get_mut("nodes").and_then(Value::as_array_mut) {
        Some(nodes) => {
            for node in nodes {
                match node.as_object_mut() {
                    Some(node) => {
                        node.remove("provenance");
                    },
                    ::core::option::Option::None => {},
                }
            }
        },
        ::core::option::Option::None => {},
    }

    root.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_string()),
    );

    match {
        ::core::primitive::bool::then_some(model
        .warning_facts
        .iter()
        .any(|fact| fact.rule_id == FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID), ())
    } {
        ::core::option::Option::Some(()) => {
            root.insert("direction".to_string(), Value::Null);
        },
        ::core::option::Option::None => {},
    }

    match {
        ::core::primitive::bool::then_some(!model.warning_facts.is_empty(), ())
    } {
        ::core::option::Option::Some(()) => {
            root.insert(
                "warnings".to_string(),
                json!(legacy_warning_messages(&model.warning_facts)),
            );
        },
        ::core::option::Option::None => {},
    }

    Ok(value)
}

#[must_use]
fn positive_number(value: f64) -> Value {
    match {
        ::core::primitive::bool::then_some(value.fract() == 0.0 && value < u64::MAX as f64, ())
    } {
        ::core::option::Option::Some(()) => {
            json!(value as u64)
        },
        ::core::option::Option::None => {
            json!(value)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MermaidConfig;

    fn project_with_config(config: Value) -> Value {
        let model = serde_json::from_value(json!({
            "nodes": [],
            "edges": [
                {"id": "first", "from": "A", "to": "B", "length": 1, "step": 1},
                {"id": "second", "from": "B", "to": "C", "length": 1, "step": 4},
                {"id": "third", "from": "B", "to": "D", "length": 1, "step": 4},
                {"id": "plain", "from": "C", "to": "E", "length": 1, "animate": true}
            ]
        }))
        .unwrap();
        let config = MermaidConfig::from_value(config);
        project(
            &model,
            &CompatJsonMetadata {
                diagram_type: "flowchart-v2",
                config: &config,
                authored_config: &config,
                title: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn step_timing_is_derived_only_for_step_edges_and_uses_the_maximum_step() {
        let projected = project_with_config(json!({}));
        assert_eq!(projected["edges"][0]["step"], json!(1));
        for edge in &projected["edges"].as_array().unwrap()[..3] {
            assert_eq!(edge["stepCount"], json!(4));
            assert_eq!(edge["stepDuration"].as_f64(), Some(1.0));
            assert!(edge.get("showStepNumber").is_none());
        }
        let plain = &projected["edges"][3];
        for key in ["step", "stepCount", "stepDuration", "showStepNumber"] {
            assert!(plain.get(key).is_none(), "{key}");
        }
    }

    #[test]
    fn step_duration_requires_a_finite_positive_number_and_badges_require_true() {
        for (duration, expected) in [
            (json!(0.5), 0.5),
            (json!(0), 1.0),
            (json!(-1), 1.0),
            (json!("0.5"), 1.0),
            (json!(true), 1.0),
            (Value::Null, 1.0),
        ] {
            for badge in [
                json!(true),
                json!(false),
                json!("true"),
                json!(1),
                Value::Null,
            ] {
                let projected = project_with_config(json!({
                    "flowchart": {"stepDuration": duration, "stepNumbers": badge}
                }));
                let edge = &projected["edges"][0];
                assert_eq!(edge["stepDuration"].as_f64(), Some(expected));
                assert_eq!(
                    edge.get("showStepNumber").and_then(Value::as_bool),
                    (badge == json!(true)).then_some(true)
                );
                assert!(projected["edges"][3].get("showStepNumber").is_none());
            }
        }
    }

    #[test]
    fn dropped_members_project_to_other_without_changing_empty_model_json() {
        let mut model: FlowchartModel =
            serde_json::from_value(json!({"nodes": [], "edges": []})).unwrap();
        let config = MermaidConfig::empty_object();
        let meta = CompatJsonMetadata {
            diagram_type: "flowchart-v2",
            config: &config,
            authored_config: &config,
            title: None,
        };
        assert!(project(&model, &meta).unwrap().get("other").is_none());
        model
            .dropped_subgraph_members
            .push(super::super::FlowDroppedSubgraphMember {
                node: "A".to_string(),
                kept_by: "one".to_string(),
                dropped_from: "two".to_string(),
            });
        let projected = project(&model, &meta).unwrap();
        assert!(projected.get("droppedSubGraphMembers").is_none());
        assert_eq!(
            projected["other"],
            json!({
                "droppedSubGraphMembers": [{"node": "A", "keptBy": "one", "droppedFrom": "two"}]
            })
        );
    }
}
