//! Every colour as it was solved, read out of the report.
//!
//! Read rather than resolved, because the engine that solves it is
//! `console-palette`. Resolving it a second time here would be a second place a
//! colour could be decided, which is the thing this whole arrangement exists
//! to prevent. `theme/report.md` is the palette written down, and a test in
//! `console-palette` fails if it has fallen behind what `theme/palette.toml`
//! says.

use std::collections::BTreeMap;

use console_core_never::Never;

fn row(line: &str) -> Result<Option<(String, String)>, Never> {
    let Some(row) = line.strip_prefix("| ") else { return Ok(None) };

    let mut fields = row.split(" | ");

    let Some(said) = fields.next() else { return Ok(None) };

    let name = said.trim_matches('`');

    let Some(written) = fields.next() else { return Ok(None) };

    let Some(code) = written.trim_matches('`').strip_prefix('#') else { return Ok(None) };

    let sound = |text: &str, of: &dyn Fn(char) -> bool| !text.is_empty() && text.chars().all(of);
    let named = sound(name, &|letter| {
        letter.is_ascii_alphanumeric() || letter == '_'
    });
    let coloured = code.len() == 6 && sound(code, &|digit| digit.is_ascii_hexdigit());

    Ok((named && coloured).then(|| (name.to_string(), code.to_string())))
}

pub fn read(report: &str) -> Result<BTreeMap<String, String>, String> {
    let mut colours: BTreeMap<String, String> = BTreeMap::new();

    for line in report.lines() {
        let Ok(row) = row(line);

        match row {
            Some((name, code)) => {
                let _ = colours.insert(name, code);
            }
            None => {},
        }
    }

    match colours.is_empty() {
        true => Err("theme/report.md holds no colours; run `just theme`".to_string()),
        false => Ok(colours),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_is_read_out_of_a_row_of_the_table() {
        assert_eq!(
            row("| `night` | `#110b12` | the deepest ground |"),
            Ok(Some(("night".to_string(), "110b12".to_string())))
        );
    }

    #[test]
    fn a_row_that_is_not_a_colour_is_not_one() {
        assert_eq!(row("| name | ratio | where |"), Ok(None));
        assert_eq!(row("| `night` | `#110b1` | short |"), Ok(None));
        assert_eq!(row("nothing at all"), Ok(None));
    }

    #[test]
    fn a_report_holding_no_colours_says_to_run_the_theme() {
        assert!(read("# nothing here\n").is_err());
    }
}
