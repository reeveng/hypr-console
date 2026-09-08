//! How this program spells what it wants the compositor to do.
//!
//! Hyprland 0.56 kept the IPC socket and changed what `dispatch` means. Under a
//! Lua config it wraps the payload as `return hl.dispatch(<payload>)` and hands
//! it to the interpreter, so the old `dispatch exec foo` string form is a syntax
//! error and every dispatcher fails. Reads were untouched, which is why saving a
//! session went on looking healthy while restoring one never got off the ground.
//!
//! So everything asked for here is asked as Lua. What carries it is
//! `console-compositor`, which is where this desktop keeps hyprctl: the fork
//! this was ported from opened the socket itself and had a second copy of where
//! that socket lives, which is the fault that crate exists to end.
//!
//! What stays is the spelling. A selector, a rule table and an escaped string
//! are this program's words for its own saved file, and the general crate has no
//! business knowing what `fullscreenstate` used to be called. `quote` is the one
//! that will move: window titles and command lines arrive from other people's
//! programs, and the first other caller that puts one into Lua is when escaping
//! stops being ours and becomes the compositor crate's.

use console_compositor::{Done, Told};
use console_core_never::Never;

pub fn eval(lua: &str) -> Result<Done, Never> {
    console_compositor::told(Told::Eval, lua)
}

pub fn dispatch(dispatcher: &str) -> Result<Done, Never> {
    eval(&format!("hl.dispatch({dispatcher})"))
}

pub fn exec(command: &str, rules: &str) -> Result<Done, Never> {
    let Ok(quoted) = quote(command);

    eval(&format!("hl.exec_cmd({quoted}, {{{rules}}})"))
}

pub fn window(address: &str) -> Result<String, Never> {
    quote(&format!("address:{address}"))
}

pub fn quote(text: &str) -> Result<String, Never> {
    let mut quoted = String::with_capacity(text.len().saturating_add(2));
    quoted.push('"');

    for character in text.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            plain => match u32::from(plain) < 0x20 {
                true => quoted.push_str(&format!("\\{:03}", u32::from(plain))),
                false => quoted.push(plain),
            },
        }
    }

    quoted.push('"');

    Ok(quoted)
}

pub fn rules_table(prefix: &str) -> Result<String, Never> {
    Ok(prefix
        .split(';')
        .map(str::trim)
        .filter(|rule| !rule.is_empty())
        .map(|rule| {
            let named = |effect: &str| {
                let Ok(spelt) = effect_name(effect);
                let Ok(quoted) = quote(&spelt);

                quoted
            };

            let valued = |value: &str| {
                let Ok(quoted) = quote(value);

                quoted
            };

            match rule.split_once(' ') {
                Some((effect, value)) => format!("[{}] = {}", named(effect), valued(value)),
                None => format!("[{}] = true", named(rule)),
            }
        })
        .collect::<Vec<String>>()
        .join(", "))
}

fn effect_name(legacy: &str) -> Result<String, Never> {
    Ok(match legacy {
        "fullscreenstate" => "fullscreen_state".to_string(),
        "noinitialfocus" => "no_initial_focus".to_string(),
        "keepaspectratio" => "keep_aspect_ratio".to_string(),
        "maxsize" => "max_size".to_string(),
        "minsize" => "min_size".to_string(),
        other => other.to_string(),
    })
}

pub fn split_exec_line(line: &str) -> Result<(String, &str), Never> {
    let line = line.trim();

    let inside = match line.strip_prefix('[') {
        Some(rest) => rest.split_once(']'),
        None => None,
    };

    match inside {
        Some((prefix, command)) => {
            let Ok(rules) = rules_table(prefix);

            Ok((rules, command.trim()))
        },
        None => Ok((String::new(), line)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_what_would_otherwise_end_the_string() {
        assert_eq!(quote(r#"say "hi""#), Ok(r#""say \"hi\"""#.to_string()));
        assert_eq!(quote(r"C:\path"), Ok(r#""C:\\path""#.to_string()));
        assert_eq!(quote("two\nlines"), Ok(r#""two\nlines""#.to_string()));
    }

    #[test]
    fn a_character_no_keyboard_types_is_spelled_rather_than_sent() {
        assert_eq!(quote("a\u{7}b"), Ok(r#""a\007b""#.to_string()));
    }

    #[test]
    fn reads_a_rule_prefix_as_a_lua_table() {
        assert_eq!(
            rules_table("monitor 0;workspace 2 silent;float"),
            Ok(r#"["monitor"] = "0", ["workspace"] = "2 silent", ["float"] = true"#.to_string())
        );
    }

    #[test]
    fn renames_the_effects_lua_spells_differently() {
        assert_eq!(rules_table("fullscreenstate 0"), Ok(r#"["fullscreen_state"] = "0""#.to_string()));
    }

    #[test]
    fn splits_a_line_into_rules_and_command() {
        assert_eq!(
            split_exec_line("[workspace 2 silent] foot --working-directory=/home"),
            Ok((r#"["workspace"] = "2 silent""#.to_string(), "foot --working-directory=/home"))
        );
        assert_eq!(split_exec_line("foot"), Ok((String::new(), "foot")));
    }

    #[test]
    fn a_window_is_named_to_a_dispatcher_by_the_address_it_answers_to() {
        assert_eq!(window("0x55f1"), Ok(r#""address:0x55f1""#.to_string()));
    }
}
