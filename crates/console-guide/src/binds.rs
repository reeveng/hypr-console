//! The binds for when a real keyboard is plugged in, read out of the
//! compositor's own configuration.

/// What the compositor is told through, when a bind is asked for from a panel
/// rather than from the keyboard nobody has plugged in.
pub const HYPRCTL: [&str; 2] = ["hyprctl", "dispatch"];

/// One bind: the keys, what they do, and what doing it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bind {
    pub keys: String,
    pub does: String,
    /// The command that does the same thing: what the bind starts, or the
    /// dispatch handed to the compositor.
    pub runs: Vec<String>,
}

/// Every `hl.bind(..., hl.dsp....)` in the configuration, said in words.
pub fn binds(lua: &str) -> Vec<Bind> {
    lua.lines().filter_map(one).collect()
}

fn one(line: &str) -> Option<Bind> {
    let rest = line.split_once("hl.bind(")?.1;
    // Split at what it does rather than at the comma. A bind on two keys has
    // a comma of its own between them, and the first one is not the seam.
    let at = rest.find("hl.dsp.")?;
    let (keys, from) = rest.split_at(at);
    let dispatch = whole(from)?;
    let does = dispatch.strip_prefix("hl.dsp.")?;
    let command = command(does);
    Some(Bind {
        keys: said(keys.trim().trim_end_matches(',')),
        does: match &command {
            Some(argv) => named(argv),
            None => dispatched(does),
        },
        runs: command.unwrap_or_else(|| through(dispatch)),
    })
}

/// The dispatch, from `hl.dsp.` to the bracket that closes it.
///
/// Counted rather than cut at the last bracket on the line. A bind carrying
/// options has a table after the dispatch with brackets of its own, and a
/// dispatch read as far as those is one the compositor will not answer to.
fn whole(from: &str) -> Option<&str> {
    let mut depth = 0i32;
    let mut quoted = false;
    for (at, letter) in from.char_indices() {
        match letter {
            '"' => quoted = !quoted,
            _ if quoted => (),
            '(' => depth += 1,
            ')' if depth == 1 => return Some(&from[..=at]),
            ')' => depth -= 1,
            _ => (),
        }
    }
    None
}

/// The keys, with the quoting and the modifier's own name taken out.
fn said(keys: &str) -> String {
    let keys = keys.replace("mod .. \"", "Super").replace('"', "");
    keys.split_whitespace().collect::<Vec<&str>>().join(" ")
}

/// The command a bind starts, in words, where it starts one.
fn command(does: &str) -> Option<Vec<String>> {
    let rest = does.split_once("exec_cmd(\"")?.1;
    let argv: Vec<String> =
        rest.split_once('"')?.0.split_whitespace().map(str::to_string).collect();
    match argv.is_empty() {
        true => None,
        false => Some(argv),
    }
}

/// A dispatch, asked for the way anything here asks for one.
fn through(dispatch: &str) -> Vec<String> {
    HYPRCTL.iter().map(|word| (*word).to_string()).chain([dispatch.to_string()]).collect()
}

/// A command, named by its program. A path is a fact about where something is
/// kept, which is not what the key does.
fn named(argv: &[String]) -> String {
    let said: Vec<&str> = argv
        .iter()
        .enumerate()
        .map(|(at, word)| match at {
            0 => word.rsplit('/').next().unwrap_or(word),
            _ => word.as_str(),
        })
        .collect();
    format!("run {}", said.join(" "))
}

/// What a dispatch is called, said in words.
fn dispatched(does: &str) -> String {
    does.split('(').next().unwrap_or(does).replace("window.", "").replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const LUA: &str = r#"
hl.bind(mod .. "Q", hl.dsp.window.close())
hl.bind(mod .. " SHIFT, Return", hl.dsp.exec_cmd("/usr/local/bin/launcher"))
hl.bind(mod .. "T", hl.dsp.toggle_floating())
hl.bind(mod .. " left", hl.dsp.focus({ direction = "left" }))
hl.bind("XF86AudioRaiseVolume", hl.dsp.exec_cmd("console-volume up"), { locked = true })
hl.bind("XF86Sleep", hl.dsp.window.close(), { locked = true, repeating = true })
local something = 3
"#;

    #[test]
    fn a_bind_is_the_keys_and_what_they_do() {
        assert_eq!(binds(LUA)[0].keys, "SuperQ");
        assert_eq!(binds(LUA)[0].does, "close");
    }

    /// A path is a fact about where a program is kept, which is not what the
    /// key does.
    #[test]
    fn a_bind_that_runs_something_is_named_by_the_program() {
        assert_eq!(binds(LUA)[1].does, "run launcher");
        assert_eq!(binds(LUA)[1].keys, "Super SHIFT, Return");
    }

    /// Reading the keys off a guide and then reaching for a keyboard that is
    /// not plugged in is the one thing the guide could have done for you.
    #[test]
    fn a_bind_that_runs_something_carries_what_to_run() {
        assert_eq!(binds(LUA)[1].runs, ["/usr/local/bin/launcher"]);
    }

    /// What is said keeps the arguments and drops the directory; what is run
    /// keeps both.
    #[test]
    fn a_command_with_words_after_it_keeps_them() {
        assert_eq!(binds(LUA)[4].does, "run console-volume up");
        assert_eq!(binds(LUA)[4].runs, ["console-volume", "up"]);
    }

    /// A bind that tells the compositor something is asked for the same way,
    /// so every line in the guide is a line that can be chosen.
    #[test]
    fn a_bind_that_dispatches_is_asked_for_through_hyprctl() {
        assert_eq!(binds(LUA)[0].runs, ["hyprctl", "dispatch", "hl.dsp.window.close()"]);
        assert_eq!(binds(LUA)[2].runs, ["hyprctl", "dispatch", "hl.dsp.toggle_floating()"]);
    }

    /// A dispatch given a table is brackets inside brackets, and the whole of
    /// it is the dispatch.
    #[test]
    fn a_dispatch_carrying_a_table_is_read_to_the_end_of_it() {
        assert_eq!(binds(LUA)[3].does, "focus");
        assert_eq!(binds(LUA)[3].runs[2], r#"hl.dsp.focus({ direction = "left" })"#);
    }

    /// The options a bind is given are the bind's, not the dispatch's. Read as
    /// far as their closing bracket, the compositor is handed nonsense.
    #[test]
    fn what_the_bind_was_given_is_not_part_of_the_dispatch() {
        assert_eq!(binds(LUA)[5].runs[2], "hl.dsp.window.close()");
    }

    #[test]
    fn a_line_that_is_not_a_bind_is_not_one() {
        assert_eq!(binds(LUA).len(), 6);
        assert!(binds("").is_empty());
    }
}
