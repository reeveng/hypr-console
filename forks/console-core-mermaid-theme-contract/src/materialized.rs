use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use crate::canonical_json::CanonicalJsonError;
use crate::finite::ContainsNonFiniteNumber;
use crate::spec::DiagramThemeSpecWireV1;
use crate::version::THEME_CONTRACT_V1;

const MATERIALIZED_THEME_SCHEMA_VERSION_V1: u32 = 1;

/// The closed version 1 success envelope produced by theme materialization.
///
/// This wire carries the resolved contract versions, complete theme spec, and optional authored
/// reference locations for editing. Reference locations are not execution or capability evidence;
/// diagnostics and render admission belong to their respective authorities.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaterializedThemeWireV1 {
    schema_version: u32,
    authoring_schema_version: u32,
    expansion_version: u32,
    spec_schema_version: u32,
    spec: DiagramThemeSpecWireV1,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    references: Vec<crate::ThemeDesignReferenceUse>,
}

impl MaterializedThemeWireV1 {
    /// Creates a materialized success envelope using the supported version 1 contract tuple.
    pub fn try_new(spec: DiagramThemeSpecWireV1) -> Result<Self, CanonicalJsonError> {
        match {
            ::core::primitive::bool::then_some(spec.contains_non_finite_number(), ())
        } {
            ::core::option::Option::Some(()) => {
                return Err(CanonicalJsonError::invalid_input(
                    "materialized theme spec contains a non-finite number",
                ));
            },
            ::core::option::Option::None => {},
        }

        Ok(Self {
            schema_version: MATERIALIZED_THEME_SCHEMA_VERSION_V1,
            authoring_schema_version: THEME_CONTRACT_V1.authoring_schema_version(),
            expansion_version: THEME_CONTRACT_V1.expansion_version(),
            spec_schema_version: THEME_CONTRACT_V1.spec_schema_version(),
            spec,
            references: Vec::new(),
        })
    }

    /// Returns the materialized success-envelope schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the authoring schema version used to produce the complete spec.
    #[must_use]
    pub const fn authoring_schema_version(&self) -> u32 {
        self.authoring_schema_version
    }

    /// Returns the deterministic expansion-table version used to produce the complete spec.
    #[must_use]
    pub const fn expansion_version(&self) -> u32 {
        self.expansion_version
    }

    /// Returns the complete-spec schema version carried by this envelope.
    #[must_use]
    pub const fn spec_schema_version(&self) -> u32 {
        self.spec_schema_version
    }

    /// Returns the materialized complete theme spec.
    #[must_use]
    pub const fn spec(&self) -> &DiagramThemeSpecWireV1 {
        &self.spec
    }

    /// Consumes the envelope and returns the materialized complete theme spec.
    #[must_use]
    pub fn into_spec(self) -> DiagramThemeSpecWireV1 {
        self.spec
    }

    /// Attaches declared authoring bindings; these are not execution or capability evidence.
    #[must_use]
    pub fn with_references(mut self, references: Vec<crate::ThemeDesignReferenceUse>) -> Self {
        self.references = references;
        self
    }

    /// Returns declared binding locations and consumer-owned input domains.
    #[must_use]
    pub fn references(&self) -> &[crate::ThemeDesignReferenceUse] {
        &self.references
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterializedThemeWireV1Decode {
    schema_version: u32,
    authoring_schema_version: u32,
    expansion_version: u32,
    spec_schema_version: u32,
    spec: DiagramThemeSpecWireV1,
    #[serde(default)]
    references: Vec<crate::ThemeDesignReferenceUse>,
}

impl<'de> Deserialize<'de> for MaterializedThemeWireV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let decoded = MaterializedThemeWireV1Decode::deserialize(deserializer)?;

        match {
            ::core::primitive::bool::then_some(decoded.schema_version != MATERIALIZED_THEME_SCHEMA_VERSION_V1, ())
        } {
            ::core::option::Option::Some(()) => {
                return Err(D::Error::custom(format_args!(
                    "MaterializedThemeWireV1 requires schema version {MATERIALIZED_THEME_SCHEMA_VERSION_V1}"
                )));
            },
            ::core::option::Option::None => {},
        }

        match {
            ::core::primitive::bool::then_some(decoded.authoring_schema_version != THEME_CONTRACT_V1.authoring_schema_version()
            || decoded.expansion_version != THEME_CONTRACT_V1.expansion_version()
            || decoded.spec_schema_version != THEME_CONTRACT_V1.spec_schema_version(), ())
        } {
            ::core::option::Option::Some(()) => {
                return Err(D::Error::custom(format_args!(
                    "MaterializedThemeWireV1 requires contract version tuple ({}, {}, {})",
                    THEME_CONTRACT_V1.authoring_schema_version(),
                    THEME_CONTRACT_V1.expansion_version(),
                    THEME_CONTRACT_V1.spec_schema_version()
                )));
            },
            ::core::option::Option::None => {},
        }

        Self::try_new(decoded.spec)
            .map(|value| value.with_references(decoded.references))
            .map_err(D::Error::custom)
    }
}
