#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! # merman-theme-contract
//!
//! `merman-theme-contract` owns Merman's unreleased version 1 theme documents, typed authoring values,
//! materialized results, and canonical JSON. It has no parser, renderer, bindings, or host dependency.
//! The first-release schema remains 1 while its unreleased design evolves; there is no legacy decoder.
//!
//! ## Save and share a theme
//!
//! `ThemeRecipeV1` is the exchange envelope accepted by binding `options.theme`, CLI `--theme-file`,
//! and Rust `DiagramThemeCompiler::compile_recipe`. `design` retains editable values and bindings;
//! `complete_spec` carries a concrete specification for precise low-level authoring.
//!
//! ```json
//! {
//!   "schema_version": 1,
//!   "kind": "design",
//!   "design": {
//!     "tokens": {
//!       "brand": { "type": "color", "value": "#60a5fa", "label": "Brand", "group": "Colors" }
//!     },
//!     "styles": {
//!       "node": { "stroke": { "paint": { "kind": "solid", "color": { "ref": "brand" } } } }
//!     },
//!     "canvas": { "base": "#0f172a" }
//!   }
//! }
//! ```
//!
//! Each object has a closed wire shape. Unknown fields, mixed payload kinds, unknown versions, and
//! non-finite numbers are rejected. Omitted collections are empty; explicit `null` is permitted only
//! at clearable style fields. Direct Serde decoding is appropriate for trusted or externally bounded
//! input. Transport entry points must enforce byte and collection limits before decoding.
//!
//! ## Author typed values
//!
//! Direct values require no tokens. A token is an optional named value with a declared type and
//! editor metadata. Values can reference exact document-local IDs with `{ "ref": "brand" }`; dots
//! in an ID are literal, and types are never implicitly converted. Colors, numbers, strings, lengths,
//! font stacks, palettes, paints, strokes, text styles, effects, insets, and line heights are supported.
//!
//! ```rust
//! use merman_theme_contract::{ThemeDesignTokenV1, ThemeDesignTokenValueV1, ThemeDesignV1, ThemeValueV1};
//!
//! let mut design = ThemeDesignV1::default();
//! design.tokens.insert("brand".into(), ThemeDesignTokenV1 {
//!     value: ThemeDesignTokenValueV1::Color(ThemeValueV1::Literal("#60a5fa".into())),
//!     label: Some("Brand".into()),
//!     description: None,
//!     group: Some("Colors".into()),
//! });
//! let readable = serde_json::to_string_pretty(&design)?;
//! let restored: ThemeDesignV1 = serde_json::from_str(&readable)?;
//! assert_eq!(design.canonical_json_bytes()?, restored.canonical_json_bytes()?);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! `styles` contains semantic defaults keyed by target. Ordered `rules` provide family, variant, or
//! ordinal exceptions after defaults; `palettes` then supplies ordinal color cycles. Nested paints,
//! gradient stops, shadows, typography, and canvas layers can retain their own references. Editing a
//! shared value affects only explicit consumers; equal literal colors remain independent.
//!
//! The Rust renderer owns bounded reference resolution, cycle detection, consumer domains, and
//! compilation. This contract crate describes values without duplicating those semantics.
//!
//! ## Materialized results
//!
//! `MaterializedThemeWireV1` contains the concrete `spec` and resolved version tuple. Its optional
//! `references` list supplies authored locations and consumer domains for editor controls. These
//! locations are not execution evidence, portability claims, preset qualification, or render receipts.
//!
//! ```rust
//! use merman_theme_contract::{DiagramThemeSpecWireV1, MaterializedThemeWireV1};
//!
//! let result = MaterializedThemeWireV1::try_new(DiagramThemeSpecWireV1::default())?;
//! assert_eq!(result.schema_version(), 1);
//! assert_eq!(result.spec_schema_version(), 1);
//! assert!(result.references().is_empty());
//! # Ok::<(), merman_theme_contract::CanonicalJsonError>(())
//! ```
//!
//! ## Canonical JSON
//!
//! Canonical JSON uses RFC 8785 property ordering and number serialization. It is available only for
//! contract-owned typed values, without a public arbitrary-`Serialize` canonicalizer.
//!
//! ```rust
//! use merman_theme_contract::ThemeDesignV1;
//! assert_eq!(ThemeDesignV1::default().canonical_json_bytes()?, b"{}");
//! # Ok::<(), merman_theme_contract::CanonicalJsonError>(())
//! ```
//!
//! Saving a document does not embed host fonts or establish support for every diagram family and
//! output. Use renderer support and execution results for those decisions. Asset redistribution
//! requirements belong in the accompanying README and LICENSE files.
//!
//! This crate has no default Cargo features.

mod canonical_json;
mod design;
mod facet;
mod finite;
mod materialization_error;
mod materialized;
mod preset_catalog;
mod spec;
mod support;
mod theme_recipe;
mod version;
mod wire;

pub use canonical_json::{CanonicalJsonError, CanonicalJsonErrorKind};
pub use design::{
    ThemeDesignCanvasLayerV1, ThemeDesignCanvasV1, ThemeDesignEffectV1, ThemeDesignGradientStopV1,
    ThemeDesignInspection, ThemeDesignPaintObjectV1, ThemeDesignPaintV1, ThemeDesignReferenceUse,
    ThemeDesignRuleV1, ThemeDesignShadowV1, ThemeDesignStrokeV1, ThemeDesignStyleV1,
    ThemeDesignTextStyleV1, ThemeDesignTokenV1, ThemeDesignTokenValueV1, ThemeDesignTypographyV1,
    ThemeDesignV1, ThemeDesignValueDomain, ThemeReferenceV1, ThemeShadowCompositionV1,
    ThemeValueV1,
};
pub use facet::ThemeRuleFacetV1;
pub use materialization_error::{ThemeMaterializationDiagnosticV1, ThemeMaterializationErrorV1};
pub use materialized::MaterializedThemeWireV1;
pub use preset_catalog::{
    THEME_PRESET_CATALOG_SCHEMA_VERSION_V1, ThemePresetApplicationNoteV1,
    ThemePresetFamilyDesignV1, ThemePresetMetadataV1, ThemePresetQualifiedCellV1,
};
pub use spec::{
    DiagramThemeSpecWireV1, MermaidThemeCompatibilityWireV1, MermaidThemeValueWireV1,
    ThemeAssetsWireV1, ThemeCanvasLayerWireV1, ThemeCanvasSpecWireV1, ThemeEffectEntryWireV1,
    ThemeEffectPrimitiveWireV1, ThemeFontAliasWireV1, ThemeFontAssetWireV1,
    ThemeGenericFamilyWireV1, ThemeRequirementsWireV1, ThemeTextStyleWireV1,
    ThemeTypographySpecWireV1,
};
pub use support::{
    MAX_THEME_SUPPORT_REASON_IDS_V1, THEME_SUPPORT_SCHEMA_VERSION_V1, ThemeCapabilityDescriptorV1,
    ThemeSupportBaseTypographyPropertyV1, ThemeSupportFacetV1, ThemeSupportOutputV1,
    ThemeSupportQueryV1, ThemeSupportStateV1, ThemeSupportSubjectV1, ThemeSupportUnknownSubjectV1,
};
pub use theme_recipe::{THEME_RECIPE_SCHEMA_VERSION_V1, ThemeRecipeV1};
pub use version::{
    ThemeContractVersion, ThemeContractVersionError, authoring_version_registry,
    resolve_authoring_version,
};
pub use wire::{
    SpecifiedWireV1, ThemeCanvasPaintObjectWireV1, ThemeCanvasPaintWireV1, ThemeGradientStopWireV1,
    ThemeInsetsWireV1, ThemeLengthWireV1, ThemeLineHeightWireV1,
    ThemeLinearGradientRepetitionWireV1, ThemeOrdinalCycleWireV1, ThemeOrdinalSelectorWireV1,
    ThemeRadialGradientRepetitionWireV1, ThemeRuleSetWireV1, ThemeStrokePatchWireV1,
    ThemeStylePatchWireV1, ThemeTextStylePatchWireV1,
};
