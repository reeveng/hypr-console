use serde_json::Value;

use crate::resources::{InputResourcePolicy, ModelComplexity};
use crate::{OperationControl, OperationControlResult};

use super::PresentationConfigError;

/// Construction-local work, independent of a diagram's retained model complexity.
///
/// Validate borrowed data before allocating indexes or copying joined ownership paths. The
/// policy's existing capacities bound this operation without changing artifact accounting.
pub(super) struct ConfigConstructionBudget<'a> {
    policy: &'a InputResourcePolicy,
    control: &'a OperationControl,
    work: ModelComplexity,
}

impl<'a> ConfigConstructionBudget<'a> {
    #[must_use]
    pub(super) fn new(policy: &'a InputResourcePolicy, control: &'a OperationControl) -> Self {
        Self {
            policy,
            control,
            work: ModelComplexity::default(),
        }
    }

    pub(super) fn checkpoint(&self) -> OperationControlResult<()> {
        self.control.checkpoint()
    }

    pub(super) fn charge(
        &mut self,
        items: usize,
        text_bytes: usize,
        nesting_depth: usize,
    ) -> OperationControlResult<Result<(), PresentationConfigError>> {
        self.control.checkpoint()?;

        let Some(items) = self.work.items.checked_add(items) else {
            return Ok(Err(PresentationConfigError::SizeOverflow));
        };

        let Some(text_bytes) = self.work.text_bytes.checked_add(text_bytes) else {
            return Ok(Err(PresentationConfigError::SizeOverflow));
        };

        self.work = ModelComplexity::new(
            items,
            text_bytes,
            self.work.nesting_depth.max(nesting_depth),
        );
        Ok(self
            .policy
            .check_model_complexity(self.work)
            .map_err(PresentationConfigError::from))
    }

    pub(super) fn charge_json(
        &mut self,
        root: &Value,
    ) -> OperationControlResult<Result<(), PresentationConfigError>> {
        match { let checked_value = self.charge(1, 0, 0)?; checked_value } {
            Err(error) => {
                return Ok(Err(error));
            },
            ::core::result::Result::Ok(_) => {},
        }

        let mut pending = vec![JsonFrame::Value(root, 0)];

        while let Some(frame) = pending.pop() {
            self.control.checkpoint()?;
            let charged = match frame {
                JsonFrame::Value(value, depth) => match value {
                    Value::String(value) => self.charge(0, value.len(), 0)?,
                    Value::Array(values) => {
                        let charged = self.charge(values.len(), 0, depth)?;

                        match charged {
                            ::core::result::Result::Ok(_) => {
                                let Some(child_depth) = depth.checked_add(1) else {
                                    return Ok(Err(PresentationConfigError::SizeOverflow));
                                };

                                pending.push(JsonFrame::Array(values.iter(), child_depth));
                            },
                            ::core::result::Result::Err(_) => {},
                        }

                        charged
                    }
                    Value::Object(values) => {
                        let charged = self.charge(values.len(), 0, depth)?;

                        match charged {
                            ::core::result::Result::Ok(_) => {
                                let Some(child_depth) = depth.checked_add(1) else {
                                    return Ok(Err(PresentationConfigError::SizeOverflow));
                                };

                                pending.push(JsonFrame::Object(values.iter(), child_depth));
                            },
                            ::core::result::Result::Err(_) => {},
                        }

                        charged
                    }
                    Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
                },
                JsonFrame::Array(mut values, depth) => {
                    match values.next() {
                        Some(value) => {
                            pending.push(JsonFrame::Array(values, depth));
                            pending.push(JsonFrame::Value(value, depth));
                        },
                        ::core::option::Option::None => {},
                    }

                    Ok(())
                }
                JsonFrame::Object(mut values, depth) => {
                    match values.next() {
                        Some((key, value)) => {
                            let charged = self.charge(0, key.len(), 0)?;

                            match charged {
                                ::core::result::Result::Ok(_) => {
                                    pending.push(JsonFrame::Object(values, depth));
                                    pending.push(JsonFrame::Value(value, depth));
                                },
                                ::core::result::Result::Err(_) => {},
                            }

                            charged
                        },
                        ::core::option::Option::None => {
                            Ok(())
                        },
                    }
                }
            };

            match charged {
                Err(error) => {
                    return Ok(Err(error));
                },
                ::core::result::Result::Ok(_) => {},
            }
        }

        Ok(Ok(()))
    }
}

enum JsonFrame<'a> {
    Value(&'a Value, usize),
    Array(std::slice::Iter<'a, Value>, usize),
    Object(serde_json::map::Iter<'a>, usize),
}
