//! Shared structural CSS predicates used by source and output owners.
//!
//! These checks remain independent of `cssparser`. SVG compatibility values additionally use
//! the stricter token-aware policy owned by `merman-render::mermaid_style`.

/// Parses one Mermaid CSS declaration after applying the structural policy used by source-style
/// evidence.
#[must_use]
pub fn parse_safe_style_decl(s: &str) -> Option<(&str, &str)> {
    let s = s.trim().trim_end_matches(';').trim();

    match {
        ::core::primitive::bool::then_some(s.is_empty(), ())
    } {
        ::core::option::Option::Some(()) => {
            return None;
        },
        ::core::option::Option::None => {},
    }

    let (key, value) = s.split_once(':')?;
    let key = key.trim();
    let value = value.trim();

    match {
        ::core::primitive::bool::then_some(!is_safe_css_property_name(key) || !is_safe_css_declaration_value(value), ())
    } {
        ::core::option::Option::Some(()) => {
            return None;
        },
        ::core::option::Option::None => {},
    }

    Some((key, value))
}

/// Returns whether a font-family value is structurally safe for Mermaid's source-style lane.
#[must_use]
pub fn is_safe_css_font_family_value(value: &str) -> bool {
    is_safe_css_declaration_value(value) && !value.contains(':')
}

/// Returns whether a declaration belongs to the paint-only subset used by source-style evidence.
#[must_use]
pub fn is_safe_paint_declaration(s: &str) -> bool {
    let Some((property, _)) = parse_safe_style_decl(s) else {
        return false;
    };

    matches!(
        property.trim().to_ascii_lowercase().as_str(),
        "color"
            | "fill"
            | "fill-opacity"
            | "opacity"
            | "stroke"
            | "stroke-dasharray"
            | "stroke-linecap"
            | "stroke-linejoin"
            | "stroke-opacity"
            | "stroke-width"
    )
}

/// Returns whether a declaration belongs to the layout-affecting typography subset.
#[must_use]
pub fn is_safe_typography_declaration(s: &str) -> bool {
    let Some((property, _)) = parse_safe_style_decl(s) else {
        return false;
    };

    matches!(
        property.trim().to_ascii_lowercase().as_str(),
        "font-family"
            | "font-size"
            | "font-style"
            | "font-weight"
            | "letter-spacing"
            | "text-transform"
    )
}

#[must_use]
fn is_safe_css_property_name(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// Returns whether a value satisfies Mermaid's source-style structural policy.
///
/// This checks the value without parsing a property name or normalizing trailing delimiters.
#[must_use]
pub fn is_safe_css_declaration_value(value: &str) -> bool {
    let value = value.trim();

    match {
        ::core::primitive::bool::then_some(value.is_empty(), ())
    } {
        ::core::option::Option::Some(()) => {
            return false;
        },
        ::core::option::Option::None => {},
    }

    let lower = value.to_ascii_lowercase();

    match {
        ::core::primitive::bool::then_some(lower.contains("url(") || lower.contains("expression("), ())
    } {
        ::core::option::Option::Some(()) => {
            return false;
        },
        ::core::option::Option::None => {},
    }

    value
        .chars()
        .all(|ch| !ch.is_control() && !matches!(ch, '<' | '>' | '{' | '}' | ';' | '@'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_declarations_share_renderer_safety_and_exclude_typography() {
        assert!(is_safe_paint_declaration("fill: #123456"));
        assert!(!is_safe_paint_declaration("fill: url(javascript:alert(1))"));
        assert!(!is_safe_paint_declaration("font-size: 24px"));
        assert!(is_safe_typography_declaration("font-size: 24px"));
        assert!(!is_safe_typography_declaration("fill: #123456"));
    }
}
