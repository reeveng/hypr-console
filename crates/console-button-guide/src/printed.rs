//! The guide, printed.
//!
//! Named by number, not by shade: 35 and 37 are whatever the terminal's palette
//! says magenta and white are, which on this machine is the pink and the quiet
//! color every other surface uses. The dim attribute is not used anywhere
//! here. It halves whatever it is applied to, and half of a color chosen to
//! clear 7:1 is a color that does not.

use console_core_never::Never;
use console_core_number_conversion::index;
use crate::guide::Section;

pub const RULE: u32 = 46;

pub const COLUMN: u32 = 22;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HexColor {
    pub bold: &'static str,
    pub quiet: &'static str,
    pub pink: &'static str,
    pub off: &'static str,
}

pub const COLORED: HexColor =
    HexColor { bold: "\u{1b}[1m", quiet: "\u{1b}[37m", pink: "\u{1b}[35m", off: "\u{1b}[0m" };

pub const PLAIN: HexColor = HexColor { bold: "", quiet: "", pink: "", off: "" };

pub fn guide(sections: &[Section], ink: HexColor) -> Result<String, Never> {
    let mut said = format!("\n{}The buttons on this device{}\n", ink.bold, ink.off);
    let Ok(rule) = index(RULE);
    let Ok(column) = index(COLUMN);

    for section in sections.iter().filter(|section| !section.lines.is_empty()) {
        said.push_str(&format!(
            "\n{}{}{}{}\n{}{}{}\n",
            ink.pink,
            ink.bold,
            section.title,
            ink.off,
            ink.quiet,
            "\u{2500}".repeat(rule),
            ink.off
        ));

        for line in &section.lines {
            said.push_str(&format!(
                "  {}{:<column$}{}{}\n",
                ink.bold, line.button, ink.off, line.does
            ));
        }
    }

    said.push_str(&format!(
        "\n{}  Not sure which paddle is which? Run:  console-buttons --identify{}\n\n",
        ink.quiet, ink.off
    ));
    Ok(said)
}

#[cfg(test)]
mod tests {
    use super::*;
    use console_input_controller::actions::Table;

    use crate::guide::{Line, TYPED, sections};

    fn ours() -> Table {
        let Ok(table) = Table::ours();

        table
    }

    fn said() -> String {
        let Ok(sections) = sections(&ours());
        let Ok(said) = guide(&sections, PLAIN);

        said
    }

    #[test]
    fn a_terminal_that_is_not_one_is_given_no_escapes() {
        assert!(!said().contains('\u{1b}'), "an escape reached something reading a file");
    }

    #[test]
    fn every_line_is_the_button_and_what_it_does() {
        assert!(said().contains("  Touchpad              move the pointer"));
    }

    #[test]
    fn a_section_with_nothing_in_it_is_not_printed() {
        let Ok(mut every) = sections(&ours());

        every.push(Section { title: "Nothing at all".to_string(), lines: Vec::new() });

        let Ok(said) = guide(&every, PLAIN);

        assert!(!said.contains("Nothing at all"));
    }

    #[test]
    fn a_section_with_something_in_it_is() {
        let Ok(mut every) = sections(&ours());
        let line = Line { button: "Super Q".to_string(), does: "close".to_string() };

        every.last_mut().expect("a section").lines.push(line);

        let Ok(said) = guide(&every, PLAIN);

        assert!(said.contains(TYPED));
    }
}
