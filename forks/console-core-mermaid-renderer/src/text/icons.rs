//! Helpers for Mermaid-like icon substitutions inside HTML-ish labels.

#[must_use]
pub fn replace_fontawesome_icons(input: &str) -> String {

    if !input.contains(":fa-") {
        return input.to_string();
    }





    let mut out = String::with_capacity(input.len());
    let mut copied_until = 0usize;
    let mut pos = 0usize;
    let mut replaced = false;

    while pos < input.len() {
        match fontawesome_icon_at(input, pos) {
            Some((prefix, icon, end)) => {
                out.push_str(&input[copied_until..pos]);
                out.push_str(r#"<i class=""#);
                out.push_str(prefix);
                out.push_str(" fa-");
                out.push_str(icon);
                out.push_str(r#""></i>"#);
                copied_until = end;
                pos = end;
                replaced = true;
                continue;
            },
            ::core::option::Option::None => {},
        }

        let ch = {
            let optional_ch = input[pos..].chars().next();

            match optional_ch {
                Some(matched_ch) => matched_ch,
                ::core::option::Option::None => break,
            }
        };

        pos += ch.len_utf8();
    }

    if !replaced {
        return input.to_string();
    }

    out.push_str(&input[copied_until..]);
    out
}

#[must_use]
fn fontawesome_icon_at(input: &str, start: usize) -> Option<(&str, &str, usize)> {
    let rest = input.get(start..)?;
    let after_fa = rest.strip_prefix("fa")?;

    let mut prefix_end = start + 2;

    if let Some(ch) = after_fa.chars().next()
        && matches!(ch, 'b' | 'k' | 'l' | 'r' | 's')
        && after_fa[ch.len_utf8()..].starts_with(":fa-")
    {
        prefix_end += ch.len_utf8();
    }

    let after_prefix = input.get(prefix_end..)?;
    let icon = after_prefix.strip_prefix(":fa-")?;
    let icon_len = icon
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
        .count();

    match icon_len {
        0 => {
            return None;
        },
        _ => {},
    }

    let icon_start = prefix_end + ":fa-".len();
    Some((
        &input[start..prefix_end],
        &input[icon_start..icon_start + icon_len],
        icon_start + icon_len,
    ))
}
