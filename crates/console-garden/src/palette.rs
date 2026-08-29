//! Every colour as it was solved, read out of the report.
//!
//! Read rather than resolved, because the engine that solves it is
//! `console-theme`. Resolving it a second time here would be a second place a
//! colour could be decided, which is the thing this whole arrangement exists
//! to prevent. `theme/report.md` is the palette written down, and a test in
//! `console-theme` fails if it has fallen behind what `theme/palette.toml`
//! says.

use std::collections::BTreeMap;

/// One row of the report's table, if it is one: a name and a hexcode.
fn row(line: &str) -> Option<(String, String)> {
    let mut fields = line.strip_prefix("| ")?.split(" | ");
    let name = fields.next()?.trim_matches('`');
    let code = fields.next()?.trim_matches('`').strip_prefix('#')?;
    let sound = |text: &str, of: &dyn Fn(char) -> bool| !text.is_empty() && text.chars().all(of);
    let named = sound(name, &|letter| {
        letter.is_ascii_alphanumeric() || letter == '_'
    });
    let coloured = code.len() == 6 && sound(code, &|digit| digit.is_ascii_hexdigit());
    (named && coloured).then(|| (name.to_string(), code.to_string()))
}

/// The whole palette, out of the report's table.
pub fn read(report: &str) -> Result<BTreeMap<String, String>, String> {
    let colours: BTreeMap<String, String> = report.lines().filter_map(row).collect();
    match colours.is_empty() {
        true => Err("theme/report.md holds no colours; run `make theme`".to_string()),
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
            Some(("night".to_string(), "110b12".to_string()))
        );
    }

    #[test]
    fn a_row_that_is_not_a_colour_is_not_one() {
        assert_eq!(row("| name | ratio | where |"), None);
        assert_eq!(row("| `night` | `#110b1` | short |"), None);
        assert_eq!(row("nothing at all"), None);
    }

    #[test]
    fn a_report_holding_no_colours_says_to_run_the_theme() {
        assert!(read("# nothing here\n").is_err());
    }
}
