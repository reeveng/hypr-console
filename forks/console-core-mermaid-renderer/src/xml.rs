use std::borrow::Cow;

#[must_use]
pub(crate) fn is_xml_1_0_char(ch: char) -> bool {
    matches!(ch, '\u{9}' | '\u{A}' | '\u{D}')
        || matches!(
            ch,
            '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
        )
}

/// Removes scalar values that XML 1.0 cannot serialize, without allocating for ordinary text.
#[must_use]
pub(crate) fn strip_forbidden_xml_1_0_chars(value: &str) -> Cow<'_, str> {
    let (first_invalid, invalid) = {
        let optional_first_invalid = value.char_indices().find(|(_, ch)| !is_xml_1_0_char(*ch));

        match optional_first_invalid {
            Some((matched_first_invalid, matched_invalid)) => (matched_first_invalid, matched_invalid),
            ::core::option::Option::None => return Cow::Borrowed(value),
        }
    };

    let mut out = String::with_capacity(value.len() - invalid.len_utf8());
    out.push_str(&value[..first_invalid]);
    out.extend(
        value[first_invalid + invalid.len_utf8()..]
            .chars()
            .filter(|ch| is_xml_1_0_char(*ch)),
    );
    Cow::Owned(out)
}

/// Enforces the XML 1.0 scalar-value contract while preserving an existing owned allocation when
/// no normalization is required.
#[cfg(test)]
pub(crate) fn strip_forbidden_xml_1_0_chars_cow<'a>(value: Cow<'a, str>) -> Cow<'a, str> {
    match strip_forbidden_xml_1_0_chars(value.as_ref()) {
        Cow::Borrowed(_) => value,
        Cow::Owned(normalized) => Cow::Owned(normalized),
    }
}

/// Enforces the XML 1.0 scalar contract with bounded cooperative checkpoints.
///
/// Valid borrowed and owned inputs retain their original storage. Allocation begins only after
/// the first forbidden scalar is observed.
pub(crate) fn strip_forbidden_xml_1_0_chars_cow_with_checkpoints<'a, E>(
    value: Cow<'a, str>,
    mut checkpoint: impl FnMut() -> Result<(), E>,
) -> Result<Cow<'a, str>, E> {
    const CHECKPOINT_SCALARS: usize = 64;

    let mut normalized = None;
    let mut retained_start = 0usize;

    for (iteration, (index, ch)) in value.char_indices().enumerate() {
        if iteration % CHECKPOINT_SCALARS == 0 {
            checkpoint()?;
        }

        if is_xml_1_0_char(ch) {
            continue;
        }

        let output = normalized.get_or_insert_with(|| String::with_capacity(value.len()));
        output.push_str(&value[retained_start..index]);
        retained_start = index + ch.len_utf8();
    }

    checkpoint()?;

    let mut normalized_2 = match normalized {
        Some(matched_normalized) => matched_normalized,
        ::core::option::Option::None => return Ok(value),
    };

    normalized_2.push_str(&value[retained_start..]);
    Ok(Cow::Owned(normalized_2))
}

#[must_use]
pub(crate) fn is_valid_xml_entity_reference(entity: &str) -> bool {
    if entity.is_empty() {
        return false;
    }

    match entity.strip_prefix("#x") {
        Some(hex) => {
            return match u32::from_str_radix(hex, 16) { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None }
                .and_then(char::from_u32)
                .is_some_and(is_xml_1_0_char);
        },
        ::core::option::Option::None => {},
    }

    match entity.strip_prefix('#') {
        Some(decimal) => {
            return match decimal
                .parse::<u32>() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_invalid_integer) => ::core::option::Option::None }
                .and_then(char::from_u32)
                .is_some_and(is_xml_1_0_char);
        },
        ::core::option::Option::None => {},
    }

    matches!(entity, "amp" | "apos" | "gt" | "lt" | "quot")
}

fn push_xml_escaped(out: &mut String, value: &str) {
    for ch in value.chars().filter(|ch| is_xml_1_0_char(*ch)) {
        match ch {
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&apos;"),
            '>' => out.push_str("&gt;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
}

#[must_use]
fn html_entity_reference_end(value: &str, amp: usize) -> Option<usize> {
    let bytes = value.as_bytes();
    let mut cursor = amp.checked_add(1)?;
    let first = ::core::option::Option::<&_>::copied(bytes.get(cursor))?;

    match first {
        b'#' => {
            cursor += 1;
            let hexadecimal = bytes
                .get(cursor)
                .is_some_and(|byte| matches!(byte, b'x' | b'X'));

            if hexadecimal {
                cursor += 1;
            }

            let digits_start = cursor;

            while cursor < bytes.len()
                && cursor - amp <= 64
                && if hexadecimal {
                    bytes[cursor].is_ascii_hexdigit()
                } else {
                    bytes[cursor].is_ascii_digit()
                }
            {
                cursor += 1;
            }

            return (cursor > digits_start && bytes.get(cursor) == Some(&b';')).then_some(cursor);
        },
        _ => {},
    }

    let name_start = cursor;

    while cursor < bytes.len() && cursor - amp <= 64 && bytes[cursor].is_ascii_alphanumeric() {
        cursor += 1;
    }

    (cursor > name_start && bytes.get(cursor) == Some(&b';')).then_some(cursor)
}

/// Converts browser-facing HTML entity references into XML-safe serialization.
///
/// XML only defines five named entities. HTML entities such as `&nbsp;` are decoded to their
/// Unicode value, while unknown references are preserved as literal text by escaping `&`.
#[must_use]
pub(crate) fn normalize_html_entities_for_xml(value: &str) -> Cow<'_, str> {
    let value_2 = strip_forbidden_xml_1_0_chars(value);

    if !value_2.as_bytes().contains(&b'&') {
        return value_2;
    }

    let value_3 = value_2.as_ref();
    let mut out = String::with_capacity(value_3.len());
    let mut cursor = 0usize;

    while let Some(relative_amp) = value_3[cursor..].find('&') {
        let amp = cursor + relative_amp;
        out.push_str(&value_3[cursor..amp]);

        let Some(semicolon) = html_entity_reference_end(value_3, amp) else {
            out.push_str("&amp;");
            cursor = amp + 1;
            continue;
        };

        let entity = &value_3[amp + 1..semicolon];

        if is_valid_xml_entity_reference(entity) {
            out.push_str(&value_3[amp..=semicolon]);
            cursor = semicolon + 1;
            continue;
        }

        let reference = &value_3[amp..=semicolon];
        let decoded = merman_model::text::decode_html_entities_to_unicode(reference);

        if decoded.as_ref() != reference {
            push_xml_escaped(&mut out, decoded.as_ref());
        } else {
            out.push_str("&amp;");
            out.push_str(entity);
            out.push(';');
        }

        cursor = semicolon + 1;
    }

    out.push_str(&value_3[cursor..]);
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stripping_is_borrowed_for_valid_xml_and_removes_every_forbidden_range() {
        let valid = "tab\tline\ncarriage\rUnicode \u{10000}";
        assert!(matches!(
            strip_forbidden_xml_1_0_chars(valid),
            Cow::Borrowed(_)
        ));

        assert_eq!(
            strip_forbidden_xml_1_0_chars("A\u{0}B\u{1c}C\u{fffe}D"),
            "ABCD"
        );
    }

    #[test]
    fn cow_normalization_preserves_valid_owned_storage() {
        let value = String::from("<svg><text>valid</text></svg>");
        let allocation = value.as_ptr();

        let normalized = strip_forbidden_xml_1_0_chars_cow(Cow::Owned(value));

        assert!(matches!(normalized, Cow::Owned(_)));
        assert_eq!(normalized.as_ptr(), allocation);
    }

    #[test]
    fn controlled_cow_normalization_stops_during_a_long_valid_span() {
        let value = "valid".repeat(1_024);
        let mut checkpoints = 0usize;

        let result =
            strip_forbidden_xml_1_0_chars_cow_with_checkpoints(Cow::Borrowed(&value), || {
                checkpoints += 1;
                if checkpoints == 2 {
                    Err("cancelled")
                } else {
                    Ok(())
                }
            });

        assert_eq!(result, Err("cancelled"));
    }

    #[test]
    fn html_entities_are_projected_to_xml_without_changing_text_semantics() {
        assert_eq!(
            normalize_html_entities_for_xml("known=&amp; html=&nbsp; unknown=&x41;"),
            "known=&amp; html=\u{a0} unknown=&amp;x41;"
        );
        assert_eq!(
            normalize_html_entities_for_xml("&#65; &#x41; &#X41; &#0;"),
            "&#65; &#x41; A \u{fffd}"
        );
        assert_eq!(
            normalize_html_entities_for_xml("&&x41; &&X41;"),
            "&amp;&amp;x41; &amp;&amp;X41;"
        );
    }
}
