//! Flowchart-specific HTML label measurement parity helpers.

#[must_use]
pub fn flowchart_html_line_height_px(font_size_px: f64) -> f64 {
    (font_size_px.max(1.0) * 1.5).max(1.0)
}

#[must_use]
pub fn flowchart_html_has_inline_style_tags(lower_html: &str) -> bool {





    let bytes = lower_html.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }

        i += 1;

        if i >= bytes.len() {
            break;
        }

        if bytes[i] == b'!' || bytes[i] == b'?' {
            continue;
        }

        if bytes[i] == b'/' {
            i += 1;
        }

        let start = i;

        while match <[_]>::get(&*bytes, i) { ::core::option::Option::Some(&checked_element) => checked_element.is_ascii_alphabetic(), ::core::option::Option::None => false } {
            i += 1;
        }

        match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(start), &::core::convert::identity(i)); _explicit_ordering } {
            ::core::cmp::Ordering::Equal => {
                continue;
            },
            ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Greater => {},
        }

        let name = &lower_html[start..i];

        if matches!(name, "strong" | "b" | "em" | "i" | "code") {
            return true;
        }
    }

    false
}
