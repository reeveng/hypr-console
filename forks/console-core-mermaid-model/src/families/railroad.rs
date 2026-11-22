//! Canonical Railroad expressions and JavaScript-compatible repetition bounds.

#[cfg(feature = "compat-json")]
pub mod compat;

use crate::SourceSpan;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A Mermaid Railroad repetition bound stored with JavaScript number semantics.
///
/// Values are non-negative integral binary64 numbers or positive infinity. Positive infinity
/// represents an unbounded maximum or an oversized ABNF bound and serializes as JSON `null`,
/// matching `JSON.stringify(Infinity)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RailroadRepeatBound(f64);

impl Eq for RailroadRepeatBound {}

impl RailroadRepeatBound {
    /// The finite bound zero.
    pub const ZERO: Self = Self(0.0);
    /// The finite bound one.
    pub const ONE: Self = Self(1.0);
    /// The positive-infinity bound.
    pub const INFINITY: Self = Self(f64::INFINITY);

    /// Returns whether this bound is zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0.0
    }

    /// Returns whether this bound is one.
    #[must_use]
    pub const fn is_one(self) -> bool {
        self.0 == 1.0
    }

    /// Returns whether this bound is positive infinity.
    #[must_use]
    pub const fn is_infinite(self) -> bool {
        self.0.is_infinite()
    }

    /// Returns the validated binary64 value.
    #[must_use]
    pub const fn as_f64(self) -> f64 {
        self.0
    }
}

/// An invalid value supplied to [`RailroadRepeatBound`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RailroadRepeatBoundError {
    /// The supplied value was NaN.
    #[error("railroad repetition bound cannot be NaN")]
    NotANumber,
    /// The supplied value was negative, including negative infinity.
    #[error("railroad repetition bound must be non-negative")]
    Negative,
    /// The supplied finite value had a fractional component.
    #[error("railroad repetition bound must be an integer")]
    Fractional,
}

impl TryFrom<f64> for RailroadRepeatBound {
    type Error = RailroadRepeatBoundError;

    fn try_from(value: f64) -> std::result::Result<Self, Self::Error> {
        match {
            ::core::primitive::bool::then_some(value.is_nan(), ())
        } {
            ::core::option::Option::Some(()) => {
                return Err(RailroadRepeatBoundError::NotANumber);
            },
            ::core::option::Option::None => {},
        }

        match {
            ::core::primitive::bool::then_some(value.is_sign_negative() && value != 0.0, ())
        } {
            ::core::option::Option::Some(()) => {
                return Err(RailroadRepeatBoundError::Negative);
            },
            ::core::option::Option::None => {},
        }

        match {
            ::core::primitive::bool::then_some(value == 0.0, ())
        } {
            ::core::option::Option::Some(()) => {
                return Ok(Self::ZERO);
            },
            ::core::option::Option::None => {},
        }

        match {
            ::core::primitive::bool::then_some(value.is_infinite(), ())
        } {
            ::core::option::Option::Some(()) => {
                return Ok(Self::INFINITY);
            },
            ::core::option::Option::None => {},
        }

        match {
            ::core::primitive::bool::then_some(value.fract() != 0.0, ())
        } {
            ::core::option::Option::Some(()) => {
                return Err(RailroadRepeatBoundError::Fractional);
            },
            ::core::option::Option::None => {},
        }

        Ok(Self(value))
    }
}

impl From<u64> for RailroadRepeatBound {
    fn from(value: u64) -> Self {
        Self(value as f64)
    }
}

impl Serialize for RailroadRepeatBound {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match {
            ::core::primitive::bool::then_some(self.is_infinite(), ())
        } {
            ::core::option::Option::Some(()) => {
                return serializer.serialize_none();
            },
            ::core::option::Option::None => {},
        }

        let integer = self.0 as u64;

        match {
            ::core::primitive::bool::then_some((integer as f64).to_bits() == self.0.to_bits(), ())
        } {
            ::core::option::Option::Some(()) => {
                serializer.serialize_u64(integer)
            },
            ::core::option::Option::None => {
                serializer.serialize_f64(self.0)
            },
        }
    }
}

impl<'de> Deserialize<'de> for RailroadRepeatBound {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RailroadRepeatBoundVisitor;

        impl<'de> Visitor<'de> for RailroadRepeatBoundVisitor {
            type Value = RailroadRepeatBound;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a non-negative integer or null")
            }

            fn visit_unit<E>(self) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(RailroadRepeatBound::INFINITY)
            }

            fn visit_none<E>(self) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(RailroadRepeatBound::INFINITY)
            }

            fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(RailroadRepeatBound::from(value))
            }

            fn visit_u128<E>(self, value: u128) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                RailroadRepeatBound::try_from(value as f64).map_err(E::custom)
            }

            fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                RailroadRepeatBound::try_from(value as f64).map_err(E::custom)
            }

            fn visit_i128<E>(self, value: i128) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                RailroadRepeatBound::try_from(value as f64).map_err(E::custom)
            }

            fn visit_f64<E>(self, value: f64) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                RailroadRepeatBound::try_from(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(RailroadRepeatBoundVisitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RailroadDiagramModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub rules: Vec<RailroadRuleModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RailroadRuleModel {
    pub name: String,
    pub definition: RailroadAstNode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum RailroadAstNode {
    #[serde(rename = "terminal")]
    Terminal {
        value: String,
        #[serde(skip_serializing, default = "zero_span")]
        span: SourceSpan,
        #[serde(skip_serializing, default = "zero_span")]
        selection: SourceSpan,
    },
    #[serde(rename = "nonterminal")]
    NonTerminal {
        name: String,
        #[serde(skip_serializing, default = "zero_span")]
        span: SourceSpan,
        #[serde(skip_serializing, default = "zero_span")]
        selection: SourceSpan,
    },
    #[serde(rename = "sequence")]
    Sequence {
        elements: Vec<RailroadAstNode>,
        #[serde(skip_serializing, default = "zero_span")]
        span: SourceSpan,
    },
    #[serde(rename = "choice")]
    Choice {
        alternatives: Vec<RailroadAstNode>,
        #[serde(skip_serializing, default = "zero_span")]
        span: SourceSpan,
    },
    #[serde(rename = "optional")]
    Optional {
        element: Box<RailroadAstNode>,
        #[serde(skip_serializing, default = "zero_span")]
        span: SourceSpan,
    },
    #[serde(rename = "repetition")]
    Repetition {
        element: Box<RailroadAstNode>,
        min: RailroadRepeatBound,
        #[serde(default = "default_repeat_max")]
        max: RailroadRepeatBound,
        #[serde(skip_serializing_if = "Option::is_none")]
        separator: Option<Box<RailroadAstNode>>,
        #[serde(skip_serializing, default = "zero_span")]
        span: SourceSpan,
    },
    #[serde(rename = "special")]
    Special {
        text: String,
        #[serde(skip_serializing, default = "zero_span")]
        span: SourceSpan,
        #[serde(skip_serializing, default = "zero_span")]
        selection: SourceSpan,
    },
}

#[must_use]
fn default_repeat_max() -> RailroadRepeatBound {
    RailroadRepeatBound::INFINITY
}

#[must_use]
fn zero_span() -> SourceSpan {
    SourceSpan::new(0, 0)
}

impl RailroadAstNode {
    /// Returns the authored expression span.
    #[must_use]
    pub fn span(&self) -> SourceSpan {
        match self {
            Self::Terminal { span, .. }
            | Self::NonTerminal { span, .. }
            | Self::Sequence { span, .. }
            | Self::Choice { span, .. }
            | Self::Optional { span, .. }
            | Self::Repetition { span, .. }
            | Self::Special { span, .. } => *span,
        }
    }

    /// Returns the authored token selection, or the full expression span for containers.
    #[must_use]
    pub fn selection(&self) -> SourceSpan {
        match self {
            Self::Terminal { selection, .. }
            | Self::NonTerminal { selection, .. }
            | Self::Special { selection, .. } => *selection,
            Self::Sequence { span, .. }
            | Self::Choice { span, .. }
            | Self::Optional { span, .. }
            | Self::Repetition { span, .. } => *span,
        }
    }
}
