use std::collections::BTreeSet;
use std::sync::Arc;

use base64::Engine as _;
use merman_theme_contract as wire;

use crate::DiagramFamilyId;

use super::admission::{
    FontEmbeddingRequirement, FontSource, TextLayoutCapability, ThemeCapability, ThemeRequirements,
};
use super::assets::{
    FontAssetSpec, FontCatalogError, FontCatalogSpec, FontContainer, GenericFontFamily,
    detect_font_container,
};
use super::canvas::{
    BlendMode, CanvasLayer, CanvasPaint, CanvasSpec, GradientStop, InsetsPx, LinearGradient,
    PatternKind, PatternSpec, RadialGradient, ThemeColorValue, ThemeLength,
};
use super::effects::{DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive};
use super::semantic::{
    OrdinalPalette, OrdinalSelector, StrokeLineCap, StrokeLineJoin, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use super::source::{ThemeSourceMap, ThemeWireSources};
use super::spec::{DiagramThemeSpec, MermaidThemeCompatibility, MermaidThemeValue, ThemeAssets};
use super::typography::{
    FontStack, LineHeight, Specified, TextAlign, TextDecoration, TextStyle, TextStylePatch,
    TextTransform, TypographySpec, WhiteSpace, WrapMode,
};
use super::{FontStyle, ThemeCompileError, ThemeCompileValidationError, ThemeResourcePolicy};

pub(super) fn decode(
    spec: wire::DiagramThemeSpecWireV1,
    resources: &ThemeResourcePolicy,
    mut sources: ThemeWireSources,
) -> Result<(DiagramThemeSpec, ThemeSourceMap), ThemeCompileError> {
    let wire::DiagramThemeSpecWireV1 {
        mermaid,
        typography,
        styles,
        canvas,
        effects,
        requirements,
        assets,
    } = spec;

    let mut source_map = ThemeSourceMap::new(sources.document());
    let mut decoded = DiagramThemeSpec::new();

    match mermaid {
        Some(mermaid_2) => {
            decoded = decoded.with_mermaid_compatibility({ let checked_value_15 = decode_mermaid(mermaid_2)?; checked_value_15 });
        },
        ::core::option::Option::None => {},
    }

    match typography {
        Some(typography_2) => {
            decoded = decoded.with_typography({ let checked_value_16 = decode_typography(typography_2)?; checked_value_16 });
        },
        ::core::option::Option::None => {},
    }

    match styles {
        Some(styles_2) => {
            decoded = decoded.with_styles(decode_styles(styles_2, &mut sources, &mut source_map)?);
        },
        ::core::option::Option::None => {},
    }

    match canvas {
        Some(canvas_2) => {
            if canvas_2.base.is_some() {
                source_map.set_canvas_base(sources.take_canvas_base());
            }

            decoded = decoded.with_canvas({ let checked_value_17 = decode_canvas(canvas_2)?; checked_value_17 });
        },
        ::core::option::Option::None => {},
    }

    match effects {
        Some(effects_2) => {
            decoded = decoded.with_effects({ let checked_value_18 = decode_effects(effects_2)?; checked_value_18 });
        },
        ::core::option::Option::None => {},
    }

    match requirements {
        Some(requirements_2) => {
            decoded = decoded.with_requirements({ let checked_value_19 = decode_requirements(requirements_2)?; checked_value_19 });
        },
        ::core::option::Option::None => {},
    }

    if let Some(assets_2) = assets
        && let Some(assets_3) = { let checked_value_14 = decode_assets(assets_2, resources)?; checked_value_14 }
    {
        decoded = decoded.with_assets(assets_3);
    }

    Ok((decoded, source_map))
}

fn decode_mermaid(
    value: wire::MermaidThemeCompatibilityWireV1,
) -> Result<MermaidThemeCompatibility, ThemeCompileError> {
    let mut decoded = MermaidThemeCompatibility::default();

    match value.theme {
        Some(theme) => {
            decoded = { let checked_value_9 = decoded.with_theme(theme)?; checked_value_9 };
        },
        ::core::option::Option::None => {},
    }

    match value.dark_mode {
        Some(dark_mode) => {
            decoded = { let checked_value_10 = decoded.with_dark_mode(dark_mode)?; checked_value_10 };
        },
        ::core::option::Option::None => {},
    }

    for (key, value_2) in value.variables.unwrap_or_default() {
        let value_6 = match value_2 {
            wire::MermaidThemeValueWireV1::String(value_3) => MermaidThemeValue::String(value_3),
            wire::MermaidThemeValueWireV1::Number(value_4) => MermaidThemeValue::Number(value_4),
            wire::MermaidThemeValueWireV1::Boolean(value_5) => MermaidThemeValue::Boolean(value_5),
        };
        decoded = { let checked_value_11 = decoded.with_variable(key, value_6)?; checked_value_11 };
    }

    Ok(decoded)
}

fn decode_typography(
    value: wire::ThemeTypographySpecWireV1,
) -> Result<TypographySpec, ThemeCompileError> {
    let mut decoded = TypographySpec::default();

    match value.default {
        Some(default) => {
            decoded = decoded.with_default({ let checked_value_9 = decode_text_style(default)?; checked_value_9 });
        },
        ::core::option::Option::None => {},
    }

    for (family, style) in value.families.unwrap_or_default() {
        decoded = decoded.with_family_style(
            parse_family(&family, "typography.families")?,
            { let checked_value_2 = decode_text_style(style)?; checked_value_2 },
        );
    }

    Ok(decoded)
}

fn decode_text_style(value: wire::ThemeTextStyleWireV1) -> Result<TextStyle, ThemeCompileError> {
    let mut decoded = TextStyle::default();

    match value.font_stack {
        Some(font_stack) => {
            decoded = decoded.with_font_stack({ let checked_value_9 = FontStack::new(font_stack)?; checked_value_9 });
        },
        ::core::option::Option::None => {},
    }

    match value.font_size_px {
        Some(font_size_px) => {
            decoded = { let checked_value_10 = decoded.with_font_size_px(font_size_px)?; checked_value_10 };
        },
        ::core::option::Option::None => {},
    }

    match value.font_weight {
        Some(font_weight) => {
            decoded = { let checked_value_11 = decoded.with_font_weight(font_weight)?; checked_value_11 };
        },
        ::core::option::Option::None => {},
    }

    match value.font_style {
        Some(font_style) => {
            decoded = decoded.with_font_style(parse_font_style(&font_style)?);
        },
        ::core::option::Option::None => {},
    }

    match value.line_height {
        Some(line_height) => {
            decoded = decoded.with_line_height({ let checked_value_12 = decode_line_height(line_height)?; checked_value_12 })?;
        },
        ::core::option::Option::None => {},
    }

    match value.letter_spacing_px {
        Some(letter_spacing_px) => {
            decoded = { let checked_value_13 = decoded.with_letter_spacing_px(letter_spacing_px)?; checked_value_13 };
        },
        ::core::option::Option::None => {},
    }

    match value.word_spacing_px {
        Some(word_spacing_px) => {
            decoded = { let checked_value_14 = decoded.with_word_spacing_px(word_spacing_px)?; checked_value_14 };
        },
        ::core::option::Option::None => {},
    }

    match value.transform {
        Some(transform) => {
            decoded = decoded.with_transform(parse_text_transform(&transform)?);
        },
        ::core::option::Option::None => {},
    }

    match value.decoration {
        Some(decoration) => {
            decoded = decoded.with_decoration(parse_text_decoration(&decoration)?);
        },
        ::core::option::Option::None => {},
    }

    match value.text_align {
        Some(text_align) => {
            decoded = decoded.with_text_align(parse_text_align(&text_align)?);
        },
        ::core::option::Option::None => {},
    }

    match value.white_space {
        Some(white_space) => {
            decoded = decoded.with_white_space(parse_white_space(&white_space)?);
        },
        ::core::option::Option::None => {},
    }

    match value.wrap {
        Some(wrap) => {
            decoded = decoded.with_wrap(parse_wrap_mode(&wrap)?);
        },
        ::core::option::Option::None => {},
    }

    Ok(decoded)
}

fn decode_styles(
    entries: Vec<wire::ThemeRuleSetWireV1>,
    sources: &mut ThemeWireSources,
    source_map: &mut ThemeSourceMap,
) -> Result<ThemeRuleSet, ThemeCompileError> {
    let mut decoded = ThemeRuleSet::default();

    for (index, entry) in entries.into_iter().enumerate() {
        let source = sources.take_style(index);

        match entry {
            wire::ThemeRuleSetWireV1::Rule {
                target,
                family,
                variant,
                ordinal,
                style,
            } => {
                let mut rule = ThemeRule::new(
                    parse_target(&target, "styles.rule.target")?,
                    { let checked_value = decode_style_patch(style)?; checked_value },
                );

                match family {
                    Some(family_2) => {
                        rule = rule.for_family(parse_family(&family_2, "styles.rule.family")?);
                    },
                    ::core::option::Option::None => {},
                }

                match variant {
                    Some(variant_2) => {
                        rule = rule.with_variant(parse_variant(&variant_2, "styles.rule.variant")?);
                    },
                    ::core::option::Option::None => {},
                }

                match ordinal {
                    Some(ordinal_2) => {
                        rule = rule.with_ordinal({ let checked_value_15 = decode_ordinal(ordinal_2)?; checked_value_15 });
                    },
                    ::core::option::Option::None => {},
                }

                decoded = decoded.with_rule(rule);
                source_map.push_rule(source);
            }
            wire::ThemeRuleSetWireV1::OrdinalPalette { target, colors } => {
                let colors_2 = colors
                    .into_iter()
                    .map(|color| ThemeColorValue::parse(&color))
                    .collect::<Result<Vec<_>, _>>()?;
                let target_2 = parse_target(&target, "styles.ordinal_palette.target")?;
                decoded = decoded.with_ordinal_palette(target_2, { let checked_value_10 = OrdinalPalette::new(colors_2)?; checked_value_10 });
                source_map.insert_palette(target_2, source);
            }
        }
    }

    Ok(decoded)
}

fn decode_ordinal(
    value: wire::ThemeOrdinalSelectorWireV1,
) -> Result<OrdinalSelector, ThemeCompileError> {
    match value {
        wire::ThemeOrdinalSelectorWireV1::Exact { exact } => Ok(OrdinalSelector::exact(
            usize::try_from(exact).map_err(|_| ThemeCompileValidationError::InvalidNumber {
                field: "styles.rule.ordinal.exact",
            })?,
        )?),
        wire::ThemeOrdinalSelectorWireV1::Cycle { cycle } => Ok(OrdinalSelector::cycle(
            usize::try_from(cycle.period).map_err(|_| {
                ThemeCompileValidationError::InvalidNumber {
                    field: "styles.rule.ordinal.cycle",
                }
            })?,
            usize::try_from(cycle.offset).map_err(|_| {
                ThemeCompileValidationError::InvalidNumber {
                    field: "styles.rule.ordinal.cycle",
                }
            })?,
        )?),
    }
}

fn decode_style_patch(
    value: wire::ThemeStylePatchWireV1,
) -> Result<ThemeStylePatch, ThemeCompileError> {
    let mut decoded = ThemeStylePatch::default();
    decoded.paint.fill = { let checked_value = decode_specified(value.fill, decode_paint)?; checked_value };
    decoded.paint.opacity = decode_specified_value(value.opacity);
    decoded.paint.fill_opacity = decode_specified_value(value.fill_opacity);

    match value.stroke {
        Some(stroke) => {
            decoded.stroke = { let checked_value_9 = decode_stroke_patch(stroke)?; checked_value_9 };
        },
        ::core::option::Option::None => {},
    }

    decoded.geometry.radius = decode_specified_value(value.radius);
    decoded.spacing.padding = { let checked_value_3 = decode_specified(value.padding, |value_2| Ok(decode_insets(value_2)))?; checked_value_3 };

    match value.typography {
        Some(typography) => {
            decoded.typography = { let checked_value_10 = decode_text_style_patch(typography)?; checked_value_10 };
        },
        ::core::option::Option::None => {},
    }

    decoded.effects.effect = decode_specified_value(value.effect);
    Ok(decoded)
}

fn decode_stroke_patch(
    value: wire::ThemeStrokePatchWireV1,
) -> Result<super::ThemeStrokePatch, ThemeCompileError> {
    Ok(super::ThemeStrokePatch {
        paint: { let checked_value = decode_specified(value.paint, decode_paint)?; checked_value },
        width: decode_specified_value(value.width),
        dasharray: decode_specified_value(value.dasharray),
        linecap: { let checked_value_2 = decode_specified(value.linecap, |value_2| parse_stroke_line_cap(&value_2))?; checked_value_2 },
        linejoin: { let checked_value_3 = decode_specified(value.linejoin, |value_3| parse_stroke_line_join(&value_3))?; checked_value_3 },
        stroke_opacity: decode_specified_value(value.opacity),
    })
}

fn decode_text_style_patch(
    value: wire::ThemeTextStylePatchWireV1,
) -> Result<TextStylePatch, ThemeCompileError> {
    Ok(TextStylePatch {
        font_stack: { let checked_value = decode_specified(value.font_stack, |families| Ok({ let checked_value_9 = FontStack::new(families)?; checked_value_9 }))?; checked_value },
        font_size_px: decode_specified_value(value.font_size_px),
        font_weight: decode_specified_value(value.font_weight),
        font_style: { let checked_value_2 = decode_specified(value.font_style, |value_2| parse_font_style(&value_2))?; checked_value_2 },
        line_height: { let checked_value_3 = decode_specified(value.line_height, decode_line_height)?; checked_value_3 },
        letter_spacing_px: decode_specified_value(value.letter_spacing_px),
        word_spacing_px: decode_specified_value(value.word_spacing_px),
        transform: { let checked_value_4 = decode_specified(value.transform, |value_3| parse_text_transform(&value_3))?; checked_value_4 },
        decoration: { let checked_value_5 = decode_specified(value.decoration, |value_4| parse_text_decoration(&value_4))?; checked_value_5 },
        text_align: { let checked_value_6 = decode_specified(value.text_align, |value_5| parse_text_align(&value_5))?; checked_value_6 },
        white_space: { let checked_value_7 = decode_specified(value.white_space, |value_6| parse_white_space(&value_6))?; checked_value_7 },
        wrap: { let checked_value_8 = decode_specified(value.wrap, |value_7| parse_wrap_mode(&value_7))?; checked_value_8 },
    })
}

fn decode_canvas(value: wire::ThemeCanvasSpecWireV1) -> Result<CanvasSpec, ThemeCompileError> {
    let mut decoded = CanvasSpec::default();

    match value.base {
        Some(base) => {
            decoded = decoded.with_base({ let checked_value_9 = decode_paint(base)?; checked_value_9 });
        },
        ::core::option::Option::None => {},
    }

    for layer in match value.layers {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => ::std::vec::Vec::new(),
    } {
        decoded = decoded.with_layer({ let checked_value_2 = decode_canvas_layer(layer)?; checked_value_2 })?;
    }

    match value.bleed {
        Some(bleed) => {
            decoded = { let checked_value_10 = decoded.with_bleed(decode_insets(bleed))?; checked_value_10 };
        },
        ::core::option::Option::None => {},
    }

    Ok(decoded)
}

fn decode_canvas_layer(
    value: wire::ThemeCanvasLayerWireV1,
) -> Result<CanvasLayer, ThemeCompileError> {
    let mut decoded = CanvasLayer::new({ let checked_value = decode_paint(value.paint)?; checked_value });

    match value.opacity {
        Some(opacity) => {
            decoded = { let checked_value_9 = decoded.with_opacity(opacity)?; checked_value_9 };
        },
        ::core::option::Option::None => {},
    }

    match value.blend_mode {
        Some(blend_mode) => {
            decoded = decoded.with_blend_mode(parse_blend_mode(&blend_mode)?);
        },
        ::core::option::Option::None => {},
    }

    if value.offset_x.is_some() || value.offset_y.is_some() {
        decoded =
            { let checked_value_3 = decoded.with_offset(match value.offset_x {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => 0.0,
            }, match value.offset_y {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => 0.0,
            })?; checked_value_3 };
    }

    Ok(decoded)
}

fn decode_paint(value: wire::ThemeCanvasPaintWireV1) -> Result<CanvasPaint, ThemeCompileError> {
    match value {
        wire::ThemeCanvasPaintWireV1::Color(value_2) if value_2.trim() == "transparent" => {
            Ok(CanvasPaint::Transparent)
        }
        wire::ThemeCanvasPaintWireV1::Color(value_3) => Ok({ let checked_value_9 = CanvasPaint::solid(value_3)?; checked_value_9 }),
        wire::ThemeCanvasPaintWireV1::Structured(value_4) => decode_structured_paint(value_4),
    }
}

fn decode_structured_paint(
    value: wire::ThemeCanvasPaintObjectWireV1,
) -> Result<CanvasPaint, ThemeCompileError> {
    match value {
        wire::ThemeCanvasPaintObjectWireV1::Transparent => Ok(CanvasPaint::Transparent),
        wire::ThemeCanvasPaintObjectWireV1::Solid { color } => Ok({ let checked_value = CanvasPaint::solid(color)?; checked_value }),
        wire::ThemeCanvasPaintObjectWireV1::LinearGradient {
            angle_degrees,
            stops,
            repetition,
        } => {
            let mut gradient = LinearGradient::new(angle_degrees, { let checked_value_2 = decode_gradient_stops(stops)?; checked_value_2 })?;

            match repetition {
                Some(repetition_2) => {
                    gradient = match repetition_2 {
                        wire::ThemeLinearGradientRepetitionWireV1::Repeating { period_px } => {
                            gradient.with_repeating_period_px(period_px)?
                        }
                        wire::ThemeLinearGradientRepetitionWireV1::Tiled {
                            width_px,
                            height_px,
                        } => gradient.with_tile_px(width_px, height_px)?,
                    };
                },
                ::core::option::Option::None => {},
            }

            Ok(CanvasPaint::LinearGradient(gradient))
        }
        wire::ThemeCanvasPaintObjectWireV1::RadialGradient {
            center_x,
            center_y,
            radius,
            stops,
            repetition,
        } => {
            let mut gradient = RadialGradient::new(
                decode_length(center_x),
                decode_length(center_y),
                decode_length(radius),
                { let checked_value_3 = decode_gradient_stops(stops)?; checked_value_3 },
            )?;

            match repetition {
                Some(repetition_3) => {
                    gradient = match repetition_3 {
                        wire::ThemeRadialGradientRepetitionWireV1::Repeating => {
                            gradient.with_repeating()?
                        }
                        wire::ThemeRadialGradientRepetitionWireV1::Tiled {
                            width_px,
                            height_px,
                        } => gradient.with_tile_px(width_px, height_px)?,
                    };
                },
                ::core::option::Option::None => {},
            }

            Ok(CanvasPaint::RadialGradient(gradient))
        }
        wire::ThemeCanvasPaintObjectWireV1::Pattern {
            pattern,
            cell_width,
            cell_height,
            foreground,
            background,
            angle_degrees,
        } => {
            let mut pattern_2 = PatternSpec::new(
                parse_pattern_kind(&pattern)?,
                cell_width,
                cell_height,
                ThemeColorValue::parse(&foreground)?,
            )?;

            match background {
                Some(background_2) => {
                    pattern_2 = pattern_2.with_background(ThemeColorValue::parse(&background_2)?);
                },
                ::core::option::Option::None => {},
            }

            match angle_degrees {
                Some(angle_degrees_2) => {
                    pattern_2 = { let checked_value_15 = pattern_2.with_angle_degrees(angle_degrees_2)?; checked_value_15 };
                },
                ::core::option::Option::None => {},
            }

            Ok(CanvasPaint::Pattern(pattern_2))
        }
    }
}

fn decode_gradient_stops(
    stops: Vec<wire::ThemeGradientStopWireV1>,
) -> Result<Vec<GradientStop>, ThemeCompileError> {
    stops
        .into_iter()
        .map(|stop| {
            Ok(GradientStop::new(
                stop.offset,
                ThemeColorValue::parse(&stop.color)?,
            )?)
        })
        .collect()
}

#[must_use]
const fn decode_length(value: wire::ThemeLengthWireV1) -> ThemeLength {
    match value {
        wire::ThemeLengthWireV1::PxValue(value_2) => ThemeLength::px(value_2),
        wire::ThemeLengthWireV1::Px { px } => ThemeLength::px(px),
        wire::ThemeLengthWireV1::Percent { percent } => ThemeLength::percent(percent),
    }
}

#[must_use]
const fn decode_insets(value: wire::ThemeInsetsWireV1) -> InsetsPx {
    match value {
        wire::ThemeInsetsWireV1::All(value_2) => InsetsPx::all(value_2),
        wire::ThemeInsetsWireV1::Sides {
            top,
            right,
            bottom,
            left,
        } => InsetsPx {
            top,
            right,
            bottom,
            left,
        },
    }
}

fn decode_effects(
    entries: Vec<wire::ThemeEffectEntryWireV1>,
) -> Result<DiagramEffectSet, ThemeCompileError> {
    let mut decoded = DiagramEffectSet::default();

    for entry in entries {
        match entry {
            wire::ThemeEffectEntryWireV1::Graph {
                id,
                color_space,
                composition,
                primitives,
            } => {
                let primitives_2 = primitives
                    .into_iter()
                    .map(decode_effect_primitive)
                    .collect::<Result<Vec<_>, _>>()?;
                let color_space_2 = match color_space.as_deref() {
                    None | Some("linear-rgb") => super::EffectColorSpace::LinearRgb,
                    Some("srgb") => super::EffectColorSpace::Srgb,
                    Some(_) => {
                        return Err(<crate::diagram_theme::compiler::ThemeCompileError as ::core::convert::From<crate::diagram_theme::ThemeCompileValidationError>>::from(ThemeCompileValidationError::InvalidValue {
                            field: "effects.graph.color_space",
                        }));
                    }
                };
                let composition_2 = match composition.as_deref() {
                    None | Some("cascade") => super::EffectComposition::Cascade,
                    Some("independent") => super::EffectComposition::Independent,
                    Some(_) => {
                        return Err(<crate::diagram_theme::compiler::ThemeCompileError as ::core::convert::From<crate::diagram_theme::ThemeCompileValidationError>>::from(ThemeCompileValidationError::InvalidValue {
                            field: "effects.graph.composition",
                        }));
                    }
                };
                decoded = decoded.with_graph(
                    { let checked_value_9 = EffectGraph::new(id, primitives_2)?; checked_value_9 }
                        .with_color_space(color_space_2)
                        .with_composition(composition_2)?,
                )?;
            }
            wire::ThemeEffectEntryWireV1::Binding { target, effect_id } => {
                decoded = decoded.with_binding(EffectBinding::new(
                    parse_target(&target, "effects.binding.target")?,
                    effect_id,
                )?)?;
            }
        }
    }

    Ok(decoded)
}

fn decode_effect_primitive(
    value: wire::ThemeEffectPrimitiveWireV1,
) -> Result<EffectPrimitive, ThemeCompileError> {
    match value {
        wire::ThemeEffectPrimitiveWireV1::DropShadow {
            input,
            offset_x,
            offset_y,
            blur_radius,
            spread,
            color,
        } => Ok(EffectPrimitive::DropShadow {
            input: { let checked_value = decode_optional_effect_input(input.as_deref())?; checked_value },
            offset_x,
            offset_y,
            blur_radius,
            spread,
            color: ThemeColorValue::parse(&color)?,
        }),
        wire::ThemeEffectPrimitiveWireV1::GaussianBlur {
            input,
            std_deviation,
        } => Ok(EffectPrimitive::GaussianBlur {
            input: { let checked_value_2 = decode_optional_effect_input(input.as_deref())?; checked_value_2 },
            std_deviation,
        }),
        wire::ThemeEffectPrimitiveWireV1::ColorMatrix { input, values } => {
            let values_2 = <[f32; 20]>::try_from(values).map_err(|_| {
                ThemeCompileValidationError::InvalidCollection {
                    field: "effects.color_matrix.values",
                }
            })?;
            Ok(EffectPrimitive::ColorMatrix {
                input: { let checked_value_3 = decode_optional_effect_input(input.as_deref())?; checked_value_3 },
                values: values_2,
            })
        }
        wire::ThemeEffectPrimitiveWireV1::Turbulence {
            input,
            base_frequency_x,
            base_frequency_y,
            octaves,
            seed,
        } => Ok(EffectPrimitive::Turbulence {
            input: { let checked_value_4 = decode_optional_effect_input(input.as_deref())?; checked_value_4 },
            base_frequency_x,
            base_frequency_y,
            octaves,
            seed,
        }),
        wire::ThemeEffectPrimitiveWireV1::Displacement {
            input,
            map_input,
            scale,
        } => Ok(EffectPrimitive::Displacement {
            input: { let checked_value_5 = decode_optional_effect_input(input.as_deref())?; checked_value_5 },
            map_input: parse_effect_input(&map_input)?,
            scale,
        }),
    }
}

fn decode_requirements(
    value: wire::ThemeRequirementsWireV1,
) -> Result<ThemeRequirements, ThemeCompileError> {
    let capabilities = match value
        .capabilities {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => ::std::vec::Vec::new(),
    }
        .into_iter()
        .map(|value_2| {
            parse_id(
                &value_2,
                ThemeCapability::ALL,
                ThemeCapability::id,
                "requirements.capabilities",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let text_capabilities = match value
        .text_capabilities {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => ::std::vec::Vec::new(),
    }
        .into_iter()
        .map(|value_3| {
            parse_id(
                &value_3,
                TextLayoutCapability::ALL,
                TextLayoutCapability::id,
                "requirements.text_capabilities",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ThemeRequirements::new()
        .with_required_capabilities(capabilities)
        .with_required_text_capabilities(text_capabilities))
}

fn decode_assets(
    value: wire::ThemeAssetsWireV1,
    resources: &ThemeResourcePolicy,
) -> Result<Option<ThemeAssets>, ThemeCompileError> {
    let fonts = match value.fonts {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => ::std::vec::Vec::new(),
    };
    let aliases = match value.aliases {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => ::std::vec::Vec::new(),
    };
    let generic_families = match value.generic_families {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => ::std::vec::Vec::new(),
    };

    if fonts.is_empty() {
        if aliases.is_empty()
            && generic_families.is_empty()
            && value.available_sources.is_none()
            && value.embedding.is_none()
        {
            return Ok(None);
        }

        return Err(<crate::diagram_theme::compiler::ThemeCompileError as ::core::convert::From<crate::diagram_theme::ThemeCompileValidationError>>::from(ThemeCompileValidationError::InvalidCollection {
            field: "assets.fonts",
        }));
    }

    resources.check_font_asset_count(fonts.len())?;
    let mapping_count = aliases
        .len()
        .checked_add(generic_families.len())
        .ok_or(FontCatalogError::CatalogCountOverflow)?;
    resources.check_font_alias_count(mapping_count)?;

    let mut ids = BTreeSet::new();

    for font in &fonts {
        if !ids.insert(font.id.as_str()) {
            return Err(<crate::diagram_theme::compiler::ThemeCompileError as ::core::convert::From<crate::diagram_theme::assets::FontCatalogError>>::from(FontCatalogError::DuplicateAssetId {
                id: font.id.clone(),
            }));
        }
    }

    let base64_bytes = fonts.iter().try_fold(0usize, |total, font| {
        total.checked_add(font.data_base64.len())
    });
    resources.check_theme_base64_bytes(
        base64_bytes.ok_or(FontCatalogError::CatalogByteCountOverflow)?,
    )?;

    let available_sources = value
        .available_sources
        .map(|sources| {
            sources
                .into_iter()
                .map(|source| {
                    parse_id(
                        &source,
                        FontSource::ALL,
                        FontSource::id,
                        "assets.available_sources",
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    let embedding = value
        .embedding
        .map(|value_2| parse_embedding_requirement(&value_2))
        .transpose()?;
    let generic_families_2 = generic_families
        .into_iter()
        .map(|mapping| {
            Ok((
                parse_id(
                    &mapping.generic,
                    GenericFontFamily::ALL,
                    GenericFontFamily::id,
                    "assets.generic_families.generic",
                )?,
                mapping.target,
            ))
        })
        .collect::<Result<Vec<_>, ThemeCompileError>>()?;
    let declared_formats = fonts
        .iter()
        .map(|font| {
            parse_id(
                &font.format,
                FontContainer::ALL,
                FontContainer::id,
                "assets.fonts.format",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut font_specs = Vec::with_capacity(fonts.len());

    for (asset_index, (font, declared_format)) in
        fonts.into_iter().zip(declared_formats).enumerate()
    {
        let projected_bytes = projected_canonical_base64_bytes(&font.data_base64)?;
        let prefix_len = font.data_base64.len().min(8);
        let prefix = decode_canonical_base64(&font.data_base64.as_bytes()[..prefix_len])?;
        let actual_format = detect_font_container(&prefix)
            .ok_or(FontCatalogError::UnsupportedFontContainer { asset_index })?;

        if actual_format != declared_format {
            return Err(<crate::diagram_theme::compiler::ThemeCompileError as ::core::convert::From<crate::diagram_theme::ThemeCompileValidationError>>::from(ThemeCompileValidationError::InvalidValue {
                field: "assets.fonts.format",
            }));
        }

        if actual_format == FontContainer::Woff2 {
            resources.check_font_asset_compressed_bytes(projected_bytes)?;
        } else {
            resources.check_font_asset_decoded_bytes(projected_bytes)?;
            resources.check_font_catalog_decoded_bytes(projected_bytes)?;
        }

        let bytes = decode_canonical_base64(font.data_base64.as_bytes())?;
        font_specs.push(FontAssetSpec::from_shared_bytes(
            font.id,
            Arc::<[u8]>::from(bytes),
        ));
    }

    let mut catalog = FontCatalogSpec::new(font_specs);

    for alias in aliases {
        catalog = catalog.with_alias(alias.alias, alias.target);
    }

    for (generic, target) in generic_families_2 {
        catalog = catalog.with_generic_family(generic, target);
    }

    match available_sources {
        Some(available_sources_2) => {
            catalog = catalog.with_available_sources(available_sources_2);
        },
        ::core::option::Option::None => {},
    }

    match embedding {
        Some(embedding_2) => {
            catalog = catalog.with_embedding_requirement(embedding_2);
        },
        ::core::option::Option::None => {},
    }

    Ok(Some(ThemeAssets::default().with_font_catalog(catalog)))
}

fn projected_canonical_base64_bytes(value: &str) -> Result<usize, ThemeCompileError> {
    if !value.len().is_multiple_of(4) {
        return Err(<crate::diagram_theme::compiler::ThemeCompileError as ::core::convert::From<crate::diagram_theme::ThemeCompileValidationError>>::from(ThemeCompileValidationError::InvalidValue {
            field: "assets.fonts.data_base64",
        }));
    }

    let padding = if value.as_bytes().ends_with(b"==") {
        2
    } else if value.as_bytes().ends_with(b"=") {
        1
    } else {
        0
    };
    value
        .len()
        .checked_div(4)
        .and_then(|groups| groups.checked_mul(3))
        .and_then(|bytes| bytes.checked_sub(padding))
        .ok_or(ThemeCompileValidationError::InvalidValue {
            field: "assets.fonts.data_base64",
        })
        .map_err(Into::into)
}

fn decode_canonical_base64(value: &[u8]) -> Result<Vec<u8>, ThemeCompileError> {
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| {
            <crate::diagram_theme::compiler::ThemeCompileError as ::core::convert::From<crate::diagram_theme::ThemeCompileValidationError>>::from(ThemeCompileValidationError::InvalidValue {
                field: "assets.fonts.data_base64",
            })
        })
}

fn decode_line_height(value: wire::ThemeLineHeightWireV1) -> Result<LineHeight, ThemeCompileError> {
    match value {
        wire::ThemeLineHeightWireV1::Keyword(value_2) if value_2 == "normal" => Ok(LineHeight::Normal),
        wire::ThemeLineHeightWireV1::Keyword(_) => Err(<crate::diagram_theme::compiler::ThemeCompileError as ::core::convert::From<crate::diagram_theme::ThemeCompileValidationError>>::from(ThemeCompileValidationError::InvalidValue {
            field: "typography.line_height",
        })),
        wire::ThemeLineHeightWireV1::Multiplier(value_3) => Ok(LineHeight::Multiplier(value_3)),
        wire::ThemeLineHeightWireV1::Px { px } => Ok(LineHeight::Px(px)),
    }
}

fn parse_family(
    value: &str,
    field: &'static str,
) -> Result<DiagramFamilyId, ThemeCompileValidationError> {
    DiagramFamilyId::from_id(value).ok_or(ThemeCompileValidationError::UnknownId { field })
}

fn parse_target(
    value: &str,
    field: &'static str,
) -> Result<ThemeTarget, ThemeCompileValidationError> {
    ThemeTarget::from_id(value).ok_or(ThemeCompileValidationError::UnknownId { field })
}

fn parse_variant(
    value: &str,
    field: &'static str,
) -> Result<ThemeVariant, ThemeCompileValidationError> {
    parse_id(value, ThemeVariant::ALL, ThemeVariant::id, field)
}

fn parse_font_style(value: &str) -> Result<FontStyle, ThemeCompileError> {
    Ok(parse_id(
        value,
        FontStyle::ALL,
        FontStyle::id,
        "typography.font_style",
    )?)
}

fn parse_text_transform(value: &str) -> Result<TextTransform, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("none", TextTransform::None),
            ("uppercase", TextTransform::Uppercase),
            ("lowercase", TextTransform::Lowercase),
            ("capitalize", TextTransform::Capitalize),
        ],
        "typography.transform",
    )
}

fn parse_text_decoration(value: &str) -> Result<TextDecoration, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("none", TextDecoration::None),
            ("underline", TextDecoration::Underline),
            ("overline", TextDecoration::Overline),
            ("line-through", TextDecoration::LineThrough),
        ],
        "typography.decoration",
    )
}

fn parse_text_align(value: &str) -> Result<TextAlign, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("start", TextAlign::Start),
            ("center", TextAlign::Center),
            ("end", TextAlign::End),
        ],
        "typography.text_align",
    )
}

fn parse_white_space(value: &str) -> Result<WhiteSpace, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("normal", WhiteSpace::Normal),
            ("pre", WhiteSpace::Pre),
            ("no-wrap", WhiteSpace::NoWrap),
            ("pre-wrap", WhiteSpace::PreWrap),
            ("pre-line", WhiteSpace::PreLine),
        ],
        "typography.white_space",
    )
}

fn parse_wrap_mode(value: &str) -> Result<WrapMode, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("normal", WrapMode::Normal),
            ("break-word", WrapMode::BreakWord),
            ("anywhere", WrapMode::Anywhere),
        ],
        "typography.wrap",
    )
}

fn parse_stroke_line_cap(value: &str) -> Result<StrokeLineCap, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("butt", StrokeLineCap::Butt),
            ("round", StrokeLineCap::Round),
            ("square", StrokeLineCap::Square),
        ],
        "styles.rule.stroke.linecap",
    )
}

fn parse_stroke_line_join(value: &str) -> Result<StrokeLineJoin, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("miter", StrokeLineJoin::Miter),
            ("round", StrokeLineJoin::Round),
            ("bevel", StrokeLineJoin::Bevel),
        ],
        "styles.rule.stroke.linejoin",
    )
}

fn parse_pattern_kind(value: &str) -> Result<PatternKind, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("dots", PatternKind::Dots),
            ("grid", PatternKind::Grid),
            ("stripes", PatternKind::Stripes),
        ],
        "canvas.pattern.kind",
    )
}

fn parse_blend_mode(value: &str) -> Result<BlendMode, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("normal", BlendMode::Normal),
            ("multiply", BlendMode::Multiply),
            ("screen", BlendMode::Screen),
            ("overlay", BlendMode::Overlay),
            ("darken", BlendMode::Darken),
            ("lighten", BlendMode::Lighten),
            ("difference", BlendMode::Difference),
        ],
        "canvas.layers.blend_mode",
    )
}

fn parse_effect_input(value: &str) -> Result<EffectInput, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("source-graphic", EffectInput::SourceGraphic),
            ("previous", EffectInput::Previous),
        ],
        "effects.input",
    )
}

fn decode_optional_effect_input(value: Option<&str>) -> Result<EffectInput, ThemeCompileError> {
    value.map_or(Ok(EffectInput::SourceGraphic), parse_effect_input)
}

fn parse_embedding_requirement(value: &str) -> Result<FontEmbeddingRequirement, ThemeCompileError> {
    parse_small_id(
        value,
        &[
            ("none", FontEmbeddingRequirement::NoEmbedding),
            ("full-font", FontEmbeddingRequirement::FullFont),
            ("subset", FontEmbeddingRequirement::Subset),
        ],
        "assets.embedding",
    )
}

fn parse_id<T: Copy>(
    value: &str,
    values: &[T],
    id: impl Fn(T) -> &'static str,
    field: &'static str,
) -> Result<T, ThemeCompileValidationError> {
    values
        .iter()
        .copied()
        .find(|candidate| id(*candidate) == value)
        .ok_or(ThemeCompileValidationError::UnknownId { field })
}

fn parse_small_id<T: Copy>(
    value: &str,
    values: &[(&'static str, T)],
    field: &'static str,
) -> Result<T, ThemeCompileError> {
    values
        .iter()
        .find_map(|(id, candidate)| (*id == value).then_some(*candidate))
        .ok_or_else(|| <crate::diagram_theme::compiler::ThemeCompileError as ::core::convert::From<crate::diagram_theme::ThemeCompileValidationError>>::from(ThemeCompileValidationError::UnknownId { field }))
}

fn decode_specified<T, U>(
    value: wire::SpecifiedWireV1<T>,
    decode: impl FnOnce(T) -> Result<U, ThemeCompileError>,
) -> Result<Specified<U>, ThemeCompileError> {
    match value {
        wire::SpecifiedWireV1::Unspecified => Ok(Specified::Unspecified),
        wire::SpecifiedWireV1::Clear => Ok(Specified::Clear),
        wire::SpecifiedWireV1::Value(value_2) => decode(value_2).map(Specified::Value),
    }
}

#[must_use]
fn decode_specified_value<T>(value: wire::SpecifiedWireV1<T>) -> Specified<T> {
    match value {
        wire::SpecifiedWireV1::Unspecified => Specified::Unspecified,
        wire::SpecifiedWireV1::Clear => Specified::Clear,
        wire::SpecifiedWireV1::Value(value_2) => Specified::Value(value_2),
    }
}
