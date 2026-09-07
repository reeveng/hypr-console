//! The binds for when a real keyboard is plugged in, read out of the
//! compositor's own configuration.

use console_core_external_programs::Program;
use console_core_never::Never;

pub fn hyprctl() -> Result<[&'static str; 2], Never> {
    let Ok(name) = Program::Hyprctl.name();

    Ok([name, "dispatch"])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bind {
    pub keys: String,
    pub does: String,
    pub runs: Vec<String>,
}

pub fn binds(lua: &str) -> Result<Vec<Bind>, Never> {
    Ok(lua
        .lines()
        .filter_map(|line| {
            let Ok(one) = one(line);

            one
        })
        .collect())
}

fn one(line: &str) -> Result<Option<Bind>, Never> {
    let Some((_, rest)) = line.split_once("hl.bind(") else { return Ok(None) };

    let Some(at) = rest.find("hl.dsp.") else { return Ok(None) };

    let (keys, from) = rest.split_at(at);

    let Ok(Some(dispatch)) = whole(from) else { return Ok(None) };

    let Some(does) = dispatch.strip_prefix("hl.dsp.") else { return Ok(None) };

    let Ok(command) = command(does);
    let Ok(keys) = said(keys.trim().trim_end_matches(','));
    let Ok(says) = match &command {
        Some(argv) => named(argv),
        None => dispatched(does),
    };
    let runs = match command {
        Some(argv) => argv,
        None => {
            let Ok(through) = through(dispatch);

            through
        },
    };

    Ok(Some(Bind { keys, does: says, runs }))
}

fn whole(from: &str) -> Result<Option<&str>, Never> {
    let mut depth = 0i32;
    let mut quoted = false;

    for (at, letter) in from.char_indices() {
        match letter {
            '"' => quoted = !quoted,
            _ if quoted => (),
            '(' => depth = depth.saturating_add(1),
            ')' if depth == 1 => return Ok(from.get(..=at)),
            ')' => depth = depth.saturating_sub(1),
            _ => (),
        }
    }

    Ok(None)
}

fn said(keys: &str) -> Result<String, Never> {
    let keys = keys.replace("mod .. \"", "Super").replace('"', "");
    Ok(keys.split_whitespace().collect::<Vec<&str>>().join(" "))
}

fn command(does: &str) -> Result<Option<Vec<String>>, Never> {
    let Some((_, rest)) = does.split_once("exec_cmd(\"") else { return Ok(None) };

    let Some((inside, _)) = rest.split_once('"') else { return Ok(None) };

    let argv: Vec<String> = inside.split_whitespace().map(str::to_string).collect();

    Ok(match argv.is_empty() {
        true => None,
        false => Some(argv),
    })
}

fn through(dispatch: &str) -> Result<Vec<String>, Never> {
    let Ok(hyprctl) = hyprctl();

    Ok(hyprctl.iter().map(|word| (*word).to_string()).chain([dispatch.to_string()]).collect())
}

fn named(argv: &[String]) -> Result<String, Never> {
    let said: Vec<&str> = argv
        .iter()
        .enumerate()
        .map(|(at, word)| match at {
            0 => word.rsplit('/').next().unwrap_or(word),
            _ => word.as_str(),
        })
        .collect();
    Ok(format!("run {}", said.join(" ")))
}

fn dispatched(does: &str) -> Result<String, Never> {
    Ok(does.split('(').next().unwrap_or(does).replace("window.", "").replace('_', " "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binds(lua: &str) -> Vec<Bind> {
        let Ok(binds) = super::binds(lua);

        binds
    }

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

    #[test]
    fn a_bind_that_runs_something_is_named_by_the_program() {
        assert_eq!(binds(LUA)[1].does, "run launcher");
        assert_eq!(binds(LUA)[1].keys, "Super SHIFT, Return");
    }

    #[test]
    fn a_bind_that_runs_something_carries_what_to_run() {
        assert_eq!(binds(LUA)[1].runs, ["/usr/local/bin/launcher"]);
    }

    #[test]
    fn a_command_with_words_after_it_keeps_them() {
        assert_eq!(binds(LUA)[4].does, "run console-volume up");
        assert_eq!(binds(LUA)[4].runs, ["console-volume", "up"]);
    }

    #[test]
    fn a_bind_that_dispatches_is_asked_for_through_hyprctl() {
        let Ok(hyprctl) = Program::Hyprctl.name();

        assert_eq!(binds(LUA)[0].runs, [hyprctl, "dispatch", "hl.dsp.window.close()"]);
        assert_eq!(binds(LUA)[2].runs, [hyprctl, "dispatch", "hl.dsp.toggle_floating()"]);
    }

    #[test]
    fn a_dispatch_carrying_a_table_is_read_to_the_end_of_it() {
        assert_eq!(binds(LUA)[3].does, "focus");
        assert_eq!(binds(LUA)[3].runs[2], r#"hl.dsp.focus({ direction = "left" })"#);
    }

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
