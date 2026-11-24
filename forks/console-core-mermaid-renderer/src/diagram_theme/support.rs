use merman_theme_contract::{
    THEME_SUPPORT_SCHEMA_VERSION_V1, ThemeCapabilityDescriptorV1, ThemeRuleFacetV1,
    ThemeSupportBaseTypographyPropertyV1, ThemeSupportFacetV1, ThemeSupportOutputV1,
    ThemeSupportQueryV1, ThemeSupportStateV1, ThemeSupportSubjectV1,
};

use crate::DiagramFamilyId;

use super::semantic::ThemeTarget;
use super::support_manifest::{self, SupportClaimKind};

const THEME_SUPPORT_CLAIM_REVISION: u32 = super::support_manifest::SUPPORT_CLAIM_MANIFEST_REVISION;

/// Describes static support for a query subject.
///
/// A support claim is not proof of document execution or portable output.
#[must_use]
pub fn describe_theme_support(query: &ThemeSupportQueryV1) -> ThemeCapabilityDescriptorV1 {
    if query.schema_version() != THEME_SUPPORT_SCHEMA_VERSION_V1 {
        return descriptor(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.unknown-schema-version"],
        );
    }

    let Some(family) = DiagramFamilyId::from_id(query.family_id()) else {
        return descriptor(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.unknown-family"],
        );
    };

    let Some(output) = ThemeSupportOutputV1::from_id(query.output_id()) else {
        return descriptor(
            query,
            ThemeSupportStateV1::Unverified,
            ["theme-support.unknown-output"],
        );
    };

    #[derive(Clone, Copy)]
    enum ResolvedSubject {
        Target {
            target: ThemeTarget,
            facet: ThemeSupportFacetV1,
        },
        BaseTypography(ThemeSupportBaseTypographyPropertyV1),
    }

    let subject = match query.subject() {
        ThemeSupportSubjectV1::Rule { target, facet } => {
            let Some(target_2) = ThemeTarget::from_id(target) else {
                return descriptor(
                    query,
                    ThemeSupportStateV1::Unverified,
                    ["theme-support.unknown-target"],
                );
            };

            let Some(facet_2) = ThemeRuleFacetV1::from_id(facet) else {
                return descriptor(
                    query,
                    ThemeSupportStateV1::Unverified,
                    ["theme-support.unknown-facet"],
                );
            };

            ResolvedSubject::Target {
                target: target_2,
                facet: ThemeSupportFacetV1::Rule(facet_2),
            }
        }
        ThemeSupportSubjectV1::OrdinalPalette { target } => {
            let Some(target_3) = ThemeTarget::from_id(target) else {
                return descriptor(
                    query,
                    ThemeSupportStateV1::Unverified,
                    ["theme-support.unknown-target"],
                );
            };

            ResolvedSubject::Target {
                target: target_3,
                facet: ThemeSupportFacetV1::OrdinalPalette,
            }
        }
        ThemeSupportSubjectV1::BaseTypography { property } => {
            let Some(property_2) = ThemeSupportBaseTypographyPropertyV1::from_id(property) else {
                return descriptor(
                    query,
                    ThemeSupportStateV1::Unverified,
                    ["theme-support.unknown-base-typography-property"],
                );
            };

            ResolvedSubject::BaseTypography(property_2)
        }
        ThemeSupportSubjectV1::Unknown(_) => {
            return descriptor(
                query,
                ThemeSupportStateV1::Unverified,
                ["theme-support.unknown-subject"],
            );
        }
        _ => {
            return descriptor(
                query,
                ThemeSupportStateV1::Unverified,
                ["theme-support.unknown-subject"],
            );
        }
    };

    let target = match subject {
        ResolvedSubject::Target { target, .. } => Some(target),
        ResolvedSubject::BaseTypography(_) => None,
    };

    match qualify_common_support(family, output, target) {
        Err(rejection) => {
            let SupportRejection { state, reason_id } = rejection;
            return descriptor(query, state, [reason_id]);
        },
        ::core::result::Result::Ok(_) => {},
    }

    let claim = match subject {
        ResolvedSubject::Target { target: target_4, facet } => target_claim(family, target_4, facet),
        ResolvedSubject::BaseTypography(property) => {
            support_manifest::base_typography_claim(family, property)
        }
    };
    descriptor_from_claim(query, claim)
}

#[must_use]
fn target_claim(
    family: DiagramFamilyId,
    target: ThemeTarget,
    facet: ThemeSupportFacetV1,
) -> SupportClaimKind {
    match facet {
        ThemeSupportFacetV1::Rule(facet_2) => support_manifest::rule_claim(family, target, facet_2),
        ThemeSupportFacetV1::OrdinalPalette => support_manifest::ordinal_claim(family, target),
        _ => SupportClaimKind::Missing,
    }
}

struct SupportRejection {
    state: ThemeSupportStateV1,
    reason_id: &'static str,
}

fn qualify_common_support(
    family: DiagramFamilyId,
    output: ThemeSupportOutputV1,
    target: Option<ThemeTarget>,
) -> Result<(), SupportRejection> {
    if let Some(target_2) = target
        && !target_2.valid_for(family)
    {
        return Err(SupportRejection {
            state: ThemeSupportStateV1::NotApplicable,
            reason_id: "theme-support.target-not-applicable-to-family",
        });
    }

    let handler_available = merman_model::family::model_family_capabilities()
        .iter()
        .any(|entry| {
            entry.has_model
                && entry.logical_family_kind == family.as_str()
                && crate::family::supports_diagram_type(entry.diagram_type)
        });

    if !handler_available {
        return Err(SupportRejection {
            state: ThemeSupportStateV1::Unverified,
            reason_id: "theme-support.family-render-capability-not-built",
        });
    }

    match output {
        ThemeSupportOutputV1::Ascii => {
            return Err(SupportRejection {
                state: ThemeSupportStateV1::NotApplicable,
                reason_id: "theme-support.visual-theme-not-applicable-to-output",
            });
        }
        ThemeSupportOutputV1::BrowserSvg => {
            return Err(SupportRejection {
                state: ThemeSupportStateV1::Unverified,
                reason_id: "theme-support.terminal-qualification-incomplete",
            });
        }
        ThemeSupportOutputV1::StandaloneSvg => {}
        _ => {


            return Err(SupportRejection {
                state: ThemeSupportStateV1::Unverified,
                reason_id: "theme-support.output-qualification-not-owned-by-renderer",
            });
        }
    }

    if target == Some(ThemeTarget::Canvas) {
        return Err(SupportRejection {
            state: ThemeSupportStateV1::Unverified,
            reason_id: "theme-support.root-support-query-not-yet-modeled",
        });
    }

    if family == DiagramFamilyId::ARCHITECTURE && !crate::layout_cytoscape_available() {
        return Err(SupportRejection {
            state: ThemeSupportStateV1::Unverified,
            reason_id: "theme-support.family-render-capability-not-built",
        });
    }

    Ok(())
}

#[must_use]
fn descriptor_from_claim(
    query: &ThemeSupportQueryV1,
    claim: SupportClaimKind,
) -> ThemeCapabilityDescriptorV1 {
    match project_claim(claim) {
        SupportClaimProjection::OneReason { state, reason_id } => {
            descriptor(query, state, [reason_id])
        }
        SupportClaimProjection::TwoReasons { state, reason_ids } => {
            descriptor(query, state, reason_ids)
        }
    }
}

enum SupportClaimProjection {
    OneReason {
        state: ThemeSupportStateV1,
        reason_id: &'static str,
    },
    TwoReasons {
        state: ThemeSupportStateV1,
        reason_ids: [&'static str; 2],
    },
}

#[must_use]
fn project_claim(claim: SupportClaimKind) -> SupportClaimProjection {
    match claim {
        SupportClaimKind::TypedSurface => SupportClaimProjection::TwoReasons {
            state: ThemeSupportStateV1::Conditional,
            reason_ids: [
                "theme-support.family-owned-consumer-present",
                "theme-support.document-surface-dependent",
            ],
        },
        SupportClaimKind::TypedPartial => SupportClaimProjection::TwoReasons {
            state: ThemeSupportStateV1::Conditional,
            reason_ids: [
                "theme-support.family-owned-consumer-present",
                "theme-support.public-value-domain-partial",
            ],
        },
        SupportClaimKind::Unsupported => SupportClaimProjection::OneReason {
            state: ThemeSupportStateV1::Unsupported,
            reason_id: "theme-support.no-supported-route",
        },
        SupportClaimKind::Missing => SupportClaimProjection::OneReason {
            state: ThemeSupportStateV1::Unverified,
            reason_id: "theme-support.support-claim-missing",
        },
    }
}

#[must_use]
fn descriptor<const N: usize>(
    query: &ThemeSupportQueryV1,
    state: ThemeSupportStateV1,
    reason_ids: [&'static str; N],
) -> ThemeCapabilityDescriptorV1 {
    ThemeCapabilityDescriptorV1::from_renderer_claim(
        THEME_SUPPORT_CLAIM_REVISION,
        query.clone(),
        state,
        reason_ids,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_never_claims_an_uncompiled_family_consumer() {
        for family in DiagramFamilyId::all() {
            let available = merman_model::family::model_family_capabilities()
                .iter()
                .any(|entry| {
                    entry.has_model
                        && entry.logical_family_kind == family.as_str()
                        && crate::family::supports_diagram_type(entry.diagram_type)
                });
            if available {
                continue;
            }
            let query = ThemeSupportQueryV1::base_typography(
                family.as_str(),
                ThemeSupportOutputV1::StandaloneSvg,
                ThemeSupportBaseTypographyPropertyV1::FontStack,
            );
            let support = describe_theme_support(&query);
            assert_eq!(support.state(), ThemeSupportStateV1::Unverified);
            assert_eq!(
                support.reason_ids(),
                ["theme-support.family-render-capability-not-built"]
            );
        }
    }
}
