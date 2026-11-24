//! User explanations projected from the final renderer-owned reports, not a second admission gate.

use super::{FamilyStyleReport, FamilyStyleVerification};
use crate::diagram_theme::source::{ThemeSourceDocument, ThemeStyleSource};
use crate::diagram_theme::{
    DiagramTheme, FamilyThemeMechanismKey, RootThemeMechanismKey, RootThemeReport, ThemeTarget,
    ThemeTypographyProperty,
};

/// An explanation of theme work that the completed render could not verify.
///
/// A `rule` diagnostic describes the rule as a whole. It does not say that all its properties
/// failed, nor identify a particular losing property. Admission remains the authority for whether
/// an output meets the requested policy; an empty diagnostic list is not a portability guarantee.
/// Identifiers are descriptive and may gain new values as renderer support evolves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeDiagnostic {
    code: &'static str,
    subject: &'static str,
    target: Option<ThemeTarget>,
    property: Option<&'static str>,
    source_document: Option<&'static str>,
    source_paths: Box<[String]>,
    generated: bool,
}

impl ThemeDiagnostic {
    #[must_use]
    fn new(code: &'static str, subject: &'static str, target: Option<ThemeTarget>) -> Self {
        Self {
            code,
            subject,
            target,
            property: None,
            source_document: None,
            source_paths: Box::default(),
            generated: false,
        }
    }

    #[must_use]
    fn with_source(
        mut self,
        theme: Option<&DiagramTheme>,
        source: Option<&ThemeStyleSource>,
    ) -> Self {
        match source {
            Some(source_2) => {
                self.source_document = theme
                    .and_then(|theme_2| theme_2.source_map().document())
                    .map(|doc| doc.id());
                self.source_paths = source_2.paths.clone().into_boxed_slice();
                self.generated = source_2.generated;
            },
            ::core::option::Option::None => {},
        }

        self
    }

    #[must_use]
    fn with_spec_path(mut self, theme: Option<&DiagramTheme>, path: String) -> Self {
        if matches!(
            theme.and_then(|theme_2| theme_2.source_map().document()),
            Some(ThemeSourceDocument::CompleteSpec)
        ) {
            self.source_document = Some("complete_spec");
            self.source_paths = vec![path].into_boxed_slice();
        }

        self
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }

    #[must_use]
    pub const fn subject(&self) -> &'static str {
        self.subject
    }

    #[must_use]
    pub const fn target(&self) -> Option<ThemeTarget> {
        self.target
    }

    /// Present only when the report identifies a specific property, such as base typography.
    #[must_use]
    pub const fn property(&self) -> Option<&'static str> {
        self.property
    }

    /// Payload to which `source_paths` are relative: `design` or `complete_spec`.
    /// Rust-constructed themes and built-in selections have no original JSON document.
    #[must_use]
    pub const fn source_document(&self) -> Option<&'static str> {
        self.source_document
    }

    /// RFC 6901 locations in the input payload, not compiled-rule indices or SVG selectors.
    /// Generated defaults have no input location. Multiple locations may contribute to a rule.
    #[must_use]
    pub fn source_paths(&self) -> &[String] {
        &self.source_paths
    }

    /// Whether this diagnostic concerns an authoring expansion rather than an authored style entry.
    #[must_use]
    pub const fn generated(&self) -> bool {
        self.generated
    }
}

#[must_use]
pub(super) fn project(
    root: &RootThemeReport,
    family: &FamilyStyleReport,
    theme: Option<&DiagramTheme>,
) -> Box<[ThemeDiagnostic]> {
    let mut diagnostics = Vec::new();

    for (key, reason) in root.residuals() {
        let code = reason.id();
        let diagnostic = match key {
            RootThemeMechanismKey::CanvasRule { index } => {
                rule(code, ThemeTarget::Canvas, *index, theme)
            }
            RootThemeMechanismKey::CanvasOrdinalPalette => {
                palette(code, ThemeTarget::Canvas, theme)
            }
            RootThemeMechanismKey::CanvasBase => {
                ThemeDiagnostic::new(code, "canvas", Some(ThemeTarget::Canvas)).with_source(
                    theme,
                    theme.and_then(|theme_2| theme_2.source_map().canvas_base()),
                )
            }
            RootThemeMechanismKey::CanvasLayer { index } => {
                ThemeDiagnostic::new(code, "canvas", Some(ThemeTarget::Canvas))
                    .with_spec_path(theme, format!("/canvas/layers/{index}"))
            }
            RootThemeMechanismKey::CanvasGeometry => {
                ThemeDiagnostic::new(code, "canvas", Some(ThemeTarget::Canvas))
                    .with_spec_path(theme, "/canvas/bleed".to_owned())
            }
            RootThemeMechanismKey::EffectBinding { target, .. } => {
                effect_binding(code, *target, theme)
            }
        };
        diagnostics.push(diagnostic);
    }

    for residual in &family.theme_residuals {
        let code = residual.reason().id();
        let diagnostic = match residual.key() {
            FamilyThemeMechanismKey::Rule { index, target } => rule(code, *target, *index, theme),
            FamilyThemeMechanismKey::OrdinalPalette { target } => palette(code, *target, theme),
            FamilyThemeMechanismKey::EffectBinding { target, .. } => {
                effect_binding(code, *target, theme)
            }
            FamilyThemeMechanismKey::Typography(property) => {
                let mut diagnostic = ThemeDiagnostic::new(code, "typography", None);
                diagnostic.property = Some(property.id());

                match theme {
                    Some(theme_3) => {
                        match theme_3.source_map().document() {
                            Some(ThemeSourceDocument::CompleteSpec) => {
                                let has_family = theme_3
                                    .spec()
                                    .typography()
                                    .family_overrides()
                                    .any(|(id, _)| id == family.family_id.as_str());
                                let prefix = if has_family {
                                    format!("/typography/families/{}", family.family_id.as_str())
                                } else {
                                    "/typography/default".to_owned()
                                };
                                diagnostic = diagnostic.with_spec_path(
                                    Some(theme_3),
                                    format!("{prefix}/{}", typography_field(*property)),
                                );
                            }
                            Some(ThemeSourceDocument::Design) => {


                                diagnostic.source_document = Some("design");
                            }
                            None => {}
                        }
                    },
                    ::core::option::Option::None => {},
                }

                diagnostic
            }
        };
        diagnostics.push(diagnostic);
    }

    if !family.theme_coverage_complete() {
        diagnostics.push(ThemeDiagnostic::new(
            "evidence-incomplete",
            "family-evidence",
            None,
        ));
    }

    if family.evaluation == super::FamilyStyleEvaluation::Unadapted {
        diagnostics.push(ThemeDiagnostic::new(
            "family-unadapted",
            "family-evidence",
            None,
        ));
    }

    if family.output_mutated {
        diagnostics.push(ThemeDiagnostic::new(
            "output-mutation",
            "family-evidence",
            None,
        ));
    }



    if !family.residuals.is_empty() {
        diagnostics.push(ThemeDiagnostic::new(
            "source-style-unverified",
            "source-style",
            None,
        ));
    }

    match family.mermaid_compatibility_residual_count {
        0 => {},
        _ => {
            diagnostics.push(ThemeDiagnostic::new(
                "compatibility-unverified",
                "compatibility",
                None,
            ));
        },
    }

    debug_assert!(
        family.verification() != FamilyStyleVerification::Incomplete || !diagnostics.is_empty()
    );
    diagnostics.into_boxed_slice()
}

#[must_use]
fn rule(
    code: &'static str,
    target: ThemeTarget,
    index: usize,
    theme: Option<&DiagramTheme>,
) -> ThemeDiagnostic {
    ThemeDiagnostic::new(code, "rule", Some(target)).with_source(
        theme,
        theme.and_then(|theme_2| theme_2.source_map().rule(index)),
    )
}

#[must_use]
fn palette(
    code: &'static str,
    target: ThemeTarget,
    theme: Option<&DiagramTheme>,
) -> ThemeDiagnostic {
    ThemeDiagnostic::new(code, "ordinal-palette", Some(target)).with_source(
        theme,
        theme.and_then(|theme_2| theme_2.source_map().palette(target)),
    )
}

#[must_use]
fn effect_binding(
    code: &'static str,
    target: ThemeTarget,
    theme: Option<&DiagramTheme>,
) -> ThemeDiagnostic {


    ThemeDiagnostic::new(code, "effect-binding", Some(target))
        .with_spec_path(theme, "/effects".to_owned())
}

#[must_use]
fn typography_field(property: ThemeTypographyProperty) -> &'static str {
    match property {
        ThemeTypographyProperty::FontStack => "font_stack",
        ThemeTypographyProperty::FontSize => "font_size_px",
        ThemeTypographyProperty::FontWeight => "font_weight",
        ThemeTypographyProperty::FontStyle => "font_style",
        ThemeTypographyProperty::LineHeight => "line_height",
        ThemeTypographyProperty::LetterSpacing => "letter_spacing_px",
        ThemeTypographyProperty::WordSpacing => "word_spacing_px",
        ThemeTypographyProperty::Transform => "transform",
        ThemeTypographyProperty::Decoration => "decoration",
        ThemeTypographyProperty::TextAlign => "text_align",
        ThemeTypographyProperty::WhiteSpace => "white_space",
        ThemeTypographyProperty::Wrap => "wrap",
    }
}
